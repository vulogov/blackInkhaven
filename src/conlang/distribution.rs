//! LING-1 Wave-2 — positional phoneme distribution.
//!
//! `metrics` counts *how often* each phoneme is used; this reports *where*. For
//! every phoneme it records whether it turns up as a syllable onset, nucleus or
//! coda, and at the word edges — and surfaces the *restricted* (defective)
//! distributions: the consonant that only ever appears in codas, the one that
//! never starts a word. Restricted distributions are a real and common feature
//! of natural phonologies (English /ŋ/ never starts a word; /h/ never ends a
//! syllable), and worth seeing in your own. Pure + deterministic.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::conlang::phonology::syllable::syllabify;
use crate::conlang::types::Phonology;
use crate::conlang::types::phoneme::PhonemeKind;
use crate::language_entry::DictionaryEntry;

/// Where one phoneme appears across the lexicon.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct PhonemeDistribution {
    pub ipa: String,
    pub total: usize,
    pub as_onset: usize,
    pub as_nucleus: usize,
    pub as_coda: usize,
    pub word_initial: usize,
    pub word_final: usize,
    /// The most salient positional restriction, if the phoneme has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restriction: Option<String>,
}

/// The positional-distribution report for a language.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct DistributionReport {
    pub analyzable_words: usize,
    /// Consonants attested as syllable onsets.
    pub onset_inventory: Vec<String>,
    /// Consonants attested as syllable codas.
    pub coda_inventory: Vec<String>,
    /// Consonants with a defective distribution (onset-only, coda-only, or barred
    /// from a word edge).
    pub restricted: Vec<PhonemeDistribution>,
    /// The full per-phoneme table (frequency-ordered).
    pub phonemes: Vec<PhonemeDistribution>,
}

/// Compute the positional distribution of `entries` under `phon`.
pub fn distribution(phon: &Phonology, entries: &[DictionaryEntry]) -> DistributionReport {
    let mut table: BTreeMap<String, PhonemeDistribution> = BTreeMap::new();
    let mut words = 0usize;

    for e in entries {
        let word = e.word.to_lowercase();
        let seq = phon.segment(&word);
        if seq.is_empty() || !seq.iter().all(|s| phon.phoneme(s).is_some()) {
            continue;
        }
        words += 1;
        let first = seq.first().cloned();
        let last = seq.last().cloned();
        for s in &seq {
            let d = table.entry(s.clone()).or_insert_with(|| PhonemeDistribution {
                ipa: s.clone(),
                ..Default::default()
            });
            d.total += 1;
            if first.as_deref() == Some(s.as_str()) {
                d.word_initial += 1;
            }
            if last.as_deref() == Some(s.as_str()) {
                d.word_final += 1;
            }
        }
        for syl in syllabify(phon, &seq) {
            for s in &syl.onset {
                table.entry(s.clone()).or_default().as_onset += 1;
            }
            for s in &syl.nucleus {
                table.entry(s.clone()).or_default().as_nucleus += 1;
            }
            for s in &syl.coda {
                table.entry(s.clone()).or_default().as_coda += 1;
            }
        }
    }

    let mut r = DistributionReport { analyzable_words: words, ..Default::default() };
    if words == 0 {
        return r;
    }

    // Fill ipa on any entries created via `or_default()` in the syllable pass.
    for (ipa, d) in table.iter_mut() {
        if d.ipa.is_empty() {
            d.ipa = ipa.clone();
        }
        d.restriction = restriction_of(phon, d);
    }

    let is_consonant =
        |ipa: &str| phon.phoneme(ipa).map(|p| p.kind == PhonemeKind::Consonant).unwrap_or(false);

    r.onset_inventory =
        table.values().filter(|d| d.as_onset > 0 && is_consonant(&d.ipa)).map(|d| d.ipa.clone()).collect();
    r.coda_inventory =
        table.values().filter(|d| d.as_coda > 0 && is_consonant(&d.ipa)).map(|d| d.ipa.clone()).collect();
    r.restricted =
        table.values().filter(|d| d.restriction.is_some()).cloned().collect();
    r.restricted.sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.ipa.cmp(&b.ipa)));

    let mut phonemes: Vec<PhonemeDistribution> = table.into_values().collect();
    phonemes.sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.ipa.cmp(&b.ipa)));
    r.phonemes = phonemes;

    r
}

/// The most salient restriction for a consonant (vowels are unrestricted here).
fn restriction_of(phon: &Phonology, d: &PhonemeDistribution) -> Option<String> {
    let is_consonant =
        phon.phoneme(&d.ipa).map(|p| p.kind == PhonemeKind::Consonant).unwrap_or(false);
    if !is_consonant || d.total < 2 {
        return None; // need a little evidence, and only judge consonants
    }
    // Onset/coda restriction is the strongest signal.
    if d.as_coda == 0 && d.as_onset > 0 {
        return Some("onset only".to_string());
    }
    if d.as_onset == 0 && d.as_coda > 0 {
        return Some("coda only".to_string());
    }
    // Otherwise a word-edge restriction.
    if d.word_initial == 0 {
        return Some("never word-initial".to_string());
    }
    if d.word_final == 0 {
        return Some("never word-final".to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phon() -> Phonology {
        let body = r#"{
            phonemes: [
                { ipa: "p", kind: "consonant" }, { ipa: "t", kind: "consonant" },
                { ipa: "k", kind: "consonant" }, { ipa: "n", kind: "consonant" },
                { ipa: "ŋ", kind: "consonant" }, { ipa: "h", kind: "consonant" },
                { ipa: "a", kind: "vowel" }, { ipa: "i", kind: "vowel" }
            ]
        }"#;
        Phonology::from_hjson(body).unwrap().unwrap()
    }

    fn entry(word: &str) -> DictionaryEntry {
        DictionaryEntry { word: word.into(), pos: "noun".into(), translation: "x".into(), ..Default::default() }
    }

    fn find<'a>(r: &'a DistributionReport, ipa: &str) -> &'a PhonemeDistribution {
        r.phonemes.iter().find(|d| d.ipa == ipa).unwrap()
    }

    #[test]
    fn counts_onset_coda_and_word_edges() {
        // "pat": p onset+initial, a nucleus, t coda+final.
        let r = distribution(&phon(), &[entry("pat"), entry("tap")]);
        assert_eq!(r.analyzable_words, 2);
        let p = find(&r, "p");
        assert!(p.as_onset > 0 && p.word_initial > 0);
        assert!(r.onset_inventory.contains(&"p".to_string()));
    }

    #[test]
    fn a_coda_only_consonant_is_flagged() {
        // /ŋ/ only ever in a coda; /h/ only ever in an onset (word-initial here).
        let r = distribution(
            &phon(),
            &[entry("taŋ"), entry("kaŋ"), entry("haka"), entry("hata")],
        );
        assert_eq!(find(&r, "ŋ").restriction.as_deref(), Some("coda only"));
        assert_eq!(find(&r, "h").restriction.as_deref(), Some("onset only"));
        assert!(r.restricted.iter().any(|d| d.ipa == "ŋ"));
    }

    #[test]
    fn an_unrestricted_consonant_has_no_flag() {
        // /t/ appears as onset and coda, and at both edges.
        let r = distribution(&phon(), &[entry("tat"), entry("atta"), entry("tita")]);
        assert_eq!(find(&r, "t").restriction, None);
    }
}
