//! Paradigm generation (LANG-1 P3.1).
//!
//! Realize a paradigm template against a root: per cell, assemble the
//! underlying form (prefixes + root + suffixes), run the phonology's
//! allophony rules across the affix boundaries (P1.3), and render the surface
//! form + a Leipzig-style gloss. Pure and deterministic.

use std::collections::BTreeMap;

use crate::conlang::phonology::{allophony_eval, rewrite, syllable};
use crate::conlang::types::morphology::{AffixPosition, MorphProcess, Morphology, ParadigmTemplate};
use crate::conlang::types::phoneme::PhonemeKind;
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
/// Unknown morpheme ids are skipped. Concatenative affixes (prefix / suffix /
/// infix / circumfix) and non-concatenative processes (ablaut / reduplication)
/// are all applied; allophony runs across every boundary.
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
            // The stem starts as the segmented root; stem-modifying morphemes
            // (ablaut, reduplication, infix, circumfix) reshape it in cell order,
            // and their glosses sit just after the root. Outer prefixes and
            // suffixes wrap the finished stem, ordered by `precedence`.
            let mut stem = phon.segment(&root.to_lowercase());
            let mut inner_gloss: Vec<String> = Vec::new();
            let mut prefixes = Vec::new();
            let mut suffixes = Vec::new();

            for mid in &cell.morphemes {
                let Some(m) = morph.morpheme(mid) else { continue };
                let gloss = (!m.gloss.is_empty()).then(|| m.gloss.clone());
                match m.process {
                    Some(MorphProcess::Ablaut) => {
                        stem = rewrite::apply_ordered(&stem, &m.rules, &phon.classes);
                        inner_gloss.extend(gloss);
                    }
                    Some(MorphProcess::Reduplication) => {
                        stem = reduplicate(phon, &stem, m.reduplicate.as_deref());
                        inner_gloss.extend(gloss);
                    }
                    None => match m.position {
                        Some(AffixPosition::Prefix) => prefixes.push(m),
                        Some(AffixPosition::Suffix) => suffixes.push(m),
                        Some(AffixPosition::Infix) => {
                            stem = apply_infix(phon, &stem, &m.form, m.anchor.as_deref());
                            inner_gloss.extend(gloss);
                        }
                        Some(AffixPosition::Circumfix) => {
                            stem = apply_circumfix(phon, &stem, &m.form);
                            inner_gloss.extend(gloss);
                        }
                        None => {} // no position and no process — nothing to do.
                    },
                }
            }

            // Order the outer affixes by closeness to the root. `0` ("any")
            // sorts outermost; the stable sort keeps declared order among equals.
            let key = |p: u8| if p == 0 { u32::MAX } else { p as u32 };
            suffixes.sort_by_key(|m| key(m.precedence));
            prefixes.sort_by_key(|m| std::cmp::Reverse(key(m.precedence)));

            let mut underlying: Vec<String> = Vec::new();
            for m in &prefixes {
                underlying.extend(phon.segment(&m.form));
            }
            underlying.extend(stem);
            for m in &suffixes {
                underlying.extend(phon.segment(&m.form));
            }

            let surface = allophony_eval::surface_form(phon, &underlying);
            let form = render(phon, &surface);

            let mut parts: Vec<String> =
                prefixes.iter().filter(|m| !m.gloss.is_empty()).map(|m| m.gloss.clone()).collect();
            parts.push(root_gloss.to_string());
            parts.extend(inner_gloss);
            parts.extend(suffixes.iter().filter(|m| !m.gloss.is_empty()).map(|m| m.gloss.clone()));

            ParadigmRow { features: cell.features.clone(), form, gloss: parts.join("-") }
        })
        .collect()
}

/// Inflect `root` through `template` and return the cell whose features match
/// every `wanted` feature (case-insensitively). `None` if none matches.
pub fn realize_features(
    phon: &Phonology,
    morph: &Morphology,
    template: &ParadigmTemplate,
    root: &str,
    root_gloss: &str,
    wanted: &BTreeMap<String, String>,
) -> Option<ParadigmRow> {
    generate(phon, morph, template, root, root_gloss).into_iter().find(|r| {
        wanted
            .iter()
            .all(|(k, v)| r.features.get(k).is_some_and(|rv| rv.eq_ignore_ascii_case(v)))
    })
}

/// Insert an infix's segments into the stem. The default anchor
/// `before_first_vowel` places it after the first consonant (the Tagalog
/// pattern, `sulat` → `s-um-ulat`); `after_first_vowel` places it just past it.
fn apply_infix(phon: &Phonology, stem: &[String], form: &str, anchor: Option<&str>) -> Vec<String> {
    let infix = phon.segment(form);
    let first_vowel = stem
        .iter()
        .position(|s| matches!(phon.kind_of(s), Some(PhonemeKind::Vowel)));
    let at = match (anchor.unwrap_or("before_first_vowel"), first_vowel) {
        ("after_first_vowel", Some(i)) => i + 1,
        (_, Some(i)) => i,           // before the first vowel
        (_, None) => stem.len(),     // all consonants — append
    };
    let at = at.min(stem.len());
    let mut out = stem[..at].to_vec();
    out.extend(infix);
    out.extend_from_slice(&stem[at..]);
    out
}

