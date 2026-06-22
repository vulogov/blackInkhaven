//! LANG-3 Tier 1 (RBMT) — the reverse direction (conlang → English) and
//! cross-conlang translation (conlang A → conlang B through an English pivot).
//!
//! Reverse is the mirror of [`super::translate`]: each conlang surface word is
//! mapped *back* to its English gloss. The catch is morphology — a surface word
//! may be inflected — so the reverse index is built not just from headwords but
//! from every form each headword's paradigm generates (and any explicit
//! inflections recorded on the entry), so an inflected word un-inflects to its
//! lemma. Roles are read off the language's own `word_order`, and a deliberately
//! plain English sentence is generated.
//!
//! Cross-conlang translation (RFC §8.7) is the documented degraded path: reverse
//! A into English, then forward that English into B. The English pivot is part
//! of the result so the meaning's waypoint is always visible.
//!
//! Pure and deterministic.

use std::collections::BTreeMap;
use std::collections::HashMap;

use crate::conlang::morphology::paradigm;
use crate::conlang::types::morphology::Morphology;
use crate::conlang::Phonology;
use crate::language_entry::DictionaryEntry;

/// Everything the engine needs about one language, bundled so cross-translation
/// can take two of them without a dozen parameters.
pub struct LangCtx<'a> {
    pub phon: &'a Phonology,
    pub morph: &'a Morphology,
    pub typology: &'a BTreeMap<String, String>,
    pub entries: &'a [DictionaryEntry],
}

/// What a conlang surface form reverses to: its English gloss and whether the
/// form was a plural (so English generation can agree number and tense).
#[derive(Debug, Clone)]
struct RevForm {
    gloss: String,
    plural: bool,
}

/// An index from every known conlang surface form to its English gloss.
pub struct ReverseIndex {
    forms: HashMap<String, RevForm>,
}

/// The broad class a part-of-speech names, for picking a paradigm.
fn broad_paradigm(pos: &str) -> Option<&'static str> {
    let p = pos.to_lowercase();
    if p.starts_with('n') {
        Some("noun")
    } else if p.starts_with('v') {
        Some("verb")
    } else {
        None
    }
}

impl ReverseIndex {
    /// Build the index from a language's dictionary, expanding each entry into
    /// the surface forms its paradigm produces so inflected words are findable.
    pub fn build(phon: &Phonology, morph: &Morphology, entries: &[DictionaryEntry]) -> Self {
        let mut forms: HashMap<String, RevForm> = HashMap::new();
        for e in entries {
            if e.word.trim().is_empty() || e.translation.trim().is_empty() {
                continue;
            }
            // Generated paradigm forms first (lowest priority), tagging the
            // plural cells so number survives the round trip.
            if let Some(name) = broad_paradigm(&e.pos) {
                if let Some(tmpl) = morph.paradigm(name) {
                    for row in paradigm::generate(phon, morph, tmpl, &e.word, &e.translation) {
                        let plural = row
                            .features
                            .get("number")
                            .map(|n| n.starts_with("pl"))
                            .unwrap_or(false);
                        forms
                            .entry(row.form.to_lowercase())
                            .or_insert_with(|| RevForm { gloss: e.translation.clone(), plural });
                    }
                }
            }
            // Explicitly recorded inflections (number unknown → treated singular).
            for v in e.inflection.values() {
                forms.entry(v.to_lowercase()).or_insert_with(|| RevForm {
                    gloss: e.translation.clone(),
                    plural: false,
                });
            }
            // The bare headword wins any tie.
            forms.insert(
                e.word.to_lowercase(),
                RevForm { gloss: e.translation.clone(), plural: false },
            );
        }
        ReverseIndex { forms }
    }

    fn lookup(&self, surface: &str) -> Option<&RevForm> {
        self.forms.get(&surface.to_lowercase())
    }
}

/// Naive English pluralizer for a noun lemma.
fn pluralize_en(noun: &str) -> String {
    if let Some(stem) = noun.strip_suffix('y') {
        if !stem.ends_with(['a', 'e', 'i', 'o', 'u']) {
            return format!("{stem}ies"); // city → cities
        }
    }
    if noun.ends_with('s')
        || noun.ends_with('x')
        || noun.ends_with('z')
        || noun.ends_with("ch")
        || noun.ends_with("sh")
    {
        return format!("{noun}es"); // box → boxes
    }
    format!("{noun}s")
}

/// English 3rd-person-singular present of a verb lemma (`see` → `sees`).
fn verb_3sg(verb: &str) -> String {
    if let Some(stem) = verb.strip_suffix('y') {
        if !stem.ends_with(['a', 'e', 'i', 'o', 'u']) {
            return format!("{stem}ies"); // carry → carries
        }
    }
    if verb.ends_with('s')
        || verb.ends_with('x')
        || verb.ends_with('z')
        || verb.ends_with("ch")
        || verb.ends_with("sh")
    {
        return format!("{verb}es"); // watch → watches
    }
    format!("{verb}s")
}

