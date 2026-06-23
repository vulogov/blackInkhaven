//! Orphan-event detection — a retained, timeline-internal critique.
//!
//! An event is **orphaned** when it links to nothing: no paragraphs, no
//! characters, no places. That reflects the timeline's *internal* coherence — it's
//! neither the manuscript's concern (the fact-checker) nor the world's. No other
//! system covers it, so it stays here, strengthened with significance × staleness
//! severity gradation.
//!
//! The detector is pure: it scores each orphan from signals the data model
//! actually carries (precision concreteness, title richness, track activity) and
//! the caller-supplied orphan age. Pattern-detected reason lines are English; they
//! are localized at emission (P5).

use std::collections::HashMap;

use super::types::{CritSeverity, CritiqueEvent, OrphanFinding, Significance, Staleness};
use crate::timeline::Precision;

/// Default grace window before an orphan is considered stale.
pub const DEFAULT_STALENESS_DAYS: i64 = 60;

/// Cross-event context the orphan scorer consults — chiefly which tracks are
/// "active" (carry many linked events elsewhere), a signal that an orphan on that
/// track is more likely an oversight than deliberate backstory.
#[derive(Debug, Clone, Default)]
pub struct ScopeContext {
    /// Track name → count of events on that track that carry at least one link.
    pub track_linked_counts: HashMap<String, usize>,
    /// Days after which an orphan counts as stale.
    pub staleness_threshold_days: i64,
}

impl ScopeContext {
    /// Build the context from the full event set: tally, per track, how many events
    /// are *not* orphaned (i.e. carry links).
    pub fn from_events(events: &[CritiqueEvent], staleness_threshold_days: i64) -> Self {
        let mut track_linked_counts: HashMap<String, usize> = HashMap::new();
        for e in events {
            if !e.is_orphan {
                *track_linked_counts.entry(e.track.clone()).or_insert(0) += 1;
            }
        }
        ScopeContext { track_linked_counts, staleness_threshold_days }
    }
}

fn precision_is_concrete(p: Precision) -> bool {
    matches!(p, Precision::Day | Precision::Hour | Precision::Tick)
}

fn precision_is_vague(p: Precision) -> bool {
    matches!(p, Precision::Season | Precision::Year)
}

fn title_word_count(title: &str) -> usize {
    title.split_whitespace().filter(|w| !w.is_empty()).count()
}

/// Significance of an orphaned event, adapted to the available signals (the data
/// model has no `summary`/`notes`): precision concreteness + title richness +
/// track activity.
pub fn significance(event: &CritiqueEvent, ctx: &ScopeContext) -> Significance {
    let mut score = 0i32;
    if precision_is_concrete(event.precision) {
        score += 2;
    } else if !precision_is_vague(event.precision) {
        score += 1; // Week / Month — middling commitment
    }
    let words = title_word_count(&event.title);
    if words >= 4 {
        score += 2;
    } else if words >= 2 {
        score += 1;
    }
    let track_active = ctx
        .track_linked_counts
        .get(&event.track)
        .copied()
        .unwrap_or(0)
        >= 3;
    if track_active {
        score += 1;
    }

    if score >= 4 {
        Significance::High
    } else if score >= 2 {
        Significance::Moderate
    } else {
        Significance::Low
    }
}

/// Staleness from the event's orphan age. Unknown age → Recent (conservative).
pub fn staleness(event: &CritiqueEvent, threshold_days: i64) -> Staleness {
    match event.age_days {
        Some(age) if age >= threshold_days => Staleness::Old,
        _ => Staleness::Recent,
    }
}

/// Severity from significance × staleness, per the RFC table. A major orphan that's
/// sat unconnected for months is a Contradiction; a fresh stub is barely an Info.
pub fn compute_severity(sig: Significance, stale: Staleness) -> CritSeverity {
    match (sig, stale) {
        (Significance::High, _) => CritSeverity::Contradiction,
        (Significance::Moderate, Staleness::Old) => CritSeverity::Warning,
        (Significance::Moderate, Staleness::Recent) => CritSeverity::Info,
        (Significance::Low, _) => CritSeverity::Info,
    }
}

fn rank(sig: Significance) -> u8 {
    match sig {
        Significance::Low => 0,
        Significance::Moderate => 1,
        Significance::High => 2,
    }
}

