//! LANG-3 — translation (RFC `Documentation/PROPOSALS/LANG-3_PLAN.md`).
//!
//! The three-tier translation stack. This module currently implements **Tier 1**
//! — the pure-Rust, deterministic *rule-based* spine (RBMT) — which is the only
//! tier that needs no downloaded model and works fully offline. The neural tiers
//! (per-language fine-tuned NMT, optional tiny-LLM resolver) and the routing /
//! merge layer that chooses between them arrive in later phases; the [`Tier`]
//! enum already names them so the public surface is stable.
//!
//! **Why RBMT is feasible for a conlang** (and not for an arbitrary natural
//! language): a LANG-1 language carries a complete formal description — a
//! phoneme inventory, a morphology spec, typological grammar tags, and a lexicon
//! whose every headword records an English gloss. So translation is mostly
//! *orchestration* over the LANG-1 engines:
//!
//! 1. [`english::analyze`] recovers a simple clause from the English source.
//! 2. [`lexmap`] maps each English lemma to a conlang headword by its gloss.
//! 3. [`crate::conlang::syntax::assemble`] does the heavy lifting already built
//!    for LANG-1 — ordering by `word_order`, case-marking by `alignment`,
//!    inflecting via the morphology spec, running agreement, and applying
//!    allophony — so the translator reuses it wholesale rather than reimplements
//!    word order, case, and inflection.
//!
//! Every output carries a per-constituent [`TraceEntry`] (which lexicon entry,
//! what decision, what confidence) and a list of any English words the lexicon
//! could not cover, so the result is always inspectable and honest.

pub mod english;
pub mod lexmap;
pub mod reverse;

use std::collections::BTreeMap;

use crate::conlang::syntax::{self, Clause, NounPhrase, Word};
use crate::conlang::types::morphology::Morphology;
use crate::conlang::Phonology;
use crate::language_entry::DictionaryEntry;

use english::EnglishNp;
use lexmap::{GlossIndex, Mapping, PosHint};

/// Which tier produced a translation. Only Tier 1 (the rule-based spine) exists
/// today; the neural tiers (`Nmt`, a per-language fine-tuned model) and the
/// optional tiny-LLM `Resolver` get their variants when those phases land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// The rule-based spine (this module).
    Rbmt,
}

impl Tier {
    /// A short human label.
    pub fn label(self) -> &'static str {
        match self {
            Tier::Rbmt => "Tier 1 RBMT",
        }
    }
}

/// The decision that produced one target constituent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Mapped to a lexicon headword by its gloss.
    LexiconLookup { word: String, pos: String },
    /// No lexicon entry carries this meaning; the source word was passed through
    /// marked (`«word»`) and listed in [`Translation::unresolved`].
    Untranslatable,
}

/// A per-constituent record of how the source became the target.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceEntry {
    /// The English lemma.
    pub source: String,
    /// The constituent's role: `"subject"`, `"verb"`, or `"object"`.
    pub role: &'static str,
    /// The conlang root chosen (before inflection).
    pub target: String,
    pub decision: Decision,
    /// `0.0..=1.0`.
    pub confidence: f32,
}

/// A finished translation.
#[derive(Debug, Clone)]
pub struct Translation {
    /// The English source.
    pub source: String,
    /// The conlang surface string.
    pub target: String,
    /// `(surface, gloss)` for each rendered word, for an interlinear display.
    pub words: Vec<(String, String)>,
    /// A literal back-rendering in subject–verb–object order.
    pub literal: String,
    /// Aggregate confidence over the content words, `0.0..=1.0`.
    pub confidence: f32,
    /// The per-constituent decision trace.
    pub trace: Vec<TraceEntry>,
    /// English content words the lexicon could not cover (candidates for
    /// coining or for `add-word`).
    pub unresolved: Vec<String>,
    /// Which tier produced this.
    pub tier: Tier,
}

/// Resolve one English lemma to a conlang root, returning the root, the
/// decision, and a confidence. An unmapped word is marked `«lemma»`.
fn resolve(idx: &GlossIndex, lemma: &str, hint: PosHint) -> (String, Decision, f32) {
    match idx.map(lemma, hint) {
        Mapping::Found { word, pos } => {
            let decision = Decision::LexiconLookup { word: word.clone(), pos };
            (word, decision, 0.9)
        }
        Mapping::Missing => (format!("«{lemma}»"), Decision::Untranslatable, 0.2),
    }
}

