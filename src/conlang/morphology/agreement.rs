//! Agreement / concord (LANG-1 P3.x).
//!
//! A *dependent* word (an adjective, a verb) copies grammatical features from
//! the *head* it modifies (its noun, its subject) and realises them — an
//! adjective taking its noun's number and case, a verb taking its subject's
//! person and number. Given the head's features, [`agree`] selects the
//! dependent paradigm cell that matches on the agreement features and returns
//! the inflected dependent form. Pure + deterministic; reuses paradigm
//! generation.

use std::collections::BTreeMap;

use crate::conlang::morphology::paradigm;
use crate::conlang::types::morphology::{AgreementRule, Morphology};
use crate::conlang::types::Phonology;

/// The outcome of making a dependent agree with a head.
#[derive(Debug, Clone)]
pub struct Agreement {
    /// The agreeing surface form of the dependent.
    pub form: String,
    /// Its Leipzig gloss.
    pub gloss: String,
    /// The features that were matched (the projection of the head's features
    /// onto the rule's agreement features).
    pub matched: BTreeMap<String, String>,
}

/// Make `dependent_root` (gloss `dependent_gloss`) agree with a head whose
/// grammatical features are `head_features`, under `rule`. Returns `None` when
/// the rule's paradigm is missing or no cell matches the projected features.
pub fn agree(
    phon: &Phonology,
    morph: &Morphology,
    rule: &AgreementRule,
    dependent_root: &str,
    dependent_gloss: &str,
    head_features: &BTreeMap<String, String>,
) -> Option<Agreement> {
    // Project the head's features onto just the agreement features.
    let wanted: BTreeMap<String, String> = rule
        .features
        .iter()
        .filter_map(|f| head_features.get(f).map(|v| (f.clone(), v.clone())))
        .collect();

    // Pick the dependent paradigm cell that matches every agreement feature.
    let template = morph.paradigm(&rule.paradigm)?;
    let row = paradigm::realize_features(phon, morph, template, dependent_root, dependent_gloss, &wanted)?;

    Some(Agreement { form: row.form, gloss: row.gloss, matched: wanted })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phon() -> Phonology {
        let body = r#"{ phonemes: [
            { ipa: "k", kind: "consonant" }, { ipa: "t", kind: "consonant" },
            { ipa: "m", kind: "consonant" }, { ipa: "r", kind: "consonant" },
            { ipa: "a", kind: "vowel" }, { ipa: "i", kind: "vowel" }
        ] }"#;
        Phonology::from_hjson(body).unwrap().unwrap()
    }

    fn morph() -> Morphology {
        // An adjective agrees with its noun in number; the `adj` paradigm marks
        // plural with -i.
        let body = r#"{
            morphemes: [ { id: "pl", gloss: "PL", form: "i", position: "suffix" } ]
            paradigms: [ { name: "adj", cells: [
                { features: { number: "sg" }, morphemes: [] }
                { features: { number: "pl" }, morphemes: ["pl"] }
            ] } ]
            agreement: [
                { dependent: "adjective", head: "noun", features: ["number"], paradigm: "adj" }
            ]
        }"#;
        Morphology::from_hjson(body).unwrap().unwrap()
    }

    #[test]
    fn adjective_agrees_with_plural_noun() {
        let p = phon();
        let m = morph();
        let rule = m.agreement_for("adjective").unwrap();
        let head: BTreeMap<String, String> =
            [("number".to_string(), "pl".to_string())].into_iter().collect();
        let a = agree(&p, &m, rule, "mira", "bright", &head).unwrap();
        assert_eq!(a.form, "mirai"); // bright + PL
        assert_eq!(a.gloss, "bright-PL");
        assert_eq!(a.matched.get("number").unwrap(), "pl");
    }

    #[test]
    fn singular_head_gives_bare_form() {
        let p = phon();
        let m = morph();
        let rule = m.agreement_for("adjective").unwrap();
        let head: BTreeMap<String, String> =
            [("number".to_string(), "sg".to_string())].into_iter().collect();
        let a = agree(&p, &m, rule, "mira", "bright", &head).unwrap();
        assert_eq!(a.form, "mira");
    }
}
