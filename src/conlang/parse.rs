//! LING-1 L-P5 (Wave 3) — the morphological parser.
//!
//! The existing gloss path *generates* forms and indexes them; it cannot analyse
//! a surface word it has not already produced. This parser works the other way:
//! given a surface form, it strips the language's known affixes and asks whether
//! what remains is a root in the lexicon — a genuine reverse of the forward
//! generator (RFC amendment A-1), not a lookup. It returns every analysis it
//! finds, simplest first.
//!
//! This is a first, deliberately conservative cut: concatenative prefixes and
//! suffixes, bottoming out at a dictionary root. Non-concatenative processes
//! (ablaut, reduplication) and scoring against a morphotactic grammar come with
//! the rest of the Wave-3 engine.

use std::collections::HashMap;

use serde::Serialize;

use crate::conlang::types::Phonology;
use crate::conlang::types::morphology::{AffixPosition, Morphology};
use crate::conlang::types::phoneme::PhonemeKind;
use crate::language_entry::DictionaryEntry;

/// One analysis of a surface word.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Parse {
    /// The root form (rendered in the language's graphemes).
    pub root: String,
    /// The root's dictionary gloss.
    pub gloss: String,
    /// The affixes stripped to reach the root, outermost first (by gloss/id).
    pub affixes: Vec<String>,
}

/// The parse report for one surface form.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct ParseReport {
    pub surface: String,
    pub segments: Vec<String>,
    /// Every analysis found, fewest affixes first.
    pub parses: Vec<Parse>,
}

const MAX_AFFIXES: usize = 6;

/// Parse `surface` against `morph`'s affixes and the `entries` lexicon.
pub fn parse(phon: &Phonology, morph: &Morphology, entries: &[DictionaryEntry], surface: &str) -> ParseReport {
    let segs = phon.segment(&surface.to_lowercase());
    let mut report = ParseReport { surface: surface.to_string(), segments: segs.clone(), ..Default::default() };
    if segs.is_empty() {
        return report;
    }

    // Roots: each dictionary word's segmentation → its gloss.
    let mut roots: HashMap<Vec<String>, String> = HashMap::new();
    for e in entries {
        let rs = phon.segment(&e.word.to_lowercase());
        if !rs.is_empty() {
            roots.entry(rs).or_insert_with(|| e.translation.clone());
        }
    }

    // Pre-segment the concatenative affixes.
    let affixes: Vec<(Vec<String>, String, bool)> = morph
        .morphemes
        .iter()
        .filter_map(|m| match m.position {
            Some(AffixPosition::Suffix) => Some((phon.segment(&m.form), affix_label(&m.gloss, &m.id), false)),
            Some(AffixPosition::Prefix) => Some((phon.segment(&m.form), affix_label(&m.gloss, &m.id), true)),
            _ => None,
        })
        .filter(|(f, _, _)| !f.is_empty())
        .collect();

    let mut applied: Vec<String> = Vec::new();
    strip(&segs, phon, &roots, &affixes, &mut applied, &mut report.parses);

    // Fewest affixes first, then by root for stability.
    report.parses.sort_by(|a, b| a.affixes.len().cmp(&b.affixes.len()).then_with(|| a.root.cmp(&b.root)));
    report.parses.dedup();
    report
}

#[allow(clippy::too_many_arguments)]
fn strip(
    segs: &[String],
    phon: &Phonology,
    roots: &HashMap<Vec<String>, String>,
    affixes: &[(Vec<String>, String, bool)],
    applied: &mut Vec<String>,
    out: &mut Vec<Parse>,
) {
    // A root here is a complete analysis (the bare stem is also allowed).
    if let Some(gloss) = roots.get(segs) {
        out.push(Parse { root: render(phon, segs), gloss: gloss.clone(), affixes: applied.clone() });
    }
    if applied.len() >= MAX_AFFIXES {
        return;
    }

    // Non-concatenative: full reduplication (X X → REDUP + X). The reduplicant
    // reduces length by half, so the recursion terminates.
    if segs.len() >= 2 && segs.len() % 2 == 0 {
        let half = segs.len() / 2;
        if segs[..half] == segs[half..] {
            applied.push("REDUP".to_string());
            strip(&segs[..half], phon, roots, affixes, applied, out);
            applied.pop();
        }
    }

    // Non-concatenative: partial (initial-syllable) reduplication (CV~ + X). The
    // reduplicant copies the base's first syllable and prefixes it, so the
    // surface's opening syllable is duplicated — `segs[..k] == segs[k..2k]`. Try
    // the CV reduplicant (onset + first vowel) and the CVC (adding a coda), and
    // require a longer base after it so this is genuinely *partial*, not the full
    // reduplication handled above. Stripping `k ≥ 1` segments terminates.
    if let Some(cv) = first_syllable_len(phon, segs) {
        let cvc = if segs.get(cv).map(|s| phon.kind_of(s) == Some(PhonemeKind::Consonant)).unwrap_or(false) {
            Some(cv + 1)
        } else {
            None
        };
        for k in [Some(cv), cvc].into_iter().flatten() {
            if k >= 1 && segs.len() > 2 * k && segs[..k] == segs[k..2 * k] {
                applied.push("REDUP~".to_string());
                strip(&segs[k..], phon, roots, affixes, applied, out);
                applied.pop();
            }
        }
    }

    for (form, label, is_prefix) in affixes {
        let stripped: Option<&[String]> = if *is_prefix {
            segs.strip_prefix(form.as_slice())
        } else {
            segs.strip_suffix(form.as_slice())
        };
        if let Some(rem) = stripped {
            if rem.len() < segs.len() && !rem.is_empty() {
                applied.push(label.clone());
                strip(rem, phon, roots, affixes, applied, out);
                applied.pop();
            }
        }
    }
}

