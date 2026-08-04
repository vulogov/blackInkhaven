//! CHRONICLE-1 (CH-P1) — `inkhaven chronicle {mark, list}`.
//!
//! `mark` captures the current draft state (the readers' metrics + finding set)
//! as a named milestone; `list` shows the captured milestones. The trend/diff
//! report (`chronicle` bare, `chronicle diff`) and the cleared/introduced hook land
//! in CH-P2/P3.

use std::path::Path;

use crate::chronicle::store::ChronicleStore;
use crate::chronicle::{
    capture, diff_findings, diff_vectors, Direction, FindingDiff, FindingRef, Milestone, Trend,
    TrendDelta,
};
use crate::error::{Error, Result};

/// Capture a draft milestone now and persist it.
pub fn mark(
    project: &Path,
    label: &str,
    git_ref: Option<&str>,
    book_name: Option<&str>,
) -> Result<()> {
    if label.trim().is_empty() {
        return Err(Error::Store("chronicle mark: a milestone label is required".into()));
    }
    let (metrics, findings) = capture(project, book_name)?;
    let book_slug = resolve_book_slug(project, book_name)?;
    let milestone = Milestone {
        id: uuid::Uuid::new_v4(),
        label: label.trim().to_string(),
        day: crate::dayclock::today_days(),
        ts: crate::dayclock::now_secs(),
        book_slug,
        git_ref: git_ref.map(str::to_string),
        metrics: metrics.clone(),
    };
    let store = ChronicleStore::open_for_project(project).map_err(store_err)?;
    store.insert_milestone(&milestone, &findings).map_err(store_err)?;
    println!(
        "\u{2713} marked \u{201c}{}\u{201d} — {} finding(s) ({} error · {} warn · {} info)",
        milestone.label, metrics.total, metrics.errors, metrics.warnings, metrics.infos
    );
    Ok(())
}

