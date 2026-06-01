//! Chord-action binding table.
//!
//! Stage 1 of the rebindable-keys roadmap: extract every meta- and
//! bund-sub-chord from the hardcoded `match` arms in `app.rs` into
//! a data-driven `KeyBindings` struct. App dispatch becomes a
//! single table lookup followed by a `run_action` switch.
//!
//! ## What's here (Stage 1)
//!
//! * `Action` — one variant per reachable handler. Names are
//!   `snake_case` so they serialise to dotted strings in HJSON
//!   (`tree.add_chapter`, `bund.run_buffer`, …).
//! * `Scope` — pane filter on each binding entry.
//! * `BindingEntry` — `(chord, action, scope)` triple.
//! * `KeyBindings::defaults()` — produces the canonical table
//!   matching today's hardcoded chord layout exactly.
//! * `KeyBindings::resolve_*` — table lookups consulted by
//!   `handle_meta_action` / `handle_bund_action`.
//!
//! ## What's not here yet (Stage 2)
//!
//! * `ink.key.*` Bund stdlib for runtime rebinding.
//! * Auto-generated status-bar hint strings.
//! * Migration of F-keys (F1/F3/F4/F5/F6/F7) into the table.

use crossterm::event::KeyEvent;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock};

use super::focus::Focus;
use super::keymap::KeyChord;

/// Which pane(s) a binding applies in. The first binding whose
/// scope matches the current focus wins, so narrow-scoped entries
/// (`Editor`) MUST come before broad ones (`Any`) in
/// `KeyBindings::defaults()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// Active in any pane.
    Any,
    /// Editor pane only.
    Editor,
    /// Tree pane + the search bar (which lives above the tree).
    Tree,
    /// AI pane + the AI prompt input line.
    Ai,
}

impl Scope {
    pub fn matches(self, focus: Focus) -> bool {
        match self {
            Scope::Any => true,
            Scope::Editor => focus == Focus::Editor,
            Scope::Tree => matches!(focus, Focus::Tree | Focus::SearchBar),
            Scope::Ai => matches!(focus, Focus::Ai | Focus::AiPrompt),
        }
    }
}

