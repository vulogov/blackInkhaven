//! LING-1 L-P6 (Wave 3) — the Oracle.
//!
//! Deterministic well-formedness checks over a candidate word, layered by
//! linguistic level. Where `audit` scans the finished lexicon, the Oracle judges
//! *arbitrary* input — a word you are about to coin, or one found in the prose —
//! and says whether it could be a word of the language and why not.
//!
//! [`check_word`] covers levels 1–2 over a candidate word: phonotactics (unknown
//! segments + declared constraint violations, reusing the phonology validator) and
//! morphology (does it analyse as root + affixes?). [`check_clause`] adds levels
//! 3–4 over a whole clause: agreement (does the verb inflect for its subject's
//! features?) and syntax (does the argument count match the verb's valence?),
//! reusing the agreement generator and the argument linker.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::conlang::phonology::validator;
use crate::conlang::types::Phonology;
use crate::conlang::types::morphology::Morphology;
use crate::language_entry::DictionaryEntry;

/// One Oracle finding, tagged by level.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Finding {
    /// `phonotactics` | `morphology` | `agreement` | `syntax`.
    pub level: &'static str,
    pub message: String,
}

/// The Oracle report for one word.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct OracleReport {
    pub word: String,
    pub findings: Vec<Finding>,
}

impl OracleReport {
    pub fn ok(&self) -> bool {
        self.findings.is_empty()
    }
}

/// Run the Oracle over `word`.
pub fn check_word(
    phon: &Phonology,
    morph: &Morphology,
    entries: &[DictionaryEntry],
    word: &str,
) -> OracleReport {
    let mut r = OracleReport { word: word.to_string(), ..Default::default() };
    let seq = phon.segment(&word.to_lowercase());
    if seq.is_empty() {
        r.findings.push(Finding { level: "phonotactics", message: "empty or unsegmentable".into() });
        return r;
    }

    // ── Level 1: phonotactics ──
    let unknown: Vec<&str> =
        seq.iter().filter(|s| phon.phoneme(s).is_none()).map(|s| s.as_str()).collect();
    if !unknown.is_empty() {
        r.findings.push(Finding {
            level: "phonotactics",
            message: format!("segment(s) not in the inventory: {}", unknown.join(" ")),
        });
        // Un-analyzable at higher levels once the segments themselves are foreign.
        return r;
    }
    for v in validator::violations(phon, &seq) {
        r.findings.push(Finding { level: "phonotactics", message: format!("contains {v}") });
    }

    // ── Level 2: morphology ──
    // Skip if it is itself a dictionary word (a bare, listed root is fine).
    let is_listed = entries.iter().any(|e| e.word.eq_ignore_ascii_case(word));
    if !is_listed {
        let parse = crate::conlang::parse::parse(phon, morph, entries, word);
        if parse.parses.is_empty() {
            r.findings.push(Finding {
                level: "morphology",
                message: "no analysis: not a listed root, and no affix stripping reaches one".into(),
            });
        }
    }

    r
}

/// A clause to judge — the observed verb form and its arguments, plus what the
/// verb *should* agree with. Surface forms throughout; `verb_root` and
/// `subject_features` are what a level-3 agreement check needs to regenerate the
/// expected verb form.
#[derive(Debug, Clone)]
pub struct ClauseInput<'a> {
    /// The observed verb surface form.
    pub verb: &'a str,
    /// The verb's root, for regenerating its expected agreeing form (level 3).
    pub verb_root: Option<&'a str>,
    /// The verb's valence (`transitive` etc.); empty ⇒ inferred, no arity check.
    pub valence: &'a str,
    /// The clause's core arguments, subject first.
    pub args: &'a [String],
    /// The subject's grammatical features (e.g. `number: pl`), for agreement.
    pub subject_features: &'a BTreeMap<String, String>,
}

