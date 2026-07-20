//! IGT-1 (Wave 4) — interlinear glossed text.
//!
//! Interlinear Glossed Text is the linguist's core artifact: the morpheme-
//! segmented sentence, a gloss under each morpheme, and a free translation,
//! aligned in columns so the reader can see how the sentence is built. [`build`]
//! auto-glosses — it reuses the forward auto-gloss index ([`gloss`]), which
//! carries the morpheme segmentation — and lays the result out as an aligned
//! Leipzig block. Persisting an IGT as a stored annotation is the CLI's job (a
//! `Texts` chapter in the language book); the model here round-trips through it.
//! A curated free translation and a TUI editing surface are the next slices of the
//! Annotation Workbench. Pure and deterministic.

use serde::{Deserialize, Serialize};

use crate::conlang::morphology::gloss;
use crate::conlang::morphology::paradigm::MorphSeg;
use crate::conlang::types::Phonology;
use crate::conlang::types::morphology::Morphology;
use crate::language_entry::DictionaryEntry;

/// One glossed word — a column of the interlinear.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IgtWord {
    pub surface: String,
    /// The recognised root headword, if the word was glossed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    /// The Leipzig gloss (`stone-PL`), or `None` when the word wasn't recognised.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gloss: Option<String>,
    /// Morpheme segmentation of the surface form (`kata-t`), aligned to the gloss.
    /// Empty when the word wasn't recognised.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub segments: Vec<MorphSeg>,
}

impl IgtWord {
    /// The morpheme-segmented surface (`kata-t`), or the plain surface when the
    /// word carries no segmentation.
    fn segmented(&self) -> String {
        if self.segments.is_empty() {
            self.surface.clone()
        } else {
            self.segments.iter().map(|s| s.surface.as_str()).collect::<Vec<_>>().join("-")
        }
    }
}

/// One interlinear-glossed sentence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Igt {
    /// The original sentence.
    pub text: String,
    /// The glossed words, in order.
    pub words: Vec<IgtWord>,
    /// An auto *literal* translation — the recognised words' lexical meanings in
    /// order (unrecognised words pass through). A scaffold the author refines.
    pub translation: String,
    /// How many words the gloss index recognised.
    pub recognised: usize,
}

/// The lexical meaning of a Leipzig gloss — the part before the first morpheme
/// boundary (`stone-PL` → `stone`). Grammatical tags are dropped for the literal
/// translation; a portmanteau root gloss with no boundary is kept whole.
fn lexical_meaning(gloss: &str) -> &str {
    gloss.split_once('-').map(|(base, _)| base).unwrap_or(gloss)
}

/// Build the interlinear gloss of `text` against the language.
pub fn build(phon: &Phonology, morph: &Morphology, entries: &[DictionaryEntry], text: &str) -> Igt {
    let index = gloss::build_index(phon, morph, entries);
    let mut words = Vec::new();
    let mut literal = Vec::new();
    let mut recognised = 0;
    for item in index.gloss_text(text) {
        match &item.gloss {
            Some(g) => {
                recognised += 1;
                literal.push(lexical_meaning(g).to_string());
            }
            // Unrecognised (a name, an undefined word) passes through untranslated.
            None => literal.push(item.surface.clone()),
        }
        words.push(IgtWord {
            surface: item.surface,
            root: item.root,
            gloss: item.gloss,
            segments: item.segments,
        });
    }
    Igt { text: text.to_string(), words, translation: literal.join(" "), recognised }
}

impl Igt {
    /// Render as an aligned Leipzig block: the morpheme-segmented sentence, the
    /// gloss, and the literal translation. Columns are padded so each word's
    /// segmentation and its gloss line up.
    pub fn render(&self) -> String {
        // Line 1 is the segmented surface (`kata-t`); the gloss cell shows the
        // surface itself when a word wasn't recognised, so names and undefined
        // words pass through visibly.
        let cells: Vec<(String, String)> = self
            .words
            .iter()
            .map(|w| (w.segmented(), w.gloss.clone().unwrap_or_else(|| w.surface.clone())))
            .collect();

        let mut surface_line = String::new();
        let mut gloss_line = String::new();
        for (surface, gloss) in &cells {
            let width = surface.chars().count().max(gloss.chars().count()) + 2;
            surface_line.push_str(&pad(surface, width));
            gloss_line.push_str(&pad(gloss, width));
        }

        format!(
            "{}\n{}\n'{}'",
            surface_line.trim_end(),
            gloss_line.trim_end(),
            self.translation
        )
    }
}