/// Every user-reachable chord-action. New chord features add a
/// variant here + an arm in `App::run_action`. Variant names
/// serialise (via serde) to the canonical dotted form used in
/// HJSON `keys.bindings`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    // ── Tree pane ─────────────────────────────────────────────
    #[serde(rename = "tree.add_book")]
    AddBook,
    #[serde(rename = "tree.add_chapter")]
    AddChapter,
    #[serde(rename = "tree.add_subchapter")]
    AddSubchapter,
    #[serde(rename = "tree.add_paragraph")]
    AddParagraph,
    #[serde(rename = "tree.delete_node")]
    DeleteNode,
    #[serde(rename = "tree.morph_type")]
    MorphType,
    #[serde(rename = "tree.reorder_up")]
    ReorderUp,
    #[serde(rename = "tree.reorder_down")]
    ReorderDown,

    // ── Editor pane ───────────────────────────────────────────
    #[serde(rename = "editor.save")]
    Save,
    #[serde(rename = "editor.create_snapshot")]
    CreateSnapshot,
    #[serde(rename = "editor.cycle_status")]
    CycleStatus,
    #[serde(rename = "editor.open_function_picker")]
    OpenFunctionPicker,
    #[serde(rename = "editor.rename_to_first_sentence")]
    RenameToFirstSentence,
    /// `P` in the editor — context-sensitive: image-picker when
    /// the cursor sits inside `#image(...)`, otherwise Places
    /// lexicon lookup.
    #[serde(rename = "editor.lookup_places_or_image")]
    LookupPlacesOrImage,
    #[serde(rename = "editor.lookup_characters")]
    LookupCharacters,
    #[serde(rename = "editor.lookup_notes")]
    LookupNotes,
    #[serde(rename = "editor.lookup_artefacts")]
    LookupArtefacts,
    #[serde(rename = "editor.open_quickref")]
    OpenQuickref,

    // ── Global meta ───────────────────────────────────────────
    #[serde(rename = "global.open_credits")]
    OpenCredits,
    #[serde(rename = "global.open_book_info")]
    OpenBookInfo,
    #[serde(rename = "global.open_llm_picker")]
    OpenLlmPicker,
    #[serde(rename = "global.toggle_sound")]
    ToggleSound,
    #[serde(rename = "global.schedule_assemble")]
    ScheduleAssemble,
    #[serde(rename = "global.schedule_build")]
    ScheduleBuild,
    #[serde(rename = "global.schedule_take")]
    ScheduleTake,
    /// 1.2.6+ — `Ctrl+B Shift+B`. Trigger an immediate project
    /// backup, ignoring the recency cooldown the exit hook uses.
    /// Honours `backup.wait_for_key_after_backup`.
    #[serde(rename = "global.backup_now")]
    BackupNow,
    #[serde(rename = "global.toggle_typewriter")]
    ToggleTypewriter,
    #[serde(rename = "global.toggle_ai_fullscreen")]
    ToggleAiFullscreen,
    #[serde(rename = "global.status_filter_ready")]
    StatusFilterReady,
    #[serde(rename = "global.status_filter_final")]
    StatusFilterFinal,
    #[serde(rename = "global.status_filter_third")]
    StatusFilterThird,
    #[serde(rename = "global.status_filter_second")]
    StatusFilterSecond,
    #[serde(rename = "global.status_filter_first")]
    StatusFilterFirst,
    #[serde(rename = "global.status_filter_napkin")]
    StatusFilterNapkin,
    #[serde(rename = "global.status_filter_none")]
    StatusFilterNone,

    /// Ctrl+B ] (1.2.5+) — open the tag picker for the currently
    /// open paragraph. Inside: A adds a new tag, D deletes a tag
    /// project-wide, Space selects, T applies selected tags.
    #[serde(rename = "global.tag_paragraph")]
    TagParagraph,
    /// Ctrl+B } (1.2.5+) — open the search-by-tag picker. Enter
    /// on a tag lists paragraphs that carry it, with a filter
    /// input; Enter on a paragraph opens it in the editor.
    #[serde(rename = "global.tag_search")]
    TagSearch,

    // ── AI pane ───────────────────────────────────────────────
    #[serde(rename = "ai.clear_chat")]
    ClearChat,

    // ── Bund prefix ───────────────────────────────────────────
    #[serde(rename = "bund.run_buffer")]
    BundRunBuffer,
    #[serde(rename = "bund.new_script")]
    BundNewScript,
    #[serde(rename = "bund.open_eval_modal")]
    BundOpenEvalModal,
    /// Ctrl+Z ? — open the script picker. Lists scripts in the
    /// cursor's branch; `A` toggles to the `Scripts` system book.
    #[serde(rename = "bund.open_script_picker")]
    BundOpenScriptPicker,

    // ── Top-level (1.2.4+ F-key migration) ────────────────────
    /// F1 anywhere — open the Help-book query modal.
    #[serde(rename = "help.query")]
    HelpQuery,
    /// F2 in Tree — rename the cursor's node.
    #[serde(rename = "tree.rename")]
    RenameNode,
    /// F3 in Tree — file picker, import context.
    #[serde(rename = "tree.file_picker_import")]
    FilePickerTreeImport,
    /// F3 in Editor — file picker, "load into buffer" context.
    #[serde(rename = "editor.file_picker_load")]
    FilePickerEditorLoad,
    /// F4 in Editor — toggle split-edit mode.
    #[serde(rename = "editor.toggle_split")]
    ToggleSplit,
    /// Shift+F4 (1.2.12+) — toggle the full-screen
    /// two-paragraph split-view layout.  Left pane is
    /// the current primary buffer; right pane is the
    /// `App.secondary` slot (populated by pickers in
    /// Phase B).  Tab swaps focus left ↔ right in
    /// split-view; tree + AI response panes are
    /// hidden while split-view is active.  The AI
    /// prompt input bar still spans the bottom so
    /// `Ctrl+I` calls work from either pane.  F4
    /// (same-paragraph snapshot split) and Ctrl+F4
    /// (accept-snapshot) are untouched.  See
    /// `Documentation/PROPOSALS/SPLIT_VIEW.md`.
    #[serde(rename = "editor.toggle_split_view")]
    ToggleSplitView,
    /// Ctrl+V Shift+B (1.2.12+) — sibling-book lookup
    /// for the split-view secondary pane.  Given the
    /// open paragraph's slug, walks the project's
    /// hierarchy looking for paragraphs with the same
    /// slug under a *different* top-level book.
    ///
    ///   * Zero matches → status message names the
    ///     slug we tried.
    ///   * Exactly one match → auto-pin to the
    ///     `secondary` slot (the user typically
    ///     follows up with Shift+F4 to view the
    ///     split).
    ///   * Two or more matches → open a fuzzy
    ///     paragraph picker scoped to the matches
    ///     so the user picks which translation /
    ///     mirror to compare.
    ///
    /// Primary translation-workflow chord: if you're
    /// in `manuscript-en/03-rain`, this finds
    /// `manuscript-ru/03-rain` and pins it next to
    /// you.  See `Documentation/PROPOSALS/SPLIT_VIEW.md`
    /// §10 Phase D.
    #[serde(rename = "view.sibling_book_lookup")]
    ViewSiblingBookLookup,
    /// Ctrl+V Shift+H.  Open the
    /// Threads picker: list every plot-thread paragraph
    /// under the `Threads` system book with status /
    /// weight / tension / link-count columns.  `↑↓`
    /// navigate; `Enter` opens the entry; `Shift+Enter`
    /// pins to the split-view secondary slot; `w`
    /// opens the swim-lane weave view sub-modal; `/`
    /// filters by typed substring; `Esc` closes.  See
    /// `Documentation/PROPOSALS/1.2.14_PLAN.md` §3.
    #[serde(rename = "view.threads_picker")]
    ViewThreadsPicker,
    /// Ctrl+V Shift+A.  AI
    /// thread audit.  Resolves the cursor's scope
    /// from the F9 AiMode (chapter / subchapter /
    /// book; defaults to chapter when AiMode is
    /// None / Selection / Paragraph).  Composes a
    /// prompt envelope reading every Thread
    /// paragraph's HJSON + a blind-spots pre-pass
    /// (link counts, payoff-marked threads whose
    /// payoff hasn't fired) + the scope's
    /// paragraph contents.  Streams into the AI
    /// pane.  See `Documentation/PROPOSALS/1.2.14_PLAN.md`
    /// §3.4.
    #[serde(rename = "ai.thread_audit")]
    AiThreadAudit,
    /// Ctrl+V Shift+D.  TUI
    /// version of `inkhaven thread doctor`: pops a
    /// modal showing the thread status / weight
    /// distributions + blind-spot detector output
    /// (ZERO LINKS / PAYOFF UNFIRED / DORMANT).
    /// Identical math to the CLI; same per-
    /// detector labels.
    #[serde(rename = "view.thread_doctor")]
    ViewThreadDoctor,
    /// Ctrl+V c.  Anchor an
    /// inline comment to the selection (or the
    /// word at the cursor when no selection is
    /// active).  Pops a multi-line text input
    /// modal for the comment body; on commit, the
    /// sidecar `<paragraph>.comments.json` file is
    /// written alongside the paragraph's `.typ` so
    /// the comment travels with the prose in git.
    /// Character-offset spans (not byte) so UTF-8
    /// boundary edits don't break anchoring.  See
    /// `Documentation/PROPOSALS/1.2.14_PLAN.md`
    /// §4.
    #[serde(rename = "view.add_comment")]
    ViewAddComment,
    /// Ctrl+V Shift+C.  Open
    /// the project-wide comments panel.  Lists
    /// every comment from every paragraph's
    /// sidecar JSON file with author / age /
    /// breadcrumb / text-snippet columns.
    /// Filter, resolve, reopen, delete, jump to
    /// source paragraph.  See
    /// `Documentation/PROPOSALS/1.2.14_PLAN.md`
    /// §4.4.
    #[serde(rename = "view.comments_panel")]
    ViewCommentsPanel,
    /// Ctrl+V d.  AI
    /// continuation drafting.  "Continue this
    /// paragraph in my voice" — the prompt
    /// envelope sends the previous N paragraphs
    /// as voice anchors + the open paragraph's
    /// existing text with the cursor position
    /// marked.  Response wrapped in `<<<DRAFT>>>`
    /// / `<<<END>>>` markers; the AI pane's `I`
    /// apply lifts only the draft block at the
    /// cursor.
    #[serde(rename = "ai.continuation_draft")]
    AiContinuationDraft,
    /// Ctrl+V f.  Insert an
    /// inline footnote at the cursor.  Pops a
    /// multi-line text input modal for the
    /// footnote body; on commit, inserts
    /// `#footnote[<body>]` (Typst — the default)
    /// or `[^id]` + a `[^id]: <body>` line
    /// (markdown — when `editor.footnote_style =
    /// "markdown"`).
    #[serde(rename = "editor.insert_footnote")]
    EditorInsertFootnote,
    /// Ctrl+V Shift+G.
    /// Project-level word-count goal +
    /// projection modal.
    #[serde(rename = "view.project_goal_modal")]
    ViewProjectGoalModal,
    /// Ctrl+V y.  Style
    /// transfer rewrite: pick a reference
    /// paragraph; AI rewrites the open paragraph
    /// in that style.  Response wrapped in
    /// `<<<REWRITE>>>` / `<<<END>>>` markers.
    #[serde(rename = "ai.style_transfer_rewrite")]
    AiStyleTransferRewrite,
    /// Ctrl+F4 in Editor — accept the snapshot pane into the
    /// live buffer.
    #[serde(rename = "editor.accept_split_snapshot")]
    AcceptSplitSnapshot,
    /// F6 in Editor — open the snapshot picker.
    #[serde(rename = "editor.snapshot_picker")]
    OpenSnapshotPicker,
    /// F7 in Editor — grammar check the open paragraph.
    #[serde(rename = "editor.grammar_check")]
    GrammarCheck,
    /// F9 anywhere — cycle AI scope mode.
    #[serde(rename = "ai.cycle_mode")]
    CycleAiMode,
    /// F10 anywhere — toggle inference mode (Local ↔ Full).
    #[serde(rename = "ai.toggle_inference_mode")]
    ToggleInferenceMode,
    /// F8 (1.2.6+) — open the floating typst-diagnostics list
    /// modal. Lists every parse / semantic diagnostic in the
    /// open paragraph with line:col + message; Enter jumps the
    /// editor cursor.
    #[serde(rename = "editor.diagnostics_list")]
    DiagnosticsList,
    /// Ctrl+F12 (1.2.6+) — send the typst diagnostic at the
    /// cursor (or nearest one, with surrounding context) to
    /// the AI pane with an explain-or-fix prompt. Started life
    /// on bare F11 but macOS grabs F11 globally (Show Desktop
    /// / Mission Control) so the chord never reached the TUI.
    #[serde(rename = "editor.explain_diagnostic")]
    ExplainDiagnostic,
    /// F12 (1.2.6+) — AI critique of the open paragraph. In the
    /// editor: "what's weak" prompt. In split-edit (F4) mode:
    /// "evaluate changes" prompt. Prompt resolution: Prompts
    /// book paragraph → prompts.hjson entry → embedded default.
    #[serde(rename = "editor.critique")]
    Critique,

    // ── View prefix (1.2.4+, default Ctrl+V) ──────────────────
    /// Ctrl+V 1 (Editor) — write the open paragraph's live buffer
    /// as markdown to cwd.
    #[serde(rename = "view.export_markdown_buffer")]
    ViewExportMarkdownBuffer,
    /// Ctrl+V 2 (Editor) — write the containing subchapter's
    /// subtree as markdown to cwd.
    #[serde(rename = "view.export_markdown_subchapter")]
    ViewExportMarkdownSubchapter,
    /// Ctrl+V 1 (Tree) — write the tree-cursor's node + descendants
    /// as markdown to cwd.
    #[serde(rename = "view.export_markdown_subtree")]
    ViewExportMarkdownSubtree,
    /// Ctrl+V S — toggle similar-paragraph mode (vector-similarity
    /// picker, side-by-side editor).
    #[serde(rename = "view.toggle_similar_mode")]
    ViewToggleSimilarMode,
    /// Ctrl+V G — open the writing-progress modal.
    #[serde(rename = "view.open_progress")]
    ViewOpenProgress,
    /// Ctrl+V T — open the per-paragraph target-words input modal.
    #[serde(rename = "view.open_paragraph_target")]
    ViewOpenParagraphTarget,
    /// Ctrl+V A — switch the tree pane into "select paragraph
    /// to link" mode. Enter on a paragraph adds it to the open
    /// paragraph's `linked_paragraphs`.
    #[serde(rename = "view.add_link")]
    ViewAddLink,
    /// Ctrl+V I — reverse of `view.add_link`. Tree pane picker;
    /// Enter on a paragraph adds the OPEN paragraph to THAT
    /// paragraph's outgoing links (creates an incoming link
    /// for current).
    #[serde(rename = "view.add_incoming_link")]
    ViewAddIncomingLink,
    /// Ctrl+V L — open the linked-paragraphs floating modal
    /// (`D` removes a link).
    #[serde(rename = "view.list_links")]
    ViewListLinks,
    /// Ctrl+V K — open the backlinks floating modal. Reverse of
    /// `view.list_links`: shows paragraphs whose
    /// `linked_paragraphs` contains the open paragraph.
    #[serde(rename = "view.list_backlinks")]
    ViewListBacklinks,
    /// Ctrl+V B — toggle bookmark on the open paragraph.
    #[serde(rename = "view.toggle_bookmark")]
    ViewToggleBookmark,
    /// Ctrl+V M — open the bookmark picker.
    #[serde(rename = "view.list_bookmarks")]
    ViewListBookmarks,
    /// Ctrl+V P — fuzzy paragraph picker (1.2.4+).
    #[serde(rename = "view.fuzzy_paragraph_picker")]
    ViewFuzzyParagraphPicker,
    /// Ctrl+V Shift+P (1.2.7+) — same fuzzy paragraph
    /// picker but entries are sorted by `modified_at` desc.
    /// Answers "what did I work on most recently?" without
    /// trawling the tree pane.
    #[serde(rename = "view.recent_paragraph_picker")]
    ViewRecentParagraphPicker,
    /// Ctrl+V Shift+U (1.2.8+) — picker over the deleted-
    /// paragraph kill-ring. Ctrl+B U restores the most-
    /// recent entry; this chord opens a modal to choose
    /// any of the (up to 10) buffered recoveries.
    #[serde(rename = "view.kill_ring_picker")]
    ViewKillRingPicker,
    /// Ctrl+V h (1.2.8+) — one-shot report on the open
    /// paragraph's "hidden" characters: tab count, trailing-
    /// whitespace line count, CR count. Status-bar only;
    /// no buffer rewrite. The visual editor overlay is
    /// scheduled for 1.2.9 once the column-bookkeeping
    /// integrates cleanly with the existing match /
    /// lexicon highlight layers.
    #[serde(rename = "view.hidden_chars_report")]
    ViewHiddenCharsReport,
    /// Ctrl+V Shift+S (1.2.8+) — show the cursor row's
    /// breadcrumb path on the status bar: "Book ▸ Chapter
    /// ▸ Subchapter ▸ Paragraph". Pane-aware: in the tree
    /// it walks from the tree cursor; in the editor it
    /// walks from the open paragraph.
    #[serde(rename = "view.show_breadcrumb")]
    ViewShowBreadcrumb,
    /// Ctrl+Z o (1.2.8+) — open or close the embedded
    /// nushell pane.  Closing preserves the engine state
    /// (env vars, defs) and the turn buffer for the next
    /// open.  No-op (with a status hint) when
    /// `shell.enabled = false` in HJSON.
    #[serde(rename = "bund.open_shell")]
    BundOpenShell,
    /// Ctrl+Z O (Shift, 1.2.8+) — drop the cached shell
    /// engine + turn buffer and open a fresh shell.  Use
    /// when the env / scope has drifted into a confusing
    /// state.
    #[serde(rename = "bund.open_shell_fresh")]
    BundOpenShellFresh,
    /// Ctrl+Z h (1.2.8+) — inside the shell pane, toggle
    /// history-selection mode.  ↑↓ walks turn-by-turn,
    /// `c` copies the highlighted turn's output to the
    /// system clipboard, `i` inserts it into the editor
    /// wrapped in the configured typst-box template.
    /// Re-press exits selection mode.
    #[serde(rename = "bund.shell_selection")]
    BundShellSelection,
    /// Ctrl+B 0 (1.2.8+) — open the project's
    /// `inkhaven.hjson` in a full-screen editor modal.
    /// Syntax-highlighted via `hjson_highlight`.  Save
    /// with Ctrl+S; if the saved bytes differ from the
    /// loaded bytes, a "restart inkhaven" overlay pops
    /// (config changes apply on next launch).  Was bound
    /// to `|` originally; reassigned to `0` because some
    /// terminals don't forward Shift+\ through the
    /// chord-prefix state machine.
    #[serde(rename = "bund.edit_project_hjson")]
    BundEditProjectHjson,
    /// Ctrl+B S in editor scope (1.2.9+) — read the open
    /// paragraph aloud via the OS TTS engine (`tts-rs`).
    /// Replaces the pre-1.2.9 editor-scope `Ctrl+B S =
    /// Save` binding which was a redundant duplicate of
    /// plain `Ctrl+S`.  Gated on
    /// `editor.tts.enabled = true` in HJSON; when
    /// disabled, fires a "TTS disabled" explanation
    /// modal.  On platforms where the TTS engine can't
    /// initialise (Linux without `speech-dispatcher`,
    /// containers, …), a similar explanation modal
    /// fires with the engine-level error string.
    #[serde(rename = "editor.tts_read_paragraph")]
    TtsReadParagraph,
    /// Ctrl+B Shift+R (1.2.9+) — save the open paragraph
    /// as an audio file via macOS `say -o <path>`.
    /// Opens a save-as picker pre-filled with
    /// `<project>/audio/<paragraph-slug>.aiff`; Enter
    /// commits, Esc cancels.  Output format is AIFF
    /// (native to `say`); the user can change the
    /// extension to coerce a different container — `.m4a`
    /// / `.wav` work on recent macOS.
    #[serde(rename = "editor.tts_save_as_audio")]
    TtsSaveAsAudio,
    /// 1.2.15+ Phase D.3 — `Ctrl+B Shift+0` opens
    /// the project-wide doctor panel.  Paired
    /// mnemonically with `Ctrl+B 0` (HJSON config
    /// editor): the digit-0 row is the "system
    /// inspection" cluster.  Triggers a sync
    /// project scan + presents the findings;
    /// cursor-driven `r` / `R` apply repairs.
    #[serde(rename = "view.doctor_panel")]
    OpenDoctorPanel,
    /// Ctrl+B Shift+G (1.2.9+) — open the writing-
    /// streak heatmap modal.  Shows the last 91 days
    /// of project-wide word deltas as a GitHub-style
    /// 13×7 grid, with current streak + longest
    /// streak + monthly totals at the bottom.  Esc
    /// closes.
    #[serde(rename = "view.open_writing_streak_heatmap")]
    OpenWritingStreakHeatmap,
    /// Ctrl+B < (1.2.9+) — jump the editor cursor to the
    /// previous scene-break line in the open paragraph.
    /// A "scene break" is a typographic divider line
    /// like `* * *`, `***`, `---`, `___`, `###`, `~~~`,
    /// or a lone `§`.  No match → status warns "no scene
    /// break above" and the cursor doesn't move.
    #[serde(rename = "editor.scene_break_prev")]
    SceneBreakPrev,
    /// Ctrl+B > (1.2.9+) — jump to the next scene-break
    /// line.  Same detector as `SceneBreakPrev`.
    #[serde(rename = "editor.scene_break_next")]
    SceneBreakNext,
    /// Ctrl+B Shift+F (1.2.9+) — toggle the inline
    /// style-warning overlays (filter words today;
    /// repeated phrases / show-don't-tell / etc. as
    /// they land).  Session-local toggle on top of the
    /// HJSON `editor.style_warnings.enabled` master
    /// switch — flip the chord during a writing
    /// session without rewriting config.
    #[serde(rename = "editor.toggle_style_warnings")]
    ToggleStyleWarnings,
    /// Ctrl+B Shift+T (1.2.9+) — AI-driven show-
    /// don't-tell scan of the open paragraph.  Sends
    /// the paragraph to the configured LLM with a
    /// system prompt asking for telling passages and
    /// suggested rewrites.  The response streams
    /// into the AI pane.  The mnemonic is `T` for
    /// "tell".  Complements the always-on regex
    /// overlay (`editor.style_warnings.show_dont_tell`)
    /// with deeper analysis.
    #[serde(rename = "ai.analyse_show_dont_tell")]
    AnalyseShowDontTell,
    /// Ctrl+B Shift+H (1.2.9+) — open the sentence-
    /// rhythm gauge modal for the open paragraph.
    /// Splits prose into sentences, computes word-
    /// count mean / stdev / coefficient of variation
    /// (CV), maps CV to a verdict (Monotone / Steady
    /// / Varied / Choppy), and shows a per-sentence
    /// bar chart + outlier callouts.  The H is for
    /// "heartbeat" — the felt rhythm of the prose.
    #[serde(rename = "view.open_sentence_rhythm")]
    OpenSentenceRhythm,
    /// Ctrl+B Shift+M (1.2.11+) — AI-driven sentence-
    /// rhythm rewrite of the open paragraph.  Sends
    /// the paragraph body to the LLM with a prompt
    /// asking it to break up monotonous rhythm
    /// (the `Ctrl+B Shift+H` gauge's MONOTONE verdict
    /// has the same target).  When the stream
    /// completes, auto-opens an AI diff modal so
    /// the user can review the rewrite line by
    /// line.  On accept, a snapshot is created with
    /// annotation "Sentence rhythm rewrite" before
    /// the buffer is replaced; on reject, nothing
    /// changes.  Mnemonic: M for "Modulate" /
    /// "Mix it up".  Prompt resolution follows the
    /// standard pattern (Prompts book →
    /// `prompts.hjson` → embedded fallback).
    /// Multilingual via the project's `language`
    /// setting.  Also fires from inside the
    /// `Ctrl+B Shift+H` rhythm-gauge modal — the
    /// natural diagnose-then-rewrite path: open
    /// the gauge, see a MONOTONE verdict, press
    /// `Ctrl+B Shift+M` to fix it.
    #[serde(rename = "ai.rewrite_sentence_rhythm")]
    AiRewriteRhythm,
    /// Ctrl+B Shift+P (1.2.9+) — toggle the POV /
    /// character chip on the status bar.  Session-local
    /// override on top of `editor.pov_chip_enabled` in
    /// HJSON.  When the chip is on, the status bar shows
    /// the most-mentioned character in the open
    /// paragraph (the heuristic POV character) + up to
    /// three additional named characters present.
    #[serde(rename = "view.toggle_pov_chip")]
    TogglePovChip,
    /// Ctrl+B Shift+N (1.2.12+) — toggle prompt-language
    /// resolution mode between book-defined and
    /// paragraph-detected (whatlang).  Session-local
    /// override on top of `editor.prompt_language_mode`
    /// in HJSON; the chord does NOT rewrite the HJSON.
    /// The AI pane title bar reflects the active mode
    /// so the user can confirm what language the
    /// resolver will target on the next AI call.
    /// See `Documentation/PROPOSALS/MULTILINGUAL_PROMPTS.md`.
    #[serde(rename = "view.toggle_prompt_language_mode")]
    TogglePromptLanguageMode,
    /// Ctrl+B Shift+L (1.2.9+) — open the project-wide
    /// concordance modal.  Lists every distinct lexical
    /// stem in the project with its total count + KWIC
    /// samples.  Stop-words and digits are filtered out
    /// so the list surfaces the words actually carrying
    /// the prose's weight.  Type to filter, `s` to
    /// toggle sort (count ↔ alphabetical), Esc closes.
    #[serde(rename = "view.open_concordance")]
    OpenConcordance,
    /// Ctrl+V R (1.2.5+) — render the open paragraph in-process
    /// via typst-render and float a PNG preview on top of the
    /// editor. `Esc` closes, `S` opens a save-as picker for the
    /// full-DPI PNG.
    #[serde(rename = "view.render_paragraph")]
    ViewRenderParagraph,
    /// Ctrl+V N (1.2.5+) — jump the editor cursor to the next
    /// typst diagnostic in the open buffer (parse or semantic).
    /// Wraps at the end. No-op when the diagnostic cache is
    /// empty.
    #[serde(rename = "view.next_diagnostic")]
    ViewNextDiagnostic,
    /// Ctrl+V Shift+W (1.2.5+) — story view of the current
    /// book: book at the centre, every chapter / subchapter /
    /// paragraph + paragraph links + lexicon mentions on
    /// concentric rings. Rendered to PNG and floated on top
    /// of the editor; `S` saves, `Esc` closes.
    #[serde(rename = "view.story_graph")]
    ViewStoryGraph,
    /// Ctrl+V w (1.2.6+) — paragraph mini story view: the
    /// open paragraph at the centre, its paragraph link neighbours
    /// (one hop out + one hop in) on the first ring, and any
    /// Characters / Places / Artefacts it mentions on the
    /// outer ring. Same render pipeline + save flow as the
    /// book view.
    #[serde(rename = "view.story_graph_paragraph")]
    ViewStoryGraphParagraph,
    /// Ctrl+V e (1.2.6+) — open the timeline event picker.
    /// Lists every event in the project chronologically;
    /// the user can filter by track, jump to events, or
    /// close with Esc. Requires `timeline.enabled: true` in
    /// HJSON.
    #[serde(rename = "view.event_picker")]
    ViewEventPicker,
    /// Ctrl+V Shift+E (1.2.6+) — open the timeline view AND
    /// immediately trigger the new-event prompt, so a fresh
    /// project can add its first event without going through
    /// the CLI's `inkhaven event add`. Honours
    /// `timeline.enabled`.
    #[serde(rename = "view.new_event_prompt")]
    ViewNewEventPrompt,
    /// Ctrl+Shift+M (1.2.7+) — toggle TUI mouse capture.
    /// Default ON (TUI sees click-to-focus, scroll wheel,
    /// etc.). Toggle OFF to let the terminal handle mouse
    /// natively: drag-to-select inside the editor / AI
    /// pane, system-clipboard copy via Cmd+C (macOS) or
    /// Ctrl+Shift+C (Linux/Windows). Status reports the
    /// new state.
    #[serde(rename = "global.toggle_mouse_capture")]
    ToggleMouseCapture,
    /// Alt+Left (1.2.7+) — browser-style "back" through
    /// the visited-paragraph history. Doesn't push to the
    /// history itself (so back/forward is reversible).
    #[serde(rename = "global.visited_back")]
    VisitedBack,
    /// Alt+Right (1.2.7+) — browser-style "forward". Only
    /// active when the user has gone back at least once.
    #[serde(rename = "global.visited_forward")]
    VisitedForward,
    /// Ctrl+B U (1.2.7+) — undo the most recent paragraph
    /// delete. Single-slot kill-ring; content + tags +
    /// linked_paragraphs + event data restored. The
    /// restored paragraph gets a NEW uuid — cross-refs
    /// from elsewhere that pointed at the deleted id stay
    /// broken (status hint flags this on each restore).
    /// Only paragraph deletes are recoverable; branch
    /// (chapter / book) deletes can't be undone.
    #[serde(rename = "global.undo_last_delete")]
    UndoLastDelete,
    /// Ctrl+V Shift+I (1.2.6+) — open a one-line edit prompt for
    /// the open event paragraph's start / end / track (pipe-
    /// separated). Precision is re-derived from the start
    /// string each commit. No-op when the open paragraph
    /// isn't an event.
    #[serde(rename = "view.edit_event_metadata")]
    ViewEditEventMetadata,
    /// Ctrl+V t (1.2.6+) — open the swim-lane timeline view
    /// scoped to the current paragraph's nearest Subchapter
    /// (or Chapter, or Book). Inside the modal:
    ///   u/U up-scope · d/D down-scope · b/B book · p/P project
    ///   ←/→ scroll · +/- zoom · Tab cycle track · Enter open
    ///   y critique scope · Y all-tracks · Ctrl+Y book-wide
    /// Requires `timeline.enabled: true` in HJSON.
    #[serde(rename = "view.timeline")]
    ViewTimeline,

    /// Explicit "this chord does nothing" — overlay entries can
    /// set `action: "none"` to disable a default chord.
    #[serde(rename = "none")]
    None,

    /// Ctrl+B Q.  Translate the open
    /// paragraph from the project's working language INTO an
    /// invented language defined under the `Language` system
    /// book.  Composes a prompt envelope from the language's
    /// Dictionary (RAG-filtered to words present in the
    /// source), Grammar (all rules), Phonology (all rules),
    /// and Sample-text chapters, then streams the response
    /// into the AI pane.  With zero Language sub-books the
    /// chord errors out; with exactly one it translates
    /// directly; with two or more it pops a picker (1.2.13
    /// Phase C.2 — ↑↓ + Enter, or type the first letter to
    /// jump-and-commit).
    #[serde(rename = "ai.translate_to_invented")]
    TranslateToInvented,
    /// Ctrl+B Shift+Q.  Reverse
    /// direction: translate the open paragraph FROM an
    /// invented language back into the working language.
    /// Same envelope shape, flipped direction labels.  The
    /// natural roundtrip workflow is `Ctrl+B Q` → copy the
    /// translation into the next paragraph → `Ctrl+B Shift+Q`
    /// — when the resulting working-language text matches
    /// the original the grammar rules + dictionary entries
    /// hold together end-to-end.
    #[serde(rename = "ai.translate_from_invented")]
    TranslateFromInvented,

    /// Runtime-only: a Bund lambda registered under the given
    /// name via `ink.key.bind_lambda`. Dispatch routes to
    /// `scripting::hooks::fire(name, vec![])`. `#[serde(skip)]` —
    /// these can't appear in HJSON; they live only in memory and
    /// vanish on process exit.
    #[serde(skip)]
    BundLambda(Arc<str>),
}

