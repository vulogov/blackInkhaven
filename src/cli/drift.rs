//! 1.3.10 WORLD-2 — `inkhaven drift <subcommand>`.
//!
//! Semantic drift: two descriptions of the same entity that diverge without a
//! hard factual clash. The retrieval half (this phase, P0) reuses the existing
//! on-save paragraph vector index: for each Character / Place / Artefact, it
//! semantically retrieves the paragraphs that describe it and keeps the ones
//! that actually name it. `inkhaven drift list` prints what the retriever
//! found (deterministic, no AI); the AI adjudication + sidecar land in P1.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use uuid::Uuid;

use crate::ai::AiClient;
use crate::config::Config;
use crate::drift::{
    assemble_descriptions, attribute_continuations, parse_drift_pairs, resolve_conflicts, Candidate,
    DescriptionSnippet, DriftReport, EntityDescriptions, EntityKind,
};
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;
use crate::store::{Store, SYSTEM_TAG_ARTEFACTS, SYSTEM_TAG_CHARACTERS, SYSTEM_TAG_PLACES};

use super::DriftCommand;

const DRIFT_SYSTEM_PROMPT: &str = "You are a continuity editor for a work of fiction. You receive \
NUMBERED descriptions of a SINGLE entity (a character, place, or object), each drawn from a \
different point in the manuscript, in chapter order. Flag pairs that CONTRADICT each other — the \
same attribute described in incompatible ways (a place cramped vs spacious, smoky vs airy; a \
character soft-spoken vs booming; an object pristine vs battered) with no in-story event that \
would explain the change. Do NOT flag descriptions that merely add new detail, describe different \
aspects, or reflect a change the story clearly dramatizes. Output ONE contradiction per line, in \
the exact form:\n\
  i | j | why\n\
where `i` and `j` are the description NUMBERS and `why` is a one-line explanation of the \
contradiction. Output nothing else — no preamble, no commentary, no markdown. If the descriptions \
are consistent, output nothing.";

pub fn run(project: &Path, cmd: DriftCommand) -> Result<()> {
    match cmd {
        DriftCommand::List { json, entity } => list(project, json, entity.as_deref()),
        DriftCommand::Scan { provider, json } => scan(project, provider.as_deref(), json),
    }
}

/// Retrieve the description snippets for every entity in the project's
/// Characters / Places / Artefacts books. The reusable WORLD-2 substrate —
/// P1's `scan` judges these, P3's story bible renders them.
pub fn collect_entity_descriptions(project: &Path) -> Result<Vec<EntityDescriptions>> {
    let layout = ProjectLayout::new(project);
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let hierarchy = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    Ok(gather(&store, &hierarchy, &cfg.drift, &cfg.language))
}

/// The store-backed retrieval, factored out so the project-open boilerplate
/// stays in `collect_entity_descriptions`.
fn gather(
    store: &Store,
    hierarchy: &Hierarchy,
    cfg: &crate::config::DriftConfig,
    language: &str,
) -> Vec<EntityDescriptions> {
    let index = chapter_index(hierarchy);
    let lexicon = entities(hierarchy);
    // 1.3.11 — coreference-lite: attribute pronoun-only descriptions to the
    // last unambiguously-named entity (per-language pronouns), so they survive
    // the name filter.
    let chapters = chapter_paragraphs(store, hierarchy);
    let coref = attribute_continuations(&chapters, &lexicon, language);
    let mut out = Vec::new();
    for (entity, kind) in lexicon.iter().cloned() {
        let coref_ids: HashSet<Uuid> = coref
            .iter()
            .filter(|(_, names)| names.iter().any(|n| n.eq_ignore_ascii_case(&entity)))
            .map(|(p, _)| *p)
            .collect();
        let snippets = retrieve(store, &index, &entity, cfg, &coref_ids);
        if !snippets.is_empty() {
            out.push(EntityDescriptions { entity, kind, snippets });
        }
    }
    out
}

/// Every user-book chapter as an ordered list of `(paragraph, plain text)` —
/// the sequence the coref pass walks. System books are excluded.
fn chapter_paragraphs(store: &Store, h: &Hierarchy) -> Vec<Vec<(Uuid, String)>> {
    let mut chapters = Vec::new();
    for book in h.iter().filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none()) {
        for chapter in h.children_of(Some(book.id)) {
            if chapter.kind != NodeKind::Chapter {
                continue;
            }
            let mut paras = Vec::new();
            for pid in h.collect_subtree(chapter.id) {
                if h.get(pid).map(|n| n.kind) != Some(NodeKind::Paragraph) {
                    continue;
                }
                if let Ok(Some(bytes)) = store.get_content(pid) {
                    let text = crate::audiobook::typst_to_plain(&String::from_utf8_lossy(&bytes))
                        .trim()
                        .to_string();
                    if !text.is_empty() {
                        paras.push((pid, text));
                    }
                }
            }
            if !paras.is_empty() {
                chapters.push(paras);
            }
        }
    }
    chapters
}