/// Wrap the stem in a circumfix. The `form` uses `_` to mark the stem position
/// (`ge_t` → `ge` + stem + `t`); with no `_` the whole form is treated as a
/// prefixing half.
fn apply_circumfix(phon: &Phonology, stem: &[String], form: &str) -> Vec<String> {
    let (pre, post) = form.split_once('_').unwrap_or((form, ""));
    let mut out = phon.segment(pre);
    out.extend_from_slice(stem);
    out.extend(phon.segment(post));
    out
}

/// Copy part (or all) of the stem. Modes: `full` (the whole stem doubled),
/// `initial_cv` (the first consonant + vowel prefixed), `initial_syllable`
/// (the first syllable prefixed), `final_syllable` (the last syllable suffixed).
fn reduplicate(phon: &Phonology, stem: &[String], mode: Option<&str>) -> Vec<String> {
    let copy_syllable = |idx_from_end: bool| -> Vec<String> {
        let sylls = syllable::syllabify(phon, stem);
        let syl = if idx_from_end { sylls.last() } else { sylls.first() };
        syl.map(|s| [s.onset.clone(), s.nucleus.clone(), s.coda.clone()].concat())
            .unwrap_or_default()
    };
    match mode.unwrap_or("full") {
        "initial_cv" => {
            // The onset + nucleus of the first syllable.
            let first_vowel = stem
                .iter()
                .position(|s| matches!(phon.kind_of(s), Some(PhonemeKind::Vowel)));
            let cv: Vec<String> = match first_vowel {
                Some(i) => stem[..=i].to_vec(),
                None => stem.to_vec(),
            };
            [cv, stem.to_vec()].concat()
        }
        "initial_syllable" => [copy_syllable(false), stem.to_vec()].concat(),
        "final_syllable" => [stem.to_vec(), copy_syllable(true)].concat(),
        // "full" (and any unknown mode): the whole stem, doubled.
        _ => [stem.to_vec(), stem.to_vec()].concat(),
    }
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
    fn infix_lands_after_the_first_consonant() {
        // Tagalog-style: "sulat" + infix "um" before the first vowel → "sumulat".
        let p = phon();
        let body = r#"{
            morphemes: [ { id: "ag", gloss: "AG", form: "um", position: "infix" } ]
            paradigms: [ { name: "v", cells: [ { features: {}, morphemes: ["ag"] } ] } ]
        }"#;
        let m = Morphology::from_hjson(body).unwrap().unwrap();
        let rows = generate(&p, &m, m.paradigm("v").unwrap(), "tanik", "write");
        // "tanik" → first vowel at index 1 → "t" + "um" + "anik" = "tumanik".
        assert_eq!(rows[0].form, "tumanik");
        assert_eq!(rows[0].gloss, "write-AG");
    }

    #[test]
    fn circumfix_wraps_the_stem() {
        // German-style ge-…-t: form "ka_t" → "ka" + stem + "t".
        let p = phon();
        let body = r#"{
            morphemes: [ { id: "pp", gloss: "PTCP", form: "ka_t", position: "circumfix" } ]
            paradigms: [ { name: "v", cells: [ { features: {}, morphemes: ["pp"] } ] } ]
        }"#;
        let m = Morphology::from_hjson(body).unwrap().unwrap();
        let rows = generate(&p, &m, m.paradigm("v").unwrap(), "tana", "do");
        assert_eq!(rows[0].form, "katanat");
        assert_eq!(rows[0].gloss, "do-PTCP");
    }

    #[test]
    fn ablaut_changes_the_stem_vowel() {
        // A vowel swap inside the stem (i → a), like sing → sang.
        let p = phon();
        let body = r#"{
            morphemes: [ { id: "pst", gloss: "PST", process: "ablaut", rules: [ { rule: "i > a" } ] } ]
            paradigms: [ { name: "v", cells: [ { features: {}, morphemes: ["pst"] } ] } ]
        }"#;
        let m = Morphology::from_hjson(body).unwrap().unwrap();
        let rows = generate(&p, &m, m.paradigm("v").unwrap(), "kit", "sing");
        assert_eq!(rows[0].form, "kat");
        assert_eq!(rows[0].gloss, "sing-PST");
    }

    #[test]
    fn reduplication_copies_the_stem() {
        let p = phon();
        let body = r#"{
            morphemes: [
                { id: "intens", gloss: "INTENS", process: "reduplication", reduplicate: "full" }
                { id: "plr", gloss: "PL", process: "reduplication", reduplicate: "initial_cv" }
            ]
            paradigms: [ { name: "n", cells: [
                { features: { kind: "full" }, morphemes: ["intens"] }
                { features: { kind: "cv" },   morphemes: ["plr"] }
            ] } ]
        }"#;
        let m = Morphology::from_hjson(body).unwrap().unwrap();
        let rows = generate(&p, &m, m.paradigm("n").unwrap(), "kata", "stone");
        assert_eq!(rows[0].form, "katakata"); // full doubling
        assert_eq!(rows[0].gloss, "stone-INTENS");
        assert_eq!(rows[1].form, "kakata");   // initial CV "ka" prefixed
        assert_eq!(rows[1].gloss, "stone-PL");
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
