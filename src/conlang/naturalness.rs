//! LING-1 Wave-2 — phoneme-inventory naturalness.
//!
//! Judges how a language's phoneme inventory sits against cross-linguistic
//! tendencies, using the distinctive-feature matrix (`conlang::features`):
//! voicing-pair symmetry, place-series coverage, presence of the near-universal
//! segments, and inventory size. A gap is a *flag*, not an error — plenty of
//! natural languages have /k/ but no /ɡ/, or lack /p/ — the point is to show the
//! conlanger where their inventory is typologically ordinary and where it is
//! marked. Pure + deterministic.

use serde::Serialize;

use crate::conlang::features::{self, Feat};
use crate::conlang::types::Phonology;

/// Near-universal segments (PHOIBLE high-frequency): their absence is notable.
const NEAR_UNIVERSAL: &[&str] = &["m", "n", "p", "t", "k", "s", "l", "j", "w", "i", "a", "u"];

/// Inventory-size band.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SizeClass {
    Small,
    Typical,
    Large,
}

/// The naturalness report for one inventory.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct NaturalnessReport {
    pub phoneme_count: usize,
    pub consonants: usize,
    pub vowels: usize,
    /// Inventory segments not found in the feature matrix (exotic or mis-typed);
    /// excluded from the checks below.
    pub unknown_segments: Vec<String>,

    /// `(voiceless, voiced)` obstruent pairs both present.
    pub voicing_pairs: Vec<(String, String)>,
    /// Obstruents with no voicing counterpart in the inventory.
    pub voicing_gaps: Vec<String>,

    /// Which of labial / coronal / dorsal are represented among obstruents.
    pub places_present: Vec<String>,

    /// Near-universal segments present, and those missing.
    pub common_present: usize,
    pub missing_common: Vec<String>,

    pub size_class: SizeClass,
    /// Composite 0..1 naturalness score (see the module weights).
    pub score: f64,
}

impl Default for SizeClass {
    fn default() -> Self {
        SizeClass::Typical
    }
}

fn is(ipa: &str, feature: &str, want: Feat) -> bool {
    features::value(ipa, feature) == Some(want)
}

/// Obstruent = [−sonorant, −syllabic] and known to the matrix.
fn is_obstruent(ipa: &str) -> bool {
    is(ipa, "sonorant", Feat::Minus) && is(ipa, "syllabic", Feat::Minus)
}

/// Compute the naturalness report for `phon`.
pub fn naturalness(phon: &Phonology) -> NaturalnessReport {
    use crate::conlang::types::phoneme::PhonemeKind;

    let mut r = NaturalnessReport {
        phoneme_count: phon.phonemes.len(),
        consonants: phon.phonemes.iter().filter(|p| p.kind == PhonemeKind::Consonant).count(),
        vowels: phon.phonemes.iter().filter(|p| p.kind == PhonemeKind::Vowel).count(),
        ..Default::default()
    };

    let inventory: Vec<&str> = phon.phonemes.iter().map(|p| p.ipa.as_str()).collect();
    r.unknown_segments = inventory
        .iter()
        .filter(|ipa| !features::is_known(ipa))
        .map(|s| s.to_string())
        .collect();

    // Voicing symmetry over obstruents.
    let obstruents: Vec<&str> = inventory.iter().copied().filter(|ipa| is_obstruent(ipa)).collect();
    let voiceless: Vec<&str> =
        obstruents.iter().copied().filter(|ipa| is(ipa, "voice", Feat::Minus)).collect();
    let voiced: Vec<&str> =
        obstruents.iter().copied().filter(|ipa| is(ipa, "voice", Feat::Plus)).collect();
    let mut paired_voiced: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for &vl in &voiceless {
        // A voiced counterpart differs only in [voice].
        let mate = voiced.iter().copied().find(|&vd| {
            features::differing_features(vl, vd).map(|f| f == ["voice"]).unwrap_or(false)
        });
        match mate {
            Some(vd) => {
                r.voicing_pairs.push((vl.to_string(), vd.to_string()));
                paired_voiced.insert(vd);
            }
            None => r.voicing_gaps.push(vl.to_string()),
        }
    }
    // Voiced obstruents that never matched a voiceless one are gaps too.
    for &vd in &voiced {
        if !paired_voiced.contains(vd) {
            r.voicing_gaps.push(vd.to_string());
        }
    }

    // Place-series coverage among obstruents.
    for (feat, label) in [("labial", "labial"), ("coronal", "coronal"), ("dorsal", "dorsal")] {
        if obstruents.iter().any(|ipa| is(ipa, feat, Feat::Plus)) {
            r.places_present.push(label.to_string());
        }
    }

    // Near-universal coverage.
    for &seg in NEAR_UNIVERSAL {
        if inventory.contains(&seg) {
            r.common_present += 1;
        } else {
            r.missing_common.push(seg.to_string());
        }
    }

    // Size.
    let total = r.consonants + r.vowels;
    r.size_class = match total {
        0..=14 => SizeClass::Small,
        15..=45 => SizeClass::Typical,
        _ => SizeClass::Large,
    };

    r.score = score(&r, obstruents.len());
    r
}

