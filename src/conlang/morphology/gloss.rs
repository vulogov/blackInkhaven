//! Auto-gloss (LANG-1 P3.2) — interlinear glossing of conlang text.
//!
//! The inverse of paradigm generation. Morphological parsing of surface forms
//! is hard (ambiguity + allophony obscuring boundaries), so instead of peeling
//! affixes we **generate forward**: for every lexicon entry that declares a
//! `paradigm`, run `paradigm::generate` (allophony already applied) and index
//! every resulting surface form → `(root, Leipzig gloss)`. Glossing a word is
//! then a lookup. Pure and deterministic.

use std::collections::HashMap;

use crate::conlang::morphology::paradigm::{self, MorphSeg};
use crate::conlang::types::morphology::Morphology;
use crate::conlang::types::Phonology;
use crate::language_entry::DictionaryEntry;

/// One indexed surface form: its root headword, gloss, and morpheme segmentation.
struct Indexed {
    root: String,
    gloss: String,
    segments: Vec<MorphSeg>,
}

/// Surface-form → [`Indexed`] index.
pub struct GlossIndex {
    map: HashMap<String, Indexed>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlossItem {
    pub surface: String,
    /// `None` when the word wasn't recognised.
    pub root: Option<String>,
    pub gloss: Option<String>,
    /// Morpheme segmentation (`kata-t` glossed `stone-DAT`); empty when the word
    /// wasn't recognised or couldn't be segmented.
    pub segments: Vec<MorphSeg>,
}

/// Build the reverse index from a dictionary. Every entry's bare form is
/// indexed; entries that declare a `paradigm` also index each inflected
/// surface form. First write wins on a collision (note: ambiguity isn't
/// resolved — that's a later refinement).
pub fn build_index(phon: &Phonology, morph: &Morphology, entries: &[DictionaryEntry]) -> GlossIndex {
    let mut map: HashMap<String, Indexed> = HashMap::new();
    for e in entries {
        let word = e.word.trim();
        if word.is_empty() {
            continue;
        }
        let base_gloss = if e.translation.trim().is_empty() {
            word.to_string()
        } else {
            e.translation.trim().to_string()
        };
        // The bare form is a single (unsegmented) morpheme.
        map.entry(word.to_lowercase()).or_insert_with(|| Indexed {
            root: word.to_string(),
            gloss: base_gloss.clone(),
            segments: vec![MorphSeg { surface: word.to_string(), gloss: base_gloss.clone() }],
        });

        if let Some(pname) = &e.paradigm {
            if let Some(tmpl) = morph.paradigm(pname) {
                for row in paradigm::generate(phon, morph, tmpl, word, &base_gloss) {
                    map.entry(row.form.to_lowercase()).or_insert_with(|| Indexed {
                        root: word.to_string(),
                        gloss: row.gloss,
                        segments: row.segments,
                    });
                }
            }
        }
    }
    GlossIndex { map }
}

impl GlossIndex {
    pub fn gloss_word(&self, word: &str) -> GlossItem {
        match self.map.get(&word.to_lowercase()) {
            Some(hit) => GlossItem {
                surface: word.to_string(),
                root: Some(hit.root.clone()),
                gloss: Some(hit.gloss.clone()),
                segments: hit.segments.clone(),
            },
            None => GlossItem { surface: word.to_string(), root: None, gloss: None, segments: Vec::new() },
        }
    }

