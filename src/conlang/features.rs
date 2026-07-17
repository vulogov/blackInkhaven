//! LING-1 Wave-2 — the distinctive-feature matrix.
//!
//! A curated, PHOIBLE-aligned IPA → distinctive-features lookup (see
//! `assets/linguistic/features.tsv`), parsed once and queried by segment. It is
//! a *separate* table keyed by IPA, not an extension of the `Phoneme` model
//! (RFC amendment A-5): the phonology stays sonority + V/C, and feature-based
//! analyses (feature distance, natural classes, minimal-pair contrasts) read
//! from here.
//!
//! Distance is the count of features specified in both segments whose values
//! differ — the standard measure of phonological similarity.

use std::collections::HashMap;
use std::sync::LazyLock;

/// The 16 features, in the column order of the data file.
pub const FEATURE_NAMES: &[&str] = &[
    "syllabic", "consonantal", "sonorant", "continuant", "voice", "nasal", "lateral", "labial",
    "coronal", "dorsal", "anterior", "strident", "high", "low", "back", "round",
];

const RAW: &str = include_str!("../../assets/linguistic/features.tsv");

/// A single feature value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feat {
    Plus,
    Minus,
    /// Unspecified / not applicable to this segment.
    Unspec,
}

impl Feat {
    fn parse(s: &str) -> Option<Feat> {
        match s {
            "+" => Some(Feat::Plus),
            "-" => Some(Feat::Minus),
            "0" => Some(Feat::Unspec),
            _ => None,
        }
    }
}

/// The feature vector of one segment (positional, matching [`FEATURE_NAMES`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureSet {
    pub vals: [Feat; 16],
}

static TABLE: LazyLock<HashMap<String, FeatureSet>> = LazyLock::new(|| parse(RAW));

fn parse(raw: &str) -> HashMap<String, FeatureSet> {
    let mut table = HashMap::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut cols = line.split_whitespace();
        let Some(ipa) = cols.next() else { continue };
        let mut vals = [Feat::Unspec; 16];
        let mut ok = true;
        for slot in vals.iter_mut() {
            match cols.next().and_then(Feat::parse) {
                Some(f) => *slot = f,
                None => {
                    ok = false;
                    break;
                }
            }
        }
        // Well-formed rows have exactly 16 values and no trailing columns.
        if ok && cols.next().is_none() {
            table.insert(ipa.to_string(), FeatureSet { vals });
        }
    }
    table
}

/// The feature vector for `ipa`, if the segment is in the matrix.
pub fn features(ipa: &str) -> Option<&'static FeatureSet> {
    TABLE.get(ipa)
}

// `is_known`, `len` and `distance` are the feature-matrix's public primitives —
// `differing_features` (below) drives the minimal-pair verb today; these back the
// forthcoming feature-based verbs (`/naturalness`, `/distribution`) and are
// exercised by the tests. Marked so the current single consumer doesn't warn.

/// Whether `ipa` is a known segment.
#[allow(dead_code)]
pub fn is_known(ipa: &str) -> bool {
    TABLE.contains_key(ipa)
}

/// The number of segments in the matrix.
#[allow(dead_code)]
pub fn len() -> usize {
    TABLE.len()
}

/// Feature distance between two segments: the count of features specified in
/// *both* whose values differ. `None` if either segment is unknown.
#[allow(dead_code)]
pub fn distance(a: &str, b: &str) -> Option<usize> {
    let (fa, fb) = (features(a)?, features(b)?);
    Some(count_differences(fa, fb).len())
}

/// The names of the features that distinguish `a` from `b` (specified in both,
/// opposite value). `None` if either segment is unknown.
pub fn differing_features(a: &str, b: &str) -> Option<Vec<&'static str>> {
    let (fa, fb) = (features(a)?, features(b)?);
    Some(count_differences(fa, fb))
}

fn count_differences(a: &FeatureSet, b: &FeatureSet) -> Vec<&'static str> {
    let mut out = Vec::new();
    for i in 0..16 {
        let (x, y) = (a.vals[i], b.vals[i]);
        let differs = matches!(
            (x, y),
            (Feat::Plus, Feat::Minus) | (Feat::Minus, Feat::Plus)
        );
        if differs {
            out.push(FEATURE_NAMES[i]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_matrix_loads_a_reasonable_inventory() {
        // ~85 curated segments; guard against a truncated/broken data file.
        assert!(len() > 70, "feature matrix only has {} segments", len());
        assert!(is_known("a"));
        assert!(is_known("t"));
        assert!(!is_known("Q")); // not IPA
    }

    #[test]
    fn voicing_pairs_differ_in_exactly_one_feature() {
        // p/b, t/d, k/g, s/z — a single [voice] contrast each.
        for (a, b) in [("p", "b"), ("t", "d"), ("k", "ɡ"), ("s", "z")] {
            assert_eq!(distance(a, b), Some(1), "{a}/{b} should differ by 1");
            assert_eq!(differing_features(a, b).unwrap(), vec!["voice"]);
        }
    }

    #[test]
    fn place_and_manner_contrasts_are_captured() {
        // p/t — same manner, different place. `anterior` is unspecified on the
        // labial, so the contrast is [labial] + [coronal] only.
        let pt = differing_features("p", "t").unwrap();
        assert!(pt.contains(&"labial") && pt.contains(&"coronal"));
        assert!(!pt.contains(&"anterior"), "anterior is unspecified on p, not a difference");
        // t/s — same place, stop vs fricative (continuant + strident).
        let ts = differing_features("t", "s").unwrap();
        assert!(ts.contains(&"continuant") && ts.contains(&"strident"));
    }

    #[test]
    fn distant_segments_and_unknowns() {
        // A high front vowel and a voiceless stop share almost nothing.
        assert!(distance("i", "p").unwrap() >= 5);
        // Unknown segment → None, not a panic.
        assert_eq!(distance("§", "a"), None);
        assert_eq!(differing_features("a", "§"), None);
    }
}
