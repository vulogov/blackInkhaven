//! Aggregates the status-bar widget + Ctrl+V G modal consume.
//!
//! All aggregates are computed at query time — `writing_events`
//! and `writing_baselines` are the source of truth, no cached
//! summaries. The query volume is tiny (one snapshot per modal
//! open, one per status-bar redraw which we throttle), so this
//! stays cheap even for projects with years of history.

use std::collections::HashMap;

use anyhow::Result;
use uuid::Uuid;

use super::store::{ProgressStore, PROJECT_SCOPE_BOOK_ID};
use crate::config::GoalsConfig;

/// Top-level structure handed to the renderer. All counts are
/// signed — negative `today_words` means the user deleted more
/// than they wrote today.
#[derive(Debug, Clone)]
pub struct ProgressSnapshot {
    pub project: BookProgress,
    pub books: Vec<BookProgress>,
    pub status: StatusLadderCounts,
    pub streak: StreakStatus,
    /// Last-30-days sparkline data, oldest first, project-wide.
    pub sparkline: Vec<i64>,
    /// Active writing seconds today — sum of save→save gaps,
    /// each gap capped at 5 min so AFK doesn't inflate. Honest
    /// about "time at the keyboard" without keystroke tracking.
    pub active_seconds_today: i64,
    /// Same calculation over the trailing 7 days.
    pub active_seconds_week: i64,
}

