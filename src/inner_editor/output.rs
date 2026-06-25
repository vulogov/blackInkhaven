//! Bridging Editor findings to the PANE-1 Output pane. A finding emits as an
//! `inner_editor_observation` message carrying its provenance (`inner-editor`),
//! severity (Praise/Note/Concern → Info/Warning/Contradiction), category, the
//! observation (in the paragraph's language, plus the English fallback), and the
//! grounding evidence. Mirrors `inner_socrates::output::emit_finding`.
//!
//! The visible-by-default threshold (RFC §3.4) is applied at the emit site via
//! [`meets_threshold`]: at the default `note`, Praise is persisted but not
//! pushed to Output (the author opts in by lowering the threshold or listing
//! findings explicitly).

use super::types::{EditorFinding, EditorSeverity};

/// Whether a finding of `sev` should surface given the configured
/// `severity_threshold` string (`praise` | `note` | `concern`; default `note`).
pub fn meets_threshold(sev: EditorSeverity, threshold: &str) -> bool {
    let floor = EditorSeverity::from_id(threshold);
    sev.rank() >= floor.rank()
}

/// Emit an Editor finding to the Output pane. `source` is the paragraph the
/// finding is about (so a re-engagement can clear its prior findings). A no-op
/// when no Output store is installed (headless CLI).
pub fn emit_finding(f: &EditorFinding, source: Option<uuid::Uuid>) {
    use crate::pane::output::{kinds, Lifetime, Message, Severity as OutSev};
    let severity = match f.severity {
        EditorSeverity::Concern => OutSev::Contradiction,
        EditorSeverity::Note => OutSev::Warning,
        EditorSeverity::Praise => OutSev::Info,
    };
    let mut msg = Message::new(
        kinds::INNER_EDITOR_OBSERVATION,
        severity,
        Lifetime::UntilActedOn,
        serde_json::json!({
            "text": f.observation,
            "observation_en": f.observation_en,
            "category": f.category.id(),
            "label": f.category.label(),
            "severity_label": f.severity.label(),
            "evidence": f.evidence,
            "conditional": f.conditional,
            "suppressed_by": f.suppressed_by,
            "provenance": "inner-editor",
        }),
    );
    if let Some(id) = source {
        msg = msg.with_source_paragraph(id);
    }
    crate::pane::output::emit(&msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_hides_praise_at_default_note() {
        assert!(!meets_threshold(EditorSeverity::Praise, "note"));
        assert!(meets_threshold(EditorSeverity::Note, "note"));
        assert!(meets_threshold(EditorSeverity::Concern, "note"));
        // Lower the threshold and Praise surfaces.
        assert!(meets_threshold(EditorSeverity::Praise, "praise"));
        // Raise it and only Concern surfaces.
        assert!(!meets_threshold(EditorSeverity::Note, "concern"));
        assert!(meets_threshold(EditorSeverity::Concern, "concern"));
        // Unknown threshold → treated as the default `note`.
        assert!(!meets_threshold(EditorSeverity::Praise, "garbage"));
    }
}
