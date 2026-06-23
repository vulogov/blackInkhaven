//! Shared types for the refactored timeline critique (TIMELINE-2-INTEGRATION).
//!
//! The two retained, timeline-internal checks — orphan detection and
//! fuzzy-precision overlap — operate on a critique-local [`CritiqueEvent`] rather
//! than on `tui::timeline_state::TimelineEvent` or `store::EventData`. That keeps
//! the detectors pure, dependency-light, and trivially testable: the TUI and CLI
//! each project their own event snapshots into `CritiqueEvent`s, supplying the one
//! signal neither snapshot carries — the orphan age in days.

use uuid::Uuid;

use crate::timeline::Precision;

/// The scope a critique run covers. Mirrors the `y` / `Y` / `Ctrl+Y` / F12 chord
/// family (track / view / book / project) so CLI and TUI describe scope the same
/// way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// One track in the current view (`y`).
    Track,
    /// All tracks in the current view (`Y`).
    View,
    /// All events in the current book (`Ctrl+Y`).
    Book,
    /// All events in all books in the project (F12).
    Project,
}

impl Scope {
    pub fn label(self) -> &'static str {
        match self {
            Scope::Track => "track",
            Scope::View => "view",
            Scope::Book => "book",
            Scope::Project => "project",
        }
    }
}

/// A single event projected for the critique detectors. Built by the caller from
/// whatever event snapshot it has; the detectors never touch `Node`/`EventData`.
#[derive(Debug, Clone, PartialEq)]
pub struct CritiqueEvent {
    pub id: Uuid,
    pub title: String,
    pub start_ticks: i64,
    pub end_ticks: Option<i64>,
    pub precision: Precision,
    /// Resolved track (the caller has already applied the default-track fallback).
    pub track: String,
    /// True when the event has no linked paragraphs, characters, or places.
    pub is_orphan: bool,
    pub linked_paragraph_count: usize,
    pub characters: Vec<Uuid>,
    pub places: Vec<Uuid>,
    /// How long the event has been orphaned, in days. `None` when the caller can't
    /// establish it (no creation timestamp) — treated as [`Staleness::Recent`].
    pub age_days: Option<i64>,
}

impl CritiqueEvent {
    /// The event's `[start, end]` tick span (`end` = `start` for an instant).
    pub fn span(&self) -> (i64, i64) {
        (self.start_ticks, self.end_ticks.unwrap_or(self.start_ticks))
    }

    /// Do two events' spans overlap (inclusive)?
    pub fn overlaps(&self, other: &CritiqueEvent) -> bool {
        let (a0, a1) = self.span();
        let (b0, b1) = other.span();
        a0 <= b1 && b0 <= a1
    }

    /// Number of distinct outbound references — a cheap richness proxy.
    pub fn reference_count(&self) -> usize {
        self.linked_paragraph_count + self.characters.len() + self.places.len()
    }
}

/// Severity computed by the pure detectors. Maps to `pane::output::Severity` only
/// at emission time (P1), so the core carries no PANE-1 dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CritSeverity {
    /// Worth a glance — an Inquiry/Notice-flavoured finding.
    Info,
    /// Worth examining.
    Warning,
    /// Probably an authorial slip.
    Contradiction,
}

impl CritSeverity {
    /// A short label for the Output line, in the Notice/Inquiry/Probe family the
    /// sibling systems use.
    pub fn label(self) -> &'static str {
        match self {
            CritSeverity::Info => "Inquiry",
            CritSeverity::Warning => "Warning",
            CritSeverity::Contradiction => "Contradiction",
        }
    }
}

/// How significant an event looks, for orphan-severity gradation.
///
/// The RFC's heuristic keys off `summary`/`notes` prose, which the data model
/// doesn't carry. We adapt to the signals we have (see `orphan::significance`):
/// precision concreteness, title richness, and track activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Significance {
    Low,
    Moderate,
    High,
}

/// How long an event has sat orphaned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Staleness {
    /// Orphaned within the grace window.
    Recent,
    /// Orphaned longer than the grace window.
    Old,
}