/// Composite score. Near-universal coverage dominates (missing very common
/// segments is the strongest unnaturalness signal); place + voicing symmetry +
/// size plausibility round it out.
fn score(r: &NaturalnessReport, obstruent_count: usize) -> f64 {
    let common = r.common_present as f64 / NEAR_UNIVERSAL.len() as f64;
    let place = r.places_present.len() as f64 / 3.0;
    let voicing = if obstruent_count == 0 {
        1.0
    } else {
        1.0 - (r.voicing_gaps.len() as f64 / obstruent_count as f64)
    };
    let size = match r.size_class {
        SizeClass::Typical => 1.0,
        SizeClass::Small | SizeClass::Large => 0.7,
    };
    (0.45 * common + 0.2 * place + 0.15 * voicing + 0.2 * size).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phon(ipas: &[&str]) -> Phonology {
        let items: Vec<String> = ipas
            .iter()
            .map(|ipa| {
                // Vowels: our test set uses a/i/u/e/o.
                let kind = if "aeiouɛɔəæ".contains(*ipa) { "vowel" } else { "consonant" };
                format!("{{ ipa: \"{ipa}\", kind: \"{kind}\" }}")
            })
            .collect();
        let body = format!("{{ phonemes: [ {} ] }}", items.join(", "));
        Phonology::from_hjson(&body).unwrap().unwrap()
    }

    #[test]
    fn a_balanced_inventory_scores_high_and_symmetric() {
        // Every obstruent paired: p/b t/d k/ɡ s/z.
        let p =
            phon(&["p", "b", "t", "d", "k", "ɡ", "s", "z", "m", "n", "l", "j", "w", "i", "a", "u"]);
        let r = naturalness(&p);
        assert_eq!(r.voicing_gaps, Vec::<String>::new());
        assert_eq!(r.voicing_pairs.len(), 4); // p/b, t/d, k/ɡ, s/z
        assert_eq!(r.places_present.len(), 3); // labial, coronal, dorsal
        assert!(r.missing_common.is_empty());
        assert!(r.score > 0.9, "score was {}", r.score);
    }

    #[test]
    fn a_voicing_gap_is_flagged() {
        // /k/ but no /ɡ/ (and /s/ with no /z/).
        let p = phon(&["p", "b", "t", "d", "k", "s", "m", "n", "a", "i", "u"]);
        let r = naturalness(&p);
        assert!(r.voicing_gaps.contains(&"k".to_string()));
        assert!(r.voicing_gaps.contains(&"s".to_string()));
        assert!(r.voicing_pairs.contains(&("p".to_string(), "b".to_string())));
    }

    #[test]
    fn missing_near_universals_lower_the_score() {
        // No /m/, /a/, /i/ — very marked.
        let full = naturalness(&phon(&["p", "t", "k", "m", "n", "s", "a", "i", "u"]));
        let gapped = naturalness(&phon(&["p", "t", "k", "n", "s", "u", "e", "o"]));
        assert!(gapped.missing_common.contains(&"m".to_string()));
        assert!(gapped.missing_common.contains(&"a".to_string()));
        assert!(gapped.score < full.score);
    }

    #[test]
    fn exotic_segments_are_listed_not_crashed() {
        let r = naturalness(&phon(&["p", "t", "k", "ǃ", "a", "i"]));
        assert!(r.unknown_segments.contains(&"ǃ".to_string()));
        // Known segments still drive the checks.
        assert!(r.places_present.contains(&"labial".to_string()));
    }
}
