//! Sentence assembly (LANG-1 syntax).
//!
//! The grammar pillar's capstone: take a subject, verb, and object and build a
//! *clause* that obeys the language's own grammar — ordering the constituents by
//! its `word_order`, case-marking the nouns by its `alignment`, running
//! agreement (adjective ↔ noun, verb ↔ subject), and emitting the surface clause
//! with an interlinear gloss and a literal English rendering. Ties together word
//! order + case + agreement + the lexicon. Pure + deterministic; everything is
//! built on paradigm generation and the agreement engine. Degrades gracefully —
//! a missing paradigm or case just leaves a word in its bare form.

use std::collections::BTreeMap;

use crate::conlang::morphology::paradigm;
use crate::conlang::types::morphology::Morphology;
use crate::conlang::Phonology;

/// A single word: its root and a short gloss.
#[derive(Debug, Clone)]
pub struct Word {
    pub root: String,
    pub gloss: String,
}

/// A noun phrase: a head noun, its number, and an optional adjective.
#[derive(Debug, Clone)]
pub struct NounPhrase {
    pub head: Word,
    pub number: String,
    pub adjective: Option<Word>,
}

/// A clause to assemble. The object is optional (intransitive when absent).
#[derive(Debug, Clone)]
pub struct Clause {
    pub subject: Option<NounPhrase>,
    pub verb: Option<Word>,
    /// The subject's person, for verb agreement (`1` / `2` / `3`).
    pub verb_person: String,
    pub object: Option<NounPhrase>,
    /// Paradigm names for nouns and verbs (defaults `noun` / `verb`).
    pub noun_paradigm: String,
    pub verb_paradigm: String,
}

/// The assembled clause.
#[derive(Debug, Clone)]
pub struct RenderedClause {
    /// `(surface, gloss)` for each word, in the language's order.
    pub words: Vec<(String, String)>,
    /// The surface clause as a single string.
    pub surface: String,
    /// A literal English rendering, in subject–verb–object order.
    pub literal: String,
}

/// Roles a constituent can play.
#[derive(Clone, Copy, PartialEq)]
enum Role {
    Subject,
    Verb,
    Object,
}

/// Assemble a clause from its parts, obeying the language's typology.
pub fn assemble(
    phon: &Phonology,
    morph: &Morphology,
    typology: &BTreeMap<String, String>,
    clause: &Clause,
) -> RenderedClause {
    let transitive = clause.object.is_some();
    let alignment = typology.get("alignment").map(String::as_str).unwrap_or("nominative_accusative");
    let order = typology.get("word_order").map(String::as_str).unwrap_or("svo");
    let adj_before = typology
        .get("adjective_order")
        .map(|v| !v.to_lowercase().contains("noun_adjective"))
        .unwrap_or(true);

    // Case of the subject and object, by alignment.
    let (subj_case, obj_case) = case_roles(alignment, transitive);

    // Render each constituent into its ordered list of (surface, gloss) words.
    let render_np = |np: &NounPhrase, case: Option<&str>| -> Vec<(String, String)> {
        let mut feats: BTreeMap<String, String> = BTreeMap::new();
        if !np.number.is_empty() {
            feats.insert("number".into(), np.number.clone());
        }
        if let Some(c) = case {
            feats.insert("case".into(), c.to_string());
        }
        let noun = inflect(phon, morph, &clause.noun_paradigm, &np.head, &feats);
        let mut out = Vec::new();
        // The adjective agrees with its noun in number and case.
        if let Some(adj) = &np.adjective {
            let adj_paradigm = morph
                .agreement_for("adjective")
                .map(|r| r.paradigm.clone())
                .unwrap_or_else(|| clause.noun_paradigm.clone());
            let a = inflect(phon, morph, &adj_paradigm, adj, &feats);
            if adj_before {
                out.push(a);
                out.push(noun);
            } else {
                out.push(noun);
                out.push(a);
            }
        } else {
            out.push(noun);
        }
        out
    };

    let subject = clause.subject.as_ref().map(|np| render_np(np, subj_case.as_deref()));
    let object = clause.object.as_ref().map(|np| render_np(np, obj_case.as_deref()));
    let verb = clause.verb.as_ref().map(|v| {
        // The verb agrees with its subject in person + number, when a rule says so.
        if let (Some(rule), Some(subj)) = (morph.agreement_for("verb"), &clause.subject) {
            let mut head: BTreeMap<String, String> = BTreeMap::new();
            head.insert("person".into(), clause.verb_person.clone());
            head.insert("number".into(), subj.number.clone());
            if let Some(a) = crate::conlang::morphology::agreement::agree(
                phon, morph, rule, &v.root, &v.gloss, &head,
            ) {
                return vec![(a.form, a.gloss)];
            }
        }
        vec![inflect(phon, morph, &clause.verb_paradigm, v, &BTreeMap::new())]
    });

    // Order the constituents by the language's word order.
    let mut words: Vec<(String, String)> = Vec::new();
    for role in word_order(order) {
        let part = match role {
            Role::Subject => &subject,
            Role::Verb => &verb,
            Role::Object => &object,
        };
        if let Some(ws) = part {
            words.extend(ws.iter().cloned());
        }
    }

    let surface = words.iter().map(|(w, _)| w.as_str()).collect::<Vec<_>>().join(" ");
    let literal = literal_english(clause);
    RenderedClause { words, surface, literal }
}

