//! GRAPHMIND GM-P4 — the TUI layer for the Graph AI scope (chat with your
//! graph). Reuses the Book-scope retrieval verbatim to pick the seed passages,
//! then folds in each seed node's one-hop graph neighbourhood (the edges
//! touching it) so the LLM answers grounded in how the book's parts connect.
//!
//! The pure composition + system prompt live in [`crate::graph_rag`]; this layer
//! owns the store, the hierarchy, the endpoint labels, and the per-session cache.

use crate::store::graph::EndpointRef;
use uuid::Uuid;

impl super::App {
    /// Build the graph-grounded context prefix for a Graph-scope prompt.
    /// Retrieves the relevant passages (the same retrieval Book scope uses),
    /// then renders each one's graph relations beneath it. Errors (no anchor,
    /// search failure) abort the submission with a status message.
    pub(super) fn graph_rag_context(&mut self, query: &str) -> Result<String, String> {
        // Retrieve ONCE per chat session, like Book scope: a follow-up in the
        // same conversation reuses the cached passages+relations; clearing chat
        // history re-grounds the next Graph prompt.
        if !self.chat_history.is_empty() {
            if let Some(passages) = self.graph_rag_last_retrieval.as_ref() {
                return Ok(crate::graph_rag::compose_graph_context(passages));
            }
        }

        let book_id = self.book_rag_anchor_book()?;
        let passages = crate::book_rag::retrieval::retrieve(
            &self.store,
            &self.hierarchy,
            &self.cfg.book_rag,
            book_id,
            query,
        )?;
        // Fold each seed's graph neighbourhood in. The immutable borrows of
        // `self` (store + hierarchy) complete before the cache is written.
        let graph_passages: Vec<crate::graph_rag::GraphPassage> = passages
            .into_iter()
            .map(|p| {
                let relations = self.graph_relation_lines(p.id);
                crate::graph_rag::GraphPassage { passage: p, relations }
            })
            .collect();
        let prefix = crate::graph_rag::compose_graph_context(&graph_passages);
        self.graph_rag_last_retrieval = Some(graph_passages);
        Ok(prefix)
    }

    /// Render the one-hop graph relations touching `node` as readable prompt
    /// lines — `→ contradicts fact "…" — reason`. Direction arrows match the
    /// neighbourhood view (`→` out, `←` in, `⇄` symmetric). Best-effort: a
    /// graph read error yields no relations rather than aborting the prompt
    /// (the prose alone still grounds the answer). Capped per node so a hub
    /// paragraph can't flood the context window.
    fn graph_relation_lines(&self, node: Uuid) -> Vec<String> {
        const PER_NODE: usize = 12;
        let edges = self.store.neighbors(node, &[]).unwrap_or_default();
        let here = EndpointRef::Node(node);
        let h = &self.hierarchy;
        let label = |ep: &EndpointRef| -> String {
            match ep {
                EndpointRef::Node(u) => h
                    .get(*u)
                    .map(|n| n.title.clone())
                    .filter(|t| !t.trim().is_empty())
                    .unwrap_or_else(|| format!("node {}", &u.to_string()[..8])),
                EndpointRef::Extern(_) => {
                    let (k, r) = ep.as_columns();
                    format!("{k} {r}")
                }
            }
        };
        let total = edges.len();
        let mut lines: Vec<String> = edges
            .iter()
            .take(PER_NODE)
            .map(|e| {
                let arrow = if !e.directed {
                    "⇄"
                } else if e.src == here {
                    "→"
                } else {
                    "←"
                };
                let reason = e
                    .reason
                    .as_deref()
                    .filter(|r| !r.is_empty())
                    .map(|r| format!(" — {r}"))
                    .unwrap_or_default();
                format!("{arrow} {} {}{}", e.kind.as_str(), label(e.other_endpoint(&here)), reason)
            })
            .collect();
        if total > PER_NODE {
            lines.push(format!("… +{} more relation(s)", total - PER_NODE));
        }
        lines
    }
}
