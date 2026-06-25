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

/// Where the AI-pane `L` (lift) chord should file a finished generator
/// response. Set only by the generators whose output belongs in a system
/// book — the submission drafts (Submissions) and the structural analysis
/// (Planning). Scoped to a single inference via `stamp` (the owning
/// `Inference::started_at`) so a stale target can never leak into a later
/// plain chat that reuses the AI pane.
#[derive(Debug, Clone)]
pub(super) struct LiftTarget {
    /// `SYSTEM_TAG_SUBMISSIONS` or `SYSTEM_TAG_PLANNING`.
    pub book_tag: &'static str,
    /// Human label of the destination book, for the status line.
    pub book_label: &'static str,
    /// Paragraph title to upsert (overwrites an existing same-titled draft).
    pub title: String,
    /// Short label of the artefact ("query letter", "structural analysis").
    pub what: String,
    /// When set, the lift files a **scene card** instead of a system-book
    /// paragraph: the response is parsed as goal/conflict/disaster and
    /// upserted as the card titled `title` for this chapter slug. (1.3.5 P1)
    pub scene_chapter: Option<String>,
    /// Identity of the inference this target belongs to.
    pub stamp: std::time::Instant,
}

/// Scope of context an AI prompt sweeps in along with the user's query.
/// Cycled by F9: None → Selection → Paragraph → Subchapter → Chapter →
/// Book → Facts → None. Each non-None scope prepends the relevant text
/// to the query before sending; after a successful submission the mode
/// auto-resets to None so a follow-up prompt isn't surprised by stale
/// scope.
///
/// `Facts` (1.2.21+) is the odd one out: instead of text drawn from the
/// manuscript around the cursor, it loads every paragraph of the `Facts`
/// system book — the world's invariants — as ground-truth context, and
/// pre-seeds the chat with a fact-analysis system prompt.  It sits after
/// `Book` (the widest manuscript scope) since it's broader still: not a
/// slice of the book, but the reference the whole book answers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AiMode {
    None,
    Selection,
    Paragraph,
    Subchapter,
    Chapter,
    Book,
    Facts,
    /// INNER_SOCRATES-1 — a sticky conversation scope: the AI reads the open
    /// paragraph as the active Reader Persona and discusses its questions.
    Socratic,
    /// INNER_EDITOR-1 — a sticky conversation scope: the AI is the Inner Editor,
    /// discussing its literary/stylistic observations about the open paragraph.
    EditorConversation,
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
            AiMode::Facts => "Facts",
            AiMode::Socratic => "Socrates",
            AiMode::EditorConversation => "Editor",
        }
    }
    pub(super) fn next(self) -> Self {
        match self {
            AiMode::None => AiMode::Selection,
            AiMode::Selection => AiMode::Paragraph,
            AiMode::Paragraph => AiMode::Subchapter,
            AiMode::Subchapter => AiMode::Chapter,
            AiMode::Chapter => AiMode::Book,
            AiMode::Book => AiMode::Facts,
            AiMode::Facts => AiMode::Socratic,
            AiMode::Socratic => AiMode::EditorConversation,
            AiMode::EditorConversation => AiMode::None,
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

#[cfg(test)]
mod ai_mode_tests {
    use super::AiMode;

    // 1.2.21+ — Facts cycles after Book (the widest manuscript
    // scope) and wraps back to None; F9 visits all seven scopes.
    #[test]
    fn cycle_includes_facts_after_book_and_wraps() {
        let order: Vec<&str> = {
            let mut m = AiMode::None;
            let mut seen = vec![m.label()];
            loop {
                m = m.next();
                if m == AiMode::None {
                    break;
                }
                seen.push(m.label());
            }
            seen
        };
        assert_eq!(
            order,
            vec![
                "None", "Selection", "Paragraph", "Subchapter", "Chapter", "Book", "Facts",
                "Socrates", "Editor",
            ],
        );
        // Explicit: the new edges.
        assert_eq!(AiMode::Book.next(), AiMode::Facts);
        assert_eq!(AiMode::Facts.next(), AiMode::Socratic);
        // INNER_EDITOR-1 — the Editor conversation scope cycles after Socrates.
        assert_eq!(AiMode::Socratic.next(), AiMode::EditorConversation);
        assert_eq!(AiMode::EditorConversation.next(), AiMode::None);
    }
}
