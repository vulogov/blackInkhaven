//! Deterministic phonotactic validation (LANG-1 P1.1).
//!
//! A single linear pass over a candidate phoneme sequence (by IPA) decides
//! legality against every declared constraint. Pure and total.

use crate::conlang::types::{PhonemeKind, Phonology, PhonotacticConstraint};

/// True iff `seq` (phonemes by IPA) satisfies every constraint.
pub fn is_legal(phon: &Phonology, seq: &[String]) -> bool {
    phon.constraints.iter().all(|c| satisfies(phon, seq, c))
}

fn satisfies(phon: &Phonology, seq: &[String], c: &PhonotacticConstraint) -> bool {
    match c {
        PhonotacticConstraint::MaxClusterSize(n) => max_consonant_run(phon, seq) <= *n,
        PhonotacticConstraint::NoGeminate => !seq.windows(2).any(|w| w[0] == w[1]),
        PhonotacticConstraint::ForbidBigram(a, b) => {
            !seq.windows(2).any(|w| &w[0] == a && &w[1] == b)
        }
    }
}

/// Length of the longest run of consecutive consonants. A phoneme whose
/// kind is unknown (not in the inventory) is treated as non-consonantal so
/// a stray symbol can't inflate the cluster count.
fn max_consonant_run(phon: &Phonology, seq: &[String]) -> usize {
    let mut max = 0;
    let mut run = 0;
    for ipa in seq {
        if matches!(phon.kind_of(ipa), Some(PhonemeKind::Consonant)) {
            run += 1;
            max = max.max(run);
        } else {
            run = 0;
        }
    }
    max
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::types::Phoneme;

    fn cons(ipa: &str) -> Phoneme {
        Phoneme { ipa: ipa.into(), romanize: None, kind: PhonemeKind::Consonant }
    }
    fn vow(ipa: &str) -> Phoneme {
        Phoneme { ipa: ipa.into(), romanize: None, kind: PhonemeKind::Vowel }
    }
    fn seq(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    fn base() -> Phonology {
        Phonology {
            phonemes: vec![cons("p"), cons("t"), cons("k"), cons("s"), cons("r"), vow("a"), vow("i")],
            ..Default::default()
        }
    }

    #[test]
    fn max_cluster_size_counts_consonant_runs() {
        let mut p = base();
        p.constraints = vec![PhonotacticConstraint::MaxClusterSize(2)];
        assert!(is_legal(&p, &seq(&["p", "a", "t", "r", "a"]))); // runs of 1, 2
        assert!(!is_legal(&p, &seq(&["s", "t", "r", "a"]))); // run of 3
    }

    #[test]
    fn no_geminate_rejects_doubles() {
        let mut p = base();
        p.constraints = vec![PhonotacticConstraint::NoGeminate];
        assert!(is_legal(&p, &seq(&["p", "a", "t", "a"])));
        assert!(!is_legal(&p, &seq(&["p", "p", "a"])));
        assert!(!is_legal(&p, &seq(&["a", "a"])));
    }

    #[test]
    fn forbid_bigram_blocks_the_ordered_pair() {
        let mut p = base();
        p.constraints = vec![PhonotacticConstraint::ForbidBigram("s".into(), "r".into())];
        assert!(!is_legal(&p, &seq(&["s", "r", "a"])));
        assert!(is_legal(&p, &seq(&["r", "s", "a"]))); // reverse is fine
    }
}
