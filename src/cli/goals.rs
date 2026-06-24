//! `inkhaven goals` — the writing-goals terminal report (road to 1.4.0).
//!
//! The `Ctrl+V g` progress modal already shows the full picture inside the TUI;
//! this is its missing terminal counterpart, mirroring `inkhaven cost` /
//! `inkhaven stats`. It reuses the exact same `progress::build_snapshot` engine —
//! no goal logic is duplicated here, only a plain-text renderer.
//!
//! Read-only: it opens `progress.db` and walks the hierarchy for live word
//! totals, then aggregates. It records nothing and changes no config.

use std::collections::HashMap;
use std::path::Path;

use uuid::Uuid;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::progress::aggregates::{build_snapshot, ProgressSnapshot};
use crate::progress::word_count::count_words;
use crate::progress::LiveTotals;
use crate::progress::store::ProgressStore;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;
use crate::store::Store;

/// Walk the hierarchy and total words per **user** book (system books are
/// inkhaven internals, never manuscript content). Mirrors the TUI's
/// `compute_word_totals_now` so the CLI and the modal agree to the word.
fn live_totals(layout: &ProjectLayout, h: &Hierarchy) -> LiveTotals {
    let mut per_book: HashMap<Uuid, i64> = HashMap::new();
    let mut book_titles: HashMap<Uuid, String> = HashMap::new();
    let mut book_slugs: HashMap<Uuid, String> = HashMap::new();
    for (node, _) in h.flatten() {
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let book = h
            .ancestors(node)
            .into_iter()
            .find(|a| a.kind == NodeKind::Book)
            .filter(|b| b.system_tag.is_none());
        let Some(book) = book else { continue };
        let Some(rel) = node.file.as_ref() else { continue };
        let body = std::fs::read_to_string(layout.root.join(rel)).unwrap_or_default();
        *per_book.entry(book.id).or_insert(0) += count_words(&body);
        book_titles.entry(book.id).or_insert_with(|| book.title.clone());
        book_slugs
            .entry(book.id)
            .or_insert_with(|| book.slug.clone());
    }
    let project_total = per_book.values().sum();
    LiveTotals {
        per_book,
        project_total,
        book_titles,
        book_slugs,
    }
}

/// Eight-level text sparkline scaled to the series max. Negative days (net
/// deletions) render as the baseline tick so the row stays one cell wide.
fn sparkline(series: &[i64]) -> String {
    const TICKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let max = series.iter().copied().max().unwrap_or(0).max(1);
    series
        .iter()
        .map(|&v| {
            if v <= 0 {
                '▁'
            } else {
                let idx = ((v * 7) / max).clamp(0, 7) as usize;
                TICKS[idx]
            }
        })
        .collect()
}

/// A 20-cell progress bar with trailing percent, matching `cost::bar`.
fn bar(done: i64, goal: i64) -> String {
    if goal <= 0 {
        return String::new();
    }
    const WIDTH: i64 = 20;
    let done = done.max(0);
    let filled = ((done * WIDTH) / goal).clamp(0, WIDTH);
    let pct = (done * 100 / goal).clamp(0, 999);
    format!(
        "[{}{}] {pct}%",
        "█".repeat(filled as usize),
        "░".repeat((WIDTH - filled) as usize)
    )
}

fn hms(seconds: i64) -> String {
    let m = (seconds / 60).max(0);
    if m >= 60 {
        format!("{}h{:02}m", m / 60, m % 60)
    } else {
        format!("{m}m")
    }
}

