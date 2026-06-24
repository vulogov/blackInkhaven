//! BOOK_RAG-1 P1 — the retrieval that grounds AI-pane Book scope.
//!
//! Generalises the shipped Facts semantic-search (`search_facts`): semantic
//! search over the vecstore, filtered to the current book + the included
//! author-content system books, expanded with surrounding paragraphs,
//! token-budgeted, composed into the grounding context. The pure pieces
//! (compose / system prompt / token estimate) live in `crate::book_rag`.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::book_rag::RetrievedPassage;
use crate::config::BookRagConfig;
use crate::store::node::NodeKind;
use crate::tui::search_results::SearchHit;

impl super::App {
    /// Build the retrieval-grounded context prefix for a Book-scope prompt.
    /// Replaces the old "send the whole book" assembly. Errors (no anchor,
    /// search failure) abort the submission with a status message.
    pub(super) fn book_rag_context(&mut self, query: &str) -> Result<String, String> {
        let book_id = self.book_rag_anchor_book()?;
        let scope = self.book_rag_scope_ids(book_id);
        let top_k = self.cfg.book_rag.top_k;

        // Semantic search → over-fetch → keep in-scope paragraphs → top-K.
        let pool = (top_k * 4 + 16).max(16);
        let raw = self
            .store
            .search_text(query, pool)
            .map_err(|e| format!("book search: {e}"))?;
        let mut hits: Vec<(Uuid, f64)> = Vec::new();
        for v in &raw {
            let Some(hit) = SearchHit::parse(v) else { continue };
            if !matches!(hit.kind, NodeKind::Paragraph) {
                continue;
            }
            if !scope.contains(&hit.id) {
                continue;
            }
            hits.push((hit.id, hit.score));
            if hits.len() >= top_k {
                break;
            }
        }

        let cfg = self.cfg.book_rag.clone();
        let passages = self.book_rag_assemble(&hits, &cfg);
        let prefix = crate::book_rag::compose_context_prefix(&passages);
        // Keep the retrieval for citation validation (P2) + transparency (P3).
        self.book_rag_last_retrieval = Some(passages);
        Ok(prefix)
    }

    /// The user book containing the current anchor (open paragraph, else the
    /// tree cursor). Mirrors `build_ai_mode_context`'s anchor resolution.
    fn book_rag_anchor_book(&self) -> Result<Uuid, String> {
        let anchor_id = self
            .opened
            .as_ref()
            .map(|d| d.id)
            .or_else(|| self.rows.get(self.tree_cursor).map(|(id, _)| *id))
            .ok_or_else(|| "AI scope `Book` needs an open paragraph or tree cursor".to_string())?;
        let anchor = self
            .hierarchy
            .get(anchor_id)
            .ok_or_else(|| "AI scope `Book` anchor vanished".to_string())?;
        if anchor.kind == NodeKind::Book {
            return Ok(anchor.id);
        }
        self.hierarchy
            .ancestors(anchor)
            .into_iter()
            .find(|n| n.kind == NodeKind::Book)
            .map(|n| n.id)
            .ok_or_else(|| "AI scope `Book` requires the cursor to be inside a book".to_string())
    }

    /// Retrieval pool = the current book's subtree ∪ the included system
    /// books' subtrees (by `system_tag`), minus the excluded ones.
    fn book_rag_scope_ids(&self, book_id: Uuid) -> HashSet<Uuid> {
        let cfg = &self.cfg.book_rag;
        let mut ids: HashSet<Uuid> =
            self.hierarchy.collect_subtree(book_id).into_iter().collect();
        for tag in &cfg.include_system_books {
            if cfg.exclude_system_books.contains(tag) {
                continue;
            }
            if let Some(sid) = self.system_book_id(tag) {
                ids.extend(self.hierarchy.collect_subtree(sid));
            }
        }
        ids
    }

    /// Expand each hit with ±N sibling paragraphs, dedup, enforce the token
    /// budget (best hits first), then order by manuscript position so the
    /// context reads naturally.
    fn book_rag_assemble(
        &self,
        hits: &[(Uuid, f64)],
        cfg: &BookRagConfig,
    ) -> Vec<RetrievedPassage> {
        // Manuscript order index (tree walk order).
        let order: HashMap<Uuid, usize> = self
            .hierarchy
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
            for (pid, is_hit) in self.book_rag_unit(hit_id, cfg.context_expansion) {
                if seen.contains(&pid) {
                    continue;
                }
                let body = bodies
                    .entry(pid)
                    .or_insert_with(|| self.book_rag_body(pid))
                    .clone();
                let t = crate::book_rag::estimate_tokens(&body);
                // Always admit at least the first passage; otherwise stop
                // before overflowing the budget.
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
                breadcrumb: self.title_breadcrumb(id),
                body: bodies.remove(&id).unwrap_or_else(|| self.book_rag_body(id)),
                score,
                is_hit,
            })
            .collect()
    }

    /// A hit plus its ±N sibling **paragraphs** in document order. The hit is
    /// flagged `is_hit`; the neighbours aren't.
    fn book_rag_unit(&self, hit_id: Uuid, expand: usize) -> Vec<(Uuid, bool)> {
        let Some(node) = self.hierarchy.get(hit_id) else {
            return vec![(hit_id, true)];
        };
        let siblings: Vec<Uuid> = self
            .hierarchy
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
        siblings[lo..=hi]
            .iter()
            .map(|&id| (id, id == hit_id))
            .collect()
    }

    /// The paragraph's `.typ` body (Typst prose) from the store blob.
    fn book_rag_body(&self, id: Uuid) -> String {
        match self.store.get_content(id) {
            Ok(Some(bytes)) => String::from_utf8_lossy(&bytes).into_owned(),
            _ => String::new(),
        }
    }
}
