//! BOOK_RAG-1 — the shared retrieval core (used by the TUI Book scope and
//! the `inkhaven book-rag` CLI).
//!
//! Semantic search over the vecstore (`Store::search_text`), filtered to the
//! current book + the included author-content system books, expanded with
//! surrounding paragraphs, token-budgeted, and ordered by manuscript
//! position. Parses the search-result JSON directly so this module stays
//! free of any TUI dependency.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use super::RetrievedPassage;
use crate::config::BookRagConfig;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;
use crate::store::Store;

/// Retrieve the grounding passages for `query` within `book_id`.
pub fn retrieve(
    store: &Store,
    hierarchy: &Hierarchy,
    cfg: &BookRagConfig,
    book_id: Uuid,
    query: &str,
) -> Result<Vec<RetrievedPassage>, String> {
    let scope = scope_ids(hierarchy, cfg, book_id);
    let pool = (cfg.top_k * 4 + 16).max(16);
    let raw = store
        .search_text(query, pool)
        .map_err(|e| format!("book search: {e}"))?;

    let mut hits: Vec<(Uuid, f64)> = Vec::new();
    for v in &raw {
        let Some(id) = v
            .get("id")
            .and_then(|x| x.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
        else {
            continue;
        };
        let is_para = v
            .get("metadata")
            .and_then(|m| m.get("kind"))
            .and_then(|k| k.as_str())
            == Some("paragraph");
        if !is_para || !scope.contains(&id) {
            continue;
        }
        let score = v.get("score").and_then(|x| x.as_f64()).unwrap_or(0.0);
        hits.push((id, score));
        if hits.len() >= cfg.top_k {
            break;
        }
    }
    Ok(assemble(store, hierarchy, cfg, &hits))
}

/// Pool = the book's subtree ∪ the included system books' subtrees.
fn scope_ids(hierarchy: &Hierarchy, cfg: &BookRagConfig, book_id: Uuid) -> HashSet<Uuid> {
    let mut ids: HashSet<Uuid> = hierarchy.collect_subtree(book_id).into_iter().collect();
    for tag in &cfg.include_system_books {
        if cfg.exclude_system_books.contains(tag) {
            continue;
        }
        if let Some(sid) = system_book_id(hierarchy, tag) {
            ids.extend(hierarchy.collect_subtree(sid));
        }
    }
    ids
}

/// Expand each hit with ±N sibling paragraphs, dedup, enforce the token
/// budget (best hits first), order by manuscript position.
fn assemble(
    store: &Store,
    hierarchy: &Hierarchy,
    cfg: &BookRagConfig,
    hits: &[(Uuid, f64)],
) -> Vec<RetrievedPassage> {
    let order: HashMap<Uuid, usize> = hierarchy
        .flatten()
        .into_iter()
        .enumerate()
        .map(|(i, (n, _))| (n.id, i))
        .collect();

    let mut chosen: Vec<(Uuid, f64, bool)> = Vec::new();
    let mut seen: HashSet<Uuid> = HashSet::new();
    let mut bodies: HashMap<Uuid, String> = HashMap::new();
    let mut tokens = 0usize;

    'hits: for &(hit_id, score) in hits {
        for (pid, is_hit) in unit(hierarchy, hit_id, cfg.context_expansion) {
            if seen.contains(&pid) {
                continue;
            }
            let body = bodies.entry(pid).or_insert_with(|| body(store, pid)).clone();
            let t = super::estimate_tokens(&body);
            if !chosen.is_empty() && tokens + t > cfg.max_context_tokens {
                break 'hits;
            }
            tokens += t;
            seen.insert(pid);
            chosen.push((pid, score, is_hit));
        }
    }

    chosen.sort_by_key(|(id, _, _)| *order.get(id).unwrap_or(&usize::MAX));
    chosen
        .into_iter()
        .map(|(id, score, is_hit)| RetrievedPassage {
            id,
            breadcrumb: breadcrumb(hierarchy, id),
            body: bodies.remove(&id).unwrap_or_else(|| body(store, id)),
            score,
            is_hit,
        })
        .collect()
}

/// A hit plus its ±N sibling paragraphs in document order.
fn unit(hierarchy: &Hierarchy, hit_id: Uuid, expand: usize) -> Vec<(Uuid, bool)> {
    let Some(node) = hierarchy.get(hit_id) else {
        return vec![(hit_id, true)];
    };
    let siblings: Vec<Uuid> = hierarchy
        .children_of(node.parent_id)
        .into_iter()
        .filter(|n| n.kind == NodeKind::Paragraph)
        .map(|n| n.id)
        .collect();
    let Some(idx) = siblings.iter().position(|&id| id == hit_id) else {
        return vec![(hit_id, true)];
    };
    let lo = idx.saturating_sub(expand);
    let hi = (idx + expand).min(siblings.len() - 1);
    siblings[lo..=hi].iter().map(|&id| (id, id == hit_id)).collect()
}

fn body(store: &Store, id: Uuid) -> String {
    match store.get_content(id) {
        Ok(Some(bytes)) => String::from_utf8_lossy(&bytes).into_owned(),
        _ => String::new(),
    }
}

fn breadcrumb(hierarchy: &Hierarchy, id: Uuid) -> String {
    hierarchy
        .get(id)
        .map(|n| hierarchy.slug_path(n))
        .unwrap_or_default()
}

/// The id of the top-level system book carrying `tag`, if present.
fn system_book_id(hierarchy: &Hierarchy, tag: &str) -> Option<Uuid> {
    hierarchy
        .children_of(None)
        .into_iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(tag))
        .map(|n| n.id)
}
