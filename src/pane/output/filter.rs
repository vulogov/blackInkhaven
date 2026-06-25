//! Output-pane filtering (road to 1.4.0).
//!
//! The Output pane gathers findings from every subsystem onto one surface. This is
//! the read-side view that narrows it by **source** (which subsystem), **severity**
//! (hide the low-signal noise), and **the open paragraph**. Pure: a predicate over
//! a [`Message`] plus the optional open-paragraph id. The pane filters its
//! `active()` fetch through [`OutputFilter::matches`] each frame (P1); no schema or
//! store change.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::types::{kinds, Message, Severity};

/// The human-facing **source** groups, in cycle order. `message_source` maps every
/// message kind onto one of these.
pub const SOURCES: &[&str] = &[
    // COMPANIONS-1 — a meta-source: the examined-authorship companions together
    // (fact-check + socrates + inner-editor + timeline-critique). Not a real
    // `message_source` output; `Filter::matches` expands it.
    "companions",
    "fact-check",
    "socrates",
    "inner-editor",
    "timeline-critique",
    "world",
    "translation",
    "lexicon",
    "variety",
    "ai",
    "bund",
    "other",
];

/// The examined-authorship companion sources the `companions` meta-filter shows.
pub const COMPANION_SOURCES: &[&str] = &["fact-check", "socrates", "inner-editor", "timeline-critique"];

/// Whether a `message_source` group is one of the examined-authorship companions.
pub fn is_companion_source(source: &str) -> bool {
    COMPANION_SOURCES.contains(&source)
}

/// Classify a message into one human source group (stable; the filter's primary
/// dimension). Keyed off `kind` — the single reliable discriminator (provenance in
/// `metadata` is inconsistent across kinds).
pub fn message_source(msg: &Message) -> &'static str {
    match msg.kind.as_str() {
        kinds::FACT_CHECK_WARNING => "fact-check",
        kinds::SOCRATIC_INQUIRY => "socrates",
        kinds::INNER_EDITOR_OBSERVATION => "inner-editor",
        kinds::TIMELINE_ORPHAN_WARNING | kinds::TIMELINE_FUZZY_OVERLAP_WARNING => {
            "timeline-critique"
        }
        kinds::WORLD_COMPILER_PROPOSAL => "world",
        kinds::TRANSLATION_RESULT
        | kinds::TRANSLATION_MEMORY_LISTING
        | kinds::TRANSLATION_CORPUS_PROGRESS
        | kinds::TRANSLATION_EVAL_RESULT
        | kinds::TRANSLATION_EXPORT_RESULT
        | kinds::TRANSLATION_UNCOVERED_WORD_REPORT => "translation",
        kinds::LEXICON_PROPOSAL => "lexicon",
        kinds::VARIETY_RENDERING => "variety",
        kinds::AI_TASK_COMPLETE => "ai",
        kinds::BUND_PRINT | kinds::BUND_LOG => "bund",
        _ => "other",
    }
}

/// A linear rank for the severity filter. `Progress` (transient task ticks) ranks
/// lowest so a "warnings and up" filter hides it along with `Info`.
fn severity_rank(s: Severity) -> u8 {
    match s {
        Severity::Progress => 0,
        Severity::Info => 1,
        Severity::Warning => 2,
        Severity::Contradiction => 3,
    }
}

/// The active Output-pane filter. All-`None`/`false` means "show everything"
/// ([`is_active`] is then false). Serializable for session persistence (P2).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputFilter {
    /// Show only this source group (see [`SOURCES`]); `None` = every source.
    pub source: Option<String>,
    /// Show only messages at or above this severity; `None` = every severity.
    pub min_severity: Option<Severity>,
    /// Show only messages about the currently-open paragraph.
    pub only_open_paragraph: bool,
}

impl OutputFilter {
    /// Whether any narrowing is in effect.
    pub fn is_active(&self) -> bool {
        self.source.is_some() || self.min_severity.is_some() || self.only_open_paragraph
    }

    /// Reset to "show everything".
    pub fn clear(&mut self) {
        *self = OutputFilter::default();
    }

    /// Does `msg` pass the filter? `open_paragraph` is the id of the paragraph the
    /// editor currently has open (for the `only_open_paragraph` dimension).
    pub fn matches(&self, msg: &Message, open_paragraph: Option<Uuid>) -> bool {
        if let Some(src) = self.source.as_deref() {
            if src == "companions" {
                if !is_companion_source(message_source(msg)) {
                    return false;
                }
            } else if message_source(msg) != src {
                return false;
            }
        }
        if let Some(min) = self.min_severity {
            if severity_rank(msg.severity) < severity_rank(min) {
                return false;
            }
        }
        if self.only_open_paragraph
            && (open_paragraph.is_none() || msg.source_paragraph_id != open_paragraph)
        {
            return false;
        }
        true
    }