/// Build a conlang [`NounPhrase`] from an English NP, recording the trace.
fn map_np(
    idx: &GlossIndex,
    np: &EnglishNp,
    role: &'static str,
    trace: &mut Vec<TraceEntry>,
    unresolved: &mut Vec<String>,
) -> NounPhrase {
    let (root, decision, conf) = resolve(idx, &np.head, PosHint::Noun);
    if matches!(decision, Decision::Untranslatable) {
        unresolved.push(np.head.clone());
    }
    trace.push(TraceEntry {
        source: np.head.clone(),
        role,
        target: root.clone(),
        decision,
        confidence: conf,
    });
    NounPhrase {
        head: Word { root, gloss: np.head.clone() },
        number: np.number.clone(),
        adjective: None,
    }
}

/// **Tier 1 (RBMT).** Translate English into the conlang deterministically,
/// reusing the LANG-1 syntax engine for ordering, case, inflection, and
/// agreement. `phon` / `morph` / `typology` describe the language; `entries`
/// are its dictionary.
pub fn translate(
    phon: &Phonology,
    morph: &Morphology,
    typology: &BTreeMap<String, String>,
    entries: &[DictionaryEntry],
    text: &str,
) -> Translation {
    let idx = GlossIndex::build(entries);
    let parse = english::analyze(text);

    let mut trace: Vec<TraceEntry> = Vec::new();
    let mut unresolved: Vec<String> = Vec::new();

    let subject = parse.subject.as_ref().map(|np| map_np(&idx, np, "subject", &mut trace, &mut unresolved));
    let object = parse.object.as_ref().map(|np| map_np(&idx, np, "object", &mut trace, &mut unresolved));

    let verb = parse.verb.as_ref().map(|v| {
        let (root, decision, conf) = resolve(&idx, v, PosHint::Verb);
        if matches!(decision, Decision::Untranslatable) {
            unresolved.push(v.clone());
        }
        trace.push(TraceEntry {
            source: v.clone(),
            role: "verb",
            target: root.clone(),
            decision,
            confidence: conf,
        });
        Word { root, gloss: v.clone() }
    });

    let clause = Clause {
        subject,
        verb,
        verb_person: parse.verb_person.clone(),
        object,
        noun_paradigm: "noun".into(),
        verb_paradigm: "verb".into(),
        ..Default::default()
    };

    let rendered = syntax::assemble(phon, morph, typology, &clause);

    let confidence = if trace.is_empty() {
        0.0
    } else {
        trace.iter().map(|t| t.confidence).sum::<f32>() / trace.len() as f32
    };

    Translation {
        source: text.to_string(),
        target: rendered.surface,
        words: rendered.words,
        literal: rendered.literal,
        confidence,
        trace,
        unresolved,
        tier: Tier::Rbmt,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::types::morphology::Morphology;

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

    #[test]
    fn translates_a_simple_svo_sentence() {
        let phon = Phonology::default();
        let morph = Morphology::default();
        let mut typ = BTreeMap::new();
        typ.insert("word_order".to_string(), "svo".to_string());
        let entries = lexicon();

        let t = translate(&phon, &morph, &typ, &entries, "the bird sees the stone");
        // All three content words resolved.
        assert!(t.unresolved.is_empty());
        assert_eq!(t.trace.len(), 3);
        // SVO order: the conlang roots appear subject, verb, object.
        assert_eq!(t.target, "kira nami pata");
        assert!(t.confidence > 0.8);
    }

    #[test]
    fn sov_order_is_respected() {
        let phon = Phonology::default();
        let morph = Morphology::default();
        let mut typ = BTreeMap::new();
        typ.insert("word_order".to_string(), "sov".to_string());
        let entries = lexicon();

        let t = translate(&phon, &morph, &typ, &entries, "the bird sees the stone");
        // SOV: subject, object, verb.
        assert_eq!(t.target, "kira pata nami");
    }

    #[test]
    fn unresolved_words_are_marked_and_listed() {
        let phon = Phonology::default();
        let morph = Morphology::default();
        let typ = BTreeMap::new();
        let entries = lexicon();

        let t = translate(&phon, &morph, &typ, &entries, "the dragon sees the stone");
        assert_eq!(t.unresolved, vec!["dragon".to_string()]);
        assert!(t.target.contains("«dragon»"));
        // Confidence drops with an untranslatable word.
        assert!(t.confidence < 0.8);
    }
}
