//! 3.18 LI-2 — the in-memory model of one language book.
//!
//! Before this, every `language` command, every Bund `ink.lang.*` word, the
//! Oracle scans and the TUI translate built Phonology, Morphology, the
//! typology, the dictionary and the rest from the store on every call: a
//! chapter walk plus one `get_content` round trip per paragraph, and the
//! gloss / reverse / root indexes rebuilt per call (per *word* for the
//! Oracle). [`LanguageModel`] reads the whole book once — one batched blob
//! fetch — parses every block once, runs the phonology validator once, and
//! builds its indexes lazily on first use.
//!
//! [`cached`] keeps one model per language book for the process and
//! revalidates it against a [`ModelStamp`] computed from the hierarchy the
//! caller already holds (the subtree's node count and latest `modified_at`),
//! so a script looping over `ink.lang.translate` loads once, the TUI's
//! on-save Oracle scan reloads only after the language book itself changed,
//! and a CLI command that asks for the phonology, the morphology and the
//! dictionary pays one load instead of three.
//!
//! The parsing rules are exactly the ones the former per-block loaders in
//! `cli::language` applied (which chapters are searched, which block wins,
//! what is merged); those loaders are now thin views over this model.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::conlang::parse::Analyzer;
use crate::conlang::translate::lexmap::GlossIndex;
use crate::conlang::translate::reverse::ReverseIndex;
use crate::conlang::types::contact::{Contact, LoanPhonology};
use crate::conlang::types::diachronic::Diachronics;
use crate::conlang::types::grammar::GrammarSpec;
use crate::conlang::types::morphology::Morphology;
use crate::conlang::types::validate::Finding;
use crate::conlang::types::variety::Varieties;
use crate::conlang::Phonology;
use crate::error::Result;
use crate::language_entry::{DictionaryEntry, MetaOverview};
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};
use crate::store::Store;

/// What a cached model was built from: the language book subtree's node
/// count and its latest `modified_at`. Every store write to the book bumps
/// one of the two (creates and content updates stamp `modified_at`; deletes
/// change the count), and both come from the hierarchy the caller already
/// holds, so revalidation costs no store access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelStamp {
    pub nodes: usize,
    pub latest: DateTime<Utc>,
}

impl ModelStamp {
    /// The stamp of `book_id`'s subtree in `hierarchy`.
    pub fn of(hierarchy: &Hierarchy, book_id: Uuid) -> Self {
        let mut nodes = 0usize;
        let mut latest = DateTime::<Utc>::MIN_UTC;
        for id in hierarchy.collect_subtree(book_id) {
            if let Some(n) = hierarchy.get(id) {
                nodes += 1;
                if n.modified_at > latest {
                    latest = n.modified_at;
                }
            }
        }
        Self { nodes, latest }
    }
}

/// One language book, parsed.
pub struct LanguageModel {
    /// What this model was built from (see [`ModelStamp`]).
    pub stamp: ModelStamp,
    /// The `Meta/overview` block, when present and parseable.
    pub overview: Option<MetaOverview>,
    /// The first non-empty phoneme block under the `Phonology` chapter.
    pub phonology: Option<Phonology>,
    /// Title of the paragraph that holds the phonology block.
    pub phonology_title: String,
    /// A malformed HJSON block under `Phonology` (`Phonology/<title>: <error>`).
    /// The CLI loader surfaces this as a hard error, as it always did.
    pub phonology_error: Option<String>,
    /// `Phonology::validate()` over the loaded block, run once.
    pub findings: Vec<Finding>,
    /// The first non-empty morphology block under `Morphology` or `Grammar`.
    pub morphology: Option<Morphology>,
    /// The `{ grammar: … }` typology block under `Grammar` (default when
    /// absent) and the paragraph that holds it (for in-place edits).
    pub grammar: GrammarSpec,
    pub grammar_node: Option<Node>,
    /// Every `{ varieties: [ … ] }` block under `Grammar`, merged by id.
    pub varieties: Varieties,
    /// The first `{ contact: … }` block under `Grammar`.
    pub contact: Option<Contact>,
    /// The first `{ loan_phonology: … }` block under `Phonology`, or defaults.
    pub loan_phonology: LoanPhonology,
    /// The first `{ diachronics: … }` block under `Phonology`.
    pub diachronics: Option<Diachronics>,
    /// Every parseable entry under `Dictionary`, in tree order.
    pub entries: Vec<DictionaryEntry>,
    /// Every stored interlinear under `Texts`, as `(title, igt)`.
    pub texts: Vec<(String, crate::conlang::igt::Igt)>,
    /// Every stored hypothesis under `Hypotheses`.
    pub hypotheses: Vec<crate::conlang::hypothesis::Hypothesis>,