/// Run the Oracle's levels 3–4 over a clause: subject–verb agreement and
/// argument structure. Level-1/2 word checks are [`check_word`]'s job.
pub fn check_clause(phon: &Phonology, morph: &Morphology, clause: &ClauseInput) -> OracleReport {
    let mut r = OracleReport { word: clause.verb.to_string(), ..Default::default() };

    // ── Level 4: argument structure ──
    // Reuse the argument linker; surface only genuine arity mismatches, not its
    // informational "inferring from arity" note (which fires on an unknown valence).
    let link = crate::conlang::link::link(clause.verb, clause.valence, clause.args);
    for issue in link.issues.into_iter().filter(|i| !i.contains("inferring")) {
        r.findings.push(Finding { level: "syntax", message: issue });
    }

    // ── Level 3: subject–verb agreement ──
    // Only when the language declares a verb-agreement rule and we were given the
    // subject's features and the verb's root to regenerate the expected form.
    if let (Some(rule), Some(root)) = (morph.agreement_for("verb"), clause.verb_root) {
        if !clause.subject_features.is_empty() {
            match crate::conlang::morphology::agreement::agree(
                phon,
                morph,
                rule,
                root,
                "V",
                clause.subject_features,
            ) {
                Some(expected) if !expected.form.eq_ignore_ascii_case(clause.verb) => {
                    r.findings.push(Finding {
                        level: "agreement",
                        message: format!(
                            "verb `{}` does not agree with the subject — expected `{}` ({})",
                            clause.verb, expected.form, expected.gloss
                        ),
                    });
                }
                None => {
                    r.findings.push(Finding {
                        level: "agreement",
                        message: "cannot check agreement — the verb paradigm has no cell for the subject's features".into(),
                    });
                }
                _ => {}
            }
        }
    }

    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phon() -> Phonology {
        let body = r#"{
            phonemes: [
                { ipa: "k", kind: "consonant" }, { ipa: "t", kind: "consonant" },
                { ipa: "n", kind: "consonant" }, { ipa: "a", kind: "vowel" }, { ipa: "i", kind: "vowel" }
            ],
            constraints: [ { kind: "no_geminate" } ]
        }"#;
        Phonology::from_hjson(body).unwrap().unwrap()
    }

    fn morph() -> Morphology {
        Morphology::from_hjson(r#"{ morphemes: [ { id: "pl", gloss: "PL", form: "i", position: "suffix" } ] }"#)
            .unwrap()
            .unwrap()
    }

    fn entry(w: &str) -> DictionaryEntry {
        DictionaryEntry { word: w.into(), pos: "noun".into(), translation: "x".into(), ..Default::default() }
    }

    #[test]
    fn a_well_formed_listed_word_passes() {
        let r = check_word(&phon(), &morph(), &[entry("kata")], "kata");
        assert!(r.ok(), "findings: {:?}", r.findings);
    }

    #[test]
    fn a_foreign_segment_is_flagged() {
        let r = check_word(&phon(), &morph(), &[entry("kata")], "kabu");
        assert!(r.findings.iter().any(|f| f.level == "phonotactics" && f.message.contains("not in the inventory")));
    }

    #[test]
    fn a_constraint_violation_is_flagged() {
        // "katta" has a geminate /tt/.
        let r = check_word(&phon(), &morph(), &[entry("kata")], "katta");
        assert!(r.findings.iter().any(|f| f.level == "phonotactics" && f.message.contains("geminate")));
    }

    #[test]
    fn an_unanalyzable_but_legal_word_is_flagged_at_morphology() {
        // "nika" is legal phonotactically but reaches no root.
        let r = check_word(&phon(), &morph(), &[entry("kata")], "nika");
        assert!(r.findings.iter().any(|f| f.level == "morphology"));
    }

    #[test]
    fn a_suffixed_form_of_a_root_passes() {
        // "katai" = kata + PL — the parser reaches the root, so no morphology finding.
        let r = check_word(&phon(), &morph(), &[entry("kata")], "katai");
        assert!(!r.findings.iter().any(|f| f.level == "morphology"), "findings: {:?}", r.findings);
    }

    // A morphology with subject–verb agreement: the verb conjugates for the
    // subject's number via the `vconj` paradigm (plural adds -i).
    fn agreeing_morph() -> Morphology {
        let body = r#"{
            morphemes: [ { id: "pl", gloss: "PL", form: "i", position: "suffix" } ]
            paradigms: [ { name: "vconj", cells: [
                { features: { number: "sg" }, morphemes: [] }
                { features: { number: "pl" }, morphemes: ["pl"] }
            ] } ]
            agreement: [ { dependent: "verb", head: "subject", features: ["number"], paradigm: "vconj" } ]
        }"#;
        Morphology::from_hjson(body).unwrap().unwrap()
    }

    fn feats(pairs: &[(&str, &str)]) -> std::collections::BTreeMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn a_well_formed_transitive_clause_passes() {
        let args = vec!["she".to_string(), "bird".to_string()];
        let none = feats(&[]);
        let clause = ClauseInput { verb: "kata", verb_root: None, valence: "transitive", args: &args, subject_features: &none };
        let r = check_clause(&phon(), &agreeing_morph(), &clause);
        assert!(r.ok(), "findings: {:?}", r.findings);
    }

    #[test]
    fn an_arity_mismatch_is_flagged_at_syntax() {
        // Two arguments for an intransitive verb.
        let args = vec!["she".to_string(), "bed".to_string()];
        let none = feats(&[]);
        let clause = ClauseInput { verb: "kata", verb_root: None, valence: "intransitive", args: &args, subject_features: &none };
        let r = check_clause(&phon(), &agreeing_morph(), &clause);
        assert!(r.findings.iter().any(|f| f.level == "syntax"), "findings: {:?}", r.findings);
    }

    #[test]
    fn a_verb_agreeing_with_its_subject_passes() {
        // Plural subject → the verb should be "katai"; it is.
        let args = vec!["she".to_string(), "bird".to_string()];
        let pl = feats(&[("number", "pl")]);
        let clause = ClauseInput {
            verb: "katai",
            verb_root: Some("kata"),
            valence: "transitive",
            args: &args,
            subject_features: &pl,
        };
        let r = check_clause(&phon(), &agreeing_morph(), &clause);
        assert!(!r.findings.iter().any(|f| f.level == "agreement"), "findings: {:?}", r.findings);
    }

    #[test]
    fn a_verb_not_agreeing_with_its_subject_is_flagged() {
        // Plural subject but the bare (singular) verb form → agreement finding.
        let args = vec!["she".to_string(), "bird".to_string()];
        let pl = feats(&[("number", "pl")]);
        let clause = ClauseInput {
            verb: "kata",
            verb_root: Some("kata"),
            valence: "transitive",
            args: &args,
            subject_features: &pl,
        };
        let r = check_clause(&phon(), &agreeing_morph(), &clause);
        assert!(
            r.findings.iter().any(|f| f.level == "agreement" && f.message.contains("expected `katai`")),
            "findings: {:?}",
            r.findings
        );
    }
}
