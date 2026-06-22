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

pub mod corpus;
pub mod english;
pub mod eval;
pub mod export;
pub mod lexmap;
pub mod memory;
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

/// An alternative rendering the author can choose instead of the primary — a
/// rule-based output displaced by a translation-memory hit, or a near-match
/// retrieved from memory. The merge layer never silently picks; it surfaces
/// these.
#[derive(Debug, Clone, PartialEq)]
pub struct Alternative {
    /// The alternative conlang surface.
    pub text: String,
    /// Where it came from (`"rbmt"` / `"translation-memory"`).
    pub source: &'static str,
    /// `0.0..=1.0`.
    pub confidence: f32,
    /// Why it is offered.
    pub rationale: String,
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
    /// Alternative renderings the author may pick instead (from the translation
    /// memory). Empty unless the merge layer surfaced any.
    pub alternatives: Vec<Alternative>,
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

/// Lexicon-aware structuring: classify each prepared token by the lexicon's
/// parts of speech. The English source is SVO, so the subject precedes the verb
/// and the object follows it; within each region the last word is the head noun
/// and a preceding word the lexicon knows as an adjective is recovered as one.
/// Falls back to the positional [`english::analyze`] when the verb is not in the
/// lexicon (so an all-unknown sentence still produces a best-effort parse).
fn structure(idx: &GlossIndex, text: &str) -> english::EnglishClause {
    use english::{EnglishClause, EnglishNp};
    let prep = english::prepare(text);
    let toks = &prep.tokens;

    // The verb is the first token carrying a verb sense (the subject precedes it).
    let verb_idx = toks.iter().position(|t| {
        idx.has_sense(&english::delemmatize_verb(t), PosHint::Verb)
            || idx.has_sense(t, PosHint::Verb)
    });
    let Some(vi) = verb_idx else {
        return english::analyze(text);
    };

    let build_np = |region: &[String]| -> Option<EnglishNp> {
        let head = region.last()?;
        let (lemma, plural) = english::depluralize(head);
        let number = if plural { "pl".to_string() } else { "sg".to_string() };
        // An adjective is the head's immediate predecessor when the lexicon
        // knows it as one.
        let adjective = if region.len() >= 2 {
            let cand = &region[region.len() - 2];
            idx.has_sense(cand, PosHint::Adjective).then(|| cand.clone())
        } else {
            None
        };
        Some(EnglishNp { head: lemma, number, adjective })
    };

    let subject = if prep.pronoun_subject { None } else { build_np(&toks[..vi]) };
    let object = build_np(&toks[vi + 1..]);
    EnglishClause {
        subject,
        verb: Some(english::delemmatize_verb(&toks[vi])),
        verb_person: prep.person,
        object,
    }
}

/// Build a conlang [`NounPhrase`] from an English NP, resolving its head and any
/// adjective and recording the trace.
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

    let adjective = np.adjective.as_ref().map(|adj| {
        let (aroot, adecision, aconf) = resolve(idx, adj, PosHint::Adjective);
        if matches!(adecision, Decision::Untranslatable) {
            unresolved.push(adj.clone());
        }
        trace.push(TraceEntry {
            source: adj.clone(),
            role: "adjective",
            target: aroot.clone(),
            decision: adecision,
            confidence: aconf,
        });
        Word { root: aroot, gloss: adj.clone() }
    });

    NounPhrase {
        head: Word { root, gloss: np.head.clone() },
        number: np.number.clone(),
        adjective,
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
    let parse = structure(&idx, text);

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
        alternatives: Vec::new(),
        tier: Tier::Rbmt,
    }
}

/// **Tier 2 (retrieval).** Layer a translation memory over a rule-based
/// translation: an *exact* remembered translation (an author-confirmed pair)
/// replaces the rule-based primary, which is kept as an alternative; a *near*
/// match (semantic when `query_embedding` is supplied and the memory is
/// embedded, else lexical) is surfaced as an alternative for the author to pick.
/// A miss leaves the rule-based output untouched. The memory never silently
/// overrides without keeping the rule-based reading visible — and the retrieval
/// strategy is entirely inside [`memory::TranslationMemory::best`], so this merge
/// policy is unchanged whether the match was lexical or semantic.
pub fn apply_memory(
    mut t: Translation,
    mem: &memory::TranslationMemory,
    query_embedding: Option<&[f32]>,
) -> Translation {
    match mem.best(&t.source, query_embedding) {
        memory::MemoryHit::Exact { conlang } => {
            if conlang != t.target {
                t.alternatives.insert(
                    0,
                    Alternative {
                        text: t.target.clone(),
                        source: "rbmt",
                        confidence: t.confidence,
                        rationale: "rule-based".to_string(),
                    },
                );
                t.target = conlang;
            }
            t.confidence = t.confidence.max(0.99);
        }
        memory::MemoryHit::Fuzzy { conlang, score, english } => {
            t.alternatives.push(Alternative {
                text: conlang,
                source: "translation-memory",
                confidence: score,
                rationale: format!("translation memory · {:.0}% match to \"{english}\"", score * 100.0),
            });
        }
        memory::MemoryHit::None => {}
    }
    t
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
    fn recovers_and_places_an_adjective() {
        let phon = Phonology::default();
        let morph = Morphology::default();
        let mut typ = BTreeMap::new();
        typ.insert("word_order".to_string(), "svo".to_string());
        let mut entries = lexicon();
        entries.push(entry("mira", "adjective", "bright"));

        let t = translate(&phon, &morph, &typ, &entries, "the bright bird sees the stone");
        assert!(t.unresolved.is_empty(), "all words resolved: {:?}", t.unresolved);
        // Adjective before noun (default), then verb, then object.
        assert_eq!(t.target, "mira kira nami pata");
        // Four constituents traced: subject head, its adjective, verb, object.
        assert_eq!(t.trace.len(), 4);
    }

    #[test]
    fn memory_overrides_with_rbmt_as_alternative() {
        let phon = Phonology::default();
        let morph = Morphology::default();
        let mut typ = BTreeMap::new();
        typ.insert("word_order".to_string(), "svo".to_string());
        let entries = lexicon();

        let t = translate(&phon, &morph, &typ, &entries, "the bird sees the stone");
        assert_eq!(t.target, "kira nami pata");

        // An author-confirmed correction in the memory.
        let mut mem = memory::TranslationMemory::default();
        mem.add("the bird sees the stone", "kira pata-corrected nami");
        let t2 = apply_memory(t, &mem, None);
        assert_eq!(t2.target, "kira pata-corrected nami");
        // The rule-based reading is kept as an alternative, not discarded.
        assert_eq!(t2.alternatives.len(), 1);
        assert_eq!(t2.alternatives[0].text, "kira nami pata");
        assert!(t2.confidence > 0.98);
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
