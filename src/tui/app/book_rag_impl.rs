//! BOOK_RAG-1 — the retrieval that grounds AI-pane Book scope.
//!
//! The retrieval core itself lives in `crate::book_rag::retrieval` as a free
//! function shared with the `inkhaven book-rag` CLI; this `App` layer adds the
//! TUI-only concerns: anchor resolution (open paragraph / tree cursor),
//! the once-per-session retrieval cache, and the post-edit staleness nudge.

use crate::store::node::NodeKind;
use crate::tui::inference::AiMode;
use uuid::Uuid;

impl super::App {
    /// Build the retrieval-grounded context prefix for a Book-scope prompt.
    /// Replaces the old "send the whole book" assembly. Errors (no anchor,
    /// search failure) abort the submission with a status message.
    pub(super) fn book_rag_context(&mut self, query: &str) -> Result<String, String> {
        // Retrieve ONCE per chat session. A follow-up in the same
        // conversation reuses the cached retrieval; clearing chat history
        // (which empties `chat_history`, the session signal) makes the next
        // Book prompt retrieve fresh. So: reuse only when the conversation is
        // ongoing AND a retrieval is already cached.
        if !self.chat_history.is_empty() {
            if let Some(passages) = self.book_rag_last_retrieval.as_ref() {
                let prefix = crate::book_rag::compose_context_prefix(passages);
                return Ok(self.with_canon_block(prefix));
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
        let prefix = crate::book_rag::compose_context_prefix(&passages);
        // Keep the retrieval for citation validation (P2) + transparency (P3).
        self.book_rag_last_retrieval = Some(passages);
        // CANON-UI-1 (CU1-P2) — pack the canon decisions behind the query
        // (semantic hits → their units → closure-complete pack to the budget).
        // Advisory + local: a failure or an empty ledger just means no block.
        self.book_rag_last_canon = None;
        if self.book_rag_canon_grounding {
            let canon = &self.cfg.canon;
            match self.store.raw().canon_context_for_query(
                query,
                self.cfg.book_rag.top_k.max(1),
                canon.context_budget,
                canon.context_reserve,
            ) {
                Ok(ctx) if !ctx.views.is_empty() => self.book_rag_last_canon = Some(ctx),
                Ok(_) => {}
                Err(e) => self.status = format!("canon grounding skipped: {e}"),
            }
        }
        let prefix = self.with_canon_block(prefix);
        // 3.9 — a new grounding set: restart `[`/`]` citation navigation.
        self.book_rag_cite_cursor = usize::MAX;
        // A fresh retrieval grounds on the current text — clear the
        // staleness nudge so it can fire again after the next edit.
        self.book_rag_nudged_stale = false;
        Ok(prefix)
    }

    /// CANON-UI-1 (CU1-P2) — append the cached canon block (framed in the
    /// prompt language) to a Book-scope prefix; unchanged when grounding is off
    /// or nothing relevant was packed.
    fn with_canon_block(&self, prefix: String) -> String {
        if !self.book_rag_canon_grounding {
            return prefix;
        }
        let Some(ctx) = self.book_rag_last_canon.as_ref() else { return prefix };
        let block = crate::book_rag::canon_context_block(&self.active_prompt_language(), &ctx.to_prompt());
        if block.is_empty() { prefix } else { format!("{prefix}\n\n{block}") }
    }

    /// Called after a paragraph save (which re-embeds it): if a Book-scope
    /// conversation is active, its cached retrieval is now grounded in
    /// pre-edit text, so nudge the author — once — to clear chat and
    /// re-ground. Non-blocking; the existing conversation stays valid.
    pub(super) fn book_rag_note_possible_staleness(&mut self) {
        if self.book_rag_nudged_stale {
            return;
        }
        let active = !self.chat_history.is_empty()
            && ((self.ai_mode == AiMode::Book && self.book_rag_last_retrieval.is_some())
                || (self.ai_mode == AiMode::Graph && self.graph_rag_last_retrieval.is_some()));
        if !active {
            return;
        }
        self.book_rag_nudged_stale = true;
        self.status =
            "book changed since retrieval — clear chat to re-ground on the new text".into();
    }

    /// The user book containing the current anchor (open paragraph, else the
    /// tree cursor). Mirrors `build_ai_mode_context`'s anchor resolution. Shared
    /// by Book scope and the GM-P4 Graph scope — both retrieve within the book
    /// the cursor is in — so the messages name no single scope.
    pub(super) fn book_rag_anchor_book(&self) -> Result<Uuid, String> {
        let anchor_id = self
            .opened
            .as_ref()
            .map(|d| d.id)
            .or_else(|| self.rows.get(self.tree_cursor).map(|(id, _)| *id))
            .ok_or_else(|| "retrieval needs an open paragraph or tree cursor".to_string())?;
        let anchor = self
            .hierarchy
            .get(anchor_id)
            .ok_or_else(|| "retrieval anchor vanished".to_string())?;
        if anchor.kind == NodeKind::Book {
            return Ok(anchor.id);
        }
        self.hierarchy
            .ancestors(anchor)
            .into_iter()
            .find(|n| n.kind == NodeKind::Book)
            .map(|n| n.id)
            .ok_or_else(|| "retrieval requires the cursor to be inside a book".to_string())
    }
}