    default_phonology: Phonology,
    default_morphology: Morphology,
    gloss: OnceLock<GlossIndex>,
    reverse: OnceLock<ReverseIndex>,
    analyzer: OnceLock<Analyzer>,
}

impl LanguageModel {
    /// Read `book` from `store`: one batched fetch of every paragraph body in
    /// the book's subtree, then [`Self::build`].
    pub fn load(store: &Store, hierarchy: &Hierarchy, book: &Node) -> Result<Self> {
        let perf = crate::store::perf_trace_enabled();
        let t0 = std::time::Instant::now();
        let ids: Vec<Uuid> = hierarchy
            .collect_subtree(book.id)
            .into_iter()
            .filter(|id| hierarchy.get(*id).map(|n| n.kind == NodeKind::Paragraph).unwrap_or(false))
            .collect();
        let contents = store.get_content_batch(&ids)?;
        crate::store::perf_mark(perf, "conlang.model.fetch", t0.elapsed());
        let t1 = std::time::Instant::now();
        let model = Self::build(hierarchy, book, &contents);
        crate::store::perf_mark(perf, "conlang.model.parse", t1.elapsed());
        Ok(model)
    }

    /// Parse a language book from its hierarchy and already-fetched bodies.
    /// Pure: no store access, so the parsing rules are unit-testable.
    pub fn build(hierarchy: &Hierarchy, book: &Node, contents: &HashMap<Uuid, Vec<u8>>) -> Self {
        let body = |id: Uuid| -> Option<String> {
            contents.get(&id).map(|b| String::from_utf8_lossy(b).into_owned())
        };
        let chapters = |title: &str| -> Vec<&Node> {
            hierarchy
                .children_of(Some(book.id))
                .into_iter()
                .filter(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case(title))
                .collect()
        };
        let paragraphs_of = |chapter: &Node| -> Vec<&Node> {
            hierarchy
                .children_of(Some(chapter.id))
                .into_iter()
                .filter(|n| n.kind == NodeKind::Paragraph)
                .collect()
        };
        let subtree_paragraphs = |chapter: &Node| -> Vec<&Node> {
            hierarchy
                .collect_subtree(chapter.id)
                .into_iter()
                .filter_map(|id| hierarchy.get(id))
                .filter(|n| n.kind == NodeKind::Paragraph)
                .collect()
        };

        // Meta/overview.
        let overview = chapters("Meta")
            .first()
            .and_then(|meta| {
                paragraphs_of(meta).into_iter().find(|n| n.title.eq_ignore_ascii_case("overview"))
            })
            .and_then(|n| body(n.id))
            .and_then(|b| crate::language_entry::parse_meta_overview(&b).ok().flatten());

        // Phonology: the first block with phonemes wins; a malformed block
        // under the chapter is an error, as `load_phonology` always treated it.
        let mut phonology = None;
        let mut phonology_title = String::new();
        let mut phonology_error = None;
        let phon_chapters = chapters("Phonology");
        if let Some(chapter) = phon_chapters.first() {
            for para in paragraphs_of(chapter) {
                let Some(b) = body(para.id) else { continue };
                match Phonology::from_hjson(&b) {
                    Ok(Some(p)) if !p.phonemes.is_empty() => {
                        phonology = Some(p);
                        phonology_title = para.title.clone();
                        break;
                    }
                    Ok(_) => continue,
                    Err(e) => {
                        phonology_error = Some(format!("Phonology/{}: {e}", para.title));
                        break;
                    }
                }
            }
        }
        let findings = phonology.as_ref().map(|p| p.validate()).unwrap_or_default();

        // Loan phonology + diachronics live under Phonology too (every
        // Phonology chapter for the former, the first for the latter —
        // mirroring the loaders they replace).
        let mut loan_phonology = None;
        for chapter in &phon_chapters {
            for para in paragraphs_of(chapter) {
                let Some(b) = body(para.id) else { continue };
                if let Ok(Some(lp)) = LoanPhonology::from_hjson(&b) {
                    loan_phonology = Some(lp);
                    break;
                }
            }
            if loan_phonology.is_some() {
                break;
            }
        }
        let mut diachronics = None;
        if let Some(chapter) = phon_chapters.first() {
            for para in paragraphs_of(chapter) {
                let Some(b) = body(para.id) else { continue };
                if let Ok(Some(d)) = Diachronics::from_hjson(&b) {
                    diachronics = Some(d);
                    break;
                }
            }
        }

        // Morphology: the 1.2.13 scaffold has no Morphology chapter, so the
        // block may live under Grammar (or a hand-added Morphology chapter).
        let mut morphology = None;
        'morph: for chapter in hierarchy.children_of(Some(book.id)).into_iter().filter(|n| {
            n.kind == NodeKind::Chapter
                && (n.title.eq_ignore_ascii_case("Morphology") || n.title.eq_ignore_ascii_case("Grammar"))
        }) {
            for para in paragraphs_of(chapter) {
                let Some(b) = body(para.id) else { continue };
                if let Ok(Some(m)) = Morphology::from_hjson(&b) {
                    if !m.morphemes.is_empty() || !m.paradigms.is_empty() || !m.derivations.is_empty() {
                        morphology = Some(m);
                        break 'morph;
                    }
                }
            }
        }

        // Grammar chapter: typology spec (+ its node), varieties (merged
        // across every Grammar chapter), contact (first).
        let grammar_chapters = chapters("Grammar");
        let mut grammar = GrammarSpec::default();
        let mut grammar_node = None;
        if let Some(chapter) = grammar_chapters.first() {
            for para in paragraphs_of(chapter) {
                let Some(b) = body(para.id) else { continue };
                if let Ok(Some(spec)) = GrammarSpec::from_hjson(&b) {
                    grammar = spec;
                    grammar_node = Some((*para).clone());
                    break;
                }
            }
        }
        let mut varieties = Varieties::default();
        let mut contact = None;
        for chapter in &grammar_chapters {
            for para in paragraphs_of(chapter) {
                let Some(b) = body(para.id) else { continue };
                if let Ok(Some(v)) = Varieties::from_hjson(&b) {
                    for var in v.varieties {
                        if !varieties.varieties.iter().any(|x| x.id.eq_ignore_ascii_case(&var.id)) {
                            varieties.varieties.push(var);
                        }
                    }
                }
                if contact.is_none() {
                    if let Ok(Some(c)) = Contact::from_hjson(&b) {
                        contact = Some(c);
                    }
                }
            }
        }

        // Dictionary, Texts, Hypotheses: whole subtrees, tree order.
        let entries = chapters("Dictionary")
            .first()
            .map(|ch| {
                subtree_paragraphs(ch)
                    .into_iter()
                    .filter_map(|n| body(n.id))
                    .filter_map(|b| crate::language_entry::parse(&b).ok().flatten())
                    .collect()
            })
            .unwrap_or_default();
        let texts = chapters("Texts")
            .first()
            .map(|ch| {
                subtree_paragraphs(ch)
                    .into_iter()
                    .filter_map(|n| body(n.id).map(|b| (n.title.clone(), b)))
                    .filter_map(|(t, b)| {
                        serde_json::from_str::<crate::conlang::igt::Igt>(&b).ok().map(|igt| (t, igt))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let hypotheses = chapters("Hypotheses")
            .first()
            .map(|ch| {
                subtree_paragraphs(ch)
                    .into_iter()
                    .filter_map(|n| body(n.id))
                    .filter_map(|b| serde_json::from_str::<crate::conlang::hypothesis::Hypothesis>(&b).ok())
                    .collect()
            })
            .unwrap_or_default();

        Self {
            stamp: ModelStamp::of(hierarchy, book.id),
            overview,
            phonology,
            phonology_title,
            phonology_error,
            findings,
            morphology,
            grammar,
            grammar_node,
            varieties,
            contact,
            loan_phonology: loan_phonology.unwrap_or_default(),
            diachronics,
            entries,
            texts,
            hypotheses,
            default_phonology: Phonology::default(),
            default_morphology: Morphology::default(),
            gloss: OnceLock::new(),
            reverse: OnceLock::new(),
            analyzer: OnceLock::new(),
        }
    }

    /// The phonology, or an empty one when the book declares none — what
    /// callers that `unwrap_or_default()` the loader result get, by reference.
    pub fn phon(&self) -> &Phonology {
        self.phonology.as_ref().unwrap_or(&self.default_phonology)
    }

    /// The morphology, or an empty one when the book declares none.
    pub fn morph(&self) -> &Morphology {
        self.morphology.as_ref().unwrap_or(&self.default_morphology)
    }

    /// The typology map (`grammar.grammar`).
    pub fn typology(&self) -> &std::collections::BTreeMap<String, String> {
        &self.grammar.grammar
    }

    /// English-gloss → headword index, built on first use.
    pub fn gloss(&self) -> &GlossIndex {
        self.gloss.get_or_init(|| GlossIndex::build(&self.entries))
    }

    /// Surface-form → gloss index (paradigm-expanded), built on first use.
    pub fn reverse_index(&self) -> &ReverseIndex {
        self.reverse.get_or_init(|| ReverseIndex::build(self.phon(), self.morph(), &self.entries))
    }

    /// The morphological analyzer (root map, affixes, ablaut images), built
    /// on first use. Drives the Oracle and the parser.
    pub fn analyzer(&self) -> &Analyzer {
        self.analyzer.get_or_init(|| Analyzer::new(self.phon(), self.morph(), &self.entries))
    }
}

// ── the per-process cache ─────────────────────────────────────────────────

static CACHE: OnceLock<Mutex<HashMap<Uuid, Arc<LanguageModel>>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<Uuid, Arc<LanguageModel>>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The model of `book`, loaded on the first call and reused while the
/// book's [`ModelStamp`] (from `hierarchy`) is unchanged. The flag says
/// whether this call loaded it — the CLI prints the phonology findings only
/// then, so a command that asks twice warns once.
pub fn cached(store: &Store, hierarchy: &Hierarchy, book: &Node) -> Result<(Arc<LanguageModel>, bool)> {
    let stamp = ModelStamp::of(hierarchy, book.id);
    lookup(cache(), book.id, stamp, || LanguageModel::load(store, hierarchy, book))
}

/// The cache decision, separated from the process global so it can be tested:
/// a hit needs an entry under `book_id` whose stamp equals `stamp`.
fn lookup(
    map: &Mutex<HashMap<Uuid, Arc<LanguageModel>>>,
    book_id: Uuid,
    stamp: ModelStamp,
    load: impl FnOnce() -> Result<LanguageModel>,
) -> Result<(Arc<LanguageModel>, bool)> {
    if let Ok(guard) = map.lock() {
        if let Some(m) = guard.get(&book_id) {
            if m.stamp == stamp {
                return Ok((Arc::clone(m), false));
            }
        }
    }
    let model = Arc::new(load()?);
    if let Ok(mut guard) = map.lock() {
        guard.insert(book_id, Arc::clone(&model));
    }
    Ok((model, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: Uuid, kind: &str, title: &str, parent: Option<Uuid>, order: u32, modified: &str) -> Node {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "kind": kind,
            "title": title,
            "slug": title.to_lowercase().replace(' ', "-"),
            "path": [],
            "parent_id": parent,
            "order": order,
            "file": null,
            "modified_at": modified,
        }))
        .expect("test node deserialises")
    }

    const PHON: &str = r#"{
        phonemes: [
            { ipa: "k", kind: "consonant" }, { ipa: "t", kind: "consonant" },
            { ipa: "a", kind: "vowel" }, { ipa: "i", kind: "vowel" }
        ],
        classes: { C: ["k", "t"], V: ["a", "i"] },
        constraints: [ { kind: "forbid_in_coda", classes: ["NoSuchClass"] } ]
    }"#;
    const MORPH: &str = r#"{ morphemes: [ { id: "pl", gloss: "PL", form: "i", position: "suffix" } ] }"#;
    const GRAMMAR: &str = r#"{ grammar: { word_order: "sov" } }"#;
    const VAR_A: &str = r#"{ varieties: [ { id: "north", name: "Northern" } ] }"#;
    const VAR_B: &str = r#"{ varieties: [ { id: "North", name: "dup" }, { id: "south", name: "Southern" } ] }"#;
    const ENTRY_KATA: &str = r#"{ word: "kata", type: "noun", translation: "stone" }"#;
    const ENTRY_TIKA: &str = r#"{ word: "tika", type: "verb", translation: "to see" }"#;

    struct Fixture {
        hierarchy: Hierarchy,
        book: Node,
        contents: HashMap<Uuid, Vec<u8>>,
        phon_para: Uuid,
        grammar_para: Uuid,
    }

    /// Book → Phonology/inventory, Grammar/typology + Grammar/varieties ×2 +
    /// Grammar/morphology, Dictionary/K/kata + Dictionary/T/tika.
    fn fixture(phon_body: &str) -> Fixture {
        let t = "2026-01-01T00:00:00Z";
        let book = Uuid::now_v7();
        let (phon, gram, dict) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        let (p_inv, p_typ, p_var1, p_var2, p_morph) =
            (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        let (k, t_, e_kata, e_tika) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        let nodes = vec![
            node(book, "book", "Avesha", None, 1, t),
            node(phon, "chapter", "Phonology", Some(book), 1, t),
            node(p_inv, "paragraph", "inventory", Some(phon), 1, t),
            node(gram, "chapter", "Grammar", Some(book), 2, t),
            node(p_typ, "paragraph", "typology", Some(gram), 1, t),
            node(p_var1, "paragraph", "varieties", Some(gram), 2, t),
            node(p_var2, "paragraph", "more varieties", Some(gram), 3, t),
            node(p_morph, "paragraph", "morphology", Some(gram), 4, t),
            node(dict, "chapter", "Dictionary", Some(book), 3, t),
            node(k, "subchapter", "K", Some(dict), 1, t),
            node(e_kata, "paragraph", "kata", Some(k), 1, t),
            node(t_, "subchapter", "T", Some(dict), 2, t),
            node(e_tika, "paragraph", "tika", Some(t_), 1, t),
        ];
        let book_node = nodes[0].clone();
        let mut contents = HashMap::new();
        contents.insert(p_inv, phon_body.as_bytes().to_vec());
        contents.insert(p_typ, GRAMMAR.as_bytes().to_vec());
        contents.insert(p_var1, VAR_A.as_bytes().to_vec());
        contents.insert(p_var2, VAR_B.as_bytes().to_vec());
        contents.insert(p_morph, MORPH.as_bytes().to_vec());
        contents.insert(e_kata, ENTRY_KATA.as_bytes().to_vec());
        contents.insert(e_tika, ENTRY_TIKA.as_bytes().to_vec());
        Fixture {
            hierarchy: Hierarchy::from_nodes_for_test(nodes),
            book: book_node,
            contents,
            phon_para: p_inv,
            grammar_para: p_typ,
        }
    }

    #[test]
    fn build_parses_every_block_with_the_loaders_rules() {
        let f = fixture(PHON);
        let m = LanguageModel::build(&f.hierarchy, &f.book, &f.contents);
        assert_eq!(m.phon().phonemes.len(), 4);
        assert_eq!(m.phonology_title, "inventory");
        assert!(m.phonology_error.is_none());
        // validate() ran once: the undeclared class is reported.
        assert!(m.findings.iter().any(|x| x.message.contains("NoSuchClass")), "{:?}", m.findings);
        // morphology found under Grammar (no Morphology chapter).
        assert_eq!(m.morph().morphemes.len(), 1);
        assert_eq!(m.typology().get("word_order").map(String::as_str), Some("sov"));
        assert_eq!(m.grammar_node.as_ref().map(|n| n.id), Some(f.grammar_para));
        // varieties merged across paragraphs, ids deduplicated case-insensitively.
        let ids: Vec<&str> = m.varieties.varieties.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(ids, ["north", "south"]);
        assert!(m.contact.is_none());
        assert!(m.diachronics.is_none());
        // dictionary across buckets, tree order.
        let words: Vec<&str> = m.entries.iter().map(|e| e.word.as_str()).collect();
        assert_eq!(words, ["kata", "tika"]);
        assert!(m.texts.is_empty() && m.hypotheses.is_empty());
    }

    #[test]
    fn a_malformed_phonology_block_is_reported_not_swallowed() {
        let f = fixture("{ phonemes: [ { ipa: \"k\", kind: \"consonant\" } ");
        let m = LanguageModel::build(&f.hierarchy, &f.book, &f.contents);
        assert!(m.phonology.is_none());
        let err = m.phonology_error.expect("malformed block is an error");
        assert!(err.starts_with("Phonology/inventory: "), "{err}");
    }

    #[test]
    fn indexes_build_once_and_are_shared() {
        let f = fixture(PHON);
        let m = LanguageModel::build(&f.hierarchy, &f.book, &f.contents);
        let a = m.gloss() as *const GlossIndex;
        let b = m.gloss() as *const GlossIndex;
        assert_eq!(a, b);
        assert!(m.gloss().has_sense("stone", crate::conlang::translate::lexmap::PosHint::Noun));
        let r1 = m.reverse_index() as *const ReverseIndex;
        let r2 = m.reverse_index() as *const ReverseIndex;
        assert_eq!(r1, r2);
        let p1 = m.analyzer() as *const Analyzer;
        let p2 = m.analyzer() as *const Analyzer;
        assert_eq!(p1, p2);
        // The analyzer sees the roots: kata + the PL suffix parses.
        assert!(!m.analyzer().analyze(m.phon(), "katai").parses.is_empty());
    }

    #[test]
    fn stamp_changes_when_a_node_is_touched_or_added() {
        let f = fixture(PHON);
        let before = ModelStamp::of(&f.hierarchy, f.book.id);
        assert_eq!(before.nodes, 13);

        // Touch the phonology paragraph.
        let mut nodes: Vec<Node> = f.hierarchy.iter().cloned().collect();
        for n in &mut nodes {
            if n.id == f.phon_para {
                n.modified_at = "2026-02-01T00:00:00Z".parse().unwrap();
            }
        }
        let touched = ModelStamp::of(&Hierarchy::from_nodes_for_test(nodes.clone()), f.book.id);
        assert_ne!(before, touched);
        assert_eq!(touched.nodes, before.nodes);

        // Add an entry with an old timestamp: the count moves even if the
        // clock did not.
        let dict = nodes.iter().find(|n| n.title == "Dictionary").unwrap().id;
        nodes.push(node(Uuid::now_v7(), "paragraph", "late", Some(dict), 9, "2025-01-01T00:00:00Z"));
        let added = ModelStamp::of(&Hierarchy::from_nodes_for_test(nodes), f.book.id);
        assert_eq!(added.nodes, before.nodes + 1);
        assert_ne!(added, touched);
    }

    #[test]
    fn cache_hits_on_equal_stamp_and_reloads_on_change() {
        let f = fixture(PHON);
        let map = Mutex::new(HashMap::new());
        let loads = std::cell::Cell::new(0usize);
        let load = || {
            loads.set(loads.get() + 1);
            Ok(LanguageModel::build(&f.hierarchy, &f.book, &f.contents))
        };
        let s1 = ModelStamp::of(&f.hierarchy, f.book.id);
        let (m1, fresh1) = lookup(&map, f.book.id, s1, load).unwrap();
        let (m2, fresh2) = lookup(&map, f.book.id, s1, load).unwrap();
        assert!(fresh1 && !fresh2);
        assert!(Arc::ptr_eq(&m1, &m2));
        assert_eq!(loads.get(), 1);

        let s2 = ModelStamp { nodes: s1.nodes + 1, ..s1 };
        let (m3, fresh3) = lookup(&map, f.book.id, s2, load).unwrap();
        assert!(fresh3);
        assert!(!Arc::ptr_eq(&m1, &m3));
        assert_eq!(loads.get(), 2);

        // Another book is a separate entry.
        let other = Uuid::now_v7();
        let (_, fresh4) = lookup(&map, other, s1, load).unwrap();
        assert!(fresh4);
        assert_eq!(loads.get(), 3);
    }
}