/// The length (in segments) of the initial CV syllable — leading onset
/// consonants plus the first vowel — or `None` when there is no vowel. This is
/// the light-syllable reduplicant a partial reduplication copies.
fn first_syllable_len(phon: &Phonology, segs: &[String]) -> Option<usize> {
    segs.iter().position(|s| phon.kind_of(s) == Some(PhonemeKind::Vowel)).map(|v| v + 1)
}

fn affix_label(gloss: &str, id: &str) -> String {
    if gloss.trim().is_empty() { id.to_string() } else { gloss.to_string() }
}

fn render(phon: &Phonology, segs: &[String]) -> String {
    segs.iter().map(|ipa| phon.phoneme(ipa).map(|p| p.grapheme()).unwrap_or(ipa.as_str())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phon() -> Phonology {
        let body = r#"{
            phonemes: [
                { ipa: "k", kind: "consonant" }, { ipa: "t", kind: "consonant" },
                { ipa: "n", kind: "consonant" }, { ipa: "s", kind: "consonant" },
                { ipa: "a", kind: "vowel" }, { ipa: "i", kind: "vowel" }
            ]
        }"#;
        Phonology::from_hjson(body).unwrap().unwrap()
    }

    fn morph() -> Morphology {
        let body = r#"{
            morphemes: [
                { id: "pl",  gloss: "PL",  form: "i",  position: "suffix" }
                { id: "dat", gloss: "DAT", form: "s",  position: "suffix" }
                { id: "def", gloss: "DEF", form: "na", position: "prefix" }
            ]
        }"#;
        Morphology::from_hjson(body).unwrap().unwrap()
    }

    fn entry(w: &str, g: &str) -> DictionaryEntry {
        DictionaryEntry { word: w.into(), pos: "noun".into(), translation: g.into(), ..Default::default() }
    }

    #[test]
    fn parses_a_suffixed_form_to_its_root() {
        // "katai" = kata + PL.
        let lex = [entry("kata", "stone")];
        let r = parse(&phon(), &morph(), &lex, "katai");
        // The 1-affix analysis should be present.
        assert!(r.parses.iter().any(|p| p.root == "kata" && p.affixes == vec!["PL".to_string()]));
    }

    #[test]
    fn parses_a_stacked_affix_form() {
        // "nakatas" = DEF + kata + DAT.
        let lex = [entry("kata", "stone")];
        let r = parse(&phon(), &morph(), &lex, "nakatas");
        assert!(r.parses.iter().any(|p| {
            p.root == "kata" && p.affixes.contains(&"DEF".to_string()) && p.affixes.contains(&"DAT".to_string())
        }));
    }

    #[test]
    fn a_bare_root_parses_with_no_affixes() {
        let lex = [entry("kata", "stone")];
        let r = parse(&phon(), &morph(), &lex, "kata");
        assert!(r.parses.iter().any(|p| p.root == "kata" && p.affixes.is_empty()));
    }

    #[test]
    fn an_unanalyzable_form_yields_no_parses() {
        let lex = [entry("kata", "stone")];
        // "sini" strips to nothing in the lexicon.
        let r = parse(&phon(), &morph(), &lex, "sini");
        assert!(r.parses.is_empty());
    }

    #[test]
    fn full_reduplication_is_analysed() {
        // "katakata" = REDUP + kata.
        let lex = [entry("kata", "stone")];
        let r = parse(&phon(), &morph(), &lex, "katakata");
        assert!(r.parses.iter().any(|p| p.root == "kata" && p.affixes == vec!["REDUP".to_string()]));
    }

    #[test]
    fn reduplication_combines_with_affixes() {
        // "katakatai" = REDUP + kata + PL.
        let lex = [entry("kata", "stone")];
        let r = parse(&phon(), &morph(), &lex, "katakatai");
        assert!(r.parses.iter().any(|p| {
            p.root == "kata"
                && p.affixes.contains(&"REDUP".to_string())
                && p.affixes.contains(&"PL".to_string())
        }));
    }

    #[test]
    fn partial_reduplication_is_analysed() {
        // "kakata" = ka~ + kata (the base's initial CV, "ka", prefixed).
        let lex = [entry("kata", "stone")];
        let r = parse(&phon(), &morph(), &lex, "kakata");
        assert!(
            r.parses.iter().any(|p| p.root == "kata" && p.affixes == vec!["REDUP~".to_string()]),
            "parses: {:?}",
            r.parses
        );
    }

    #[test]
    fn partial_reduplication_combines_with_affixes() {
        // "kakatai" = ka~ + kata + PL.
        let lex = [entry("kata", "stone")];
        let r = parse(&phon(), &morph(), &lex, "kakatai");
        assert!(r.parses.iter().any(|p| {
            p.root == "kata"
                && p.affixes.contains(&"REDUP~".to_string())
                && p.affixes.contains(&"PL".to_string())
        }));
    }

    #[test]
    fn a_plain_suffixed_form_is_not_read_as_partial_reduplication() {
        // "katai" = kata + PL; its opening CV ("ka") is not duplicated, so no REDUP~.
        let lex = [entry("kata", "stone")];
        let r = parse(&phon(), &morph(), &lex, "katai");
        assert!(!r.parses.iter().any(|p| p.affixes.contains(&"REDUP~".to_string())), "parses: {:?}", r.parses);
    }
}
