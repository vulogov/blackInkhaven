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
                ph("e", PhonemeKind::Vowel), ph("s", PhonemeKind::Consonant),
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
        // What `Phonology::from_hjson` does: NFC + split joined tokens (LI-4).
        p.normalize();
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
    fn monophthongisation_ai_to_e() {
        // LI-4: a two-segment target, written with spaces or as one token.
        for rule in ["a i > e / _ #", "ai > e / _ #"] {
            let p = base(&[rule]);
            assert_eq!(p.allophony[0].lhs.len(), 2, "{rule}: target resolved to two atoms");
            assert_eq!(surface_form(&p, &seq(&["t", "a", "i"])), seq(&["t", "e"]), "{rule}");
            assert_eq!(surface_form(&p, &seq(&["t", "a", "i", "t"])), seq(&["t", "a", "i", "t"]), "{rule}: not final");
            assert_eq!(surface_form(&p, &seq(&["t", "i"])), seq(&["t", "i"]), "{rule}: no `a`");
            // The match consumes both segments: `a i a i` at the edge → `a i e`.
            assert_eq!(surface_form(&p, &seq(&["a", "i", "a", "i"])), seq(&["a", "i", "e"]), "{rule}");
        }
    }

    #[test]
    fn affrication_t_to_t_s_emits_two_segments() {
        for rule in ["t > t s / _ i", "t > ts / _ i"] {
            let p = base(&[rule]);
            assert_eq!(p.allophony[0].rhs, seq(&["t", "s"]), "{rule}: replacement resolved to two items");
            assert_eq!(surface_form(&p, &seq(&["t", "i"])), seq(&["t", "s", "i"]), "{rule}");
            assert_eq!(surface_form(&p, &seq(&["t", "a"])), seq(&["t", "a"]), "{rule}");
        }
        // A declared multi-character phoneme is one segment, not a sequence.
        let p = base(&["k > tʃ / _ i"]);
        assert_eq!(p.allophony[0].rhs, seq(&["tʃ"]));
        assert_eq!(surface_form(&p, &seq(&["k", "i"])), seq(&["tʃ", "i"]));
    }

    #[test]
    fn sequence_target_with_class_in_context() {
        // Monophthongise only after a consonant and before a vowel: C _ V.
        let p = base(&["a i > e / C _ V"]);
        assert_eq!(surface_form(&p, &seq(&["t", "a", "i", "a"])), seq(&["t", "e", "a"]));
        assert_eq!(surface_form(&p, &seq(&["a", "i", "a"])), seq(&["a", "i", "a"])); // no left C
        assert_eq!(surface_form(&p, &seq(&["t", "a", "i"])), seq(&["t", "a", "i"])); // no right V
        // A class may also stand inside the target: any vowel followed by `i`.
        let p = base(&["V i > e"]);
        assert_eq!(surface_form(&p, &seq(&["t", "ə", "i", "n"])), seq(&["t", "e", "n"]));
    }

    #[test]
    fn optional_rules_are_skipped_in_the_canonical_form() {
        let mut p = base(&["k > tʃ / _ i"]);
        p.allophony[0].optional = true;
        assert_eq!(surface_form(&p, &seq(&["k", "i"])), seq(&["k", "i"]));
    }
}
