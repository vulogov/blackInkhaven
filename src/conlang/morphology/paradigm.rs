//! Paradigm generation (LANG-1 P3.1).
//!
//! Realize a paradigm template against a root: per cell, assemble the
//! underlying form (prefixes + root + suffixes), run the phonology's
//! allophony rules across the affix boundaries (P1.3), and render the surface
//! form + a Leipzig-style gloss. Pure and deterministic.

use std::collections::BTreeMap;

use crate::conlang::phonology::allophony_eval;
use crate::conlang::types::morphology::{AffixPosition, Morphology, ParadigmTemplate};
use crate::conlang::types::Phonology;

#[derive(Debug, Clone, PartialEq)]
pub struct ParadigmRow {
    pub features: BTreeMap<String, String>,
    /// Surface form (after allophony), rendered in the language's graphemes.
    pub form: String,
    /// Leipzig-style gloss, e.g. `PL-stone-DAT`.
    pub gloss: String,
}

/// Generate the full paradigm of `root` (gloss `root_gloss`) under `template`.
/// Unknown morpheme ids are skipped; infix / circumfix affixes are not yet
/// applied (P3.x) and are skipped with no effect.
pub fn generate(
    phon: &Phonology,
    morph: &Morphology,
    template: &ParadigmTemplate,
    root: &str,
    root_gloss: &str,
) -> Vec<ParadigmRow> {
    template
        .cells
        .iter()
        .map(|cell| {
            let mut prefix = String::new();
            let mut suffix = String::new();
            let mut pre_gloss: Vec<String> = Vec::new();
            let mut suf_gloss: Vec<String> = Vec::new();

            for mid in &cell.morphemes {
                let Some(m) = morph.morpheme(mid) else { continue };
                match m.position {
                    AffixPosition::Prefix => {
                        prefix.push_str(&m.form);
                        if !m.gloss.is_empty() {
                            pre_gloss.push(m.gloss.clone());
                        }
                    }
                    AffixPosition::Suffix => {
                        suffix.push_str(&m.form);
                        if !m.gloss.is_empty() {
                            suf_gloss.push(m.gloss.clone());
                        }
                    }
                    // Infix / circumfix land in a later P3 increment.
                    _ => {}
                }
            }

            let underlying = format!("{prefix}{root}{suffix}");
            let surface = allophony_eval::surface_form(phon, &phon.segment(&underlying));
            let form = render(phon, &surface);

            let mut parts = pre_gloss;
            parts.push(root_gloss.to_string());
            parts.extend(suf_gloss);

            ParadigmRow { features: cell.features.clone(), form, gloss: parts.join("-") }
        })
        .collect()
}

/// Render a phoneme sequence to graphemes (romanization when present).
fn render(phon: &Phonology, seq: &[String]) -> String {
    seq.iter()
        .map(|ipa| phon.phoneme(ipa).map(|p| p.grapheme()).unwrap_or(ipa.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::types::{Phoneme, PhonemeKind};

    fn ph(ipa: &str, kind: PhonemeKind) -> Phoneme {
        Phoneme { ipa: ipa.into(), romanize: Some(ipa.into()), kind, sonority: None }
    }

    /// Inventory + a final-devoicing allophony rule (d → t / _ #).
    fn phon() -> Phonology {
        let body = r#"{
            phonemes: [
                { ipa: "k", kind: "consonant" }, { ipa: "t", kind: "consonant" },
                { ipa: "d", kind: "consonant" }, { ipa: "n", kind: "consonant" },
                { ipa: "a", kind: "vowel" }, { ipa: "i", kind: "vowel" }
            ],
            allophony: [ { rule: "d > t / _ #" } ]
        }"#;
        Phonology::from_hjson(body).unwrap().unwrap()
    }

    fn morph() -> Morphology {
        let body = r#"{
            kind: "agglutinative"
            morphemes: [
                { id: "pl",  gloss: "PL",  form: "i",  position: "suffix" }
                { id: "dat", gloss: "DAT", form: "d",  position: "suffix" }
                { id: "def", gloss: "DEF", form: "na", position: "prefix" }
            ]
            paradigms: [ { name: "noun", cells: [
                { features: { number: "sg", case: "nom" }, morphemes: [] }
                { features: { number: "pl", case: "nom" }, morphemes: ["pl"] }
                { features: { number: "sg", case: "dat" }, morphemes: ["dat"] }
                { features: { number: "sg", case: "nom", def: "yes" }, morphemes: ["def"] }
            ] } ]
        }"#;
        Morphology::from_hjson(body).unwrap().unwrap()
    }

    #[test]
    fn generates_forms_and_glosses() {
        let p = phon();
        let m = morph();
        let t = m.paradigm("noun").unwrap();
        let rows = generate(&p, &m, t, "kata", "stone");
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].form, "kata"); // bare root
        assert_eq!(rows[0].gloss, "stone");
        assert_eq!(rows[1].form, "katai"); // + PL suffix
        assert_eq!(rows[1].gloss, "stone-PL");
        assert_eq!(rows[3].form, "nakata"); // DEF prefix
        assert_eq!(rows[3].gloss, "DEF-stone");
    }

    #[test]
    fn allophony_applies_across_the_affix_boundary() {
        // root "kata" + DAT "d" → "katad" → final devoicing → "katat".
        let p = phon();
        let m = morph();
        let t = m.paradigm("noun").unwrap();
        let rows = generate(&p, &m, t, "kata", "stone");
        let dat = rows.iter().find(|r| r.gloss == "stone-DAT").unwrap();
        assert_eq!(dat.form, "katat");
    }

    #[test]
    fn unknown_morpheme_id_is_skipped() {
        let p = phon();
        let m = morph();
        let t = ParadigmTemplate {
            name: "x".into(),
            cells: vec![crate::conlang::types::morphology::ParadigmCell {
                features: BTreeMap::new(),
                morphemes: vec!["nope".into()],
            }],
        };
        let rows = generate(&p, &m, &t, "kata", "stone");
        assert_eq!(rows[0].form, "kata");
    }
}
