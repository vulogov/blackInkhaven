//! Allophony evaluation (LANG-1 P1.3).
//!
//! Apply the language's ordered allophony rules to an *underlying* phoneme
//! sequence to derive the *surface* sequence. Rules apply in declaration
//! order, each in a single left-to-right pass over the output of the
//! previous one (standard feeding order, no rule re-applies to its own
//! output within one pass). Optional rules — variant pronunciations — are
//! skipped in this canonical derivation. Pure and deterministic.

use crate::conlang::phonology::rewrite;
use crate::conlang::types::Phonology;

/// Derive the surface form of an underlying phoneme sequence (IPA) by
/// applying every non-optional allophony rule in order. Thin wrapper over
/// the generic ordered-rewrite engine (shared with tone sandhi).
pub fn surface_form(phon: &Phonology, underlying: &[String]) -> Vec<String> {
    rewrite::apply_ordered(underlying, &phon.allophony, &phon.classes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::types::{AllophonyRule, Phoneme, PhonemeKind};

    fn ph(ipa: &str, kind: PhonemeKind) -> Phoneme {
        Phoneme { ipa: ipa.into(), romanize: None, kind, sonority: None }
    }

    fn base(rules: &[&str]) -> Phonology {
        let mut p = Phonology {
            phonemes: vec![
                ph("p", PhonemeKind::Consonant), ph("t", PhonemeKind::Consonant),
                ph("k", PhonemeKind::Consonant), ph("d", PhonemeKind::Consonant),
                ph("tʃ", PhonemeKind::Consonant), ph("x", PhonemeKind::Consonant),
                ph("n", PhonemeKind::Consonant), ph("ə", PhonemeKind::Vowel),
                ph("a", PhonemeKind::Vowel), ph("i", PhonemeKind::Vowel),
            ],
            ..Default::default()
        };
        p.classes = [
            ("C".to_string(), vec!["p", "t", "k", "d", "n"].into_iter().map(String::from).collect()),
            ("V".to_string(), vec!["a", "i", "ə"].into_iter().map(String::from).collect()),
        ]
        .into_iter()
        .collect();
        p.allophony = rules
            .iter()
            .map(|r| {
                serde_hjson::from_str::<AllophonyRule>(&format!("{{ rule: \"{r}\" }}")).unwrap()
            })
            .collect();
        p
    }

    fn seq(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn palatalization_before_i() {
        let p = base(&["k > tʃ / _ i"]);
        assert_eq!(surface_form(&p, &seq(&["k", "i"])), seq(&["tʃ", "i"]));
        assert_eq!(surface_form(&p, &seq(&["k", "a"])), seq(&["k", "a"])); // no context
    }

    #[test]
    fn final_devoicing_at_boundary() {
        let p = base(&["d > t / _ #"]);
        assert_eq!(surface_form(&p, &seq(&["a", "d"])), seq(&["a", "t"]));
        assert_eq!(surface_form(&p, &seq(&["d", "a"])), seq(&["d", "a"])); // not final
    }

    #[test]
    fn intervocalic_lenition_with_classes() {
        let p = base(&["k > x / V _ V"]);
        assert_eq!(surface_form(&p, &seq(&["a", "k", "a"])), seq(&["a", "x", "a"]));
        assert_eq!(surface_form(&p, &seq(&["k", "a"])), seq(&["k", "a"]));
    }

    #[test]
    fn epenthesis_between_consonants() {
        let p = base(&["∅ > ə / C _ C"]);
        assert_eq!(surface_form(&p, &seq(&["t", "n"])), seq(&["t", "ə", "n"]));
        assert_eq!(surface_form(&p, &seq(&["t", "a"])), seq(&["t", "a"]));
    }

    #[test]
    fn deletion_of_final_vowel() {
        let p = base(&["V > 0 / _ #"]);
        assert_eq!(surface_form(&p, &seq(&["t", "a", "k", "a"])), seq(&["t", "a", "k"]));
    }

    #[test]
    fn rules_apply_in_feeding_order() {
        // First palatalize k→tʃ before i, then (a separate rule) tʃ→x before a:
        // ordering means rule 1 can feed rule 2's input.
        let p = base(&["k > tʃ / _ i", "i > 0 / _ #"]);
        // /k i/ → palatalize → /tʃ i/ → final-i-deletion → /tʃ/
        assert_eq!(surface_form(&p, &seq(&["k", "i"])), seq(&["tʃ"]));
    }

    #[test]
    fn optional_rules_are_skipped_in_the_canonical_form() {
        let mut p = base(&["k > tʃ / _ i"]);
        p.allophony[0].optional = true;
        assert_eq!(surface_form(&p, &seq(&["k", "i"])), seq(&["k", "i"]));
    }
}