/// Left-align `s` in a field `width` columns wide (by character count).
fn pad(s: &str, width: usize) -> String {
    let n = s.chars().count();
    let mut out = s.to_string();
    out.extend(std::iter::repeat_n(' ', width.saturating_sub(n)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phon() -> Phonology {
        let body = r#"{
            phonemes: [
                { ipa: "k", kind: "consonant" }, { ipa: "t", kind: "consonant" },
                { ipa: "n", kind: "consonant" }, { ipa: "l", kind: "consonant" },
                { ipa: "a", kind: "vowel" }, { ipa: "i", kind: "vowel" }, { ipa: "o", kind: "vowel" }
            ]
        }"#;
        Phonology::from_hjson(body).unwrap().unwrap()
    }

    fn morph() -> Morphology {
        let body = r#"{
            morphemes: [ { id: "pl", gloss: "PL", form: "i", position: "suffix" } ]
            paradigms: [ { name: "noun", cells: [
                { features: {}, morphemes: [] }
                { features: {}, morphemes: ["pl"] }
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

    fn lex() -> Vec<DictionaryEntry> {
        vec![entry("kata", "stone", Some("noun")), entry("nilo", "friend", None)]
    }

    #[test]
    fn glosses_a_recognised_sentence() {
        let igt = build(&phon(), &morph(), &lex(), "katai nilo");
        assert_eq!(igt.words.len(), 2);
        assert_eq!(igt.words[0].gloss.as_deref(), Some("stone-PL"));
        assert_eq!(igt.words[1].gloss.as_deref(), Some("friend"));
        assert_eq!(igt.recognised, 2);
    }

    #[test]
    fn the_literal_translation_drops_grammatical_tags() {
        let igt = build(&phon(), &morph(), &lex(), "katai nilo");
        // "stone-PL" → "stone"; the literal line is the lexical meanings.
        assert_eq!(igt.translation, "stone friend");
    }

    #[test]
    fn an_unrecognised_word_passes_through() {
        let igt = build(&phon(), &morph(), &lex(), "xyz nilo");
        assert!(igt.words[0].gloss.is_none());
        assert_eq!(igt.recognised, 1);
        // It appears untranslated in the literal line.
        assert!(igt.translation.starts_with("xyz"));
    }

    #[test]
    fn segments_an_inflected_word() {
        let igt = build(&phon(), &morph(), &lex(), "katai");
        let w = &igt.words[0];
        assert_eq!(w.segments.len(), 2);
        assert_eq!(w.segments[0].surface, "kata");
        assert_eq!(w.segments[0].gloss, "stone");
        assert_eq!(w.segments[1].surface, "i");
        assert_eq!(w.segments[1].gloss, "PL");
    }

    #[test]
    fn the_segmented_line_shows_morpheme_boundaries() {
        let igt = build(&phon(), &morph(), &lex(), "katai nilo");
        let line1 = igt.render().lines().next().unwrap().to_string();
        // Line 1 is the morpheme-segmented surface, not the plain word.
        assert!(line1.contains("kata-i"), "line1: {line1}");
    }

    #[test]
    fn igt_round_trips_through_json() {
        // The stored form (a Texts-chapter paragraph) must reload identically —
        // including an *unrecognised* word, whose empty fields are omitted on
        // serialize and so must default on load.
        let igt = build(&phon(), &morph(), &lex(), "katai zzz");
        assert!(igt.words[1].gloss.is_none() && igt.words[1].segments.is_empty(), "zzz should be unrecognised");
        let json = serde_json::to_string(&igt).unwrap();
        let back: Igt = serde_json::from_str(&json).unwrap();
        assert_eq!(igt, back);
    }

    #[test]
    fn render_aligns_the_surface_and_gloss_columns() {
        let igt = build(&phon(), &morph(), &lex(), "katai nilo");
        let block = igt.render();
        let lines: Vec<&str> = block.lines().collect();
        assert_eq!(lines.len(), 3);
        // Column 2 ("nilo"/"friend") starts at the same offset on both lines.
        let s_col = lines[0].find("nilo").unwrap();
        let g_col = lines[1].find("friend").unwrap();
        assert_eq!(s_col, g_col, "columns misaligned:\n{block}");
        assert_eq!(lines[2], "'stone friend'");
    }
}
