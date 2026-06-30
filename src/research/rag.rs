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
pub(super) fn build_context(
    store: &Store,
    cfg: &Config,
    hierarchy: &Hierarchy,
    facts_book_id: Option<Uuid>,
    pinned: &[Uuid],
    rag_mode: RagMode,
    query: &str,
) -> Option<String> {
    if rag_mode == RagMode::FullOnly {
        return None;
    }
    let book_id = facts_book_id?;
    let mut sections: Vec<String> = Vec::new();

    // Pinned nodes (G4) — always included, in pin order.
    let mut pinned_block = String::new();
    for id in pinned {
        if let Ok(Some(bytes)) = store.get_content(*id) {
            let body = String::from_utf8_lossy(&bytes);
            if body.trim().is_empty() {
                continue;
            }
            let loc = hierarchy.get(*id).map(|n| hierarchy.slug_path(n)).unwrap_or_default();
            pinned_block.push_str(&format!("[pinned: {loc}]\n{}\n\n", body.trim()));
        }
    }
    if !pinned_block.trim().is_empty() {
        sections.push(pinned_block.trim_end().to_string());
    }

    // Semantic Facts retrieval, deduplicated against the pins.
    if let Ok(passages) = crate::book_rag::retrieval::retrieve(
        store,
        hierarchy,
        &cfg.book_rag,
        book_id,
        query,
    ) {
        let fresh: Vec<_> = passages
            .into_iter()
            .filter(|p| !pinned.contains(&p.id))
            .take(cfg.research.rag_top_n.max(1))
            .collect();
        if !fresh.is_empty() {
            sections.push(crate::book_rag::compose_context_prefix(&fresh));
        }
    }

    let combined = sections.join("\n\n");
    if combined.trim().is_empty() { None } else { Some(combined) }
}
