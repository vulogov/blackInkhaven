//! LING-1 L-P2 — deterministic quantitative metrics over a language's lexicon
//! and phonology. Complements `analysis::profile` (descriptive counts) with the
//! information-theoretic + prosodic measures: phoneme-distribution entropy, the
//! Zipf fit of that distribution, phonotactic saturation, and mora weight.
//!
//! Pure and corpus-free — everything is computed from the dictionary forms that
//! parse cleanly as the language's own phonemes (the same filter `profile`
//! uses). Lexical measures that genuinely need running text (lexical TTR, word
//! Zipf) wait for a corpus source in a later wave.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::conlang::phonology::syllable::syllabify;
use crate::conlang::types::Phonology;
use crate::language_entry::DictionaryEntry;

/// Quantitative metrics for one language.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct LanguageMetrics {
    /// Dictionary forms that parsed entirely as the language's phonemes.
    pub analyzable_words: usize,
    pub total_segments: usize,

    // ── Information theory over the phoneme distribution ──
    /// Shannon entropy of the phoneme distribution, in bits.
    pub phoneme_entropy: f64,
    /// Maximum entropy for the number of distinct phonemes used: `log2(k)`.
    pub phoneme_entropy_max: f64,
    /// Evenness `H / Hmax` (0..1): 1 = every phoneme equally frequent.
    pub phoneme_evenness: f64,
    /// Perplexity `2^H`: the effective phoneme-inventory size.
    pub phoneme_perplexity: f64,

    // ── Zipf fit of the phoneme rank-frequency ──
    /// Slope of `ln(freq)` on `ln(rank)`; a Zipfian distribution sits near −1.
    pub zipf_slope: f64,
    /// Coefficient of determination (0..1) of that fit.
    pub zipf_r2: f64,

    // ── Phonotactic saturation ──
    /// Distinct attested syllable types across the lexicon.
    pub attested_syllables: usize,
    /// Combinatorial syllable space spanned by the attested slot-fillers
    /// (onsets × nuclei × codas, each including the empty slot where attested).
    pub possible_syllables: usize,
    /// `attested / possible` (0..1): how densely the lexicon fills its own grid.
    pub syllable_saturation: f64,

    // ── Prosody ──
    /// Mean moras per word (light syllable = 1, heavy = 2).
    pub mean_moras: f64,
    /// Fraction of syllables that are heavy (closed, or a long/complex nucleus).
    pub heavy_ratio: f64,
}

/// Compute the metrics for `entries` under `phon`. Returns all-zero when nothing
/// analyzable is present (empty lexicon, or no word parses cleanly).
pub fn metrics(phon: &Phonology, entries: &[DictionaryEntry]) -> LanguageMetrics {
    let mut m = LanguageMetrics::default();

    let mut phoneme_count: BTreeMap<String, usize> = BTreeMap::new();
    let mut syllable_types: BTreeSet<String> = BTreeSet::new();
    let mut onsets: BTreeSet<String> = BTreeSet::new();
    let mut nuclei: BTreeSet<String> = BTreeSet::new();
    let mut codas: BTreeSet<String> = BTreeSet::new();
    let mut total_syllables = 0usize;
    let mut heavy_syllables = 0usize;
    let mut total_moras = 0usize;

    for e in entries {
        let word = e.word.to_lowercase();
        let seq = phon.segment(&word);
        if seq.is_empty() || !seq.iter().all(|s| phon.phoneme(s).is_some()) {
            continue; // skip loanwords / names / un-segmentable glosses
        }
        m.analyzable_words += 1;
        m.total_segments += seq.len();
        for s in &seq {
            *phoneme_count.entry(s.clone()).or_default() += 1;
        }
        for syl in syllabify(phon, &seq) {
            let onset = syl.onset.join("");
            let nucleus = syl.nucleus.join("");
            let coda = syl.coda.join("");
            syllable_types.insert(format!("{onset}.{nucleus}.{coda}"));
            onsets.insert(onset);
            nuclei.insert(nucleus.clone());
            codas.insert(coda.clone());

            total_syllables += 1;
            // Heavy = closed (has a coda) or a long/complex nucleus (>1 segment).
            let heavy = !syl.coda.is_empty() || syl.nucleus.len() > 1;
            if heavy {
                heavy_syllables += 1;
                total_moras += 2;
            } else {
                total_moras += 1;
            }
        }
    }

    if m.analyzable_words == 0 {
        return m;
    }

    // Entropy over the phoneme distribution.
    let total = m.total_segments as f64;
    let distinct = phoneme_count.len();
    let mut h = 0.0f64;
    for &c in phoneme_count.values() {
        let p = c as f64 / total;
        h -= p * p.log2();
    }
    m.phoneme_entropy = h;
    m.phoneme_entropy_max = if distinct > 1 { (distinct as f64).log2() } else { 0.0 };
    m.phoneme_evenness =
        if m.phoneme_entropy_max > 0.0 { h / m.phoneme_entropy_max } else { 0.0 };
    m.phoneme_perplexity = h.exp2();

    // Zipf fit: ln(freq) on ln(rank), freq sorted descending.
    let mut freqs: Vec<usize> = phoneme_count.values().copied().collect();
    freqs.sort_unstable_by(|a, b| b.cmp(a));
    let (slope, r2) = zipf_fit(&freqs);
    m.zipf_slope = slope;
    m.zipf_r2 = r2;

    // Phonotactic saturation.
    m.attested_syllables = syllable_types.len();
    m.possible_syllables =
        onsets.len().max(1) * nuclei.len().max(1) * codas.len().max(1);
    m.syllable_saturation = if m.possible_syllables > 0 {
        (m.attested_syllables as f64 / m.possible_syllables as f64).min(1.0)
    } else {
        0.0
    };

    // Prosody.
    m.mean_moras = total_moras as f64 / m.analyzable_words as f64;
    m.heavy_ratio =
        if total_syllables > 0 { heavy_syllables as f64 / total_syllables as f64 } else { 0.0 };

    m
}

