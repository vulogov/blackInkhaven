//! DIALOG-1 (D-P5) — per-character dialogue fingerprint builder. For each named
//! character, computes six deterministic voice properties (RFC §6.1) from all
//! of that character's `Certain`-attributed spans across the book, and upserts
//! them. `Inferred`/`None` spans are excluded — only certain attributions feed
//! the fingerprint, so heuristic error never pollutes a character profile.
//!
//! MATTR and the modal (hedging) word list are reused from NARR-1
//! (`crate::prose`). A full rebuild over `certain_spans` is cheap at literary
//! scale; the RFC's incremental `last_chapter_seen` optimisation is deferred
//! (the field is stored for a future DIALOG-2).

use std::collections::HashMap;

use anyhow::Result;

use crate::prose::{ProseLanguage, mattr, modal_unigrams};

use super::CharacterDialogueFingerprint;
use super::store::DialogueStore;

const MATTR_WINDOW: usize = 50;

/// Rebuild every character's fingerprint from the book's `Certain` spans.
pub(super) fn rebuild_fingerprints(
    store: &DialogueStore,
    book_slug: &str,
    lang: &ProseLanguage,
    now: &str,
) -> Result<()> {
    let spans = store.certain_spans(book_slug)?;
    let mut groups: HashMap<String, Vec<(u32, &super::DialogueSpan)>> = HashMap::new();
    for (ord, span) in &spans {
        if let Some(name) = &span.attribution_name {
            groups.entry(name.clone()).or_default().push((*ord, span));
        }
    }
    let modal: Vec<String> = modal_unigrams(lang).iter().map(|s| s.to_lowercase()).collect();
    for (name, group) in groups {
        let last_chapter = group.iter().map(|(o, _)| *o).max().unwrap_or(0);
        let fp = compute_fingerprint(name, &group, &modal);
        store.upsert_fingerprint(book_slug, &fp, last_chapter, now)?;
    }
    Ok(())
}

fn compute_fingerprint(
    name: String,
    group: &[(u32, &super::DialogueSpan)],
    modal: &[String],
) -> CharacterDialogueFingerprint {
    let count = group.len() as u32;
    let mut all_tokens: Vec<String> = Vec::new();
    let mut total_words = 0u32;
    let (mut q, mut ex) = (0u32, 0u32);
    for (_, span) in group {
        let toks = tokenize(&span.speech_text);
        total_words += toks.len() as u32;
        all_tokens.extend(toks);
        if span.ends_question {
            q += 1;
        }
        if span.ends_exclamation {
            ex += 1;
        }
    }
    let denom = count.max(1) as f32;
    let tok_refs: Vec<&str> = all_tokens.iter().map(String::as_str).collect();
    let hedge = if all_tokens.is_empty() {
        0.0
    } else {
        all_tokens.iter().filter(|t| modal.contains(t)).count() as f32 / all_tokens.len() as f32
    };
    CharacterDialogueFingerprint {
        character_name: name,
        utterance_count: count,
        mean_utterance_words: total_words as f32 / denom,
        utterance_mattr: mattr(&tok_refs, MATTR_WINDOW),
        question_ratio: q as f32 / denom,
        exclamation_ratio: ex as f32 / denom,
        hedge_density: hedge,
    }
}

/// Lowercase word tokens, surrounding punctuation trimmed.
fn tokenize(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| {
            w.to_lowercase()
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_string()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogue::{AttributionConfidence, DialogueSpan, SpanForm, TagVerbClass};

    fn span(speech: &str, q: bool, ex: bool) -> DialogueSpan {
        DialogueSpan {
            para_id: "p".into(),
            span_index: 0,
            form: SpanForm::QuotePair,
            char_start: 0,
            char_end: 0,
            word_count: speech.split_whitespace().count() as u32,
            speech_text: speech.into(),
            attribution_name: Some("Mara".into()),
            attribution_conf: AttributionConfidence::Certain,
            has_attribution_signal: true,
            tag_verb: Some("said".into()),
            tag_verb_class: Some(TagVerbClass::Neutral),
            ends_question: q,
            ends_exclamation: ex,
        }
    }

    #[test]
    fn fingerprint_metrics() {
        let dir = tempfile::tempdir().unwrap();
        let st = DialogueStore::open(dir.path()).unwrap();
        // 4 utterances: 1 question, 1 exclamation, one hedged ("maybe").
        for (i, s) in [
            span("Where are we going?", true, false),
            span("Get out now!", false, true),
            span("Maybe we should wait here", false, false),
            span("I am ready", false, false),
        ]
        .into_iter()
        .enumerate()
        {
            let mut s = s;
            s.span_index = i as u32;
            s.para_id = format!("p{i}");
            st.upsert_span("tale", 1, &s, "now", 1).unwrap();
        }
        rebuild_fingerprints(&st, "tale", &ProseLanguage::En, "now").unwrap();

        let fp = st.fingerprint("tale", "Mara").unwrap().unwrap();
        assert_eq!(fp.utterance_count, 4);
        assert!((fp.question_ratio - 0.25).abs() < 1e-3);
        assert!((fp.exclamation_ratio - 0.25).abs() < 1e-3);
        // "maybe" is an EN modal unigram → 1 hedge token across ~16 tokens.
        assert!(fp.hedge_density > 0.0, "hedge: {}", fp.hedge_density);
        assert!(fp.utterance_mattr > 0.0 && fp.utterance_mattr <= 1.0);
        assert!(fp.mean_utterance_words > 0.0);
    }

    #[test]
    fn inferred_spans_excluded_from_fingerprint() {
        let dir = tempfile::tempdir().unwrap();
        let st = DialogueStore::open(dir.path()).unwrap();
        let mut s = span("Only inferred", false, false);
        s.attribution_conf = AttributionConfidence::Inferred;
        st.upsert_span("tale", 1, &s, "now", 1).unwrap();
        rebuild_fingerprints(&st, "tale", &ProseLanguage::En, "now").unwrap();
        // No Certain spans → no fingerprint.
        assert!(st.fingerprint("tale", "Mara").unwrap().is_none());
    }
}
