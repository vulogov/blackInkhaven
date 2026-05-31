//! `Modal` — the discriminated union over every overlay /
//! picker / prompt the TUI ever puts in front of the editor —
//! plus the picker-row data shapes its variants own. Variant
//! fields are accessed across the module boundary by `App`
//! method bodies, so the enum is `pub(super)`; its fields are
//! public-by-default through the enum visibility. Extracted
//! from `tui::app` in the 1.2.7 refactor.
//!
//! No methods on `Modal` itself — every state transition lives
//! in `App` so the dispatch table can read/write multiple
//! pieces of App state in one shot. The fns kept here are pure
//! row filters (`visible_event_entries`) used by both the modal
//! lifecycle and its draw helpers.

use std::path::PathBuf;

use tui_textarea::TextArea;
use uuid::Uuid;

use crate::store::node::NodeKind;
use crate::store::{InsertPosition, Snapshot};

use super::diff_utils::SnapshotDiffRow;
use super::file_picker::FilePicker;
use super::focus::Focus;
use super::inference::InferenceAction;
use super::input::TextInput;
use super::timeline_state::TimelineViewState;

/// One entry in the `/` prompt picker. Wraps both shipping HJSON prompts
/// (`PromptSource::System`) and user-authored paragraphs under the Prompts
/// book (`PromptSource::Book`). The body is lazily fetched for book
/// paragraphs so we don't hit the store while filtering as the user types.
#[derive(Debug, Clone)]
pub(super) struct PromptCandidate {
    pub name: String,
    pub description: String,
    pub body: PromptBody,
    pub source: PromptSource,
    /// 1.2.12+ Phase C — language tag attached to this
    /// prompt, if any.  For hjson entries this is the
    /// `language` field; for Prompts-book paragraphs
    /// it's the `lang:<code>` tag value.  `None` =
    /// untagged.  Drives sectioning + chip display in
    /// the `/` picker.
    pub language: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) enum PromptBody {
    Static(String),
    BookParagraph(Uuid),
}

#[derive(Debug, Clone, Copy)]
pub(super) enum PromptSource {
    System,
    Book,
}

/// Where the `Ctrl+Z ?` script picker is sourcing entries from.
/// `A` inside the modal toggles between the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScriptPickerScope {
    /// Scripts under the cursor's nearest containing branch
    /// (subchapter / chapter / book — whichever is closest).
    Branch,
    /// Scripts under the `Scripts` system book.
    ScriptsBook,
}

/// One row in the script-picker modal.
#[derive(Debug, Clone)]
pub(super) struct ScriptPickerEntry {
    pub id: Uuid,
    pub title: String,
    pub slug_path: String,
}

/// One row in the similar-paragraph picker modal.
#[derive(Debug, Clone)]
pub(super) struct SimilarPickerEntry {
    pub id: Uuid,
    pub title: String,
    pub slug_path: String,
    pub score: f64,
    pub snippet: String,
}

/// One page of a rendered paragraph kept in the preview modal —
/// just enough state for ratatui-image to repaint it and for the
/// title bar to show "page N/M · width×height".
pub(super) struct RenderedPageProto {
    pub proto: ratatui_image::protocol::StatefulProtocol,
    pub width: u32,
    pub height: u32,
}

/// Which set of nodes the Ctrl+B ] / g tag picker applies tags
/// to when the user hits T. `Search` is the read-only mode
/// triggered by Ctrl+B }; T is a no-op there and Enter opens
/// the tag-search results instead.
#[derive(Debug, Clone)]
pub(super) enum TagPickerTarget {
    /// Editor pane: the open paragraph (carries its title for
    /// the modal's status hint).
    EditorParagraph { id: Uuid, title: String },
    /// Tree pane: the marked set (or the cursor row when marks
    /// are empty) — every paragraph-kind node in the list.
    TreeSelection(Vec<Uuid>),
    /// Search-by-tag: T / Space have no effect; Enter on a tag
    /// opens the TagSearchResults modal.
    Search,
}

/// Save-mode for the SaveRenderedPng picker. Drives whether one
/// page or every page lands on disk.
#[derive(Debug, Clone)]
pub(super) enum PagesToSave {
    /// Single page at the given 0-based index. File path is the
    /// user's input verbatim.
    Single(usize),
    /// Every page. The user's input is the *base* path
    /// (`/path/to/render` or `/path/to/render.png`); inkhaven
    /// inserts `-page-NNN` before the `.png` extension and
    /// writes one file per page.
    All,
}

/// One row in the `Ctrl+B P`-while-inside-`#image(...)` picker. The
/// `fname` is what gets inserted at the cursor — already in
/// `NN-slug.<ext>` form (Node::fs_name).
#[derive(Debug, Clone)]
pub(super) struct ImagePickerEntry {
    pub fname: String,
    pub title: String,
    pub size_bytes: u64,
}

/// One row in the `Ctrl+B 1..7` status-filter list. Carries the
/// paragraph id (for opening on Enter) plus a pre-rendered
/// breadcrumb so the user can disambiguate same-titled paragraphs
/// across chapters at a glance.
#[derive(Debug, Clone)]
pub(super) struct StatusFilterEntry {
    pub id: Uuid,
    pub title: String,
    pub breadcrumb: String,
}

/// 1.2.6+ — one entry in the Ctrl+V e event picker.
/// Snapshot built at open time so navigation is pure UI work
/// (no hierarchy reload per keystroke).
#[derive(Debug, Clone)]
pub(super) struct EventPickerEntry {
    pub id: Uuid,
    pub title: String,
    pub start_ticks: i64,
    pub start_str: String,
    pub glyph: String,
    pub track: Option<String>,
    pub is_orphan: bool,
}