/// The role each surface position fills, in the language's order, for a clause
/// of the given transitivity. Defaults to SVO when `word_order` is unset or odd.
fn role_sequence(word_order: &str, transitive: bool) -> Vec<char> {
    let mut chars: Vec<char> =
        word_order.to_lowercase().chars().filter(|c| matches!(c, 's' | 'o' | 'v')).collect();
    if chars.len() != 3 {
        chars = vec!['s', 'v', 'o'];
    }
    if !transitive {
        chars.retain(|c| *c != 'o');
    }
    chars
}

/// Strip a leading English/Romance infinitive marker so a verb gloss reads as a
/// finite English verb ("to see" → "see").
fn finite(gloss: &str) -> &str {
    gloss.strip_prefix("to ").unwrap_or(gloss)
}

/// A finished reverse translation.
#[derive(Debug, Clone)]
pub struct ReverseTranslation {
    /// The conlang source.
    pub source: String,
    /// The generated English.
    pub english: String,
    /// `(surface, gloss)` per word, for a word-by-word display.
    pub words: Vec<(String, String)>,
    /// Conlang words no lexicon form matched (passed through `«word»`).
    pub unresolved: Vec<String>,
    /// Aggregate confidence, `0.0..=1.0`.
    pub confidence: f32,
}

/// **Reverse (conlang → English).** Map each surface word back to its gloss by
/// the (paradigm-expanded) lexicon, read roles off `typology.word_order`, and
/// generate a plain English sentence.
pub fn reverse(
    phon: &Phonology,
    morph: &Morphology,
    typology: &BTreeMap<String, String>,
    entries: &[DictionaryEntry],
    surface: &str,
) -> ReverseTranslation {
    let idx = ReverseIndex::build(phon, morph, entries);

    let tokens: Vec<String> = surface
        .split(|c: char| c.is_whitespace() || matches!(c, '.' | ',' | '!' | '?' | ';' | ':'))
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect();

    let mut words: Vec<(String, String)> = Vec::new();
    let mut unresolved: Vec<String> = Vec::new();
    let mut confs: Vec<f32> = Vec::new();

    // Reverse-gloss every token to its English meaning and number (or `«token»`).
    let glossed: Vec<(String, bool)> = tokens
        .iter()
        .map(|t| match idx.lookup(t) {
            Some(f) => {
                words.push((t.clone(), f.gloss.clone()));
                confs.push(0.9);
                (f.gloss.clone(), f.plural)
            }
            None => {
                let marked = format!("«{t}»");
                words.push((t.clone(), marked.clone()));
                unresolved.push(t.clone());
                confs.push(0.2);
                (marked, false)
            }
        })
        .collect();

    let word_order = typology.get("word_order").map(String::as_str).unwrap_or("svo");
    let transitive = glossed.len() >= 3;
    let roles = role_sequence(word_order, transitive);

    // Assign roles positionally per the declared order.
    let mut subject: Option<&(String, bool)> = None;
    let mut verb: Option<&(String, bool)> = None;
    let mut object: Option<&(String, bool)> = None;
    for (i, g) in glossed.iter().enumerate() {
        match roles.get(i) {
            Some('s') => subject = Some(g),
            Some('v') => verb = Some(g),
            Some('o') => object = Some(g),
            _ => {}
        }
    }

    // Generate a plain English clause, agreeing number on the nouns and the
    // present-tense verb with a singular subject. ("the bird sees the stones".)
    let noun_en = |(gloss, plural): &(String, bool)| -> String {
        if *plural { pluralize_en(finite(gloss)) } else { finite(gloss).to_string() }
    };
    let subject_singular = subject.map(|(_, p)| !*p).unwrap_or(true);
    let mut english = String::new();
    if let Some(s) = subject {
        english.push_str(&format!("the {} ", noun_en(s)));
    }
    if let Some((vg, _)) = verb {
        let v = finite(vg);
        english.push_str(&if subject_singular { verb_3sg(v) } else { v.to_string() });
    }
    if let Some(o) = object {
        english.push_str(&format!(" the {}", noun_en(o)));
    }
    let english = english.trim().to_string();

    let confidence =
        if confs.is_empty() { 0.0 } else { confs.iter().sum::<f32>() / confs.len() as f32 };

    ReverseTranslation { source: surface.to_string(), english, words, unresolved, confidence }
}

/// A finished cross-conlang translation.
#[derive(Debug, Clone)]
pub struct CrossTranslation {
    /// The source-language surface.
    pub source: String,
    /// The English the source reversed into (the pivot).
    pub english: String,
    /// The target-language surface.
    pub target: String,
    /// `(surface, gloss)` per target word.
    pub words: Vec<(String, String)>,
    /// Source words that did not reverse, plus English words the target lexicon
    /// could not cover.
    pub unresolved: Vec<String>,
    /// Combined confidence (the two passes multiplied — error compounds).
    pub confidence: f32,
}

