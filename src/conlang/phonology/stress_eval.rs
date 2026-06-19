//! Stress evaluation (LANG-1 P1.4).
//!
//! Given a syllabified word and the language's stress rule, return the index
//! of the syllable that carries primary stress. Pure and deterministic;
//! short words clamp gracefully (a monosyllable always stresses itself).

use crate::conlang::phonology::syllable::Syllable;
use crate::conlang::types::stress::{StressPlacement, StressRule};

/// Index of the primary-stressed syllable, or `None` for an empty word.
pub fn primary_stress(rule: &StressRule, sylls: &[Syllable]) -> Option<usize> {
    let n = sylls.len();
    if n == 0 {
        return None;
    }
    let idx = match rule.primary {
        StressPlacement::Initial => 0,
        StressPlacement::Final => n - 1,
        StressPlacement::Penultimate => n.saturating_sub(2),
        StressPlacement::Antepenultimate => n.saturating_sub(3),
        StressPlacement::LatinRule => match n {
            1 => 0,
            2 => 0, // penult == first
            _ => {
                let penult = n - 2;
                if is_heavy(&sylls[penult]) {
                    penult
                } else {
                    n - 3
                }
            }
        },
    };
    Some(idx)
}

/// A syllable is *heavy* if it is closed (has a coda) or has a branching
/// nucleus (a long vowel / diphthong) — the standard weight criterion the
/// Latin rule turns on.
pub fn is_heavy(s: &Syllable) -> bool {
    !s.coda.is_empty() || s.nucleus.len() > 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn syl(onset: &[&str], nucleus: &[&str], coda: &[&str]) -> Syllable {
        let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect();
        Syllable { onset: v(onset), nucleus: v(nucleus), coda: v(coda) }
    }

    fn light(n: usize) -> Vec<Syllable> {
        (0..n).map(|_| syl(&["t"], &["a"], &[])).collect()
    }

    fn rule(p: StressPlacement) -> StressRule {
        StressRule { primary: p }
    }

    #[test]
    fn fixed_placements() {
        let w = light(4);
        assert_eq!(primary_stress(&rule(StressPlacement::Initial), &w), Some(0));
        assert_eq!(primary_stress(&rule(StressPlacement::Final), &w), Some(3));
        assert_eq!(primary_stress(&rule(StressPlacement::Penultimate), &w), Some(2));
        assert_eq!(primary_stress(&rule(StressPlacement::Antepenultimate), &w), Some(1));
    }

    #[test]
    fn short_words_clamp() {
        let one = light(1);
        assert_eq!(primary_stress(&rule(StressPlacement::Penultimate), &one), Some(0));
        assert_eq!(primary_stress(&rule(StressPlacement::Antepenultimate), &one), Some(0));
        assert_eq!(primary_stress(&rule(StressPlacement::Final), &one), Some(0));
        assert_eq!(primary_stress(&rule(StressPlacement::Initial), &[]), None);
    }

    #[test]
    fn latin_rule_follows_penult_weight() {
        // 3 syllables, penult LIGHT → antepenult.
        let mut w = light(3);
        assert_eq!(primary_stress(&rule(StressPlacement::LatinRule), &w), Some(0));
        // penult HEAVY (closed) → penult.
        w[1] = syl(&["t"], &["a"], &["n"]);
        assert_eq!(primary_stress(&rule(StressPlacement::LatinRule), &w), Some(1));
        // penult heavy via long nucleus → penult.
        w[1] = syl(&["t"], &["a", "a"], &[]);
        assert_eq!(primary_stress(&rule(StressPlacement::LatinRule), &w), Some(1));
    }

    #[test]
    fn weight_classification() {
        assert!(!is_heavy(&syl(&["t"], &["a"], &[]))); // open short
        assert!(is_heavy(&syl(&["t"], &["a"], &["n"]))); // closed
        assert!(is_heavy(&syl(&[], &["a", "i"], &[]))); // diphthong
    }
}