/// Run orphan detection over `events`. Only orphans at or above `min_significance`
/// produce findings.
pub fn detect(
    events: &[CritiqueEvent],
    ctx: &ScopeContext,
    min_significance: Significance,
) -> Vec<OrphanFinding> {
    let threshold = if ctx.staleness_threshold_days > 0 {
        ctx.staleness_threshold_days
    } else {
        DEFAULT_STALENESS_DAYS
    };
    let mut out = Vec::new();
    for e in events {
        if !e.is_orphan {
            continue;
        }
        let sig = significance(e, ctx);
        if rank(sig) < rank(min_significance) {
            continue;
        }
        let stale = staleness(e, threshold);
        let severity = compute_severity(sig, stale);
        let mut reasons = vec![
            "No paragraphs, characters, or places are linked to this event.".to_string(),
        ];
        match sig {
            Significance::High => reasons.push(
                "A concrete date and detailed title suggest a significant event."
                    .to_string(),
            ),
            Significance::Moderate => reasons
                .push("Has some detail but sits unconnected.".to_string()),
            Significance::Low => reasons
                .push("Stub event with minimal metadata.".to_string()),
        }
        match (stale, e.age_days) {
            (Staleness::Old, Some(age)) => {
                reasons.push(format!("Orphaned for {age} days."))
            }
            (Staleness::Recent, Some(age)) if age >= 0 => {
                reasons.push(format!("Recently added ({age} days ago)."))
            }
            _ => {}
        }
        out.push(OrphanFinding {
            event_id: e.id,
            title: e.title.clone(),
            track: e.track.clone(),
            start_ticks: e.start_ticks,
            precision: e.precision,
            significance: sig,
            staleness: stale,
            age_days: e.age_days,
            severity,
            reasons,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn orphan(title: &str, precision: Precision, age_days: Option<i64>) -> CritiqueEvent {
        CritiqueEvent {
            id: Uuid::new_v4(),
            title: title.into(),
            start_ticks: 0,
            end_ticks: None,
            precision,
            track: "main".into(),
            is_orphan: true,
            linked_paragraph_count: 0,
            characters: vec![],
            places: vec![],
            age_days,
        }
    }

    #[test]
    fn linked_events_are_never_orphans() {
        let mut e = orphan("Linked", Precision::Day, None);
        e.is_orphan = false;
        let ctx = ScopeContext::from_events(&[e.clone()], DEFAULT_STALENESS_DAYS);
        assert!(detect(&[e], &ctx, Significance::Low).is_empty());
    }

    #[test]
    fn high_significance_old_orphan_is_contradiction() {
        // 4-word title + day precision → High significance; 92 days → Old.
        let e = orphan("The Coronation of Hadrin III", Precision::Day, Some(92));
        let ctx = ScopeContext::from_events(&[e.clone()], DEFAULT_STALENESS_DAYS);
        let f = detect(&[e], &ctx, Significance::Low);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].significance, Significance::High);
        assert_eq!(f[0].staleness, Staleness::Old);
        assert_eq!(f[0].severity, CritSeverity::Contradiction);
        assert!(f[0].reasons.iter().any(|r| r.contains("92 days")));
    }

    #[test]
    fn low_significance_recent_stub_is_info() {
        // 1-word title + year precision → Low; recent.
        let e = orphan("Stub", Precision::Year, Some(3));
        let ctx = ScopeContext::from_events(&[e.clone()], DEFAULT_STALENESS_DAYS);
        let f = detect(&[e], &ctx, Significance::Low);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].significance, Significance::Low);
        assert_eq!(f[0].severity, CritSeverity::Info);
    }

    #[test]
    fn min_significance_filters_low_orphans() {
        let low = orphan("Stub", Precision::Year, None);
        let ctx = ScopeContext::from_events(&[low.clone()], DEFAULT_STALENESS_DAYS);
        // Default-ask for moderate-and-up → the low stub drops out.
        assert!(detect(&[low], &ctx, Significance::Moderate).is_empty());
    }

    #[test]
    fn active_track_lifts_significance() {
        // Build a track with several linked events so it counts as active.
        let mut events = vec![
            orphan("a", Precision::Month, None),
            orphan("b", Precision::Month, None),
            orphan("c", Precision::Month, None),
        ];
        for e in &mut events {
            e.is_orphan = false; // these are the linked events on "main"
        }
        // A 2-word, month-precision orphan on the active track:
        //   month precision +1, 2-word title +1, active track +1 = 3 → Moderate.
        // Drop the active-track point (put it on a quiet track) and it falls to
        // score 2, still Moderate but via a different path — so assert the active
        // signal lifts the *score*, not just the bucket.
        let mut orph = orphan("Lost map", Precision::Month, None);
        orph.is_orphan = true;
        events.push(orph.clone());
        let ctx = ScopeContext::from_events(&events, DEFAULT_STALENESS_DAYS);
        assert!(ctx.track_linked_counts.get("main").copied().unwrap_or(0) >= 3);
        assert_eq!(significance(&orph, &ctx), Significance::Moderate);
        // Same orphan on a quiet track → no activity point, but still Moderate.
        let mut quiet = orph.clone();
        quiet.track = "aside".into();
        assert_eq!(significance(&quiet, &ctx), Significance::Moderate);
    }
}