/// Filter helper: returns refs to the entries whose `track`
/// equals `filter` (case-insensitive). `None` filter passes
/// everything through.
pub(super) fn visible_event_entries<'a>(
    entries: &'a [EventPickerEntry],
    filter: Option<&str>,
) -> Vec<&'a EventPickerEntry> {
    match filter {
        Some(t) => entries
            .iter()
            .filter(|e| {
                e.track
                    .as_deref()
                    .map(|track| track.eq_ignore_ascii_case(t))
                    .unwrap_or(false)
            })
            .collect(),
        None => entries.iter().collect(),
    }
}

pub(super) enum Modal {
    None,
    Adding {
        kind: NodeKind,
        parent_id: Option<Uuid>,
        parent_label: String,
        input: TextInput,
        /// Where in the parent's children list the new node lands.
        position: InsertPosition,
    },
    Deleting {
        root_id: Uuid,
        root_kind: NodeKind,
        title: String,
        descendant_count: usize,
        ids: Vec<Uuid>,
    },
    Renaming {
        node_id: Uuid,
        kind: NodeKind,
        input: TextInput,
    },
    /// Ctrl+Z E — one-shot Bund eval. The user types an
    /// expression; Enter runs it against Adam and pops the
    /// result onto the status bar. Esc cancels.
    BundEval {
        input: TextInput,
    },
    /// Generic input modal opened by the `ink.input` Bund word.
    /// Shows `prompt`, reads a single line of text, and on Enter
    /// fires the named hook with the typed string on the stack
    /// (`scripting::hooks::fire(hook, [Value::String])`). Esc
    /// closes without firing. Hook-driven rather than synchronous
    /// because a blocking modal would freeze autosave + inference
    /// polling for the duration of the prompt.
    BundInput {
        prompt: String,
        input: TextInput,
        hook: String,
    },
    /// Floating Bund output pane. Opened by `ink.pane.show`,
    /// receives every subsequent `print`/`println` output until
    /// `ink.pane.close` (or Esc). While this is open, the print
    /// buffer that normally drains to the status bar is bypassed
    /// and text lands here instead — letting scripts emit long /
    /// multi-line output without clobbering the status line.
    BundPane {
        title: String,
        lines: Vec<String>,
        scroll: usize,
    },
    /// Ctrl+Z ? — pick + execute a Bund script. `scope`
    /// switches between the cursor's containing branch and the
    /// global `Scripts` system book via the `A` key.
    ScriptPicker {
        scope: ScriptPickerScope,
        entries: Vec<ScriptPickerEntry>,
        cursor: usize,
        scroll: usize,
    },
    /// Ctrl+V G — writing-progress overview. Renders the cached
    /// snapshot (today/streak/per-book/status ladder) plus a
    /// 30-day sparkline. Read-only; refresh on open.
    Progress {
        scroll: usize,
    },
    /// Ctrl+V T — set / clear the per-paragraph word-count goal.
    /// Empty or `0` clears the target. Lives in the same input-
    /// modal family as BundEval / HelpQuery.
    ParagraphTarget {
        input: TextInput,
    },
    /// Ctrl+V 1/2 save-as modal (1.2.4+). Pre-filled with the
    /// default markdown destination — Enter writes; Esc cancels.
    /// `body` is the markdown bytes computed before the modal
    /// opened; `label` is the human-readable name used for any
    /// fallback default path computation.
    SaveMarkdown {
        input: TextInput,
        body: String,
        label: String,
    },
    /// Ctrl+V L (1.2.4+) — linked-paragraphs floating modal.
    /// Lists the open paragraph's outgoing `linked_paragraphs`
    /// metadata entries. `D` on a row removes the link.
    LinkPicker {
        owner: Uuid,
        entries: Vec<ScriptPickerEntry>,
        cursor: usize,
        scroll: usize,
    },
    /// Ctrl+V K (1.2.4+) — backlinks floating modal. Reverse of
    /// LinkPicker: lists paragraphs whose `linked_paragraphs`
    /// contains `target`. `D` removes the source's outgoing
    /// link to `target` (mutates the source paragraph).
    BacklinkPicker {
        target: Uuid,
        entries: Vec<ScriptPickerEntry>,
        cursor: usize,
        scroll: usize,
    },
    /// Ctrl+V M (1.2.4+) — bookmark picker. Lists every
    /// paragraph with `bookmark = true`. Enter opens; D
    /// clears the bookmark flag.
    BookmarkPicker {
        entries: Vec<ScriptPickerEntry>,
        cursor: usize,
        scroll: usize,
    },
    /// Ctrl+V P (1.2.4+) — fuzzy paragraph picker. The
    /// `entries` field is pre-computed from every paragraph
    /// node; the input box narrows the visible list as the
    /// user types.
    FuzzyParagraphPicker {
        input: TextInput,
        entries: Vec<ScriptPickerEntry>,
        cursor: usize,
        scroll: usize,
    },
    /// F6 picker → `V` opens a two-pane diff of the cursor's
    /// snapshot against the open paragraph's current buffer.
    /// Read-only; Esc returns to the snapshot picker.
    SnapshotDiff {
        paragraph_title: String,
        when: String,
        /// Aligned line pairs: `(left_label, left_text,
        /// right_label, right_text, kind)` per row. `kind`
        /// drives the row colour.
        rows: Vec<SnapshotDiffRow>,
        scroll: usize,
        /// Stashed snapshot picker we came from, so `Esc`
        /// restores it intact instead of closing both layers.
        return_to: Box<Modal>,
    },
    /// Ctrl+V S — pick a paragraph similar to the current buffer.
    /// Result list comes from the vector index seeded with the
    /// current paragraph's text; entries always exclude the
    /// current paragraph itself.
    SimilarPicker {
        entries: Vec<SimilarPickerEntry>,
        cursor: usize,
        scroll: usize,
    },
    FilePicker(FilePicker),
    /// Ctrl+H quick reference. Pane-aware: content is fetched from
    /// `quickref::entries_for(focus_when_opened)`.
    QuickRef {
        focus: Focus,
        scroll: usize,
    },
    /// Ctrl+B V — version, author, and credits panel. Scrollable.
    /// Content is rendered fresh each frame so it picks up the current
    /// `CARGO_PKG_VERSION` / `CARGO_PKG_AUTHORS` env vars.
    ///
    /// 1.2.5+: optional `logo` `StatefulProtocol` rendered as a
    /// banner above the text — populated from the embedded
    /// `logo.png` when the host terminal supports ratatui-image
    /// (kitty / iterm2 / sixel / unicode half-blocks). `None`
    /// when image-preview is disabled or the terminal can't
    /// negotiate a graphics protocol.
    Credits {
        scroll: usize,
        logo: Option<ratatui_image::protocol::StatefulProtocol>,
    },
    /// Ctrl+B I — current-book info panel: backup / artefacts paths,
    /// structural counts (chapters / subchapters / paragraphs /
    /// sentences / words), reading-time estimate, and rendered-PDF
    /// status. Content is recomputed each frame so the figures stay
    /// fresh as the user edits.
    BookInfo {
        scroll: usize,
    },
    /// Ctrl+B L — pick a different `llm.default` provider from the set
    /// configured in inkhaven.hjson. On commit we rewrite just the
    /// `default:` line of the HJSON file in place so user comments and
    /// the rest of the config survive.
    LlmPicker {
        providers: Vec<String>,
        cursor: usize,
        initial_default: String,
    },
    /// Ctrl+B P fired with the cursor inside `#image("…")`: pick a
    /// sibling Image node to insert. Filename gets inserted at the
    /// cursor (plus a closing `"` when the call had none).
    ImagePicker {
        entries: Vec<ImagePickerEntry>,
        cursor: usize,
        /// Tells `commit_image_picker` whether to append a `"` after
        /// the filename — true when the `#image(` call was unclosed.
        close_quote: bool,
    },
    /// Enter-on-Image preview using ratatui-image. The `proto` is a
    /// resize-aware StatefulProtocol scoped to one image; it's
    /// re-encoded each frame against the modal's current rect so a
    /// terminal resize Just Works. None when the picker isn't
    /// available — caller falls back to the status-line info path.
    ImagePreview {
        title: String,
        fs_rel: String,
        size_bytes: u64,
        proto: ratatui_image::protocol::StatefulProtocol,
    },
    /// Ctrl+V R (1.2.5+) — float a rasterised PNG of the open
    /// paragraph on top of the editor. `pages` holds one
    /// ratatui-image protocol per page of the compiled
    /// document; `current_page` selects which one renders.
    /// `body` + `settings` are captured so an `S` or `A`
    /// keypress can re-render at high DPI without re-prompting.
    RenderedPreview {
        title: String,
        body: String,
        settings: crate::typst_world::WorldSettings,
        pages: Vec<RenderedPageProto>,
        current_page: usize,
        /// 1.2.6+ — pixels-per-typst-point factor used for the
        /// current page set. Initialised to 2.0 (≈ 144 dpi)
        /// when the modal first opens; `+`/`=` boost by 0.5
        /// (capped at 6.0), `-`/`_` reduce by 0.5 (floored at
        /// 0.5). Each change re-runs `render_all` with the new
        /// PPI and replaces `pages` in-place. Save flow (S /
        /// A) keeps using its own default DPI — that's the
        /// "publish" copy, not the screen preview.
        ppi: f32,
    },
    /// Ctrl+B ] (editor), `g` (tree), or Ctrl+B } (search) —
    /// 1.2.5+ project-wide tag picker. Shows every tag in use
    /// across the project; keys depend on `target` (see
    /// `TagPickerTarget`).
    TagPicker {
        target: TagPickerTarget,
        all_tags: Vec<String>,
        cursor: usize,
        /// Multi-select state — only meaningful in `EditorParagraph`
        /// and `TreeSelection` modes. Stored as a `BTreeSet` for
        /// deterministic glyph rendering in the modal.
        selected: std::collections::BTreeSet<String>,
    },
    /// `A` from `TagPicker` — prompt for a new tag name. Enter
    /// adds the tag to the project-wide set AND keeps the
    /// underlying picker's selection state via `return_to`.
    TagAddPrompt {
        input: TextInput,
        return_to: Box<Modal>,
    },
    /// `D` from `TagPicker` — confirm + execute project-wide tag
    /// deletion. Removes the tag from every node that carries it.
    /// `affected` reports how many nodes will be touched.
    TagDeleteConfirm {
        tag: String,
        affected: usize,
        return_to: Box<Modal>,
    },
    /// `R` from `TagPicker` (1.2.6+) — project-wide rename of
    /// the cursor tag. Empty input cancels; Enter rewrites
    /// every node carrying `old_tag` to use the new name.
    /// Merges into an existing tag if the new name already
    /// exists in the project.
    TagRenamePrompt {
        input: TextInput,
        old_tag: String,
        affected: usize,
        return_to: Box<Modal>,
    },
    /// Enter from `TagPicker` in `Search` mode — show every
    /// paragraph that carries the chosen tag, with a typeable
    /// filter input that narrows the list.
    TagSearchResults {
        tag: String,
        filter: TextInput,
        all_results: Vec<ScriptPickerEntry>,
        cursor: usize,
    },
    /// F8 (1.2.6+) — floating typst-diagnostics list for the
    /// open paragraph. Pure UI: reads from `opened.typst_diagnostics`
    /// on every frame, no copy held. Enter on a row moves the
    /// editor cursor to the diagnostic's line/col.
    DiagnosticsList {
        cursor: usize,
    },
    /// Ctrl+V e (1.2.6+) — vertical event picker. `entries`
    /// is a chronological snapshot built at open-time
    /// (`open_event_picker`); the picker doesn't refresh
    /// while open. Enter jumps to the event paragraph.
    EventPicker {
        entries: Vec<EventPickerEntry>,
        cursor: usize,
        track_filter: Option<String>,
    },
    /// Ctrl+V t (1.2.6+) — swim-lane timeline view (Phase 2).
    /// Scope-aware: opens at the current paragraph's nearest
    /// Subchapter / Chapter / Book; up/down chords walk the
    /// tree. The modal builds its event snapshot at open
    /// time and rebuilds on scope changes — pure UI state
    /// the rest of the lifecycle.
    TimelineView {
        state: TimelineViewState,
    },
    /// `n` from `TimelineView` — title prompt for a new
    /// event at the cursor's tick. Enter commits to the
    /// store via the same path as `inkhaven event add`; Esc
    /// returns to the underlying TimelineView modal.
    TimelineNewEventPrompt {
        input: TextInput,
        book_id: Uuid,
        cursor_ticks: i64,
        track: Option<String>,
        return_to: Box<Modal>,
    },
    /// 1.2.6+ — `Ctrl+V Shift+I` on an event paragraph. One-line
    /// edit prompt for start / end / track, pipe-separated.
    /// Example pre-fill: `Sol 13 | Sol 14 | main`. Empty middle
    /// (`Sol 13 |  | main`) means "no end". Empty trailing track
    /// (`Sol 13 | Sol 14 |`) means "drop the track". Precision
    /// is re-derived from the start string each commit.
    TimelineEditEventPrompt {
        input: TextInput,
        event_id: Uuid,
    },
    /// 1.2.6+ — side-by-side diff review before a buffer-
    /// replacing AI apply lands. Built by `apply_inference`
    /// when `ai.diff_review_on_apply = true` (default) and
    /// the action is `Replace` or `ReplaceCorrected`. The
    /// user reviews and presses `a` (accept) / `r` (reject)
    /// / `e` (edit — accept and refocus the editor).
    AiDiffReview {
        before_lines: Vec<String>,
        after_lines: Vec<String>,
        action: InferenceAction,
        scroll: usize,
        /// 1.2.11+ — when `Some`, the apply-step
        /// creates a snapshot annotated with this
        /// string BEFORE replacing the buffer.
        /// Used by the rhythm-rewrite flow
        /// (`Ctrl+B Shift+M`) so the pre-rewrite
        /// state is preserved + labelled.  `None`
        /// for the existing grammar / critique
        /// paths.
        post_accept_snapshot: Option<String>,
        /// 1.2.11+ — total rendered row count after
        /// long-line wrapping.  Written by the
        /// renderer each frame (it owns the column
        /// width), read by the key handler to
        /// clamp scroll.  Defaults to 0; the
        /// handler falls back to source-line count
        /// until the first render populates it.
        wrapped_total: usize,
    },
    /// F5 (1.2.6+) — annotation prompt that pops before a new
    /// snapshot is committed. `body` is captured at open time
    /// so the user can keep typing in the editor without
    /// affecting what gets snapshotted. Enter commits with the
    /// typed annotation (empty allowed); Esc cancels.
    SnapshotAnnotation {
        input: TextInput,
        parent_id: Uuid,
        parent_title: String,
        body: Vec<u8>,
    },
    /// Ctrl+V W (1.2.5+) — story view: floating PNG of the
    /// current book's DOT graph, rendered via `layout-rs` +
    /// `resvg`. `proto` drives the ratatui-image widget;
    /// `png_bytes` is kept around so `S` can dump it to disk
    /// without re-running the layout.
    StoryView {
        book_title: String,
        width: u32,
        height: u32,
        png_bytes: Vec<u8>,
        proto: ratatui_image::protocol::StatefulProtocol,
    },
    /// `S` from `StoryView` — save-as picker for the rendered
    /// PNG. Same shape as `SaveRenderedPng`; Esc restores the
    /// `StoryView` modal via `return_to`.
    SaveStoryPng {
        input: TextInput,
        png_bytes: Vec<u8>,
        book_title: String,
        return_to: Box<Modal>,
    },
    /// `S` (current page) or `A` (all pages) from
    /// `RenderedPreview` — save-as path picker for the full-DPI
    /// PNG(s). Enter writes the file(s); Esc restores the
    /// underlying `RenderedPreview` so navigation state survives
    /// a cancelled save.
    SaveRenderedPng {
        input: TextInput,
        body: String,
        settings: crate::typst_world::WorldSettings,
        title: String,
        pages: PagesToSave,
        /// Stash of the underlying preview modal so Esc returns
        /// to it. Same `return_to: Box<Modal>` pattern the
        /// snapshot-diff picker uses.
        return_to: Box<Modal>,
    },
    /// Ctrl+B F (editor pane) — Typst function picker. The filter
    /// input narrows the baked-in list as the user types; Enter
    /// inserts `#<name>(|)` at the cursor with the editor cursor
    /// positioned between the parens (Phase 1 = markup-mode default).
    FunctionPicker {
        filter: TextInput,
        cursor: usize,
    },
    /// Ctrl+F in AI-fullscreen — query string entry for the chat-
    /// history search. Enter commits the query into
    /// `App::chat_search`; Esc cancels with no search.
    ChatSearchPrompt {
        input: TextInput,
    },
    /// Ctrl+B 1..7 — list paragraphs whose `status` matches the
    /// chord's target value (1 = Ready, 2 = Final, …, 7 = None),
    /// scoped to the tree cursor's enclosing branch (or the whole
    /// project when the cursor sits at the root). Actions inside the
    /// modal:
    ///   Enter → jump tree cursor + open the paragraph
    ///   r / R → cycle the highlighted paragraph's status forward
    ///           (if it no longer matches, the row disappears from
    ///           the list and the next one slides up)
    ///   - / Backspace → cycle status backward
    StatusFilter {
        status_label: &'static str,
        scope: String,
        entries: Vec<StatusFilterEntry>,
        cursor: usize,
    },
    /// F1 help-manual query. Asks a free-form question against the Help
    /// system book (RAG), then streams the constrained LLM answer into the
    /// AI pane. Esc cancels; Enter submits.
    HelpQuery {
        input: TextInput,
    },
    /// Find / replace prompt. `replace` is None for Ctrl+F search-only mode
    /// and Some for Ctrl+R replace mode (Tab switches focus between the two
    /// input fields when present).
    FindReplace {
        search_input: TextInput,
        replace_input: Option<TextInput>,
        focus_replace: bool,
    },
    SnapshotPicker {
        /// Kept for potential refresh ops after future snapshot mutations.
        #[allow(dead_code)]
        paragraph_id: Uuid,
        paragraph_title: String,
        snapshots: Vec<Snapshot>,
        cursor: usize,
    },
    /// 1.2.8+ — `Ctrl+V Shift+U` picker over the deleted-
    /// paragraph kill-ring. Renders each entry as title +
    /// original parent breadcrumb + first-line preview.
    /// Enter restores the cursor selection (removes from
    /// ring); Esc cancels. State carries only the cursor
    /// index — the entries live on `App.kill_ring`, read
    /// fresh each frame.
    KillRingPicker {
        cursor: usize,
    },
    /// 1.2.8+ — `Ctrl+Z o` floating shell pane.  Hosts an
    /// embedded nushell engine living on `App.shell_engine`;
    /// renders the turn buffer from `App.shell_history` +
    /// reads typed input from `input`.  `selection_mode`
    /// flips the pane into history-selection (Phase 6)
    /// where `↑↓` walks turns and `c` / `i` copy / insert.
    /// `scroll` is the line offset into the rendered turn
    /// buffer (anchored to the bottom by default so the
    /// most-recent output is visible).
    ShellPane {
        input: TextInput,
        /// Index into `App.shell_command_history` while the
        /// user is walking with Up/Down.  `None` when the
        /// user has typed something fresh (Down past the
        /// newest entry resets this to None and clears the
        /// input — same as the AI prompt history pattern).
        command_history_cursor: Option<usize>,
        /// Selection mode: false = normal shell, true =
        /// "history selection mode" where `↑↓` walks turns,
        /// `c` copies, `i` inserts the selected output into
        /// the editor as a typst raw block.  Toggled by
        /// `Ctrl+Z h`.
        selection_mode: bool,
        /// Cursor index into `App.shell_history` while in
        /// selection mode.  Ignored when `selection_mode =
        /// false`.
        selection_cursor: usize,
        /// Scroll position into the turn-list rendering
        /// (lines from the bottom).  `0` keeps the newest
        /// output flush with the bottom of the pane.
        /// PgUp / PgDown bump this in 10-line steps; Home
        /// jumps to the top of the buffer, End back to the
        /// newest output.  Reset to `0` whenever a new turn
        /// is appended (so fresh output is auto-visible).
        /// Render clamps to the valid range; the field
        /// itself is allowed to grow past max_scroll without
        /// being written back.
        scroll: usize,
        /// 1.2.8+ — when `true`, an overlay renders on top
        /// of the pane listing chord + command basics.
        /// Toggled by `Ctrl+B H` while inside the pane.
        /// Other keys (Esc, any key) dismiss the overlay.
        /// Engine + history + scroll + input cursor all
        /// preserve underneath.
        show_help: bool,
    },
    /// 1.2.8+ — full-screen HJSON config editor for
    /// `<project_root>/inkhaven.hjson`.  Reuses the same
    /// `tui-textarea` widget as the paragraph editor, but
    /// stripped of all per-paragraph chrome (no gutter
    /// markers, no typst diagnostics, no lexicon hits, no
    /// match overlays).  Saving writes the buffer to disk
    /// and — when the saved bytes differ from the bytes
    /// loaded on open — flips `restart_required` so the
    /// renderer pops a "config changed; restart inkhaven"
    /// overlay on top.  The overlay is informational
    /// only; closing the modal returns to the main editor
    /// (which keeps running with the OLD config until the
    /// user actually quits + relaunches).  Esc closes the
    /// modal; a status-line warning fires when closing with
    /// unsaved edits.
    HjsonEditor {
        textarea: TextArea<'static>,
        /// Bytes loaded from disk at open time.  Used to
        /// (a) detect "buffer dirty since open" for Esc
        /// warnings and (b) decide whether the restart
        /// overlay fires after Ctrl+S.
        original_content: String,
        /// Absolute path to the HJSON file.  Captured at
        /// open time so a `cd` in another modal doesn't
        /// re-target the save.
        path: PathBuf,
        /// Set after a save whose written bytes != the
        /// pre-open original.  Render uses this to draw
        /// the centered restart-required overlay.  Any key
        /// dismisses the overlay (keeping the modal open).
        restart_required: bool,
        /// Vertical scroll into the editor lines (in rows).
        /// tui-textarea handles cursor + selection state
        /// internally; we own scroll because the editor's
        /// `widget()` path doesn't fit the custom-render
        /// styling we apply.
        scroll_row: usize,
        /// Horizontal scroll into the editor lines.
        scroll_col: usize,
    },
    /// 1.2.8+ — Ctrl+Q confirmation modal.  Opened only
    /// when `editor.confirm_quit = true` in HJSON.
    /// Y / Enter proceeds with the existing
    /// `request_quit` flow; N / Esc cancels.  No fields —
    /// the modal is fully transient.
    ConfirmQuit,
    /// 1.2.9+ — TTS unavailable / disabled modal.  Opens
    /// when `Ctrl+B S` fires while either the feature is
    /// disabled in HJSON (`editor.tts.enabled = false`)
    /// or the TTS engine couldn't initialise on this
    /// platform.  `reason` is the user-facing text;
    /// `title` distinguishes "TTS disabled" from
    /// "TTS unavailable" in the modal header.  Any key
    /// dismisses.
    TtsUnavailable {
        title: String,
        reason: String,
    },
    /// 1.2.9+ — writing-streak heatmap modal (Ctrl+B
    /// Shift+G).  GitHub-style 13×7 grid of the last 91
    /// days of project-wide word deltas, plus the
    /// current streak, longest streak in the window,
    /// and per-month totals.  Data captured at open
    /// time so the modal doesn't re-query DuckDB every
    /// frame.  Esc closes.
    WritingStreakHeatmap {
        /// Project-wide word deltas, oldest first.
        /// Length 91 (13 weeks × 7 days).
        daily_words: Vec<i64>,
        /// Current consecutive-writing-days streak.
        streak_days: u32,
        /// Longest streak in the 91-day window.
        longest_streak: u32,
        /// Today (UTC) as `(year, month, day)` so the
        /// render can label months / paint today's
        /// cell with a marker without re-parsing.
        today_ymd: (i32, u32, u32),
    },
    /// 1.2.9+ — TTS save-as-audio path picker.  Opens
    /// when the user presses Ctrl+B Shift+R; the
    /// default path lands as `<project>/audio/<slug>.aiff`
    /// pre-filled in the input.  Enter spawns
    /// `say -o <path>` with the configured voice + rate;
    /// Esc cancels.  `body` carries the paragraph text;
    /// `voice` + `wpm` are captured at open time so a
    /// subsequent HJSON edit doesn't change what gets
    /// written.
    TtsSaveAsAudio {
        input: super::input::TextInput,
        body: String,
        voice: String,
        wpm: u16,
        voice_label: String,
    },
    /// 1.2.9+ — TTS playback modal.  Opens when
    /// `Ctrl+B S` successfully kicks off speech via
    /// `tts-rs`.  The actual TTS engine handle lives on
    /// `App.tts_engine` (lazy-init, reused across
    /// playbacks).  `started_at` drives the elapsed-time
    /// counter; `preview` is the first ~80 chars of the
    /// paragraph for the modal title.  The render loop
    /// polls `tts.is_speaking()` each frame and closes
    /// the modal automatically when playback ends.  Any
    /// key calls `tts.stop()` and closes the modal.
    TtsPlayback {
        started_at: std::time::Instant,
        preview: String,
        voice_label: String,
    },
    /// 1.2.9+ — sentence-rhythm gauge modal
    /// (Ctrl+B Shift+H).  Built once at open time
    /// from the open paragraph's body; the render
    /// loop reads cached stats and paints a per-
    /// sentence bar list + outlier callouts.  Esc
    /// closes; PgUp/PgDn/↑/↓ scroll the per-
    /// sentence list.
    SentenceRhythm {
        stats: super::sentence_rhythm::RhythmStats,
        scroll: usize,
    },
    /// 1.2.9+ — project-wide concordance modal
    /// (Ctrl+B Shift+L).  Built once at open time from
    /// the in-memory hierarchy + paragraph bodies; the
    /// render loop just slices `data.entries` against
    /// the live `filter` text.  Esc closes, arrow keys
    /// move the cursor, `s` toggles sort, typing
    /// narrows by prefix (or substring once a `*` is
    /// in the filter).
    Concordance {
        data: super::concordance::ConcordanceData,
        filter: super::input::TextInput,
        cursor: usize,
        scroll: usize,
        sort: super::concordance::SortMode,
        /// Cached "visible" view that mirrors `entries`
        /// under the current filter.  Rebuilt by
        /// `App::concordance_refilter` whenever filter
        /// text or sort mode change.  Stores indices
        /// into `data.entries`.
        visible: Vec<usize>,
    },
    /// 1.2.13+ Phase C.2 — `Ctrl+B Q` / `Ctrl+B Shift+Q`
    /// disambiguation picker.  Pops only when 2+ Language
    /// sub-books exist; with 0 the chord errors out, with
    /// exactly 1 the translation kicks off directly without
    /// the modal.  Carries the parsed source body + title
    /// + direction so the commit handler can spawn the
    /// inference without re-reading editor state (the editor
    /// may have moved between open and commit).
    TranslationLanguagePicker {
        /// (language-sub-book uuid, display name) pairs in
        /// canonical hierarchy order — matches the order the
        /// per-language sub-letter sub-chord will assume in
        /// a future iteration.
        entries: Vec<(Uuid, String)>,
        cursor: usize,
        direction: TranslationDirection,
        source_title: String,
        source_body: String,
    },
    /// 1.2.14+ Phase A.2 — `Ctrl+V Shift+H` picker.
    /// Lists every plot-thread paragraph under the
    /// `Threads` system book with summary columns.
    /// `↑↓` navigate; `Enter` open in editor;
    /// `Shift+Enter` pin to secondary; `w` open the
    /// weave-view sub-modal; `/` filter by typed
    /// substring; `Esc` close.
    ThreadsPicker {
        /// Summary rows materialised at open time so
        /// scrolling doesn't re-parse HJSON.  Built by
        /// `App::collect_thread_picker_entries`.
        entries: Vec<ThreadsPickerEntry>,
        cursor: usize,
        /// Substring filter typed via `/`.  Empty →
        /// every entry visible.  Match is
        /// case-insensitive against name + status +
        /// weight.
        filter: super::input::TextInput,
        /// True while `/` filter input is active —
        /// keystrokes go to the filter, `Enter` /
        /// `Esc` exit filter mode (not the modal).
        filter_active: bool,
        /// Cached indices into `entries` matching the
        /// current filter.  Rebuilt on filter edits.
        visible: Vec<usize>,
    },
    /// 1.2.14+ Phase C.2 — project-wide comments
    /// panel.  Materialises every paragraph's
    /// `.comments.json` sidecar at open time so
    /// scrolling is pure cursor math; resolve /
    /// delete / unresolve write through to disk
    /// IMMEDIATELY (no batch-on-close) so a
    /// crash mid-review doesn't lose decisions.
    /// Pops on `Ctrl+V Shift+C` from any pane.
    CommentsPanel {
        /// All comments, paragraph-grouped, in
        /// hierarchy / created-at order.
        entries: Vec<CommentsPanelEntry>,
        cursor: usize,
        /// Substring filter typed via `/`.  Empty
        /// → every visible entry.  Match is
        /// case-insensitive against the comment
        /// text + author + paragraph breadcrumb.
        filter: super::input::TextInput,
        /// True while `/` filter input is active —
        /// keystrokes go to the filter, `Enter` /
        /// `Esc` exit filter mode (not the
        /// modal).
        filter_active: bool,
        /// When true, resolved comments are
        /// hidden.  Toggled with `R`.  Default
        /// `true` — focused on actionable work.
        hide_resolved: bool,
        /// Cached indices into `entries` after
        /// applying the `filter` + `hide_resolved`
        /// pass.
        visible: Vec<usize>,
    },
    /// 1.2.14+ Phase Q.3 — `Ctrl+V f` inline
    /// footnote editor.  Multi-line text input
    /// (like `Modal::CommentEditor` but the body
    /// becomes `#footnote[…]` Typst markup or
    /// `[^id]` markdown at the cursor on commit
    /// instead of writing a sidecar).
    FootnoteEditor {
        textarea: tui_textarea::TextArea<'static>,
        paragraph_id: Uuid,
    },
    /// 1.2.14+ Phase Q.4 — `Ctrl+V Shift+G`
    /// project goal modal.  Materialises the
    /// word-count math at open time so the
    /// render path is pure read.  Esc-only —
    /// the modal doesn't mutate config.
    ProjectGoalModal {
        data: super::project_goal::ProjectGoalData,
    },
    /// 1.2.14+ Phase Q.4 — `Ctrl+V y` style
    /// transfer picker.  Pops first; on a
    /// commit, the source paragraph is rewritten
    /// in the picked reference paragraph's style
    /// via the AI pane stream.
    StyleTransferPicker {
        entries: Vec<(Uuid, String)>,
        cursor: usize,
        filter: super::input::TextInput,
        filter_active: bool,
        visible: Vec<usize>,
        /// Paragraph being rewritten — UUID
        /// captured at open time so a mid-flight
        /// paragraph switch doesn't confuse the
        /// commit handler.
        target_paragraph_id: Uuid,
    },
    /// 1.2.14+ Phase C.1 — comment editor.  Pops on
    /// `Ctrl+V c` once the anchor span has been
    /// resolved (selection range or word-at-cursor).
    /// Multi-line TextArea for the comment body;
    /// `Ctrl+S` / `Esc` commit, `Esc` (when buffer
    /// empty) cancels.  On commit, a new `Comment`
    /// is appended to the open paragraph's sidecar
    /// JSON file with the anchor span, current
    /// author, and `created_at: now`.
    CommentEditor {
        /// Multi-line buffer for the comment body.
        textarea: tui_textarea::TextArea<'static>,
        /// Character span the comment anchors to in
        /// the underlying paragraph body.
        anchor_start: usize,
        anchor_end: usize,
        /// Snippet of the anchor span's text (≤80
        /// chars) for the modal header so the
        /// author sees what they're commenting on.
        anchor_preview: String,
        /// Paragraph UUID the comment belongs to —
        /// used to dispatch the save to the right
        /// OpenedDoc when the modal commits.
        paragraph_id: Uuid,
    },
    /// 1.2.14+ Phase D.4 — TUI thread doctor.
    /// Matches `inkhaven thread doctor` output:
    /// status + weight distributions + average
    /// tension + three blind-spot detector
    /// passes.  Read-only; `Esc` closes.
    ThreadDoctor {
        data: ThreadDoctorData,
    },
    /// 1.2.14+ Phase A.2 — swim-lane weave view.
    /// Pushed by `w` from inside `ThreadsPicker`;
    /// `Esc` returns to the picker (stored in
    /// `return_to`).  Rendered as a row-per-thread
    /// table with one column per Chapter across every
    /// user book; each cell shows a count of
    /// paragraphs in that chapter that link to the
    /// thread, with `Enter` jumping to the first
    /// linking paragraph in the cell.
    ThreadWeaveView {
        /// Threads down the side, in the same order
        /// the picker showed them.
        threads: Vec<ThreadsPickerEntry>,
        /// Chapter columns across the top, in
        /// canonical hierarchy order.  `(chapter_id,
        /// book_title, chapter_title)`.
        chapters: Vec<(Uuid, String, String)>,
        /// `grid[thread_idx][chapter_idx]` is the set
        /// of paragraph UUIDs in that chapter that
        /// link to that thread.  Pre-computed at
        /// open time so navigation is pure cursor
        /// math.
        grid: Vec<Vec<Vec<Uuid>>>,
        cursor_row: usize,
        cursor_col: usize,
        scroll_row: usize,
        scroll_col: usize,
        return_to: Box<Modal>,
    },
}