/// Render the snapshot as plain lines (the CLI body; kept separate so it is
/// unit-testable without a project on disk).
pub fn render_lines(snap: &ProgressSnapshot, day: &str) -> Vec<String> {
    let mut out = vec![format!("Writing goals — {day} (UTC)"), String::new()];

    // Project: total + today vs daily goal.
    let p = &snap.project;
    out.push(format!("  project total       {:>8} words", p.total_words));
    match p.daily_goal {
        Some(goal) => out.push(format!(
            "  today               {:>8} / {:<6}  {}",
            p.today_words,
            goal,
            bar(p.today_words, goal)
        )),
        None => out.push(format!(
            "  today               {:>8} words  (set goals.daily_words for a target)",
            p.today_words
        )),
    }

    // Streak.
    let s = &snap.streak;
    let grace = if s.grace_per_week > 0 {
        format!("  (grace {}/{} per week used)", s.grace_used, s.grace_per_week)
    } else {
        String::new()
    };
    let best = if s.best > s.days {
        format!("  ·  best {} days", s.best)
    } else {
        String::new()
    };
    out.push(format!("  streak              {:>8} days{best}{grace}", s.days));

    // Active time.
    out.push(format!(
        "  active time         today {}   ·   week {}",
        hms(snap.active_seconds_today),
        hms(snap.active_seconds_week)
    ));

    // Sparkline.
    if !snap.sparkline.is_empty() {
        out.push(String::new());
        out.push(format!("  last {} days        {}", snap.sparkline.len(), sparkline(&snap.sparkline)));
    }

    // Per-book pacing.
    if !snap.books.is_empty() {
        out.push(String::new());
        out.push("  books:".into());
        for b in &snap.books {
            let mut line = format!("    {:<22} {:>7} words", truncate(&b.label, 22), b.total_words);
            if let Some(target) = b.target_words {
                let pct = if target > 0 { (b.total_words * 100 / target).clamp(0, 999) } else { 0 };
                line.push_str(&format!(" / {target} ({pct}%)"));
            }
            if let (Some(pace), Some(dd)) = (b.required_pace, b.days_to_deadline) {
                if dd >= 0 {
                    line.push_str(&format!("  · need {pace}/day, {dd}d left"));
                } else {
                    line.push_str(&format!("  · {} overdue, {pace} to go", -dd));
                }
            }
            out.push(line);
        }
    }

    // Status ladder — this week's promotions vs the per-status weekly goal.
    if !snap.status.recent.is_empty() || !snap.status.goals.is_empty() {
        let goals: HashMap<&str, i64> =
            snap.status.goals.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        let recent: HashMap<&str, i64> =
            snap.status.recent.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        out.push(String::new());
        out.push("  status promotions (last 7 days):".into());
        // Union of both keysets, stable-sorted for deterministic output.
        let mut keys: Vec<&str> = recent.keys().chain(goals.keys()).copied().collect();
        keys.sort_unstable();
        keys.dedup();
        for k in keys {
            let got = recent.get(k).copied().unwrap_or(0);
            match goals.get(k).copied().filter(|g| *g > 0) {
                Some(g) => out.push(format!("    {:<14} {:>3} / {:<3}  {}", k, got, g, bar(got, g))),
                None => out.push(format!("    {:<14} {:>3}", k, got)),
            }
        }
    }

    out.push(String::new());
    out.push("  Goals are informative. Resets at 00:00 UTC. (TUI: Ctrl+V g)".into());
    out
}

fn truncate(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else {
        let cut: String = chars.into_iter().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

pub fn run(project: &Path) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;

    let live = live_totals(&layout, &h);
    let progress = ProgressStore::open(&layout.root.join("progress.db"))
        .map_err(|e| Error::Store(format!("goals: progress store: {e}")))?;
    let snap = build_snapshot(&progress, &cfg.goals, &live)
        .map_err(|e| Error::Store(format!("goals: build snapshot: {e}")))?;

    let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
    for line in render_lines(&snap, &day) {
        println!("{line}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progress::aggregates::{BookProgress, StatusLadderCounts, StreakStatus};

    fn snap() -> ProgressSnapshot {
        ProgressSnapshot {
            project: BookProgress {
                label: "project".into(),
                today_words: 450,
                daily_goal: Some(500),
                total_words: 42_000,
                target_words: None,
                required_pace: None,
                days_to_deadline: None,
            },
            books: vec![BookProgress {
                label: "The Long Road".into(),
                today_words: 450,
                daily_goal: Some(500),
                total_words: 42_000,
                target_words: Some(80_000),
                required_pace: Some(633),
                days_to_deadline: Some(60),
            }],
            status: StatusLadderCounts {
                recent: vec![("ready".into(), 3)],
                goals: vec![("ready".into(), 5)],
            },
            streak: StreakStatus { days: 12, grace_used: 1, grace_per_week: 1, best: 30 },
            sparkline: vec![0, 100, 250, 0, 480, 600, 300],
            active_seconds_today: 5_400,
            active_seconds_week: 26_000,
        }
    }

    #[test]
    fn renders_core_sections() {
        let lines = render_lines(&snap(), "2026-06-24");
        let blob = lines.join("\n");
        assert!(blob.contains("Writing goals — 2026-06-24"));
        assert!(blob.contains("project total"));
        assert!(blob.contains("450 / 500")); // today vs daily goal
        assert!(blob.contains("streak                    12 days"));
        assert!(blob.contains("best 30 days"));
        assert!(blob.contains("grace 1/1"));
        assert!(blob.contains("The Long Road"));
        assert!(blob.contains("need 633/day, 60d left"));
        assert!(blob.contains("status promotions"));
        assert!(blob.contains("ready"));
        assert!(blob.contains("1h30m")); // active today 5400s
    }

    #[test]
    fn bar_clamps() {
        assert_eq!(bar(0, 0), "");
        assert!(bar(250, 500).ends_with("50%"));
        assert!(bar(1000, 500).ends_with("200%")); // honest over-goal
    }

    #[test]
    fn sparkline_scales_and_floors_negatives() {
        let sp = sparkline(&[0, -5, 100]);
        assert_eq!(sp.chars().count(), 3);
        assert!(sp.starts_with('▁')); // zero and negative → baseline
        assert!(sp.ends_with('█')); // series max → full
    }

    #[test]
    fn empty_snapshot_still_renders() {
        let lines = render_lines(&ProgressSnapshot::empty(), "2026-06-24");
        assert!(lines.iter().any(|l| l.contains("Writing goals")));
    }
}
