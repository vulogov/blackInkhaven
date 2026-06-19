//! Sound-change application (LANG-1 P4.1).
//!
//! Apply an ordered diachronic rule chain to a proto-form to get the daughter
//! form, reusing the generic ordered-rewrite engine (sound changes *are*
//! ordered context rewrites over phonemes). Deriving a whole daughter lexicon
//! is the same applied to every proto entry. Pure and deterministic.

use crate::conlang::phonology::rewrite;
use crate::conlang::types::{AllophonyRule, Phonology};
use crate::language_entry::DictionaryEntry;

/// Apply the rule chain to a single written proto-form. Segments with `phon`
/// (the proto's inventory), runs the chain, and renders in `phon`'s
/// graphemes. (A distinct daughter romanization is a later refinement.)
pub fn derive_form(phon: &Phonology, rules: &[AllophonyRule], proto_form: &str) -> String {
    let seq = phon.segment(proto_form);
    let out = rewrite::apply_ordered(&seq, rules, &phon.classes);
    out.iter()
        .map(|ipa| phon.phoneme(ipa).map(|p| p.grapheme()).unwrap_or(ipa.as_str()))
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct DerivedEntry {
    pub proto_form: String,
    pub form: String,
    pub gloss: String,
    pub pos: String,
}

/// Derive a daughter lexicon: apply the chain to every proto entry, carrying
/// the gloss + POS forward.
pub fn derive_lexicon(
    phon: &Phonology,
    rules: &[AllophonyRule],
    entries: &[DictionaryEntry],
) -> Vec<DerivedEntry> {
    entries
        .iter()
        .filter(|e| !e.word.trim().is_empty())
        .map(|e| {
            let proto_form = e.word.trim().to_string();
            DerivedEntry {
                form: derive_form(phon, rules, &proto_form),
                proto_form,
                gloss: e.translation.trim().to_string(),
                pos: e.pos.trim().to_string(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phon() -> Phonology {
        let body = r#"{
            phonemes: [
                { ipa: "p", kind: "consonant" }, { ipa: "t", kind: "consonant" },
                { ipa: "k", kind: "consonant" }, { ipa: "f", kind: "consonant" },
                { ipa: "h", kind: "consonant" }, { ipa: "a", kind: "vowel" }, { ipa: "i", kind: "vowel" }
            ],
            classes: { V: ["a", "i"] }
        }"#;
        Phonology::from_hjson(body).unwrap().unwrap()
    }

    fn rules(specs: &[&str]) -> Vec<AllophonyRule> {
        specs
            .iter()
            .map(|r| serde_hjson::from_str(&format!("{{ rule: \"{r}\" }}")).unwrap())
            .collect()
    }

    #[test]
    fn applies_an_ordered_sound_change_chain() {
        // Final p → f; intervocalic k → h.
        let p = phon();
        let r = rules(&["p > f / _ #", "k > h / V _ V"]);
        assert_eq!(derive_form(&p, &r, "tap"), "taf"); // final p → f
        assert_eq!(derive_form(&p, &r, "aka"), "aha"); // intervocalic k → h
        assert_eq!(derive_form(&p, &r, "kata"), "kata"); // initial k unchanged
    }

    #[test]
    fn feeding_order_matters() {
        // p → t / _ #, then t → f / _ #  ⇒  final p ends up f (rule 1 feeds 2).
        let p = phon();
        let r = rules(&["p > t / _ #", "t > f / _ #"]);
        assert_eq!(derive_form(&p, &r, "tap"), "taf");
    }

    #[test]
    fn derive_lexicon_carries_gloss() {
        let p = phon();
        let r = rules(&["p > f / _ #"]);
        let e = DictionaryEntry { word: "tap".into(), pos: "noun".into(), translation: "water".into(), ..Default::default() };
        let d = derive_lexicon(&p, &r, std::slice::from_ref(&e));
        assert_eq!(d[0].proto_form, "tap");
        assert_eq!(d[0].form, "taf");
        assert_eq!(d[0].gloss, "water");
    }
}
