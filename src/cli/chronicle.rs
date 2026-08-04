//! CHRONICLE-1 (CH-P1) — `inkhaven chronicle {mark, list}`.
//!
//! `mark` captures the current draft state (the readers' metrics + finding set)
//! as a named milestone; `list` shows the captured milestones. The trend/diff
//! report (`chronicle` bare, `chronicle diff`) and the cleared/introduced hook land
//! in CH-P2/P3.

use std::path::Path;

use crate::chronicle::capture;
use crate::chronicle::store::ChronicleStore;
use crate::chronicle::Milestone;
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
