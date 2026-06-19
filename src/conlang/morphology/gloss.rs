//! Auto-gloss (LANG-1 P3.2) — interlinear glossing of conlang text.
//!
//! The inverse of paradigm generation. Morphological parsing of surface forms
//! is hard (ambiguity + allophony obscuring boundaries), so instead of peeling
//! affixes we **generate forward**: for every lexicon entry that declares a
//! `paradigm`, run `paradigm::generate` (allophony already applied) and index
//! every resulting surface form → `(root, Leipzig gloss)`. Glossing a word is
//! then a lookup. Pure and deterministic.

use std::collections::HashMap;

use crate::conlang::morphology::paradigm;
use crate::conlang::types::morphology::Morphology;
use crate::conlang::types::Phonology;
use crate::language_entry::DictionaryEntry;

/// Surface-form → `(root headword, gloss)` index.
pub struct GlossIndex {
    map: HashMap<String, (String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlossItem {
    pub surface: String,
    /// `None` when the word wasn't recognised.
    pub root: Option<String>,
    pub gloss: Option<String>,
}

/// Build the reverse index from a dictionary. Every entry's bare form is
/// indexed; entries that declare a `paradigm` also index each inflected
/// surface form. First write wins on a collision (note: ambiguity isn't
/// resolved — that's a later refinement).
pub fn build_index(phon: &Phonology, morph: &Morphology, entries: &[DictionaryEntry]) -> GlossIndex {
    let mut map: HashMap<String, (String, String)> = HashMap::new();
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
        map.entry(word.to_lowercase())
            .or_insert((word.to_string(), base_gloss.clone()));

        if let Some(pname) = &e.paradigm {
            if let Some(tmpl) = morph.paradigm(pname) {
                for row in paradigm::generate(phon, morph, tmpl, word, &base_gloss) {
                    map.entry(row.form.to_lowercase())
                        .or_insert((word.to_string(), row.gloss));
                }
            }
        }
    }
    GlossIndex { map }
}

impl GlossIndex {
    pub fn gloss_word(&self, word: &str) -> GlossItem {
        match self.map.get(&word.to_lowercase()) {
            Some((root, gloss)) => GlossItem {
                surface: word.to_string(),
                root: Some(root.clone()),
                gloss: Some(gloss.clone()),
            },
            None => GlossItem { surface: word.to_string(), root: None, gloss: None },
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
    fn gloss_text_is_in_order_and_case_insensitive() {
        let idx = build_index(&phon(), &morph(), &[entry("kata", "stone", Some("noun"))]);
        let items = idx.gloss_text("Kata katai");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].gloss.as_deref(), Some("stone")); // capitalized → matched
        assert_eq!(items[1].gloss.as_deref(), Some("stone-PL"));
    }
}
