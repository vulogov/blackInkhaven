//! RESRCH-1 (R-P6) — the AI chat model. A turn is a query + its (possibly
//! streaming) response. Rendering lives in `render.rs`; this holds the data and
//! the small helpers the streaming loop (R-P7) drives.

/// One research exchange shown in the chat pane.
pub(super) struct ChatTurn {
    /// The user's query (or `/command`), shown as a `[query N]`-style header.
    pub prompt: String,
    /// The assistant's response, appended token-by-token while `streaming`.
    pub response: String,
    /// True while tokens are still arriving (R-P7).
    pub streaming: bool,
    /// Per-turn cost in USD (set on stream completion).
    pub cost: f64,
    /// R2-B — names of imported documents (or, R2-C, web URLs/titles) that
    /// grounded this answer (provenance `document` / `web` on a later `/fact`).
    pub sources: Vec<String>,
    /// R2-C — true when this answer was grounded on live web results, so a
    /// `/fact` from it is fact-checked before it commits.
    pub web_grounded: bool,
}

impl ChatTurn {
    pub(super) fn new(prompt: String) -> ChatTurn {
        ChatTurn {
            prompt,
            response: String::new(),
            streaming: false,
            cost: 0.0,
            sources: Vec::new(),
            web_grounded: false,
        }
    }

    /// A turn pre-filled with a final response (no streaming) — used for
    /// command output (`/diff`, `/verify`) and placeholders.
    pub(super) fn with_response(prompt: String, response: String) -> ChatTurn {
        ChatTurn {
            prompt,
            response,
            streaming: false,
            cost: 0.0,
            sources: Vec::new(),
            web_grounded: false,
        }
    }
}