/// List captured milestones (newest first).
pub fn list(project: &Path, book_name: Option<&str>, json: bool) -> Result<()> {
    let store = ChronicleStore::open_for_project(project).map_err(store_err)?;
    let book_slug = resolve_book_slug(project, book_name)?;
    let milestones = store.list_milestones(book_slug.as_deref()).map_err(store_err)?;

    if json {
        let rows: Vec<serde_json::Value> = milestones
            .iter()
            .map(|m| {
                serde_json::json!({
                    "label": m.label,
                    "date": fmt_date(m.ts),
                    "book": m.book_slug,
                    "git_ref": m.git_ref,
                    "metrics": m.metrics,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows).unwrap_or_else(|_| "[]".into()));
        return Ok(());
    }

    if milestones.is_empty() {
        println!("no milestones yet — `inkhaven chronicle mark <label>` captures the first.");
        return Ok(());
    }
    for m in &milestones {
        let scope = m.book_slug.as_deref().map(|b| format!(" · {b}")).unwrap_or_default();
        let git = m.git_ref.as_deref().map(|r| format!(" @{r}")).unwrap_or_default();
        println!(
            "{}  {:<18} {} finding(s)  {}✗ {}⚠ {}·{scope}{git}",
            fmt_date(m.ts),
            m.label,
            m.metrics.total,
            m.metrics.errors,
            m.metrics.warnings,
            m.metrics.infos,
        );
    }
    Ok(())
}

/// The trend since the last milestone: capture the live state and diff it against
/// the most recent mark (the "did it get better since I last looked" view).
pub fn trend(project: &Path, book_name: Option<&str>, json: bool) -> Result<()> {
    let store = ChronicleStore::open_for_project(project).map_err(store_err)?;
    let book_slug = resolve_book_slug(project, book_name)?;
    let Some(base) = store.latest(book_slug.as_deref()).map_err(store_err)? else {
        println!("no milestone yet — `inkhaven chronicle mark <label>` captures a baseline first.");
        return Ok(());
    };
    let (current, current_refs) = capture(project, book_name)?;
    let base_refs = store.findings_for(base.id).map_err(store_err)?;
    let t = diff_vectors(&base.metrics, &current);
    let fd = diff_findings(&base_refs, &current_refs);
    if json {
        println!("{}", trend_json(&base.label, fmt_date(base.ts), &t, &fd));
        return Ok(());
    }
    render_trend(&format!("since “{}” ({}) → now", base.label, fmt_date(base.ts)), &t, &fd);
    Ok(())
}

/// Diff two named milestones head-to-head (`a` → `b`).
pub fn diff(
    project: &Path,
    a: &str,
    b: &str,
    book_name: Option<&str>,
    json: bool,
) -> Result<()> {
    let store = ChronicleStore::open_for_project(project).map_err(store_err)?;
    let book_slug = resolve_book_slug(project, book_name)?;
    let by = |label: &str| -> Result<Milestone> {
        store
            .by_label(label, book_slug.as_deref())
            .map_err(store_err)?
            .ok_or_else(|| Error::Store(format!("chronicle diff: no milestone labelled “{label}”")))
    };
    let (ma, mb) = (by(a)?, by(b)?);
    let a_refs = store.findings_for(ma.id).map_err(store_err)?;
    let b_refs = store.findings_for(mb.id).map_err(store_err)?;
    let t = diff_vectors(&ma.metrics, &mb.metrics);
    let fd = diff_findings(&a_refs, &b_refs);
    if json {
        println!("{}", trend_json(&ma.label, mb.label.clone(), &t, &fd));
        return Ok(());
    }
    render_trend(&format!("“{}” → “{}”", ma.label, mb.label), &t, &fd);
    Ok(())
}

/// Shared `--json` body for `trend` / `diff`: the deltas + the three finding lists.
fn trend_json(from: &str, to_or_date: String, t: &Trend, fd: &FindingDiff) -> String {
    let finding = |f: &FindingRef| {
        serde_json::json!({
            "category": f.category,
            "severity": f.severity,
            "location": f.location,
            "message": f.message(),
        })
    };
    let v = serde_json::json!({
        "since": from,
        "to": to_or_date,
        "trend": t,
        "findings": {
            "cleared": fd.cleared.iter().map(finding).collect::<Vec<_>>(),
            "introduced": fd.introduced.iter().map(finding).collect::<Vec<_>>(),
            "persisted": fd.persisted.len(),
        },
    });
    serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".into())
}

fn render_trend(header: &str, t: &Trend, fd: &FindingDiff) {
    println!("Chronicle — {header}\n");
    for d in &t.headline {
        println!("  {}", fmt_delta(d));
    }
    if let (Some(o), Some(n)) = (t.old_intensity, t.new_intensity) {
        println!("  {:<16} {o:>4.2} → {n:<4.2}   (reading)", "intensity");
    }
    if t.categories.is_empty() {
        println!("\n  no category moved.");
    } else {
        println!("\n  by category:");
        for d in &t.categories {
            println!("    {}", fmt_delta(d));
        }
    }

    // The REDLINE hook: what the revision cleared, what it introduced.
    println!(
        "\n  \u{2713} {} cleared    \u{25b2} {} introduced    \u{b7} {} unchanged",
        fd.cleared.len(),
        fd.introduced.len(),
        fd.persisted.len()
    );
    if !fd.introduced.is_empty() {
        println!("\n  introduced (new since the last mark):");
        for f in &fd.introduced {
            println!("    {}", fmt_finding(f));
        }
    }
}

/// One introduced/cleared finding row: severity icon · category · location · head.
fn fmt_finding(f: &FindingRef) -> String {
    let icon = match f.severity.as_str() {
        "error" => "✗",
        "warn" => "⚠",
        _ => "·",
    };
    let loc = f.location.as_deref().unwrap_or("—");
    format!("{icon} {:<12} {:<10} {}", f.category, loc, head(f.message(), 56))
}

/// Char-safe truncation with an ellipsis (the finding message head).
fn head(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_string();
    }
    let cut = max.saturating_sub(1);
    format!("{}…", chars[..cut].iter().collect::<String>())
}

fn fmt_delta(d: &TrendDelta) -> String {
    let arrow = match d.direction {
        Direction::Better => "▼",
        Direction::Worse => "▲",
        Direction::Same => "·",
    };
    let note = if d.old == 0 && d.new > 0 {
        "  NEW"
    } else if d.new == 0 && d.old > 0 {
        "  cleared"
    } else {
        ""
    };
    format!("{:<16} {:>3} → {:>3}   {arrow}{note}", d.key, d.old, d.new)
}

/// `unix-seconds → "YYYY-MM-DD"` for the milestone rows.
fn fmt_date(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

fn store_err(e: anyhow::Error) -> Error {
    Error::Store(e.to_string())
}

/// The canonical book slug to scope a milestone by. `None` → whole project (no
/// hierarchy load needed). When a book is named, resolve it (slug or title) so
/// `mark` and `list` agree regardless of which form the writer typed.
fn resolve_book_slug(project: &Path, book_name: Option<&str>) -> Result<Option<String>> {
    let Some(name) = book_name else {
        return Ok(None);
    };
    let layout = crate::project::ProjectLayout::new(project);
    let cfg = crate::config::Config::load_layered(&layout.config_path())?;
    let store = crate::store::Store::open(layout, &cfg)?;
    let h = crate::store::hierarchy::Hierarchy::load(&store)?;
    let book = crate::cli::resolve_user_book(&h, Some(name), "chronicle").map_err(Error::Store)?;
    Ok(Some(book.slug.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(key: &str, old: i64, new: i64, direction: Direction) -> TrendDelta {
        TrendDelta { key: key.into(), old, new, direction }
    }

    #[test]
    fn fmt_delta_marks_arrows_new_and_cleared() {
        // improvement ▼
        assert!(fmt_delta(&d("echo", 4, 2, Direction::Better)).contains("▼"));
        // regression ▲, freshly introduced → NEW
        let worse = fmt_delta(&d("confusion", 0, 1, Direction::Worse));
        assert!(worse.contains("▲") && worse.contains("NEW"));
        // fell to zero → cleared
        let gone = fmt_delta(&d("co_location", 2, 0, Direction::Better));
        assert!(gone.contains("▼") && gone.contains("cleared"));
        // unchanged → neutral, no NEW/cleared tag
        let same = fmt_delta(&d("filter", 1, 1, Direction::Same));
        assert!(same.contains("·") && !same.contains("NEW") && !same.contains("cleared"));
    }
}