impl Action {
    /// Short label used in the auto-generated status-bar meta
    /// hint ("add chapter", "morph-type", …). Returns `""` for
    /// `None` and the lambda name for `BundLambda`.
    pub fn label(&self) -> String {
        match self {
            Action::AddBook => "add book".into(),
            Action::AddChapter => "add chapter".into(),
            Action::AddSubchapter => "add subchapter".into(),
            Action::AddParagraph => "add paragraph".into(),
            Action::DeleteNode => "delete".into(),
            Action::MorphType => "morph-type".into(),
            Action::ReorderUp => "↑ reorder".into(),
            Action::ReorderDown => "↓ reorder".into(),

            Action::Save => "save".into(),
            Action::CreateSnapshot => "snapshot".into(),
            Action::CycleStatus => "status".into(),
            Action::OpenFunctionPicker => "func".into(),
            Action::RenameToFirstSentence => "retitle".into(),
            Action::LookupPlacesOrImage => "place/pic".into(),
            Action::LookupCharacters => "character".into(),
            Action::LookupNotes => "notes".into(),
            Action::LookupArtefacts => "artefacts".into(),
            Action::OpenQuickref => "help".into(),

            Action::OpenCredits => "credits".into(),
            Action::OpenBookInfo => "info".into(),
            Action::OpenLlmPicker => "LLM".into(),
            Action::ToggleSound => "sound".into(),
            Action::ScheduleAssemble => "assemble".into(),
            Action::ScheduleBuild => "build".into(),
            Action::ScheduleTake => "take".into(),
            Action::BackupNow => "backup".into(),
            Action::ToggleTypewriter => "focus mode".into(),
            Action::ToggleAiFullscreen => "AI-full".into(),
            Action::StatusFilterReady => "Ready".into(),
            Action::StatusFilterFinal => "Final".into(),
            Action::StatusFilterThird => "Third".into(),
            Action::StatusFilterSecond => "Second".into(),
            Action::StatusFilterFirst => "First".into(),
            Action::StatusFilterNapkin => "Napkin".into(),
            Action::StatusFilterNone => "None".into(),

            Action::TagParagraph => "tag ¶".into(),
            Action::TagSearch => "tag search".into(),

            Action::ClearChat => "clear chat".into(),

            Action::BundRunBuffer => "run buffer".into(),
            Action::BundNewScript => "new script".into(),
            Action::BundOpenEvalModal => "eval".into(),
            Action::BundOpenScriptPicker => "pick script".into(),

            Action::HelpQuery => "help".into(),
            Action::RenameNode => "rename".into(),
            Action::FilePickerTreeImport => "file picker".into(),
            Action::FilePickerEditorLoad => "load file".into(),
            Action::ToggleSplit => "split".into(),
            Action::AcceptSplitSnapshot => "accept snap".into(),
            Action::ToggleSplitView => "split view".into(),
            Action::ViewSiblingBookLookup => "sibling book".into(),
            Action::ViewThreadsPicker => "threads".into(),
            Action::AiThreadAudit => "thread audit".into(),
            Action::ViewThreadDoctor => "thread doctor".into(),
            Action::ViewAddComment => "add comment".into(),
            Action::ViewCommentsPanel => "comments".into(),
            Action::AiContinuationDraft => "continue".into(),
            Action::EditorInsertFootnote => "footnote".into(),
            Action::ViewProjectGoalModal => "goal".into(),
            Action::AiStyleTransferRewrite => "style xfer".into(),
            Action::OpenSnapshotPicker => "snapshots".into(),
            Action::GrammarCheck => "grammar".into(),
            Action::DiagnosticsList => "diags".into(),
            Action::ExplainDiagnostic => "explain diag".into(),
            Action::Critique => "critique".into(),
            Action::CycleAiMode => "AI mode".into(),
            Action::ToggleInferenceMode => "infer mode".into(),

            Action::ViewExportMarkdownBuffer => "md buffer".into(),
            Action::ViewExportMarkdownSubchapter => "md subchap".into(),
            Action::ViewExportMarkdownSubtree => "md subtree".into(),
            Action::ViewToggleSimilarMode => "similar".into(),
            Action::ViewOpenProgress => "progress".into(),
            Action::ViewOpenParagraphTarget => "para target".into(),
            Action::ViewAddLink => "add link".into(),
            Action::ViewAddIncomingLink => "add ← link".into(),
            Action::ViewListLinks => "list links".into(),
            Action::ViewListBacklinks => "backlinks".into(),
            Action::ViewToggleBookmark => "bookmark".into(),
            Action::ViewListBookmarks => "bookmarks".into(),
            Action::ViewFuzzyParagraphPicker => "find ¶".into(),
            Action::ViewRecentParagraphPicker => "recent ¶".into(),
            Action::ViewKillRingPicker => "kill-ring".into(),
            Action::ViewHiddenCharsReport => "hidden chars".into(),
            Action::ViewShowBreadcrumb => "breadcrumb".into(),
            Action::BundOpenShell => "shell".into(),
            Action::BundOpenShellFresh => "shell fresh".into(),
            Action::BundShellSelection => "shell select".into(),
            Action::BundEditProjectHjson => "edit hjson".into(),
            Action::TtsReadParagraph => "read aloud".into(),
            Action::TtsSaveAsAudio => "save audio".into(),
            Action::OpenWritingStreakHeatmap => "streak".into(),
            Action::OpenDoctorPanel => "doctor".into(),
            Action::SceneBreakPrev => "prev scene break".into(),
            Action::SceneBreakNext => "next scene break".into(),
            Action::ToggleStyleWarnings => "style warnings".into(),
            Action::OpenConcordance => "concordance".into(),
            Action::TogglePovChip => "pov chip".into(),
            Action::TogglePromptLanguageMode => "prompt lang mode".into(),
            Action::OpenSentenceRhythm => "rhythm".into(),
            Action::AiRewriteRhythm => "rhythm rewrite".into(),
            Action::AnalyseShowDontTell => "show↛tell AI".into(),
            Action::TranslateToInvented => "translate →".into(),
            Action::TranslateFromInvented => "translate ←".into(),
            Action::ViewRenderParagraph => "render ¶".into(),
            Action::ViewNextDiagnostic => "next diag".into(),
            Action::ViewStoryGraph => "story view".into(),
            Action::ViewStoryGraphParagraph => "story view (¶)".into(),
            Action::ViewEventPicker => "events".into(),
            Action::ViewNewEventPrompt => "new event".into(),
            Action::ToggleMouseCapture => "mouse".into(),
            Action::VisitedBack => "← back".into(),
            Action::VisitedForward => "fwd →".into(),
            Action::UndoLastDelete => "undo del".into(),
            Action::ViewEditEventMetadata => "edit event".into(),
            Action::ViewTimeline => "timeline".into(),

            Action::None => String::new(),
            Action::BundLambda(name) => format!("λ {name}"),
        }
    }

