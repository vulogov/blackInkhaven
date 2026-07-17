//! LING-1 L-P6 (Wave 3) — the Oracle.
//!
//! Deterministic well-formedness checks over a candidate word, layered by
//! linguistic level. Where `audit` scans the finished lexicon, the Oracle judges
//! *arbitrary* input — a word you are about to coin, or one found in the prose —
//! and says whether it could be a word of the language and why not.
//!
//! This first cut covers levels 1–2: phonotactics (unknown segments + declared
//! constraint violations, reusing the phonology validator) and morphology (does
//! it analyse as root + affixes?). Agreement and syntax (levels 3–4) join it with
//! the clause engine.

use serde::Serialize;

use crate::conlang::phonology::validator;
use crate::conlang::types::Phonology;
use crate::conlang::types::morphology::Morphology;
use crate::language_entry::DictionaryEntry;

/// One Oracle finding, tagged by level.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Finding {
    /// `phonotactics` | `morphology`.
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
}
