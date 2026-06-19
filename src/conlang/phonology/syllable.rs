//! Syllabification (LANG-1 P1.2).
//!
//! Breaks a flat phoneme sequence (by IPA) into syllables using sonority
//! peaks as nuclei and the Maximal Onset Principle to split medial clusters:
//! the onset of a following syllable greedily takes the longest sonority-
//! rising suffix of the intervocalic cluster, the rest falls to the
//! preceding coda. Pure and deterministic; the basis for the onset / coda /
//! sonority constraints and (later) stress placement + interlinear glossing.

use crate::conlang::phonology::ipa;
use crate::conlang::types::{PhonemeKind, Phonology};

/// One syllable: onset consonants, the vowel nucleus (one or more adjacent
/// vowels form a diphthong nucleus), and coda consonants. Phonemes are IPA
/// strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Syllable {
    pub onset: Vec<String>,
    pub nucleus: Vec<String>,
    pub coda: Vec<String>,
}

impl Syllable {
    fn empty() -> Self {
        Self { onset: Vec::new(), nucleus: Vec::new(), coda: Vec::new() }
    }
}

fn is_vowel(phon: &Phonology, ipa: &str) -> bool {
    match phon.phoneme(ipa).map(|p| p.kind) {
        Some(k) => k == PhonemeKind::Vowel,
        // Unknown symbol: fall back to the sonority table — a vowel-ranked
        // symbol counts as a nucleus even if it isn't in the inventory.
        None => ipa::sonority_of(phon, ipa) >= ipa::VOWEL,
    }
}

/// Syllabify a phoneme sequence. A run with no vowel at all becomes a single
/// nucleus-less syllable (all coda) so the function is total; well-formed
/// words always have at least one vowel.
pub fn syllabify(phon: &Phonology, seq: &[String]) -> Vec<Syllable> {
    // Nucleus spans: maximal runs of vowels.
    let mut nuclei: Vec<(usize, usize)> = Vec::new(); // [start, end)
    let mut i = 0;
    while i < seq.len() {
        if is_vowel(phon, &seq[i]) {
            let start = i;
            while i < seq.len() && is_vowel(phon, &seq[i]) {
                i += 1;
            }
            nuclei.push((start, i));
        } else {
            i += 1;
        }
    }

    if nuclei.is_empty() {
        let mut s = Syllable::empty();
        s.coda = seq.to_vec();
        return if seq.is_empty() { Vec::new() } else { vec![s] };
    }

    let mut sylls: Vec<Syllable> = Vec::with_capacity(nuclei.len());
    for (idx, &(ns, ne)) in nuclei.iter().enumerate() {
        let mut s = Syllable::empty();
        s.nucleus = seq[ns..ne].to_vec();

        // Onset: the consonants between the previous nucleus's end and this
        // nucleus, after the maximal-onset split has assigned some to the
        // previous coda.
        let prev_end = if idx == 0 { 0 } else { nuclei[idx - 1].1 };
        let cluster = &seq[prev_end..ns];
        // The leading cluster (no preceding nucleus) onsets entirely onto the
        // first syllable; a medial cluster splits coda|onset by max onset.
        let split = if idx == 0 { 0 } else { max_onset_split(phon, cluster) };
        s.onset = cluster[split..].to_vec();

        // The consonants before the split belong to the *previous*
        // syllable's coda.
        if idx > 0 {
            sylls[idx - 1].coda = cluster[..split].to_vec();
        }
        sylls.push(s);
    }

    // Trailing consonants after the last nucleus → coda of the last syllable.
    let last_end = nuclei.last().unwrap().1;
    if last_end < seq.len() {
        if let Some(last) = sylls.last_mut() {
            last.coda = seq[last_end..].to_vec();
        }
    }

    sylls
}