/// Inflect a word to the wanted features through a named paradigm. Falls back
/// to the bare root when the paradigm or a matching cell is missing — first
/// trying the full feature set, then number only.
fn inflect(
    phon: &Phonology,
    morph: &Morphology,
    paradigm_name: &str,
    word: &Word,
    wanted: &BTreeMap<String, String>,
) -> (String, String) {
    let bare = || (word.root.clone(), word.gloss.clone());
    let Some(template) = morph.paradigm(paradigm_name) else {
        return bare();
    };
    // Try every case spelling the wanted features might use, relaxing to
    // number-only if the full set has no cell.
    let attempts = relax(wanted);
    for w in &attempts {
        if let Some(row) = paradigm::realize_features(phon, morph, template, &word.root, &word.gloss, w) {
            return (row.form, row.gloss);
        }
    }
    bare()
}

/// Progressive feature sets to try, most-specific first: the full set (with
/// each candidate case spelling), then case alone, then number alone. A cell
/// matches only when it carries every feature asked for, so this lets a
/// case-only paradigm match a `{number, case}` request and vice versa.
fn relax(wanted: &BTreeMap<String, String>) -> Vec<BTreeMap<String, String>> {
    let mut out = Vec::new();
    let number = wanted.get("number");
    let case_spellings: Vec<&str> = wanted
        .get("case")
        .map(|c| case_spellings(c))
        .unwrap_or_default();

    // Full sets: number + each case spelling.
    for sp in &case_spellings {
        let mut w = BTreeMap::new();
        if let Some(n) = number {
            w.insert("number".to_string(), n.clone());
        }
        w.insert("case".to_string(), sp.to_string());
        out.push(w);
    }
    // Case alone, each spelling.
    for sp in &case_spellings {
        out.push([("case".to_string(), sp.to_string())].into_iter().collect());
    }
    // Number alone.
    if let Some(n) = number {
        out.push([("number".to_string(), n.clone())].into_iter().collect());
    }
    // The wanted set verbatim (covers the case where `case` had no known
    // spelling but the cell uses that literal value).
    out.push(wanted.clone());
    out
}

/// Short + long spellings a case might be written as in a paradigm cell.
fn case_spellings(case: &str) -> Vec<&'static str> {
    match case.to_lowercase().as_str() {
        "nom" | "nominative" => vec!["nom", "nominative"],
        "acc" | "accusative" => vec!["acc", "accusative"],
        "erg" | "ergative" => vec!["erg", "ergative"],
        "abs" | "absolutive" => vec!["abs", "absolutive"],
        "dat" | "dative" => vec!["dat", "dative"],
        "gen" | "genitive" => vec!["gen", "genitive"],
        _ => vec![],
    }
}

/// The case label for the subject and object, by alignment + transitivity.
fn case_roles(alignment: &str, transitive: bool) -> (Option<String>, Option<String>) {
    if alignment.to_lowercase().contains("ergative") {
        if transitive {
            (Some("erg".into()), Some("abs".into()))
        } else {
            (Some("abs".into()), None)
        }
    } else {
        // Nominative–accusative (the default).
        (Some("nom".into()), transitive.then(|| "acc".into()))
    }
}

/// Order the roles by a word-order code (`sov`, `svo`, `vso`, …).
fn word_order(code: &str) -> Vec<Role> {
    code.to_lowercase()
        .chars()
        .filter_map(|c| match c {
            's' => Some(Role::Subject),
            'v' => Some(Role::Verb),
            'o' => Some(Role::Object),
            _ => None,
        })
        .collect::<Vec<_>>()
        .into_iter()
        .fold(Vec::new(), |mut acc, r| {
            if !acc.contains(&r) {
                acc.push(r);
            }
            acc
        })
}

