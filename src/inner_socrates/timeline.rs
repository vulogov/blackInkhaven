//! Timeline awareness for the Slow track (RFC §3.4 / §8.5). The three
//! timeline-aware categories read the existing timeline feature's events —
//! nodes carrying `EventData` plus their `linked_paragraphs` — and ask whether
//! the prose has absorbed what the timeline declares:
//!
//! - **dramatization_gap** — events no paragraph depicts (`linked_paragraphs`
//!   empty);
//! - **implication_tracing** — events whose consequences should ripple forward;
//! - **temporal_density** — clusters of events the prose's rhythm should reflect.
//!
//! The analysis functions are pure (over a `&[SocEvent]`); `gather_events` is the
//! thin adapter over the project hierarchy. When a project has no events the whole
//! pass is silently skipped (no spurious output).

use uuid::Uuid;

use crate::store::hierarchy::Hierarchy;

/// A lightweight projection of a timeline event for the Socratic pass.
#[derive(Debug, Clone, PartialEq)]
pub struct SocEvent {
    pub id: Uuid,
    pub title: String,
    pub start_ticks: i64,
    /// How many paragraphs depict this event (`0` = a dramatization gap).
    pub depicted_by: usize,
    pub track: Option<String>,
}

/// Gather the project's events from the hierarchy (nodes carrying `EventData`),
/// ordered by world-time. Thin I/O adapter; the analysis below is pure.
pub fn gather_events(hierarchy: &Hierarchy) -> Vec<SocEvent> {
    let mut out: Vec<SocEvent> = hierarchy
        .iter()
        .filter_map(|n| {
            let ev = n.event.as_ref()?;
            Some(SocEvent {
                id: n.id,
                title: n.title.clone(),
                start_ticks: ev.start_ticks,
                depicted_by: n.linked_paragraphs.len(),
                track: ev.track.clone(),
            })
        })
        .collect();
    out.sort_by_key(|e| e.start_ticks);
    out
}

/// The events no paragraph depicts — dramatization-gap candidates.
pub fn dramatization_gaps(events: &[SocEvent]) -> Vec<&SocEvent> {
    events.iter().filter(|e| e.depicted_by == 0).collect()
}

/// The densest window: the maximum number of events whose start falls within any
/// `window` ticks. A high count over a short window is what `temporal_density`
/// asks the prose to honour.
pub fn densest_window(events: &[SocEvent], window: i64) -> usize {
    if events.is_empty() || window <= 0 {
        return events.len().min(1);
    }
    // `events` is sorted; slide a right edge and count how many starts are within
    // `window` of each left anchor.
    let starts: Vec<i64> = events.iter().map(|e| e.start_ticks).collect();
    let mut best = 1usize;
    let mut lo = 0usize;
    for hi in 0..starts.len() {
        while starts[hi] - starts[lo] > window {
            lo += 1;
        }
        best = best.max(hi - lo + 1);
    }
    best
}

/// A compact summary of the timeline for the LLM prompt: each event, its time,
/// and whether it is depicted, with the gaps called out.
pub fn timeline_summary(events: &[SocEvent]) -> String {
    if events.is_empty() {
        return "None.".to_string();
    }
    let mut s = String::new();
    for e in events {
        let mark = if e.depicted_by == 0 { "UNDEPICTED" } else { "depicted" };
        let track = e.track.as_deref().map(|t| format!(" [{t}]")).unwrap_or_default();
        s.push_str(&format!("- t={} {}{}: {}\n", e.start_ticks, e.title, track, mark));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(title: &str, t: i64, depicted: usize) -> SocEvent {
        SocEvent { id: Uuid::nil(), title: title.into(), start_ticks: t, depicted_by: depicted, track: None }
    }

    #[test]
    fn finds_dramatization_gaps() {
        let events = vec![ev("Coronation", 10, 3), ev("Secret pact", 20, 0), ev("Famine", 30, 0)];
        let gaps = dramatization_gaps(&events);
        assert_eq!(gaps.len(), 2);
        assert_eq!(gaps[0].title, "Secret pact");
    }

    #[test]
    fn summary_marks_undepicted() {
        let s = timeline_summary(&[ev("Coronation", 10, 2), ev("Pact", 20, 0)]);
        assert!(s.contains("Coronation [") || s.contains("Coronation: depicted"));
        assert!(s.contains("Pact: UNDEPICTED"));
        assert_eq!(timeline_summary(&[]), "None.");
    }

    #[test]
    fn densest_window_counts_clusters() {
        // Three events within 5 ticks, then one far away.
        let events = vec![ev("a", 100, 1), ev("b", 102, 1), ev("c", 104, 1), ev("d", 900, 1)];
        assert_eq!(densest_window(&events, 5), 3);
        assert_eq!(densest_window(&events, 1000), 4);
        assert_eq!(densest_window(&[], 10), 0);
    }
}