/// **Cross (conlang A → conlang B).** Reverse A into English, then forward that
/// English into B. The English pivot is exposed; confidence is the product of
/// the two passes, reflecting compounding error.
pub fn cross(from: &LangCtx, to: &LangCtx, surface: &str) -> CrossTranslation {
    let rev = reverse(from.phon, from.morph, from.typology, from.entries, surface);
    let fwd = super::translate(to.phon, to.morph, to.typology, to.entries, &rev.english);

    let mut unresolved = rev.unresolved.clone();
    unresolved.extend(fwd.unresolved.clone());

    CrossTranslation {
        source: surface.to_string(),
        english: rev.english,
        target: fwd.target,
        words: fwd.words,
        unresolved,
        confidence: rev.confidence * fwd.confidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(word: &str, pos: &str, translation: &str) -> DictionaryEntry {
        DictionaryEntry {
            word: word.into(),
            pos: pos.into(),
            translation: translation.into(),
            ..Default::default()
        }
    }

    fn lexicon() -> Vec<DictionaryEntry> {
        vec![
            entry("kira", "noun", "bird"),
            entry("nami", "verb", "to see"),
            entry("pata", "noun", "stone"),
        ]
    }

    fn svo() -> BTreeMap<String, String> {
        let mut t = BTreeMap::new();
        t.insert("word_order".into(), "svo".into());
        t
    }

    #[test]
    fn reverses_an_svo_clause() {
        let phon = Phonology::default();
        let morph = Morphology::default();
        let entries = lexicon();
        let r = reverse(&phon, &morph, &svo(), &entries, "kira nami pata");
        assert_eq!(r.english, "the bird sees the stone");
        assert!(r.unresolved.is_empty());
        assert!(r.confidence > 0.8);
    }

    #[test]
    fn reverse_respects_word_order() {
        let phon = Phonology::default();
        let morph = Morphology::default();
        let mut sov = BTreeMap::new();
        sov.insert("word_order".to_string(), "sov".to_string());
        let entries = lexicon();
        // In an SOV language the surface is subject–object–verb.
        let r = reverse(&phon, &morph, &sov, &entries, "kira pata nami");
        assert_eq!(r.english, "the bird sees the stone");
    }

    #[test]
    fn reverse_agrees_number_and_tense() {
        let phon = Phonology::default();
        let morph = Morphology::from_hjson(
            r#"{
                kind: "agglutinative"
                morphemes: [ { id: "pl", gloss: "PL", form: "i", position: "suffix" } ]
                paradigms: [ { name: "noun", cells: [
                    { features: { number: "sg" }, morphemes: [] }
                    { features: { number: "pl" }, morphemes: ["pl"] }
                ] } ]
            }"#,
        )
        .unwrap()
        .unwrap();
        let entries = lexicon();
        // Singular subject → 3rd-singular verb.
        let r = reverse(&phon, &morph, &svo(), &entries, "kira nami pata");
        assert_eq!(r.english, "the bird sees the stone");
        // Plural subject + object (the -i forms) → bare verb, pluralized nouns.
        let r2 = reverse(&phon, &morph, &svo(), &entries, "kirai nami patai");
        assert_eq!(r2.english, "the birds see the stones");
    }

    #[test]
    fn unknown_conlang_word_is_marked() {
        let phon = Phonology::default();
        let morph = Morphology::default();
        let entries = lexicon();
        let r = reverse(&phon, &morph, &svo(), &entries, "kira nami zuxa");
        assert_eq!(r.unresolved, vec!["zuxa".to_string()]);
        assert!(r.english.contains("«zuxa»"));
    }

    #[test]
    fn cross_pivots_through_english() {
        let phon = Phonology::default();
        let morph = Morphology::default();
        // A: SVO, B: SOV — same concepts, different headwords + order.
        let a_entries = lexicon();
        let a_typ = svo();
        let mut b_typ = BTreeMap::new();
        b_typ.insert("word_order".to_string(), "sov".to_string());
        let b_entries = vec![
            entry("turi", "noun", "bird"),
            entry("vela", "verb", "to see"),
            entry("moki", "noun", "stone"),
        ];
        let from = LangCtx { phon: &phon, morph: &morph, typology: &a_typ, entries: &a_entries };
        let to = LangCtx { phon: &phon, morph: &morph, typology: &b_typ, entries: &b_entries };
        let c = cross(&from, &to, "kira nami pata");
        assert_eq!(c.english, "the bird sees the stone");
        // B is SOV: subject, object, verb.
        assert_eq!(c.target, "turi moki vela");
    }
}