    /// Long, user-friendly description used by Ctrl+B H (the
    /// quick-reference panel). Where `label()` is squeezed into
    /// the status-bar hint and is therefore terse to the point of
    /// cryptic, this is a full sentence aimed at someone reading
    /// the panel for the first time. Returns `""` for `None` and
    /// a generic "user-bound Bund lambda" line for `BundLambda`.
    pub fn description(&self) -> String {
        match self {
            // ── Tree ──────────────────────────────────────────
            Action::AddBook => "Add a new top-level Book to the project.".into(),
            Action::AddChapter => "Add a Chapter under the current branch.".into(),
            Action::AddSubchapter =>
                "Add a Subchapter under the current chapter / subchapter.".into(),
            Action::AddParagraph =>
                "Add a Paragraph leaf under the current branch (typst content).".into(),
            Action::DeleteNode =>
                "Delete the node under the tree cursor (asks for confirmation).".into(),
            Action::MorphType =>
                "Cycle the selected leaf's flavour: Paragraph(typst) → Paragraph(hjson) → Script(bund).".into(),
            Action::ReorderUp =>
                "Move the current node up among its siblings.".into(),
            Action::ReorderDown =>
                "Move the current node down among its siblings.".into(),

            // ── Editor / save / snapshots ─────────────────────
            Action::Save =>
                "Save the open paragraph to disk (autosave also fires on idle).".into(),
            Action::CreateSnapshot =>
                "Snapshot the open paragraph (history kept under F6 picker).".into(),
            Action::CycleStatus =>
                "Cycle the open paragraph's status: None → Napkin → First → Second → Third → Final → Ready.".into(),
            Action::OpenFunctionPicker =>
                "Open the Typst function picker — type to filter, Enter inserts #name(…).".into(),
            Action::RenameToFirstSentence =>
                "Rename the open paragraph using its first sentence as the new title.".into(),
            Action::LookupPlacesOrImage =>
                "Inside #image(\"…\"): pick a sibling image. Otherwise run a Places RAG over the selection.".into(),
            Action::LookupCharacters =>
                "Character RAG — selection is queried against the Characters book, answer streams in AI pane.".into(),
            Action::LookupNotes =>
                "Notes RAG — selection is queried against the Notes book, answer streams in AI pane.".into(),
            Action::LookupArtefacts =>
                "Artefacts RAG — selection is queried against the Artefacts book, answer streams in AI pane.".into(),
            Action::OpenQuickref =>
                "Open this Quick reference panel (live keymap + static cheatsheet).".into(),

            // ── Global / panels ───────────────────────────────
            Action::OpenCredits =>
                "Show inkhaven version, author, and bundled-component credits.".into(),
            Action::OpenBookInfo =>
                "Open the current book's info panel: paths, stats, PDF status.".into(),
            Action::OpenLlmPicker =>
                "Switch the active LLM provider — choice is persisted to inkhaven.hjson.".into(),
            Action::ToggleSound =>
                "Toggle typewriter SFX (Enter / focus-out clicks). Choice is persisted to inkhaven.hjson.".into(),
            Action::ScheduleAssemble =>
                "Book assembly — emit a typst-compilable tree under the artefacts dir.".into(),
            Action::ScheduleBuild =>
                "Build the book — assemble + run `typst compile` (PDF lands in artefacts dir).".into(),
            Action::ScheduleTake =>
                "Take the book — build then copy the PDF (and any configured extras) into the launch cwd.".into(),
            Action::BackupNow =>
                "Run a project backup now (Ctrl+B Shift+B). Always fires — ignores the exit-hook recency cooldown.".into(),
            Action::ToggleTypewriter =>
                "Toggle distraction-free / focus mode — hides every other pane (Tree, AI, Search, AI prompt) and gives the editor the full window. Re-press to restore the four-pane layout. Internally still called \"typewriter mode\" in some log strings + the HJSON config field; the chord serde key is `global.toggle_typewriter` for backward-compat.".into(),
            Action::ToggleAiFullscreen =>
                "Toggle full-screen AI mode — AI pane | chat history + AI prompt.".into(),
            Action::StatusFilterReady =>
                "Filter the tree to paragraphs marked Ready under the cursor.".into(),
            Action::StatusFilterFinal =>
                "Filter the tree to paragraphs marked Final under the cursor.".into(),
            Action::StatusFilterThird =>
                "Filter the tree to paragraphs marked Third under the cursor.".into(),
            Action::StatusFilterSecond =>
                "Filter the tree to paragraphs marked Second under the cursor.".into(),
            Action::StatusFilterFirst =>
                "Filter the tree to paragraphs marked First under the cursor.".into(),
            Action::StatusFilterNapkin =>
                "Filter the tree to paragraphs marked Napkin under the cursor.".into(),
            Action::StatusFilterNone =>
                "Filter the tree to paragraphs with no status under the cursor.".into(),

            Action::TagParagraph =>
                "Open the tag picker scoped to the open paragraph: Space selects, T applies selected tags, A adds a new tag (prompt), D deletes a tag project-wide.".into(),
            Action::TagSearch =>
                "Open the search-by-tag picker. Enter on a tag lists paragraphs that carry it (with a filter input); Enter on a paragraph opens it in the editor.".into(),

            // ── AI ────────────────────────────────────────────
            Action::ClearChat =>
                "Clear the chat history and any in-flight inference for a fresh AI session.".into(),

            // ── Bund prefix ───────────────────────────────────
            Action::BundRunBuffer =>
                "Evaluate the currently-open .bund script against Adam (Bund VM).".into(),
            Action::BundNewScript =>
                "Add a new Bund script under the Scripts system book.".into(),
            Action::BundOpenEvalModal =>
                "Open the one-shot Bund eval modal — type an expression, see its result in the status bar.".into(),
            Action::BundOpenScriptPicker =>
                "Open the script picker — list scripts in the current branch (A toggles to Scripts book), Enter runs.".into(),

            // ── Top-level F-keys (1.2.4+) ──────────────────────
            Action::HelpQuery =>
                "Open the Help-book RAG query modal — natural-language question against the Help book.".into(),
            Action::RenameNode =>
                "Rename the tree-cursor's node (paragraphs also rename their .typ on disk).".into(),
            Action::FilePickerTreeImport =>
                "Open the file picker in import mode — a file becomes a new paragraph, a directory recursively imports as branches.".into(),
            Action::FilePickerEditorLoad =>
                "Open the file picker in load mode — replaces the open paragraph's buffer with the picked file's content.".into(),
            Action::ToggleSplit =>
                "Toggle split-edit mode — captures the current buffer as a read-only lower pane.".into(),
            Action::AcceptSplitSnapshot =>
                "Replace the live buffer with the split's captured snapshot, exit split, mark dirty.".into(),
            Action::ToggleSplitView =>
                "Toggle fullscreen split-view (Shift+F4, 1.2.12+). Left pane is the primary buffer; right pane is the `secondary` slot, populated by pickers (tree Enter, Ctrl+V P / M / Shift+P, Ctrl+V Shift+B). Tab swaps focus. Tree + AI response panes are hidden; AI prompt input bar still spans the bottom so Ctrl+I works from either pane. Existing F4 (same-paragraph snapshot split) and Ctrl+F4 (accept snapshot) are untouched.".into(),
            Action::ViewSiblingBookLookup =>
                "Sibling-book lookup for the split-view secondary pane (Ctrl+V Shift+B, 1.2.12+). Given the open paragraph's slug, walks the project hierarchy for paragraphs with the same slug under a different top-level book. Zero matches → status message names the slug. Single match → auto-pin to secondary. Multiple matches → open a fuzzy picker. Primary translation-workflow chord: from `manuscript-en/03-rain`, finds `manuscript-ru/03-rain` and pins it for side-by-side review via Shift+F4.".into(),
            Action::ViewThreadsPicker =>
                "Open the Threads picker (Ctrl+V Shift+H, 1.2.14+). Lists every plot-thread paragraph under the `Threads` system book with status (setup/develop/payoff/resolved/abandoned) / weight (major/subplot/runner/bridge) / tension (0-10) / character + place + linked-paragraph counts. Picker chords: ↑↓ navigate, Enter opens the thread entry in the editor, Shift+Enter pins to the split-view secondary slot, w opens the swim-lane weave view (threads × chapters with marks at every paragraph that links to the thread), `/` filters the list by typed substring (status, weight, or title), Esc closes. The weave view's chord set: ↑↓ moves between threads, ←→ moves between chapters, Enter on a cell jumps to a linking paragraph, Esc returns to the picker.".into(),
            Action::AiThreadAudit =>
                "AI thread audit (Ctrl+V Shift+A, 1.2.14+). Resolves the cursor's scope from the F9 AiMode (Chapter / Subchapter / Book; Selection / Paragraph / None default to the cursor's containing Chapter). Composes a prompt envelope reading every Thread paragraph's HJSON (title / status / weight / opening / midpoint / payoff / connections / tension), a blind-spots pre-pass (link counts per thread, payoff-marked threads whose payoff hasn't yet been linked, stale threads not advanced in the scope), and every paragraph in the scope (paragraph_id + title + body + linked_paragraphs). Streams the response into the AI pane.  Asks the model to score each scope paragraph for which threads it advances / touches incidentally / should advance but doesn't, then call out structural concerns (dormant arcs, premature payoffs, miscategorised weights).".into(),
            Action::ViewThreadDoctor =>
                "Open the thread doctor modal (Ctrl+V Shift+D, 1.2.14+). Walks every Threads paragraph + computes the same numbers `inkhaven thread doctor` prints: status distribution, weight distribution, average tension, and three blind-spot passes (ZERO LINKS — status past `setup` but no paragraph links to the thread; PAYOFF UNFIRED — status `payoff` but no paragraph links; DORMANT — status `develop` but ≤1 link project-wide).  Read-only modal; Esc closes.  Pairs with `Ctrl+V Shift+H` (picker, per-thread detail) + `Ctrl+V Shift+A` (AI audit of scope) — the doctor is the project-wide structural health check.".into(),
            Action::ViewAddComment =>
                "Add an inline comment on the current selection (Ctrl+V c, 1.2.14+). When a selection is active, the comment anchors to that character range. When no selection, it anchors to the word at the cursor (Unicode word boundaries). Pops a multi-line text input modal for the comment body; on commit, writes a sidecar JSON file alongside the paragraph's .typ (`<paragraph>.comments.json`) so the comment travels with the prose in git and diffs cleanly. The commented span is rendered with `theme.comment_span_modifier` (default underline+italic); cursor inside the span surfaces the comment text + author + age in the editor footer. Character offsets (not byte) so UTF-8 boundary edits preserve anchoring.".into(),
            Action::ViewCommentsPanel =>
                "Open the project-wide comments panel (Ctrl+V Shift+C, 1.2.14+). Walks every paragraph's `.comments.json` sidecar and lists every comment with breadcrumb / author / age / text-snippet columns. Panel chords: ↑↓ navigate, Enter open the source paragraph (cursor positioned at the comment span start), r resolve, R reopen (cycles the resolved-filter), d delete (immediate, no confirm), / filter (substring across paragraph slug, author, text body), Esc close. Resolved comments hide by default; press R to toggle them back into view. Reads + writes the sidecar files at panel time — no in-memory cache to stale, so a CLI `inkhaven comments resolve` change between sessions is visible on next panel open.".into(),
            Action::AiContinuationDraft =>
                "AI continuation drafting (Ctrl+V d, 1.2.14+). Asks the configured LLM to continue the open paragraph in the author's voice.  Prompt envelope sends the previous N paragraphs (configurable via `editor.continuation_anchor_count`, default 3) as voice anchors and the open paragraph with the cursor position marked.  Response wrapped in <<<DRAFT>>> / <<<END>>> markers; AI pane I apply lifts only the draft block at the cursor.  Pairs with snippet expansion (\\tdo + Ctrl+V d for AI-generated TODOs).".into(),
            Action::EditorInsertFootnote =>
                "Insert an inline footnote at the cursor (Ctrl+V f, 1.2.14+). Pops a multi-line text input modal for the footnote body.  On commit, inserts `#footnote[<body>]` at the cursor (Typst, the default) or `[^id]` plus a trailing `[^id]: <body>` line (markdown, when `editor.footnote_style = \"markdown\"`).  Mostly for academic / reference writing; the Typst markup is already supported by the assembled-book renderer.".into(),
            Action::ViewProjectGoalModal =>
                "Project-level word-count goal + projection modal (Ctrl+V Shift+G, 1.2.14+). Reads `project.word_count_goal`, `project.target_date`, `project.counted_books` from the HJSON config.  Computes total project words, percentage of goal, days remaining, words-per-day required from today, recent average from the daily streak event log, and the projected completion date.  Per-book breakdown shows which book(s) contribute most.  Read-only; close with Esc.".into(),
            Action::AiStyleTransferRewrite =>
                "Style transfer rewrite (Ctrl+V y, 1.2.14+). Pops a paragraph picker scoped to the current book.  On selection, composes a prompt envelope asking the LLM to rewrite the open paragraph in the picked reference paragraph's style (sentence-length distribution, vocabulary register, rhythm, mood, narrative distance) while preserving literal meaning + named entities + plot facts.  Response wrapped in <<<REWRITE>>> / <<<END>>> markers; AI pane I apply extracts only the rewrite block.  Different from Ctrl+B Shift+M rhythm rewrite (which targets rhythm only with a prompt-defined style); this chord targets a CONCRETE EXAMPLE paragraph the author picks.".into(),
            Action::OpenSnapshotPicker =>
                "Open the snapshot picker for the current paragraph (↑↓ navigate · Enter loads · V diff · D delete).".into(),
            Action::GrammarCheck =>
                "Grammar-check the open paragraph — runs the configured F7 prompt against the AI, applies via `g` in the AI pane.".into(),
            Action::DiagnosticsList =>
                "Open the floating typst-diagnostics list. Enter on a row jumps the editor cursor to that diagnostic.".into(),
            Action::ExplainDiagnostic =>
                "Send the typst diagnostic at the cursor (with surrounding context lines) to the AI pane with the configured explain-or-fix prompt.".into(),
            Action::Critique =>
                "AI critique of the open paragraph. In edit mode: 'what's weak' prompt. In split-edit mode: 'evaluate changes' prompt. Prompt resolution: Prompts book > prompts.hjson > embedded default.".into(),
            Action::CycleAiMode =>
                "Cycle AI scope: None → Selection → Paragraph → Subchapter → Chapter → Book → None.".into(),
            Action::ToggleInferenceMode =>
                "Toggle inference mode: Local-only RAG ↔ Full general knowledge (Help is pinned to Local regardless).".into(),

            // ── View prefix ────────────────────────────────────
            Action::ViewExportMarkdownBuffer =>
                "Export the open paragraph's live buffer (including unsaved edits) as markdown to the launch cwd.".into(),
            Action::ViewExportMarkdownSubchapter =>
                "Export the containing subchapter's subtree as markdown to the launch cwd.".into(),
            Action::ViewExportMarkdownSubtree =>
                "Export the tree-cursor's node and all descendants as markdown to the launch cwd.".into(),
            Action::ViewToggleSimilarMode =>
                "Toggle similar-paragraph mode — vector-similarity picker; selecting a hit opens a second editor side-by-side. Re-press to save both and exit.".into(),
            Action::ViewOpenProgress =>
                "Open the writing-progress modal (today / streak / per-book pace / 30-day sparkline / status-ladder counts).".into(),
            Action::ViewOpenParagraphTarget =>
                "Set or clear the open paragraph's word-count goal. Saves that cross the target auto-promote status one ladder step.".into(),
            Action::ViewAddLink =>
                "Add a linked paragraph — tree pane switches to `select paragraph to link` mode; Enter links, Esc cancels. Stored as metadata, never embedded in typst source.".into(),
            Action::ViewAddIncomingLink =>
                "Add an incoming link — tree pane picker; Enter on a paragraph adds the OPEN paragraph to THAT paragraph's outgoing links (reverse of Ctrl+V A).".into(),
            Action::ViewListLinks =>
                "Open the linked-paragraphs modal — list outgoing paragraph links for the open paragraph; press D on a row to remove.".into(),
            Action::ViewListBacklinks =>
                "Open the backlinks modal — list paragraphs that link to the open paragraph (reverse of Ctrl+V L). Enter opens; D removes the source's outgoing link to current.".into(),
            Action::ViewToggleBookmark =>
                "Toggle bookmark on the open paragraph. Bookmarks are surfaced by the Ctrl+V M picker; survive restart via metadata.".into(),
            Action::ViewListBookmarks =>
                "Open the bookmark picker — every bookmarked paragraph in the project. Enter opens; D removes the bookmark.".into(),
            Action::ViewFuzzyParagraphPicker =>
                "Fuzzy paragraph picker — type any substring of the title or slug path, Enter opens the highlighted hit.".into(),
            Action::ViewRecentParagraphPicker =>
                "Recent paragraph picker (1.2.7+) — same fuzzy picker as Ctrl+V P but sorted by modified_at desc. Answers \"what did I work on most recently?\" without trawling the tree.".into(),
            Action::ViewKillRingPicker =>
                "Kill-ring picker (1.2.8+) — list of recently-deleted paragraphs (up to 10). Enter restores the highlighted entry at its original position; Esc cancels. Ctrl+B U alone restores the most-recent without opening the picker.".into(),
            Action::ViewHiddenCharsReport =>
                "Hidden-character report (1.2.8+) — status-bar summary of the open paragraph's tabs / trailing-whitespace lines / CR characters. Useful for spotting import noise (Scrivener / web paste). Visual editor overlay scheduled for 1.2.9.".into(),
            Action::ViewShowBreadcrumb =>
                "Show breadcrumb (1.2.8+) — print the hierarchy path from project root to the cursor on the status bar (Book ▸ Chapter ▸ Subchapter ▸ Paragraph). Pane-aware: in tree walks from the tree cursor, in editor walks from the open paragraph.".into(),
            Action::BundOpenShell =>
                "Open / close the embedded nushell pane (1.2.8+). Floating fullscreen; engine state and turn buffer preserved across close+reopen. No-op when shell.enabled = false in HJSON.".into(),
            Action::BundOpenShellFresh =>
                "Drop the cached shell engine + turn buffer and open a fresh shell (1.2.8+). Use when env / scope has drifted into a confusing state.".into(),
            Action::BundShellSelection =>
                "Inside the shell pane, toggle history-selection mode (1.2.8+) — ↑↓ walks turn-by-turn, `c` copies output to clipboard, `i` inserts wrapped in the configured typst-box template. Re-press exits.".into(),
            Action::BundEditProjectHjson =>
                "Open `<project>/inkhaven.hjson` in a full-screen editor (1.2.8+, Ctrl+B 0). Syntax-highlighted via the hand-rolled HJSON lexer. Ctrl+S saves; when saved bytes differ from the loaded bytes, a restart-required overlay pops up (config applies on next launch). Esc closes; unsaved-edit warnings fire on close.".into(),
            Action::TtsReadParagraph =>
                "Read the open paragraph aloud via the OS TTS engine (1.2.9+, Ctrl+B S in editor scope). Cross-platform via `tts-rs`: AVFoundation on macOS, SAPI / WinRT on Windows, Speech Dispatcher on Linux. Gated by `editor.tts.enabled = true` in HJSON; default is off. Default voice is `Milena` (Russian female; ships free with macOS + Windows). When TTS is disabled, or the engine fails to initialise (Linux without speech-dispatcher, missing voices, etc.), a friendly explanation modal fires instead.".into(),
            Action::TtsSaveAsAudio =>
                "Save the open paragraph as an audio file via macOS `say -o <path>` (1.2.9+, Ctrl+B Shift+R). Opens a path picker pre-filled with `<project>/audio/<paragraph-slug>.aiff`; Enter commits, Esc cancels. Output is AIFF by default; coerce another format with the file extension (`.m4a`, `.wav` work on macOS 13+). Same voice + rate as the configured chord-driven TTS. macOS-only; non-macOS hosts surface the same `TTS unavailable` modal as Ctrl+B S.".into(),
            Action::OpenWritingStreakHeatmap =>
                "Open the writing-streak heatmap modal (1.2.9+, Ctrl+B Shift+G). GitHub-style 13×7 grid of the last 91 days of project-wide word deltas, plus current streak + longest streak in the window + per-month totals. Data comes from the existing progress store (the same source feeding the startup pulse splash + Ctrl+V G modal). Esc closes.".into(),
            Action::OpenDoctorPanel =>
                "Open the project-wide doctor panel (1.2.15+, Ctrl+B Shift+0). Runs the same scan as the `inkhaven doctor --scan` CLI: zero-byte paragraph files, orphan DB rows, missing referenced files, corrupt comment sidecars. Each finding shows class + severity + path + a one-line detail; `r` repairs the highlighted finding, `R` repairs every finding, `Esc` closes. Repairs are logged to `<project>/.inkhaven/doctor.log` with timestamp + class + outcome for audit. Paired mnemonically with `Ctrl+B 0` (HJSON config editor): digit-0 row is the system-inspection cluster.".into(),
            Action::SceneBreakPrev =>
                "Jump editor cursor to the previous scene-break line (1.2.9+, Ctrl+B <). Scene breaks are typographic divider lines: `* * *`, `***`, `---`, `___`, `###`, `~~~`, or a lone `§`. Detection is hand-rolled — any line consisting only of 3+ copies of `*`/`-`/`_`/`~`/`#` (optionally space-separated) counts, plus `§` alone. Useful for navigating multi-scene paragraphs in a single pass.".into(),
            Action::SceneBreakNext =>
                "Jump editor cursor to the next scene-break line (1.2.9+, Ctrl+B >). Same detector as `SceneBreakPrev`.".into(),
            Action::ToggleStyleWarnings =>
                "Toggle the inline style-warning overlays (1.2.9+, Ctrl+B Shift+F). Currently flags filter words — intensifier crutches like `just`, `really`, `very`, `просто`, `очень` — drawn in amber + underlined. Session-local override on top of `editor.style_warnings.enabled` in HJSON. Per-language defaults ship for English, Russian, French, German, Spanish; the active list is keyed by the project's top-level `language` field. Add more via `editor.style_warnings.filter_words.extra_words`. Repeated-phrase / show-don't-tell / sentence-rhythm detectors will share this toggle as they land.".into(),
            Action::OpenConcordance =>
                "Open the project-wide concordance modal (1.2.9+, Ctrl+B Shift+L). Lists every distinct lexical stem in the project with its total count plus up to three KWIC samples. Stop-words, single-character tokens, and pure-digit runs are filtered out so the list surfaces the words actually carrying the prose's weight. System books (Prompts, Characters, Places, Lore, Help, Notes, Artefacts, etc.) are excluded from the corpus since they're metadata/scaffolding, not prose (1.2.11+). Multilingual via the same Snowball stemmer + stop-list plumbing as the repeated-phrase detector — `language` in HJSON drives the algorithm choice. Type to filter (substring match); Ctrl+S toggles sort (count ↔ alphabetical); Enter jumps to the first sample's source paragraph at the matching line (1.2.11+); Esc closes.".into(),
            Action::TogglePovChip =>
                "Toggle the POV / character chip on the status bar (1.2.9+, Ctrl+B Shift+P). When enabled, the status bar shows the most-mentioned character in the open paragraph (the heuristic POV character) plus up to three additional named characters present. Driven by the project's existing `characters` lexicon — no separate tagging needed. Ties broken by first-mention order. Session-local override on top of `editor.pov_chip_enabled` in HJSON.".into(),
            Action::TogglePromptLanguageMode =>
                "Toggle prompt-language resolution mode between `book_defined` (use the top-level `language` field) and `paragraph_detected` (run whatlang on the open paragraph; fall back to book language for short paragraphs) (1.2.12+, Ctrl+B Shift+N). Session-local override on top of `editor.prompt_language_mode` in HJSON — the chord does NOT rewrite the HJSON. The AI pane title bar reflects the active mode: `AI · ru (book)` vs `AI · ru (paragraph)`. The status bar echoes the new mode on toggle. Mnemonic: N for Natural language / laNguage picker. See Documentation/PROPOSALS/MULTILINGUAL_PROMPTS.md.".into(),
            Action::OpenSentenceRhythm =>
                "Open the sentence-rhythm gauge modal for the open paragraph (1.2.9+, Ctrl+B Shift+H). Splits prose into sentences (hand-rolled walker with abbreviation suppression), tallies word counts, computes mean / stdev / coefficient of variation (CV), and maps CV to a verdict: Monotone (CV < 0.25 — drones), Steady (0.25-0.45 — workable), Varied (0.45-0.80 — strong prose rhythm), Choppy (≥ 0.80 — fragments + long sentences mixed). Shows a per-sentence bar list and the three shortest + three longest outliers. Mnemonic: H for heartbeat — the felt rhythm of the prose.".into(),
            Action::AiRewriteRhythm =>
                "AI-driven sentence-rhythm rewrite of the open paragraph (1.2.11+, Ctrl+B Shift+M). Sends the paragraph to the configured LLM with a prompt asking it to break monotonous rhythm by mixing short and long sentences while preserving voice + meaning. Prompt resolution follows the standard pattern: the project's Prompts book first (look up by slug or title `sentence-rhythm-rewrite`), then prompts.hjson, then an embedded multilingual fallback that respects the project's `language` setting. When the stream completes, an AI diff modal pops automatically so the user can review the rewrite line by line. Accept commits the rewrite into the buffer AND creates a snapshot annotated `Sentence rhythm rewrite` first; reject leaves the buffer untouched. Mnemonic: M for Modulate / Mix it up. Pairs with the Ctrl+B Shift+H rhythm gauge — and the chord ALSO fires from inside that gauge modal, so the natural diagnose-then-rewrite workflow needs no extra keystrokes: open the gauge, see MONOTONE, press Ctrl+B Shift+M to fix it. The gauge dismisses automatically as the rewrite spawns.".into(),
            Action::AnalyseShowDontTell =>
                "AI-driven show-don't-tell scan of the open paragraph (1.2.9+, Ctrl+B Shift+T). Sends the paragraph to the configured LLM with a system prompt asking for telling passages plus suggested rewrites. The response streams into the AI pane. Complements the always-on regex overlay (`editor.style_warnings.show_dont_tell`) with deeper analysis — the regex catches the obvious 2-grams (`was angry`, `realised`); the AI scan catches subtler instances and proposes alternatives. Mnemonic: T for tell.".into(),
            Action::TranslateToInvented =>
                "AI-driven translation of the open paragraph from the project's working language INTO an invented language defined under the Language system book (1.2.13+, Ctrl+B Q). Composes a prompt envelope from the language's Dictionary (RAG-filtered to words present in the source), Grammar (all rules), Phonology (all rules), and Sample-text chapters, then streams the response into the AI pane. With zero Language sub-books the chord errors out; with exactly one it translates directly; with two or more it pops a picker — ↑↓ + Enter, or type the first letter to jump-and-commit (the proposal's Ctrl+B Q Q for Quenya sub-letter pattern, unbundled). The translation block is wrapped between <<<TRANSLATION>>> / <<<END>>> markers so the I apply chord in the AI pane lifts only the target-language prose, no gloss table or commentary.".into(),
            Action::TranslateFromInvented =>
                "Reverse-direction AI translation (1.2.13+, Ctrl+B Shift+Q). Translate the open paragraph FROM an invented language defined under the Language system book back into the project's working language. Same prompt envelope shape and language-picker semantics as Ctrl+B Q. The natural roundtrip-test workflow is Ctrl+B Q → copy the translation into the next paragraph → Ctrl+B Shift+Q: when the resulting working-language text matches the original, the grammar rules and dictionary entries hold together end-to-end — exposes grammar drift before it bites in the manuscript.".into(),
            Action::ViewRenderParagraph =>
                "Render the open paragraph in-process and float the PNG preview on top of the editor. Esc closes; S opens a save-as picker for the full-DPI PNG.".into(),
            Action::ViewNextDiagnostic =>
                "Jump the editor cursor to the next typst diagnostic (parse or semantic) in the open buffer. Wraps around at the end; no-op when there are no diagnostics.".into(),
            Action::ViewStoryGraph =>
                "Story view of the current user book — book at the centre, every chapter / subchapter / paragraph + paragraph links + lexicon mentions on concentric rings. Float a PNG on top of the editor; S saves, Esc closes.".into(),
            Action::ViewStoryGraphParagraph =>
                "Paragraph mini story view — the open paragraph at the centre, its paragraph link neighbours (one hop out + one hop in) on the first ring, and any Characters / Places / Artefacts it mentions on the outer ring. Same render + save flow as the book view.".into(),
            Action::ViewEventPicker =>
                "Open the timeline event picker (1.2.6+). Lists every event in the project sorted by start time; Enter jumps to the event paragraph. Requires `timeline.enabled: true` in HJSON.".into(),
            Action::ViewNewEventPrompt =>
                "Open the swim-lane timeline view and immediately prompt for a new event title (1.2.6+). Same flow as opening the timeline then pressing `n`. Requires `timeline.enabled: true`.".into(),
            Action::ToggleMouseCapture =>
                "Toggle TUI mouse capture (1.2.7+). Default ON. When OFF, the terminal handles mouse natively: drag-to-select text in the editor / AI pane, system-clipboard copy via Cmd+C (macOS) or Ctrl+Shift+C (Linux/Windows). Toggle back to re-enable click-to-focus + scroll-wheel inside the TUI.".into(),
            Action::VisitedBack =>
                "Browser-style back (1.2.7+) — re-open the previously-visited paragraph. Default chord: Alt+Left. History persists across sessions in .session.json.".into(),
            Action::VisitedForward =>
                "Browser-style forward (1.2.7+) — re-open the next paragraph in the visit history. Default chord: Alt+Right. Only active after at least one back-press.".into(),
            Action::UndoLastDelete =>
                "Undo the most-recent paragraph delete (1.2.7+) — single-slot kill-ring. Restores content + tags + linked_paragraphs + event data, but the restored ¶ gets a NEW uuid so paragraph links from elsewhere stay broken. Branch deletes (chapter / book) can't be undone. Default chord: Ctrl+B U.".into(),
            Action::ViewEditEventMetadata =>
                "Edit the open event paragraph's start / end / track (pipe-separated, 1.2.6+). Pre-fills with current values; empty middle = no end; empty trailing = drop track. Precision re-derived from start on commit. No-op when the open paragraph isn't an event.".into(),
            Action::ViewTimeline =>
                "Open the swim-lane timeline view (1.2.6+). Scope-aware: anchors to the current paragraph's nearest Subchapter / Chapter / Book by default. Inside: u/U up-scope, d/D down-scope picker, b/B book, p/P project; ←/→ scroll, +/- zoom, Tab cycle track, Enter open event. Requires `timeline.enabled: true`.".into(),

            Action::None => String::new(),
            Action::BundLambda(name) =>
                format!("User-bound Bund lambda `{name}` (registered via ink.key.bind_lambda)."),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BindingEntry {
    pub chord: KeyChord,
    pub action: Action,
    pub scope: Scope,
}

/// Live binding table. Held in the process-wide `ACTIVE` slot
/// and consulted on every meta- / bund-sub-chord dispatch.
/// `ink.key.*` stdlib words mutate the same struct under the
/// shared RwLock.
#[derive(Debug, Clone)]
pub struct KeyBindings {
    /// Prefix chord that gates the meta sub-chord table (default
    /// `Ctrl+B`). Stored here so `ink.key.*` stdlib words can
    /// parse `"Ctrl+b m"` shorthand without taking a separate
    /// dependency on the App.
    pub meta_prefix: KeyChord,
    /// Same for the Bund sub-chord table (default `Ctrl+Z`).
    /// `None` when the user disabled it via empty config.
    pub bund_prefix: Option<KeyChord>,
    /// View-prefix chord (1.2.4+, default `Ctrl+V`). Gates the
    /// markdown-export / similar-mode / progress / paragraph-target
    /// sub-chords. `None` disables the layer entirely.
    pub view_prefix: Option<KeyChord>,
    pub meta_sub: Vec<BindingEntry>,
    pub bund_sub: Vec<BindingEntry>,
    pub view_sub: Vec<BindingEntry>,
    /// Top-level (no-prefix) chords. 1.2.4+ home for the F-keys
    /// that used to be hardcoded in `handle_key`. Single-token
    /// chord strings in HJSON `keys.bindings` (e.g. `"F1"`,
    /// `"Shift+F4"`) route here.
    pub top_level: Vec<BindingEntry>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self::defaults()
    }
}

impl KeyBindings {
    /// The canonical chord layout — must reproduce the behaviour
    /// of the hardcoded match arms `app.rs` had before Stage 1.
    /// Narrow-scoped entries come BEFORE broad ones (`Any`) so
    /// pane-specific bindings beat global ones when both match.
    pub fn defaults() -> Self {
        Self {
            meta_prefix: KeyChord::parse("Ctrl+b").expect("default meta_prefix"),
            bund_prefix: Some(KeyChord::parse("Ctrl+z").expect("default bund_prefix")),
            view_prefix: Some(KeyChord::parse("Ctrl+v").expect("default view_prefix")),
            meta_sub: vec![
                // ── Tree pane ─────────────────────────────────
                entry("c", Action::AddChapter, Scope::Tree),
                entry("s", Action::AddSubchapter, Scope::Tree),
                entry("p", Action::AddParagraph, Scope::Tree),
                entry("d", Action::DeleteNode, Scope::Tree),
                entry("m", Action::MorphType, Scope::Tree),
                entry("Up", Action::ReorderUp, Scope::Tree),
                entry("Down", Action::ReorderDown, Scope::Tree),
                // Reorder aliases used in the old keymap.
                entry("u", Action::ReorderUp, Scope::Tree),
                entry("j", Action::ReorderDown, Scope::Tree),

                // ── Editor pane ───────────────────────────────
                // 1.2.9+ — editor-scope Ctrl+B S was a redundant
                // duplicate of plain Ctrl+S; reclaimed for the new
                // TTS read-aloud action.  Tree-scope Ctrl+B S =
                // AddSubchapter stays.
                entry("s", Action::TtsReadParagraph, Scope::Editor),
                entry("n", Action::CreateSnapshot, Scope::Editor),
                entry("r", Action::CycleStatus, Scope::Editor),
                entry("f", Action::OpenFunctionPicker, Scope::Editor),
                entry("t", Action::RenameToFirstSentence, Scope::Editor),
                entry("m", Action::MorphType, Scope::Editor),
                entry("p", Action::LookupPlacesOrImage, Scope::Editor),
                entry("c", Action::LookupCharacters, Scope::Editor),
                entry("g", Action::LookupNotes, Scope::Editor),
                entry("y", Action::LookupArtefacts, Scope::Editor),

                // ── AI pane ───────────────────────────────────
                entry("c", Action::ClearChat, Scope::Ai),

                // ── Global (Any) ──────────────────────────────
                // H is pane-aware-content but pane-agnostic-binding —
                // every pane gets a "quickref" overlay tailored to
                // the focused area.
                entry("h", Action::OpenQuickref, Scope::Any),
                entry("v", Action::OpenCredits, Scope::Any),
                entry("i", Action::OpenBookInfo, Scope::Any),
                entry("l", Action::OpenLlmPicker, Scope::Any),
                entry("e", Action::ToggleSound, Scope::Any),
                entry("a", Action::ScheduleAssemble, Scope::Any),
                entry("b", Action::ScheduleBuild, Scope::Any),
                // 1.2.6+: Ctrl+B Shift+B → manual project backup.
                // Distinct chord from `Ctrl+B b` (lowercase build)
                // because the matcher tracks SHIFT separately.
                entry("Shift+b", Action::BackupNow, Scope::Any),
                entry("o", Action::ScheduleTake, Scope::Any),
                // 1.2.7+ — Ctrl+B U undoes the most-recent
                // paragraph delete (single-slot kill-ring).
                entry("u", Action::UndoLastDelete, Scope::Any),
                entry("w", Action::ToggleTypewriter, Scope::Any),
                entry("k", Action::ToggleAiFullscreen, Scope::Any),
                entry("1", Action::StatusFilterReady, Scope::Any),
                entry("2", Action::StatusFilterFinal, Scope::Any),
                entry("3", Action::StatusFilterThird, Scope::Any),
                entry("4", Action::StatusFilterSecond, Scope::Any),
                entry("5", Action::StatusFilterFirst, Scope::Any),
                entry("6", Action::StatusFilterNapkin, Scope::Any),
                entry("7", Action::StatusFilterNone, Scope::Any),
                // Tag picker (1.2.5+). `]` opens the per-paragraph
                // tag picker; `}` opens the search-by-tag picker.
                entry("]", Action::TagParagraph, Scope::Any),
                entry("}", Action::TagSearch, Scope::Any),
                // 1.2.8+ — Ctrl+B 0 → full-screen HJSON config
                // editor for `<project>/inkhaven.hjson`.  Digit
                // row, no modifier — unambiguous on every
                // terminal layout (previous `|` binding was
                // dropped on some terminals' chord state).
                entry("0", Action::BundEditProjectHjson, Scope::Any),
                // 1.2.15+ Phase D.3 — Ctrl+B Shift+0
                // opens the project-wide doctor panel.
                // Same digit row as Ctrl+B 0 (config
                // editor) so the "system inspection"
                // chord cluster lives together.
                entry("Shift+0", Action::OpenDoctorPanel, Scope::Any),
                // 1.2.9+ — Ctrl+B Shift+F toggles inline
                // style-warning overlays (filter words).
                entry("Shift+f", Action::ToggleStyleWarnings, Scope::Any),
                // 1.2.9+ — Ctrl+B Shift+R saves the
                // current paragraph as an audio file
                // via macOS `say -o`.
                entry("Shift+r", Action::TtsSaveAsAudio, Scope::Editor),
                // 1.2.9+ — Ctrl+B Shift+G opens the
                // writing-streak heatmap modal.
                entry("Shift+g", Action::OpenWritingStreakHeatmap, Scope::Any),
                // 1.2.9+ — Ctrl+B < / Ctrl+B > scene-break
                // navigation in the editor.  Originally
                // requested as `Shift+{` / `Shift+}`, but
                // `}` is already TagSearch (1.2.5).  `<`
                // and `>` are vim-style and free.
                entry("<", Action::SceneBreakPrev, Scope::Editor),
                entry(">", Action::SceneBreakNext, Scope::Editor),
                // 1.2.9+ — Ctrl+B Shift+L opens the project-
                // wide concordance modal.
                entry("Shift+l", Action::OpenConcordance, Scope::Any),
                // 1.2.9+ — Ctrl+B Shift+P toggles the
                // status-bar POV / character chip.
                entry("Shift+p", Action::TogglePovChip, Scope::Any),
                // 1.2.12+ — Ctrl+B Shift+N toggles
                // prompt-language resolution mode
                // (book_defined ↔ paragraph_detected).
                // Session-local; AI pane title bar
                // reflects the active mode.
                entry("Shift+n", Action::TogglePromptLanguageMode, Scope::Any),
                // 1.2.9+ — Ctrl+B Shift+H opens the
                // sentence-rhythm gauge modal.
                entry("Shift+h", Action::OpenSentenceRhythm, Scope::Editor),
                // 1.2.9+ — Ctrl+B Shift+T sends the
                // open paragraph to the LLM for a
                // show-don't-tell scan.
                entry("Shift+t", Action::AnalyseShowDontTell, Scope::Editor),
                // 1.2.11+ — Ctrl+B Shift+M asks the
                // LLM to rewrite the open paragraph
                // for rhythm variety; AI diff modal
                // pops on completion; snapshot
                // annotated "Sentence rhythm
                // rewrite" on accept.
                entry("Shift+m", Action::AiRewriteRhythm, Scope::Editor),
                // 1.2.13+ Phase C — Ctrl+B Q.
                // Translate the open paragraph INTO
                // an invented language defined under
                // the Language system book.
                entry("q", Action::TranslateToInvented, Scope::Editor),
                // 1.2.13+ Phase C.2 — Ctrl+B Shift+Q.
                // Reverse direction: translate FROM
                // invented back to the working
                // language.  Roundtrip test.
                entry("Shift+q", Action::TranslateFromInvented, Scope::Editor),
            ],
            bund_sub: vec![
                entry("r", Action::BundRunBuffer, Scope::Any),
                entry("n", Action::BundNewScript, Scope::Any),
                entry("e", Action::BundOpenEvalModal, Scope::Any),
                entry("?", Action::BundOpenScriptPicker, Scope::Any),
                // 1.2.8+ — embedded nushell pane.
                entry("o", Action::BundOpenShell, Scope::Any),
                entry("Shift+o", Action::BundOpenShellFresh, Scope::Any),
                entry("h", Action::BundShellSelection, Scope::Any),
            ],
            view_sub: vec![
                // Editor / AI-prompt: 1 = buffer markdown, 2 =
                // containing-subchapter subtree markdown.
                entry("1", Action::ViewExportMarkdownBuffer, Scope::Editor),
                entry("2", Action::ViewExportMarkdownSubchapter, Scope::Editor),
                entry("1", Action::ViewExportMarkdownBuffer, Scope::Ai),
                entry("2", Action::ViewExportMarkdownSubchapter, Scope::Ai),
                // Tree: 1 = subtree markdown.
                entry("1", Action::ViewExportMarkdownSubtree, Scope::Tree),
                // Global suffixes.
                entry("s", Action::ViewToggleSimilarMode, Scope::Any),
                entry("g", Action::ViewOpenProgress, Scope::Any),
                entry("t", Action::ViewOpenParagraphTarget, Scope::Any),
                entry("a", Action::ViewAddLink, Scope::Any),
                entry("i", Action::ViewAddIncomingLink, Scope::Any),
                entry("l", Action::ViewListLinks, Scope::Any),
                entry("k", Action::ViewListBacklinks, Scope::Any),
                entry("b", Action::ViewToggleBookmark, Scope::Any),
                // 1.2.12+ Phase D — sibling-book lookup
                // for the split-view secondary pane.
                entry("Shift+b", Action::ViewSiblingBookLookup, Scope::Any),
                entry("m", Action::ViewListBookmarks, Scope::Any),
                entry("p", Action::ViewFuzzyParagraphPicker, Scope::Any),
                // 1.2.7+ — same picker sorted by modified_at desc.
                entry("Shift+p", Action::ViewRecentParagraphPicker, Scope::Any),
                // 1.2.8+ — kill-ring picker (paragraph undelete history).
                entry("Shift+u", Action::ViewKillRingPicker, Scope::Any),
                // 1.2.8+ — hidden-character report on the open paragraph.
                entry("h", Action::ViewHiddenCharsReport, Scope::Any),
                // 1.2.8+ — show cursor breadcrumb on the status bar.
                entry("Shift+s", Action::ViewShowBreadcrumb, Scope::Any),
                entry("r", Action::ViewRenderParagraph, Scope::Any),
                entry("n", Action::ViewNextDiagnostic, Scope::Any),
                // 1.2.6+: case differentiates view scope. Plain
                // `w` opens the paragraph mini story view;
                // Shift+W opens the full book story view.
                entry("w", Action::ViewStoryGraphParagraph, Scope::Any),
                entry("Shift+W", Action::ViewStoryGraph, Scope::Any),
                // 1.2.6+ — timeline event picker.
                entry("e", Action::ViewEventPicker, Scope::Any),
                // 1.2.6+ — new event from any pane. Opens the
                // timeline view and immediately triggers the
                // new-event prompt, so a fresh project (zero
                // events) can add its first event without going
                // through `inkhaven event add` on the CLI.
                entry("Shift+e", Action::ViewNewEventPrompt, Scope::Any),
                // 1.2.6+ — edit timing of the open event ¶.
                entry("Shift+i", Action::ViewEditEventMetadata, Scope::Any),
                // 1.2.6+ — swim-lane timeline view. Bound to
                // Shift+T so the lowercase `t` chord stays free
                // for `ViewOpenParagraphTarget` (open the
                // paragraph link target under the cursor) — the two
                // used to collide on plain `t`, with the
                // earlier-listed `ViewOpenParagraphTarget`
                // shadowing this entry entirely.
                entry("Shift+t", Action::ViewTimeline, Scope::Any),
                // 1.2.14+ Phase A.2 — Ctrl+V Shift+H
                // opens the Threads picker.  H for tHread
                // (lowercase h is already
                // ViewHiddenCharsReport).
                entry("Shift+h", Action::ViewThreadsPicker, Scope::Any),
                // 1.2.14+ Phase A.3 — Ctrl+V Shift+A
                // fires the AI thread audit. A for
                // thread Audit (lowercase a is
                // ViewAddLink).
                entry("Shift+a", Action::AiThreadAudit, Scope::Any),
                // 1.2.14+ Phase D.4 — Ctrl+V Shift+D
                // pops the thread doctor modal
                // (TUI equivalent of CLI `thread
                // doctor`).
                entry("Shift+d", Action::ViewThreadDoctor, Scope::Any),
                // 1.2.14+ Phase C.1 — Ctrl+V c adds an
                // inline comment.  C for Comment.
                entry("c", Action::ViewAddComment, Scope::Editor),
                // 1.2.14+ Phase C.2 — Ctrl+V Shift+C
                // opens the project-wide comments
                // panel.
                entry("Shift+c", Action::ViewCommentsPanel, Scope::Any),
                // 1.2.14+ Phase Q.3 — Ctrl+V d
                // continuation drafting.
                entry("d", Action::AiContinuationDraft, Scope::Editor),
                // 1.2.14+ Phase Q.3 — Ctrl+V f
                // insert footnote.
                entry("f", Action::EditorInsertFootnote, Scope::Editor),
                // 1.2.14+ Phase Q.4 — Ctrl+V Shift+G
                // project goal modal.
                entry("Shift+g", Action::ViewProjectGoalModal, Scope::Any),
                // 1.2.14+ Phase Q.4 — Ctrl+V y
                // style-transfer rewrite.
                entry("y", Action::AiStyleTransferRewrite, Scope::Editor),
            ],
            top_level: vec![
                // F1 anywhere: Help-book RAG modal.
                entry("F1", Action::HelpQuery, Scope::Any),
                // F2: rename — pane-aware-content but bound in Tree
                // (where the cursor lives) + Editor (where rename
                // can still be triggered for the open paragraph).
                entry("F2", Action::RenameNode, Scope::Tree),
                entry("F2", Action::RenameNode, Scope::Editor),
                // F3: pane-specific file picker. Tree → import,
                // Editor → load.
                entry("F3", Action::FilePickerTreeImport, Scope::Tree),
                entry("F3", Action::FilePickerEditorLoad, Scope::Editor),
                // F4 / Ctrl+F4 — split-edit and "accept split".
                entry("F4", Action::ToggleSplit, Scope::Editor),
                entry("Ctrl+F4", Action::AcceptSplitSnapshot, Scope::Editor),
                // 1.2.12+ — Shift+F4 toggles fullscreen
                // two-paragraph split-view.  Editor-scope so
                // F4 / Ctrl+F4's existing meanings don't
                // shadow it.
                entry("Shift+F4", Action::ToggleSplitView, Scope::Editor),
                // F5 — snapshot the open paragraph (same as
                // Ctrl+B N inside meta_sub).
                entry("F5", Action::CreateSnapshot, Scope::Editor),
                // F6 — snapshot picker.
                entry("F6", Action::OpenSnapshotPicker, Scope::Editor),
                // F7 — grammar check.
                entry("F7", Action::GrammarCheck, Scope::Editor),
                // F8 (1.2.6+) — typst diagnostics list modal.
                // 1.2.7+ — scope widened to `Any` so the chord
                // fires from Tree / AI / Search panes too,
                // not just from a focused editor. The handler
                // re-focuses the editor when the modal opens.
                entry("F8", Action::DiagnosticsList, Scope::Any),
                // 1.2.7+ — Ctrl+Shift+M toggles mouse capture
                // so the user can drag-select text in the
                // editor / AI pane via the terminal's native
                // selection model + system clipboard copy.
                entry("Ctrl+Shift+m", Action::ToggleMouseCapture, Scope::Any),
                // 1.2.7+ — Alt+Left / Alt+Right back/forward
                // through visited-paragraph history.
                entry("Alt+Left", Action::VisitedBack, Scope::Any),
                entry("Alt+Right", Action::VisitedForward, Scope::Any),
                // F9 / F10 — global AI mode + inference toggle.
                entry("F9", Action::CycleAiMode, Scope::Any),
                entry("F10", Action::ToggleInferenceMode, Scope::Any),
                // Ctrl+F12 (1.2.6+) — AI explain the diagnostic
                // at the cursor. Used to live on bare F11, but
                // macOS grabs F11 (Show Desktop / Mission
                // Control) so it never reached the TUI. Anyone
                // who wants F11 back can rebind via HJSON
                // `keys.bindings` — see KEYS_REASSIGNMENT.md.
                entry("Ctrl+F12", Action::ExplainDiagnostic, Scope::Editor),
                // F12 (1.2.6+) — AI critique (mode-aware).
                entry("F12", Action::Critique, Scope::Editor),
            ],
        }
    }

    /// Resolve a single (top-level) keystroke against the
    /// `top_level` table — the home for F-keys after 1.2.4's
    /// migration.
    pub fn resolve_top_level(&self, ev: &KeyEvent, focus: Focus) -> Option<Action> {
        resolve_in(&self.top_level, ev, focus)
    }

    /// Resolve a meta sub-chord against the current focus. Returns
    /// `None` when no binding matches, `Some(Action::None)` when a
    /// binding was explicitly disabled by the user overlay.
    pub fn resolve_meta_sub(&self, ev: &KeyEvent, focus: Focus) -> Option<Action> {
        resolve_in(&self.meta_sub, ev, focus)
    }

    /// Same as `resolve_meta_sub` for chords after the bund_prefix.
    pub fn resolve_bund_sub(&self, ev: &KeyEvent, focus: Focus) -> Option<Action> {
        resolve_in(&self.bund_sub, ev, focus)
    }

    /// Same as `resolve_meta_sub` for chords after the view_prefix
    /// (1.2.4+, default Ctrl+V).
    pub fn resolve_view_sub(&self, ev: &KeyEvent, focus: Focus) -> Option<Action> {
        resolve_in(&self.view_sub, ev, focus)
    }

    /// Apply a list of `(layer, entry)` overlay pairs on top of
    /// the existing table. Each new entry replaces any existing
    /// `(chord, scope)` match in the same layer and gets
    /// prepended so it wins resolution against the defaults.
    pub fn apply_overlay(&mut self, overlay: Vec<(Layer, BindingEntry)>) {
        for (layer, new) in overlay {
            let table = self.layer_table_mut(layer);
            table.retain(|b| !(b.chord == new.chord && b.scope == new.scope));
            table.insert(0, new);
        }
    }

    fn layer_table_mut(&mut self, layer: Layer) -> &mut Vec<BindingEntry> {
        match layer {
            Layer::MetaSub => &mut self.meta_sub,
            Layer::BundSub => &mut self.bund_sub,
            Layer::ViewSub => &mut self.view_sub,
            Layer::TopLevel => &mut self.top_level,
        }
    }

    /// Build a `KeyBindings` from `defaults()` overlaid with the
    /// parsed HJSON `keys.bindings` entries. Caller supplies the
    /// already-parsed meta + bund + view prefixes so the overlay
    /// parser can route `"Ctrl+b m"` → meta_sub table by prefix
    /// match.
    pub fn from_overrides(
        meta_prefix: KeyChord,
        bund_prefix: Option<KeyChord>,
        view_prefix: Option<KeyChord>,
        overrides: &[(String, String, Option<String>)],
    ) -> Result<Self, String> {
        let mut bindings = Self::defaults();
        bindings.meta_prefix = meta_prefix;
        bindings.bund_prefix = bund_prefix;
        bindings.view_prefix = view_prefix;
        let mut overlay: Vec<(Layer, BindingEntry)> = Vec::new();
        for (chord_str, action_str, scope_str) in overrides {
            let entry = parse_overlay(
                meta_prefix,
                bund_prefix.unwrap_or_else(disabled_chord_placeholder),
                view_prefix.unwrap_or_else(disabled_chord_placeholder),
                chord_str,
                action_str,
                scope_str,
            )?;
            overlay.push(entry);
        }
        bindings.apply_overlay(overlay);
        Ok(bindings)
    }

    /// Add or replace a single binding. Used by `ink.key.bind` /
    /// `ink.key.bind_lambda`. Same `(chord, scope)` uniqueness
    /// semantics as the HJSON overlay: a new entry shadows any
    /// existing one with matching key.
    pub fn add(&mut self, layer: Layer, entry: BindingEntry) {
        let table = self.layer_table_mut(layer);
        table.retain(|b| !(b.chord == entry.chord && b.scope == entry.scope));
        table.insert(0, entry);
    }

    /// Remove every entry whose `(chord, scope)` matches. Returns
    /// the number of entries removed (zero when nothing matched).
    pub fn remove(&mut self, layer: Layer, chord: &KeyChord, scope: Scope) -> usize {
        let table = self.layer_table_mut(layer);
        let before = table.len();
        table.retain(|b| !(b.chord == *chord && b.scope == scope));
        before - table.len()
    }

    /// Parse a `"<prefix> <suffix>"` shorthand and return
    /// `(layer, suffix_chord)`. Used by `ink.key.*` stdlib words
    /// AND the HJSON overlay parser via `parse_overlay`.
    pub fn parse_sub_chord(&self, s: &str) -> Result<(Layer, KeyChord), String> {
        let parts: Vec<&str> = s.split_whitespace().collect();
        let (prefix_str, suffix_str) = match parts.as_slice() {
            [single] => {
                return Err(format!(
                    "chord `{single}`: top-level (no-prefix) binding not yet supported \
                     — use `<meta_prefix> <key>` or `<bund_prefix> <key>`"
                ));
            }
            [prefix, suffix] => (*prefix, *suffix),
            _ => return Err(format!("chord `{s}`: expected `<prefix> <suffix>`")),
        };
        let prefix = KeyChord::parse(prefix_str)
            .map_err(|e| format!("chord `{s}` prefix: {e}"))?;
        let suffix = KeyChord::parse(suffix_str)
            .map_err(|e| format!("chord `{s}` suffix: {e}"))?;
        let layer = if prefix == self.meta_prefix {
            Layer::MetaSub
        } else if Some(prefix) == self.bund_prefix {
            Layer::BundSub
        } else if Some(prefix) == self.view_prefix {
            Layer::ViewSub
        } else {
            return Err(format!(
                "chord `{s}`: prefix `{prefix_str}` is not meta_prefix / bund_prefix / view_prefix"
            ));
        };
        if suffix == self.meta_prefix
            || Some(suffix) == self.bund_prefix
            || Some(suffix) == self.view_prefix
        {
            return Err(format!(
                "chord `{s}`: suffix collides with a prefix chord"
            ));
        }
        Ok((layer, suffix))
    }
}

impl KeyBindings {
    /// Build the status-bar hint string for the meta-prefix
    /// chord on the given focus. Iterates `meta_sub` in
    /// registration order, skipping disabled entries and
    /// deduplicating actions (so `Up` + `u` for ReorderUp
    /// surface as one entry).
    pub fn meta_hint(&self, focus: Focus) -> String {
        self.hint_for(&self.meta_sub, "META", focus)
    }

    /// Same for the bund-prefix chord.
    pub fn bund_hint(&self, focus: Focus) -> String {
        self.hint_for(&self.bund_sub, "BUND", focus)
    }

    /// Same for the view-prefix chord (1.2.4+, default Ctrl+V).
    pub fn view_hint(&self, focus: Focus) -> String {
        self.hint_for(&self.view_sub, "VIEW", focus)
    }

    fn hint_for(&self, table: &[BindingEntry], prefix: &str, focus: Focus) -> String {
        use std::collections::HashSet;
        let mut parts: Vec<String> = vec![prefix.to_string()];
        let mut seen: HashSet<String> = HashSet::new();
        for entry in table {
            if !entry.scope.matches(focus) {
                continue;
            }
            if matches!(entry.action, Action::None) {
                continue;
            }
            let label = entry.action.label();
            if label.is_empty() {
                continue;
            }
            // De-dupe by action label: a user who bound the same
            // action to two chords (e.g. ReorderUp on Up and u)
            // only sees the action once in the hint.
            if !seen.insert(label.clone()) {
                continue;
            }
            parts.push(format!("{} {}", entry.chord.to_display_string(), label));
        }
        parts.push("Esc cancel".into());
        parts.join(" · ")
    }
}

/// Placeholder chord matched by nothing real — used to satisfy
/// `parse_overlay`'s `bund_prefix` arg when the user disabled the
/// bund prefix via empty config.
fn disabled_chord_placeholder() -> KeyChord {
    KeyChord {
        code: crossterm::event::KeyCode::Null,
        modifiers: crossterm::event::KeyModifiers::NONE,
    }
}

/// Which sub-chord table the overlay entry targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    MetaSub,
    BundSub,
    /// 1.2.4+: Ctrl+V family — markdown export / similar mode /
    /// progress / paragraph target.
    ViewSub,
    /// 1.2.4+: top-level (no-prefix) chords — home for the
    /// F-keys after the migration. HJSON `keys.bindings` chord
    /// strings that contain a single token (no prefix) land here.
    TopLevel,
}

fn parse_overlay(
    meta_prefix: KeyChord,
    bund_prefix: KeyChord,
    view_prefix: KeyChord,
    chord: &str,
    action: &str,
    scope: &Option<String>,
) -> Result<(Layer, BindingEntry), String> {
    // Shorthand split: "Ctrl+b y" → ["Ctrl+b", "y"]. Trim runs of
    // whitespace so "Ctrl+b   y" also parses cleanly.
    let parts: Vec<&str> = chord.split_whitespace().collect();
    // 1.2.4+: single-token chord strings (e.g. `"F1"`, `"Shift+F4"`)
    // bind into the `top_level` table — no prefix required.
    if parts.len() == 1 {
        let single = KeyChord::parse(parts[0])
            .map_err(|e| format!("binding chord `{chord}`: {e}"))?;
        let action_enum = parse_action(action)?;
        let scope_enum = parse_scope(scope.as_deref())?;
        return Ok((
            Layer::TopLevel,
            BindingEntry {
                chord: single,
                action: action_enum,
                scope: scope_enum,
            },
        ));
    }
    let (prefix_str, suffix_str) = match parts.as_slice() {
        [prefix, suffix] => (*prefix, *suffix),
        _ => {
            return Err(format!(
                "binding chord `{chord}`: expected `<prefix> <suffix>` (two tokens) or single top-level chord"
            ));
        }
    };
    let prefix = KeyChord::parse(prefix_str)
        .map_err(|e| format!("binding chord `{chord}` prefix: {e}"))?;
    let suffix = KeyChord::parse(suffix_str)
        .map_err(|e| format!("binding chord `{chord}` suffix: {e}"))?;
    let layer = if prefix == meta_prefix {
        Layer::MetaSub
    } else if prefix == bund_prefix {
        Layer::BundSub
    } else if prefix == view_prefix {
        Layer::ViewSub
    } else {
        return Err(format!(
            "binding chord `{chord}`: prefix `{prefix_str}` is not meta_prefix / bund_prefix / view_prefix"
        ));
    };
    // Reject rebinding the prefixes themselves and the hard-quit
    // chord — those are configured via top-level `keys.*` slots,
    // not the bindings overlay.
    if suffix == meta_prefix || suffix == bund_prefix || suffix == view_prefix {
        return Err(format!(
            "binding chord `{chord}`: suffix collides with a prefix chord"
        ));
    }
    let scope = parse_scope(scope.as_deref())?;
    let action = parse_action(action)?;
    Ok((
        layer,
        BindingEntry {
            chord: suffix,
            action,
            scope,
        },
    ))
}

fn parse_scope(s: Option<&str>) -> Result<Scope, String> {
    match s {
        None | Some("any") => Ok(Scope::Any),
        Some("editor") => Ok(Scope::Editor),
        Some("tree") => Ok(Scope::Tree),
        Some("ai") => Ok(Scope::Ai),
        Some(other) => Err(format!(
            "scope `{other}`: expected one of any / editor / tree / ai"
        )),
    }
}

fn parse_action(s: &str) -> Result<Action, String> {
    // Round-trip via serde: variant rename attributes give us the
    // canonical dotted form. `serde_json::from_str` reads a JSON
    // string literal and matches it against the rename map.
    serde_json::from_str::<Action>(&format!("\"{s}\""))
        .map_err(|e| format!("action `{s}`: {e}"))
}

fn resolve_in(table: &[BindingEntry], ev: &KeyEvent, focus: Focus) -> Option<Action> {
    table
        .iter()
        .find(|b| b.scope.matches(focus) && b.chord.matches(ev))
        .map(|b| b.action.clone())
}

fn entry(chord: &str, action: Action, scope: Scope) -> BindingEntry {
    BindingEntry {
        chord: KeyChord::parse(chord).expect("invalid default chord — programmer error"),
        action,
        scope,
    }
}

// ── Shared active KeyBindings ────────────────────────────────────────
//
// App reads from this on every chord dispatch; `ink.key.*` Bund
// stdlib writes to it. Lazily initialised with `KeyBindings::defaults()`
// on first access — so CLI subcommands (`inkhaven bund`) that don't
// build an `App` still see a functioning binding table.
//
// `install` replaces the contents under the write lock, so TUI
// startup (which parses the HJSON overlay) wins over the lazy
// defaults whenever it runs.

static ACTIVE: LazyLock<RwLock<KeyBindings>> =
    LazyLock::new(|| RwLock::new(KeyBindings::defaults()));

/// Replace the active KeyBindings. Called by `App::new` after
/// applying the HJSON overlay. Cheap because the new value is
/// move-swapped under the write lock.
pub fn install(bindings: KeyBindings) {
    *ACTIVE.write() = bindings;
}

/// Read access. Lazy default-init means this never blocks on
/// missing installation — CLI smoke usage gets defaults.
pub fn read() -> RwLockReadGuard<'static, KeyBindings> {
    ACTIVE.read()
}

