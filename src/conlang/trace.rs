//! LING-1 L-P1 — the Consequence Tracer.
//!
//! Preview a *pending* sound change before committing it: apply the rule to a
//! copy of the whole lexicon, re-derive every form, and diff against the current
//! forms. It answers the questions a conlanger actually has — *which words does
//! this change touch? does it merge any distinctions? does it create new
//! homophones?* — using the existing SPE rewriter (`diachronic::derive_form`),
//! so the trace behaves exactly as a real derivation would.
//!
//! Non-destructive: nothing is written; the tracer only reports.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::conlang::diachronic::apply::derive_form;
use crate::conlang::types::Phonology;
use crate::conlang::types::allophony::AllophonyRule;
use crate::language_entry::DictionaryEntry;

/// One form the pending change would alter.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FormChange {
    pub word: String,
    /// Current (canonical) form.
    pub from: String,
    /// Form after the pending change.
    pub to: String,
    pub gloss: String,
}

/// A set of words that the change would collapse to the same form (they were
/// distinct before).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct NewHomophones {
    pub form: String,
    /// The words (citation forms) now identical.
    pub words: Vec<String>,
}

/// The consequence report for one pending sound change.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct TraceReport {
    pub rule: String,
    /// False when the rule string didn't parse — nothing else is populated.
    pub rule_valid: bool,
    pub analyzable_words: usize,
    /// How many forms the change alters.
    pub affected: usize,
    /// A bounded sample of the changes.
    pub changes: Vec<FormChange>,
    /// New homophone sets the change would create.
    pub new_homophones: Vec<NewHomophones>,
    /// Total words drawn into a new homophone set.
    pub merged_words: usize,
}

/// Parse `rule_str` into a rewrite rule (`s > ʃ / _ i`, `d > t / _ #`, …).
fn parse_rule(rule_str: &str) -> Option<AllophonyRule> {
    let json = format!("{{ \"rule\": {} }}", serde_json::to_string(rule_str).ok()?);
    serde_hjson::from_str::<AllophonyRule>(&json).ok()
}

/// Trace the consequences of `rule_str` across `entries` under `phon`. `sample`
/// bounds the number of example changes returned (the counts reflect all).
pub fn trace_sound_change(
    phon: &Phonology,
    entries: &[DictionaryEntry],
    rule_str: &str,
    sample: usize,
) -> TraceReport {
    let mut r = TraceReport { rule: rule_str.to_string(), ..Default::default() };

    let Some(rule) = parse_rule(rule_str) else {
        return r; // rule_valid stays false
    };
    r.rule_valid = true;
    let rules = std::slice::from_ref(&rule);

    // Derive every analyzable word with and without the pending rule. The empty-
    // rule pass gives the canonical current form, so only the rule causes diffs.
    let mut after_forms: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new(); // to -> [(word, from)]
    for e in entries {
        let word = e.word.to_lowercase();
        let seq = phon.segment(&word);
        if seq.is_empty() || !seq.iter().all(|s| phon.phoneme(s).is_some()) {
            continue;
        }
        r.analyzable_words += 1;
        let from = derive_form(phon, &[], &word);
        let to = derive_form(phon, rules, &word);
        if from != to {
            r.affected += 1;
            if r.changes.len() < sample {
                r.changes.push(FormChange {
                    word: e.word.clone(),
                    from: from.clone(),
                    to: to.clone(),
                    gloss: e.translation.clone(),
                });
            }
        }
        after_forms.entry(to).or_default().push((e.word.clone(), from));
    }

    // New homophones: post-change forms shared by words that had *distinct*
    // current forms.
    for (to, members) in &after_forms {
        let distinct_from: std::collections::BTreeSet<&String> =
            members.iter().map(|(_, from)| from).collect();
        if members.len() > 1 && distinct_from.len() > 1 {
            let words: Vec<String> = members.iter().map(|(w, _)| w.clone()).collect();
            r.merged_words += words.len();
            r.new_homophones.push(NewHomophones { form: to.clone(), words });
        }
    }
    r.new_homophones.sort_by(|a, b| b.words.len().cmp(&a.words.len()).then_with(|| a.form.cmp(&b.form)));

    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phon() -> Phonology {
        let body = r#"{
            phonemes: [
                { ipa: "p", kind: "consonant" }, { ipa: "t", kind: "consonant" },
                { ipa: "k", kind: "consonant" }, { ipa: "s", kind: "consonant" },
                { ipa: "ʃ", kind: "consonant" }, { ipa: "f", kind: "consonant" },
                { ipa: "a", kind: "vowel" }, { ipa: "i", kind: "vowel" }
            ]
        }"#;
        Phonology::from_hjson(body).unwrap().unwrap()
    }

    fn entry(word: &str, gloss: &str) -> DictionaryEntry {
        DictionaryEntry {
            word: word.into(),
            pos: "noun".into(),
            translation: gloss.into(),
            ..Default::default()
        }
    }

    #[test]
    fn traces_which_words_a_change_touches() {
        // s > ʃ / _ i : palatalise /s/ before /i/.
        let p = phon();
        let entries = [entry("si", "one"), entry("sa", "two"), entry("kasi", "three")];
        let r = trace_sound_change(&p, &entries, "s > ʃ / _ i", 10);
        assert!(r.rule_valid);
        assert_eq!(r.analyzable_words, 3);
        assert_eq!(r.affected, 2); // si, kasi (sa unchanged — no following i)
        let touched: Vec<&str> = r.changes.iter().map(|c| c.word.as_str()).collect();
        assert!(touched.contains(&"si") && touched.contains(&"kasi"));
        assert!(!touched.contains(&"sa"));
    }

    #[test]
    fn detects_new_homophones_from_a_merger() {
        // s > ʃ merges /s/ and /ʃ/ before i: "si" and "ʃi"... use s/ʃ collision.
        // "sa" and "ʃa": rule s > ʃ / _ a makes sa → ʃa, colliding with existing ʃa.
        let p = phon();
        let entries = [entry("sa", "river"), entry("ʃa", "star")];
        let r = trace_sound_change(&p, &entries, "s > ʃ / _ a", 10);
        assert_eq!(r.affected, 1); // sa → ʃa
        assert_eq!(r.new_homophones.len(), 1);
        assert_eq!(r.new_homophones[0].form, "ʃa");
        assert_eq!(r.merged_words, 2);
    }

    #[test]
    fn an_inapplicable_rule_changes_nothing() {
        let p = phon();
        let r = trace_sound_change(&p, &[entry("kata", "stone")], "p > f / _ #", 10);
        assert!(r.rule_valid);
        assert_eq!(r.affected, 0);
        assert!(r.new_homophones.is_empty());
    }

    #[test]
    fn an_invalid_rule_is_reported_not_panicked() {
        let p = phon();
        let r = trace_sound_change(&p, &[entry("si", "x")], "this is not a rule", 10);
        assert!(!r.rule_valid);
        assert_eq!(r.affected, 0);
    }
}