/// 1.2.14+ Phase C.2 — one row of the project-
/// wide comments panel.  Snapshots the comment
/// data + the surrounding paragraph metadata so
/// the panel can render + navigate without
/// re-walking the hierarchy / re-reading
/// sidecars per keystroke.
#[derive(Debug, Clone)]
pub(super) struct CommentsPanelEntry {
    pub paragraph_id: Uuid,
    /// Paragraph slug-path breadcrumb shown in the
    /// panel ("manuscript-en/chapter-3/03-rain").
    pub paragraph_breadcrumb: String,
    /// Absolute path to the paragraph's `.typ` —
    /// resolved once at open time so the resolve /
    /// delete dispatchers can re-load + re-write
    /// the sidecar without walking the hierarchy
    /// again.
    pub typ_abs_path: std::path::PathBuf,
    /// Index of this comment inside the sidecar's
    /// `comments` Vec.  Used by the
    /// resolve / delete dispatchers to locate the
    /// right comment when the sidecar is
    /// reloaded.
    pub comment_index: usize,
    pub author: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub resolved: bool,
    pub text: String,
    pub char_start: usize,
    /// Span end character offset.  Reserved for a
    /// future "open paragraph + SELECT span" jump
    /// (Enter currently positions the cursor at
    /// char_start; it doesn't select the range).
    #[allow(dead_code)]
    pub char_end: usize,
    /// One-indexed position of this comment among
    /// its paragraph's comments (`(2, 5)` means
    /// the 2nd of 5).  Drives the per-row
    /// `(N/M in ¶)` dense indicator in the panel.
    pub paragraph_position: usize,
    pub paragraph_total_comments: usize,
}

