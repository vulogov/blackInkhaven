//! LING-1 Wave-2 — minimal-pair analysis over a language's lexicon.
//!
//! Two words that are identical but for a single segment are a *minimal pair* —
//! the classic evidence that the two segments contrast (are separate phonemes).
//! This finds them across the dictionary and, via the distinctive-feature matrix
//! (`conlang::features`), reports the feature that distinguishes each — so a
//! conlanger sees the functional load of every contrast their language draws.
//!
//! Pure and deterministic. Pairs are found in O(words × length) by keying each
//! word on "its segments with one position blanked".

use std::collections::HashMap;

use serde::Serialize;

use crate::conlang::features;
use crate::conlang::types::Phonology;
use crate::language_entry::DictionaryEntry;

/// One minimal pair and the contrast between them.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MinimalPair {
    pub a: String,
    pub b: String,
    /// 0-based position of the contrasting segment.
    pub position: usize,
    pub seg_a: String,
    pub seg_b: String,
    /// The distinctive features that distinguish the two segments (empty when a
    /// segment is outside the feature matrix).
    pub features: Vec<String>,
}

/// The full minimal-pair report for a language.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct PairsReport {
    pub analyzable_words: usize,
    pub pair_count: usize,
    /// A bounded sample of pairs (single-feature contrasts first).
    pub pairs: Vec<MinimalPair>,
    /// `(feature, count)` — how many single-feature minimal pairs turn on each
    /// feature (the functional load), most-used first.
    pub contrast_load: Vec<(String, usize)>,
    /// Minimal pairs whose two segments differ in more than one feature (or fall
    /// outside the matrix) — a multi-feature contrast.
    pub complex_contrasts: usize,
}

/// Find minimal pairs across `entries` under `phon`. `sample` bounds the number
/// of pairs returned (the counts + load reflect all of them).
pub fn minimal_pairs(phon: &Phonology, entries: &[DictionaryEntry], sample: usize) -> PairsReport {
    let mut r = PairsReport::default();

    // Segment the analyzable words once.
    let mut words: Vec<(String, Vec<String>)> = Vec::new();
    for e in entries {
        let w = e.word.to_lowercase();
        let seq = phon.segment(&w);
        if seq.is_empty() || !seq.iter().all(|s| phon.phoneme(s).is_some()) {
            continue;
        }
        words.push((w, seq));
    }
    r.analyzable_words = words.len();

    // Key each word on "segments with position i blanked" → the group of words
    // sharing that key are all minimal pairs at position i.
    let mut buckets: HashMap<String, Vec<usize>> = HashMap::new();
    for (idx, (_, seq)) in words.iter().enumerate() {
        for i in 0..seq.len() {
            let mut key = String::new();
            key.push_str(&i.to_string());
            key.push('\u{1}');
            for (j, s) in seq.iter().enumerate() {
                if j == i {
                    key.push('\u{2}'); // blank
                } else {
                    key.push_str(s);
                }
                key.push('\u{3}');
            }
            buckets.entry(key).or_default().push(idx);
        }
    }

    let mut load: HashMap<&'static str, usize> = HashMap::new();
    let mut pairs: Vec<MinimalPair> = Vec::new();
    let mut seen: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();

    for (key, members) in &buckets {
        if members.len() < 2 {
            continue;
        }
        // Position `i` is the prefix of the key up to \u{1}.
        let position: usize = key.split('\u{1}').next().and_then(|n| n.parse().ok()).unwrap_or(0);
        for a in 0..members.len() {
            for b in (a + 1)..members.len() {
                let (ia, ib) = (members[a], members[b]);
                let seg_a = &words[ia].1[position];
                let seg_b = &words[ib].1[position];
                if seg_a == seg_b {
                    continue; // homophone at this slot
                }
                let dedup = if ia < ib { (ia, ib) } else { (ib, ia) };
                if !seen.insert((dedup.0, dedup.1 * 1000 + position)) {
                    continue;
                }
                r.pair_count += 1;
                let feats = features::differing_features(seg_a, seg_b).unwrap_or_default();
                if feats.len() == 1 {
                    *load.entry(feats[0]).or_default() += 1;
                } else {
                    r.complex_contrasts += 1;
                }
                pairs.push(MinimalPair {
                    a: words[ia].0.clone(),
                    b: words[ib].0.clone(),
                    position,
                    seg_a: seg_a.clone(),
                    seg_b: seg_b.clone(),
                    features: feats.iter().map(|s| s.to_string()).collect(),
                });
            }
        }
    }

    // Single-feature contrasts first, then by word for stability.
    pairs.sort_by(|x, y| {
        x.features.len().cmp(&y.features.len()).then_with(|| x.a.cmp(&y.a)).then_with(|| x.b.cmp(&y.b))
    });
    pairs.truncate(sample);
    r.pairs = pairs;

    let mut load: Vec<(String, usize)> = load.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    load.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    r.contrast_load = load;

    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phon() -> Phonology {
        let body = r#"{
            phonemes: [
                { ipa: "p", kind: "consonant" }, { ipa: "b", kind: "consonant" },
                { ipa: "t", kind: "consonant" }, { ipa: "k", kind: "consonant" },
                { ipa: "a", kind: "vowel" }, { ipa: "i", kind: "vowel" }
            ]
        }"#;
        Phonology::from_hjson(body).unwrap().unwrap()
    }

    fn entry(word: &str) -> DictionaryEntry {
        let raw = serde_json::json!({ "word": word, "pos": "noun", "translation": "x" });
        serde_json::from_value(raw).expect("entry")
    }

    #[test]
    fn finds_a_voicing_minimal_pair_and_its_contrast() {
        // pat / bat — a [voice] minimal pair at position 0.
        let r = minimal_pairs(&phon(), &[entry("pat"), entry("bat")], 10);
        assert_eq!(r.analyzable_words, 2);
        assert_eq!(r.pair_count, 1);
        let p = &r.pairs[0];
        assert_eq!((p.seg_a.as_str(), p.seg_b.as_str()), ("p", "b"));
        assert_eq!(p.features, vec!["voice"]);
        assert_eq!(r.contrast_load, vec![("voice".to_string(), 1)]);
    }

    #[test]
    fn counts_the_functional_load_across_positions() {
        // pat/bat (voice, pos 0), pat/pit (a/i vowel contrast, pos 1),
        // pat/kat (place, pos 0). No minimal pair between bat/pit etc.
        let r = minimal_pairs(
            &phon(),
            &[entry("pat"), entry("bat"), entry("pit"), entry("kat")],
            50,
        );
        // pat–bat, pat–kat (pos 0); pat–pit (pos 1); bat–? kat–? none more.
        assert!(r.pair_count >= 3);
        // The voice contrast is one single-feature pair (pat/bat).
        let voice = r.contrast_load.iter().find(|(f, _)| f == "voice").map(|(_, c)| *c);
        assert_eq!(voice, Some(1));
    }

    #[test]
    fn no_pairs_in_a_lexicon_with_no_minimal_pairs() {
        let r = minimal_pairs(&phon(), &[entry("pat"), entry("kiki")], 10);
        assert_eq!(r.pair_count, 0);
        assert!(r.pairs.is_empty());
    }
}