impl ProgressSnapshot {
    pub fn empty() -> Self {
        Self {
            project: BookProgress::empty("project"),
            books: Vec::new(),
            status: StatusLadderCounts::default(),
            streak: StreakStatus::default(),
            sparkline: Vec::new(),
            active_seconds_today: 0,
            active_seconds_week: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BookProgress {
    pub label: String,
    pub today_words: i64,
    pub daily_goal: Option<i64>,
    pub total_words: i64,
    pub target_words: Option<i64>,
    /// Words/day the user must average to hit `target_words` by
    /// the deadline. None if no deadline / target.
    pub required_pace: Option<i64>,
    /// Days remaining until the deadline (negative = overdue).
    pub days_to_deadline: Option<i64>,
}

impl BookProgress {
    pub fn empty(label: &str) -> Self {
        Self {
            label: label.to_string(),
            today_words: 0,
            daily_goal: None,
            total_words: 0,
            target_words: None,
            required_pace: None,
            days_to_deadline: None,
        }
    }
}

/// Last 7 days of recorded promotions, grouped by `to` status,
/// plus the user's per-status goal for the week.
#[derive(Debug, Clone, Default)]
pub struct StatusLadderCounts {
    pub recent: Vec<(String, i64)>,
    pub goals: Vec<(String, i64)>,
}

#[derive(Debug, Clone, Default)]
pub struct StreakStatus {
    pub days: i64,
    pub grace_used: i64,
    pub grace_per_week: i64,
    /// Longest streak ever recorded, computed over the project's
    /// full writing history (not just the recent window) with the
    /// same grace rule. `0` when there is no history yet.
    pub best: i64,
}

/// Streak-length milestones worth celebrating, ascending. A hook
/// fires the first time the current streak crosses one upward.
pub const STREAK_MILESTONES: [i64; 4] = [7, 30, 100, 365];

/// The largest milestone newly crossed when the streak grows from
/// `prev` to `now` (both current-streak lengths). `None` when no
/// milestone boundary sits in `(prev, now]`.
pub fn milestone_crossed(prev: i64, now: i64) -> Option<i64> {
    STREAK_MILESTONES
        .iter()
        .rev()
        .copied()
        .find(|&m| prev < m && now >= m)
}

/// Caller-supplied "live" word counts (computed from the
/// hierarchy walk). The aggregator can't read paragraph bodies
/// itself without re-implementing the hierarchy crawl, so the
/// editor passes these in alongside the goals.
#[derive(Debug, Clone, Default)]
pub struct LiveTotals {
    pub per_book: HashMap<Uuid, i64>,
    pub project_total: i64,
    /// Slug→title map for nice labels in the modal. The status-
    /// bar widget only needs project-wide, so this can be empty.
    pub book_titles: HashMap<Uuid, String>,
    /// Slug for each book — used to match HJSON `goals.books.<slug>`
    /// entries. Lowercased so the lookup is case-insensitive.
    pub book_slugs: HashMap<Uuid, String>,
}

/// Build the snapshot. `live` carries the per-book + project
/// word totals + labels the editor computed; `store` provides
/// the historical aggregates.
pub fn build_snapshot(
    store: &ProgressStore,
    goals: &GoalsConfig,
    live: &LiveTotals,
) -> Result<ProgressSnapshot> {
    let today = super::store::today_utc_days();

    // Project-wide aggregates.
    let project_today = store
        .today_words(PROJECT_SCOPE_BOOK_ID, live.project_total)
        .unwrap_or(0);
    let project = BookProgress {
        label: "project".into(),
        today_words: project_today,
        daily_goal: nonzero(goals.daily_words),
        total_words: live.project_total,
        target_words: None,
        required_pace: None,
        days_to_deadline: None,
    };

    // Per-book breakdown.
    let mut books: Vec<BookProgress> = Vec::new();
    for (id, total) in live.per_book.iter() {
        let today_w = store.today_words(*id, *total).unwrap_or(0);
        let slug = live
            .book_slugs
            .get(id)
            .cloned()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let title = live
            .book_titles
            .get(id)
            .cloned()
            .unwrap_or_else(|| slug.clone());
        let goal = goals.books.get(&slug);
        let target_words = goal.map(|g| g.target_words).filter(|n| *n > 0);
        let days_to_deadline = goal
            .filter(|g| !g.deadline.is_empty())
            .and_then(|g| parse_iso_date_days(&g.deadline))
            .map(|d| d - today);
        let required_pace = match (target_words, days_to_deadline) {
            (Some(t), Some(dd)) => required_pace(*total, t, dd),
            _ => None,
        };
        books.push(BookProgress {
            label: title,
            today_words: today_w,
            daily_goal: nonzero(goals.daily_words),
            total_words: *total,
            target_words,
            required_pace,
            days_to_deadline,
        });
    }
    books.sort_by(|a, b| a.label.cmp(&b.label));

    // Streak. Query the full history (DISTINCT writing-days is one
    // cheap row per day) so the current streak and the lifetime best
    // both come from the same vector — no separate windowed query.
    let writing_days = store.writing_days_recent(ALL_HISTORY_DAYS).unwrap_or_default();
    let mut streak = compute_streak(&writing_days, today, goals.streak_grace_per_week);
    streak.best = longest_streak(&writing_days, goals.streak_grace_per_week);

    // Status ladder.
    let recent = store.status_promotions_recent(7).unwrap_or_default();
    let goal_pairs: Vec<(String, i64)> = goals
        .status_ladder
        .iter()
        .map(|(k, v)| (k.to_ascii_lowercase(), *v))
        .collect();
    let status = StatusLadderCounts {
        recent,
        goals: goal_pairs,
    };

    // Sparkline.
    let sparkline = store
        .last_n_daily(PROJECT_SCOPE_BOOK_ID, live.project_total, 30)
        .unwrap_or_default();

    // Active-time aggregates (1.2.4+). Today's window is from
    // today-start (UTC) to now; week is last 7×86400s.
    let today_start = today * 86_400;
    let now_secs = today_start + 86_400; // future bound — saves can't
                                         // be in the future anyway
    const ACTIVE_GAP_CAP_SEC: i64 = 300; // 5 min per gap
    let active_seconds_today = store
        .active_seconds_in_range(today_start, now_secs, ACTIVE_GAP_CAP_SEC)
        .unwrap_or(0);
    let week_start = (today - 6) * 86_400;
    let active_seconds_week = store
        .active_seconds_in_range(week_start, now_secs, ACTIVE_GAP_CAP_SEC)
        .unwrap_or(0);

    Ok(ProgressSnapshot {
        project,
        books,
        status,
        streak,
        sparkline,
        active_seconds_today,
        active_seconds_week,
    })
}

fn nonzero(n: i64) -> Option<i64> {
    if n > 0 { Some(n) } else { None }
}

/// Parse `YYYY-MM-DD` into days-since-epoch UTC. Returns None on
/// any parse failure — the caller treats absence as "no deadline".
fn parse_iso_date_days(s: &str) -> Option<i64> {
    let parsed = chrono::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok()?;
    let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1)?;
    Some(parsed.signed_duration_since(epoch).num_days())
}

/// "Since the beginning of the project" expressed as a look-back
/// window, because `writing_days_recent` takes a `days_back` and
/// derives its cutoff as `now − days_back·86400`. It's not a cap on
/// how long anyone writes — it just has to land the cutoff before
/// the first row, and the Unix epoch (1970) is only ~20k days back,
/// so any value past that returns the whole history. 200 years is a
/// round, overflow-safe pick comfortably below i64.
const ALL_HISTORY_DAYS: i64 = 365 * 200;

/// Streak length: trailing run of "writing days" (≥1 positive
/// save event) ending today, allowing `grace_per_week` skipped
/// days inside the rolling 7-day window. A skip beyond the
/// allowance breaks the streak.
pub fn compute_streak(
    writing_days_desc: &[i64],
    today: i64,
    grace_per_week: i64,
) -> StreakStatus {
    if writing_days_desc.is_empty() {
        return StreakStatus {
            days: 0,
            grace_used: 0,
            grace_per_week,
            best: 0,
        };
    }
    let writing: std::collections::HashSet<i64> =
        writing_days_desc.iter().copied().collect();
    let mut days: i64 = 0;
    let mut grace_used_window: i64 = 0;
    let mut window: std::collections::VecDeque<bool> =
        std::collections::VecDeque::with_capacity(7); // true = skipped
    let mut d = today;
    loop {
        let wrote = writing.contains(&d);
        let skipped = !wrote;
        // Slide the rolling 7-day window forward.
        if window.len() == 7 {
            if let Some(old) = window.pop_front() {
                if old {
                    grace_used_window -= 1;
                }
            }
        }
        window.push_back(skipped);
        if skipped {
            grace_used_window += 1;
            if grace_used_window > grace_per_week {
                break;
            }
        }
        days += 1;
        d -= 1;
        // Bound the scan — practical streaks fit easily.
        if days > 1_000 {
            break;
        }
    }
    StreakStatus {
        days,
        grace_used: grace_used_window.max(0),
        grace_per_week,
        best: 0,
    }
}

/// Longest streak ever, over the full writing-day history, with the
/// same grace rule as `compute_streak`. A streak ending on a
/// non-writing day is never longer than one ending on the previous
/// writing day, so it suffices to take the max of the streaks
/// ending at each writing-day. The candidate set is bounded (one
/// entry per writing-day) and `compute_streak` self-caps its scan,
/// so this stays cheap even for years of daily writing.
pub fn longest_streak(writing_days_desc: &[i64], grace_per_week: i64) -> i64 {
    writing_days_desc
        .iter()
        .map(|&d| compute_streak(writing_days_desc, d, grace_per_week).days)
        .max()
        .unwrap_or(0)
}

/// Required daily pace to hit `target_words` by the deadline.
/// Negative or zero days_to_deadline → pace is the remaining
/// gap (the user is past due; pacing is moot).
pub fn required_pace(current: i64, target: i64, days_to_deadline: i64) -> Option<i64> {
    if days_to_deadline <= 0 {
        let gap = target - current;
        if gap > 0 {
            Some(gap)
        } else {
            None
        }
    } else {
        let gap = (target - current).max(0);
        Some((gap + days_to_deadline - 1) / days_to_deadline) // ceil
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streak_unbroken() {
        // wrote every one of the last 5 days
        let today = 100;
        let days = vec![100, 99, 98, 97, 96];
        let s = compute_streak(&days, today, 0);
        assert_eq!(s.days, 5);
    }

    #[test]
    fn streak_breaks_no_grace() {
        let today = 100;
        // wrote 100, 99, then skipped 98, wrote 97
        let days = vec![100, 99, 97];
        let s = compute_streak(&days, today, 0);
        assert_eq!(s.days, 2);
    }

    #[test]
    fn streak_grace_one_per_week() {
        let today = 100;
        // wrote 100, 99, skipped 98, wrote 97, 96 — grace 1 lets us span it
        let days = vec![100, 99, 97, 96];
        let s = compute_streak(&days, today, 1);
        assert_eq!(s.days, 5);
    }

    #[test]
    fn longest_streak_finds_best_run_not_trailing() {
        // History (desc): a 3-day run, a gap, then today's 2-day run.
        // Current streak is 2 but the lifetime best is 3.
        let days = vec![100, 99, 95, 94, 93];
        assert_eq!(compute_streak(&days, 100, 0).days, 2);
        assert_eq!(longest_streak(&days, 0), 3);
    }

    #[test]
    fn longest_streak_respects_grace() {
        // 100,99, skip 98, 97,96 — with 1 grace/week it's one 5-run.
        let days = vec![100, 99, 97, 96];
        assert_eq!(longest_streak(&days, 0), 2); // strict: best is the 2-run
        assert_eq!(longest_streak(&days, 1), 5); // grace bridges the gap
    }

    #[test]
    fn longest_streak_empty_is_zero() {
        assert_eq!(longest_streak(&[], 0), 0);
    }

    #[test]
    fn milestone_crossings() {
        assert_eq!(milestone_crossed(6, 7), Some(7));
        assert_eq!(milestone_crossed(7, 8), None); // already past 7
        assert_eq!(milestone_crossed(29, 31), Some(30));
        // Jumping multiple milestones at once celebrates the highest.
        assert_eq!(milestone_crossed(0, 100), Some(100));
        assert_eq!(milestone_crossed(0, 0), None);
    }

    #[test]
    fn required_pace_simple() {
        assert_eq!(required_pace(0, 1000, 10), Some(100));
        assert_eq!(required_pace(500, 1000, 5), Some(100));
        assert_eq!(required_pace(1500, 1000, 5), Some(0));
    }

    #[test]
    fn required_pace_past_due() {
        // overdue: pace becomes the remaining gap in one big push.
        assert_eq!(required_pace(500, 1000, 0), Some(500));
        assert_eq!(required_pace(500, 1000, -3), Some(500));
        assert_eq!(required_pace(1000, 1000, -3), None);
    }
}
