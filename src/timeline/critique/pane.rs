//! PANE-1 emission for the retained timeline-critique findings
//! (TIMELINE-2-INTEGRATION P1).
//!
//! Orphan and fuzzy-overlap findings emit to the Output pane as structured
//! messages — `timeline_orphan_warning` / `timeline_fuzzy_overlap_warning` — with
//! `timeline-critique` provenance, joining the fact-checker's and the Socratic
//! reader's findings on the same surface with the same primitives. The headline
//! text is assembled here from caller-resolved labels (the caller owns the calendar
//! + hierarchy needed to turn ticks → dates and UUIDs → names). Localization of the
//! text lands in P5; for now `body_en` mirrors `text`.

use super::types::{CritSeverity, FuzzyOverlapFinding, OrphanFinding, Significance, Staleness, Suspicion};
use crate::pane::output::{kinds, Lifetime, Message, Severity};

/// The source label these findings carry, analogous to `inner-socrates`.
pub const PROVENANCE: &str = "timeline-critique";

fn map_severity(s: CritSeverity) -> Severity {
    match s {
        CritSeverity::Info => Severity::Info,
        CritSeverity::Warning => Severity::Warning,
        CritSeverity::Contradiction => Severity::Contradiction,
    }
}

fn significance_str(s: Significance) -> &'static str {
    match s {
        Significance::Low => "low",
        Significance::Moderate => "moderate",
        Significance::High => "high",
    }
}

fn staleness_str(s: Staleness) -> &'static str {
    match s {
        Staleness::Recent => "recent",
        Staleness::Old => "old",
    }
}

fn suspicion_str(s: Suspicion) -> &'static str {
    match s {
        Suspicion::Low => "low",
        Suspicion::Moderate => "moderate",
        Suspicion::High => "high",
        Suspicion::Cluster => "cluster",
    }
}

/// Emit an orphan finding. `date_label` is the event's start formatted by the
/// caller's calendar (e.g. `"year 120"`). `elaboration` is the optional LLM text
/// (P2); when present it's appended to the headline and stored in metadata.
pub fn emit_orphan(f: &OrphanFinding, date_label: &str, elaboration: Option<&str>) {
    let mut headline = format!(
        "\"{}\" ({}, \"{}\" track) — {}",
        f.title,
        date_label,
        f.track,
        f.reasons.join(" ")
    );
    if let Some(extra) = elaboration {
        headline.push(' ');
        headline.push_str(extra);
    }
    let meta = serde_json::json!({
        "text": headline,
        "body_en": headline,
        "category": "orphan",
        "provenance": PROVENANCE,
        "timeline": true,
        "event_ids": [f.event_id.to_string()],
        "severity_label": f.severity.label(),
        "significance": significance_str(f.significance),
        "staleness": staleness_str(f.staleness),
        "age_days": f.age_days,
        "track": f.track,
        "reasons": f.reasons,
        "elaboration": elaboration,
    });
    let msg = Message::new(
        kinds::TIMELINE_ORPHAN_WARNING,
        map_severity(f.severity),
        Lifetime::UntilActedOn,
        meta,
    );
    crate::pane::output::emit(&msg);
}

/// Emit a fuzzy-overlap finding (pair or cluster). `window_label` is the overlap
/// window formatted by the caller's calendar; `char_names` / `place_names` are the
/// resolved shared-entity names (may be empty).
pub fn emit_overlap(
    f: &FuzzyOverlapFinding,
    window_label: &str,
    char_names: &[String],
    place_names: &[String],
    elaboration: Option<&str>,
) {
    let titles = f
        .titles
        .iter()
        .map(|t| format!("\"{t}\""))
        .collect::<Vec<_>>()
        .join(" + ");
    let mut headline = if f.is_cluster {
        format!(
            "Cluster of {} fuzzy events overlapping {}: {} — {}",
            f.event_ids.len(),
            window_label,
            titles,
            f.reasons.join(" ")
        )
    } else {
        format!("{} overlap {} — {}", titles, window_label, f.reasons.join(" "))
    };
    if let Some(extra) = elaboration {
        headline.push(' ');
        headline.push_str(extra);
    }
    let meta = serde_json::json!({
        "text": headline,
        "body_en": headline,
        "category": "fuzzy_overlap",
        "provenance": PROVENANCE,
        "timeline": true,
        "event_ids": f.event_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
        "severity_label": f.severity.label(),
        "suspicion": suspicion_str(f.suspicion),
        "is_cluster": f.is_cluster,
        "track": f.track,
        "shared_characters": char_names,
        "shared_places": place_names,
        "reasons": f.reasons,
        "elaboration": elaboration,
    });
    let msg = Message::new(
        kinds::TIMELINE_FUZZY_OVERLAP_WARNING,
        map_severity(f.severity),
        Lifetime::UntilActedOn,
        meta,
    );
    crate::pane::output::emit(&msg);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::Precision;
    use uuid::Uuid;

    #[test]
    fn orphan_headline_carries_title_track_and_reason() {
        let f = OrphanFinding {
            event_id: Uuid::nil(),
            title: "The Coronation of Hadrin III".into(),
            track: "main".into(),
            start_ticks: 0,
            precision: Precision::Day,
            significance: Significance::High,
            staleness: Staleness::Old,
            age_days: Some(92),
            severity: CritSeverity::Contradiction,
            reasons: vec!["Orphaned for 92 days.".into()],
        };
        // Build the message the same way emit_orphan does, without a live store.
        let headline = format!(
            "\"{}\" ({}, \"{}\" track) — {}",
            f.title, "year 120", f.track, f.reasons.join(" ")
        );
        assert!(headline.contains("Coronation"));
        assert!(headline.contains("\"main\" track"));
        assert!(headline.contains("92 days"));
        assert_eq!(map_severity(f.severity), Severity::Contradiction);
    }

    #[test]
    fn severity_mapping_is_total() {
        assert_eq!(map_severity(CritSeverity::Info), Severity::Info);
        assert_eq!(map_severity(CritSeverity::Warning), Severity::Warning);
        assert_eq!(map_severity(CritSeverity::Contradiction), Severity::Contradiction);
    }
}
