//! Deterministic phonotactic validation (LANG-1 P1.1).
//!
//! A single linear pass over a candidate phoneme sequence (by IPA) decides
//! legality against every declared constraint. Pure and total.

use crate::conlang::phonology::ipa;
use crate::conlang::phonology::syllable::{syllabify, Syllable};
use crate::conlang::types::{PhonemeKind, Phonology, PhonotacticConstraint};

/// True iff `seq` (phonemes by IPA) satisfies every constraint. Syllable
/// structure is computed once, and only when a constraint actually needs it.
pub fn is_legal(phon: &Phonology, seq: &[String]) -> bool {
    let sylls = if phon.constraints.iter().any(|c| c.needs_syllables()) {
        Some(syllabify(phon, seq))
    } else {
        None
    };
    phon.constraints
        .iter()
        .all(|c| satisfies(phon, seq, sylls.as_deref(), c))
}

/// LING-1 L-P6 (Oracle) — describe each constraint `seq` violates (empty when
/// legal). Reuses the exact per-constraint check `is_legal` runs.
pub fn violations(phon: &Phonology, seq: &[String]) -> Vec<String> {
    let sylls = if phon.constraints.iter().any(|c| c.needs_syllables()) {
        Some(syllabify(phon, seq))
    } else {
        None
    };
    phon.constraints
        .iter()
        .filter(|c| !satisfies(phon, seq, sylls.as_deref(), c))
        .map(describe_constraint)
        .collect()
}

fn describe_constraint(c: &PhonotacticConstraint) -> String {
    match c {
        PhonotacticConstraint::MaxClusterSize(n) => format!("consonant cluster longer than {n}"),
        PhonotacticConstraint::NoGeminate => "a geminate (doubled) segment".to_string(),
        PhonotacticConstraint::ForbidBigram(a, b) => format!("the forbidden sequence /{a}{b}/"),
        PhonotacticConstraint::ForbidInOnset(cs) => format!("a forbidden onset ({})", cs.join(", ")),
        PhonotacticConstraint::ForbidInCoda(cs) => format!("a forbidden coda ({})", cs.join(", ")),
        PhonotacticConstraint::SonoritySequencing => "a sonority-sequencing violation".to_string(),
    }
}

fn satisfies(
    phon: &Phonology,
    seq: &[String],
    sylls: Option<&[Syllable]>,
    c: &PhonotacticConstraint,
) -> bool {
    match c {
        PhonotacticConstraint::MaxClusterSize(n) => max_consonant_run(phon, seq) <= *n,
        PhonotacticConstraint::NoGeminate => !seq.windows(2).any(|w| w[0] == w[1]),
        PhonotacticConstraint::ForbidBigram(a, b) => {
            !seq.windows(2).any(|w| &w[0] == a && &w[1] == b)
        }
        PhonotacticConstraint::ForbidInOnset(classes) => sylls
            .map(|s| !s.iter().any(|syl| any_in_classes(phon, &syl.onset, classes)))
            .unwrap_or(true),
        PhonotacticConstraint::ForbidInCoda(classes) => sylls
            .map(|s| !s.iter().any(|syl| any_in_classes(phon, &syl.coda, classes)))
            .unwrap_or(true),
        PhonotacticConstraint::SonoritySequencing => {
            sylls.map(|s| s.iter().all(|syl| sonority_well_formed(phon, syl))).unwrap_or(true)
        }
    }
}

/// True if any phoneme in `slot` belongs to any of the named classes.
fn any_in_classes(phon: &Phonology, slot: &[String], classes: &[String]) -> bool {
    slot.iter().any(|ipa| {
        classes
            .iter()
            .any(|c| phon.class_members(c).iter().any(|m| m == ipa))
    })
}

/// Onset sonority must strictly rise toward the nucleus; coda sonority must
/// strictly fall away from it.
fn sonority_well_formed(phon: &Phonology, syl: &Syllable) -> bool {
    let rising = syl
        .onset
        .windows(2)
        .all(|w| ipa::sonority_of(phon, &w[0]) < ipa::sonority_of(phon, &w[1]));
    let falling = syl
        .coda
        .windows(2)
        .all(|w| ipa::sonority_of(phon, &w[0]) > ipa::sonority_of(phon, &w[1]));
    rising && falling
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
        Phoneme { ipa: ipa.into(), romanize: None, kind: PhonemeKind::Consonant, sonority: None }
    }
    fn vow(ipa: &str) -> Phoneme {
        Phoneme { ipa: ipa.into(), romanize: None, kind: PhonemeKind::Vowel, sonority: None }
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

    fn with_classes() -> Phonology {
        let mut p = base();
        p.classes = [
            ("C".to_string(), vec!["p", "t", "k", "s", "r"].into_iter().map(String::from).collect()),
            ("Stop".to_string(), vec!["p".to_string(), "t".to_string(), "k".to_string()]),
            ("Liquid".to_string(), vec!["r".to_string()]),
        ]
        .into_iter()
        .collect();
        p
    }

    #[test]
    fn forbid_in_coda_rejects_a_listed_class_in_the_coda() {
        let mut p = with_classes();
        p.constraints = vec![PhonotacticConstraint::ForbidInCoda(vec!["Stop".into()])];
        // /a r/ : r codas (a liquid) — allowed.
        assert!(is_legal(&p, &seq(&["a", "r"])));
        // /a t/ : t codas (a stop) — forbidden.
        assert!(!is_legal(&p, &seq(&["a", "t"])));
    }

    #[test]
    fn forbid_in_onset_rejects_a_listed_class_in_the_onset() {
        let mut p = with_classes();
        p.constraints = vec![PhonotacticConstraint::ForbidInOnset(vec!["Liquid".into()])];
        // /r a/ : r onsets (a liquid) — forbidden.
        assert!(!is_legal(&p, &seq(&["r", "a"])));
        // /t a/ : t onsets (a stop) — allowed.
        assert!(is_legal(&p, &seq(&["t", "a"])));
    }

    #[test]
    fn sonority_sequencing_requires_rising_onset_and_falling_coda() {
        let mut p = with_classes();
        p.constraints = vec![PhonotacticConstraint::SonoritySequencing];
        // /t r a/ : onset t<r rises → legal.
        assert!(is_legal(&p, &seq(&["t", "r", "a"])));
        // /a r t/ : coda r>t falls → legal.
        assert!(is_legal(&p, &seq(&["a", "r", "t"])));
        // /a t r/ : after syllabification "tr" onsets a nucleus-less tail;
        // a genuine sonority plateau (s + s) in a coda is the clean reject:
        // /a s s/ — equal sonority doesn't strictly fall.
        assert!(!is_legal(&p, &seq(&["a", "s", "s"])));
    }
}
