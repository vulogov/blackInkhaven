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
            // Split the cell's affixes by side, then order each by `precedence`
            // (closeness to the root). `0` ("any") sorts outermost, and the
            // stable sort preserves the declared order among equals — so a
            // paradigm that sets no precedence keeps its old order exactly.
            let key = |p: u8| if p == 0 { u32::MAX } else { p as u32 };
            let mut prefixes = Vec::new();
            let mut suffixes = Vec::new();
            for mid in &cell.morphemes {
                let Some(m) = morph.morpheme(mid) else { continue };
                match m.position {
                    AffixPosition::Prefix => prefixes.push(m),
                    AffixPosition::Suffix => suffixes.push(m),
                    // Infix / circumfix land in a later P3 increment.
                    _ => {}
                }
            }
            // Suffixes: root-adjacent (lowest key) first, read left-to-right.
            suffixes.sort_by_key(|m| key(m.precedence));
            // Prefixes: outermost (highest key) first, read left-to-right.
            prefixes.sort_by_key(|m| std::cmp::Reverse(key(m.precedence)));

            let prefix: String = prefixes.iter().map(|m| m.form.as_str()).collect();
            let suffix: String = suffixes.iter().map(|m| m.form.as_str()).collect();
            let pre_gloss: Vec<String> =
                prefixes.iter().filter(|m| !m.gloss.is_empty()).map(|m| m.gloss.clone()).collect();
            let suf_gloss: Vec<String> =
                suffixes.iter().filter(|m| !m.gloss.is_empty()).map(|m| m.gloss.clone()).collect();

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
    fn precedence_orders_stacked_suffixes() {
        // Two suffixes stacked; precedence puts the case suffix (1) next to the
        // root and the number suffix (2) outside it, regardless of cell order.
        let p = phon();
        let body = r#"{
            morphemes: [
                { id: "pl",  gloss: "PL",  form: "i", position: "suffix", precedence: 2 }
                { id: "dat", gloss: "DAT", form: "n", position: "suffix", precedence: 1 }
            ]
            paradigms: [ { name: "noun", cells: [
                { features: {}, morphemes: ["pl", "dat"] }
            ] } ]
        }"#;
        let m = Morphology::from_hjson(body).unwrap().unwrap();
        let rows = generate(&p, &m, m.paradigm("noun").unwrap(), "kata", "stone");
        // DAT (prec 1) hugs the root, PL (prec 2) sits outside it.
        assert_eq!(rows[0].form, "katani");
        assert_eq!(rows[0].gloss, "stone-DAT-PL");
    }

    #[test]
    fn no_precedence_keeps_declared_order() {
        // Backward compatibility: without precedence, declared cell order wins.
        let p = phon();
        let m = morph();
        let t = ParadigmTemplate {
            name: "x".into(),
            cells: vec![crate::conlang::types::morphology::ParadigmCell {
                features: BTreeMap::new(),
                morphemes: vec!["dat".into(), "pl".into()],
            }],
        };
        let rows = generate(&p, &m, &t, "kata", "stone");
        // declared dat, pl → "kata" + "d" + "i" = "katadi", gloss stone-DAT-PL.
        assert_eq!(rows[0].gloss, "stone-DAT-PL");
        assert_eq!(rows[0].form, "katadi");
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
