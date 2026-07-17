//! LING-1 Wave-2 — vowel-harmony detection.
//!
//! Vowel harmony is a co-occurrence restriction: within a word, the vowels must
//! agree on some feature — all front or all back (Turkish, Finnish), all rounded
//! or all unrounded, and so on. This scans the lexicon and, using the
//! distinctive-feature matrix, measures how consistently the vowels of each word
//! agree on backness and rounding. A high rate is strong evidence the language
//! has harmony on that dimension; a language with no harmony scatters around the
//! chance level (~50% for a binary feature over two-vowel words). Pure +
//! deterministic.

use serde::Serialize;

use crate::conlang::features::{self, Feat};
use crate::conlang::types::Phonology;
use crate::conlang::types::phoneme::PhonemeKind;
use crate::language_entry::DictionaryEntry;

/// The harmony dimensions we test, as `(feature name, display name)`.
const DIMENSIONS: &[(&str, &str)] = &[("back", "backness"), ("round", "rounding")];

/// Harmony evidence for one feature dimension.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct HarmonyDimension {
    /// The distinctive feature tested (`back`, `round`).
    pub feature: String,
    /// Human name (`backness`, `rounding`).
    pub name: String,
    /// Words with ≥2 vowels that carry a specified value for this feature.
    pub tested_words: usize,
    /// Of those, how many have all such vowels in agreement.
    pub harmonic_words: usize,
    /// `harmonic / tested` (0..1).
    pub rate: f64,
    /// `strong` (≥0.85) · `tendency` (≥0.65) · `none`.
    pub verdict: &'static str,
}

/// The full harmony report for a language.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct HarmonyReport {
    pub analyzable_words: usize,
    pub dimensions: Vec<HarmonyDimension>,
}

fn verdict(rate: f64, tested: usize) -> &'static str {
    if tested < 4 {
        "too few words"
    } else if rate >= 0.85 {
        "strong"
    } else if rate >= 0.65 {
        "tendency"
    } else {
        "none"
    }
}

/// Analyse `entries` under `phon` for vowel harmony.
pub fn analyze(phon: &Phonology, entries: &[DictionaryEntry]) -> HarmonyReport {
    // Collect each analyzable word's vowel sequence (its own phonemes only).
    let mut words: Vec<Vec<String>> = Vec::new();
    for e in entries {
        let word = e.word.to_lowercase();
        let seq = phon.segment(&word);
        if seq.is_empty() || !seq.iter().all(|s| phon.phoneme(s).is_some()) {
            continue;
        }
        let vowels: Vec<String> = seq
            .into_iter()
            .filter(|s| phon.phoneme(s).map(|p| p.kind == PhonemeKind::Vowel).unwrap_or(false))
            .collect();
        words.push(vowels);
    }

    let mut r = HarmonyReport { analyzable_words: words.len(), ..Default::default() };

    for (feat, name) in DIMENSIONS {
        let mut tested = 0usize;
        let mut harmonic = 0usize;
        for vowels in &words {
            // The specified values of this feature across the word's vowels.
            let vals: Vec<Feat> = vowels
                .iter()
                .filter_map(|v| features::value(v, feat))
                .filter(|f| *f != Feat::Unspec)
                .collect();
            if vals.len() < 2 {
                continue; // need at least two decisive vowels to judge agreement
            }
            tested += 1;
            if vals.iter().all(|f| *f == vals[0]) {
                harmonic += 1;
            }
        }
        let rate = if tested > 0 { harmonic as f64 / tested as f64 } else { 0.0 };
        r.dimensions.push(HarmonyDimension {
            feature: feat.to_string(),
            name: name.to_string(),
            tested_words: tested,
            harmonic_words: harmonic,
            rate,
            verdict: verdict(rate, tested),
        });
    }

    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phon() -> Phonology {
        // Front unrounded i e, front rounded y ø, back rounded u o, back unrounded ɯ.
        let body = r#"{
            phonemes: [
                { ipa: "t", kind: "consonant" }, { ipa: "k", kind: "consonant" },
                { ipa: "l", kind: "consonant" },
                { ipa: "i", kind: "vowel" }, { ipa: "e", kind: "vowel" },
                { ipa: "y", kind: "vowel" }, { ipa: "u", kind: "vowel" },
                { ipa: "o", kind: "vowel" }, { ipa: "ɯ", kind: "vowel" }
            ]
        }"#;
        Phonology::from_hjson(body).unwrap().unwrap()
    }

    fn entry(word: &str) -> DictionaryEntry {
        DictionaryEntry { word: word.into(), pos: "noun".into(), translation: "x".into(), ..Default::default() }
    }

    fn dim<'a>(r: &'a HarmonyReport, feat: &str) -> &'a HarmonyDimension {
        r.dimensions.iter().find(|d| d.feature == feat).unwrap()
    }

    #[test]
    fn a_backness_harmonic_language_reads_as_strong() {
        // Every word all-front or all-back: tili, keli (front), tuku, koɯ (back).
        let p = phon();
        let entries = [entry("tili"), entry("keli"), entry("tuku"), entry("koɯ"), entry("titi"), entry("kuku")];
        let r = analyze(&p, &entries);
        let back = dim(&r, "back");
        assert_eq!(back.harmonic_words, back.tested_words);
        assert_eq!(back.verdict, "strong");
    }

    #[test]
    fn a_language_with_no_harmony_reads_as_none() {
        // Deliberately mixed front/back within words.
        let p = phon();
        let entries = [entry("tiku"), entry("kuti"), entry("teku"), entry("kuli"), entry("tuli"), entry("kilu")];
        let r = analyze(&p, &entries);
        assert!(dim(&r, "back").rate < 0.65);
        assert_eq!(dim(&r, "back").verdict, "none");
    }

    #[test]
    fn words_without_two_decisive_vowels_are_not_tested() {
        let p = phon();
        // Single-vowel words give nothing to compare.
        let r = analyze(&p, &[entry("ti"), entry("ku"), entry("ka")]);
        assert_eq!(dim(&r, "back").tested_words, 0);
    }
}
