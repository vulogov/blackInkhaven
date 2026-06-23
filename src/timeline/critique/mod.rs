//! Timeline critique (TIMELINE-2-INTEGRATION).
//!
//! Originally (1.2.6) the timeline critique was a single LLM payload — a flat event
//! summary plus a five-item audit checklist — streamed into the AI pane. WORLD-4 +
//! WORLD-5 and INNER_SOCRATES-1 since took over four of those five items:
//! travel-time / co-location → the fact-checker's `travel_time` + `co_location`;
//! paragraph-date mismatches → `date_coherence`; pacing → INNER_SOCRATES-1's
//! `temporal_density`.
//!
//! What remains is the genuinely *timeline-internal* specialty, which no other
//! system covers:
//!
//! * **Orphan detection** — events linked to nothing (no paragraphs, characters,
//!   or places). See [`orphan`].
//! * **Fuzzy-precision overlap** — `Season`/`Month`-precision events whose fuzz
//!   windows collide suspiciously. See [`fuzzy_overlap`].
//!
//! These are pure, pattern-based detectors emitting structured findings to the
//! PANE-1 Output pane (P1). The original behaviour is preserved verbatim in
//! [`legacy`] behind the deprecated `inkhaven event critique --legacy` flag.

// The detector API lands ahead of its PANE-1 / CLI / Bund consumers (P1+), the
// same way `world/mod.rs` ships its types before the phases that wire them.
#![allow(dead_code)]

pub mod fuzzy_overlap;
pub mod legacy;
pub mod orphan;
pub mod types;

// The legacy payload builder keeps its historical path
// (`crate::timeline::critique::build_health_payload`) so the `--legacy` path and
// the existing TUI handler compile unchanged.
pub use legacy::build_health_payload;

pub use types::{
    CritSeverity, CritiqueEvent, FuzzWindows, FuzzyOverlapFinding, OrphanFinding,
    Scope, Significance, Staleness, Suspicion,
};

use crate::timeline::Calendar;

/// Derive per-precision fuzz windows from a calendar. A `Day`-precision event is
/// exact; coarser precisions widen by the calendar's unit length (falling back to
/// day-multiples when a unit isn't configured).
pub fn fuzz_windows(calendar: &Calendar) -> FuzzWindows {
    let day = calendar.ticks_per("day").unwrap_or(1).max(1);
    FuzzWindows {
        year: calendar.ticks_per("year").unwrap_or(day * 360),
        season: calendar.ticks_per("season").unwrap_or(day * 90),
        month: calendar.ticks_per("month").unwrap_or(day * 30),
        week: calendar.ticks_per("week").unwrap_or(day * 7),
        day,
    }
}

/// The combined result of a refactored critique run over one scope.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CritiqueReport {
    pub orphans: Vec<OrphanFinding>,
    pub overlaps: Vec<FuzzyOverlapFinding>,
}

impl CritiqueReport {
    pub fn is_empty(&self) -> bool {
        self.orphans.is_empty() && self.overlaps.is_empty()
    }

    pub fn total(&self) -> usize {
        self.orphans.len() + self.overlaps.len()
    }
}

/// Run both retained checks over a scope's events with the given thresholds. The
/// caller has already filtered `events` to the scope and built `fuzz` from the
/// calendar.
pub fn run(
    events: &[CritiqueEvent],
    fuzz: &FuzzWindows,
    min_significance: Significance,
    min_suspicion: Suspicion,
    cluster_min_size: usize,
    staleness_threshold_days: i64,
) -> CritiqueReport {
    let ctx = orphan::ScopeContext::from_events(events, staleness_threshold_days);
    CritiqueReport {
        orphans: orphan::detect(events, &ctx, min_significance),
        overlaps: fuzzy_overlap::detect(events, fuzz, min_suspicion, cluster_min_size),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::Precision;
    use uuid::Uuid;

    fn fuzz() -> FuzzWindows {
        FuzzWindows { year: 360, season: 90, month: 30, week: 7, day: 1 }
    }

    fn orphan_ev(title: &str) -> CritiqueEvent {
        CritiqueEvent {
            id: Uuid::new_v4(),
            title: title.into(),
            start_ticks: 0,
            end_ticks: None,
            precision: Precision::Day,
            track: "main".into(),
            is_orphan: true,
            linked_paragraph_count: 0,
            characters: vec![],
            places: vec![],
            age_days: Some(100),
        }
    }

    #[test]
    fn run_collects_both_kinds() {
        let mara = Uuid::new_v4();
        let mut overlap_a = orphan_ev("Training at Velmaril");
        overlap_a.is_orphan = false;
        overlap_a.precision = Precision::Season;
        overlap_a.characters = vec![mara];
        overlap_a.linked_paragraph_count = 1;
        let mut overlap_b = overlap_a.clone();
        overlap_b.id = Uuid::new_v4();
        overlap_b.title = "Journey from Velmaril".into();
        overlap_b.start_ticks = 30;

        let events = vec![orphan_ev("The Lost Coronation Rite"), overlap_a, overlap_b];
        let report = run(
            &events,
            &fuzz(),
            Significance::Low,
            Suspicion::Moderate,
            DEFAULT_CLUSTER_MIN_SIZE,
            60,
        );
        assert_eq!(report.orphans.len(), 1);
        assert_eq!(report.overlaps.len(), 1);
        assert!(!report.is_empty());
        assert_eq!(report.total(), 2);
    }
}

/// Re-exported default thresholds for callers (CLI/TUI/config defaults).
pub use fuzzy_overlap::DEFAULT_CLUSTER_MIN_SIZE;
pub use orphan::DEFAULT_STALENESS_DAYS;
