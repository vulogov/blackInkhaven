//! LING-1 Wave-2 — phoneme suggestions.
//!
//! Where `naturalness` *detects* the gaps in an inventory, this *recommends the
//! fills*: the voiced counterpart missing beside a voiceless obstruent, the
//! near-universal segment the inventory lacks. Deterministic, feature-matrix
//! driven, and advisory — every suggestion is a "you could add this", never a
//! change to the language.

use serde::Serialize;

use crate::conlang::naturalness;
use crate::conlang::types::Phonology;

/// Common obstruent voicing pairs (voiceless, voiced).
const VOICING_PAIRS: &[(&str, &str)] = &[
    ("p", "b"),
    ("t", "d"),
    ("ʈ", "ɖ"),
    ("c", "ɟ"),
    ("k", "ɡ"),
    ("q", "ɢ"),
    ("f", "v"),
    ("θ", "ð"),
    ("s", "z"),
    ("ʃ", "ʒ"),
    ("ʂ", "ʐ"),
    ("x", "ɣ"),
    ("χ", "ʁ"),
    ("t͡s", "d͡z"),
    ("t͡ʃ", "d͡ʒ"),
];

/// One recommendation.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Suggestion {
    pub ipa: String,
    pub reason: String,
}

/// The suggestion report for a language.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct SuggestReport {
    pub phoneme_count: usize,
    pub suggestions: Vec<Suggestion>,
}

/// Recommend phonemes that would round out `phon`'s inventory.
pub fn suggest(phon: &Phonology) -> SuggestReport {
    let mut r = SuggestReport { phoneme_count: phon.phonemes.len(), ..Default::default() };
    let inv: std::collections::BTreeSet<&str> =
        phon.phonemes.iter().map(|p| p.ipa.as_str()).collect();
    let mut proposed: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    // Voicing counterparts.
    for (vl, vd) in VOICING_PAIRS {
        if inv.contains(vl) && !inv.contains(vd) && proposed.insert(vd.to_string()) {
            r.suggestions
                .push(Suggestion { ipa: vd.to_string(), reason: format!("voiced counterpart of /{vl}/") });
        }
        if inv.contains(vd) && !inv.contains(vl) && proposed.insert(vl.to_string()) {
            r.suggestions.push(Suggestion {
                ipa: vl.to_string(),
                reason: format!("voiceless counterpart of /{vd}/"),
            });
        }
    }

    // Missing near-universal segments (from the naturalness check).
    for seg in naturalness::naturalness(phon).missing_common {
        if !inv.contains(seg.as_str()) && proposed.insert(seg.clone()) {
            r.suggestions
                .push(Suggestion { ipa: seg, reason: "a near-universal segment most languages have".to_string() });
        }
    }

    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phon(ipas: &[&str]) -> Phonology {
        let items: Vec<String> = ipas
            .iter()
            .map(|ipa| {
                let kind = if "aeiou".contains(*ipa) { "vowel" } else { "consonant" };
                format!("{{ ipa: \"{ipa}\", kind: \"{kind}\" }}")
            })
            .collect();
        Phonology::from_hjson(&format!("{{ phonemes: [ {} ] }}", items.join(", "))).unwrap().unwrap()
    }

    fn has(r: &SuggestReport, ipa: &str) -> bool {
        r.suggestions.iter().any(|s| s.ipa == ipa)
    }

    #[test]
    fn suggests_the_missing_voiced_counterpart() {
        // /k/ without /ɡ/, /s/ without /z/.
        let r = suggest(&phon(&["p", "b", "t", "d", "k", "s", "m", "n", "a", "i", "u"]));
        assert!(has(&r, "ɡ"), "should suggest ɡ");
        assert!(has(&r, "z"), "should suggest z");
        // Present pairs are not suggested.
        assert!(!has(&r, "b") && !has(&r, "d"));
    }

    #[test]
    fn suggests_a_missing_near_universal() {
        // No /m/ — a near-universal.
        let r = suggest(&phon(&["p", "t", "k", "n", "s", "a", "i", "u"]));
        assert!(has(&r, "m"));
    }

    #[test]
    fn a_full_symmetric_inventory_needs_little() {
        let r = suggest(&phon(&[
            "p", "b", "t", "d", "k", "ɡ", "s", "z", "m", "n", "l", "j", "w", "a", "i", "u",
        ]));
        // No voicing gaps and all near-universals present.
        assert!(!has(&r, "ɡ") && !has(&r, "z"));
        assert!(r.suggestions.iter().all(|s| !s.reason.contains("near-universal")));
    }
}
