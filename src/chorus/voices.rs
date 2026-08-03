//! CHORUS-1 (CH-P1) — character voice fingerprints.
//!
//! Group every attributed dialogue line by speaker, then run the NARR-1 metric
//! core (`compute_profile_with`) over each character's aggregated dialogue — the
//! *same* engine that profiles the narrator, so a character's voice is measured
//! on the same axes (rhythm, lexical diversity, hedging, interiority, …). The
//! full `VoiceProfile` supersedes DIALOG-1's lightweight
//! `CharacterDialogueFingerprint`; the lightweight one stays as the fast summary.
//!
//! The distinctiveness matrix (CH-P2) and per-character drift (CH-P3) build on
//! the profiles this module produces.

use std::collections::BTreeMap;

use anyhow::Result;

use crate::config::Config;
use crate::dialogue::{DialogueSpan, DialogueStore};
use crate::prose::{
    CompiledLexicon, ProseStore, VoiceProfile, VoiceScope, compute_profile_with,
    resolve_prose_language,
};
use crate::store::node::Node;

/// How much dialogue backs a character's voice. A profile built on a handful of
/// lines is noise; downstream (CH-P2 distinctiveness) refuses to *flag* a `Low`
/// voice, though it still computes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Confidence {
    Low,
    Medium,
    High,
}

impl Confidence {
    /// Heuristic and deliberately conservative — a voice needs enough utterances
    /// *and* words before its rhythm/diversity carry meaning. (Thresholds are a
    /// starting point; CH-P2 calibrates the distinctiveness gate on top.)
    pub(crate) fn from_counts(utterances: u32, words: u32) -> Self {
        if utterances < 5 || words < 40 {
            Confidence::Low
        } else if utterances < 20 || words < 300 {
            Confidence::Medium
        } else {
            Confidence::High
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Confidence::Low => "low",
            Confidence::Medium => "medium",
            Confidence::High => "high",
        }
    }

    /// Whether a voice has enough dialogue to take part in distinctiveness /
    /// discipline comparisons. `Low` voices are computed but never flagged.
    pub(crate) fn is_comparable(self) -> bool {
        matches!(self, Confidence::Medium | Confidence::High)
    }
}

/// One character's aggregated dialogue, ready to profile, with utterance/word
/// counts. (CH-P3 adds per-chapter slices here for drift.)
pub(crate) struct CharacterCorpus {
    pub all: String,
    pub utterances: u32,
    pub words: u32,
}

/// A character's voice — the FULL NARR-1 profile over their dialogue, plus a
/// confidence from corpus size. (CH-P3 adds per-chapter profiles for drift.)
pub(crate) struct CharacterVoice {
    pub name: String,
    pub profile: VoiceProfile,
    pub confidence: Confidence,
    pub utterances: u32,
}

/// PURE — group attributed spans into per-character corpora. The grouping key is
/// the attribution name verbatim (roster-canonical); lines join with a space.
/// Unattributed / empty spans are skipped (they shouldn't reach here from
/// `attributed_spans`, but be defensive).
pub(crate) fn character_corpora(spans: &[(u32, DialogueSpan)]) -> BTreeMap<String, CharacterCorpus> {
    let mut out: BTreeMap<String, CharacterCorpus> = BTreeMap::new();
    for (_ord, span) in spans {
        let Some(name) = span.attribution_name.as_deref().map(str::trim).filter(|s| !s.is_empty())
        else {
            continue;
        };
        let line = span.speech_text.trim();
        if line.is_empty() {
            continue;
        }
        let entry = out.entry(name.to_string()).or_insert_with(|| CharacterCorpus {
            all: String::new(),
            utterances: 0,
            words: 0,
        });
        push_line(&mut entry.all, line);
        entry.utterances += 1;
        entry.words += span.word_count;
    }
    out
}

fn push_line(buf: &mut String, line: &str) {
    if !buf.is_empty() {
        buf.push(' ');
    }
    buf.push_str(line);
}