    /// Gloss whitespace-separated words of `text`, in order.
    pub fn gloss_text(&self, text: &str) -> Vec<GlossItem> {
        text.split_whitespace().map(|w| self.gloss_word(w)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::types::{Phoneme, PhonemeKind};

    fn ph(ipa: &str, kind: PhonemeKind) -> Phoneme {
        Phoneme { ipa: ipa.into(), romanize: Some(ipa.into()), kind, sonority: None }
    }

    fn phon() -> Phonology {
        let body = r#"{
            phonemes: [
                { ipa: "k", kind: "consonant" }, { ipa: "t", kind: "consonant" },
                { ipa: "d", kind: "consonant" }, { ipa: "n", kind: "consonant" },
                { ipa: "l", kind: "consonant" }, { ipa: "a", kind: "vowel" }, { ipa: "i", kind: "vowel" },
                { ipa: "o", kind: "vowel" }
            ],
            allophony: [ { rule: "d > t / _ #" } ]
        }"#;
        let _ = ph; // keep helper referenced for parity with other tests
        Phonology::from_hjson(body).unwrap().unwrap()
    }

    fn morph() -> Morphology {
        let body = r#"{
            morphemes: [
                { id: "pl",  gloss: "PL",  form: "i", position: "suffix" }
                { id: "dat", gloss: "DAT", form: "d", position: "suffix" }
            ]
            paradigms: [ { name: "noun", cells: [
                { features: {}, morphemes: [] }
                { features: {}, morphemes: ["pl"] }
                { features: {}, morphemes: ["dat"] }
            ] } ]
        }"#;
        Morphology::from_hjson(body).unwrap().unwrap()
    }

    fn entry(word: &str, gloss: &str, paradigm: Option<&str>) -> DictionaryEntry {
        DictionaryEntry {
            word: word.into(),
            translation: gloss.into(),
            paradigm: paradigm.map(String::from),
            ..Default::default()
        }
    }

    #[test]
    fn glosses_inflected_forms_via_the_paradigm() {
        let idx = build_index(
            &phon(),
            &morph(),
            &[entry("kata", "stone", Some("noun")), entry("nilo", "friend", None)],
        );
        // bare + inflected (allophony-aware: kata+DAT → katat by devoicing).
        assert_eq!(idx.gloss_word("kata").gloss.as_deref(), Some("stone"));
        assert_eq!(idx.gloss_word("katai").gloss.as_deref(), Some("stone-PL"));
        assert_eq!(idx.gloss_word("katat").gloss.as_deref(), Some("stone-DAT"));
        // un-paradigm'd entry: only the bare form.
        assert_eq!(idx.gloss_word("nilo").gloss.as_deref(), Some("friend"));
        // unknown word.
        let unk = idx.gloss_word("xyz");
        assert!(unk.root.is_none() && unk.gloss.is_none());
    }

    #[test]
    fn segments_a_form_across_a_boundary_allophony() {
        let idx = build_index(&phon(), &morph(), &[entry("kata", "stone", Some("noun"))]);
        // "kata" + DAT ("d"), with d→t at the word edge → surface "katat".
        let item = idx.gloss_word("katat");
        assert_eq!(item.gloss.as_deref(), Some("stone-DAT"));
        // The boundary still aligns after allophony: kata-t, stone-DAT.
        assert_eq!(item.segments.len(), 2);
        assert_eq!(item.segments[0].surface, "kata");
        assert_eq!(item.segments[0].gloss, "stone");
        assert_eq!(item.segments[1].surface, "t");
        assert_eq!(item.segments[1].gloss, "DAT");
    }

    #[test]
    fn a_bare_form_is_a_single_segment() {
        let idx = build_index(&phon(), &morph(), &[entry("nilo", "friend", None)]);
        let item = idx.gloss_word("nilo");
        assert_eq!(item.segments.len(), 1);
        assert_eq!(item.segments[0].surface, "nilo");
        assert_eq!(item.segments[0].gloss, "friend");
    }

    #[test]
    fn gloss_text_is_in_order_and_case_insensitive() {
        let idx = build_index(&phon(), &morph(), &[entry("kata", "stone", Some("noun"))]);
        let items = idx.gloss_text("Kata katai");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].gloss.as_deref(), Some("stone")); // capitalized → matched
        assert_eq!(items[1].gloss.as_deref(), Some("stone-PL"));
    }
}