/// Given an intervocalic consonant cluster, return the index at which it
/// splits into `coda | onset`: everything from the returned index onward is
/// the following syllable's onset (the longest sonority-rising suffix,
/// always including at least the final consonant — Maximal Onset Principle);
/// everything before is the preceding syllable's coda.
fn max_onset_split(phon: &Phonology, cluster: &[String]) -> usize {
    let n = cluster.len();
    if n == 0 {
        return 0;
    }
    // The last consonant always onsets onto the nucleus; extend the onset
    // leftward while sonority keeps strictly rising toward it.
    let mut start = n - 1;
    while start > 0
        && ipa::sonority_of(phon, &cluster[start - 1]) < ipa::sonority_of(phon, &cluster[start])
    {
        start -= 1;
    }
    start
}

/// Compact `CV.CVC`-style rendering for inspection — onset/nucleus/coda
/// concatenated per syllable, syllables joined by `.`. Uses each phoneme's
/// grapheme (romanization when present).
pub fn render(phon: &Phonology, sylls: &[Syllable]) -> String {
    let g = |ipa: &String| {
        phon.phoneme(ipa).map(|p| p.grapheme().to_string()).unwrap_or_else(|| ipa.clone())
    };
    sylls
        .iter()
        .map(|s| {
            let mut out = String::new();
            for p in s.onset.iter().chain(&s.nucleus).chain(&s.coda) {
                out.push_str(&g(p));
            }
            out
        })
        .collect::<Vec<_>>()
        .join(".")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::types::{Phoneme, PhonemeKind};

    fn ph(ipa: &str, kind: PhonemeKind) -> Phoneme {
        Phoneme { ipa: ipa.into(), romanize: None, kind, sonority: None }
    }

    fn lang() -> Phonology {
        Phonology {
            phonemes: vec![
                ph("p", PhonemeKind::Consonant), ph("t", PhonemeKind::Consonant),
                ph("k", PhonemeKind::Consonant), ph("s", PhonemeKind::Consonant),
                ph("r", PhonemeKind::Consonant), ph("l", PhonemeKind::Consonant),
                ph("n", PhonemeKind::Consonant),
                ph("a", PhonemeKind::Vowel), ph("i", PhonemeKind::Vowel), ph("o", PhonemeKind::Vowel),
            ],
            ..Default::default()
        }
    }

    fn seq(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn simple_cv_cv() {
        let p = lang();
        let s = syllabify(&p, &seq(&["t", "a", "k", "o"]));
        assert_eq!(render(&p, &s), "ta.ko");
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].onset, seq(&["t"]));
        assert_eq!(s[1].onset, seq(&["k"]));
    }

    #[test]
    fn maximal_onset_keeps_rising_cluster_together() {
        // /a t r a/ : "tr" rises (stop<liquid) → both onset the 2nd syllable.
        let p = lang();
        let s = syllabify(&p, &seq(&["a", "t", "r", "a"]));
        assert_eq!(render(&p, &s), "a.tra");
        assert_eq!(s[0].coda, Vec::<String>::new());
        assert_eq!(s[1].onset, seq(&["t", "r"]));
    }

    #[test]
    fn falling_cluster_splits_across_the_boundary() {
        // /a r t a/ : "rt" falls (liquid>stop) → r codas syll 1, t onsets syll 2.
        let p = lang();
        let s = syllabify(&p, &seq(&["a", "r", "t", "a"]));
        assert_eq!(render(&p, &s), "ar.ta");
        assert_eq!(s[0].coda, seq(&["r"]));
        assert_eq!(s[1].onset, seq(&["t"]));
    }

    #[test]
    fn leading_and_trailing_consonants() {
        let p = lang();
        let s = syllabify(&p, &seq(&["s", "t", "a", "r", "k"]));
        // "st" leads → onset of syll 1; "rk" trails → coda of syll 1.
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].onset, seq(&["s", "t"]));
        assert_eq!(s[0].nucleus, seq(&["a"]));
        assert_eq!(s[0].coda, seq(&["r", "k"]));
    }

    #[test]
    fn diphthong_nucleus() {
        let p = lang();
        let s = syllabify(&p, &seq(&["t", "a", "i", "n"]));
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].nucleus, seq(&["a", "i"]));
        assert_eq!(s[0].coda, seq(&["n"]));
    }
}
