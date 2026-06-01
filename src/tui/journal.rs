//! 1.2.16+ Phase A.2 — manuscript intelligence dashboard.
//!
//! Synthesis pane that unifies every metric inkhaven
//! has been collecting since 1.2.5 into a single view.
//! Opened via `Ctrl+V Shift+J` (View layer, J for
//! Journal).
//!
//! The dashboard runs as a snapshot: data is gathered
//! once at modal-open time, the user reads/scrolls,
//! optional `e` export writes the snapshot to a
//! markdown file.  Re-open for fresh numbers.
//!
//! This module owns:
//!   * Section data structs (`JournalSnapshot`,
//!     `WordCountSection`, etc.).
//!   * Snapshot construction (`compute_snapshot`).
//!   * Markdown export (`to_markdown`).
//!
//! Modal state + key handling lives in
//! `src/tui/app/journal_impl.rs`; rendering in
//! `src/tui/app/render/modals.rs`.
//!
//! Sections deferred to A.2.b (each adds reach into
//! existing surfaces — POV tracker, rhythm gauge,
//! style-warnings cache, AI history — without new
//! data infrastructure):
//!   * POV chip composition for the active book.
//!   * Rhythm gauge averaged across the project.
//!   * Filter-words flagged today.
//!   * Most recent AI critique line (first ~120
//!     chars).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalSnapshot {
    pub generated_at: DateTime<Utc>,
    pub word_count: WordCountSection,
    pub structure: StructureSection,
    pub threads: ThreadsSection,
    pub comments: CommentsSection,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WordCountSection {
    pub today: i64,
    pub total: i64,
    /// Project word-count goal from `project.
    /// word_count_goal` HJSON.  Zero when not set.
    pub goal: i64,
    /// `project.target_date` from HJSON, verbatim.
    pub target_date: String,
    /// Streak days from the progress store.  Days
    /// at the keyboard with a positive word delta.
    pub streak_days: i64,
    /// Active writing seconds today (capped per
    /// save→save gap to exclude AFK).
    pub active_seconds_today: i64,
    /// Same calculation over the trailing week.
    pub active_seconds_week: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StructureSection {
    /// Top-level user books (excludes system books
    /// like Characters / Places / Help).
    pub user_books: usize,
    /// Chapters across all user books.
    pub chapters: usize,
    /// Paragraphs across all user books.
    pub paragraphs: usize,
    /// Chapter word counts (one entry per chapter,
    /// across all user books).  Sorted by hierarchy
    /// order so the index in the modal matches the
    /// tree pane.
    pub chapter_word_counts: Vec<i64>,
    /// Mean of `chapter_word_counts`; 0.0 when empty.
    pub avg_chapter_words: f64,
    /// Standard deviation; 0.0 when fewer than 2
    /// chapters.
    pub stdev_chapter_words: f64,
    /// Coefficient of variation as a fraction
    /// (stdev / mean).  Zero when mean is zero.
    /// Drives the pacing verdict.
    pub cv: f64,
    /// One-word verdict derived from `cv` —
    /// `"steady"` / `"varied"` / `"choppy"`.
    pub pacing_verdict: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThreadsSection {
    /// Total thread chapters under the Threads
    /// system book.  Zero when the book isn't
    /// present (pre-1.2.14 projects).
    pub total: usize,
    /// Threads whose newest waypoint mtime is
    /// within the last `DORMANT_DAYS` days.
    pub active: usize,
    /// Threads whose newest waypoint is older than
    /// `DORMANT_DAYS` days OR has no waypoints
    /// at all.
    pub dormant: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommentsSection {
    /// Open comments across the project (resolved=false).
    pub open: usize,
    /// Resolved within the last 7 days, project-wide.
    pub resolved_this_week: usize,
    /// Total resolved (any time).  Lets the user
    /// see lifetime review momentum.
    pub resolved_total: usize,
}

/// Threshold for the dormant-thread heuristic.
/// Aligned with the 1.2.14 thread doctor's
/// dormant detector so the dashboard agrees with
/// `inkhaven thread doctor`.
pub const DORMANT_DAYS: u64 = 30;

/// Build a snapshot from the live App state.
/// `progress` may be `None` when no progress
/// cache has been built yet (very early in TUI
/// startup); the word-count section then reads
/// zeros + a "no data yet" note in the export.
pub fn compute_snapshot(
    progress: Option<&crate::progress::ProgressSnapshot>,
    hierarchy: &crate::store::hierarchy::Hierarchy,
    project_root: &std::path::Path,
    word_count_goal: u64,
    target_date: &str,
) -> JournalSnapshot {
    JournalSnapshot {
        generated_at: Utc::now(),
        word_count: word_count_section(progress, word_count_goal, target_date),
        structure: structure_section(hierarchy, project_root),
        threads: threads_section(hierarchy, project_root),
        comments: comments_section(hierarchy, project_root),
    }
}

fn word_count_section(
    progress: Option<&crate::progress::ProgressSnapshot>,
    goal: u64,
    target_date: &str,
) -> WordCountSection {
    let Some(p) = progress else {
        return WordCountSection {
            goal: goal as i64,
            target_date: target_date.to_string(),
            ..Default::default()
        };
    };
    WordCountSection {
        today: p.project.today_words,
        total: p.project.total_words,
        goal: goal as i64,
        target_date: target_date.to_string(),
        streak_days: p.streak.days,
        active_seconds_today: p.active_seconds_today,
        active_seconds_week: p.active_seconds_week,
    }
}

fn structure_section(
    hierarchy: &crate::store::hierarchy::Hierarchy,
    project_root: &std::path::Path,
) -> StructureSection {
    use crate::store::NodeKind;
    let mut user_books = 0usize;
    let mut chapters = 0usize;
    let mut paragraphs = 0usize;
    let mut chapter_word_counts: Vec<i64> = Vec::new();

    for node in hierarchy.iter() {
        match node.kind {
            NodeKind::Book => {
                if node.system_tag.is_none() {
                    user_books += 1;
                }
            }
            NodeKind::Chapter => {
                // Skip chapters under system books.
                let ancestors = hierarchy.ancestors(node);
                let under_system = ancestors.iter().any(|a| {
                    a.kind == NodeKind::Book && a.system_tag.is_some()
                });
                if under_system {
                    continue;
                }
                chapters += 1;
                let mut total = 0i64;
                for paragraph_id in hierarchy.collect_subtree(node.id) {
                    let Some(p) = hierarchy.get(paragraph_id) else { continue };
                    if p.kind != NodeKind::Paragraph {
                        continue;
                    }
                    let Some(rel) = p.file.as_ref() else { continue };
                    let abs = project_root.join(rel);
                    let Ok(body) = std::fs::read_to_string(&abs) else { continue };
                    total += crate::progress::count_words(&body);
                }
                chapter_word_counts.push(total);
            }
            NodeKind::Paragraph => {
                let ancestors = hierarchy.ancestors(node);
                let under_system = ancestors.iter().any(|a| {
                    a.kind == NodeKind::Book && a.system_tag.is_some()
                });
                if !under_system {
                    paragraphs += 1;
                }
            }
            _ => {}
        }
    }

    let (avg, stdev, cv, verdict) = pacing_stats(&chapter_word_counts);
    StructureSection {
        user_books,
        chapters,
        paragraphs,
        chapter_word_counts,
        avg_chapter_words: avg,
        stdev_chapter_words: stdev,
        cv,
        pacing_verdict: verdict.to_string(),
    }
}

/// Pure stats helper — extracted for unit testing.
/// Returns `(mean, stdev, cv, verdict)`.
pub fn pacing_stats(counts: &[i64]) -> (f64, f64, f64, &'static str) {
    if counts.is_empty() {
        return (0.0, 0.0, 0.0, "—");
    }
    let n = counts.len() as f64;
    let sum: i64 = counts.iter().sum();
    let mean = sum as f64 / n;
    let variance = if counts.len() < 2 {
        0.0
    } else {
        let sumsq: f64 = counts
            .iter()
            .map(|&x| {
                let d = x as f64 - mean;
                d * d
            })
            .sum();
        sumsq / (n - 1.0)
    };
    let stdev = variance.sqrt();
    let cv = if mean.abs() < f64::EPSILON {
        0.0
    } else {
        stdev / mean
    };
    let verdict = pacing_verdict_for(cv);
    (mean, stdev, cv, verdict)
}

fn pacing_verdict_for(cv: f64) -> &'static str {
    if cv < 0.20 {
        "steady"
    } else if cv < 0.50 {
        "varied"
    } else {
        "choppy"
    }
}

fn threads_section(
    hierarchy: &crate::store::hierarchy::Hierarchy,
    project_root: &std::path::Path,
) -> ThreadsSection {
    use crate::store::{NodeKind, SYSTEM_TAG_THREADS};
    let Some(threads_root) = hierarchy.iter().find(|n| {
        n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_THREADS)
    }) else {
        return ThreadsSection::default();
    };
    let threshold = std::time::SystemTime::now()
        - std::time::Duration::from_secs(DORMANT_DAYS * 86400);
    let mut total = 0usize;
    let mut active = 0usize;
    let mut dormant = 0usize;
    for thread in hierarchy.children_of(Some(threads_root.id)) {
        if thread.kind != NodeKind::Chapter {
            continue;
        }
        total += 1;
        // Walk waypoint paragraphs to find the
        // newest mtime.
        let mut newest: Option<std::time::SystemTime> = None;
        for waypoint in hierarchy.children_of(Some(thread.id)) {
            if waypoint.kind != NodeKind::Paragraph {
                continue;
            }
            let Some(rel) = waypoint.file.as_ref() else { continue };
            let abs = project_root.join(rel);
            let Ok(md) = std::fs::metadata(&abs) else { continue };
            let Ok(mtime) = md.modified() else { continue };
            newest = Some(match newest {
                Some(prev) if prev >= mtime => prev,
                _ => mtime,
            });
        }
        match newest {
            Some(t) if t >= threshold => active += 1,
            _ => dormant += 1,
        }
    }
    ThreadsSection { total, active, dormant }
}

fn comments_section(
    hierarchy: &crate::store::hierarchy::Hierarchy,
    project_root: &std::path::Path,
) -> CommentsSection {
    use crate::store::NodeKind;
    let week_ago = Utc::now() - chrono::Duration::days(7);
    let mut open = 0usize;
    let mut resolved_this_week = 0usize;
    let mut resolved_total = 0usize;
    for node in hierarchy.iter() {
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let Some(rel) = node.file.as_ref() else { continue };
        let abs = project_root.join(rel);
        let sidecar = crate::tui::comments::sidecar_path(&abs);
        if !sidecar.exists() {
            continue;
        }
        let Ok(file) = crate::tui::comments::load_from_sidecar(&abs) else { continue };
        for c in &file.comments {
            if c.resolved {
                resolved_total += 1;
                if let Some(r) = c.resolved_at {
                    if r >= week_ago {
                        resolved_this_week += 1;
                    }
                }
            } else {
                open += 1;
            }
        }
    }
    CommentsSection {
        open,
        resolved_this_week,
        resolved_total,
    }
}

/// Render the snapshot as portable Markdown.
/// Used by the modal's `e` export action.
pub fn to_markdown(s: &JournalSnapshot) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# Manuscript Journal — {}\n\n",
        s.generated_at.format("%Y-%m-%d %H:%M UTC")
    ));

    out.push_str("## Word count\n\n");
    out.push_str(&format!("- today: {}\n", s.word_count.today));
    out.push_str(&format!("- total: {}\n", s.word_count.total));
    if s.word_count.goal > 0 {
        let remaining = (s.word_count.goal - s.word_count.total).max(0);
        out.push_str(&format!(
            "- goal: {} (remaining: {})\n",
            s.word_count.goal, remaining
        ));
        if !s.word_count.target_date.is_empty() {
            out.push_str(&format!("- target date: {}\n", s.word_count.target_date));
        }
    }
    out.push_str(&format!("- streak: {} day(s)\n", s.word_count.streak_days));
    out.push_str(&format!(
        "- active today: {} min\n",
        s.word_count.active_seconds_today / 60
    ));
    out.push_str(&format!(
        "- active this week: {} min\n",
        s.word_count.active_seconds_week / 60
    ));

    out.push_str("\n## Structure\n\n");
    out.push_str(&format!("- user books: {}\n", s.structure.user_books));
    out.push_str(&format!("- chapters: {}\n", s.structure.chapters));
    out.push_str(&format!("- paragraphs: {}\n", s.structure.paragraphs));
    if !s.structure.chapter_word_counts.is_empty() {
        out.push_str(&format!(
            "- mean chapter: {:.0} words ± {:.0} (CV {:.0}%)\n",
            s.structure.avg_chapter_words,
            s.structure.stdev_chapter_words,
            s.structure.cv * 100.0,
        ));
        out.push_str(&format!("- pacing: {}\n", s.structure.pacing_verdict));
    }

    out.push_str("\n## Threads\n\n");
    out.push_str(&format!("- total: {}\n", s.threads.total));
    out.push_str(&format!("- active: {}\n", s.threads.active));
    out.push_str(&format!(
        "- dormant (no waypoint in > {} d): {}\n",
        DORMANT_DAYS, s.threads.dormant
    ));

    out.push_str("\n## Comments\n\n");
    out.push_str(&format!("- open: {}\n", s.comments.open));
    out.push_str(&format!(
        "- resolved this week: {}\n",
        s.comments.resolved_this_week
    ));
    out.push_str(&format!(
        "- resolved total: {}\n",
        s.comments.resolved_total
    ));

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pacing_empty_is_em_dash() {
        let (m, s, c, v) = pacing_stats(&[]);
        assert_eq!(m, 0.0);
        assert_eq!(s, 0.0);
        assert_eq!(c, 0.0);
        assert_eq!(v, "—");
    }

    #[test]
    fn pacing_single_chapter_is_steady() {
        let (m, s, c, v) = pacing_stats(&[5000]);
        assert!((m - 5000.0).abs() < 0.01);
        assert_eq!(s, 0.0);
        assert_eq!(c, 0.0);
        assert_eq!(v, "steady");
    }

    #[test]
    fn pacing_uniform_chapters_are_steady() {
        let (_, _, c, v) = pacing_stats(&[5000, 5050, 5100, 4950, 5000]);
        assert!(c < 0.20, "expected steady, got cv={c}");
        assert_eq!(v, "steady");
    }

    #[test]
    fn pacing_moderate_variation_is_varied() {
        let (_, _, c, v) = pacing_stats(&[3000, 5000, 7000, 4000, 6000]);
        assert!((0.20..0.50).contains(&c), "expected varied, got cv={c}");
        assert_eq!(v, "varied");
    }

    #[test]
    fn pacing_high_variation_is_choppy() {
        let (_, _, c, v) = pacing_stats(&[1000, 8000, 500, 9000, 200]);
        assert!(c >= 0.50, "expected choppy, got cv={c}");
        assert_eq!(v, "choppy");
    }

    #[test]
    fn word_count_section_uses_goal_when_no_progress_cache() {
        let w = super::word_count_section(None, 50000, "2026-12-31");
        assert_eq!(w.goal, 50000);
        assert_eq!(w.target_date, "2026-12-31");
        assert_eq!(w.today, 0);
        assert_eq!(w.streak_days, 0);
    }

    #[test]
    fn to_markdown_includes_every_section_header() {
        let snap = JournalSnapshot {
            generated_at: Utc::now(),
            word_count: WordCountSection::default(),
            structure: StructureSection::default(),
            threads: ThreadsSection::default(),
            comments: CommentsSection::default(),
        };
        let md = to_markdown(&snap);
        assert!(md.contains("# Manuscript Journal"));
        assert!(md.contains("## Word count"));
        assert!(md.contains("## Structure"));
        assert!(md.contains("## Threads"));
        assert!(md.contains("## Comments"));
    }

    #[test]
    fn to_markdown_omits_goal_block_when_zero() {
        let snap = JournalSnapshot {
            generated_at: Utc::now(),
            word_count: WordCountSection {
                goal: 0,
                ..Default::default()
            },
            structure: StructureSection::default(),
            threads: ThreadsSection::default(),
            comments: CommentsSection::default(),
        };
        let md = to_markdown(&snap);
        assert!(!md.contains("goal:"), "goal line should be absent when goal=0");
    }
}
