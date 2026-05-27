//! Miscellaneous data shapes owned by `App`: keymap, opened-
//! document buffer state, split-view, kill-ring stash, chat
//! overlay state, book-stat aggregator, image-call detection
//! result, and the small operational enums that thread through
//! the meta-chord dispatchers. No impls beyond `Keymap::from_config`
//! (a config-parse helper) and the in-line `compute_*` /
//! `detect_*` fns stay in `tui::app` for now. Extracted from
//! `tui::app` in the 1.2.7 refactor.

use uuid::Uuid;

use tui_textarea::TextArea;

use crate::config::Config;
use crate::error::{Error, Result as InkResult};

use super::keymap::KeyChord;
use super::search_replace::SearchState;

pub(super) struct Keymap {
    pub next_pane: KeyChord,
    pub prev_pane: KeyChord,
    pub search: KeyChord,
    pub ai_prompt: KeyChord,
    pub save: KeyChord,
    pub page_up: KeyChord,
    pub page_down: KeyChord,
    pub meta_prefix: KeyChord,
    /// Bund-meta prefix. `None` when the config sets
    /// `keys.bund_prefix = ""` to disable the chord (some users
    /// reserve Ctrl+Z for their terminal multiplexer).
    pub bund_prefix: Option<KeyChord>,
    /// View-meta prefix (1.2.4+, default `Ctrl+V`). `None`
    /// disables the layer (some terminals bind Ctrl+V to "verbatim
    /// next" and the user might want it back).
    pub view_prefix: Option<KeyChord>,
}

impl Keymap {
    pub(super) fn from_config(cfg: &Config) -> InkResult<Self> {
        let parse = |label: &str, s: &str| -> InkResult<KeyChord> {
            KeyChord::parse(s).map_err(|e| Error::Config(format!("keys.{label}: {e}")))
        };
        let bund_prefix = if cfg.keys.bund_prefix.trim().is_empty() {
            None
        } else {
            Some(parse("bund_prefix", &cfg.keys.bund_prefix)?)
        };
        let view_prefix = if cfg.keys.view_prefix.trim().is_empty() {
            None
        } else {
            Some(parse("view_prefix", &cfg.keys.view_prefix)?)
        };
        Ok(Self {
            next_pane: parse("next_pane", &cfg.keys.next_pane)?,
            prev_pane: parse("prev_pane", &cfg.keys.prev_pane)?,
            search: parse("search", &cfg.keys.search)?,
            ai_prompt: parse("ai_prompt", &cfg.keys.ai_prompt)?,
            save: parse("save", &cfg.keys.save)?,
            page_up: parse("page_up", &cfg.keys.page_up)?,
            page_down: parse("page_down", &cfg.keys.page_down)?,
            meta_prefix: parse("meta_prefix", &cfg.keys.meta_prefix)?,
            bund_prefix,
            view_prefix,
        })
    }
}

/// Which scope the Ctrl+V markdown export targets. Used by
/// `view_export_markdown` to route through the existing
/// per-scope helpers from one binding-table arm.
#[derive(Debug, Clone, Copy)]
pub(super) enum ViewMdScope {
    Buffer,
    Subchapter,
    Subtree,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum MoveDir {
    Up,
    Down,
}

/// Direction of a link-pick flow (Ctrl+V A vs Ctrl+V I).
/// `Outgoing` adds the picked target to the open paragraph's
/// outgoing links; `Incoming` adds the open paragraph to the
/// picked target's outgoing links (== an incoming link for
/// current).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LinkPickDirection {
    Outgoing,
    Incoming,
}

#[derive(Default)]
pub(super) struct ImportCounts {
    /// Any branch created during import: chapter, subchapter, or book.
    pub branches: usize,
    pub paragraphs: usize,
}

/// Active search session inside the AI-fullscreen chat-history pane.
/// `matches` is recomputed lazily by `draw_chat_history` whenever the
/// rendered line count changes (terminal resize) — we just track the
/// query + which match we're currently centred on.
#[derive(Debug, Clone)]
pub(super) struct ChatSearchState {
    pub query: String,
    /// Index into `matches`. The render hook clamps this against the
    /// freshly-computed match count each frame so terminal resize +
    /// streaming-token arrival can't push it out of range.
    pub current: usize,
}

/// "Chat selection mode" (Ctrl+C in AI-fullscreen). The cursor
/// points at a single turn in `chat_history`; Up / Down step through
/// turns, `c` / `C` copies the turn text to the clipboard, `t` / `T`
/// inserts it at the editor cursor.
#[derive(Debug, Clone, Copy)]
pub(super) struct ChatSelectionState {
    /// Index into `chat_history`. Always points at a valid turn —
    /// reset / clamped if the history shrinks while selection is
    /// active.
    pub turn: usize,
}

/// 1.2.7+ — stash for the most-recent paragraph delete. Used
/// by `Ctrl+B U` to recover content + metadata after a
/// confirmed delete. Stores everything needed to call
/// `create_node` + restore body + restore tags / linked
/// paragraphs / event data. Note: the restored node gets a
/// fresh uuid; cross-refs from other paragraphs (paragraph links,
/// event.linked_paragraphs) pointing at the OLD uuid stay
/// broken — flagged in the post-undo status.
#[derive(Debug, Clone)]
pub(super) struct DeletedParagraphStash {
    pub parent_id: Option<Uuid>,
    pub anchor_id: Option<Uuid>, // sibling to insert after; None = end of parent
    pub title: String,
    pub slug: String,
    pub content: Vec<u8>,
    pub tags: Vec<String>,
    pub linked_paragraphs: Vec<Uuid>,
    pub status: Option<String>,
    pub target_words: Option<i32>,
    pub content_type: Option<String>,
    pub event: Option<crate::store::node::EventData>,
}

