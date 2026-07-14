//! RESRCH-1 (R-P8) — RAG context assembly. Reuses the existing book-scoped
//! retriever (`book_rag::retrieval::retrieve`) scoped to the **Facts book**
//! (audit correction — facts live in the Facts book + the shared HNSW, not a
//! phantom `facts.duckdb`), and prepends the author's pinned nodes (G4). The one
//! genuinely new piece is the pin prepend; the retrieval + formatting are reused.

use uuid::Uuid;

use crate::config::Config;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;

use super::thread::RagMode;

/// Assemble the RAG context block for a query, or `None` when nothing grounds it
/// (or the mode is `FullOnly`). Pinned nodes come first (always), then the
/// semantic Facts retrieval (deduplicated against the pins).
/// Cap any single retrieved passage — a huge pinned node or imported source would
/// otherwise dominate (or blow) the context window on its own.
fn clip_passage(s: &str) -> String {
    const MAX_PASSAGE_CHARS: usize = 2000;
    s.chars().take(MAX_PASSAGE_CHARS).collect()
}

pub(super) fn build_context(
    store: &Store,
    cfg: &Config,
    hierarchy: &Hierarchy,
    facts_book_id: Option<Uuid>,
    pinned: &[Uuid],
    rag_mode: RagMode,
    query: &str,
) -> (Option<String>, Vec<String>) {
    if rag_mode == RagMode::FullOnly {
        return (None, Vec::new());
    }
    let mut sections: Vec<String> = Vec::new();

    if let Some(book_id) = facts_book_id {
        // Pinned nodes (G4) — always included, in pin order.
        let mut pinned_block = String::new();
        for id in pinned {
            if let Ok(Some(bytes)) = store.get_content(*id) {
                let body = String::from_utf8_lossy(&bytes);
                if body.trim().is_empty() {
                    continue;
                }
                let loc = hierarchy.get(*id).map(|n| hierarchy.slug_path(n)).unwrap_or_default();
                pinned_block.push_str(&format!("[pinned: {loc}]\n{}\n\n", clip_passage(body.trim())));
            }
        }
        if !pinned_block.trim().is_empty() {
            sections.push(pinned_block.trim_end().to_string());
        }

        // Semantic Facts retrieval, deduplicated against the pins.
        if let Ok(passages) =
            crate::book_rag::retrieval::retrieve(store, hierarchy, &cfg.book_rag, book_id, query)
        {
            let fresh: Vec<_> = passages
                .into_iter()
                .filter(|p| !pinned.contains(&p.id))
                .take(cfg.research.rag_top_n.max(1))
                .collect();
            if !fresh.is_empty() {
                sections.push(crate::book_rag::compose_context_prefix(&fresh));
            }
        }
    }

    // R2-B — imported document chunks (a separate axis: standalone docs tagged
    // `research_source`, not tree nodes). Returns the contributing source names
    // so the caller can record `origin=document` provenance.
    let sources = retrieve_sources(store, cfg, query, &mut sections);

    let combined = sections.join("\n\n");
    let ctx = if combined.trim().is_empty() { None } else { Some(combined) };
    (ctx, sources)
}

/// Pull the top imported-document chunks for `query`, append a cited block to
/// `sections`, and return the distinct source names that contributed.
fn retrieve_sources(store: &Store, cfg: &Config, query: &str, sections: &mut Vec<String>) -> Vec<String> {
    let want = cfg.research.rag_top_n.max(1);
    let hits = match store.search_text(query, want * 4 + 8) {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };
    let mut block = String::new();
    let mut names: Vec<String> = Vec::new();
    for v in hits {
        let meta = v.get("metadata");
        let is_source = meta
            .and_then(|m| m.get("kind"))
            .and_then(|k| k.as_str())
            .map(|k| k == super::imports::SOURCE_KIND)
            .unwrap_or(false);
        if !is_source {
            continue;
        }
        let name = meta
            .and_then(|m| m.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("source")
            .to_string();
        let body = v.get("document").and_then(|d| d.as_str()).unwrap_or("").trim();
        if body.is_empty() {
            continue;
        }
        block.push_str(&format!("[source: {name}]\n{}\n\n", clip_passage(body)));
        if !names.contains(&name) {
            names.push(name);
        }
        if names.len() >= want {
            break;
        }
    }
    if !block.trim().is_empty() {
        sections.push(format!("Imported research sources:\n{}", block.trim_end()));
    }
    names
}

/// SCHOLAR P2 — one ingested source chunk as a structured passage.
pub(crate) struct SourcePassage {
    pub name: String,
    pub body: String,
}

/// SCHOLAR P2 — retrieve the `k` ingested source chunks nearest `query` as
/// STRUCTURED passages (unlike `retrieve_sources`, which side-effects a context
/// string and returns names). Feeds the graded-relation judge.
pub(crate) fn retrieve_source_passages(store: &Store, query: &str, k: usize) -> Vec<SourcePassage> {
    let hits = match store.search_text(query, k * 4 + 8) {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for v in hits {
        let meta = v.get("metadata");
        let is_source = meta
            .and_then(|m| m.get("kind"))
            .and_then(|kk| kk.as_str())
            .map(|kk| kk == super::imports::SOURCE_KIND)
            .unwrap_or(false);
        if !is_source {
            continue;
        }
        let name = meta
            .and_then(|m| m.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("source")
            .to_string();
        let body = v.get("document").and_then(|d| d.as_str()).unwrap_or("").trim().to_string();
        if body.is_empty() {
            continue;
        }
        out.push(SourcePassage { name, body });
        if out.len() >= k {
            break;
        }
    }
    out
}
