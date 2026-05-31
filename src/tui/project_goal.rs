//! 1.2.14+ Phase Q.4 — project word-count goal +
//! projection math.  Materialised once at modal
//! open time so the renderer is pure read.
//!
//! See `Documentation/PROPOSALS/1.2.14_PLAN.md`
//! §9.

use chrono::{Local, NaiveDate};

/// Materialised projection data.  All fields are
/// pre-computed; the renderer just paints them.
#[derive(Debug, Clone)]
pub(crate) struct ProjectGoalData {
    pub total_words: u64,
    pub goal: u64,
    /// `total_words * 100 / goal` clamped to
    /// `0..=999`.  `0` when the goal is zero.
    pub pct: u32,
    /// `goal - total_words`, saturating-subtraction.
    /// `0` when already past goal.  Reserved for
    /// the Q.4.1 "you're X words from goal" status-
    /// bar chip — currently not surfaced in the
    /// modal text (the bar / percentage do).
    #[allow(dead_code)]
    pub remaining: u64,
    /// Days from today to target.  `None` when no
    /// target date is configured.  Negative when
    /// target has already passed (signalled via
    /// `Option::None` with a verdict label below).
    pub days_remaining: Option<i64>,
    /// `remaining / days_remaining`, rounded up
    /// for "you need at LEAST this many".  `None`
    /// when no target date or already past goal.
    pub required_per_day: Option<u64>,
    /// Recent average words/day from the daily
    /// streak event log.  `None` when the event
    /// log is missing or empty.
    pub recent_avg: Option<u64>,
    /// Projected completion date based on
    /// `recent_avg`.  `None` when recent_avg is
    /// None.
    pub projection_date: Option<NaiveDate>,
    /// Per-book contribution rows in canonical
    /// hierarchy order.  `(book_title, word_count, pct_of_total)`.
    pub per_book: Vec<(String, u64, u32)>,
    /// Verdict glyph + label for the modal footer.
    pub verdict: Verdict,
}

#[derive(Debug, Clone)]
pub(crate) enum Verdict {
    NoGoal,
    NoTarget,
    /// Projection is on or before target — `✓ ahead`.
    Ahead,
    /// Projection within 7 days of target — `· on track`.
    OnTrack,
    /// Projection is after target — `✗ behind`.
    Behind,
    /// Goal already met — `✓ complete`.
    Complete,
}

impl Verdict {
    pub fn glyph(&self) -> &'static str {
        match self {
            Verdict::NoGoal => "—",
            Verdict::NoTarget => "—",
            Verdict::Ahead => "✓",
            Verdict::OnTrack => "·",
            Verdict::Behind => "✗",
            Verdict::Complete => "✓",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Verdict::NoGoal => "no goal set",
            Verdict::NoTarget => "no target date",
            Verdict::Ahead => "ahead",
            Verdict::OnTrack => "on track",
            Verdict::Behind => "behind",
            Verdict::Complete => "complete",
        }
    }
}

/// Compute the verdict given the projection
/// against the target date.  Pure function so it
/// can be tested without spinning up a project.
pub fn verdict_for(
    total_words: u64,
    goal: u64,
    target: Option<NaiveDate>,
    projection: Option<NaiveDate>,
) -> Verdict {
    if goal == 0 {
        return Verdict::NoGoal;
    }
    if total_words >= goal {
        return Verdict::Complete;
    }
    let (Some(target), Some(projection)) = (target, projection) else {
        return Verdict::NoTarget;
    };
    let delta = projection.signed_duration_since(target).num_days();
    if delta <= 0 {
        Verdict::Ahead
    } else if delta <= 7 {
        Verdict::OnTrack
    } else {
        Verdict::Behind
    }
}

/// Today, in the local timezone.  Wrapper so the
/// projection-date math has a single source of
/// "now".
pub fn today_local() -> NaiveDate {
    Local::now().date_naive()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn verdict_no_goal_when_goal_zero() {
        assert!(matches!(
            verdict_for(1000, 0, None, None),
            Verdict::NoGoal
        ));
    }

    #[test]
    fn verdict_complete_when_past_goal() {
        assert!(matches!(
            verdict_for(80_000, 80_000, None, None),
            Verdict::Complete
        ));
        assert!(matches!(
            verdict_for(85_000, 80_000, None, None),
            Verdict::Complete
        ));
    }

    #[test]
    fn verdict_no_target_when_target_missing() {
        assert!(matches!(
            verdict_for(40_000, 80_000, None, None),
            Verdict::NoTarget
        ));
    }

    #[test]
    fn verdict_ahead_when_projection_before_target() {
        let target = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
        let projection = NaiveDate::from_ymd_opt(2026, 8, 15).unwrap();
        assert!(matches!(
            verdict_for(40_000, 80_000, Some(target), Some(projection)),
            Verdict::Ahead
        ));
    }

    #[test]
    fn verdict_on_track_when_projection_within_seven_days() {
        let target = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
        let projection = NaiveDate::from_ymd_opt(2026, 9, 4).unwrap();
        assert!(matches!(
            verdict_for(40_000, 80_000, Some(target), Some(projection)),
            Verdict::OnTrack
        ));
    }

    #[test]
    fn verdict_behind_when_projection_far_after_target() {
        let target = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
        let projection = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        assert!(matches!(
            verdict_for(40_000, 80_000, Some(target), Some(projection)),
            Verdict::Behind
        ));
    }

    #[test]
    fn verdict_glyphs_and_labels_are_distinct() {
        let all = [
            Verdict::NoGoal,
            Verdict::NoTarget,
            Verdict::Ahead,
            Verdict::OnTrack,
            Verdict::Behind,
            Verdict::Complete,
        ];
        for v in &all {
            assert!(!v.glyph().is_empty());
            assert!(!v.label().is_empty());
        }
    }
}
