//! AI / inference state types. Owned by `App`; touched by the
//! editor / AI panes and the streaming poll loop. Pure data
//! shapes plus their tiny label/cycle helpers — no I/O.
//! Extracted from `tui::app` in the 1.2.7 refactor.

use crate::ai::stream::StreamMsg;

/// What an AI response's `Apply` chord does to the editor buffer.
/// Picked by the user (Enter / I / T / B / C / G) when the AI pane
/// is focused on a finished response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InferenceAction {
    Replace,
    Insert,
    Top,
    Bottom,
    CopyOnly,
    /// Grammar-check-aware replace: lifts ONLY the corrected paragraph
    /// from the response (between `<<<CORRECTED>>>` / `<<<END>>>` markers,
    /// or fenced code, or a "Corrected …" heading) and overwrites the
    /// editor buffer with it. No markdown→typst conversion runs — the
    /// grammar prompt instructs the model to keep Typst markup verbatim.
    ReplaceCorrected,
}

impl InferenceAction {
    pub(super) fn label(&self) -> &'static str {
        match self {
            InferenceAction::Replace => "replaced",
            InferenceAction::Insert => "inserted at cursor",
            InferenceAction::Top => "prepended to top",
            InferenceAction::Bottom => "appended to bottom",
            InferenceAction::CopyOnly => "copied",
            InferenceAction::ReplaceCorrected => "replaced with corrected text",
        }
    }
}

/// Scope of context an AI prompt sweeps in along with the user's query.
/// Cycled by F9: None → Selection → Paragraph → Subchapter → Chapter →
/// Book → None. Each non-None scope prepends the relevant text to the
/// query before sending; after a successful submission the mode auto-
/// resets to None so a follow-up prompt isn't surprised by stale scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AiMode {
    None,
    Selection,
    Paragraph,
    Subchapter,
    Chapter,
    Book,
}

impl AiMode {
    pub(super) fn label(self) -> &'static str {
        match self {
            AiMode::None => "None",
            AiMode::Selection => "Selection",
            AiMode::Paragraph => "Paragraph",
            AiMode::Subchapter => "Subchapter",
            AiMode::Chapter => "Chapter",
            AiMode::Book => "Book",
        }
    }
    pub(super) fn next(self) -> Self {
        match self {
            AiMode::None => AiMode::Selection,
            AiMode::Selection => AiMode::Paragraph,
            AiMode::Paragraph => AiMode::Subchapter,
            AiMode::Subchapter => AiMode::Chapter,
            AiMode::Chapter => AiMode::Book,
            AiMode::Book => AiMode::None,
        }
    }
}

/// How aggressively the model is allowed to draw on its own knowledge.
/// F10 toggles between the two values. Help inferences always run as
/// `Local` regardless of the user's current toggle — the Help book is the
/// authoritative source and we don't want the model paraphrasing from
/// general training data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InferenceMode {
    /// Only the supplied RAG / scope context (and prior chat turns) may be
    /// used. The system prompt instructs the model to refuse rather than
    /// fall back on outside knowledge.
    Local,
    /// Context is treated as ground truth where present, but the model
    /// may augment with general knowledge. Default for fresh chats.
    Full,
}

impl InferenceMode {
    pub(super) fn label(self) -> &'static str {
        match self {
            InferenceMode::Local => "Local",
            InferenceMode::Full => "Full",
        }
    }
    pub(super) fn toggle(self) -> Self {
        match self {
            InferenceMode::Local => InferenceMode::Full,
            InferenceMode::Full => InferenceMode::Local,
        }
    }
}

/// In-flight chat inference. Holds the streaming receiver,
/// accumulated response text, and per-turn metadata so the
/// AI pane can render progress without re-polling the channel.
#[derive(Debug)]
pub(super) struct Inference {
    pub provider: String,
    /// Kept for diagnostics on the Debug impl; not displayed in the UI.
    #[allow(dead_code)]
    pub model: String,
    pub response: String,
    pub status: InferenceStatus,
    pub rx: tokio::sync::mpsc::UnboundedReceiver<StreamMsg>,
    pub started_at: std::time::Instant,
}

#[derive(Debug, Clone)]
pub(super) enum InferenceStatus {
    Streaming,
    Done,
    Error(String),
}
