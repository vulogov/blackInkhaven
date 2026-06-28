//! DIALOG-1 — dialogue quality & attribution engine. Deterministic, zero-AI,
//! zero-runtime-dep dialogue detection over a book's chapters, in five embedded
//! languages (EN/RU/DE/FR/ES). Sibling of `crate::prose` (NARR-1); imports its
//! [`ProseLanguage`](crate::prose::ProseLanguage) key and modal word lists.
//!
//! Detection is **not** uniform across languages: three structurally distinct
//! quotation conventions need three strategies (see [`DialogueConvention`]).
//!
//! D-P0 lands the language model: the convention dispatch, the core span/
//! attribution types, and the embedded neutral + said-bookism verb lists
//! (`verbs`). Detectors land in D-P1, attribution in D-P2.

mod attribute;
mod detect;
mod verbs;

use crate::prose::ProseLanguage;

pub(crate) use attribute::{AttributionWindows, attribute_spans};
pub(crate) use detect::detect_spans;
pub(crate) use verbs::{DialogueLexicon, classify_tag_verb, lexicon_for};

/// The three structurally distinct dialogue-quotation conventions. Detection
/// strategy is selected per book from its language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DialogueConvention {
    /// Speech between an opening and closing quotation mark (EN, DE).
    QuotePair,
    /// Guillemets `«…»` and/or em-dash `— ` paragraph openers (FR, RU).
    GuillemetsAndDash,
    /// All three forms may appear; detectors run additively (ES).
    Hybrid,
}

/// How an individual span was detected (finer than the book-level
/// [`DialogueConvention`]): which mark form bracketed this particular span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpanForm {
    /// Between paired quotation marks (`"…"`, `“…”`, `„…“`).
    QuotePair,
    /// Between guillemets (`«…»`).
    Guillemet,
    /// Introduced by an em-dash paragraph opener (`— …`).
    EmDash,
}

impl SpanForm {
    pub(crate) fn as_code(&self) -> &'static str {
        match self {
            SpanForm::QuotePair => "quote_pair",
            SpanForm::Guillemet => "guillemet",
            SpanForm::EmDash => "em_dash",
        }
    }
}

/// Confidence that a detected span was spoken by a particular character. Only
/// `Certain` attributions feed the per-character fingerprint; `Inferred` is
/// stored but excluded from voice metrics to avoid propagating heuristic error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttributionConfidence {
    Certain,
    Inferred,
    None,
}

impl AttributionConfidence {
    pub(crate) fn as_code(&self) -> &'static str {
        match self {
            AttributionConfidence::Certain => "certain",
            AttributionConfidence::Inferred => "inferred",
            AttributionConfidence::None => "none",
        }
    }
}

/// Whether a dialogue-tag verb is an invisible "neutral" tag (said/asked) or a
/// said-bookism (whispered/growled) — the distinction the density metric counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TagVerbClass {
    Neutral,
    SaidBookism,
}

impl TagVerbClass {
    pub(crate) fn as_code(&self) -> &'static str {
        match self {
            TagVerbClass::Neutral => "neutral",
            TagVerbClass::SaidBookism => "said_bookism",
        }
    }
}

/// One detected unit of speech within a paragraph, plus its attribution signal.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DialogueSpan {
    pub para_id: String,
    /// Ordinal within the paragraph (0-based).
    pub span_index: u32,
    /// Which mark form bracketed this span.
    pub form: SpanForm,
    /// Paragraph char offsets of the span (open mark .. past close mark).
    /// Runtime-only — used by the attribution windows; not persisted.
    pub char_start: usize,
    pub char_end: usize,
    /// Content inside the marks, stripped of any inline tag (FR `dit-il`).
    pub speech_text: String,
    pub word_count: u32,
    /// Character name when attribution is `Certain`; `None` otherwise.
    pub attribution_name: Option<String>,
    pub attribution_conf: AttributionConfidence,
    /// Whether *any* attribution signal was found (name ≤60 tok · verb ≤15 ·
    /// action beat ≤30 · inline tag). Distinct from `attribution_conf`: the
    /// zero-attribution finding (§5.1) clears on any signal, while the
    /// fingerprint (§6.2) only counts `Certain`. Runtime-only; not persisted.
    pub has_attribution_signal: bool,
    /// The tag verb actually used, if one was found.
    pub tag_verb: Option<String>,
    pub tag_verb_class: Option<TagVerbClass>,
    pub ends_question: bool,
    pub ends_exclamation: bool,
}

/// Six measurable properties of a named character's speech, built from all
/// `Certain`-attributed spans across the book.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CharacterDialogueFingerprint {
    pub character_name: String,
    pub utterance_count: u32,
    pub mean_utterance_words: f32,
    pub utterance_mattr: f32,
    pub question_ratio: f32,
    pub exclamation_ratio: f32,
    pub hedge_density: f32,
}

/// Select the detection convention for a book's language. `Other` falls back to
/// quote-pair (the most widespread convention) per RFC §4.4.
pub(crate) fn dialogue_convention(lang: &ProseLanguage) -> DialogueConvention {
    match lang {
        ProseLanguage::En | ProseLanguage::De => DialogueConvention::QuotePair,
        ProseLanguage::Fr | ProseLanguage::Ru => DialogueConvention::GuillemetsAndDash,
        ProseLanguage::Es => DialogueConvention::Hybrid,
        ProseLanguage::Other(_) => DialogueConvention::QuotePair,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convention_dispatch_matches_rfc_table() {
        assert_eq!(dialogue_convention(&ProseLanguage::En), DialogueConvention::QuotePair);
        assert_eq!(dialogue_convention(&ProseLanguage::De), DialogueConvention::QuotePair);
        assert_eq!(
            dialogue_convention(&ProseLanguage::Fr),
            DialogueConvention::GuillemetsAndDash
        );
        assert_eq!(
            dialogue_convention(&ProseLanguage::Ru),
            DialogueConvention::GuillemetsAndDash
        );
        assert_eq!(dialogue_convention(&ProseLanguage::Es), DialogueConvention::Hybrid);
        // Unsupported language → quote-pair fallback.
        assert_eq!(
            dialogue_convention(&ProseLanguage::Other("pl".into())),
            DialogueConvention::QuotePair
        );
    }
}
