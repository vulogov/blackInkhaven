//! 1.3.10 WORLD-2 — `inkhaven drift <subcommand>`.
//!
//! Semantic drift: two descriptions of the same entity that diverge without a
//! hard factual clash. The retrieval half (this phase, P0) reuses the existing
//! on-save paragraph vector index: for each Character / Place / Artefact, it
//! semantically retrieves the paragraphs that describe it and keeps the ones
//! that actually name it. `inkhaven drift list` prints what the retriever
//! found (deterministic, no AI); the AI adjudication + sidecar land in P1.

use std::collections::HashMap;
use std::path::Path;

use uuid::Uuid;

use crate::config::Config;
use crate::drift::{assemble_descriptions, Candidate, DescriptionSnippet, EntityDescriptions, EntityKind};
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;
use crate::store::{Store, SYSTEM_TAG_ARTEFACTS, SYSTEM_TAG_CHARACTERS, SYSTEM_TAG_PLACES};

use super::DriftCommand;

/// How many vector hits to pull per entity before name-filtering + capping.
const TOP_K: usize = 24;
/// Max description snippets kept per entity (bounds the P1 judge prompt).
const MAX_SNIPPETS: usize = 8;

pub fn run(project: &Path, cmd: DriftCommand) -> Result<()> {
    match cmd {
        DriftCommand::List { json } => list(project, json),
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
    Ok(gather(&store, &hierarchy))
}

/// The store-backed retrieval, factored out so the project-open boilerplate
/// stays in `collect_entity_descriptions`.
fn gather(store: &Store, hierarchy: &Hierarchy) -> Vec<EntityDescriptions> {
    let index = chapter_index(hierarchy);
    let mut out = Vec::new();
    for (entity, kind) in entities(hierarchy) {
        let snippets = retrieve(store, &index, &entity);
        if !snippets.is_empty() {
            out.push(EntityDescriptions { entity, kind, snippets });
        }
    }
    out
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
                    out.push((n.title.trim().to_string(), kind));
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
) -> Vec<DescriptionSnippet> {
    let query = format!("{entity} description appearance manner voice condition");
    let raw = match store.search_text(&query, TOP_K) {
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
    assemble_descriptions(entity, &candidates, MAX_SNIPPETS)
}

fn list(project: &Path, json: bool) -> Result<()> {
    let descs = collect_entity_descriptions(project)?;
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
