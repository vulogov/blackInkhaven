//! Allophony evaluation (LANG-1 P1.3).
//!
//! Apply the language's ordered allophony rules to an *underlying* phoneme
//! sequence to derive the *surface* sequence. Rules apply in declaration
//! order, each in a single left-to-right pass over the output of the
//! previous one (standard feeding order, no rule re-applies to its own
//! output within one pass). Optional rules — variant pronunciations — are
//! skipped in this canonical derivation. Pure and deterministic.

use crate::conlang::types::{AllophonyRule, PatternAtom, Phonology};

/// Derive the surface form of an underlying phoneme sequence (IPA) by
/// applying every non-optional allophony rule in order.
pub fn surface_form(phon: &Phonology, underlying: &[String]) -> Vec<String> {
    let mut seq = underlying.to_vec();
    for rule in phon.allophony.iter().filter(|r| !r.optional) {
        seq = apply_rule(phon, rule, &seq);
    }
    seq
}

/// Apply one rule across `seq` in a single left-to-right pass.
fn apply_rule(phon: &Phonology, rule: &AllophonyRule, seq: &[String]) -> Vec<String> {
    let focus_len = if rule.lhs.is_none() { 0 } else { 1 };
    let mut out: Vec<String> = Vec::with_capacity(seq.len());
    let mut i = 0usize;
    // `i` walks 0..=len so insertion at the final boundary is reachable.
    while i <= seq.len() {
        let focus_fits = i + focus_len <= seq.len();
        let matched = focus_fits
            && focus_matches(phon, &rule.lhs, seq, i)
            && left_matches(phon, &rule.left, &seq[..i])
            && right_matches(phon, &rule.right, &seq[i + focus_len..]);

        if matched {
            if let Some(r) = &rule.rhs {
                out.push(r.clone());
            }
            if focus_len == 0 {
                // Insertion: emit the segment we sit before, then step past it
                // so the rule can't re-fire at the same gap.
                if i < seq.len() {
                    out.push(seq[i].clone());
                }
                i += 1;
            } else {
                i += focus_len;
            }
        } else {
            if i < seq.len() {
                out.push(seq[i].clone());
            }
            i += 1;
        }
    }
    out
}

fn focus_matches(phon: &Phonology, lhs: &Option<PatternAtom>, seq: &[String], i: usize) -> bool {
    match lhs {
        None => true, // ∅ — insertion site, always "matches" at a gap
        Some(atom) => i < seq.len() && atom_matches(phon, atom, &seq[i]),
    }
}

/// The left context must match the END of `left` (the segments immediately
/// before the target), reading the pattern's rightmost atom as adjacent.
fn left_matches(phon: &Phonology, atoms: &[PatternAtom], left: &[String]) -> bool {
    let mut li = left.len();
    for atom in atoms.iter().rev() {
        match atom {
            PatternAtom::Boundary => return li == 0,
            _ => {
                if li == 0 {
                    return false;
                }
                li -= 1;
                if !atom_matches(phon, atom, &left[li]) {
                    return false;
                }
            }
        }
    }
    true
}

/// The right context must match the START of `right` (the segments
/// immediately after the target).
fn right_matches(phon: &Phonology, atoms: &[PatternAtom], right: &[String]) -> bool {
    let mut ri = 0;
    for atom in atoms {
        match atom {
            PatternAtom::Boundary => return ri == right.len(),
            _ => {
                if ri >= right.len() || !atom_matches(phon, atom, &right[ri]) {
                    return false;
                }
                ri += 1;
            }
        }
    }
    true
}

/// A `Symbol` matches a class member when the symbol names a declared class,
/// otherwise it matches the literal phoneme.
fn atom_matches(phon: &Phonology, atom: &PatternAtom, seg: &str) -> bool {
    match atom {
        PatternAtom::Boundary => false,
        PatternAtom::Symbol(s) => {
            if phon.classes.contains_key(s) {
                phon.class_members(s).iter().any(|m| m == seg)
            } else {
                s == seg
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::types::{Phoneme, PhonemeKind};

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