/// 1.2.14+ Phase D.4 — TUI thread doctor
/// snapshot.  Computed at modal-open time;
/// renderer is pure read.  Identical math to the
/// CLI `inkhaven thread doctor`.
#[derive(Debug, Clone)]
pub(super) struct ThreadDoctorData {
    pub thread_count: usize,
    pub avg_tension: f32,
    pub status_distribution: Vec<(String, usize)>,
    pub weight_distribution: Vec<(String, usize)>,
    pub zero_links: Vec<String>,
    pub payoff_unfired: Vec<String>,
    pub dormant: Vec<String>,
}

/// 1.2.14+ Phase A.2 — one row of the Threads
/// picker.  Mirrors the summary fields the CLI
/// `inkhaven thread list` reads.  Built once at
/// picker-open time + cached for the lifetime of
/// the modal so HJSON parsing happens only once
/// per session.
#[derive(Debug, Clone)]
pub(super) struct ThreadsPickerEntry {
    pub id: Uuid,
    /// Paragraph slug-derived title — what the tree
    /// pane shows.
    pub name: String,
    /// `title:` field from the HJSON body.  Falls
    /// back to `name` when the HJSON lacks one.
    pub title_field: String,
    pub status: String,
    pub weight: String,
    pub tension: i32,
    pub character_count: usize,
    pub place_count: usize,
    /// Reverse-link count — paragraphs anywhere in
    /// the project whose `linked_paragraphs`
    /// includes this thread's UUID.
    pub link_count: usize,
}