/// Map every user-book paragraph to its `(chapter_order, chapter_title)`.
/// System books (Characters / Facts / …) are excluded, so retrieval only
/// surfaces *prose* descriptions, never the entity's own bible entry.
fn chapter_index(h: &Hierarchy) -> HashMap<Uuid, (usize, String)> {
    let mut map = HashMap::new();
    let mut order = 0usize;
    for book in h.iter().filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none()) {
        for chapter in h.children_of(Some(book.id)) {
            if chapter.kind != NodeKind::Chapter {
                continue;
            }
            let title = if chapter.title.trim().is_empty() {
                chapter.slug.clone()
            } else {
                chapter.title.clone()
            };
            for pid in h.collect_subtree(chapter.id) {
                if h.get(pid).map(|n| n.kind) == Some(NodeKind::Paragraph) {
                    map.insert(pid, (order, title.clone()));
                }
            }
            order += 1;
        }
    }
    map
}

/// Every entity name + kind across the three entity books.
fn entities(h: &Hierarchy) -> Vec<(String, EntityKind)> {
    entities_with_nodes(h).into_iter().map(|(n, k, _)| (n, k)).collect()
}

/// As `entities`, but also the entity's own bible paragraph id (its
/// definition) — the jump target for an undescribed-entity finding. Shared
/// with the world report (undescribed-entity coverage).
pub fn entities_with_nodes(h: &Hierarchy) -> Vec<(String, EntityKind, Uuid)> {
    let books = [
        (SYSTEM_TAG_CHARACTERS, EntityKind::Character),
        (SYSTEM_TAG_PLACES, EntityKind::Place),
        (SYSTEM_TAG_ARTEFACTS, EntityKind::Artefact),
    ];
    let mut out = Vec::new();
    for (tag, kind) in books {
        let Some(book) = h
            .iter()
            .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(tag))
        else {
            continue;
        };
        for id in h.collect_subtree(book.id) {
            if let Some(n) = h.get(id) {
                if n.kind == NodeKind::Paragraph && !n.title.trim().is_empty() {
                    out.push((n.title.trim().to_string(), kind, n.id));
                }
            }
        }
    }
    out
}

/// Retrieve + assemble one entity's description snippets from the existing
/// vector index. The impure edge (vector search + content reads); the keep /
/// dedup / order / cap logic is the pure `assemble_descriptions`.
fn retrieve(
    store: &Store,
    index: &HashMap<Uuid, (usize, String)>,
    entity: &str,
    cfg: &crate::config::DriftConfig,
    coref_ids: &HashSet<Uuid>,
) -> Vec<DescriptionSnippet> {
    let query = format!("{entity} description appearance manner voice condition");
    let raw = match store.search_text(&query, cfg.top_k) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let mut candidates = Vec::new();
    for v in raw {
        let Some(id) = v
            .get("id")
            .and_then(|x| x.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
        else {
            continue;
        };
        let Some((order, title)) = index.get(&id) else {
            continue; // not a user-book paragraph (system book, branch, …)
        };
        if let Ok(Some(bytes)) = store.get_content(id) {
            let text = crate::audiobook::typst_to_plain(&String::from_utf8_lossy(&bytes))
                .trim()
                .to_string();
            if text.is_empty() {
                continue;
            }
            candidates.push(Candidate {
                paragraph: id,
                chapter_order: *order,
                chapter_title: title.clone(),
                text,
            });
        }
    }
    assemble_descriptions(entity, &candidates, cfg.max_snippets, coref_ids)
}

fn list(project: &Path, json: bool, entity: Option<&str>) -> Result<()> {
    let mut descs = collect_entity_descriptions(project)?;
    if let Some(name) = entity {
        let needle = name.to_lowercase();
        descs.retain(|d| d.entity.to_lowercase().contains(&needle));
    }
    if json {
        let payload = serde_json::to_string_pretty(&descs)
            .map_err(|e| Error::Store(format!("serialize drift descriptions: {e}")))?;
        println!("{payload}");
        return Ok(());
    }
    if descs.is_empty() {
        println!(
            "drift: no entity descriptions retrieved — populate the Characters / Places / \
             Artefacts books, and make sure the vector index is built (open + save once, or \
             reindex)."
        );
        return Ok(());
    }
    let total: usize = descs.iter().map(|d| d.snippets.len()).sum();
    println!(
        "drift: {} entit{} described across {total} paragraph(s)\n",
        descs.len(),
        if descs.len() == 1 { "y" } else { "ies" }
    );
    for d in &descs {
        println!("{} ({}) — {} snippet(s):", d.entity, d.kind.label(), d.snippets.len());
        for s in &d.snippets {
            let preview: String = s.text.chars().take(100).collect();
            let ell = if s.text.chars().count() > 100 { "…" } else { "" };
            println!("  · [{}] {preview}{ell}", s.chapter);
        }
        println!();
    }
    Ok(())
}

/// The AI drift pass: for every entity with ≥2 retrieved descriptions, ask the
/// model which pairs contradict; write `<project>/.inkhaven/drift.json`.
fn scan(project: &Path, provider: Option<&str>, json: bool) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let hierarchy = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    let never = std::sync::atomic::AtomicBool::new(false);
    let report = scan_with(&store, &hierarchy, &cfg, &layout, provider, &never, &|s| eprintln!("{s}"))?;

    if json {
        let rendered = serde_json::to_string_pretty(&report)
            .map_err(|e| Error::Store(format!("drift JSON: {e}")))?;
        println!("{rendered}");
    } else if report.conflicts.is_empty() {
        let n = report.descriptions.iter().filter(|d| d.snippets.len() >= 2).count();
        println!(
            "drift scan: ✓ no description contradictions across {n} entit{}",
            if n == 1 { "y" } else { "ies" }
        );
    } else {
        println!("drift scan: {} description contradiction(s):", report.conflicts.len());
        for c in &report.conflicts {
            println!(
                "  ⚠ {} ({}) — [{}] “{}”  ⟷  [{}] “{}”\n      ↳ {}",
                c.entity, c.kind.label(), c.chapter_a, c.a, c.chapter_b, c.b, c.detail
            );
        }
        eprintln!("  (also surfaced in `inkhaven edit`)");
    }
    Ok(())
}