pub(super) struct OpenedDoc {
    pub id: Uuid,
    pub title: String,
    pub rel_path: String,
    pub textarea: TextArea<'static>,
    pub dirty: bool,
    /// Custom scroll state. tui-textarea v0.7 does not expose its viewport, so
    /// we maintain our own and never call `textarea.scroll()`.
    pub scroll_row: usize,
    pub scroll_col: usize,
    /// Anchor of a vertical-block selection (entered with Alt+arrows).
    /// While Some, the cursor's current position plus this anchor define a
    /// rectangular selection drawn with REVERSED style.
    pub block_anchor: Option<(usize, usize)>,
    /// Wall-clock of the last key event handled by the editor. Idle autosave
    /// fires when (now - last_activity) >= editor.autosave_seconds.
    pub last_activity: std::time::Instant,
    /// Snapshot of `textarea.lines()` at the most recent save / load. Used to
    /// bold characters added since then.
    pub saved_lines: Vec<String>,
    /// 1.2.7+ — wall-clock mtime of the paragraph's file at
    /// the moment we loaded it (or after the last save).
    /// The idle ticker compares this to the current mtime;
    /// if the file changed externally (CLI edit, sed, git
    /// pull, …), we either silently reload (clean buffer)
    /// or warn (dirty buffer). `None` when mtime isn't
    /// available (e.g. virtual filesystem, race).
    pub loaded_mtime: Option<std::time::SystemTime>,
    /// Set when split-edit mode is active. The lower pane shows a read-only
    /// copy of `snapshot_lines`, scrolled independently of the live editor.
    pub split: Option<SplitView>,
    /// Active find / replace session (Ctrl+F / Ctrl+R). While Some, matches
    /// are highlighted red and Ctrl+G advances or replaces.
    pub search: Option<SearchState>,
    /// True when this paragraph lives inside the Help book. The editor still
    /// renders it normally (so the user can read it, scroll, search), but
    /// every mutating keystroke is intercepted with a status message.
    pub read_only: bool,
    /// Picked from the Node's `content_type` at open time. Drives
    /// which syntax highlighter the editor uses (`"hjson"` → the
    /// hand-rolled HJSON lexer; anything else → tree-sitter-typst).
    /// Also reported in the editor header so the user can tell at a
    /// glance which language they're editing.
    pub content_type: Option<String>,
    /// Pre-correction baseline captured when the AI pane's `T` (grammar-
    /// check apply) overwrites the buffer with the model's corrected text.
    /// Lines that differ from this baseline render in `theme.grammar_change_fg`
    /// so the user can eyeball what changed. Cleared on the next save
    /// (implicit "accept the corrections") or when the user opens a
    /// different paragraph.
    pub correction_baseline: Option<Vec<String>>,
    /// Cached typst parse-time diagnostics (1.2.5+). Recomputed on
    /// save and on idle when `typst_compile.diagnostics` is on and
    /// the buffer's content type is `None` (default = typst) or
    /// `Some("typst")`. Empty when the buffer parses cleanly OR
    /// when diagnostics are disabled in HJSON. See
    /// `crate::typst_check`.
    pub typst_diagnostics: Vec<crate::typst_check::TypstDiagnostic>,
    /// Wall-clock of the last typst-syntax recheck. Throttles the
    /// idle re-check against `typst_compile.diagnostics_idle_seconds`.
    pub typst_diagnostics_checked_at: std::time::Instant,
    /// 1.2.6+ — snapshot of the last diagnostic state we fired
    /// `hook.on_diagnostic` for: `(count, first-message)`. Used
    /// to debounce the hook so it only re-fires on actual state
    /// transitions (clean → errored, count change, top-message
    /// change). `None` means we've never fired or the doc is
    /// freshly opened.
    pub typst_diag_last_fired: Option<(usize, String)>,
}

pub(super) struct SplitView {
    pub snapshot_lines: Vec<String>,
    pub scroll_row: usize,
}

/// Detection result for "is the cursor sitting inside the first
/// string argument of a `#image(...)` call on this line".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ImageCallContext {
    /// True when the open `"` has a matching close `"` further along
    /// the same line. The picker uses this to decide whether to
    /// insert a closing quote after the filename or not.
    pub closing_quote_present: bool,
}

/// Aggregate counts for one root Book, computed by walking its subtree.
/// Words come from each Paragraph's stored `word_count` (kept up to date
/// at save time); sentences are derived by re-reading paragraph bodies
/// from disk, which is fine for literary-scale projects (hundreds of
/// short files) but should be reconsidered if a project ever grows past
/// many thousands of paragraphs.
#[derive(Debug, Default)]
pub(super) struct BookStats {
    pub chapters: usize,
    pub subchapters: usize,
    pub paragraphs: usize,
    pub images: usize,
    pub sentences: usize,
    pub words: u64,
}
