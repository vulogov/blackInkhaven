//! Bridging Socratic findings to the PANE-1 Output pane. A finding emits as a
//! `socratic_inquiry` message carrying its persona provenance, category, track,
//! and the question (in the paragraph's language, plus the English fallback for
//! the AI bridge). Mirrors `world::fact_check::emit_finding`.

use super::types::{Severity, SocraticFinding, Track};

/// Emit a Socratic finding to the Output pane. `source` is the paragraph the
/// finding is about, so a re-check can clear that paragraph's prior findings
/// first. A no-op when no Output store is installed (headless CLI).
pub fn emit_finding(f: &SocraticFinding, source: Option<uuid::Uuid>) {
    use crate::pane::output::{kinds, Lifetime, Message, Severity as OutSev};
    let severity = match f.severity {
        Severity::Probe => OutSev::Contradiction,
        Severity::Inquiry => OutSev::Warning,
        Severity::Notice => OutSev::Info,
    };
    let track = match f.category.track() {
        Track::Fast => "fast",
        Track::Slow => "slow",
    };
    let mut msg = Message::new(
        kinds::SOCRATIC_INQUIRY,
        severity,
        Lifetime::UntilActedOn,
        serde_json::json!({
            "text": f.question,
            "question_en": f.question_en,
            "category": f.category.id(),
            "label": f.category.label(),
            "persona_id": f.persona_id,
            "track": track,
            "severity_label": f.severity.label(),
            "suppressed_by": f.suppressed_by,
        }),
    );
    if let Some(id) = source {
        msg = msg.with_source_paragraph(id);
    }
    crate::pane::output::emit(&msg);
}