/// Least-squares slope + R² of `ln(freq_i)` on `ln(rank_i)` (rank 1-based, freq
/// descending). Returns `(0, 0)` when there is too little spread to fit. Shared
/// with the corpus engine's word-frequency Zipf.
pub(crate) fn zipf_fit(freqs_desc: &[usize]) -> (f64, f64) {
    if freqs_desc.len() < 2 {
        return (0.0, 0.0);
    }
    let xs: Vec<f64> = (1..=freqs_desc.len()).map(|r| (r as f64).ln()).collect();
    let ys: Vec<f64> = freqs_desc.iter().map(|&f| (f.max(1) as f64).ln()).collect();
    let n = xs.len() as f64;
    let mx = xs.iter().sum::<f64>() / n;
    let my = ys.iter().sum::<f64>() / n;
    let mut sxx = 0.0;
    let mut syy = 0.0;
    let mut sxy = 0.0;
    for i in 0..xs.len() {
        let dx = xs[i] - mx;
        let dy = ys[i] - my;
        sxx += dx * dx;
        syy += dy * dy;
        sxy += dx * dy;
    }
    if sxx <= f64::EPSILON || syy <= f64::EPSILON {
        return (0.0, 0.0);
    }
    let slope = sxy / sxx;
    let r2 = (sxy * sxy) / (sxx * syy);
    (slope, r2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::types::Phonology;

    fn phon() -> Phonology {
        let body = r#"{
            phonemes: [
                { ipa: "p", kind: "consonant" }, { ipa: "t", kind: "consonant" },
                { ipa: "k", kind: "consonant" }, { ipa: "n", kind: "consonant" },
                { ipa: "a", kind: "vowel" }, { ipa: "i", kind: "vowel" },
                { ipa: "u", kind: "vowel" }
            ]
        }"#;
        Phonology::from_hjson(body).unwrap().unwrap()
    }

    fn entry(word: &str) -> DictionaryEntry {
        let raw = serde_json::json!({ "word": word, "pos": "noun", "translation": "x" });
        serde_json::from_value(raw).expect("entry")
    }

    #[test]
    fn empty_lexicon_is_all_zero() {
        let m = metrics(&phon(), &[]);
        assert_eq!(m, LanguageMetrics::default());
        assert_eq!(m.analyzable_words, 0);
    }

    #[test]
    fn entropy_evenness_and_saturation_are_in_range() {
        let words = ["pata", "kina", "tuku", "nana", "pika"];
        let entries: Vec<_> = words.iter().map(|w| entry(w)).collect();
        let m = metrics(&phon(), &entries);
        assert_eq!(m.analyzable_words, 5);
        assert!(m.phoneme_entropy > 0.0);
        assert!(m.phoneme_evenness > 0.0 && m.phoneme_evenness <= 1.0);
        assert!(m.phoneme_perplexity > 1.0);
        assert!(m.syllable_saturation > 0.0 && m.syllable_saturation <= 1.0);
        assert!(m.mean_moras >= 1.0);
        assert!(m.heavy_ratio >= 0.0 && m.heavy_ratio <= 1.0);
    }

    #[test]
    fn a_single_repeated_phoneme_has_zero_evenness() {
        // "nana" over an inventory dominated by one segment still spreads a bit;
        // a perfectly uniform two-phoneme word maximises evenness instead. Here we
        // check the degenerate guard: one distinct phoneme → evenness 0 (Hmax 0).
        let one = Phonology::from_hjson(
            r#"{ phonemes: [ { ipa: "a", kind: "vowel" } ] }"#,
        )
        .unwrap()
        .unwrap();
        let m = metrics(&one, &[entry("aaa")]);
        assert_eq!(m.phoneme_entropy_max, 0.0);
        assert_eq!(m.phoneme_evenness, 0.0);
    }

    #[test]
    fn closed_syllables_are_heavier() {
        // Monosyllables: open CV = 1 light syllable (1 mora); closed CVC = 1
        // heavy syllable (2 moras). Both angles must reflect the weight.
        let open = metrics(&phon(), &[entry("pa"), entry("ta")]);
        let closed = metrics(&phon(), &[entry("pat"), entry("tan")]);
        assert_eq!(open.heavy_ratio, 0.0);
        assert_eq!(closed.heavy_ratio, 1.0);
        assert!(
            closed.mean_moras > open.mean_moras,
            "closed {} vs open {}",
            closed.mean_moras,
            open.mean_moras
        );
    }
}
