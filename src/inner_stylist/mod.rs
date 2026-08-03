//! INNER-STYLIST-1 (CHORUS CH-P7) — the seventh inner-family reader: the book's
//! voice at scale. It doesn't measure; it *synthesises* — reading the CHORUS
//! pillars (character distinctiveness + drift, POV/head-hop, tense, register)
//! and turning the numbers into a few grounded Praise / Note / Concern
//! observations, in the book's language, and — on the slow track — LLM coaching
//! in the Inner-family voice ("I notice…", never a rewrite).
//!
//! Fast track ([`fast::synthesize`]) is deterministic and offline; the slow
//! track ([`slow`]) is the LLM coach; [`pipeline::gather`] runs the pillars and
//! synthesises; [`storage`] persists the author's suppressions.

pub(crate) mod fast;
pub(crate) mod pipeline;
pub(crate) mod slow;
pub(crate) mod storage;

/// A finding's weight. Like the Inner Poet, the Stylist earns its praise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Severity {
    Praise,
    Note,
    Concern,
}

impl Severity {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Severity::Praise => "Praise",
            Severity::Note => "Note",
            Severity::Concern => "Concern",
        }
    }
    pub(crate) fn glyph(self) -> &'static str {
        match self {
            Severity::Praise => "✓",
            Severity::Note => "·",
            Severity::Concern => "⚠",
        }
    }
}

/// One synthesised observation about the book's voice.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Finding {
    pub severity: Severity,
    /// The pillar: `distinctiveness` / `drift` / `pov` / `tense` / `register`.
    pub kind: &'static str,
    /// A stable fingerprint of the complaint — the author suppresses by key, so
    /// silencing survives the exact wording (which carries changing numbers).
    pub key: String,
    pub message: String,
}