    /// A compact one-line description for the pane header (empty when inactive).
    pub fn summary(&self) -> String {
        if !self.is_active() {
            return String::new();
        }
        let mut parts = Vec::new();
        if let Some(src) = &self.source {
            parts.push(format!("src:{src}"));
        }
        if let Some(min) = self.min_severity {
            parts.push(format!("≥{}", min.as_str()));
        }
        if self.only_open_paragraph {
            parts.push("¶ this paragraph".to_string());
        }
        parts.join(" · ")
    }

    /// Cycle the source dimension: None → SOURCES[0] → … → last → None.
    pub fn cycle_source(&mut self) {
        self.source = match &self.source {
            None => Some(SOURCES[0].to_string()),
            Some(cur) => {
                let idx = SOURCES.iter().position(|s| s == cur);
                match idx {
                    Some(i) if i + 1 < SOURCES.len() => Some(SOURCES[i + 1].to_string()),
                    _ => None,
                }
            }
        };
    }

    /// Cycle the min-severity dimension: None → Info → Warning → Contradiction → None.
    pub fn cycle_min_severity(&mut self) {
        self.min_severity = match self.min_severity {
            None => Some(Severity::Info),
            Some(Severity::Info) => Some(Severity::Warning),
            Some(Severity::Warning) => Some(Severity::Contradiction),
            Some(Severity::Contradiction) => None,
            // Progress is never a target; collapse to None.
            Some(Severity::Progress) => None,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane::output::{Lifetime, Message};

    fn msg(kind: &str, severity: Severity, para: Option<Uuid>) -> Message {
        let mut m = Message::new(kind, severity, Lifetime::UntilActedOn, serde_json::json!({}));
        if let Some(p) = para {
            m = m.with_source_paragraph(p);
        }
        m
    }

    #[test]
    fn source_classification_covers_known_kinds() {
        assert_eq!(message_source(&msg(kinds::FACT_CHECK_WARNING, Severity::Warning, None)), "fact-check");
        assert_eq!(message_source(&msg(kinds::SOCRATIC_INQUIRY, Severity::Info, None)), "socrates");
        assert_eq!(message_source(&msg(kinds::TIMELINE_ORPHAN_WARNING, Severity::Warning, None)), "timeline-critique");
        assert_eq!(message_source(&msg(kinds::TRANSLATION_RESULT, Severity::Info, None)), "translation");
        assert_eq!(message_source(&msg("something_unknown", Severity::Info, None)), "other");
    }

    #[test]
    fn empty_filter_matches_everything() {
        let f = OutputFilter::default();
        assert!(!f.is_active());
        assert!(f.matches(&msg(kinds::FACT_CHECK_WARNING, Severity::Info, None), None));
        assert!(f.summary().is_empty());
    }

    #[test]
    fn source_filter_narrows() {
        let mut f = OutputFilter::default();
        f.source = Some("socrates".into());
        assert!(f.is_active());
        assert!(f.matches(&msg(kinds::SOCRATIC_INQUIRY, Severity::Info, None), None));
        assert!(!f.matches(&msg(kinds::FACT_CHECK_WARNING, Severity::Info, None), None));
    }

    // COMPANIONS-1 — the `companions` meta-source shows all four examined-
    // authorship companions and nothing else.
    #[test]
    fn companions_meta_source_matches_all_companions() {
        let mut f = OutputFilter::default();
        f.source = Some("companions".into());
        for k in [
            kinds::FACT_CHECK_WARNING,
            kinds::SOCRATIC_INQUIRY,
            kinds::INNER_EDITOR_OBSERVATION,
            kinds::TIMELINE_ORPHAN_WARNING,
        ] {
            assert!(f.matches(&msg(k, Severity::Warning, None), None), "{k} should match companions");
        }
        // Non-companion kinds are excluded.
        assert!(!f.matches(&msg(kinds::TRANSLATION_RESULT, Severity::Info, None), None));
        assert!(!f.matches(&msg(kinds::BUND_PRINT, Severity::Info, None), None));
        // `companions` is in the cycle (so `f` reaches it).
        assert!(SOURCES.contains(&"companions"));
    }

    #[test]
    fn min_severity_hides_lower_and_progress() {
        let mut f = OutputFilter::default();
        f.min_severity = Some(Severity::Warning);
        assert!(f.matches(&msg(kinds::FACT_CHECK_WARNING, Severity::Warning, None), None));
        assert!(f.matches(&msg(kinds::FACT_CHECK_WARNING, Severity::Contradiction, None), None));
        assert!(!f.matches(&msg(kinds::FACT_CHECK_WARNING, Severity::Info, None), None));
        assert!(!f.matches(&msg(kinds::AI_TASK_COMPLETE, Severity::Progress, None), None));
    }

    #[test]
    fn only_open_paragraph_requires_a_match() {
        let para = Uuid::new_v4();
        let other = Uuid::new_v4();
        let mut f = OutputFilter::default();
        f.only_open_paragraph = true;
        assert!(f.matches(&msg(kinds::FACT_CHECK_WARNING, Severity::Info, Some(para)), Some(para)));
        assert!(!f.matches(&msg(kinds::FACT_CHECK_WARNING, Severity::Info, Some(other)), Some(para)));
        // No paragraph open, or message not tied to a paragraph → hidden.
        assert!(!f.matches(&msg(kinds::FACT_CHECK_WARNING, Severity::Info, Some(para)), None));
        assert!(!f.matches(&msg(kinds::BUND_PRINT, Severity::Info, None), Some(para)));
    }

    #[test]
    fn cycle_source_wraps_through_all_then_off() {
        let mut f = OutputFilter::default();
        for expected in SOURCES {
            f.cycle_source();
            assert_eq!(f.source.as_deref(), Some(*expected));
        }
        f.cycle_source();
        assert_eq!(f.source, None, "cycles back to off after the last source");
    }

    #[test]
    fn cycle_min_severity_loops() {
        let mut f = OutputFilter::default();
        f.cycle_min_severity();
        assert_eq!(f.min_severity, Some(Severity::Info));
        f.cycle_min_severity();
        assert_eq!(f.min_severity, Some(Severity::Warning));
        f.cycle_min_severity();
        assert_eq!(f.min_severity, Some(Severity::Contradiction));
        f.cycle_min_severity();
        assert_eq!(f.min_severity, None);
    }

    #[test]
    fn summary_is_compact_and_nonempty_when_active() {
        let mut f = OutputFilter { source: Some("fact-check".into()), ..Default::default() };
        f.min_severity = Some(Severity::Warning);
        f.only_open_paragraph = true;
        let s = f.summary();
        assert!(s.contains("fact-check"));
        assert!(s.contains("warning"));
        assert!(s.contains('¶'));
    }

    // Road-to-1.4.0 stability rider — the filter runs over untrusted message data
    // (arbitrary `kind` strings) every frame; it must never panic.
    mod prop {
        use super::super::{message_source, OutputFilter, SOURCES};
        use crate::pane::output::{Lifetime, Message, Severity};
        use proptest::prelude::*;

        fn arb_severity() -> impl Strategy<Value = Severity> {
            prop_oneof![
                Just(Severity::Info),
                Just(Severity::Warning),
                Just(Severity::Contradiction),
                Just(Severity::Progress),
            ]
        }

        fn message(kind: &str, sev: Severity) -> Message {
            Message::new(kind, sev, Lifetime::UntilActedOn, serde_json::json!({}))
        }

        proptest! {
            /// Every message — including unknown kinds — classifies to a known source.
            #[test]
            fn source_is_always_in_the_known_set(kind in ".{0,24}", sev in arb_severity()) {
                prop_assert!(SOURCES.contains(&message_source(&message(&kind, sev))));
            }

            /// `matches` never panics on any message / filter / open-paragraph combo.
            #[test]
            fn matches_never_panics(
                kind in ".{0,24}",
                sev in arb_severity(),
                src in proptest::option::of("[a-z-]{0,16}"),
                only in any::<bool>(),
            ) {
                let m = message(&kind, sev);
                let f = OutputFilter { source: src, min_severity: Some(sev), only_open_paragraph: only };
                let _ = f.matches(&m, None);
                let _ = f.matches(&m, Some(uuid::Uuid::new_v4()));
            }

            /// A source filter passes a message iff it filters for that message's own
            /// source; any other source rejects it.
            #[test]
            fn source_filter_is_exact(kind in ".{0,24}", sev in arb_severity()) {
                let m = message(&kind, sev);
                let own = message_source(&m).to_string();
                let pass = OutputFilter { source: Some(own.clone()), ..Default::default() };
                prop_assert!(pass.matches(&m, None));
                let other = if own == "other" { "fact-check" } else { "other" };
                let reject = OutputFilter { source: Some(other.to_string()), ..Default::default() };
                prop_assert!(!reject.matches(&m, None));
            }
        }
    }
}