/// A literal English rendering in subject–verb–object order.
fn literal_english(clause: &Clause) -> String {
    let np = |np: &NounPhrase| -> String {
        match &np.adjective {
            Some(a) => format!("{} {}", a.gloss, np.head.gloss),
            None => np.head.gloss.clone(),
        }
    };
    let mut parts = Vec::new();
    if let Some(s) = &clause.subject {
        parts.push(np(s));
    }
    if let Some(v) = &clause.verb {
        parts.push(v.gloss.clone());
    }
    if let Some(o) = &clause.object {
        parts.push(np(o));
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phon() -> Phonology {
        let body = r#"{ phonemes: [
            { ipa: "k", kind: "consonant" }, { ipa: "t", kind: "consonant" },
            { ipa: "m", kind: "consonant" }, { ipa: "n", kind: "consonant" },
            { ipa: "r", kind: "consonant" }, { ipa: "s", kind: "consonant" },
            { ipa: "p", kind: "consonant" }, { ipa: "l", kind: "consonant" },
            { ipa: "a", kind: "vowel" }, { ipa: "i", kind: "vowel" }, { ipa: "u", kind: "vowel" }
        ] }"#;
        Phonology::from_hjson(body).unwrap().unwrap()
    }

    fn morph() -> Morphology {
        // Nouns take a case ending (-n accusative); the `noun` paradigm has nom
        // and acc cells. Adjectives agree in case via the same `noun` paradigm.
        let body = r#"{
            morphemes: [
                { id: "acc", gloss: "ACC", form: "n", position: "suffix", category: "case", value: "accusative" }
            ]
            paradigms: [ { name: "noun", cells: [
                { features: { case: "nom" }, morphemes: [] }
                { features: { case: "acc" }, morphemes: ["acc"] }
            ] } ]
            agreement: [
                { dependent: "adjective", head: "noun", features: ["case"], paradigm: "noun" }
            ]
        }"#;
        Morphology::from_hjson(body).unwrap().unwrap()
    }

    fn clause() -> Clause {
        Clause {
            subject: Some(NounPhrase { head: Word { root: "kira".into(), gloss: "bird".into() }, number: "sg".into(), adjective: None }),
            verb: Some(Word { root: "nami".into(), gloss: "see".into() }),
            verb_person: "3".into(),
            object: Some(NounPhrase { head: Word { root: "pata".into(), gloss: "stone".into() }, number: "sg".into(), adjective: None }),
            noun_paradigm: "noun".into(),
            verb_paradigm: "verb".into(),
        }
    }

    #[test]
    fn sov_clause_orders_and_case_marks() {
        let mut t = BTreeMap::new();
        t.insert("word_order".to_string(), "sov".to_string());
        t.insert("alignment".to_string(), "nominative_accusative".to_string());
        let r = assemble(&phon(), &morph(), &t, &clause());
        // SOV: subject (nom, bare) — object (acc, +n) — verb.
        assert_eq!(r.surface, "kira patan nami");
        assert_eq!(r.words[1].1, "stone-ACC");
        assert_eq!(r.literal, "bird see stone");
    }

    #[test]
    fn svo_reorders_the_verb() {
        let mut t = BTreeMap::new();
        t.insert("word_order".to_string(), "svo".to_string());
        let r = assemble(&phon(), &morph(), &t, &clause());
        assert_eq!(r.surface, "kira nami patan"); // S V O
    }

    #[test]
    fn adjective_agrees_in_case() {
        let mut c = clause();
        c.object.as_mut().unwrap().adjective = Some(Word { root: "mira".into(), gloss: "bright".into() });
        let mut t = BTreeMap::new();
        t.insert("word_order".to_string(), "svo".to_string());
        let r = assemble(&phon(), &morph(), &t, &c);
        // The object's adjective takes accusative too: "miran patan".
        assert!(r.surface.contains("miran patan"), "got: {}", r.surface);
    }

    #[test]
    fn ergative_alignment_marks_the_subject() {
        let body = r#"{
            morphemes: [ { id: "erg", gloss: "ERG", form: "k", position: "suffix", category: "case" } ]
            paradigms: [ { name: "noun", cells: [
                { features: { case: "abs" }, morphemes: [] }
                { features: { case: "erg" }, morphemes: ["erg"] }
            ] } ]
        }"#;
        let m = Morphology::from_hjson(body).unwrap().unwrap();
        let mut t = BTreeMap::new();
        t.insert("alignment".to_string(), "ergative_absolutive".to_string());
        t.insert("word_order".to_string(), "sov".to_string());
        let r = assemble(&phon(), &m, &t, &clause());
        // Ergative: subject takes -k, object is bare absolutive.
        assert_eq!(r.words[0].1, "bird-ERG");
        assert_eq!(r.words[1].0, "pata"); // bare absolutive object
    }
}
