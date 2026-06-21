//! LANG-2 P1 — rendering a form *in a variety*.
//!
//! A variety differs from the base language by (1) an ordered set of sound
//! changes and (2) a handful of suppletive word overrides. Rendering is the same
//! operation diachronics performs, applied synchronically: run the base surface
//! form through the variety's sound changes with the shared rewrite engine.

use crate::conlang::types::variety::Variety;
use crate::conlang::Phonology;

/// Render a base surface form in `variety` by applying its sound changes. Uses
/// the base language's phonology for segmentation + classes (the changes are
/// defined over the base inventory), exactly as `derive_form` does for a proto.
pub fn render_form(base_phon: &Phonology, variety: &Variety, base_form: &str) -> String {
    if variety.sound_changes.is_empty() {
        return base_form.to_string();
    }
    crate::conlang::diachronic::apply::derive_form(base_phon, &variety.sound_changes, base_form)
}

/// The variety form of a concept. A lexical override *is* the variety's word
/// (suppletive — the sound changes don't apply to it); otherwise the base form
/// is run through the sound changes. Returns `(form, overridden)`.
pub fn render_concept(
    base_phon: &Phonology,
    variety: &Variety,
    gloss: &str,
    base_form: &str,
) -> (String, bool) {
    if let Some(over) = variety.lexicon.get(gloss) {
        return (over.clone(), true);
    }
    (render_form(base_phon, variety, base_form), false)
}

/// Render a run of whitespace-separated base words in a variety, word by word.
pub fn render_text(base_phon: &Phonology, variety: &Variety, text: &str) -> String {
    text.split_whitespace()
        .map(|w| render_form(base_phon, variety, w))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::types::variety::Varieties;

    fn phon() -> Phonology {
        // Vowels a/i, consonants t/d/k/r so the lenition rule has segments.
        let body = r#"{ phonemes: [
            { ipa: "t", kind: "consonant" }, { ipa: "d", kind: "consonant" },
            { ipa: "k", kind: "consonant" }, { ipa: "r", kind: "consonant" },
            { ipa: "a", kind: "vowel" }, { ipa: "i", kind: "vowel" }
        ], classes: { V: ["a", "i"] } }"#;
        Phonology::from_hjson(body).unwrap().unwrap()
    }

    fn lowland() -> Varieties {
        let body = r#"{ varieties: [
            { id: "lowland", sound_changes: [ { rule: "t > d / V _ V" } ],
              lexicon: { "water": "moru" } }
        ] }"#;
        Varieties::from_hjson(body).unwrap().unwrap()
    }

    #[test]
    fn sound_change_applies_synchronically() {
        let v = lowland();
        let d = v.get("lowland").unwrap();
        // intervocalic t → d: "kata" → "kada", word-initial t untouched: "taka".
        assert_eq!(render_form(&phon(), d, "kata"), "kada");
        assert_eq!(render_form(&phon(), d, "taka"), "taka");
    }

    #[test]
    fn lexical_override_is_suppletive() {
        let v = lowland();
        let d = v.get("lowland").unwrap();
        // The override wins and is NOT run through the sound changes.
        let (form, overridden) = render_concept(&phon(), d, "water", "kata");
        assert_eq!(form, "moru");
        assert!(overridden);
        // A concept with no override falls through to the sound-changed base.
        let (form, overridden) = render_concept(&phon(), d, "stone", "kata");
        assert_eq!(form, "kada");
        assert!(!overridden);
    }

    #[test]
    fn render_text_goes_word_by_word() {
        let v = lowland();
        let d = v.get("lowland").unwrap();
        assert_eq!(render_text(&phon(), d, "kata taka"), "kada taka");
    }
}