/// 1.3.12 DEEP-1 — the drift pass against an **already-open** store, progress
/// routed through `progress` (not stderr), returning the saved report. Reads
/// `cfg.language` so the judge prompt stays in the manuscript's language.
/// Safe to call from the TUI background-refresh thread (shares the store's
/// connection pool).
#[allow(clippy::too_many_arguments)]
pub fn scan_with(
    store: &Store,
    hierarchy: &Hierarchy,
    cfg: &Config,
    layout: &ProjectLayout,
    provider: Option<&str>,
    cancel: &std::sync::atomic::AtomicBool,
    progress: &dyn Fn(&str),
) -> Result<DriftReport> {
    let descs = gather(store, hierarchy, &cfg.drift, &cfg.language);
    let comparable: Vec<&EntityDescriptions> =
        descs.iter().filter(|d| d.snippets.len() >= 2).collect();
    if comparable.is_empty() {
        return Err(Error::Store(
            "drift scan: no entity has two or more retrievable descriptions to compare — \
             populate the entity books and make sure the vector index is built"
                .into(),
        ));
    }

    let language = if cfg.language.trim().is_empty() {
        "English".to_string()
    } else {
        cfg.language.clone()
    };
    let ai = AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;
    progress(&format!(
        "drift scan · language: {language} · model: {model} · {} entit{} to check",
        comparable.len(),
        if comparable.len() == 1 { "y" } else { "ies" }
    ));

    let mut conflicts = Vec::new();
    for (i, d) in comparable.iter().enumerate() {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(Error::Store("cancelled".into())); // partial report not saved
        }
        progress(&format!("drift [{}/{}] {}", i + 1, comparable.len(), d.entity));
        let prompt = build_drift_prompt(&language, d);
        let raw = run_blocking(&ai, model, DRIFT_SYSTEM_PROMPT, &prompt)?;
        let pairs = parse_drift_pairs(&raw, d.snippets.len());
        conflicts.extend(resolve_conflicts(&d.entity, d.kind, &d.snippets, &pairs));
    }

    let report = DriftReport {
        version: env!("CARGO_PKG_VERSION").to_string(),
        content_hash: DriftReport::compute_hash(&descs),
        conflicts,
        descriptions: descs,
        manuscript_fingerprint: crate::world_report::manuscript_fingerprint(hierarchy),
    };
    report
        .save(&layout.root)
        .map_err(|e| Error::Store(format!("drift save: {e}")))?;
    Ok(report)
}

/// Build the per-entity judge prompt: the entity name + its numbered,
/// chapter-ordered description snippets.
fn build_drift_prompt(language: &str, d: &EntityDescriptions) -> String {
    let mut body = format!(
        "Language of the manuscript: {language}. Write each `why` explanation in {language}.\n\
         Entity: {} ({}).\nDescriptions, in chapter order:\n",
        d.entity,
        d.kind.label()
    );
    for (i, s) in d.snippets.iter().enumerate() {
        body.push_str(&format!("[{}] (ch. {}) {}\n", i + 1, s.chapter, s.text));
    }
    body
}

fn run_blocking(ai: &AiClient, model: &str, system: &str, prompt: &str) -> Result<String> {
    crate::ai::stream::collect_blocking(
        ai.client.clone(),
        model.to_string(),
        Some(system.to_string()),
        prompt.to_string(),
    )
    .map_err(|e| Error::Store(format!("inference error: {e}")))
}