/// Write access for `ink.key.*` Bund stdlib words.
pub fn write() -> RwLockWriteGuard<'static, KeyBindings> {
    ACTIVE.write()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn ev(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn defaults_resolve_known_chords() {
        let k = KeyBindings::defaults();
        // Tree pane: C → add chapter
        assert_eq!(
            k.resolve_meta_sub(&ev('c'), Focus::Tree),
            Some(Action::AddChapter)
        );
        // Editor pane: C → character lookup (different action,
        // same key — scope discriminates).
        assert_eq!(
            k.resolve_meta_sub(&ev('c'), Focus::Editor),
            Some(Action::LookupCharacters)
        );
        // AI pane: C → clear chat
        assert_eq!(
            k.resolve_meta_sub(&ev('c'), Focus::Ai),
            Some(Action::ClearChat)
        );
        // V is global → open credits regardless of pane
        assert_eq!(
            k.resolve_meta_sub(&ev('v'), Focus::Tree),
            Some(Action::OpenCredits)
        );
        assert_eq!(
            k.resolve_meta_sub(&ev('v'), Focus::Editor),
            Some(Action::OpenCredits)
        );
    }

    #[test]
    fn view_sub_t_and_shift_t_route_to_distinct_actions() {
        // 1.2.6+ — `Ctrl+V t` opens the paragraph link target,
        // `Ctrl+V Shift+T` opens the timeline. They used to
        // collide on plain `t` (the second binding was shadowed
        // and dead).
        let k = KeyBindings::defaults();
        let lower = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE);
        assert_eq!(
            k.resolve_view_sub(&lower, Focus::Editor),
            Some(Action::ViewOpenParagraphTarget)
        );
        let upper = KeyEvent::new(KeyCode::Char('T'), KeyModifiers::SHIFT);
        assert_eq!(
            k.resolve_view_sub(&upper, Focus::Editor),
            Some(Action::ViewTimeline)
        );
    }

    #[test]
    fn lowercase_b_and_shift_b_are_distinct_actions() {
        let k = KeyBindings::defaults();
        // Ctrl+B b (lowercase) → build the book (unchanged).
        let lower = KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE);
        assert_eq!(
            k.resolve_meta_sub(&lower, Focus::Editor),
            Some(Action::ScheduleBuild)
        );
        // Ctrl+B Shift+B (uppercase) → manual backup. Different
        // chord, different action — the matcher uppercases the
        // event's char when SHIFT is set so 'B'+SHIFT and 'b'+none
        // route to different entries.
        let upper = KeyEvent::new(KeyCode::Char('B'), KeyModifiers::SHIFT);
        assert_eq!(
            k.resolve_meta_sub(&upper, Focus::Editor),
            Some(Action::BackupNow)
        );
    }

    #[test]
    fn pane_scope_beats_any() {
        let k = KeyBindings::defaults();
        // In editor, P → places-or-image (Editor scope), NOT add
        // paragraph (Tree scope). Both are listed; narrow scope
        // wins.
        assert_eq!(
            k.resolve_meta_sub(&ev('p'), Focus::Editor),
            Some(Action::LookupPlacesOrImage)
        );
        // In tree, P → add paragraph.
        assert_eq!(
            k.resolve_meta_sub(&ev('p'), Focus::Tree),
            Some(Action::AddParagraph)
        );
    }

    #[test]
    fn status_filter_digits() {
        let k = KeyBindings::defaults();
        for (c, expected) in [
            ('1', Action::StatusFilterReady),
            ('2', Action::StatusFilterFinal),
            ('3', Action::StatusFilterThird),
            ('4', Action::StatusFilterSecond),
            ('5', Action::StatusFilterFirst),
            ('6', Action::StatusFilterNapkin),
            ('7', Action::StatusFilterNone),
        ] {
            assert_eq!(
                k.resolve_meta_sub(&ev(c), Focus::Editor),
                Some(expected),
                "digit {c}"
            );
        }
    }

    #[test]
    fn bund_sub_known_chords() {
        let k = KeyBindings::defaults();
        assert_eq!(
            k.resolve_bund_sub(&ev('r'), Focus::Tree),
            Some(Action::BundRunBuffer)
        );
        assert_eq!(
            k.resolve_bund_sub(&ev('n'), Focus::Editor),
            Some(Action::BundNewScript)
        );
        assert_eq!(
            k.resolve_bund_sub(&ev('e'), Focus::Ai),
            Some(Action::BundOpenEvalModal)
        );
    }

    #[test]
    fn unknown_chord_is_none() {
        let k = KeyBindings::defaults();
        assert_eq!(k.resolve_meta_sub(&ev('z'), Focus::Editor), None);
    }
}
