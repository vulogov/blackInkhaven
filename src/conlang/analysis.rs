//! Descriptive language analysis (LANG-1 P6.1).
//!
//! A deterministic profile of a constructed language: inventory balance,
//! phoneme frequency across the lexicon, syllable-length distribution, and
//! which onsets/codas actually get used. Distinct from the P2.1 lexicon
//! *audit* (which hunts for problems) — this is a descriptive snapshot that the
//! grammar-book / dictionary output (P6.2) draws on. Pure + deterministic.

use std::collections::BTreeMap;

use crate::conlang::phonology::syllable::syllabify;
use crate::conlang::types::phoneme::PhonemeKind;
use crate::conlang::Phonology;
use crate::language_entry::DictionaryEntry;

#[derive(Debug, Default, serde::Serialize)]
pub struct LanguageProfile {
    pub phoneme_inventory: usize,
    pub consonants: usize,
    pub vowels: usize,
    /// Lexicon entries seen.
    pub word_count: usize,
    /// Entries whose headword segments entirely into the inventory (the ones
    /// the phonological stats are computed over).
    pub analyzable_words: usize,
    pub total_segments: usize,
    pub avg_phonemes: f64,
    pub avg_syllables: f64,
    /// Syllable-count → number of words, ascending by count.
    pub syllable_hist: Vec<(usize, usize)>,
    /// Phoneme → occurrences in the lexicon, descending.
    pub phoneme_freq: Vec<(String, usize)>,
    /// Onset cluster (joined) → occurrences, descending.
    pub onset_freq: Vec<(String, usize)>,
    /// Coda cluster (joined) → occurrences, descending.
    pub coda_freq: Vec<(String, usize)>,
    /// Part-of-speech → entry count, descending.
    pub pos_freq: Vec<(String, usize)>,
}

/// Sort a count map into a `(key, count)` vec, descending by count then key.
fn ranked(map: BTreeMap<String, usize>) -> Vec<(String, usize)> {
    let mut v: Vec<(String, usize)> = map.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v
}

/// Build a descriptive profile of `phon` + `entries`.
pub fn profile(phon: &Phonology, entries: &[DictionaryEntry]) -> LanguageProfile {
    let mut p = LanguageProfile {
        phoneme_inventory: phon.phonemes.len(),
        consonants: phon.phonemes.iter().filter(|x| x.kind == PhonemeKind::Consonant).count(),
        vowels: phon.phonemes.iter().filter(|x| x.kind == PhonemeKind::Vowel).count(),
        word_count: entries.len(),
        ..Default::default()
    };

    let mut phoneme_count: BTreeMap<String, usize> = BTreeMap::new();
    let mut onset_count: BTreeMap<String, usize> = BTreeMap::new();
    let mut coda_count: BTreeMap<String, usize> = BTreeMap::new();
    let mut pos_count: BTreeMap<String, usize> = BTreeMap::new();
    let mut syll_hist: BTreeMap<usize, usize> = BTreeMap::new();
    let mut total_syllables = 0usize;

    for e in entries {
        if !e.pos.trim().is_empty() {
            *pos_count.entry(e.pos.trim().to_lowercase()).or_default() += 1;
        }
        let word = e.word.to_lowercase();
        let seq = phon.segment(&word);
        // Only count words that read entirely as the language's phonemes — skip
        // loanwords, proper names, or glosses that don't segment cleanly.
        if seq.is_empty() || !seq.iter().all(|s| phon.phoneme(s).is_some()) {
            continue;
        }
        p.analyzable_words += 1;
        p.total_segments += seq.len();
        for s in &seq {
            *phoneme_count.entry(s.clone()).or_default() += 1;
        }
        let sylls = syllabify(phon, &seq);
        *syll_hist.entry(sylls.len()).or_default() += 1;
        total_syllables += sylls.len();
        for syl in &sylls {
            if !syl.onset.is_empty() {
                *onset_count.entry(syl.onset.join("")).or_default() += 1;
            }
            if !syl.coda.is_empty() {
                *coda_count.entry(syl.coda.join("")).or_default() += 1;
            }
        }
    }

    if p.analyzable_words > 0 {
        p.avg_phonemes = p.total_segments as f64 / p.analyzable_words as f64;
        p.avg_syllables = total_syllables as f64 / p.analyzable_words as f64;
    }
    p.syllable_hist = syll_hist.into_iter().collect();
    p.phoneme_freq = ranked(phoneme_count);
    p.onset_freq = ranked(onset_count);
    p.coda_freq = ranked(coda_count);
    p.pos_freq = ranked(pos_count);
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::types::phoneme::Phoneme;

    fn phon() -> Phonology {
        // k, t (consonants), a, i (vowels)
        let mk = |ipa: &str, kind| Phoneme {
            ipa: ipa.to_string(),
            romanize: None,
            kind,
            sonority: None,
        };
        Phonology {
            phonemes: vec![
                mk("k", PhonemeKind::Consonant),
                mk("t", PhonemeKind::Consonant),
                mk("a", PhonemeKind::Vowel),
                mk("i", PhonemeKind::Vowel),
            ],
            ..Default::default()
        }
    }

    fn entry(word: &str, pos: &str) -> DictionaryEntry {
        DictionaryEntry {
            word: word.to_string(),
            pos: pos.to_string(),
            translation: "x".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn counts_inventory_and_frequency() {
        let p = phon();
        let entries = vec![entry("kata", "noun"), entry("ti", "verb"), entry("aki", "noun")];
        let prof = profile(&p, &entries);
        assert_eq!(prof.consonants, 2);
        assert_eq!(prof.vowels, 2);
        assert_eq!(prof.analyzable_words, 3);
        // a appears in kata(2) + aki(1) = 3 → the most frequent phoneme.
        assert_eq!(prof.phoneme_freq.first().unwrap(), &("a".to_string(), 3));
        // pos distribution
        assert_eq!(prof.pos_freq.first().unwrap(), &("noun".to_string(), 2));
        assert!(prof.avg_phonemes > 0.0);
    }

    #[test]
    fn skips_non_segmenting_words() {
        let p = phon();
        // "xyz" doesn't segment into the inventory.
        let prof = profile(&p, &[entry("xyz", "noun")]);
        assert_eq!(prof.word_count, 1);
        assert_eq!(prof.analyzable_words, 0);
        assert_eq!(prof.avg_phonemes, 0.0);
    }
}
