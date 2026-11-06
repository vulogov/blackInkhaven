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

/// One clause of a multi-sentence input (LB-7): its text, the terminator run
/// that closed it (`.`, `!`, `?` — possibly several, empty when none), and
/// the whitespace that followed it (`" "` or `"\n"`, so a paragraph's line
/// breaks survive; empty after the last clause).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClauseSpan {
    pub text: String,
    pub terminator: String,
    pub joiner: String,
}

fn is_terminator(c: char) -> bool {
    matches!(c, '.' | '!' | '?')
}

/// Split input into clauses on sentence terminators (`.`, `!`, `?`) and line
/// breaks, keeping each clause's own punctuation so a translation can be
/// re-joined in the source's shape. A `.` between two alphanumerics (`3.5`,
/// `e.g`) is not a terminator; punctuation with no text before it (`...` at
/// the start) is glued to the previous clause or dropped. Whitespace-only
/// input yields no clauses.
pub fn split_clauses(text: &str) -> Vec<ClauseSpan> {
    fn flush(out: &mut Vec<ClauseSpan>, cur: &mut String, terminator: String, joiner: String) {
        let t = cur.trim();
        if !t.is_empty() {
            out.push(ClauseSpan { text: t.to_string(), terminator, joiner });
        } else if let Some(last) = out.last_mut() {
            last.terminator.push_str(&terminator);
            if !joiner.is_empty() {
                last.joiner = joiner;
            }
        }
        cur.clear();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let inner_dot = c == '.'
            && i > 0
            && chars[i - 1].is_alphanumeric()
            && chars.get(i + 1).is_some_and(|n| n.is_alphanumeric());
        if is_terminator(c) && !inner_dot {
            let mut term = String::new();
            while i < chars.len() && is_terminator(chars[i]) {
                term.push(chars[i]);
                i += 1;
            }
            let mut joiner = String::new();
            while i < chars.len() && chars[i].is_whitespace() {
                if chars[i] == '\n' {
                    joiner = "\n".to_string();
                } else if joiner.is_empty() {
                    joiner = " ".to_string();
                }
                i += 1;
            }
            if i >= chars.len() {
                joiner.clear();
            }
            flush(&mut out, &mut cur, term, joiner);
        } else if c == '\n' {
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            let joiner = if i >= chars.len() { String::new() } else { "\n".to_string() };
            flush(&mut out, &mut cur, String::new(), joiner);
        } else {
            cur.push(c);
            i += 1;
        }
    }
    flush(&mut out, &mut cur, String::new(), String::new());
    out
}

/// Re-join per-clause renderings in the source's shape: each rendering,
/// then its clause's terminator and the whitespace that followed it.
pub fn join_clauses(rendered: &[String], spans: &[ClauseSpan]) -> String {
    let mut out = String::new();
    for (r, span) in rendered.iter().zip(spans) {
        out.push_str(r);
        out.push_str(&span.terminator);
        out.push_str(&span.joiner);
    }
    out.trim_end().to_string()
}

/// A rendered clause: surface, interlinear words, literal reading, and the
/// mean confidence of its constituents.
struct RenderedClause {
    surface: String,
    words: Vec<(String, String)>,
    literal: String,
    confidence: f32,
}