/// An orphaned-event finding from [`super::orphan::detect`].
#[derive(Debug, Clone, PartialEq)]
pub struct OrphanFinding {
    pub event_id: Uuid,
    pub title: String,
    pub track: String,
    pub start_ticks: i64,
    pub precision: Precision,
    pub significance: Significance,
    pub staleness: Staleness,
    pub age_days: Option<i64>,
    pub severity: CritSeverity,
    /// Pattern-detected reason lines (English; localized at emission).
    pub reasons: Vec<String>,
}

/// The suspicion level of a fuzzy-precision overlap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suspicion {
    /// Below the emit threshold — filtered out.
    Low,
    /// One suspicious factor — surfaces as an Inquiry.
    Moderate,
    /// Multiple factors (same track + shared entity + tight window).
    High,
    /// A 3+ event mutual-overlap cluster.
    Cluster,
}

impl Suspicion {
    pub fn severity(self) -> CritSeverity {
        match self {
            Suspicion::Low => CritSeverity::Info,
            Suspicion::Moderate => CritSeverity::Info,
            Suspicion::High => CritSeverity::Warning,
            Suspicion::Cluster => CritSeverity::Warning,
        }
    }
}

/// A fuzzy-precision overlap finding from [`super::fuzzy_overlap::detect`]. Carries
/// 2 events (a pair) or 3+ (a cluster).
#[derive(Debug, Clone, PartialEq)]
pub struct FuzzyOverlapFinding {
    pub event_ids: Vec<Uuid>,
    pub titles: Vec<String>,
    pub track: String,
    /// `[start, end]` ticks of the overlapping window.
    pub overlap_window: (i64, i64),
    pub suspicion: Suspicion,
    pub is_cluster: bool,
    pub shared_characters: Vec<Uuid>,
    pub shared_places: Vec<Uuid>,
    pub severity: CritSeverity,
    /// Pattern-detected reason lines (English; localized at emission).
    pub reasons: Vec<String>,
}

/// Per-precision fuzz windows in ticks, derived from the calendar by the caller so
/// the detectors stay calendar-free. A `Day`-precision event has (effectively) no
/// fuzz; coarser precisions widen the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuzzWindows {
    pub year: i64,
    pub season: i64,
    pub month: i64,
    pub week: i64,
    pub day: i64,
}

impl FuzzWindows {
    /// The fuzz radius (ticks) for an event of the given precision. Events coarser
    /// than `Day` carry a window; `Day` and finer are treated as exact.
    pub fn radius(self, p: Precision) -> i64 {
        match p {
            Precision::Year => self.year,
            Precision::Season => self.season,
            Precision::Month => self.month,
            Precision::Week => self.week,
            Precision::Day | Precision::Hour | Precision::Tick => 0,
        }
    }

    /// Whether a precision is "fuzzy" (Season or Month, per the RFC's retained
    /// scope) — only fuzzy events participate in overlap detection.
    pub fn is_fuzzy(p: Precision) -> bool {
        matches!(p, Precision::Year | Precision::Season | Precision::Month | Precision::Week)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlaps_is_inclusive() {
        let a = CritiqueEvent {
            id: Uuid::nil(),
            title: "a".into(),
            start_ticks: 10,
            end_ticks: Some(20),
            precision: Precision::Day,
            track: "main".into(),
            is_orphan: false,
            linked_paragraph_count: 0,
            characters: vec![],
            places: vec![],
            age_days: None,
        };
        let b = CritiqueEvent { start_ticks: 20, end_ticks: None, ..a.clone() };
        let c = CritiqueEvent { start_ticks: 21, end_ticks: None, ..a.clone() };
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn fuzz_radius_by_precision() {
        let w = FuzzWindows { year: 360, season: 90, month: 30, week: 7, day: 1 };
        assert_eq!(w.radius(Precision::Season), 90);
        assert_eq!(w.radius(Precision::Month), 30);
        assert_eq!(w.radius(Precision::Day), 0);
        assert!(FuzzWindows::is_fuzzy(Precision::Season));
        assert!(!FuzzWindows::is_fuzzy(Precision::Day));
    }
}