/// 1.2.13+ Phase C.2 — direction the translation flow runs in.
/// `ToInvented` is the headline `Ctrl+B Q` ("translate INTO the
/// invented language for the manuscript"); `FromInvented` is the
/// reverse-direction `Ctrl+B Shift+Q` for roundtrip testing
/// ("does the LLM understand its own grammar?").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TranslationDirection {
    /// Working language → invented language.
    ToInvented,
    /// Invented language → working language.
    FromInvented,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(title: &str, ticks: i64, track: Option<&str>) -> EventPickerEntry {
        EventPickerEntry {
            id: Uuid::nil(),
            title: title.into(),
            start_ticks: ticks,
            start_str: format!("{ticks}"),
            glyph: "●".into(),
            track: track.map(str::to_owned),
            is_orphan: false,
        }
    }

    #[test]
    fn filter_none_passes_all() {
        let es = vec![
            entry("A", 0, Some("main")),
            entry("B", 1, Some("flashback")),
        ];
        assert_eq!(visible_event_entries(&es, None).len(), 2);
    }

    #[test]
    fn filter_track_case_insensitive() {
        let es = vec![
            entry("A", 0, Some("main")),
            entry("B", 1, Some("flashback")),
            entry("C", 2, None),
        ];
        let hits = visible_event_entries(&es, Some("MAIN"));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "A");
    }
}