/// Translate one clause (no sentence terminators inside), appending its
/// decisions to `trace` / `unresolved`.
fn translate_clause(
    idx: &GlossIndex,
    phon: &Phonology,
    morph: &Morphology,
    typology: &BTreeMap<String, String>,
    text: &str,
    trace: &mut Vec<TraceEntry>,
    unresolved: &mut Vec<String>,
) -> RenderedClause {
    let parse = structure(idx, text);
    let first = trace.len();

    let subject = parse.subject.as_ref().map(|np| map_np(idx, np, "subject", trace, unresolved));
    let object = parse.object.as_ref().map(|np| map_np(idx, np, "object", trace, unresolved));

    let verb = parse.verb.as_ref().map(|v| {
        let (root, decision, conf) = resolve(idx, v, PosHint::Verb);
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
    let own = &trace[first..];
    if rendered.surface.trim().is_empty() || own.is_empty() {
        // Nothing the analyser could place (an interjection, a stray
        // function word): keep the clause visible and marked rather than
        // letting it vanish from the output.
        let marked = format!("«{text}»");
        unresolved.push(text.to_string());
        trace.push(TraceEntry {
            source: text.to_string(),
            role: "clause",
            target: marked.clone(),
            decision: Decision::Untranslatable,
            confidence: 0.2,
        });
        return RenderedClause {
            surface: marked.clone(),
            words: vec![(marked.clone(), text.to_string())],
            literal: marked,
            confidence: 0.2,
        };
    }
    let confidence = own.iter().map(|t| t.confidence).sum::<f32>() / own.len() as f32;
    RenderedClause {
        surface: rendered.surface,
        words: rendered.words,
        literal: rendered.literal,
        confidence,
    }
}

/// **Tier 1 (RBMT).** Translate English into the conlang deterministically,
/// reusing the LANG-1 syntax engine for ordering, case, inflection, and
/// agreement. `phon` / `morph` / `typology` describe the language; `entries`
/// are its dictionary.
///
/// Multi-sentence input is split on `.` / `!` / `?` and line breaks
/// ([`split_clauses`]); each clause is translated on its own and the
/// renderings are re-joined with the source punctuation. The reported
/// confidence is the **minimum** over the clauses, so one untranslatable
/// sentence is not averaged away by its neighbours (LB-7).
pub fn translate(
    phon: &Phonology,
    morph: &Morphology,
    typology: &BTreeMap<String, String>,
    entries: &[DictionaryEntry],
    text: &str,
) -> Translation {
    translate_with(&GlossIndex::build(entries), phon, morph, typology, text)
}

/// [`translate`] with a prebuilt gloss index (3.18 LI-2) — the
/// `LanguageModel`'s, or one built once for a batch of sentences.
pub fn translate_with(
    idx: &GlossIndex,
    phon: &Phonology,
    morph: &Morphology,
    typology: &BTreeMap<String, String>,
    text: &str,
) -> Translation {
    let perf = crate::store::perf_trace_enabled();
    let t0 = std::time::Instant::now();
    let spans = split_clauses(text);

    let mut trace: Vec<TraceEntry> = Vec::new();
    let mut unresolved: Vec<String> = Vec::new();
    let mut surfaces: Vec<String> = Vec::new();
    let mut literals: Vec<String> = Vec::new();
    let mut words: Vec<(String, String)> = Vec::new();
    let mut confidence: Option<f32> = None;

    for span in &spans {
        let r = translate_clause(idx, phon, morph, typology, &span.text, &mut trace, &mut unresolved);
        surfaces.push(r.surface);
        literals.push(r.literal);
        words.extend(r.words);
        confidence = Some(confidence.map_or(r.confidence, |c| c.min(r.confidence)));
    }
    crate::store::perf_mark(perf, "conlang.translate", t0.elapsed());

    Translation {
        source: text.to_string(),
        target: join_clauses(&surfaces, &spans),
        words,
        literal: join_clauses(&literals, &spans),
        confidence: confidence.unwrap_or(0.0),
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
    fn two_sentences_translate_in_order_with_their_punctuation() {
        let phon = Phonology::default();
        let morph = Morphology::default();
        let mut typ = BTreeMap::new();
        typ.insert("word_order".to_string(), "svo".to_string());
        let entries = lexicon();

        let t = translate(&phon, &morph, &typ, &entries, "the bird sees the stone. the stone sees the bird!");
        assert_eq!(t.target, "kira nami pata. pata nami kira!");
        assert_eq!(t.trace.len(), 6);
        assert!(t.unresolved.is_empty());
        assert!(t.confidence > 0.8);
        // A line break between sentences is kept as a line break.
        let t = translate(&phon, &morph, &typ, &entries, "the bird sees the stone\nthe stone sees the bird");
        assert_eq!(t.target, "kira nami pata\npata nami kira");
    }

    #[test]
    fn an_untranslatable_clause_is_kept_and_sets_the_confidence() {
        let phon = Phonology::default();
        let morph = Morphology::default();
        let mut typ = BTreeMap::new();
        typ.insert("word_order".to_string(), "svo".to_string());
        let entries = lexicon();

        // The first sentence used to be dropped and the confidence reported high.
        let t = translate(&phon, &morph, &typ, &entries, "the dragon. the bird sees the stone");
        assert_eq!(t.target, "«dragon». kira nami pata");
        assert_eq!(t.unresolved, vec!["dragon".to_string()]);
        // Minimum over clauses, not the mean: 0.2 for «dragon», 0.9 for the rest.
        assert!((t.confidence - 0.2).abs() < 1e-6, "{}", t.confidence);
        // A clause with nothing the analyser can place keeps its text, marked.
        let t = translate(&phon, &morph, &typ, &entries, "the. the bird sees the stone");
        assert_eq!(t.target, "«the». kira nami pata");
    }

    #[test]
    fn split_clauses_keeps_terminators_and_joiners() {
        let sp = split_clauses("the stone. the river flows");
        assert_eq!(sp.len(), 2);
        assert_eq!((sp[0].text.as_str(), sp[0].terminator.as_str(), sp[0].joiner.as_str()), ("the stone", ".", " "));
        assert_eq!((sp[1].text.as_str(), sp[1].terminator.as_str(), sp[1].joiner.as_str()), ("the river flows", "", ""));
        // Runs of punctuation, trailing punctuation, and newlines.
        let sp = split_clauses("wait... what?!\nnothing.");
        let flat: Vec<(&str, &str, &str)> = sp.iter().map(|s| (s.text.as_str(), s.terminator.as_str(), s.joiner.as_str())).collect();
        assert_eq!(flat, vec![("wait", "...", " "), ("what", "?!", "\n"), ("nothing", ".", "")]);
        // A decimal point is not a terminator; blank input has no clauses.
        assert_eq!(split_clauses("3.5 birds").len(), 1);
        assert!(split_clauses("  \n ").is_empty());
        // Leading punctuation with no text is dropped.
        assert_eq!(split_clauses("... the bird")[0].text, "the bird");
        assert_eq!(join_clauses(&["a".into(), "b".into()], &split_clauses("x. y")), "a. b");
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