/// Compute (and persist) every character's voice profile for `book`. Assumes the
/// dialogue spans are already refreshed (the caller runs `dialogue::refresh_book`
/// first). Each aggregate profile is upserted under `VoiceScope::Character` in
/// `prose.duckdb`; the per-chapter profiles are returned in-memory for CH-P3.
/// Returns the voices sorted by name.
pub(crate) fn character_profiles(
    pstore: &ProseStore,
    dstore: &DialogueStore,
    cfg: &Config,
    book: &Node,
    explicit_lang: Option<&str>,
    now: &str,
) -> Result<Vec<CharacterVoice>> {
    let (lang, _note) = resolve_prose_language(explicit_lang, &cfg.language);
    // One lexicon for the whole book, folding in the project's `prose.extra_*`.
    let lx = CompiledLexicon::for_language_with(
        &lang,
        &cfg.prose.extra_modal_tokens,
        &cfg.prose.extra_interiority_phrases,
    );
    let deep = cfg.prose.deep_metrics;
    let window = cfg.prose.mattr_window;

    let spans = dstore.attributed_spans(&book.slug)?;
    let corpora = character_corpora(&spans);

    let mut voices = Vec::with_capacity(corpora.len());
    for (name, corpus) in corpora {
        let profile = compute_profile_with(
            &corpus.all,
            VoiceScope::Character(name.clone()),
            &lang,
            &lx,
            deep,
            window,
        );
        pstore.upsert(&book.slug, &profile, now)?;

        voices.push(CharacterVoice {
            name,
            profile,
            confidence: Confidence::from_counts(corpus.utterances, corpus.words),
            utterances: corpus.utterances,
        });
    }
    voices.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(voices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogue::{AttributionConfidence, SpanForm};
    use crate::prose::ProseLanguage;

    fn span(ord: u32, name: &str, text: &str) -> (u32, DialogueSpan) {
        (
            ord,
            DialogueSpan {
                para_id: "p".into(),
                span_index: 0,
                form: SpanForm::QuotePair,
                char_start: 0,
                char_end: 0,
                speech_text: text.into(),
                word_count: text.split_whitespace().count() as u32,
                attribution_name: Some(name.into()),
                attribution_conf: AttributionConfidence::Certain,
                has_attribution_signal: true,
                tag_verb: None,
                tag_verb_class: None,
                ends_question: false,
                ends_exclamation: false,
            },
        )
    }

    #[test]
    fn corpora_group_by_speaker() {
        let spans = vec![
            span(1, "Mara", "The tide is turning."),
            span(1, "Joren", "Is it?"),
            span(2, "Mara", "Look at the water."),
        ];
        let c = character_corpora(&spans);
        assert_eq!(c.len(), 2);
        let mara = &c["Mara"];
        assert_eq!(mara.utterances, 2);
        // Both of Mara's lines, across chapters, join into one corpus.
        assert!(mara.all.contains("tide") && mara.all.contains("water"));
        assert_eq!(mara.words, 8);
        assert_eq!(c["Joren"].utterances, 1);
    }

    #[test]
    fn empty_and_unattributed_spans_are_skipped() {
        let mut blank = span(1, "Ghost", "   ");
        let mut noname = span(1, "X", "Hello");
        noname.1.attribution_name = None;
        blank.1.speech_text = "   ".into();
        let c = character_corpora(&[blank, noname]);
        assert!(c.is_empty());
    }

    #[test]
    fn the_shared_engine_profiles_distinct_voices_distinctly() {
        // Two corpora, one clipped and one flowing, profiled by the SAME core the
        // narrator uses → measurably different rhythm.
        let lx = CompiledLexicon::for_language_with(&ProseLanguage::En, &[], &[]);
        let clipped = "Yes. No. Maybe. Fine. Go. Stop. Now. Wait.";
        let flowing = "The evening light fell slowly across the wide and silent water, \
                       and she wondered whether the tide would ever turn again before dawn.";
        let a = compute_profile_with(
            clipped,
            VoiceScope::Character("A".into()),
            &ProseLanguage::En,
            &lx,
            false,
            100,
        );
        let b = compute_profile_with(
            flowing,
            VoiceScope::Character("B".into()),
            &ProseLanguage::En,
            &lx,
            false,
            100,
        );
        assert!(a.p50 < b.p50, "clipped median {} !< flowing median {}", a.p50, b.p50);
    }

    #[test]
    fn confidence_tracks_corpus_size() {
        assert_eq!(Confidence::from_counts(2, 10), Confidence::Low);
        assert_eq!(Confidence::from_counts(10, 120), Confidence::Medium);
        assert_eq!(Confidence::from_counts(40, 800), Confidence::High);
        // Enough utterances but too few words → still not High.
        assert_eq!(Confidence::from_counts(40, 100), Confidence::Medium);
    }
}
