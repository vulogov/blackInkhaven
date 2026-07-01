//! RESRCH-1 — the Research Assistant application state + event loop.
//!
//! R-P1 — a synchronous crossterm `poll()`/`read()` loop (the writing TUI's
//! pattern), focus cycling, the hints toggle, and `q` / `Ctrl+C` exit. The
//! panes are placeholders; later phases fill them (Facts tree R-P4, chat R-P6,
//! query prompt R-P5) and add the streaming receiver drained per tick (R-P7).

use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::Backend;

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;

use tokio::sync::mpsc;
use tui_textarea::TextArea;
use uuid::Uuid;

use crate::ai::stream::{ChatTurn as AiTurn, StreamMsg, spawn_chat_stream};

use super::Focus;
use super::chat::ChatTurn;
use super::extract::{self, TargetBook};
use super::facts_tree::FactsTree;
use super::llm;
use super::render;
use super::thread::{self, ResearchThread, ResearchTurn, TurnKind};
use crate::tui::theme::Theme;

/// G7 — the inline manual fact-entry overlay: title first, then body.
pub(super) struct ManualEntry {
    pub stage: ManualStage,
    pub title: String,
    pub body: String,
}

#[derive(PartialEq, Eq)]
pub(super) enum ManualStage {
    Title,
    Body,
}

/// RESRCH-2.1 — provenance metadata carried from extraction through to insert.
#[derive(Clone, Default)]
pub(super) struct ProvMeta {
    pub origin: String, // model | manual | promoted
    pub query: String,  // the source research query (for `model`)
    pub detail: String, // the notes path (for `promoted`)
}

/// R-P10/R-P11 — an in-flight extraction stream feeding the confirmation overlay.
pub(super) struct ExtractState {
    rx: mpsc::UnboundedReceiver<StreamMsg>,
    buf: String,
    book: TargetBook,
    book_id: Uuid,
    target: Option<Uuid>,
    command: String,
    prov: ProvMeta,
    /// R2-E — index of the transient streaming preview turn (the extraction's
    /// tokens render live, dim); removed when the confirmation overlay opens.
    preview_idx: Option<usize>,
}

/// G1/G2 — the editable insertion confirmation overlay.
pub(super) struct ConfirmationState {
    pub title: TextArea<'static>,
    pub body: TextArea<'static>,
    pub book: TargetBook,
    pub book_id: Uuid,
    pub target: Option<Uuid>,
    pub field: ConfirmField,
    pub command: String,
    pub prov: ProvMeta,
    /// RESRCH-2.1 (T-P4) — a near-duplicate warning that must be confirmed twice.
    pub dup_warning: Option<String>,
    /// UX-P4 — the near-duplicate fact's body, shown beside the pending fact.
    pub dup_body: Option<String>,
    /// R2-C (WC-P3) — the pre-commit fact-check verdict for a web fact (set once);
    /// a non-ACCURATE verdict requires a second confirm.
    pub fc_checked: bool,
    pub fc_verdict: Option<String>,
    /// UX-P4 — the full multi-line verdict text (per-source triangulation lines /
    /// the fact-check reason), rendered in the overlay evidence panel.
    pub fc_detail: Option<String>,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub(super) enum ConfirmField {
    Title,
    Body,
}

/// G8 — chat search state.
pub(super) struct ChatSearch {
    pub query: String,
    pub current: usize,
}

/// Tree-edit single-line input overlay (rename / add chapter / add subchapter).
pub(super) struct TreeInput {
    pub kind: TreeInputKind,
    pub buf: String,
    /// For rename: the node being renamed. For add: the parent to create under.
    pub target: Uuid,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub(super) enum TreeInputKind {
    Rename,
    NewChapter,
    NewSubchapter,
}

/// Tree clipboard mode (copy keeps the source; move relocates it).
#[derive(PartialEq, Eq, Clone, Copy)]
pub(super) enum ClipMode {
    Copy,
    Move,
}

impl TreeInputKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            TreeInputKind::Rename => "Rename",
            TreeInputKind::NewChapter => "New chapter",
            TreeInputKind::NewSubchapter => "New subchapter",
        }
    }
}

/// `/web` (R2-C) — an in-flight web search; on completion the results either
/// ground an LLM chat answer (`ingest=false`) or are embedded as research
/// sources (`ingest=true`).
pub(super) struct WebState {
    rx: mpsc::UnboundedReceiver<std::result::Result<Vec<super::web::WebResult>, String>>,
    ingest: bool,
    query: String,
}

/// R3-A — an in-flight `/wikidata` fetch (async task → single-result channel).
pub(super) struct WikidataState {
    rx: mpsc::UnboundedReceiver<std::result::Result<super::wikidata::WdEntity, String>>,
    query: String,
}

/// RESRCH-GUTENBERG — an in-flight `/gutenberg` search + text fetch.
pub(super) struct GutenbergState {
    rx: mpsc::UnboundedReceiver<
        std::result::Result<(super::gutenberg::GutenbergBook, String), String>,
    >,
    query: String,
}

/// R3-B — an in-flight `/openalex` or `/arxiv` fetch.
pub(super) struct ScholarlyState {
    rx: mpsc::UnboundedReceiver<std::result::Result<super::scholarly::Paper, String>>,
    query: String,
}

/// R3-E — the evidence-gathering phase of `/triangulate` (source label → text);
/// once it arrives, an LLM judges each source and the answer streams as usual.
pub(super) struct TriangulateState {
    rx: mpsc::UnboundedReceiver<Vec<(String, String)>>,
    claim: String,
}

/// R3-E — the `/fact` triangulation gate: gather evidence, then have the LLM
/// judge it; a weak verdict requires a second confirm (like the web gate).
enum TriGatePhase {
    Gather(mpsc::UnboundedReceiver<Vec<(String, String)>>),
    Judge { rx: mpsc::UnboundedReceiver<StreamMsg>, buf: String },
}
pub(super) struct TriGate {
    phase: TriGatePhase,
    claim: String,
}

/// RESRCH-5 (R5-E) — `/upgrade`: gather structured-source evidence about a
/// `model` fact, then judge which source corroborates it and raise its
/// provenance tier. Non-destructive to the fact text.
enum UpgradePhase {
    Gather(mpsc::UnboundedReceiver<Vec<(String, String)>>),
    Judge { rx: mpsc::UnboundedReceiver<StreamMsg>, buf: String },
}
pub(super) struct UpgradeState {
    node_id: Uuid,
    claim: String,
    query: String,
    phase: UpgradePhase,
    preview_idx: Option<usize>,
}

/// `/factcheck` — a multi-call corpus audit: truth (per-fact, chunked) then
/// consistency (the whole set). Drained in the run loop like the chain.
pub(super) struct FactCheckState {
    facts: Vec<super::factcheck::FactEntry>,
    /// `true` while running truth chunks; `false` during the consistency pass.
    truth_phase: bool,
    chunk_idx: usize,
    truth_report: String,
    /// R2-E — the bounded consistency calls (built when the truth pass ends).
    consist_groups: Vec<super::factcheck::ConsistGroup>,
    consist_idx: usize,
    consist_report: String,
    rx: Option<mpsc::UnboundedReceiver<StreamMsg>>,
    buf: String,
    /// R2-E — index of the streaming turn showing the in-flight call's tokens
    /// (dim); reused to hold the final report when the audit completes.
    preview_idx: Option<usize>,
    /// RESRCH-UNDISPUTED (UD-P2) — count of authorial facts excluded from this
    /// audit, reported in the final report.
    undisputed: usize,
}

/// RESRCH-5 (R5-A/B/C) — the kind of grounded output over the retrieved facts.
#[derive(Clone, Copy)]
pub(super) enum GroundedKind {
    Synthesize,
    Outline,
    Gaps,
}

impl GroundedKind {
    fn command(self) -> &'static str {
        match self {
            GroundedKind::Synthesize => "synthesize",
            GroundedKind::Outline => "outline",
            GroundedKind::Gaps => "gaps",
        }
    }

    fn gerund(self) -> &'static str {
        match self {
            GroundedKind::Synthesize => "synthesizing",
            GroundedKind::Outline => "outlining",
            GroundedKind::Gaps => "finding gaps",
        }
    }

    fn system_prompt(self, language: &str) -> String {
        match self {
            GroundedKind::Synthesize => format!(
                "You are synthesizing a grounded overview from a writer's own VERIFIED facts. Use ONLY the \
                 facts below — do not add outside knowledge. Weave them into a clear, coherent synthesis of \
                 the topic, and CITE each claim by its [breadcrumb]. Where the facts are thin or silent on \
                 part of the topic, say so explicitly rather than inventing. Write in {language}."
            ),
            GroundedKind::Outline => format!(
                "You are drafting a STRUCTURED OUTLINE for writing about a topic, drawing ONLY on the \
                 writer's verified facts below — do not add outside knowledge. Produce nested bullet \
                 points / sections; under each point, CITE the supporting fact by its [breadcrumb]. Mark \
                 any point the facts don't cover with '(needs research)'. Write in {language}."
            ),
            GroundedKind::Gaps => format!(
                "You are finding what is MISSING to write well about a topic. Given ONLY the writer's \
                 facts below, list the OPEN QUESTIONS and gaps — specific things the corpus does NOT \
                 answer — as a numbered list, each a single concrete question. Do NOT answer them and do \
                 NOT invent facts; only identify what's absent. Write in {language}."
            ),
        }
    }
}

/// UX-P5 — the fact quick-view modal: Enter on a fact opens a scrollable view of
/// its body; Esc closes. Read-only.
pub(super) struct PeekState {
    pub title: String,
    pub body: String,
    pub scroll: u16,
}

/// RESRCH-UNDISPUTED (UD-P3) — the `/undisputed` common-sense pass over the
/// authorial facts (chunked, like `/factcheck`'s truth phase; read-only).
pub(super) struct UndisputedState {
    facts: Vec<super::factcheck::FactEntry>,
    chunk_idx: usize,
    report: String,
    rx: Option<mpsc::UnboundedReceiver<StreamMsg>>,
    buf: String,
    preview_idx: Option<usize>,
}

/// R-P15 — `/chain`: a sequential research pipeline. Each step's response is
/// accumulated as context for the next.
pub(super) struct ChainState {
    steps: Vec<String>,
    current: usize,
    accumulated: Vec<String>,
    rx: Option<mpsc::UnboundedReceiver<StreamMsg>>,
    turn_idx: usize,
}

pub(crate) struct ResearchApp {
    pub(super) layout: ProjectLayout,
    pub(super) cfg: Config,
    pub(super) store: Store,
    pub(super) hierarchy: Hierarchy,
    /// Decoded colour scheme from the project / global `theme:` config.
    pub(super) theme: Theme,

    /// The open, persistent research thread (R-P2).
    pub(super) thread: ResearchThread,

    /// The Facts tree pane (R-P4).
    pub(super) facts_tree: FactsTree,
    /// RE-P5 — per-fact `/factcheck` verdicts (✓ / ? / ✗ glyphs in the tree;
    /// seed for `/whatswrong`). Loaded from the sidecar, refreshed each audit.
    pub(super) fact_verdicts: super::verdicts::Verdicts,
    /// UX-P2 — per-fact provenance (source-tier glyph in the tree). Reloaded on
    /// each `reload_hierarchy` so newly-inserted facts show their tier.
    pub(super) fact_provenance: super::provenance::Provenance,
    /// UD-P4 — per-fact `/undisputed` common-sense verdicts; colours the `※`
    /// glyph (plausible/odd/incoherent).
    pub(super) undisputed_verdicts: super::verdicts::Verdicts,
    /// Pinned Facts nodes (G4); RAG injection lands in R-P8.
    pub(super) pinned_nodes: Vec<Uuid>,
    /// The inline manual-entry overlay, when active (G7).
    pub(super) manual: Option<ManualEntry>,
    /// Tree-edit input overlay (rename / add branch), when active.
    pub(super) tree_input: Option<TreeInput>,
    /// A pending delete awaiting y/N confirmation (the target node id).
    pub(super) pending_delete: Option<Uuid>,
    /// Yanked / cut node awaiting paste (copy/move from the Outline, RFC parity).
    pub(super) tree_clipboard: Option<(Uuid, ClipMode)>,

    /// The two-line query prompt (R-P5).
    pub(super) query: TextArea<'static>,
    /// Recallable prompt history (newest first), with the cursor into it.
    pub(super) prompt_history: Vec<String>,
    pub(super) prompt_history_idx: Option<usize>,
    /// The in-progress draft saved when history recall begins (restored on ↓).
    pub(super) draft_backup: String,

    /// The chat transcript (R-P6) + scroll offset (lines from the bottom).
    pub(super) chat_history: Vec<ChatTurn>,
    pub(super) chat_scroll: u16,
    /// G8 — chat search: the query + current-match ordinal (render finds the
    /// matching lines, where the window height is known).
    pub(super) chat_search: Option<ChatSearch>,

    /// In-flight stream (R-P7): the receiver + the chat-turn index it feeds.
    stream_rx: Option<mpsc::UnboundedReceiver<StreamMsg>>,
    streaming_turn: Option<usize>,
    /// In-flight extraction (R-P10/R-P11) feeding the confirmation overlay.
    extracting: Option<ExtractState>,
    /// In-flight `/verify` probe (R-P14): receiver + accumulated buffer.
    verify_rx: Option<(mpsc::UnboundedReceiver<StreamMsg>, String)>,
    /// In-flight `/chain` pipeline (R-P15).
    chain: Option<ChainState>,
    /// In-flight `/factcheck` corpus audit.
    factcheck: Option<FactCheckState>,
    undisputed_check: Option<UndisputedState>,
    /// In-flight `/web` search (R2-C).
    web: Option<WebState>,
    wikidata_state: Option<WikidataState>,
    gutenberg_state: Option<GutenbergState>,
    scholarly_state: Option<ScholarlyState>,
    triangulate: Option<TriangulateState>,
    tri_gate: Option<TriGate>,
    upgrade: Option<UpgradeState>,
    /// R3-B — a pending SOURCES-1 citation to write when the current `/fact` from
    /// a paper is confirmed (set at extraction, consumed on insert).
    pending_cite: Option<crate::sources::BibEntry>,
    /// In-flight pre-commit fact-check for a web fact (WC-P3): receiver + buffer.
    fc_confirm: Option<(mpsc::UnboundedReceiver<StreamMsg>, String)>,
    /// The editable insertion confirmation overlay (G1/G2).
    pub(super) confirmation: Option<ConfirmationState>,
    /// UX-P3 — spinner frame counter + when the current async op began (for the
    /// status-bar spinner + elapsed timer while a fetch/stream is in flight).
    pub(super) spin_tick: usize,
    pub(super) async_started: Option<std::time::Instant>,
    /// Accumulated session cost estimate (USD).
    pub(super) session_cost: f64,
    /// R2-E — true while every turn so far was priced from real provider token
    /// usage; flips false once any turn falls back to the char heuristic (drives
    /// the `$` vs `~$` status-bar prefix).
    pub(super) session_cost_exact: bool,
    /// Whether the `session_budget_warn` notice has fired this session.
    budget_warned: bool,

    pub(super) focus: Focus,
    pub(super) show_hints: bool,
    pub(super) split_ratio: u32,
    /// UX-P5 — the fact quick-view modal (Enter on a fact), when open.
    pub(super) peek: Option<PeekState>,
    /// UX-P5 — a zoomed (full-screen) pane, when active (FactsTree or AiChat).
    pub(super) zoom: Option<Focus>,
    pub(super) status_message: Option<String>,
    /// `Ctrl+B` prefix is armed, waiting for the second key (e.g. `h`).
    ctrl_b_pending: bool,
    /// The full quick-reference overlay (Ctrl+B h) is open.
    pub(super) show_help: bool,

    should_quit: bool,
}

impl ResearchApp {
    pub(crate) fn new(
        layout: ProjectLayout,
        cfg: Config,
        store: Store,
        hierarchy: Hierarchy,
        thread_name: Option<String>,
    ) -> Result<ResearchApp> {
        // R-P2: open (or create) the requested thread; default when unnamed.
        // The full thread picker for the >1-thread case lands in R-P3.
        let name = thread_name.unwrap_or_else(|| "default".to_string());
        let now = chrono::Utc::now().to_rfc3339();
        let thread = ResearchThread::open_or_create(&layout, &name, now)?;
        let facts_tree = FactsTree::new(&hierarchy);
        // G4 — restore pins persisted in the thread (skip any that no longer exist).
        let pinned_nodes: Vec<Uuid> = thread
            .pinned_nodes
            .iter()
            .filter_map(|s| Uuid::parse_str(s).ok())
            .filter(|id| hierarchy.get(*id).is_some())
            .collect();
        let prompt_history = build_prompt_history(&thread);
        let show_hints = cfg.research.show_keybind_hints;
        let split_ratio = cfg.research.split_ratio;
        let theme = Theme::from_config(&cfg.theme);
        let fact_verdicts = super::verdicts::Verdicts::load(&layout);
        let fact_provenance = super::provenance::Provenance::load(&layout);
        let undisputed_verdicts = super::verdicts::Verdicts::load_undisputed(&layout);
        Ok(ResearchApp {
            layout,
            cfg,
            store,
            hierarchy,
            theme,
            thread,
            facts_tree,
            fact_verdicts,
            fact_provenance,
            undisputed_verdicts,
            pinned_nodes,
            manual: None,
            tree_input: None,
            pending_delete: None,
            tree_clipboard: None,
            query: TextArea::default(),
            prompt_history,
            prompt_history_idx: None,
            draft_backup: String::new(),
            chat_history: Vec::new(),
            chat_scroll: 0,
            chat_search: None,
            stream_rx: None,
            streaming_turn: None,
            extracting: None,
            verify_rx: None,
            chain: None,
            factcheck: None,
            undisputed_check: None,
            web: None,
            wikidata_state: None,
            gutenberg_state: None,
            scholarly_state: None,
            triangulate: None,
            tri_gate: None,
            upgrade: None,
            pending_cite: None,
            fc_confirm: None,
            confirmation: None,
            spin_tick: 0,
            async_started: None,
            session_cost: 0.0,
            session_cost_exact: true,
            budget_warned: false,
            focus: Focus::QueryPrompt,
            show_hints,
            split_ratio,
            peek: None,
            zoom: None,
            status_message: None,
            ctrl_b_pending: false,
            show_help: false,
            should_quit: false,
        })
    }

    /// Cap on pinned nodes (`research.max_pinned_nodes`).
    fn max_pinned(&self) -> usize {
        self.cfg.research.max_pinned_nodes.max(1)
    }

    /// UX-P3 — is any async operation (stream / fetch / gate) in flight?
    pub(super) fn is_busy(&self) -> bool {
        self.stream_rx.is_some()
            || self.extracting.is_some()
            || self.verify_rx.is_some()
            || self.chain.is_some()
            || self.factcheck.is_some()
            || self.undisputed_check.is_some()
            || self.web.is_some()
            || self.wikidata_state.is_some()
            || self.gutenberg_state.is_some()
            || self.scholarly_state.is_some()
            || self.triangulate.is_some()
            || self.tri_gate.is_some()
            || self.upgrade.is_some()
            || self.fc_confirm.is_some()
    }

    /// The synchronous event loop: draw, then block up to 100 ms for a key.
    /// Later phases also drain the `StreamMsg` receiver here each tick (R-P7).
    pub(crate) fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        while !self.should_quit {
            self.poll_stream();
            self.poll_extraction();
            self.poll_verify();
            self.poll_chain();
            self.poll_factcheck();
            self.poll_undisputed();
            self.poll_web();
            self.poll_wikidata();
            self.poll_gutenberg();
            self.poll_scholarly();
            self.poll_triangulate();
            self.poll_tri_gate();
            self.poll_upgrade();
            self.poll_fc_confirm();
            // UX-P3 — drive the status-bar spinner: start the timer when work is
            // in flight, clear it when the last async op finishes.
            self.spin_tick = self.spin_tick.wrapping_add(1);
            if self.is_busy() {
                if self.async_started.is_none() {
                    self.async_started = Some(std::time::Instant::now());
                }
            } else {
                self.async_started = None;
            }
            terminal.draw(|f| render::render(f, self))?;
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Release {
                        self.on_key(key);
                    }
                }
            }
        }
        Ok(())
    }

    fn on_key(&mut self, key: KeyEvent) {
        // Ctrl+C and Ctrl+Q always exit, from any pane or overlay (the plain `q`
        // only quits outside a text field; Ctrl+Q is the universal escape hatch).
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q'))
        {
            self.should_quit = true;
            return;
        }
        // The quick-reference overlay (Ctrl+B h) swallows the next key to close.
        if self.show_help {
            self.show_help = false;
            return;
        }
        // `Ctrl+B` arms a prefix; the next key resolves it (`h` = help).
        if self.ctrl_b_pending {
            self.ctrl_b_pending = false;
            if matches!(key.code, KeyCode::Char('h') | KeyCode::Char('H')) {
                self.show_help = true;
            }
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('b') {
            self.ctrl_b_pending = true;
            return;
        }
        // The manual-entry overlay (G7) captures all keys while active.
        if self.manual.is_some() {
            self.manual_entry_key(key);
            return;
        }
        // The confirmation overlay (G1/G2) captures all keys while active.
        if self.confirmation.is_some() {
            self.confirmation_key(key);
            return;
        }
        // Tree-edit input overlay + delete confirmation capture keys too.
        if self.tree_input.is_some() {
            self.tree_input_key(key);
            return;
        }
        if self.pending_delete.is_some() {
            self.delete_confirm_key(key);
            return;
        }
        // UX-P5 — the fact quick-view modal captures keys while open.
        if self.peek.is_some() {
            self.peek_key(key);
            return;
        }
        // UX-P5 — pane zoom (Ctrl+Z), from anywhere. (Split resize is `<`/`>`,
        // pane-scoped — Ctrl+←/→ is taken by macOS Mission Control.)
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('z') {
            self.zoom = match self.zoom {
                Some(_) => None,
                None if matches!(self.focus, Focus::FactsTree) => Some(Focus::FactsTree),
                None => Some(Focus::AiChat),
            };
            return;
        }
        // Tab / Shift+Tab cycle focus — except in the prompt, where Tab first
        // tries to complete a Facts slug path (`/goto …`, `… → …`); R2-E.
        match key.code {
            KeyCode::Tab => {
                if matches!(self.focus, Focus::QueryPrompt)
                    && (self.try_command_completion() || self.try_path_completion())
                {
                    return;
                }
                self.focus = self.focus.next();
                return;
            }
            KeyCode::BackTab => {
                self.focus = self.focus.prev();
                return;
            }
            // F10 cycles the RAG mode from any pane (RFC §8).
            KeyCode::F(10) => {
                self.thread.rag_mode = self.thread.rag_mode.next();
                let _ = self.thread.save(&self.layout);
                self.status_message = Some(format!("RAG: {}", self.thread.rag_mode.label()));
                return;
            }
            _ => {}
        }
        // Pane-specific keys.
        match self.focus {
            Focus::FactsTree => {
                if self.facts_tree_key(key) {
                    return;
                }
            }
            Focus::QueryPrompt => {
                self.query_prompt_key(key);
                return;
            }
            Focus::AiChat => {
                if self.chat_key(key) {
                    return;
                }
            }
            Focus::ConfirmationOverlay => {}
        }
        // Globals (only reached for unconsumed keys outside the text prompt).
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.show_hints = !self.show_hints,
            _ => {}
        }
    }

    /// R-P5 — the query prompt: a full editable text field. Enter submits;
    /// Alt+Enter inserts a newline (multi-line queries). ←/→/Home/End and
    /// Backspace/Delete edit; ↑/↓ move the cursor between lines and fall through
    /// to prompt-history recall only at the top / bottom edge (shell-style).
    fn query_prompt_key(&mut self, key: KeyEvent) {
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            // Alt+Enter (or Ctrl+Enter) inserts a newline; plain Enter submits.
            KeyCode::Enter if alt || key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.prompt_history_idx = None;
                self.query.insert_newline();
            }
            KeyCode::Enter => self.submit_query(),
            KeyCode::Up => {
                let (row, _) = self.query.cursor();
                if row == 0 {
                    self.history_back();
                } else {
                    self.query.move_cursor(tui_textarea::CursorMove::Up);
                }
            }
            KeyCode::Down => {
                let (row, _) = self.query.cursor();
                let last = self.query.lines().len().saturating_sub(1);
                if row >= last {
                    self.history_forward();
                } else {
                    self.query.move_cursor(tui_textarea::CursorMove::Down);
                }
            }
            // Horizontal cursor movement — `input_without_shortcuts` does NOT
            // perform arrow moves, so drive the cursor explicitly (like the editor).
            KeyCode::Left => self.query.move_cursor(tui_textarea::CursorMove::Back),
            KeyCode::Right => self.query.move_cursor(tui_textarea::CursorMove::Forward),
            KeyCode::Home => self.query.move_cursor(tui_textarea::CursorMove::Head),
            KeyCode::End => self.query.move_cursor(tui_textarea::CursorMove::End),
            KeyCode::Esc => {
                if self.query_text().trim().is_empty() {
                    self.focus = Focus::FactsTree;
                } else {
                    self.query = TextArea::default();
                    self.prompt_history_idx = None;
                }
            }
            _ => {
                // History navigation resets once the user edits/moves the draft.
                self.prompt_history_idx = None;
                let input: tui_textarea::Input = key.into();
                self.query.input_without_shortcuts(input);
            }
        }
    }

    /// R-P6 / G8 — chat pane keys: scroll, plus `Ctrl+F` search.
    fn chat_key(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        // The search bar captures input while open.
        if let Some(search) = self.chat_search.as_mut() {
            match key.code {
                KeyCode::Esc => self.chat_search = None,
                KeyCode::Char('f') if ctrl => self.chat_search = None,
                KeyCode::Char('n') => search.current = search.current.wrapping_add(1),
                KeyCode::Char('N') => search.current = search.current.wrapping_sub(1),
                KeyCode::Backspace => {
                    search.query.pop();
                    search.current = 0;
                }
                KeyCode::Char(c) => {
                    search.query.push(c);
                    search.current = 0;
                }
                _ => {}
            }
            return true;
        }
        match key.code {
            KeyCode::Char('f') if ctrl => {
                self.chat_search = Some(ChatSearch { query: String::new(), current: 0 });
            }
            KeyCode::Up | KeyCode::Char('k') => self.chat_scroll = self.chat_scroll.saturating_add(1),
            KeyCode::Down | KeyCode::Char('j') => self.chat_scroll = self.chat_scroll.saturating_sub(1),
            KeyCode::Char('g') => self.chat_scroll = u16::MAX, // clamped to top by render
            KeyCode::Char('G') => self.chat_scroll = 0,        // bottom (latest)
            // UX-P5 — copy the last response to the system clipboard.
            KeyCode::Char('y') => self.yank_last_response(),
            // UX-P5 — resize the split (`>` widens the tree, `<` narrows it).
            KeyCode::Char('>') => self.resize_split(true),
            KeyCode::Char('<') => self.resize_split(false),
            KeyCode::Esc => self.focus = Focus::QueryPrompt,
            _ => return false,
        }
        true
    }

    /// The current prompt text (lines joined).
    pub(super) fn query_text(&self) -> String {
        self.query.lines().join("\n")
    }

    fn set_query_text(&mut self, text: &str) {
        let mut ta = TextArea::default();
        ta.insert_str(text);
        self.query = ta;
    }

    /// UX-P1 — Tab-complete a `/command` name when the cursor sits in the command
    /// word (`/` prefix, before the first space). Returns `true` when it handled
    /// Tab. Single match completes + adds a space; several splice the common
    /// prefix and list the options.
    fn try_command_completion(&mut self) -> bool {
        let (row, col) = self.query.cursor();
        if row != 0 {
            return false;
        }
        let text = self.query_text();
        if !text.starts_with('/') {
            return false;
        }
        // The command word spans char 1..first-whitespace; only complete when the
        // cursor is inside it.
        let first_ws = text.find(char::is_whitespace).map(|b| text[..b].chars().count()).unwrap_or_else(|| text.chars().count());
        if col == 0 || col > first_ws {
            return false;
        }
        let typed: String = text.chars().skip(1).take(first_ws - 1).collect::<String>().to_ascii_lowercase();
        let matches: Vec<&str> =
            super::command::SPECS.iter().map(|s| s.name).filter(|n| n.starts_with(&typed)).collect();
        match matches.len() {
            0 => self.status_message = Some(format!("/{typed} — no such command")),
            1 => self.splice_command(matches[0], first_ws),
            _ => {
                let owned: Vec<String> = matches.iter().map(|s| s.to_string()).collect();
                let lcp = longest_common_prefix(&owned);
                if lcp.chars().count() > typed.chars().count() {
                    self.splice_command(&lcp, first_ws);
                }
                self.status_message = Some(format!("{} commands: {}", matches.len(), matches.join(" · ")));
            }
        }
        true
    }

    /// Replace the command word (`chars[1..end]`) with `name`; add a trailing
    /// space when it fully names a command and no args follow yet.
    fn splice_command(&mut self, name: &str, end: usize) {
        let chars: Vec<char> = self.query_text().chars().collect();
        let suffix: String = chars[end.min(chars.len())..].iter().collect();
        let is_full = super::command::SPECS.iter().any(|s| s.name == name);
        let tail = if is_full && suffix.is_empty() { " " } else { suffix.as_str() };
        self.set_query_text(&format!("/{name}{tail}"));
    }

    /// R2-E — Tab-complete a Facts slug path under the cursor (after `/goto ` or
    /// after `→`/`->`). Returns `true` when it handled Tab, so the dispatcher
    /// doesn't fall through to focus-cycling. Single-line prompts only.
    fn try_path_completion(&mut self) -> bool {
        let (row, col) = self.query.cursor();
        if row != 0 {
            return false;
        }
        let text = self.query_text();
        let Some(path_start) = completable_path_start(&text) else { return false };
        if col < path_start {
            return false;
        }
        let typed: String = text.chars().skip(path_start).take(col - path_start).collect();
        let (parent, partial) = match typed.rfind('/') {
            Some(i) => (typed[..i].to_string(), typed[i + 1..].to_string()),
            None => (String::new(), typed.clone()),
        };
        let candidates = self.facts_slug_candidates(&parent, &partial);
        match candidates.len() {
            0 => {
                self.status_message = Some("no matching Facts path".to_string());
            }
            1 => {
                self.apply_completion(path_start, col, &parent, &candidates[0], true);
            }
            _ => {
                let lcp = longest_common_prefix(&candidates);
                if lcp.chars().count() > partial.chars().count() {
                    self.apply_completion(path_start, col, &parent, &lcp, false);
                }
                let shown: Vec<&str> = candidates.iter().take(8).map(String::as_str).collect();
                self.status_message =
                    Some(format!("{} matches: {}", candidates.len(), shown.join("  ")));
            }
        }
        true
    }

    /// Child slugs of the node at `parent` (a slug path; empty = the Facts book
    /// root) whose slug starts with `partial`, sorted.
    fn facts_slug_candidates(&self, parent: &str, partial: &str) -> Vec<String> {
        let parent_id = if parent.is_empty() {
            self.facts_tree.root
        } else {
            let p = parent.trim_start_matches('/');
            self.hierarchy
                .find_by_path(p)
                .or_else(|| {
                    let stripped =
                        p.strip_prefix("facts/").or_else(|| p.strip_prefix("notes/")).unwrap_or(p);
                    self.hierarchy.find_by_path(stripped)
                })
                .map(|n| n.id)
        };
        let Some(pid) = parent_id else { return Vec::new() };
        let mut out: Vec<String> = self
            .hierarchy
            .children_of(Some(pid))
            .iter()
            .filter(|n| n.slug.starts_with(partial))
            .map(|n| n.slug.clone())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// Splice a completed slug into the prompt, replacing the typed path token
    /// (`chars[path_start..end]`) with `parent/leaf` (descend → trailing `/`).
    fn apply_completion(&mut self, path_start: usize, end: usize, parent: &str, leaf: &str, descend: bool) {
        let chars: Vec<char> = self.query_text().chars().collect();
        let prefix: String = chars[..path_start].iter().collect();
        let suffix: String = chars[end.min(chars.len())..].iter().collect();
        let mut replacement = String::new();
        if !parent.is_empty() {
            replacement.push_str(parent);
            replacement.push('/');
        }
        replacement.push_str(leaf);
        if descend {
            replacement.push('/');
        }
        self.set_query_text(&format!("{prefix}{replacement}{suffix}"));
    }

    /// Submit the prompt. Commands (`/…`) route to the dispatcher (R-P9); plain
    /// text becomes a chat query. R-P7 replaces the placeholder with a real
    /// streamed response.
    fn submit_query(&mut self) {
        let text = self.query_text().trim().to_string();
        if text.is_empty() {
            return;
        }
        self.query = TextArea::default();
        self.prompt_history_idx = None;
        self.chat_scroll = 0;

        if let Some(cmd) = super::command::parse(&text) {
            self.dispatch_command(cmd);
            return;
        }
        self.start_query(text);
    }

    /// R-P9 — route a parsed `/command`. The simple commands act here; the
    /// heavier ones land in their phases (R-P10..R-P15).
    fn dispatch_command(&mut self, cmd: super::command::Command) {
        use super::command::Command;
        match cmd {
            Command::Clear => {
                self.chat_history.clear();
                self.chat_scroll = 0;
                self.status_message = Some("chat cleared".to_string());
            }
            Command::Rag(arg) => self.set_rag_mode(arg.as_deref()),
            Command::Save(name) => self.save_thread_as(name.as_deref()),
            Command::Unknown(name) => {
                self.status_message = Some(format!("unknown command: /{name}"));
            }
            Command::Fact { prompt, path } => {
                self.start_extraction(TargetBook::Facts, prompt, path, "/fact")
            }
            Command::Note { prompt, path } => {
                self.start_extraction(TargetBook::Notes, prompt, path, "/note")
            }
            Command::Goto(path) => self.goto_path(&path),
            Command::Diff => self.run_diff(),
            Command::Verify => self.run_verify(),
            Command::FactCheck => self.start_factcheck(),
            Command::Undisputed => self.start_undisputed(),
            Command::Synthesize(topic) => self.run_grounded(&topic, GroundedKind::Synthesize),
            Command::Outline(topic) => self.run_grounded(&topic, GroundedKind::Outline),
            Command::Gaps(topic) => self.run_grounded(&topic, GroundedKind::Gaps),
            Command::Bibliography => self.run_bibliography(),
            Command::Upgrade(path) => self.start_upgrade(path.as_deref()),
            Command::Stale(arg) => self.run_stale(arg.as_deref()),
            Command::Sources => self.run_sources(),
            Command::Promote { note, path } => self.start_promote(note, path),
            Command::Import(path) => match path {
                Some(p) => self.run_import(&p),
                None => self.run_imports_list(),
            },
            Command::Forget(name) => self.run_forget(&name),
            Command::Web { ingest, query } => self.start_web(ingest, query),
            Command::Calc(expr) => self.run_calc(&expr),
            Command::World(arg) => self.run_world(&arg),
            Command::Wikidata(query) => self.start_wikidata(query),
            Command::Gutenberg(query) => self.start_gutenberg(query),
            Command::OpenAlex(query) => self.start_scholarly("openalex", query),
            Command::Arxiv(query) => self.start_scholarly("arxiv", query),
            Command::Triangulate(claim) => self.start_triangulate(claim),
            Command::WhatsWrong(path) => self.run_whatswrong(path.as_deref()),
            Command::Chain(steps) => self.start_chain(steps),
        }
    }

    /// RE-P5 — `/whatswrong [facts/path]`: AI explanation of a fact flagged by
    /// `/factcheck`. Targets the given path, else the selected/cursor fact; seeds
    /// the prompt with the recorded verdict, and streams the answer into chat.
    fn run_whatswrong(&mut self, path: Option<&str>) {
        if self.stream_rx.is_some() {
            self.status_message = Some("a response is already streaming".to_string());
            return;
        }
        let target = match path {
            Some(p) if !p.trim().is_empty() => self.resolve_insertion_target(TargetBook::Facts, Some(p)),
            _ => self.facts_tree.selected(),
        };
        let Some(id) = target else {
            self.status_message =
                Some("usage: /whatswrong [facts/path] — or select a fact in the tree".to_string());
            return;
        };
        let text = match self.store.get_content(id) {
            Ok(Some(b)) => String::from_utf8_lossy(&b).trim().to_string(),
            _ => String::new(),
        };
        if text.is_empty() {
            self.status_message =
                Some("/whatswrong: select a fact paragraph (this node has no text)".to_string());
            return;
        }
        let loc = self.hierarchy.get(id).map(|n| self.hierarchy.slug_path(n)).unwrap_or_default();
        let verdict_line = match self.fact_verdicts.get(id) {
            Some(v) if !v.reason.is_empty() => {
                format!("Prior fact-check verdict: {} — {}", v.level.label(), v.reason)
            }
            Some(v) => format!("Prior fact-check verdict: {}", v.level.label()),
            None => "No prior fact-check verdict is on record for this fact.".to_string(),
        };

        let (lang, _note) = crate::prose::resolve_prose_language(None, &self.cfg.language);
        let language = super::extract::language_name(&lang);
        let system = format!(
            "You are a meticulous fact-checking assistant reviewing one statement from a writer's \
             reference database. Explain SPECIFICALLY what is inaccurate or questionable about it and \
             state the correct information, concisely and concretely (names, dates, figures). If the \
             statement is in fact accurate, say so plainly and briefly. Do not fabricate citations. \
             Write in {language}."
        );
        let user = format!(
            "Statement:\n{text}\n\n{verdict_line}\n\nWhat, if anything, is wrong with this statement?"
        );

        let ai = match crate::ai::AiClient::from_config(&self.cfg.llm) {
            Ok(a) => a,
            Err(e) => {
                self.status_message = Some(format!("no LLM provider: {e}"));
                return;
            }
        };
        let (model, _env) = match ai.resolve_provider(&self.cfg.llm, None) {
            Ok(m) => m,
            Err(e) => {
                self.status_message = Some(format!("provider error: {e}"));
                return;
            }
        };
        let mut turn = ChatTurn::new(format!("/whatswrong {loc}"));
        turn.streaming = true;
        turn.model = model.to_string();
        self.chat_history.push(turn);
        self.streaming_turn = Some(self.chat_history.len() - 1);
        self.chat_scroll = 0;
        let rx = spawn_chat_stream(
            ai.client.clone(),
            model.to_string(),
            Some(system),
            Vec::new(),
            user,
            llm::CATEGORY,
        );
        self.stream_rx = Some(rx);
        self.status_message = Some("analysing the flagged fact…".to_string());
    }

    /// RESRCH-5 (R5-A/B/C) — retrieve the facts related to a topic and stream a
    /// grounded output over them (synthesis / outline / gaps). Read-only,
    /// language-aware; cites each fact by its breadcrumb.
    fn run_grounded(&mut self, topic: &str, kind: GroundedKind) {
        if self.stream_rx.is_some() {
            self.status_message = Some("a response is already streaming".to_string());
            return;
        }
        let topic = topic.trim();
        if topic.is_empty() {
            self.status_message = Some(format!("usage: /{} <topic>", kind.command()));
            return;
        }
        let Some(book_id) = self.facts_tree.root else {
            self.status_message = Some("no Facts book".to_string());
            return;
        };
        let passages = match crate::book_rag::retrieval::retrieve(
            &self.store,
            &self.hierarchy,
            &self.cfg.book_rag,
            book_id,
            topic,
        ) {
            Ok(p) => p,
            Err(e) => {
                self.status_message = Some(format!("{}: {e}", kind.command()));
                return;
            }
        };
        if passages.is_empty() {
            self.status_message = Some(format!("no facts related to `{topic}` — add some first"));
            return;
        }
        // Build the cited context: `[breadcrumb · tier] body` per fact.
        let mut ctx = String::new();
        let n = passages.len().min(24);
        for p in passages.iter().take(n) {
            let tier = self
                .fact_provenance
                .for_node(&p.id.to_string())
                .map(|r| r.origin.as_str())
                .unwrap_or("—");
            let body: String = p.body.chars().take(600).collect();
            ctx.push_str(&format!("[{} · {tier}]\n{}\n\n", p.breadcrumb, body.trim()));
        }

        let (lang, _note) = crate::prose::resolve_prose_language(None, &self.cfg.language);
        let language = super::extract::language_name(&lang);
        let system = kind.system_prompt(language);
        let user = format!("Topic: {topic}\n\nFacts (breadcrumb · provenance):\n{ctx}");

        let ai = match crate::ai::AiClient::from_config(&self.cfg.llm) {
            Ok(a) => a,
            Err(e) => {
                self.status_message = Some(format!("no LLM provider: {e}"));
                return;
            }
        };
        let (model, _env) = match ai.resolve_provider(&self.cfg.llm, None) {
            Ok(m) => m,
            Err(e) => {
                self.status_message = Some(format!("provider error: {e}"));
                return;
            }
        };
        let mut turn = ChatTurn::new(format!("/{} {topic}", kind.command()));
        turn.streaming = true;
        turn.model = model.to_string();
        self.chat_history.push(turn);
        self.streaming_turn = Some(self.chat_history.len() - 1);
        self.chat_scroll = 0;
        let rx = spawn_chat_stream(ai.client.clone(), model.to_string(), Some(system), Vec::new(), user, llm::CATEGORY);
        self.stream_rx = Some(rx);
        self.status_message = Some(format!("{} from {n} fact(s)…", kind.gerund()));
    }

    /// RESRCH-5 (R5-D) — `/bibliography`: emit the Sources book's Research chapter
    /// (the auto-cited entries) as BibTeX into the chat.
    fn run_bibliography(&mut self) {
        let entries = collect_research_bibentries(&self.store, &self.hierarchy);
        if entries.is_empty() {
            self.status_message =
                Some("no citations yet — /openalex /arxiv /import <.bib> file them first".to_string());
            return;
        }
        let (bibtex, n) = crate::sources::compile_bibtex(&entries);
        let body = format!("[/bibliography — {n} entr{}]\n\n{bibtex}", if n == 1 { "y" } else { "ies" });
        self.chat_history.push(ChatTurn::with_response("/bibliography".to_string(), body));
        self.chat_scroll = 0;
        self.status_message = Some(format!("{n} citation(s) · y to copy the BibTeX"));
    }

    /// RESRCH-5 (R5-F) — `/stale [days]`: list `model` / `web` facts older than
    /// `days` (default 90) from their provenance `created_at`, so you can
    /// re-verify aging claims. Read-only.
    fn run_stale(&mut self, arg: Option<&str>) {
        let days: i64 = arg.and_then(|a| a.trim().parse().ok()).unwrap_or(90);
        let now = chrono::Utc::now();
        let mut rows: Vec<(i64, String, String)> = Vec::new();
        for (id, rec) in &self.fact_provenance.facts {
            if !matches!(rec.origin.as_str(), "model" | "web") {
                continue;
            }
            let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&rec.created_at) else { continue };
            let age = (now - ts.with_timezone(&chrono::Utc)).num_days();
            if age <= days {
                continue;
            }
            let loc = uuid::Uuid::parse_str(id)
                .ok()
                .and_then(|u| self.hierarchy.get(u))
                .map(|n| self.hierarchy.slug_path(n))
                .unwrap_or_else(|| id.clone());
            rows.push((age, loc, rec.origin.clone()));
        }
        rows.sort_by(|a, b| b.0.cmp(&a.0));
        let body = if rows.is_empty() {
            format!("No model/web facts older than {days} days — the corpus is fresh.")
        } else {
            let mut s = format!("[/stale — {} fact(s) older than {days} days · re-verify or /upgrade]\n", rows.len());
            for (age, loc, origin) in &rows {
                s.push_str(&format!("· {age}d  [{origin}]  {loc}\n"));
            }
            s
        };
        self.chat_history.push(ChatTurn::with_response(format!("/stale {days}"), body));
        self.chat_scroll = 0;
    }

    /// RESRCH-5 (R5-E) — `/upgrade [facts/path]`: try to re-ground a `model` fact
    /// on a structured source (Wikidata / scholarly, via the triangulation engine)
    /// and, when corroborated, **raise its provenance tier**. The fact text is
    /// never changed — only the provenance. Bare targets the selected fact.
    fn start_upgrade(&mut self, path: Option<&str>) {
        if self.upgrade.is_some() || self.stream_rx.is_some() {
            self.status_message = Some("a response is already in flight".to_string());
            return;
        }
        let target = match path {
            Some(p) if !p.trim().is_empty() => self.resolve_insertion_target(TargetBook::Facts, Some(p)),
            _ => self.facts_tree.selected(),
        };
        let Some(id) = target else {
            self.status_message =
                Some("usage: /upgrade [facts/path] — or select a model fact in the tree".to_string());
            return;
        };
        let origin = self
            .fact_provenance
            .for_node(&id.to_string())
            .map(|r| r.origin.clone())
            .unwrap_or_else(|| "model".to_string());
        if matches!(origin.as_str(), "computed" | "simulation" | "wikidata" | "openalex" | "arxiv") {
            self.status_message = Some(format!("already structured ({origin}) — nothing to upgrade"));
            return;
        }
        let claim = match self.store.get_content(id) {
            Ok(Some(b)) => String::from_utf8_lossy(&b).trim().to_string(),
            _ => String::new(),
        };
        if claim.is_empty() {
            self.status_message = Some("/upgrade: select a fact paragraph with text".to_string());
            return;
        }
        let wd_cfg = self.cfg.research.wikidata.clone();
        let sc_cfg = self.cfg.research.scholarly.clone();
        let wd_on = super::wikidata::available(&wd_cfg);
        let sc_on = super::scholarly::available(&sc_cfg);
        if !wd_on && !sc_on {
            self.status_message = Some("/upgrade: no structured sources enabled".to_string());
            return;
        }
        let (lang, _note) = crate::prose::resolve_prose_language(None, &self.cfg.language);
        let code = match lang.as_code() {
            "other" => "en".to_string(),
            c => c.to_string(),
        };
        let q = claim.clone();
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut evidence: Vec<(String, String)> = Vec::new();
            if wd_on {
                if let Ok(e) = super::wikidata::fetch(wd_cfg, q.clone(), code).await {
                    evidence.push(("Wikidata".to_string(), super::wikidata::render(&e)));
                }
            }
            if sc_on {
                if let Ok(p) = super::scholarly::openalex(sc_cfg.clone(), q.clone()).await {
                    evidence.push(("OpenAlex".to_string(), super::scholarly::render(&p)));
                }
                if let Ok(p) = super::scholarly::arxiv(q.clone()).await {
                    evidence.push(("arXiv".to_string(), super::scholarly::render(&p)));
                }
            }
            let _ = tx.send(evidence);
        });
        let loc = self.hierarchy.get(id).map(|n| self.hierarchy.slug_path(n)).unwrap_or_default();
        let mut preview = ChatTurn::new(format!("/upgrade {loc}"));
        preview.streaming = true;
        self.chat_history.push(preview);
        self.chat_scroll = 0;
        let preview_idx = Some(self.chat_history.len() - 1);
        self.upgrade = Some(UpgradeState { node_id: id, claim, query: loc, phase: UpgradePhase::Gather(rx), preview_idx });
        self.status_message = Some("upgrade: gathering structured evidence…".to_string());
    }

    /// Drive the `/upgrade` pipeline: gather → judge → (on corroboration) raise
    /// the fact's provenance tier.
    fn poll_upgrade(&mut self) {
        // Phase 1 — evidence gathered?
        let gathered =
            if let Some(UpgradeState { phase: UpgradePhase::Gather(rx), .. }) = self.upgrade.as_mut() {
                match rx.try_recv() {
                    Ok(e) => Some(Ok(e)),
                    Err(mpsc::error::TryRecvError::Empty) => return,
                    Err(_) => Some(Err(())),
                }
            } else {
                None
            };
        if let Some(res) = gathered {
            match res {
                Err(()) => self.upgrade = None,
                Ok(ev) if ev.is_empty() => {
                    self.finish_upgrade(None, "no structured source returned evidence about this fact")
                }
                Ok(ev) => {
                    let claim = self.upgrade.as_ref().unwrap().claim.clone();
                    self.start_upgrade_judge(&claim, &ev);
                }
            }
            return;
        }
        // Phase 2 — the judge stream.
        let preview_idx = self.upgrade.as_ref().and_then(|u| u.preview_idx);
        let mut done = false;
        let mut new_tokens = String::new();
        if let Some(UpgradeState { phase: UpgradePhase::Judge { rx, buf }, .. }) = self.upgrade.as_mut() {
            loop {
                match rx.try_recv() {
                    Ok(StreamMsg::Token(t)) => {
                        new_tokens.push_str(&t);
                        buf.push_str(&t);
                    }
                    Ok(StreamMsg::Done(_)) | Err(mpsc::error::TryRecvError::Disconnected) => {
                        done = true;
                        break;
                    }
                    Ok(StreamMsg::Error(_)) => {
                        done = true;
                        break;
                    }
                    Err(mpsc::error::TryRecvError::Empty) => break,
                }
            }
        } else {
            return;
        }
        if !new_tokens.is_empty() {
            if let Some(i) = preview_idx {
                if let Some(t) = self.chat_history.get_mut(i) {
                    t.response.push_str(&new_tokens);
                }
            }
        }
        if !done {
            return;
        }
        let buf = match self.upgrade.as_ref().map(|u| &u.phase) {
            Some(UpgradePhase::Judge { buf, .. }) => buf.clone(),
            _ => return,
        };
        match pick_corroborator(&buf) {
            Some(origin) => self.finish_upgrade(Some(origin), ""),
            None => self.finish_upgrade(None, "no structured source corroborated the claim"),
        }
    }

    /// Spawn the `/upgrade` corroboration-judge over the gathered evidence.
    fn start_upgrade_judge(&mut self, claim: &str, evidence: &[(String, String)]) {
        let ai = match crate::ai::AiClient::from_config(&self.cfg.llm) {
            Ok(a) => a,
            Err(e) => {
                self.status_message = Some(format!("no LLM provider: {e}"));
                self.upgrade = None;
                return;
            }
        };
        let (model, _env) = match ai.resolve_provider(&self.cfg.llm, None) {
            Ok(m) => m,
            Err(e) => {
                self.status_message = Some(format!("provider error: {e}"));
                self.upgrade = None;
                return;
            }
        };
        let (lang, _note) = crate::prose::resolve_prose_language(None, &self.cfg.language);
        let language = super::extract::language_name(&lang);
        let mut ev = String::new();
        for (label, text) in evidence {
            let body: String = text.chars().take(1200).collect();
            ev.push_str(&format!("[{label}]\n{}\n\n", body.trim()));
        }
        let system = format!(
            "You are checking whether INDEPENDENT structured sources CORROBORATE a claim from a writer's \
             database. For EACH source below, judge from the source text alone: does it SUPPORT, \
             CONTRADICT, or is it SILENT on the claim? Output one line per source:\n\
             <source>: SUPPORTS | CONTRADICTS | SILENT — <short reason>\n\
             Do not use outside knowledge. Write in {language}."
        );
        let user = format!("Claim:\n{claim}\n\nSources:\n{ev}");
        let rx = spawn_chat_stream(ai.client.clone(), model.to_string(), Some(system), Vec::new(), user, llm::CATEGORY);
        if let Some(up) = self.upgrade.as_mut() {
            up.phase = UpgradePhase::Judge { rx, buf: String::new() };
        }
        self.status_message = Some(format!("upgrade: judging {} source(s)…", evidence.len()));
    }

    /// Finalise `/upgrade`: raise the fact's provenance tier on corroboration (the
    /// text is never touched), or report it stays `model`.
    fn finish_upgrade(&mut self, corroborator: Option<String>, note: &str) {
        let Some(up) = self.upgrade.take() else { return };
        let (report, status) = match corroborator {
            Some(origin) => {
                let now = chrono::Utc::now().to_rfc3339();
                super::provenance::Provenance::record(
                    &self.layout,
                    &up.node_id.to_string(),
                    super::provenance::SourceRecord::new(&origin, "corroborated (was model)", &up.query, &self.thread.name, now),
                );
                self.fact_provenance = super::provenance::Provenance::load(&self.layout);
                (
                    format!("[/upgrade] ✓ corroborated by {origin} — provenance raised to `{origin}` (the fact text is unchanged)."),
                    format!("upgraded → {origin}"),
                )
            }
            None => (format!("[/upgrade] — {note}; stays `model`. Verify it by hand, or /triangulate it."), "not upgraded".to_string()),
        };
        match up.preview_idx.and_then(|i| self.chat_history.get_mut(i)) {
            Some(turn) => {
                turn.response = report;
                turn.streaming = false;
            }
            None => self.chat_history.push(ChatTurn::with_response("/upgrade".to_string(), report)),
        }
        self.chat_scroll = 0;
        self.status_message = Some(status);
    }

    /// `/sources` — list each Facts node with its recorded provenance (T-P3).
    fn run_sources(&mut self) {
        let prov = super::provenance::Provenance::load(&self.layout);
        let Some(book_id) = self.facts_tree.root else {
            self.status_message = Some("no Facts book".to_string());
            return;
        };
        let mut out = String::new();
        let mut n = 0usize;
        for id in self.hierarchy.collect_subtree(book_id) {
            let Some(node) = self.hierarchy.get(id) else { continue };
            if node.kind != crate::store::NodeKind::Paragraph {
                continue;
            }
            let loc = self.hierarchy.slug_path(node);
            match prov.for_node(&id.to_string()) {
                Some(rec) => out.push_str(&format!("• {loc}\n    {}\n", rec.summary())),
                None => out.push_str(&format!("• {loc}\n    (no recorded source)\n")),
            }
            n += 1;
        }
        if n == 0 {
            out = "No facts yet.".to_string();
        }
        self.chat_history
            .push(ChatTurn::with_response(format!("[/sources — {n} fact(s)]"), out));
        self.chat_scroll = 0;
    }

    /// `/import <path>` (R2-B) — read a Markdown / text / PDF file, chunk it, and
    /// embed the chunks into the shared vector store as research sources, so they
    /// ground answers and `/fact` can cite them.
    fn run_import(&mut self, path: &str) {
        let p = std::path::Path::new(path);
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        if matches!(ext.as_str(), "bib" | "bibtex" | "json") {
            // R3-D — a BibTeX / CSL-JSON file imports its entries into the Sources book.
            let result = if ext == "json" {
                import_csl_json(&self.cfg, &self.store, p)
            } else {
                import_bibtex(&self.cfg, &self.store, p)
            };
            match result {
                Ok((added, total)) => {
                    self.reload_hierarchy();
                    self.status_message =
                        Some(format!("✓ imported {added}/{total} citation(s) → Sources"));
                }
                Err(e) => self.status_message = Some(format!("citation import: {e}")),
            }
            return;
        }
        if p.is_dir() {
            // Folder / vault import (R3-D, brought forward): recurse over the
            // supported files.
            let mut files = 0usize;
            let mut chunks = 0usize;
            let mut errors = 0usize;
            for entry in walkdir::WalkDir::new(p).into_iter().filter_map(|e| e.ok()) {
                let fp = entry.path();
                if !fp.is_file() {
                    continue;
                }
                let ext = fp.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
                if !matches!(ext.as_str(), "md" | "markdown" | "txt" | "text" | "pdf") {
                    continue;
                }
                match self.import_one_file(fp) {
                    Ok(n) => {
                        files += 1;
                        chunks += n;
                    }
                    Err(_) => errors += 1,
                }
            }
            self.status_message = Some(format!(
                "✓ imported {files} file(s), {chunks} chunk(s) from `{}`{}",
                p.display(),
                if errors > 0 { format!(" ({errors} skipped)") } else { String::new() }
            ));
            return;
        }
        match self.import_one_file(p) {
            Ok(n) => {
                let name = super::imports::source_name(p);
                self.status_message =
                    Some(format!("✓ imported `{name}` — {n} chunk(s) as a research source"));
            }
            Err(e) => self.status_message = Some(format!("import: {e}")),
        }
    }

    /// Read + chunk + embed one file as a research source; returns the chunk
    /// count. Re-import drops the same-named source's previous chunks first.
    fn import_one_file(&mut self, p: &std::path::Path) -> std::result::Result<usize, String> {
        use super::imports;
        let text = imports::read_source(p).map_err(|e| e.to_string())?;
        let chunk_chars = self.cfg.research.import_chunk_chars.max(200);
        let chunks = imports::chunk_text(&text, chunk_chars);
        if chunks.is_empty() {
            return Err("no text extracted".to_string());
        }
        let name = imports::source_name(p);
        let abs = std::fs::canonicalize(p)
            .map(|c| c.to_string_lossy().into_owned())
            .unwrap_or_else(|_| p.to_string_lossy().into_owned());
        let now = chrono::Utc::now().to_rfc3339();
        let mut doc_ids = Vec::new();
        for (i, chunk) in chunks.iter().enumerate() {
            let meta = serde_json::json!({
                "kind": imports::SOURCE_KIND, "source": abs, "name": name,
                "thread": self.thread.name, "chunk": i, "imported_at": now,
            });
            match self.store.raw().add_document(meta, chunk.as_bytes()) {
                Ok(id) => doc_ids.push(id.to_string()),
                Err(e) => return Err(format!("embed failed: {e}")),
            }
        }
        let mut imports_store = imports::Imports::load(&self.layout);
        if let Some(old) = imports_store.sources.get(&name) {
            for id in &old.doc_ids {
                if let Ok(uuid) = uuid::Uuid::parse_str(id) {
                    let _ = self.store.raw().delete_document(uuid);
                }
            }
        }
        let chunks_n = doc_ids.len();
        imports_store.sources.insert(
            name.clone(),
            imports::ImportedSource {
                name,
                path: abs,
                doc_ids,
                thread: self.thread.name.clone(),
                imported_at: now,
                chunks: chunks_n,
            },
        );
        let _ = imports_store.save(&self.layout);
        Ok(chunks_n)
    }

    /// `/import` (bare) — list the imported research sources.
    fn run_imports_list(&mut self) {
        let store = super::imports::Imports::load(&self.layout);
        let mut out = String::new();
        if store.sources.is_empty() {
            out.push_str("No documents imported. Use /import <path> (md / txt / pdf).");
        } else {
            for s in store.sources.values() {
                out.push_str(&format!("• {} — {} chunk(s)\n    {}\n", s.name, s.chunks, s.path));
            }
            out.push_str("─────\n/forget <name> removes one.");
        }
        self.chat_history.push(ChatTurn::with_response(
            format!("[/import — {} source(s)]", store.sources.len()),
            out,
        ));
        self.chat_scroll = 0;
    }

    /// `/forget <name>` — drop an imported source's chunks from the index.
    fn run_forget(&mut self, name: &str) {
        let key = super::imports::source_name(std::path::Path::new(name));
        let mut store = super::imports::Imports::load(&self.layout);
        // Accept either the slug or the raw name as typed.
        let found = store.sources.remove(name).or_else(|| store.sources.remove(&key));
        match found {
            Some(src) => {
                for id in &src.doc_ids {
                    if let Ok(uuid) = uuid::Uuid::parse_str(id) {
                        let _ = self.store.raw().delete_document(uuid);
                    }
                }
                let _ = store.save(&self.layout);
                self.status_message = Some(format!("forgot `{}` ({} chunk(s))", src.name, src.doc_ids.len()));
            }
            None => self.status_message = Some(format!("no imported source named `{name}`")),
        }
    }

    /// `/calc <expr>` (R3-C) — evaluate a deterministic Bund expression
    /// (constants + unit conversions in the `calc.*` words). The result is its
    /// own proof; a `/fact` from it records `origin=computed` (no gate).
    fn run_calc(&mut self, expr: &str) {
        let expr = expr.trim();
        if expr.is_empty() {
            self.status_message =
                Some("usage: /calc <expr>  e.g. /calc 100 mi2km   ·   /calc 4.2 ly2km".to_string());
            return;
        }
        // R4-D — clear any stale World-read trail before evaluating.
        let _ = crate::scripting::take_world_reads();
        match crate::scripting::eval(expr) {
            Ok(out) => {
                let world_reads = crate::scripting::take_world_reads();
                let mut body = String::new();
                if !out.stdout.trim().is_empty() {
                    body.push_str(out.stdout.trim());
                    body.push('\n');
                }
                // R4-D — echo each World fact this calc read, transparently.
                for (path, rendered) in &world_reads {
                    body.push_str(&format!("world: {path} = {rendered}\n"));
                }
                match &out.top {
                    Some(v) => body.push_str(&format!("= {}", crate::scripting::format_value(v))),
                    None if body.is_empty() => body.push_str("(no result)"),
                    None => {}
                }
                let mut turn = ChatTurn::with_response(format!("/calc {expr}"), body);
                turn.computed = true;
                // R4-D — a World-grounded calc cites its source paths in provenance.
                if !world_reads.is_empty() {
                    let paths: Vec<&str> = world_reads.iter().map(|(p, _)| p.as_str()).collect();
                    turn.world_detail = format!("world:{}", paths.join(","));
                }
                self.chat_history.push(turn);
                self.chat_scroll = 0;
                let note = if world_reads.is_empty() {
                    "computed (deterministic) — /fact to record it"
                } else {
                    "computed from World facts — /fact to record it"
                };
                self.status_message = Some(note.to_string());
            }
            Err(e) => self.status_message = Some(format!("calc: {e}")),
        }
    }

    /// `/world [layer]` (R3-C) — surface the project's own World-simulation facts
    /// (WORLD-4). Bare lists the layers; `/world <layer>` shows that layer's facts
    /// (read from the materialized book, or recomputed from `world.hjson`). A
    /// `/fact` from it records `origin=simulation` and skips the gate.
    fn run_world(&mut self, arg: &str) {
        const LAYERS: [&str; 5] = ["Astronomy", "Geology", "Climate", "Hydrology", "Demographics"];
        let arg = arg.trim();
        if arg.is_empty() {
            let mut body = String::from("World layers — use /world <layer>:\n");
            for l in LAYERS {
                let has = crate::world::calc_read::chapter(&self.store, l).is_some();
                body.push_str(&format!("• {l}{}\n", if has { "" } else { "  (no data)" }));
            }
            body.push_str("\nA /fact from a /world result records origin=simulation (deterministic).");
            self.chat_history.push(ChatTurn::with_response("/world".to_string(), body));
            self.chat_scroll = 0;
            return;
        }
        match crate::world::calc_read::chapter(&self.store, arg) {
            Some(json) => {
                let pretty = serde_json::to_string_pretty(&json).unwrap_or_default();
                let clipped: String = pretty.chars().take(2400).collect();
                let body = format!("World / {arg}\n{clipped}");
                let mut turn = ChatTurn::with_response(format!("/world {arg}"), body);
                turn.simulation = true;
                turn.sources = vec![format!("simulation:{arg}")];
                self.chat_history.push(turn);
                self.chat_scroll = 0;
                self.status_message =
                    Some(format!("{arg} — simulation facts · /fact to record (gate-skipped)"));
            }
            None => {
                self.status_message = Some(format!(
                    "no World data for `{arg}` — try /world for the layers, or `realworld compile --materialize`"
                ));
            }
        }
    }

    /// `/web [--ingest|--chat] <query>` (R2-C) — spawn a web search; the result
    /// either grounds an LLM chat answer or is embedded as research sources.
    fn start_web(&mut self, ingest: Option<bool>, query: String) {
        if self.web.is_some() {
            self.status_message = Some("a web search is already running".to_string());
            return;
        }
        let query = query.trim().to_string();
        if query.is_empty() {
            self.status_message = Some("usage: /web [--ingest|--chat] <query>".to_string());
            return;
        }
        if !super::web::available(&self.cfg.research.web) {
            self.status_message = Some(
                "web search not configured — set research.web (provider + key/endpoint)".to_string(),
            );
            return;
        }
        let ingest = ingest.unwrap_or_else(|| self.cfg.research.web.pipeline == "ingest");
        let web_cfg = self.cfg.research.web.clone();
        let q = query.clone();
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let r = super::web::search(web_cfg, q).await.map_err(|e| e.to_string());
            let _ = tx.send(r);
        });
        self.web = Some(WebState { rx, ingest, query });
        self.status_message = Some("Searching the web…".to_string());
    }

    /// `/wikidata <query>` (R3-A) — spawn a Wikidata entity fetch. The result is
    /// structured triples (no LLM); a `/fact` from it records `origin=wikidata`
    /// (+ Q-ID) and skips the fact-check gate (top of the trust ladder).
    fn start_wikidata(&mut self, query: String) {
        if self.wikidata_state.is_some() {
            self.status_message = Some("a Wikidata query is already running".to_string());
            return;
        }
        let query = query.trim().to_string();
        if query.is_empty() {
            self.status_message = Some("usage: /wikidata <query>  e.g. /wikidata Roman aqueduct".to_string());
            return;
        }
        if !super::wikidata::available(&self.cfg.research.wikidata) {
            self.status_message = Some("wikidata disabled (research.wikidata.enabled)".to_string());
            return;
        }
        let (lang, _note) = crate::prose::resolve_prose_language(None, &self.cfg.language);
        // Wikidata wants an ISO code; unsupported → English labels.
        let code = match lang.as_code() {
            "other" => "en".to_string(),
            c => c.to_string(),
        };
        let cfg = self.cfg.research.wikidata.clone();
        let q = query.clone();
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let r = super::wikidata::fetch(cfg, q, code).await.map_err(|e| e.to_string());
            let _ = tx.send(r);
        });
        self.wikidata_state = Some(WikidataState { rx, query });
        self.status_message = Some("Querying Wikidata…".to_string());
    }

    /// RESRCH-GUTENBERG — `/gutenberg <query>`: search the Gutendex catalogue and
    /// fetch the top public-domain book's text (async). Ingested in
    /// `poll_gutenberg` via the `/import` pipeline.
    fn start_gutenberg(&mut self, query: String) {
        if self.gutenberg_state.is_some() {
            self.status_message = Some("a Gutenberg fetch is already running".to_string());
            return;
        }
        let query = query.trim().to_string();
        if query.is_empty() {
            self.status_message = Some("usage: /gutenberg <query>  e.g. /gutenberg pride and prejudice".to_string());
            return;
        }
        if !super::gutenberg::available(&self.cfg.research.gutenberg) {
            self.status_message = Some("gutenberg disabled (research.gutenberg.enabled)".to_string());
            return;
        }
        let (lang, _note) = crate::prose::resolve_prose_language(None, &self.cfg.language);
        let code = match lang.as_code() {
            "other" => String::new(),
            c => c.to_string(),
        };
        let cfg = self.cfg.research.gutenberg.clone();
        let q = query.clone();
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let r = super::gutenberg::fetch(cfg, q, code).await.map_err(|e| e.to_string());
            let _ = tx.send(r);
        });
        self.gutenberg_state = Some(GutenbergState { rx, query });
        self.status_message = Some("Searching Project Gutenberg…".to_string());
    }

    /// Drain the in-flight Gutenberg fetch; on success, ingest the book's text as
    /// a research source (chunked, `origin=gutenberg`).
    fn poll_gutenberg(&mut self) {
        let Some(gb) = self.gutenberg_state.as_mut() else { return };
        let result = match gb.rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::error::TryRecvError::Empty) => return,
            Err(mpsc::error::TryRecvError::Disconnected) => {
                self.gutenberg_state = None;
                return;
            }
        };
        let GutenbergState { query, .. } = self.gutenberg_state.take().unwrap();
        let (book, text) = match result {
            Ok(bt) => bt,
            Err(e) => {
                self.status_message = Some(format!("gutenberg: {e}"));
                return;
            }
        };
        self.ingest_gutenberg(&query, &book, &text);
    }

    /// Ingest a Gutenberg book's text as a `research_source` (mirrors the `/web
    /// --ingest` / `/import` embed path); `origin=gutenberg`.
    fn ingest_gutenberg(&mut self, query: &str, book: &super::gutenberg::GutenbergBook, text: &str) {
        use super::imports;
        if text.trim().is_empty() {
            self.status_message = Some("gutenberg: no text extracted".to_string());
            return;
        }
        let chunk_chars = self.cfg.research.import_chunk_chars.max(200);
        let now = chrono::Utc::now().to_rfc3339();
        let name = format!("{} (PG#{})", book.title, book.id);
        let source_url = format!("https://www.gutenberg.org/ebooks/{}", book.id);
        let chunks = imports::chunk_text(text, chunk_chars);
        let mut doc_ids = Vec::new();
        for (i, chunk) in chunks.iter().enumerate() {
            let meta = serde_json::json!({
                "kind": imports::SOURCE_KIND, "source": source_url, "name": name,
                "thread": self.thread.name, "chunk": i, "imported_at": now,
                "origin": "gutenberg", "title": book.title,
            });
            if let Ok(id) = self.store.raw().add_document(meta, chunk.as_bytes()) {
                doc_ids.push(id.to_string());
            }
        }
        let mut imports_store = imports::Imports::load(&self.layout);
        if let Some(old) = imports_store.sources.get(&name) {
            for id in &old.doc_ids {
                if let Ok(u) = uuid::Uuid::parse_str(id) {
                    let _ = self.store.raw().delete_document(u);
                }
            }
        }
        let chunks_n = doc_ids.len();
        imports_store.sources.insert(
            name.clone(),
            imports::ImportedSource {
                name: name.clone(),
                path: source_url.clone(),
                doc_ids,
                thread: self.thread.name.clone(),
                imported_at: now,
                chunks: chunks_n,
            },
        );
        let _ = imports_store.save(&self.layout);
        let who = if book.authors.is_empty() { String::new() } else { format!(" · {}", book.authors.join(", ")) };
        let body = format!(
            "Ingested **{}**{who} as a research source ({chunks_n} chunk(s)).\n{source_url}\n\n\
             Ask about it — the relevant passages are now retrieved and cited `[source: {}]`.",
            book.title, name
        );
        self.chat_history.push(ChatTurn::with_response(format!("/gutenberg {query}"), body));
        self.chat_scroll = 0;
        self.status_message = Some(format!("✓ Project Gutenberg: {chunks_n} chunk(s) from `{}`", book.title));
    }

    /// Drain the in-flight Wikidata fetch; on success, show the structured entity.
    fn poll_wikidata(&mut self) {
        let Some(wd) = self.wikidata_state.as_mut() else { return };
        let result = match wd.rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::error::TryRecvError::Empty) => return,
            Err(mpsc::error::TryRecvError::Disconnected) => {
                self.wikidata_state = None;
                return;
            }
        };
        let WikidataState { query, .. } = self.wikidata_state.take().unwrap();
        match result {
            Ok(entity) => {
                let body = super::wikidata::render(&entity);
                let mut turn = ChatTurn::with_response(format!("/wikidata {query}"), body);
                turn.wikidata = Some(entity.qid.clone());
                turn.sources = vec![format!("Wikidata {}", entity.qid)];
                self.chat_history.push(turn);
                self.chat_scroll = 0;
                self.status_message = Some(format!(
                    "{} ({}) — structured facts · /fact to record (gate-skipped)",
                    entity.label, entity.qid
                ));
            }
            Err(e) => self.status_message = Some(format!("wikidata: {e}")),
        }
    }

    /// `/openalex` / `/arxiv` (R3-B) — spawn a scholarly fetch. The result is a
    /// paper's metadata + abstract; a `/fact` from it records `origin=<source>`,
    /// skips the gate (authoritative metadata), and auto-creates a `BibEntry`.
    fn start_scholarly(&mut self, source: &'static str, query: String) {
        if self.scholarly_state.is_some() {
            self.status_message = Some("a scholarly query is already running".to_string());
            return;
        }
        let query = query.trim().to_string();
        if query.is_empty() {
            self.status_message = Some(format!("usage: /{source} <query>"));
            return;
        }
        if !super::scholarly::available(&self.cfg.research.scholarly) {
            self.status_message = Some("scholarly search disabled (research.scholarly.enabled)".to_string());
            return;
        }
        let cfg = self.cfg.research.scholarly.clone();
        let q = query.clone();
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let r = match source {
                "arxiv" => super::scholarly::arxiv(q).await,
                _ => super::scholarly::openalex(cfg, q).await,
            }
            .map_err(|e| e.to_string());
            let _ = tx.send(r);
        });
        self.scholarly_state = Some(ScholarlyState { rx, query });
        self.status_message = Some(format!("Querying {source}…"));
    }

    /// Drain the in-flight scholarly fetch; on success, show the paper.
    fn poll_scholarly(&mut self) {
        let Some(sc) = self.scholarly_state.as_mut() else { return };
        let result = match sc.rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::error::TryRecvError::Empty) => return,
            Err(mpsc::error::TryRecvError::Disconnected) => {
                self.scholarly_state = None;
                return;
            }
        };
        let ScholarlyState { query, .. } = self.scholarly_state.take().unwrap();
        match result {
            Ok(paper) => {
                let body = super::scholarly::render(&paper);
                let ident = paper.cite_detail();
                let mut turn = ChatTurn::with_response(format!("/{} {query}", paper.source), body);
                turn.sources = vec![format!("{} {ident}", paper.source)];
                turn.paper = Some(paper);
                self.chat_history.push(turn);
                self.chat_scroll = 0;
                self.status_message =
                    Some(format!("{ident} — /fact to record (auto-cites to Sources)"));
            }
            Err(e) => self.status_message = Some(format!("scholarly: {e}")),
        }
    }

    /// R3-B — write a SOURCES-1 `BibEntry` into the Sources book under a
    /// "Research" chapter (dedup by cite key). Mirrors the `sources import` path.
    fn add_bibentry(&self, entry: &crate::sources::BibEntry) -> anyhow::Result<bool> {
        add_bibentry(&self.store, &self.cfg, entry)
    }

    /// `/triangulate [claim]` (R3-E) — cross-check a claim against the independent
    /// structured sources (Wikidata + OpenAlex + arXiv). Phase 1 gathers each
    /// source's evidence concurrently; phase 2 (in `poll_triangulate`) has the LLM
    /// judge each source SUPPORTS / CONTRADICTS / SILENT — a far stronger check
    /// than the model grading its own output.
    fn start_triangulate(&mut self, claim: String) {
        if self.triangulate.is_some() || self.stream_rx.is_some() {
            self.status_message = Some("a response is already in flight".to_string());
            return;
        }
        let claim = if claim.trim().is_empty() {
            self.chat_history.last().map(|t| t.response.clone()).unwrap_or_default()
        } else {
            claim
        };
        let claim = claim.trim().to_string();
        if claim.is_empty() {
            self.status_message =
                Some("usage: /triangulate <claim>  (bare: cross-check the last response)".to_string());
            return;
        }
        let wd_cfg = self.cfg.research.wikidata.clone();
        let sc_cfg = self.cfg.research.scholarly.clone();
        let wd_on = super::wikidata::available(&wd_cfg);
        let sc_on = super::scholarly::available(&sc_cfg);
        if !wd_on && !sc_on {
            self.status_message =
                Some("triangulate: no structured sources enabled (wikidata / scholarly)".to_string());
            return;
        }
        let (lang, _note) = crate::prose::resolve_prose_language(None, &self.cfg.language);
        let code = match lang.as_code() {
            "other" => "en".to_string(),
            c => c.to_string(),
        };
        let q = claim.clone();
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut evidence: Vec<(String, String)> = Vec::new();
            if wd_on {
                if let Ok(e) = super::wikidata::fetch(wd_cfg, q.clone(), code).await {
                    evidence.push(("Wikidata".to_string(), super::wikidata::render(&e)));
                }
            }
            if sc_on {
                if let Ok(p) = super::scholarly::openalex(sc_cfg.clone(), q.clone()).await {
                    evidence.push(("OpenAlex".to_string(), super::scholarly::render(&p)));
                }
                if let Ok(p) = super::scholarly::arxiv(q.clone()).await {
                    evidence.push(("arXiv".to_string(), super::scholarly::render(&p)));
                }
            }
            let _ = tx.send(evidence);
        });
        self.triangulate = Some(TriangulateState { rx, claim });
        self.status_message = Some("Triangulating across sources…".to_string());
    }

    /// Phase 2 of `/triangulate` — evidence gathered; have the LLM judge each
    /// source against the claim and stream the agreement report.
    fn poll_triangulate(&mut self) {
        let Some(tr) = self.triangulate.as_mut() else { return };
        let evidence = match tr.rx.try_recv() {
            Ok(e) => e,
            Err(mpsc::error::TryRecvError::Empty) => return,
            Err(mpsc::error::TryRecvError::Disconnected) => {
                self.triangulate = None;
                return;
            }
        };
        let TriangulateState { claim, .. } = self.triangulate.take().unwrap();
        if evidence.is_empty() {
            self.status_message = Some("triangulate: no source returned evidence".to_string());
            return;
        }
        let ai = match crate::ai::AiClient::from_config(&self.cfg.llm) {
            Ok(a) => a,
            Err(e) => {
                self.status_message = Some(format!("no LLM provider: {e}"));
                return;
            }
        };
        let (model, _env) = match ai.resolve_provider(&self.cfg.llm, None) {
            Ok(m) => m,
            Err(e) => {
                self.status_message = Some(format!("provider error: {e}"));
                return;
            }
        };
        let (lang, _note) = crate::prose::resolve_prose_language(None, &self.cfg.language);
        let language = super::extract::language_name(&lang);
        let mut ev = String::new();
        let mut labels: Vec<String> = Vec::new();
        for (label, text) in &evidence {
            let body: String = text.chars().take(1200).collect();
            ev.push_str(&format!("[{label}]\n{}\n\n", body.trim()));
            labels.push(label.clone());
        }
        let system = format!(
            "You are triangulating a claim against INDEPENDENT sources. Judge ONLY from the sources \
             below — do not use outside knowledge. For EACH source, output one line:\n\
             <source>: SUPPORTS | CONTRADICTS | SILENT — <short reason>\n\
             Then a final line: `Agreement: <n>/<m> support`. Write in {language}."
        );
        let user = format!("Claim:\n{claim}\n\nSources:\n{ev}");
        let short: String = claim.chars().take(60).collect();
        let mut turn = ChatTurn::new(format!("/triangulate {short}"));
        turn.streaming = true;
        turn.model = model.to_string();
        turn.sources = labels;
        self.chat_history.push(turn);
        self.streaming_turn = Some(self.chat_history.len() - 1);
        self.chat_scroll = 0;
        let rx = spawn_chat_stream(
            ai.client.clone(),
            model.to_string(),
            Some(system),
            Vec::new(),
            user,
            llm::CATEGORY,
        );
        self.stream_rx = Some(rx);
        self.status_message =
            Some(format!("Judging {} source(s) against the claim…", evidence.len()));
    }

    /// Drain the in-flight web search; on results, ingest them or ground a chat.
    fn poll_web(&mut self) {
        let Some(web) = self.web.as_mut() else { return };
        let result = match web.rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::error::TryRecvError::Empty) => return,
            Err(mpsc::error::TryRecvError::Disconnected) => {
                self.web = None;
                return;
            }
        };
        let WebState { ingest, query, .. } = self.web.take().unwrap();
        let results = match result {
            Ok(r) if !r.is_empty() => r,
            Ok(_) => {
                self.status_message = Some("web: no results".to_string());
                return;
            }
            Err(e) => {
                self.status_message = Some(format!("web: {e}"));
                return;
            }
        };
        if ingest {
            self.web_ingest(&query, &results);
        } else {
            self.web_chat(&query, &results);
        }
    }

    /// `--ingest` — embed the fetched pages as research sources (origin web).
    fn web_ingest(&mut self, query: &str, results: &[super::web::WebResult]) {
        use super::imports;
        let chunk_chars = self.cfg.research.import_chunk_chars.max(200);
        let now = chrono::Utc::now().to_rfc3339();
        let mut imports_store = imports::Imports::load(&self.layout);
        let mut total = 0usize;
        for r in results {
            if r.text.trim().is_empty() {
                continue;
            }
            let name = imports::source_name(std::path::Path::new(&r.url));
            let chunks = imports::chunk_text(&r.text, chunk_chars);
            let mut doc_ids = Vec::new();
            for (i, chunk) in chunks.iter().enumerate() {
                let meta = serde_json::json!({
                    "kind": imports::SOURCE_KIND, "source": r.url, "name": name,
                    "thread": self.thread.name, "chunk": i, "imported_at": now, "origin": "web",
                    "title": r.title,
                });
                if let Ok(id) = self.store.raw().add_document(meta, chunk.as_bytes()) {
                    doc_ids.push(id.to_string());
                }
            }
            if let Some(old) = imports_store.sources.get(&name) {
                for id in &old.doc_ids {
                    if let Ok(u) = uuid::Uuid::parse_str(id) {
                        let _ = self.store.raw().delete_document(u);
                    }
                }
            }
            total += doc_ids.len();
            let chunks_n = doc_ids.len();
            imports_store.sources.insert(
                name.clone(),
                imports::ImportedSource {
                    name,
                    path: r.url.clone(),
                    doc_ids,
                    thread: self.thread.name.clone(),
                    imported_at: now.clone(),
                    chunks: chunks_n,
                },
            );
        }
        let _ = imports_store.save(&self.layout);
        let report: String = results
            .iter()
            .map(|r| format!("• {}\n    {}", r.title, r.url))
            .collect::<Vec<_>>()
            .join("\n");
        self.chat_history.push(ChatTurn::with_response(
            format!("/web --ingest {query}"),
            format!("Ingested {} chunk(s) from {} page(s):\n{report}", total, results.len()),
        ));
        self.chat_scroll = 0;
        self.status_message = Some(format!("✓ web: ingested {total} chunk(s)"));
    }

    /// Default — ground an LLM chat answer on the fetched pages, cited by URL.
    /// The turn is marked web-grounded so a `/fact` from it is fact-checked
    /// before it commits (WC-P3).
    fn web_chat(&mut self, query: &str, results: &[super::web::WebResult]) {
        let ai = match crate::ai::AiClient::from_config(&self.cfg.llm) {
            Ok(a) => a,
            Err(e) => {
                self.status_message = Some(format!("no LLM provider: {e}"));
                return;
            }
        };
        let (model, _env) = match ai.resolve_provider(&self.cfg.llm, None) {
            Ok(m) => m,
            Err(e) => {
                self.status_message = Some(format!("provider error: {e}"));
                return;
            }
        };
        let (lang, _note) = crate::prose::resolve_prose_language(None, &self.cfg.language);
        let language = super::extract::language_name(&lang);
        // Build the web context block (bounded per result).
        let mut ctx = String::new();
        let mut sources: Vec<String> = Vec::new();
        for r in results {
            let body: String = r.text.chars().take(1500).collect();
            ctx.push_str(&format!("[{} — {}]\n{}\n\n", r.title, r.url, body.trim()));
            sources.push(if r.title.is_empty() { r.url.clone() } else { format!("{} ({})", r.title, r.url) });
        }
        let system = format!(
            "You are a research assistant. Answer the question using ONLY these web search results, \
             and cite the sources you use by title and URL. If the results don't answer it, say so. \
             Do not invent facts. Write in {language}.\n\nWeb results:\n{ctx}"
        );
        let mut turn = ChatTurn::new(format!("/web {query}"));
        turn.streaming = true;
        turn.web_grounded = true;
        turn.sources = sources;
        self.chat_history.push(turn);
        self.streaming_turn = Some(self.chat_history.len() - 1);
        self.chat_scroll = 0;
        let rx = spawn_chat_stream(
            ai.client.clone(),
            model.to_string(),
            Some(system),
            Vec::new(),
            query.to_string(),
            llm::CATEGORY,
        );
        self.stream_rx = Some(rx);
        self.status_message = Some(format!("web: grounding answer on {} source(s)…", results.len()));
    }

    /// `/promote [notes/path] [→ facts/path]` — turn a Note into a verified Fact
    /// (T-P5): re-run extraction over the note's text into Facts, with provenance
    /// `origin=promoted`. Non-destructive — the note stays.
    fn start_promote(&mut self, note: Option<String>, path: Option<String>) {
        // Resolve the source note: an explicit path, else the thread's most recent
        // note insertion.
        let note_path = note.or_else(|| {
            self.thread
                .turns
                .iter()
                .rev()
                .find(|t| t.kind == TurnKind::NoteInsertion)
                .and_then(|t| t.insertion_path.clone())
        });
        let Some(note_path) = note_path else {
            self.status_message =
                Some("usage: /promote <notes/path>  (no recent note to promote)".to_string());
            return;
        };
        let trimmed = note_path.trim().trim_start_matches('/');
        let node = self.hierarchy.find_by_path(trimmed).or_else(|| {
            let stripped = trimmed.strip_prefix("notes/").unwrap_or(trimmed);
            self.hierarchy.find_by_path(stripped)
        });
        let Some(node) = node else {
            self.status_message = Some(format!("note not found: {note_path}"));
            return;
        };
        let note_id = node.id;
        let text = match self.store.get_content(note_id) {
            Ok(Some(bytes)) => String::from_utf8_lossy(&bytes).trim().to_string(),
            _ => String::new(),
        };
        if text.is_empty() {
            self.status_message = Some("that note is empty".to_string());
            return;
        }
        let prov = ProvMeta {
            origin: "promoted".to_string(),
            query: String::new(),
            detail: note_path.clone(),
        };
        self.start_extraction_from(TargetBook::Facts, text, None, path, "/promote", prov);
    }

    /// `/factcheck` — audit the Facts corpus: a chunked truth pass over every
    /// fact, then a single consistency pass over the whole set. Multi-call.
    fn start_factcheck(&mut self) {
        if self.factcheck.is_some() {
            self.status_message = Some("a fact-check is already running".to_string());
            return;
        }
        let Some(book_id) = self.facts_tree.root else {
            self.status_message = Some("no Facts book".to_string());
            return;
        };
        let facts = super::factcheck::gather_facts(&self.store, &self.hierarchy, book_id);
        // UD-P2 — authorial facts are excluded from the audit but counted.
        let undisputed = super::factcheck::gather_undisputed(&self.store, &self.hierarchy, book_id).len();
        if facts.is_empty() {
            self.status_message = Some(if undisputed > 0 {
                format!("no disputed facts to check — {undisputed} undisputed (try /undisputed)")
            } else {
                "no facts to check — add some first".to_string()
            });
            return;
        }
        // R2-E — one streaming turn carries the live tokens of each call, then the
        // final report when the audit finishes.
        let mut preview = ChatTurn::new("/factcheck".to_string());
        preview.streaming = true;
        self.chat_history.push(preview);
        self.chat_scroll = 0;
        let preview_idx = Some(self.chat_history.len() - 1);
        self.factcheck = Some(FactCheckState {
            facts,
            truth_phase: true,
            chunk_idx: 0,
            truth_report: String::new(),
            consist_groups: Vec::new(),
            consist_idx: 0,
            consist_report: String::new(),
            rx: None,
            buf: String::new(),
            preview_idx,
            undisputed,
        });
        self.factcheck_next_call();
    }

    /// Spawn the next `/factcheck` call (a truth chunk, or the consistency pass).
    fn factcheck_next_call(&mut self) {
        let (lang, _note) = crate::prose::resolve_prose_language(None, &self.cfg.language);
        let language = super::extract::language_name(&lang);
        let Some(fc) = self.factcheck.as_mut() else { return };

        let total = fc.facts.len();
        let chunk_size = super::factcheck::TRUTH_CHUNK;
        let n_chunks = total.div_ceil(chunk_size);

        let (system, user, status) = if fc.truth_phase && fc.chunk_idx < n_chunks {
            let base = fc.chunk_idx * chunk_size;
            let chunk: Vec<&super::factcheck::FactEntry> =
                fc.facts[base..(base + chunk_size).min(total)].iter().collect();
            (
                super::factcheck::truth_system(language),
                super::factcheck::truth_user(&chunk, base),
                format!("fact-check: truth {}/{n_chunks}…", fc.chunk_idx + 1),
            )
        } else {
            // Truth pass done — build the bounded consistency groups once.
            if fc.truth_phase {
                fc.truth_phase = false;
                fc.consist_groups =
                    super::factcheck::consistency_groups(&fc.facts, super::factcheck::CONSIST_MAX);
            }
            if fc.consist_idx >= fc.consist_groups.len() {
                // No (more) consistency calls — assemble + emit the report.
                self.finish_factcheck();
                return;
            }
            let g = &fc.consist_groups[fc.consist_idx];
            let subset: Vec<&super::factcheck::FactEntry> = g.idxs.iter().map(|&i| &fc.facts[i]).collect();
            let status = format!(
                "fact-check: consistency {}/{} ({})…",
                fc.consist_idx + 1,
                fc.consist_groups.len(),
                g.label
            );
            (
                super::factcheck::consistency_system(language),
                super::factcheck::consistency_user(&subset),
                status,
            )
        };

        let ai = match crate::ai::AiClient::from_config(&self.cfg.llm) {
            Ok(a) => a,
            Err(e) => {
                self.status_message = Some(format!("no LLM provider: {e}"));
                self.factcheck = None;
                return;
            }
        };
        let (model, _env) = match ai.resolve_provider(&self.cfg.llm, None) {
            Ok(m) => m,
            Err(e) => {
                self.status_message = Some(format!("provider error: {e}"));
                self.factcheck = None;
                return;
            }
        };
        let rx = spawn_chat_stream(ai.client.clone(), model.to_string(), Some(system), Vec::new(), user, llm::CATEGORY);
        let preview_idx = self.factcheck.as_ref().and_then(|fc| fc.preview_idx);
        if let Some(fc) = self.factcheck.as_mut() {
            fc.rx = Some(rx);
            fc.buf.clear();
        }
        // Reset the live preview to this call's header; tokens append below it.
        if let Some(i) = preview_idx {
            if let Some(turn) = self.chat_history.get_mut(i) {
                turn.response = format!("⋯ {status}\n");
            }
        }
        self.status_message = Some(status);
    }

    /// Drain the in-flight `/factcheck` call; advance through truth chunks, then
    /// the consistency pass, then emit the report.
    fn poll_factcheck(&mut self) {
        let Some(fc) = self.factcheck.as_mut() else { return };
        let Some(rx) = fc.rx.as_mut() else { return };
        let preview_idx = fc.preview_idx;
        let mut done = false;
        let mut new_tokens = String::new();
        loop {
            match rx.try_recv() {
                Ok(StreamMsg::Token(t)) => {
                    new_tokens.push_str(&t);
                    fc.buf.push_str(&t);
                }
                Ok(StreamMsg::Done(_)) | Err(mpsc::error::TryRecvError::Disconnected) => {
                    done = true;
                    break;
                }
                Ok(StreamMsg::Error(e)) => {
                    self.drop_preview(preview_idx);
                    self.status_message = Some(format!("fact-check error: {e}"));
                    self.factcheck = None;
                    return;
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
            }
        }
        // Mirror the just-arrived tokens into the live preview turn (dim).
        if !new_tokens.is_empty() {
            if let Some(i) = preview_idx {
                if let Some(turn) = self.chat_history.get_mut(i) {
                    turn.response.push_str(&new_tokens);
                }
            }
        }
        if !done {
            return;
        }
        // Record the finished call's output (truth chunk or one consistency group),
        // then spawn the next call — `factcheck_next_call` emits the final report
        // once every group is done.
        {
            let fc = self.factcheck.as_mut().unwrap();
            fc.rx = None;
            let out = std::mem::take(&mut fc.buf);
            if fc.truth_phase {
                fc.truth_report.push_str(out.trim());
                fc.truth_report.push('\n');
                fc.chunk_idx += 1;
            } else {
                let multi = fc.consist_groups.len() > 1;
                let trimmed = out.trim();
                // Suppress the boilerplate "no contradictions" per-group when there
                // are several groups — only surface the groups that found something.
                let is_clean = trimmed.eq_ignore_ascii_case("No contradictions found.")
                    || trimmed.is_empty();
                if !(multi && is_clean) {
                    if multi {
                        let label = fc.consist_groups[fc.consist_idx].label.clone();
                        fc.consist_report.push_str(&format!("· {label}: "));
                    }
                    fc.consist_report.push_str(trimmed);
                    fc.consist_report.push('\n');
                }
                fc.consist_idx += 1;
            }
        }
        self.factcheck_next_call();
    }

    /// Assemble and emit the final `/factcheck` report, then clear the state.
    fn finish_factcheck(&mut self) {
        let Some(fc) = self.factcheck.as_ref() else { return };
        let total = fc.facts.len();
        let n_groups = fc.consist_groups.len();
        let undisputed = fc.undisputed;

        // RE-P5 — capture per-fact verdicts (✓ / ? / ✗) from the truth pass and
        // persist them so the Facts tree can flag problem paragraphs.
        let now = chrono::Utc::now().to_rfc3339();
        let fact_ids: Vec<uuid::Uuid> = fc.facts.iter().map(|f| f.id).collect();
        let parsed = super::verdicts::parse_truth_report(&fc.truth_report, &fact_ids, &now);
        let (mut accurate, mut dubious, mut inaccurate) = (0usize, 0usize, 0usize);
        for v in parsed.values() {
            match v.level {
                super::verdicts::Level::Accurate => accurate += 1,
                super::verdicts::Level::Dubious => dubious += 1,
                super::verdicts::Level::Inaccurate => inaccurate += 1,
            }
        }
        // Refresh only the just-checked facts (leave older marks for untouched nodes).
        for (k, v) in parsed {
            self.fact_verdicts.facts.insert(k, v);
        }
        let _ = self.fact_verdicts.save(&self.layout);
        let consistency = {
            let c = fc.consist_report.trim();
            if c.is_empty() { "No contradictions found.".to_string() } else { c.to_string() }
        };
        let undisputed_note = if undisputed > 0 {
            format!("\n※ {undisputed} undisputed fact(s) excluded (authorial · check with /undisputed)")
        } else {
            String::new()
        };
        let report = format!(
            "[/factcheck — {total} fact(s), {n_groups} consistency group(s)]\n\
             ✓ {accurate} accurate · ? {dubious} questionable · ✗ {inaccurate} inaccurate \
             (flags shown in the Facts tree · /whatswrong on a flagged fact){undisputed_note}\n\n\
             ── Factual accuracy ──\n{}\n\n── Mutual consistency ──\n{consistency}\n",
            fc.truth_report.trim(),
        );
        let preview_idx = fc.preview_idx;
        // Reuse the streaming preview turn for the final report (no duplicate turn).
        match preview_idx.and_then(|i| self.chat_history.get_mut(i)) {
            Some(turn) => {
                turn.response = report;
                turn.streaming = false;
            }
            None => self.chat_history.push(ChatTurn::with_response("/factcheck".to_string(), report)),
        }
        self.chat_scroll = 0;
        self.factcheck = None;
        self.status_message = Some(format!(
            "fact-check complete · {total} fact(s) · ✓{accurate} ?{dubious} ✗{inaccurate}"
        ));
    }

    /// RESRCH-UNDISPUTED (UD-P3) — `/undisputed`: a chunked, language-aware
    /// common-sense check over the authorial (undisputed) facts. Read-only —
    /// reports into the chat, never rewrites. Mirrors `/factcheck`'s truth phase.
    fn start_undisputed(&mut self) {
        if self.undisputed_check.is_some() {
            self.status_message = Some("an undisputed check is already running".to_string());
            return;
        }
        let Some(book_id) = self.facts_tree.root else {
            self.status_message = Some("no Facts book".to_string());
            return;
        };
        let facts = super::factcheck::gather_undisputed(&self.store, &self.hierarchy, book_id);
        if facts.is_empty() {
            self.status_message =
                Some("no undisputed facts — mark one with `u` (※) in the Facts tree first".to_string());
            return;
        }
        let mut preview = ChatTurn::new("/undisputed".to_string());
        preview.streaming = true;
        self.chat_history.push(preview);
        self.chat_scroll = 0;
        let preview_idx = Some(self.chat_history.len() - 1);
        self.undisputed_check = Some(UndisputedState {
            facts,
            chunk_idx: 0,
            report: String::new(),
            rx: None,
            buf: String::new(),
            preview_idx,
        });
        self.undisputed_next_call();
    }

    /// Spawn the next `/undisputed` common-sense chunk, or finalise.
    fn undisputed_next_call(&mut self) {
        let (lang, _note) = crate::prose::resolve_prose_language(None, &self.cfg.language);
        let language = super::extract::language_name(&lang);
        let (base, chunk_n, total, n_chunks) = {
            let Some(uc) = self.undisputed_check.as_ref() else { return };
            let total = uc.facts.len();
            let n_chunks = total.div_ceil(super::factcheck::TRUTH_CHUNK);
            if uc.chunk_idx >= n_chunks {
                self.finish_undisputed();
                return;
            }
            (uc.chunk_idx * super::factcheck::TRUTH_CHUNK, uc.chunk_idx + 1, total, n_chunks)
        };
        let (system, user) = {
            let uc = self.undisputed_check.as_ref().unwrap();
            let chunk: Vec<&super::factcheck::FactEntry> = uc.facts
                [base..(base + super::factcheck::TRUTH_CHUNK).min(total)]
                .iter()
                .collect();
            (super::factcheck::undisputed_system(language), super::factcheck::truth_user(&chunk, base))
        };
        let ai = match crate::ai::AiClient::from_config(&self.cfg.llm) {
            Ok(a) => a,
            Err(e) => {
                self.status_message = Some(format!("no LLM provider: {e}"));
                self.undisputed_check = None;
                return;
            }
        };
        let (model, _env) = match ai.resolve_provider(&self.cfg.llm, None) {
            Ok(m) => m,
            Err(e) => {
                self.status_message = Some(format!("provider error: {e}"));
                self.undisputed_check = None;
                return;
            }
        };
        let status = format!("undisputed: common sense {chunk_n}/{n_chunks}…");
        let rx = spawn_chat_stream(ai.client.clone(), model.to_string(), Some(system), Vec::new(), user, llm::CATEGORY);
        let preview_idx = self.undisputed_check.as_ref().and_then(|u| u.preview_idx);
        if let Some(uc) = self.undisputed_check.as_mut() {
            uc.rx = Some(rx);
            uc.buf.clear();
        }
        if let Some(i) = preview_idx {
            if let Some(t) = self.chat_history.get_mut(i) {
                t.response = format!("⋯ {status}\n");
            }
        }
        self.status_message = Some(status);
    }

    /// Drain the in-flight `/undisputed` chunk; advance or finalise.
    fn poll_undisputed(&mut self) {
        let Some(uc) = self.undisputed_check.as_mut() else { return };
        let Some(rx) = uc.rx.as_mut() else { return };
        let preview_idx = uc.preview_idx;
        let mut done = false;
        let mut new_tokens = String::new();
        loop {
            match rx.try_recv() {
                Ok(StreamMsg::Token(t)) => {
                    new_tokens.push_str(&t);
                    uc.buf.push_str(&t);
                }
                Ok(StreamMsg::Done(_)) | Err(mpsc::error::TryRecvError::Disconnected) => {
                    done = true;
                    break;
                }
                Ok(StreamMsg::Error(e)) => {
                    self.drop_preview(preview_idx);
                    self.status_message = Some(format!("undisputed error: {e}"));
                    self.undisputed_check = None;
                    return;
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
            }
        }
        if !new_tokens.is_empty() {
            if let Some(i) = preview_idx {
                if let Some(t) = self.chat_history.get_mut(i) {
                    t.response.push_str(&new_tokens);
                }
            }
        }
        if !done {
            return;
        }
        {
            let uc = self.undisputed_check.as_mut().unwrap();
            uc.rx = None;
            let out = std::mem::take(&mut uc.buf);
            uc.report.push_str(out.trim());
            uc.report.push('\n');
            uc.chunk_idx += 1;
        }
        self.undisputed_next_call();
    }

    /// Assemble + emit the `/undisputed` report (with a PLAUSIBLE/ODD/INCOHERENT
    /// tally), then clear the state.
    fn finish_undisputed(&mut self) {
        let Some(uc) = self.undisputed_check.as_ref() else { return };
        let total = uc.facts.len();
        // UD-P4 — persist per-fact common-sense verdicts (colour the ※ glyph).
        let now = chrono::Utc::now().to_rfc3339();
        let fact_ids: Vec<uuid::Uuid> = uc.facts.iter().map(|f| f.id).collect();
        let parsed = super::verdicts::parse_undisputed_report(&uc.report, &fact_ids, &now);
        for (k, v) in parsed {
            self.undisputed_verdicts.facts.insert(k, v);
        }
        let _ = self.undisputed_verdicts.save_undisputed(&self.layout);
        let (mut plausible, mut odd, mut incoherent) = (0usize, 0usize, 0usize);
        for line in uc.report.lines() {
            let head = line.to_ascii_uppercase();
            let head = head.split('—').next().unwrap_or("");
            if head.contains("INCOHERENT") {
                incoherent += 1;
            } else if head.contains("ODD") {
                odd += 1;
            } else if head.contains("PLAUSIBLE") {
                plausible += 1;
            }
        }
        let report = format!(
            "[/undisputed — {total} authorial fact(s)]\n\
             ✓ {plausible} plausible · ? {odd} odd · ✗ {incoherent} incoherent \
             (internal common sense only — real-world truth deliberately not checked)\n\n{}\n",
            uc.report.trim(),
        );
        let preview_idx = uc.preview_idx;
        match preview_idx.and_then(|i| self.chat_history.get_mut(i)) {
            Some(turn) => {
                turn.response = report;
                turn.streaming = false;
            }
            None => self.chat_history.push(ChatTurn::with_response("/undisputed".to_string(), report)),
        }
        self.chat_scroll = 0;
        self.undisputed_check = None;
        self.status_message =
            Some(format!("undisputed check complete · {total} · ✓{plausible} ?{odd} ✗{incoherent}"));
    }

    /// R-P15 — start a `/chain` pipeline (steps already `→`-split + trimmed).
    fn start_chain(&mut self, steps: Vec<String>) {
        if self.chain.is_some() {
            self.status_message = Some("a chain is already running".to_string());
            return;
        }
        if steps.is_empty() {
            self.status_message = Some("usage: /chain q1 → q2 → q3".to_string());
            return;
        }
        self.chain = Some(ChainState { steps, current: 0, accumulated: Vec::new(), rx: None, turn_idx: 0 });
        self.start_chain_step();
    }

    /// Spawn the current chain step's stream (with prior steps as context).
    fn start_chain_step(&mut self) {
        let Some(chain) = self.chain.as_mut() else { return };
        let i = chain.current;
        let total = chain.steps.len();
        let step = chain.steps[i].clone();
        let accumulated = chain.accumulated.join("\n\n");

        let ai = match crate::ai::AiClient::from_config(&self.cfg.llm) {
            Ok(a) => a,
            Err(e) => {
                self.status_message = Some(format!("no LLM provider: {e}"));
                self.chain = None;
                return;
            }
        };
        let (model, _env) = match ai.resolve_provider(&self.cfg.llm, None) {
            Ok(m) => m,
            Err(e) => {
                self.status_message = Some(format!("provider error: {e}"));
                self.chain = None;
                return;
            }
        };

        let (rag, sources) = super::rag::build_context(
            &self.store,
            &self.cfg,
            &self.hierarchy,
            self.facts_tree.root,
            &self.pinned_nodes,
            self.thread.rag_mode,
            &step,
        );
        let mut system = llm::system_prompt(self.thread.rag_mode, rag.as_deref());
        if i > 0 {
            system.push_str(&format!(
                "\n\nPrevious research (step {}/{}):\n{}",
                i,
                total,
                accumulated
            ));
        }

        let mut turn = ChatTurn::new(format!("[Step {}/{}] {}", i + 1, total, step));
        turn.streaming = true;
        turn.model = model.to_string();
        turn.sources = sources;
        self.chat_history.push(turn);
        let turn_idx = self.chat_history.len() - 1;
        self.chat_scroll = 0;

        let rx = spawn_chat_stream(
            ai.client.clone(),
            model.to_string(),
            Some(system),
            Vec::new(),
            step,
            llm::CATEGORY,
        );
        if let Some(chain) = self.chain.as_mut() {
            chain.rx = Some(rx);
            chain.turn_idx = turn_idx;
        }
        self.status_message = Some(format!("[Step {}/{} running…]", i + 1, total));
    }

    /// Drain the current chain step; on completion, advance to the next step or
    /// finish the chain.
    fn poll_chain(&mut self) {
        let Some(chain) = self.chain.as_mut() else { return };
        let Some(rx) = chain.rx.as_mut() else { return };
        let idx = chain.turn_idx;
        let mut done = false;
        loop {
            match rx.try_recv() {
                Ok(StreamMsg::Token(t)) => {
                    if let Some(turn) = self.chat_history.get_mut(idx) {
                        turn.response.push_str(&t);
                    }
                }
                Ok(StreamMsg::Done(usage)) => {
                    if let Some(turn) = self.chat_history.get_mut(idx) {
                        turn.usage = usage;
                    }
                    done = true;
                    break;
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    done = true;
                    break;
                }
                Ok(StreamMsg::Error(e)) => {
                    if let Some(turn) = self.chat_history.get_mut(idx) {
                        turn.response.push_str(&format!("\n[error: {e}]"));
                    }
                    done = true;
                    break;
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
            }
        }
        if !done {
            return;
        }
        // Finalise this step.
        let response = self.chat_history.get(idx).map(|t| t.response.clone()).unwrap_or_default();
        if let Some(turn) = self.chat_history.get_mut(idx) {
            turn.streaming = false;
            let (cost, exact) = llm::cost_for(
                &self.cfg.cost,
                &turn.model,
                turn.usage,
                &turn.prompt,
                &turn.response,
            );
            turn.cost = cost;
            self.session_cost += cost;
            self.session_cost_exact &= exact;
        }
        let Some(chain) = self.chain.as_mut() else { return };
        chain.rx = None;
        chain.accumulated.push(response);
        chain.current += 1;
        if chain.current < chain.steps.len() {
            self.start_chain_step();
        } else {
            self.chain = None;
            self.status_message = Some("chain complete".to_string());
        }
    }

    /// `/rag [facts+full|facts|full]` — set the mode explicitly, or cycle when
    /// no argument is given.
    fn set_rag_mode(&mut self, arg: Option<&str>) {
        use super::thread::RagMode;
        let mode = match arg.map(|a| a.trim().to_ascii_lowercase()) {
            Some(a) if a == "facts" || a == "facts-only" || a == "factsonly" => Some(RagMode::FactsOnly),
            Some(a) if a == "full" || a == "full-only" || a == "fullonly" => Some(RagMode::FullOnly),
            Some(a) if a == "facts+full" || a == "both" => Some(RagMode::FactsPlusFull),
            Some(_) => None,
            None => Some(self.thread.rag_mode.next()),
        };
        match mode {
            Some(m) => {
                self.thread.rag_mode = m;
                let _ = self.thread.save(&self.layout);
                self.status_message = Some(format!("RAG: {}", m.label()));
            }
            None => self.status_message = Some("usage: /rag [facts+full|facts|full]".to_string()),
        }
    }

    /// `/save [name]` — rename the current thread (and migrate its file), or
    /// just persist when no name is given.
    fn save_thread_as(&mut self, name: Option<&str>) {
        if let Some(name) = name.map(str::trim).filter(|s| !s.is_empty()) {
            let old_slug = self.thread.name.clone();
            self.thread.display_name = name.to_string();
            self.thread.name = thread::thread_slug(name);
            if self.thread.save(&self.layout).is_ok() && self.thread.name != old_slug {
                let _ = thread::delete_thread(&self.layout, &old_slug);
            }
            self.status_message = Some(format!("saved as `{name}`"));
        } else {
            let _ = self.thread.save(&self.layout);
            self.status_message = Some("thread saved".to_string());
        }
    }

    /// R-P7 — start a streamed research query. Builds the system prompt (RAG
    /// context arrives in R-P8), replays the prior turns, and spawns the stream.
    fn start_query(&mut self, prompt: String) {
        // Already streaming? Ignore (one in-flight query at a time).
        if self.stream_rx.is_some() {
            self.status_message = Some("a query is still streaming…".to_string());
            return;
        }
        let ai = match crate::ai::AiClient::from_config(&self.cfg.llm) {
            Ok(a) => a,
            Err(e) => {
                self.chat_history
                    .push(ChatTurn::with_response(prompt, format!("[no LLM provider: {e}]")));
                return;
            }
        };
        let (model, _env) = match ai.resolve_provider(&self.cfg.llm, None) {
            Ok(m) => m,
            Err(e) => {
                self.chat_history
                    .push(ChatTurn::with_response(prompt, format!("[provider error: {e}]")));
                return;
            }
        };

        // Replay completed turns as conversation history.
        let history: Vec<AiTurn> = self
            .chat_history
            .iter()
            .flat_map(|t| {
                [AiTurn::User(t.prompt.clone()), AiTurn::Assistant(t.response.clone())]
            })
            .collect();

        // R-P8 / R2-B — assemble RAG context (pins + Facts + imported sources),
        // gated by mode; `sources` names the imported docs that grounded it.
        let (rag, sources) = super::rag::build_context(
            &self.store,
            &self.cfg,
            &self.hierarchy,
            self.facts_tree.root,
            &self.pinned_nodes,
            self.thread.rag_mode,
            &prompt,
        );
        let system = llm::system_prompt(self.thread.rag_mode, rag.as_deref());

        let mut turn = ChatTurn::new(prompt.clone());
        turn.streaming = true;
        turn.model = model.to_string();
        turn.sources = sources;
        self.chat_history.push(turn);
        self.streaming_turn = Some(self.chat_history.len() - 1);

        let rx = spawn_chat_stream(
            ai.client.clone(),
            model.to_string(),
            Some(system),
            history,
            prompt,
            llm::CATEGORY,
        );
        self.stream_rx = Some(rx);
    }

    /// Drain the in-flight stream (R-P7), called each tick.
    fn poll_stream(&mut self) {
        let Some(rx) = self.stream_rx.as_mut() else { return };
        let Some(idx) = self.streaming_turn else { return };
        let mut done = false;
        loop {
            match rx.try_recv() {
                Ok(StreamMsg::Token(t)) => {
                    if let Some(turn) = self.chat_history.get_mut(idx) {
                        turn.response.push_str(&t);
                    }
                }
                Ok(StreamMsg::Done(usage)) => {
                    if let Some(turn) = self.chat_history.get_mut(idx) {
                        turn.usage = usage;
                    }
                    done = true;
                    break;
                }
                Ok(StreamMsg::Error(e)) => {
                    if let Some(turn) = self.chat_history.get_mut(idx) {
                        turn.response.push_str(&format!("\n[error: {e}]"));
                    }
                    done = true;
                    break;
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    done = true;
                    break;
                }
            }
        }
        if done {
            self.finish_stream(idx);
        }
    }

    /// Finalise a completed stream: cost, persistence, history rebuild.
    fn finish_stream(&mut self, idx: usize) {
        self.stream_rx = None;
        self.streaming_turn = None;
        let (prompt, response, cost) = match self.chat_history.get_mut(idx) {
            Some(turn) => {
                turn.streaming = false;
                let (cost, exact) = llm::cost_for(
                    &self.cfg.cost,
                    &turn.model,
                    turn.usage,
                    &turn.prompt,
                    &turn.response,
                );
                turn.cost = cost;
                self.session_cost += cost;
                self.session_cost_exact &= exact;
                (turn.prompt.clone(), turn.response.clone(), cost)
            }
            None => return,
        };
        let now = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::now_v7().to_string();
        let _ = self.thread.push_turn(
            ResearchTurn::query(id, prompt, response, cost, now),
            &self.layout,
        );
        self.rebuild_prompt_history();
        self.maybe_warn_budget();
    }

    /// Inform (never block) when the session cost passes `session_budget_warn`.
    fn maybe_warn_budget(&mut self) {
        let warn = self.cfg.research.session_budget_warn;
        if !self.budget_warned && warn > 0.0 && self.session_cost > warn {
            self.budget_warned = true;
            self.status_message =
                Some(format!("session cost ~${:.3} passed the ${warn:.2} budget note", self.session_cost));
        }
    }

    /// R-P12 — `/goto facts/path/slug`: resolve the slug path against the Facts
    /// book, expand its ancestors, place the cursor, and focus the tree.
    fn goto_path(&mut self, path: &str) {
        let trimmed = path.trim().trim_start_matches('/');
        let id = self
            .hierarchy
            .find_by_path(trimmed)
            .or_else(|| {
                let stripped = trimmed.strip_prefix("facts/").unwrap_or(trimmed);
                self.hierarchy.find_by_path(stripped)
            })
            .map(|n| n.id);
        match id {
            Some(id) if self.facts_tree.reveal(&self.hierarchy, id) => {
                self.focus = Focus::FactsTree;
                self.status_message = Some(format!("→ {path}"));
            }
            _ => self.status_message = Some(format!("Path not found: {path}")),
        }
    }

    /// R-P13 — `/diff`: embed the last response and show the most similar Facts
    /// already in the corpus (so the author can spot a near-duplicate before
    /// inserting). Reuses the Facts-scoped retriever; dedups pinned nodes.
    fn run_diff(&mut self) {
        let diff_top_n = self.cfg.research.diff_top_n.max(1);
        let Some(response) = self.chat_history.last().map(|t| t.response.clone()) else {
            self.status_message = Some("no research response to compare".to_string());
            return;
        };
        let Some(book_id) = self.facts_tree.root else {
            self.status_message = Some("no Facts book".to_string());
            return;
        };
        let passages = crate::book_rag::retrieval::retrieve(
            &self.store,
            &self.hierarchy,
            &self.cfg.book_rag,
            book_id,
            &response,
        )
        .unwrap_or_default();
        let hits: Vec<_> = passages
            .into_iter()
            .filter(|p| p.is_hit && !self.pinned_nodes.contains(&p.id))
            .take(diff_top_n)
            .collect();

        let mut out = String::new();
        if hits.is_empty() {
            out.push_str("No similar facts in your corpus yet.");
        } else {
            for p in &hits {
                let excerpt: String = p.body.chars().take(160).collect();
                out.push_str(&format!("{:.2}  {}\n      {}\n\n", p.score, p.breadcrumb, excerpt.trim()));
            }
            out.push_str("─────\nUse /fact to add a new entry, or /goto <path> to open an existing one.");
        }
        self.chat_history.push(ChatTurn::with_response(
            format!("[/diff — top {diff_top_n} similar facts]"),
            out,
        ));
        self.chat_scroll = 0;
    }

    /// R-P14 — `/verify`: probe the model's confidence in the specific claims of
    /// the last response. Extracts checkable claims, then asks for a HIGH /
    /// MEDIUM / LOW assessment of each.
    fn run_verify(&mut self) {
        let min_words = self.cfg.research.verify_min_sentence_words.max(1);
        if self.verify_rx.is_some() {
            self.status_message = Some("a verification is already running".to_string());
            return;
        }
        let Some(response) = self.chat_history.last().map(|t| t.response.clone()) else {
            self.status_message = Some("no research response to verify".to_string());
            return;
        };
        let claims = super::verify::extract_claims(&response, min_words);
        if claims.is_empty() {
            self.status_message = Some("no specific claims found to verify".to_string());
            return;
        }
        let ai = match crate::ai::AiClient::from_config(&self.cfg.llm) {
            Ok(a) => a,
            Err(e) => {
                self.status_message = Some(format!("no LLM provider: {e}"));
                return;
            }
        };
        let (model, _env) = match ai.resolve_provider(&self.cfg.llm, None) {
            Ok(m) => m,
            Err(e) => {
                self.status_message = Some(format!("provider error: {e}"));
                return;
            }
        };
        let rx = spawn_chat_stream(
            ai.client.clone(),
            model.to_string(),
            Some(super::verify::PROBE_SYSTEM.to_string()),
            Vec::new(),
            super::verify::probe_user(&claims),
            llm::CATEGORY,
        );
        self.verify_rx = Some((rx, String::new()));
        self.status_message = Some(format!("Verifying {} claim(s)…", claims.len()));
    }

    /// Drain the `/verify` probe; on completion, render the verdicts (LOW marked
    /// with ⚠).
    fn poll_verify(&mut self) {
        let Some((rx, buf)) = self.verify_rx.as_mut() else { return };
        let mut done = false;
        loop {
            match rx.try_recv() {
                Ok(StreamMsg::Token(t)) => buf.push_str(&t),
                Ok(StreamMsg::Done(_)) | Err(mpsc::error::TryRecvError::Disconnected) => {
                    done = true;
                    break;
                }
                Ok(StreamMsg::Error(e)) => {
                    self.status_message = Some(format!("verify error: {e}"));
                    self.verify_rx = None;
                    return;
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
            }
        }
        if done {
            let (_, buf) = self.verify_rx.take().unwrap();
            let mut out = String::new();
            for line in buf.lines() {
                match super::verify::Confidence::parse(line) {
                    Some(super::verify::Confidence::Low) => out.push_str(&format!("⚠ {}\n", line.trim())),
                    Some(_) => out.push_str(&format!("{}\n", line.trim())),
                    None => {
                        if !line.trim().is_empty() {
                            out.push_str(&format!("{}\n", line.trim()));
                        }
                    }
                }
            }
            self.chat_history.push(ChatTurn::with_response("[/verify — claim confidence]".to_string(), out));
            self.chat_scroll = 0;
            self.status_message = None;
        }
    }

    /// The id of a system book by tag.
    fn system_book_id(&self, tag: &str) -> Option<Uuid> {
        self.hierarchy
            .children_of(None)
            .into_iter()
            .find(|n| {
                n.kind == crate::store::NodeKind::Book && n.system_tag.as_deref() == Some(tag)
            })
            .map(|n| n.id)
    }

    /// R-P10/R-P11 — start a `/fact` or `/note` extraction over the last
    /// research response. The result lands in the confirmation overlay.
    /// `/fact` / `/note` over the last research response.
    fn start_extraction(
        &mut self,
        book: TargetBook,
        prompt: Option<String>,
        path: Option<String>,
        command_name: &str,
    ) {
        let Some(last) = self.chat_history.last() else {
            self.status_message = Some("no research response yet — ask a question first".to_string());
            return;
        };
        let research = last.response.clone();
        let query = last.prompt.clone();
        // R3-B — a paper `/fact` queues a SOURCES-1 citation (written on confirm).
        let cite = last
            .paper
            .as_ref()
            .filter(|_| self.cfg.research.scholarly.auto_cite)
            .map(|p| p.to_bibentry());
        // Provenance: `web` when the answer was grounded on live web results
        // (R2-C, fact-checked before commit); `document` for imported sources
        // (R2-B); else `model`, from the originating research query.
        let prov = if last.computed {
            // R4-D — carry the `world:<path>` citation when the calc read World facts.
            ProvMeta { origin: "computed".to_string(), query, detail: last.world_detail.clone() }
        } else if last.simulation {
            // R3-C — the project's own deterministic World simulation; gate skipped.
            ProvMeta { origin: "simulation".to_string(), query, detail: last.sources.join(", ") }
        } else if let Some(qid) = last.wikidata.clone() {
            // R3-A — structured Wikidata triple, cited by Q-ID; gate skipped.
            ProvMeta { origin: "wikidata".to_string(), query, detail: qid }
        } else if let Some(paper) = &last.paper {
            // R3-B — scholarly paper; gate skipped, auto-cites to Sources on insert.
            ProvMeta { origin: paper.source.to_string(), query, detail: paper.cite_detail() }
        } else if last.web_grounded {
            ProvMeta { origin: "web".to_string(), query, detail: last.sources.join(", ") }
        } else if last.sources.is_empty() {
            ProvMeta { origin: "model".to_string(), query, detail: String::new() }
        } else {
            ProvMeta { origin: "document".to_string(), query, detail: last.sources.join(", ") }
        };
        self.pending_cite = cite;
        self.start_extraction_from(book, research, prompt, path, command_name, prov);
    }

    /// The shared extraction launcher: distil `research` into one entry for
    /// `book`, carrying `prov` through to the insert (T-P2).
    fn start_extraction_from(
        &mut self,
        book: TargetBook,
        research: String,
        prompt: Option<String>,
        path: Option<String>,
        command_name: &str,
        prov: ProvMeta,
    ) {
        if self.extracting.is_some() || self.confirmation.is_some() {
            self.status_message = Some("finish the current extraction first".to_string());
            return;
        }
        if research.trim().is_empty() {
            self.status_message = Some("nothing to extract (empty source)".to_string());
            return;
        }

        // Resolve the target book + insertion node.
        let Some(book_id) = self.system_book_id(book.system_tag()) else {
            self.status_message = Some(format!("no {} book in this project", book.label()));
            return;
        };
        let target = self.resolve_insertion_target(book, path.as_deref());

        let ai = match crate::ai::AiClient::from_config(&self.cfg.llm) {
            Ok(a) => a,
            Err(e) => {
                self.status_message = Some(format!("no LLM provider: {e}"));
                return;
            }
        };
        let (model, _env) = match ai.resolve_provider(&self.cfg.llm, None) {
            Ok(m) => m,
            Err(e) => {
                self.status_message = Some(format!("provider error: {e}"));
                return;
            }
        };

        let instruction = prompt.unwrap_or_else(|| extract::default_instruction(book).to_string());
        // Key the extraction off the project language so it never drops to
        // English when the research / corpus is in another language.
        let (lang, _note) = crate::prose::resolve_prose_language(None, &self.cfg.language);
        let language = extract::language_name(&lang);
        let system = extract::system_prompt(book, language, &instruction, &research);
        let rx = spawn_chat_stream(
            ai.client.clone(),
            model.to_string(),
            Some(system),
            Vec::new(),
            "Produce the entry as specified.".to_string(),
            llm::CATEGORY,
        );
        // R2-E — a transient streaming turn so the extraction's tokens show live
        // (dim) instead of a silent status line; popped when the overlay opens.
        let command = format!("{command_name} \"{instruction}\"");
        let mut preview = ChatTurn::new(format!("{command}  (extracting {}…)", book.label()));
        preview.streaming = true;
        self.chat_history.push(preview);
        self.chat_scroll = 0;
        let preview_idx = Some(self.chat_history.len() - 1);
        self.extracting = Some(ExtractState {
            rx,
            buf: String::new(),
            book,
            book_id,
            target,
            command,
            prov,
            preview_idx,
        });
        self.status_message = Some(format!("Extracting {}…", book.label()));
    }

    /// R2-E — drop a transient streaming preview turn (pop it if it's still the
    /// last turn; otherwise just stop its streaming marker).
    fn drop_preview(&mut self, idx: Option<usize>) {
        let Some(i) = idx else { return };
        if i + 1 == self.chat_history.len() {
            self.chat_history.pop();
        } else if let Some(t) = self.chat_history.get_mut(i) {
            t.streaming = false;
        }
    }

    /// Resolve where an extraction inserts: an explicit `→ path` (resolved
    /// against the whole hierarchy), else the Facts cursor (Facts only), else
    /// the book root.
    fn resolve_insertion_target(&self, book: TargetBook, path: Option<&str>) -> Option<Uuid> {
        if let Some(p) = path {
            // Accept an optional leading `facts/` or `notes/`.
            let trimmed = p.trim().trim_start_matches('/');
            if let Some(node) = self.hierarchy.find_by_path(trimmed) {
                return Some(node.id);
            }
            let stripped = trimmed
                .strip_prefix("facts/")
                .or_else(|| trimmed.strip_prefix("notes/"))
                .unwrap_or(trimmed);
            if let Some(node) = self.hierarchy.find_by_path(stripped) {
                return Some(node.id);
            }
            return None;
        }
        match book {
            TargetBook::Facts => self.facts_tree.selected(),
            TargetBook::Notes => None,
        }
    }

    /// Drain the extraction stream; on completion, open the confirmation overlay.
    fn poll_extraction(&mut self) {
        let Some(ex) = self.extracting.as_mut() else { return };
        let preview_idx = ex.preview_idx;
        let mut done = false;
        let mut new_tokens = String::new();
        loop {
            match ex.rx.try_recv() {
                Ok(StreamMsg::Token(t)) => {
                    new_tokens.push_str(&t);
                    ex.buf.push_str(&t);
                }
                Ok(StreamMsg::Done(_)) | Err(mpsc::error::TryRecvError::Disconnected) => {
                    done = true;
                    break;
                }
                Ok(StreamMsg::Error(e)) => {
                    self.drop_preview(preview_idx);
                    self.status_message = Some(format!("extraction error: {e}"));
                    self.extracting = None;
                    return;
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
            }
        }
        // Mirror the just-arrived tokens into the live preview turn (dim).
        if !new_tokens.is_empty() {
            if let Some(i) = preview_idx {
                if let Some(turn) = self.chat_history.get_mut(i) {
                    turn.response.push_str(&new_tokens);
                }
            }
        }
        if done {
            self.drop_preview(preview_idx);
            let ex = self.extracting.take().unwrap();
            let parsed = extract::parse(&ex.buf);
            let mut title = TextArea::default();
            title.insert_str(&parsed.title);
            let mut body = TextArea::default();
            body.insert_str(&parsed.text);
            self.confirmation = Some(ConfirmationState {
                title,
                body,
                book: ex.book,
                book_id: ex.book_id,
                target: ex.target,
                field: ConfirmField::Title,
                command: ex.command,
                prov: ex.prov,
                dup_warning: None,
                dup_body: None,
                fc_checked: false,
                fc_verdict: None,
                fc_detail: None,
            });
            self.focus = Focus::ConfirmationOverlay;
            self.status_message = None;
        }
    }

    /// G1/G2 — keys for the confirmation overlay. Tab switches field; Ctrl+Enter
    /// (or Ctrl+S) confirms; Esc discards.
    fn confirmation_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && matches!(key.code, KeyCode::Enter | KeyCode::Char('s')) {
            self.confirm_insertion();
            return;
        }
        match key.code {
            KeyCode::Esc => {
                self.confirmation = None;
                self.focus = Focus::QueryPrompt;
                self.status_message = Some("discarded".to_string());
            }
            KeyCode::Tab => {
                if let Some(c) = self.confirmation.as_mut() {
                    c.field = match c.field {
                        ConfirmField::Title => ConfirmField::Body,
                        ConfirmField::Body => ConfirmField::Title,
                    };
                }
            }
            // Arrow / Home / End cursor movement on the focused field
            // (`input_without_shortcuts` doesn't perform cursor moves).
            KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End | KeyCode::Up | KeyCode::Down => {
                use tui_textarea::CursorMove;
                let mv = match key.code {
                    KeyCode::Left => CursorMove::Back,
                    KeyCode::Right => CursorMove::Forward,
                    KeyCode::Home => CursorMove::Head,
                    KeyCode::End => CursorMove::End,
                    KeyCode::Up => CursorMove::Up,
                    _ => CursorMove::Down,
                };
                if let Some(c) = self.confirmation.as_mut() {
                    match c.field {
                        ConfirmField::Title => c.title.move_cursor(mv),
                        ConfirmField::Body => c.body.move_cursor(mv),
                    }
                }
            }
            _ => {
                if let Some(c) = self.confirmation.as_mut() {
                    let input: tui_textarea::Input = key.into();
                    match c.field {
                        ConfirmField::Title => {
                            // Single-line title — swallow Enter.
                            if key.code != KeyCode::Enter {
                                c.title.input_without_shortcuts(input);
                            }
                        }
                        ConfirmField::Body => {
                            c.body.input_without_shortcuts(input);
                        }
                    }
                }
            }
        }
    }

    /// Commit the confirmed entry into its book, then reload + persist the turn.
    fn confirm_insertion(&mut self) {
        // Peek the fields first: refuse an empty body (the extraction may have
        // produced no `fact`), keeping the editable overlay open rather than
        // inserting a paragraph that is just the seeded `= title` heading.
        let (title, body) = match &self.confirmation {
            Some(c) => (c.title.lines().join(" "), c.body.lines().join("\n")),
            None => return,
        };
        if body.trim().is_empty() {
            self.status_message =
                Some("fact body is empty — type it, or Esc to cancel".to_string());
            return;
        }
        // T-P4 — near-duplicate guard (Facts only): warn once, insert on a
        // second confirm. Skip if a warning is already showing (the second press).
        let already_warned = self.confirmation.as_ref().is_some_and(|c| c.dup_warning.is_some());
        let is_facts = self.confirmation.as_ref().is_some_and(|c| c.book == TargetBook::Facts);
        if is_facts && !already_warned {
            if let Some((dup, dup_body)) = self.find_near_duplicate(&body) {
                if let Some(c) = self.confirmation.as_mut() {
                    c.dup_warning =
                        Some(format!("similar to {dup} · Ctrl+S again to insert anyway"));
                    c.dup_body = Some(dup_body);
                }
                self.status_message = Some(format!("near-duplicate of {dup}"));
                return;
            }
        }
        // The pre-commit gate. A `model` / `web` / `document` fact needs checking;
        // `computed` / `wikidata` / `openalex` / `arxiv` are already authoritative
        // (structured / deterministic) and skip it. When `triangulate_gate` is on
        // and a structured source is available, cross-source triangulation (R3-E)
        // is the gate; otherwise the single-source web check (WC-P3). Both run
        // async (spawn + poll-drain) and require a second confirm on a weak verdict.
        let origin = self.confirmation.as_ref().map(|c| c.prov.origin.clone()).unwrap_or_default();
        let needs_gate = matches!(origin.as_str(), "model" | "web" | "document");
        let fc_done = self.confirmation.as_ref().is_some_and(|c| c.fc_checked);
        if needs_gate && !fc_done {
            let structured_available = super::wikidata::available(&self.cfg.research.wikidata)
                || super::scholarly::available(&self.cfg.research.scholarly);
            if self.cfg.research.triangulate_gate && structured_available {
                if self.tri_gate.is_none() {
                    self.start_triangulate_gate(&title, &body);
                }
                return; // wait for the triangulation verdict (poll_tri_gate finalises)
            }
            if origin == "web" {
                if self.fc_confirm.is_none() {
                    self.start_fact_check(&body);
                }
                return; // wait for the verdict (poll_fc_confirm finalises)
            }
        }
        let c = self.confirmation.take().unwrap();
        match super::insert::insert_paragraph(
            &self.store,
            &self.cfg,
            &self.hierarchy,
            c.book_id,
            c.target,
            &title,
            &body,
        ) {
            Ok(new_id) => {
                self.reload_hierarchy();
                if c.book == TargetBook::Facts {
                    let _ = self.facts_tree.reveal(&self.hierarchy, new_id);
                }
                let path = self
                    .hierarchy
                    .get(new_id)
                    .map(|n| self.hierarchy.slug_path(n))
                    .unwrap_or_default();
                let kind = match c.book {
                    TargetBook::Facts => TurnKind::FactInsertion,
                    TargetBook::Notes => TurnKind::NoteInsertion,
                };
                let now = chrono::Utc::now().to_rfc3339();
                let id = Uuid::now_v7().to_string();
                let _ = self.thread.push_turn(
                    ResearchTurn::insertion(
                        id,
                        kind,
                        c.command,
                        title.trim().to_string(),
                        body,
                        path.clone(),
                        c.book.label().to_string(),
                        now,
                    ),
                    &self.layout,
                );
                // T-P2 — record provenance for Facts inserts.
                if c.book == TargetBook::Facts {
                    let now2 = chrono::Utc::now().to_rfc3339();
                    super::provenance::Provenance::record(
                        &self.layout,
                        &new_id.to_string(),
                        super::provenance::SourceRecord::new(
                            &c.prov.origin,
                            &c.prov.detail,
                            &c.prov.query,
                            &self.thread.name,
                            now2,
                        ),
                    );
                }
                self.rebuild_prompt_history();
                // R3-B — auto-create the SOURCES-1 citation for a scholarly `/fact`.
                let mut cite_note = String::new();
                if matches!(c.prov.origin.as_str(), "openalex" | "arxiv") {
                    if let Some(entry) = self.pending_cite.take() {
                        match self.add_bibentry(&entry) {
                            Ok(true) => {
                                self.reload_hierarchy();
                                cite_note = format!("  · cited `{}` → Sources", entry.key);
                            }
                            Ok(false) => cite_note = "  · citation already in Sources".to_string(),
                            Err(e) => cite_note = format!("  · citation failed: {e}"),
                        }
                    }
                }
                self.status_message =
                    Some(format!("✓ Inserted: '{}' → {path}{cite_note}", title.trim()));
            }
            Err(e) => self.status_message = Some(format!("insert failed: {e}")),
        }
        self.focus = Focus::QueryPrompt;
    }

    /// T-P4 — the slug-path of the nearest existing Facts paragraph whose body is
    /// a near-duplicate of `body` (score ≥ the configured threshold), if any.
    /// The nearest near-duplicate fact (its breadcrumb + body), if any is at or
    /// above the dedup threshold. UX-P4 shows the body beside the pending fact.
    fn find_near_duplicate(&self, body: &str) -> Option<(String, String)> {
        let book_id = self.facts_tree.root?;
        let threshold = self.cfg.research.dedup_warn_score;
        let passages = crate::book_rag::retrieval::retrieve(
            &self.store,
            &self.hierarchy,
            &self.cfg.book_rag,
            book_id,
            body,
        )
        .ok()?;
        passages
            .into_iter()
            .find(|p| p.is_hit && p.score >= threshold)
            .map(|p| (p.breadcrumb, p.body))
    }

    /// WC-P3 — spawn the single-fact truth check for the pending web fact.
    fn start_fact_check(&mut self, body: &str) {
        let ai = match crate::ai::AiClient::from_config(&self.cfg.llm) {
            Ok(a) => a,
            Err(_) => {
                // No provider — skip the gate (mark checked so the next confirm inserts).
                if let Some(c) = self.confirmation.as_mut() {
                    c.fc_checked = true;
                    c.fc_verdict = Some("unchecked (no LLM provider)".to_string());
                }
                self.status_message = Some("fact-check skipped — Ctrl+S to insert".to_string());
                return;
            }
        };
        let (model, _env) = match ai.resolve_provider(&self.cfg.llm, None) {
            Ok(m) => m,
            Err(_) => {
                if let Some(c) = self.confirmation.as_mut() {
                    c.fc_checked = true;
                }
                return;
            }
        };
        let (lang, _note) = crate::prose::resolve_prose_language(None, &self.cfg.language);
        let language = super::extract::language_name(&lang);
        let system = super::factcheck::truth_system(language);
        let user = format!("Statements:\n1. {}\n", body.trim());
        let rx = spawn_chat_stream(ai.client.clone(), model.to_string(), Some(system), Vec::new(), user, llm::CATEGORY);
        self.fc_confirm = Some((rx, String::new()));
        self.status_message = Some("fact-checking before commit…".to_string());
    }

    /// Drain the pre-commit fact-check; ACCURATE auto-inserts, otherwise show the
    /// verdict and wait for a second confirm.
    fn poll_fc_confirm(&mut self) {
        let Some((rx, buf)) = self.fc_confirm.as_mut() else { return };
        let mut done = false;
        loop {
            match rx.try_recv() {
                Ok(StreamMsg::Token(t)) => buf.push_str(&t),
                Ok(StreamMsg::Done(_)) | Err(mpsc::error::TryRecvError::Disconnected) => {
                    done = true;
                    break;
                }
                Ok(StreamMsg::Error(_)) => {
                    done = true;
                    break;
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
            }
        }
        if !done {
            return;
        }
        let (_, buf) = self.fc_confirm.take().unwrap();
        // First non-empty verdict line, stripped of the leading "1.".
        let verdict = buf
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .map(|l| l.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ' ').to_string())
            .unwrap_or_else(|| "no verdict".to_string());
        let accurate = verdict.to_ascii_uppercase().starts_with("ACCURATE");
        let detail = buf.trim().to_string();
        if let Some(c) = self.confirmation.as_mut() {
            c.fc_checked = true;
            c.fc_verdict = Some(verdict.clone());
            c.fc_detail = Some(detail);
            // Fold the verdict into the provenance detail.
            if c.prov.detail.is_empty() {
                c.prov.detail = format!("fact-check: {verdict}");
            } else {
                c.prov.detail = format!("{} · fact-check: {verdict}", c.prov.detail);
            }
        }
        if accurate {
            self.confirm_insertion(); // gate now skipped → inserts
        } else {
            self.status_message =
                Some(format!("fact-check: {verdict} — Ctrl+S again to insert anyway"));
        }
    }

    /// R3-E — start the triangulation gate for a pending `/fact`: gather evidence
    /// from the structured sources (phase 1); `poll_tri_gate` runs the judge.
    fn start_triangulate_gate(&mut self, title: &str, body: &str) {
        let claim = if title.trim().is_empty() {
            body.trim().to_string()
        } else {
            format!("{}. {}", title.trim(), body.trim())
        };
        let wd_cfg = self.cfg.research.wikidata.clone();
        let sc_cfg = self.cfg.research.scholarly.clone();
        let wd_on = super::wikidata::available(&wd_cfg);
        let sc_on = super::scholarly::available(&sc_cfg);
        let (lang, _note) = crate::prose::resolve_prose_language(None, &self.cfg.language);
        let code = match lang.as_code() {
            "other" => "en".to_string(),
            c => c.to_string(),
        };
        let q = claim.clone();
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut evidence: Vec<(String, String)> = Vec::new();
            if wd_on {
                if let Ok(e) = super::wikidata::fetch(wd_cfg, q.clone(), code).await {
                    evidence.push(("Wikidata".to_string(), super::wikidata::render(&e)));
                }
            }
            if sc_on {
                if let Ok(p) = super::scholarly::openalex(sc_cfg.clone(), q.clone()).await {
                    evidence.push(("OpenAlex".to_string(), super::scholarly::render(&p)));
                }
                if let Ok(p) = super::scholarly::arxiv(q.clone()).await {
                    evidence.push(("arXiv".to_string(), super::scholarly::render(&p)));
                }
            }
            let _ = tx.send(evidence);
        });
        self.tri_gate = Some(TriGate { phase: TriGatePhase::Gather(rx), claim });
        self.status_message = Some("triangulating before commit…".to_string());
    }

    /// R3-E — drive the triangulation gate: gather → judge → verdict. A verdict
    /// with support and no contradiction auto-inserts; otherwise it asks for a
    /// second confirm (like the web / dedup gates).
    fn poll_tri_gate(&mut self) {
        let Some(gate) = self.tri_gate.as_mut() else { return };
        match &mut gate.phase {
            TriGatePhase::Gather(rx) => {
                let evidence = match rx.try_recv() {
                    Ok(e) => e,
                    Err(mpsc::error::TryRecvError::Empty) => return,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        self.tri_gate = None;
                        return;
                    }
                };
                if evidence.is_empty() {
                    // Nothing to corroborate against — pass through, but say so.
                    self.tri_gate = None;
                    if let Some(c) = self.confirmation.as_mut() {
                        c.fc_checked = true;
                        c.fc_verdict = Some("triangulation: no corroborating sources".to_string());
                    }
                    self.status_message =
                        Some("triangulation: no sources found — Ctrl+S again to insert".to_string());
                    return;
                }
                let claim = gate.claim.clone();
                self.start_tri_gate_judge(&claim, &evidence);
            }
            TriGatePhase::Judge { rx, buf } => {
                let mut done = false;
                loop {
                    match rx.try_recv() {
                        Ok(StreamMsg::Token(t)) => buf.push_str(&t),
                        Ok(StreamMsg::Done(_)) | Err(mpsc::error::TryRecvError::Disconnected) => {
                            done = true;
                            break;
                        }
                        Ok(StreamMsg::Error(_)) => {
                            done = true;
                            break;
                        }
                        Err(mpsc::error::TryRecvError::Empty) => break,
                    }
                }
                if !done {
                    return;
                }
                let TriGate { phase, .. } = self.tri_gate.take().unwrap();
                let TriGatePhase::Judge { buf, .. } = phase else { return };
                let (verdict, pass) = summarize_triangulation(&buf);
                let detail = buf.trim().to_string();
                if let Some(c) = self.confirmation.as_mut() {
                    c.fc_checked = true;
                    c.fc_verdict = Some(verdict.clone());
                    c.fc_detail = Some(detail);
                    c.prov.detail = if c.prov.detail.is_empty() {
                        format!("triangulation: {verdict}")
                    } else {
                        format!("{} · triangulation: {verdict}", c.prov.detail)
                    };
                }
                if pass {
                    self.confirm_insertion(); // gate satisfied → inserts
                } else {
                    self.status_message =
                        Some(format!("triangulation: {verdict} — Ctrl+S again to insert anyway"));
                }
            }
        }
    }

    /// Spawn the triangulation-gate judge LLM call over the gathered evidence.
    fn start_tri_gate_judge(&mut self, claim: &str, evidence: &[(String, String)]) {
        let ai = match crate::ai::AiClient::from_config(&self.cfg.llm) {
            Ok(a) => a,
            Err(_) => {
                self.tri_gate = None;
                if let Some(c) = self.confirmation.as_mut() {
                    c.fc_checked = true;
                    c.fc_verdict = Some("triangulation skipped (no LLM provider)".to_string());
                }
                self.status_message = Some("triangulation skipped — Ctrl+S to insert".to_string());
                return;
            }
        };
        let (model, _env) = match ai.resolve_provider(&self.cfg.llm, None) {
            Ok(m) => m,
            Err(_) => {
                self.tri_gate = None;
                if let Some(c) = self.confirmation.as_mut() {
                    c.fc_checked = true;
                }
                return;
            }
        };
        let (lang, _note) = crate::prose::resolve_prose_language(None, &self.cfg.language);
        let language = super::extract::language_name(&lang);
        let mut ev = String::new();
        for (label, text) in evidence {
            let body: String = text.chars().take(1200).collect();
            ev.push_str(&format!("[{label}]\n{}\n\n", body.trim()));
        }
        let system = format!(
            "You are triangulating a claim against INDEPENDENT sources before it enters a knowledge base. \
             Judge ONLY from the sources below — do not use outside knowledge. For EACH source, output one \
             line:\n<source>: SUPPORTS | CONTRADICTS | SILENT — <short reason>\n\
             Then a final line: `Agreement: <n>/<m> support`. Write in {language}."
        );
        let user = format!("Claim:\n{claim}\n\nSources:\n{ev}");
        let rx = spawn_chat_stream(
            ai.client.clone(),
            model.to_string(),
            Some(system),
            Vec::new(),
            user,
            llm::CATEGORY,
        );
        self.tri_gate = Some(TriGate {
            phase: TriGatePhase::Judge { rx, buf: String::new() },
            claim: claim.to_string(),
        });
        self.status_message =
            Some(format!("judging {} source(s) before commit…", evidence.len()));
    }

    fn rebuild_prompt_history(&mut self) {
        self.prompt_history = build_prompt_history(&self.thread);
        // Also include this session's live chat prompts (newest first).
        for turn in self.chat_history.iter().rev() {
            if !self.prompt_history.iter().any(|p| p == &turn.prompt) {
                self.prompt_history.insert(0, turn.prompt.clone());
            }
        }
    }

    fn history_back(&mut self) {
        if self.prompt_history.is_empty() {
            return;
        }
        let next = match self.prompt_history_idx {
            None => {
                self.draft_backup = self.query_text();
                0
            }
            Some(i) => (i + 1).min(self.prompt_history.len() - 1),
        };
        self.prompt_history_idx = Some(next);
        let text = self.prompt_history[next].clone();
        self.set_query_text(&text);
    }

    fn history_forward(&mut self) {
        match self.prompt_history_idx {
            Some(0) | None => {
                // Past the newest → restore the draft.
                self.prompt_history_idx = None;
                let draft = self.draft_backup.clone();
                self.set_query_text(&draft);
            }
            Some(i) => {
                let idx = i - 1;
                self.prompt_history_idx = Some(idx);
                let text = self.prompt_history[idx].clone();
                self.set_query_text(&text);
            }
        }
    }

    /// Facts-tree navigation + pin + manual-entry trigger (G4 / G7). Returns
    /// whether the key was consumed.
    fn facts_tree_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.facts_tree.move_up(),
            KeyCode::Down | KeyCode::Char('j') => self.facts_tree.move_down(),
            KeyCode::Char('g') => self.facts_tree.to_top(),
            KeyCode::Char('G') => self.facts_tree.to_bottom(),
            KeyCode::Right | KeyCode::Char('l') => self.facts_tree.step_in(&self.hierarchy),
            // Enter — quick-view a fact (leaf) or fold/expand a branch (UX-P5).
            KeyCode::Enter => self.peek_selected(),
            KeyCode::Left | KeyCode::Char('h') => self.facts_tree.step_out(&self.hierarchy),
            // UX-P5 — copy the selected fact's body to the system clipboard.
            KeyCode::Char('Y') => self.yank_selected_body(),
            KeyCode::Char('n') => {
                self.manual = Some(ManualEntry {
                    stage: ManualStage::Title,
                    title: String::new(),
                    body: String::new(),
                });
            }
            // Ctrl+P pins / unpins the cursor node (G4).
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => self.toggle_pin(),
            // Tree-edit operations (on par with the editor's Tree pane).
            KeyCode::Char('R') => self.tree_rename_start(),
            KeyCode::Char('c') => self.tree_add_start(TreeInputKind::NewChapter),
            KeyCode::Char('s') => self.tree_add_start(TreeInputKind::NewSubchapter),
            KeyCode::Char('-') => self.tree_delete_start(false),
            KeyCode::Char('D') => self.tree_delete_start(true),
            KeyCode::Char('K') => self.tree_move(true),
            KeyCode::Char('J') => self.tree_move(false),
            // Copy/move across parents (Outline parity): y yank-copy, x cut-move, p paste.
            KeyCode::Char('y') => self.tree_yank(ClipMode::Copy),
            KeyCode::Char('x') => self.tree_yank(ClipMode::Move),
            KeyCode::Char('p') => self.tree_paste(),
            // RESRCH-UNDISPUTED — toggle the `fact:undisputed` authorial tag.
            KeyCode::Char('u') => self.toggle_undisputed(),
            // UX-P5 — resize the split (`>` widens the tree, `<` narrows it).
            KeyCode::Char('>') => self.resize_split(true),
            KeyCode::Char('<') => self.resize_split(false),
            _ => return false,
        }
        true
    }

    /// G4 — pin / unpin the cursor node (max `max_pinned`, persisted on the thread).
    fn toggle_pin(&mut self) {
        let Some(id) = self.facts_tree.selected() else { return };
        if let Some(pos) = self.pinned_nodes.iter().position(|p| *p == id) {
            self.pinned_nodes.remove(pos);
            self.status_message = Some("unpinned".to_string());
        } else if self.pinned_nodes.len() >= self.max_pinned() {
            self.status_message =
                Some(format!("Max {} nodes pinned — unpin one first", self.max_pinned()));
            return;
        } else {
            self.pinned_nodes.push(id);
            self.status_message = Some(format!("pinned ({}/{})", self.pinned_nodes.len(), self.max_pinned()));
        }
        self.persist_pins();
    }

    /// RESRCH-UNDISPUTED — toggle the `fact:undisputed` tag on the selected fact.
    /// An undisputed fact is the author's creative invention: glyphed `※` in the
    /// tree, excluded from `/factcheck`, and checked instead by `/undisputed`.
    fn toggle_undisputed(&mut self) {
        let Some(id) = self.facts_tree.selected() else { return };
        let Some(node) = self.hierarchy.get(id) else { return };
        if node.kind != crate::store::NodeKind::Paragraph {
            self.status_message = Some("select a fact (a paragraph) to mark undisputed".to_string());
            return;
        }
        let mut updated = node.clone();
        let now_marked = match updated.tags.iter().position(|t| t == super::UNDISPUTED_TAG) {
            Some(pos) => {
                updated.tags.remove(pos);
                false
            }
            None => {
                updated.tags.push(super::UNDISPUTED_TAG.to_string());
                true
            }
        };
        if let Err(e) = self.store.raw().update_metadata(id, updated.to_json()) {
            self.status_message = Some(format!("undisputed: tag write failed: {e}"));
            return;
        }
        self.reload_hierarchy();
        self.status_message = Some(if now_marked {
            "※ marked undisputed — excluded from /factcheck; check with /undisputed".to_string()
        } else {
            "unmarked — this fact is fact-checked normally again".to_string()
        });
    }

    /// Save the current pin set onto the thread.
    fn persist_pins(&mut self) {
        self.thread.pinned_nodes = self.pinned_nodes.iter().map(|u| u.to_string()).collect();
        let _ = self.thread.save(&self.layout);
    }

    /// UX-P5 — Enter on the cursor: quick-view a fact (a leaf paragraph) in a
    /// scrollable modal, or fold/expand a branch.
    fn peek_selected(&mut self) {
        let Some(id) = self.facts_tree.selected() else { return };
        let Some(node) = self.hierarchy.get(id) else { return };
        if node.kind != crate::store::NodeKind::Paragraph {
            self.facts_tree.step_in(&self.hierarchy);
            return;
        }
        let title = node.title.clone();
        let body = match self.store.get_content(id) {
            Ok(Some(b)) => String::from_utf8_lossy(&b).trim().to_string(),
            _ => String::new(),
        };
        self.peek = Some(PeekState { title, body, scroll: 0 });
    }

    /// UX-P5 — keys for the fact quick-view modal (scroll · Esc/Enter close).
    fn peek_key(&mut self, key: KeyEvent) {
        let Some(p) = self.peek.as_mut() else { return };
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => self.peek = None,
            KeyCode::Down | KeyCode::Char('j') => p.scroll = p.scroll.saturating_add(1),
            KeyCode::Up | KeyCode::Char('k') => p.scroll = p.scroll.saturating_sub(1),
            KeyCode::Char('g') => p.scroll = 0,
            KeyCode::Char('G') => p.scroll = u16::MAX, // clamped in render
            KeyCode::Char('Y') => {
                let body = p.body.clone();
                self.copy_to_clipboard(&body);
            }
            _ => {}
        }
    }

    /// UX-P5 — copy the selected fact's body to the system clipboard.
    fn yank_selected_body(&mut self) {
        let Some(id) = self.facts_tree.selected() else { return };
        let body = match self.store.get_content(id) {
            Ok(Some(b)) => String::from_utf8_lossy(&b).trim().to_string(),
            _ => String::new(),
        };
        if body.is_empty() {
            self.status_message = Some("nothing to copy".to_string());
            return;
        }
        self.copy_to_clipboard(&body);
    }

    /// UX-P5 — copy the last chat response to the system clipboard.
    fn yank_last_response(&mut self) {
        match self.chat_history.last().map(|t| t.response.clone()).filter(|r| !r.trim().is_empty()) {
            Some(text) => self.copy_to_clipboard(&text),
            None => self.status_message = Some("no response to copy".to_string()),
        }
    }

    /// UX-P5 — nudge the tree/chat split ratio (`>` widens the tree, `<` narrows).
    fn resize_split(&mut self, wider_tree: bool) {
        self.split_ratio = if wider_tree {
            (self.split_ratio + 1).min(9)
        } else {
            self.split_ratio.saturating_sub(1).max(1)
        };
    }

    /// Reuse the editor's `arboard` clipboard (already a dependency); degrades to
    /// a status note when no clipboard is available (e.g. headless / pure SSH).
    fn copy_to_clipboard(&mut self, text: &str) {
        let ok = arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text.to_string())).is_ok();
        self.status_message = Some(if ok {
            format!("copied {} char(s) to clipboard", text.chars().count())
        } else {
            "clipboard unavailable".to_string()
        });
    }

    /// G7 — keys for the inline manual fact-entry overlay (title → body).
    fn manual_entry_key(&mut self, key: KeyEvent) {
        let Some(m) = self.manual.as_mut() else { return };
        match m.stage {
            ManualStage::Title => match key.code {
                KeyCode::Esc => self.manual = None,
                KeyCode::Enter => {
                    if !m.title.trim().is_empty() {
                        m.stage = ManualStage::Body;
                    }
                }
                KeyCode::Backspace => {
                    m.title.pop();
                }
                KeyCode::Char(c) => m.title.push(c),
                _ => {}
            },
            ManualStage::Body => {
                // Ctrl+S saves; Esc discards; Enter inserts a newline.
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
                    self.save_manual_entry();
                    return;
                }
                match key.code {
                    KeyCode::Esc => self.manual = None,
                    KeyCode::Enter => m.body.push('\n'),
                    KeyCode::Backspace => {
                        m.body.pop();
                    }
                    KeyCode::Char(c) => m.body.push(c),
                    _ => {}
                }
            }
        }
    }

    /// Insert the manual entry as a Facts paragraph, then reload + rebuild.
    fn save_manual_entry(&mut self) {
        let Some(m) = self.manual.take() else { return };
        let Some(book_id) = self.facts_tree.root else {
            self.status_message = Some("no Facts book to insert into".to_string());
            return;
        };
        let target = self.facts_tree.selected();
        match super::insert::insert_paragraph(
            &self.store,
            &self.cfg,
            &self.hierarchy,
            book_id,
            target,
            &m.title,
            &m.body,
        ) {
            Ok(new_id) => {
                self.reload_hierarchy();
                let _ = self.facts_tree.reveal(&self.hierarchy, new_id);
                // T-P2 — manual-entry provenance.
                super::provenance::Provenance::record(
                    &self.layout,
                    &new_id.to_string(),
                    super::provenance::SourceRecord::new(
                        "manual",
                        "",
                        "",
                        &self.thread.name,
                        chrono::Utc::now().to_rfc3339(),
                    ),
                );
                self.status_message = Some(format!("✓ added fact: {}", m.title.trim()));
            }
            Err(e) => self.status_message = Some(format!("insert failed: {e}")),
        }
    }

    /// Reload the hierarchy after a store mutation and rebuild the Facts tree.
    pub(super) fn reload_hierarchy(&mut self) {
        // UX-P2 — refresh provenance so a newly-inserted fact shows its tier glyph.
        self.fact_provenance = super::provenance::Provenance::load(&self.layout);
        if let Ok(h) = Hierarchy::load(&self.store) {
            self.hierarchy = h;
            self.facts_tree.rebuild(&self.hierarchy);
        }
    }

    // ── tree-edit operations (#5 — on par with the editor's Tree pane) ──────────

    /// `R` — rename the cursor node (prefilled inline input).
    fn tree_rename_start(&mut self) {
        let Some(id) = self.facts_tree.selected() else { return };
        let title = self.hierarchy.get(id).map(|n| n.title.clone()).unwrap_or_default();
        self.tree_input = Some(TreeInput { kind: TreeInputKind::Rename, buf: title, target: id });
    }

    /// `c` / `s` — add a chapter (under the Facts book) or a subchapter (under the
    /// cursor's enclosing chapter). Opens a title input.
    fn tree_add_start(&mut self, kind: TreeInputKind) {
        let parent = match kind {
            TreeInputKind::NewChapter => self.facts_tree.root,
            TreeInputKind::NewSubchapter => self.enclosing_chapter(),
            TreeInputKind::Rename => return,
        };
        match parent {
            Some(p) => self.tree_input = Some(TreeInput { kind, buf: String::new(), target: p }),
            None => {
                self.status_message = Some("no chapter to nest under — add a chapter first".to_string())
            }
        }
    }

    /// The cursor's enclosing Chapter id (walking up), if any.
    fn enclosing_chapter(&self) -> Option<Uuid> {
        use crate::store::NodeKind;
        let mut cur = self.facts_tree.selected();
        while let Some(id) = cur {
            let node = self.hierarchy.get(id)?;
            if node.kind == NodeKind::Chapter {
                return Some(id);
            }
            cur = node.parent_id;
        }
        None
    }

    /// Keys for the tree-edit input overlay.
    fn tree_input_key(&mut self, key: KeyEvent) {
        let Some(ti) = self.tree_input.as_mut() else { return };
        match key.code {
            KeyCode::Esc => self.tree_input = None,
            KeyCode::Enter => {
                let name = ti.buf.trim().to_string();
                if name.is_empty() {
                    self.tree_input = None;
                    return;
                }
                let (kind, target) = (ti.kind, ti.target);
                self.tree_input = None;
                self.tree_input_commit(kind, target, &name);
            }
            KeyCode::Backspace => {
                ti.buf.pop();
            }
            KeyCode::Char(c) => ti.buf.push(c),
            _ => {}
        }
    }

    /// Apply a rename / add-branch operation, then reload + reveal.
    fn tree_input_commit(&mut self, kind: TreeInputKind, target: Uuid, name: &str) {
        use crate::store::{InsertPosition, NodeKind};
        let result: Result<Option<Uuid>, String> = match kind {
            TreeInputKind::Rename => self
                .store
                .rename_node(&self.hierarchy, target, name)
                .map(|_| Some(target))
                .map_err(|e| e.to_string()),
            TreeInputKind::NewChapter | TreeInputKind::NewSubchapter => {
                let nk = if kind == TreeInputKind::NewChapter {
                    NodeKind::Chapter
                } else {
                    NodeKind::Subchapter
                };
                let parent = self.hierarchy.get(target).cloned();
                match parent {
                    Some(p) => self
                        .store
                        .create_node(&self.cfg, &self.hierarchy, nk, name, Some(&p), None, InsertPosition::End)
                        .map(|n| Some(n.id))
                        .map_err(|e| e.to_string()),
                    None => Err("parent missing".to_string()),
                }
            }
        };
        match result {
            Ok(new_id) => {
                self.reload_hierarchy();
                if let Some(id) = new_id {
                    let _ = self.facts_tree.reveal(&self.hierarchy, id);
                }
                self.status_message = Some(format!("{} ✓", kind.label()));
            }
            Err(e) => self.status_message = Some(format!("{}: {e}", kind.label())),
        }
    }

    /// `-` (paragraph) / `D` (branch) — begin a delete (asks for confirmation).
    fn tree_delete_start(&mut self, branch: bool) {
        use crate::store::NodeKind;
        let Some(id) = self.facts_tree.selected() else { return };
        let Some(node) = self.hierarchy.get(id) else { return };
        // Never delete the Facts book itself.
        if Some(id) == self.facts_tree.root {
            self.status_message = Some("can't delete the Facts book".to_string());
            return;
        }
        let is_para = node.kind == NodeKind::Paragraph;
        if branch && is_para {
            self.status_message = Some("press - to delete a paragraph".to_string());
            return;
        }
        if !branch && !is_para {
            self.status_message = Some("press D to delete a branch".to_string());
            return;
        }
        self.pending_delete = Some(id);
        let title = node.title.clone();
        self.status_message = Some(format!("delete `{title}`? y / N"));
    }

    /// y/N for a pending delete.
    fn delete_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => self.tree_delete_commit(),
            _ => {
                self.pending_delete = None;
                self.status_message = Some("delete cancelled".to_string());
            }
        }
    }

    /// Perform the confirmed delete (the node + its subtree), then reload.
    fn tree_delete_commit(&mut self) {
        use crate::store::NodeKind;
        let Some(id) = self.pending_delete.take() else { return };
        let Some(node) = self.hierarchy.get(id).cloned() else { return };
        let ids = self.hierarchy.collect_subtree(id);
        let fs_rel = if node.kind == NodeKind::Paragraph {
            node.file.as_ref().map(std::path::PathBuf::from).unwrap_or_default()
        } else {
            self.hierarchy.fs_path(&node, &self.layout)
        };
        // Drop any pins inside the deleted subtree.
        self.pinned_nodes.retain(|p| !ids.contains(p));
        self.persist_pins();
        match self.store.delete_subtree(&fs_rel, &ids) {
            Ok(()) => {
                self.reload_hierarchy();
                self.status_message = Some(format!("deleted `{}`", node.title));
            }
            Err(e) => self.status_message = Some(format!("delete failed: {e}")),
        }
    }

    /// `y` / `x` — mark the cursor node for copy / move (paste with `p`).
    fn tree_yank(&mut self, mode: ClipMode) {
        let Some(id) = self.facts_tree.selected() else { return };
        if Some(id) == self.facts_tree.root {
            self.status_message = Some("can't copy/move the Facts book".to_string());
            return;
        }
        self.tree_clipboard = Some((id, mode));
        let title = self.hierarchy.get(id).map(|n| n.title.clone()).unwrap_or_default();
        self.status_message = Some(match mode {
            ClipMode::Copy => format!("yanked `{title}` — p to paste a copy"),
            ClipMode::Move => format!("cut `{title}` — p to paste (move)"),
        });
    }

    /// `p` — paste the clipboard node under the cursor (into a branch, or beside
    /// a paragraph). Copy duplicates a paragraph; move relocates a childless node.
    fn tree_paste(&mut self) {
        use crate::store::NodeKind;
        let Some((src, mode)) = self.tree_clipboard else {
            self.status_message = Some("nothing to paste (y yank / x cut first)".to_string());
            return;
        };
        let Some(cursor) = self.facts_tree.selected() else { return };
        // Target parent: the cursor if it can host content, else the cursor's parent.
        let parent = match self.hierarchy.get(cursor).map(|n| n.kind) {
            Some(NodeKind::Book | NodeKind::Chapter | NodeKind::Subchapter) => Some(cursor),
            _ => self.hierarchy.get(cursor).and_then(|n| n.parent_id),
        };
        let result: Result<Uuid, String> = match mode {
            ClipMode::Copy => self
                .store
                .copy_paragraph_to_parent(&self.cfg, &self.hierarchy, src, parent)
                .map_err(|e| e.to_string()),
            ClipMode::Move => self
                .store
                .move_node_to_parent(&self.cfg, &self.hierarchy, src, parent)
                .map(|_| src)
                .map_err(|e| e.to_string()),
        };
        match result {
            Ok(landed) => {
                if mode == ClipMode::Move {
                    self.tree_clipboard = None;
                }
                self.reload_hierarchy();
                let _ = self.facts_tree.reveal(&self.hierarchy, landed);
                self.status_message =
                    Some(if mode == ClipMode::Copy { "pasted a copy".into() } else { "moved".into() });
            }
            Err(e) => self.status_message = Some(format!("paste failed: {e}")),
        }
    }

    /// `K` / `J` — move the cursor node up / down among its siblings.
    fn tree_move(&mut self, up: bool) {
        use crate::store::NodeKind;
        let Some(id) = self.facts_tree.selected() else { return };
        let Some(node) = self.hierarchy.get(id) else { return };
        let parent = node.parent_id;
        // Content siblings in display order.
        let siblings: Vec<Uuid> = self
            .hierarchy
            .children_of(parent)
            .into_iter()
            .filter(|n| !matches!(n.kind, NodeKind::Image | NodeKind::Script))
            .map(|n| n.id)
            .collect();
        let Some(pos) = siblings.iter().position(|s| *s == id) else { return };
        let other = if up {
            if pos == 0 {
                return;
            }
            siblings[pos - 1]
        } else {
            if pos + 1 >= siblings.len() {
                return;
            }
            siblings[pos + 1]
        };
        match self.store.swap_siblings(&self.hierarchy, id, other) {
            Ok(()) => {
                self.reload_hierarchy();
                let _ = self.facts_tree.reveal(&self.hierarchy, id);
                self.status_message = Some(if up { "moved up".into() } else { "moved down".into() });
            }
            Err(e) => self.status_message = Some(format!("move failed: {e}")),
        }
    }
}

/// Recallable prompts from a thread's turns (newest first): query prompts and
/// the `/command` strings of insertions (RFC §18.3).
fn build_prompt_history(thread: &ResearchThread) -> Vec<String> {
    let mut out = Vec::new();
    for turn in thread.turns.iter().rev() {
        let entry = match turn.kind {
            TurnKind::Query => turn.prompt.clone(),
            TurnKind::FactInsertion | TurnKind::NoteInsertion => turn.command.clone(),
        };
        if let Some(e) = entry {
            if !e.trim().is_empty() && !out.contains(&e) {
                out.push(e);
            }
        }
    }
    out
}

/// `inkhaven research --list-threads`. (R-P19 may extend the formatting.)
/// `inkhaven research --import <path>` — ingest a document non-interactively
/// (R2-B). Thread-agnostic (recorded under thread ""), available to every thread.
pub(crate) fn import_cli(layout: &ProjectLayout, cfg: &Config, store: &Store, path: &str) -> Result<()> {
    let p = std::path::Path::new(path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if matches!(ext.as_str(), "bib" | "bibtex") {
        let (added, total) = import_bibtex(cfg, store, p)?;
        println!("imported {added}/{total} citation(s) from `{path}` → Sources");
        return Ok(());
    }
    if ext == "json" {
        let (added, total) = import_csl_json(cfg, store, p)?;
        println!("imported {added}/{total} CSL-JSON citation(s) from `{path}` → Sources");
        return Ok(());
    }
    if p.is_dir() {
        let (mut files, mut chunks) = (0usize, 0usize);
        for entry in walkdir::WalkDir::new(p).into_iter().filter_map(|e| e.ok()) {
            let fp = entry.path();
            let ext = fp.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            if fp.is_file() && matches!(ext.as_str(), "md" | "markdown" | "txt" | "text" | "pdf") {
                match embed_source_file(layout, cfg, store, fp) {
                    Ok(n) => {
                        files += 1;
                        chunks += n;
                    }
                    Err(e) => eprintln!("  skipped {}: {e}", fp.display()),
                }
            }
        }
        println!("imported {files} file(s), {chunks} chunk(s) from `{}`", p.display());
        return Ok(());
    }
    let n = embed_source_file(layout, cfg, store, p)?;
    println!("imported `{}` — {n} chunk(s) as a research source", super::imports::source_name(p));
    Ok(())
}

/// R3-D — parse a BibTeX file and add each entry to the Sources book (a Research
/// chapter, dedup by cite key). Returns `(added, total)`.
pub(crate) fn import_bibtex(cfg: &Config, store: &Store, p: &std::path::Path) -> Result<(usize, usize)> {
    let text = std::fs::read_to_string(p).map_err(|e| anyhow::anyhow!("read {}: {e}", p.display()))?;
    let entries = crate::sources::parse_bibtex(&text);
    let total = entries.len();
    let mut added = 0usize;
    for entry in &entries {
        if add_bibentry(store, cfg, entry).unwrap_or(false) {
            added += 1;
        }
    }
    Ok((added, total))
}

/// R3-D — parse a CSL-JSON file (an array of citation objects) into `BibEntry`s
/// and add each to the Sources book. Returns `(added, total)`.
pub(crate) fn import_csl_json(cfg: &Config, store: &Store, p: &std::path::Path) -> Result<(usize, usize)> {
    let text = std::fs::read_to_string(p).map_err(|e| anyhow::anyhow!("read {}: {e}", p.display()))?;
    let entries = parse_csl_json(&text)?;
    let total = entries.len();
    let mut added = 0usize;
    for entry in &entries {
        if add_bibentry(store, cfg, entry).unwrap_or(false) {
            added += 1;
        }
    }
    Ok((added, total))
}

/// Parse a CSL-JSON array (`[{ "type", "title", "author":[{family,given}], … }]`)
/// into `BibEntry`s.
pub(crate) fn parse_csl_json(text: &str) -> Result<Vec<crate::sources::BibEntry>> {
    let arr: serde_json::Value =
        serde_json::from_str(text).map_err(|e| anyhow::anyhow!("CSL-JSON parse: {e}"))?;
    let items = arr.as_array().ok_or_else(|| anyhow::anyhow!("CSL-JSON must be a JSON array"))?;
    Ok(items.iter().filter_map(csl_to_bibentry).collect())
}

/// Map one CSL-JSON object to a `BibEntry`.
fn csl_to_bibentry(obj: &serde_json::Value) -> Option<crate::sources::BibEntry> {
    let s = |k: &str| obj.get(k).and_then(|v| v.as_str()).map(str::to_string);
    let title = s("title")?;
    // Authors: "family, given and family, given …".
    let author = obj
        .get("author")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    let fam = a.get("family").and_then(|v| v.as_str());
                    let giv = a.get("given").and_then(|v| v.as_str());
                    match (fam, giv) {
                        (Some(f), Some(g)) => Some(format!("{f}, {g}")),
                        (Some(f), None) => Some(f.to_string()),
                        _ => a.get("literal").and_then(|v| v.as_str()).map(str::to_string),
                    }
                })
                .collect::<Vec<_>>()
                .join(" and ")
        })
        .unwrap_or_default();
    // Year: issued.date-parts[0][0].
    let year = obj
        .get("issued")
        .and_then(|i| i.get("date-parts"))
        .and_then(|d| d.as_array())
        .and_then(|a| a.first())
        .and_then(|p| p.as_array())
        .and_then(|p| p.first())
        .and_then(|y| y.as_i64())
        .map(|y| y.to_string())
        .unwrap_or_default();
    let entry_type = match obj.get("type").and_then(|v| v.as_str()).unwrap_or("") {
        "article-journal" | "article" => "article",
        "book" => "book",
        "chapter" => "incollection",
        "paper-conference" => "inproceedings",
        "thesis" => "phdthesis",
        _ => "misc",
    }
    .to_string();
    // Cite key: CSL `id`, else family+year slug.
    let key = s("id").unwrap_or_else(|| {
        let fam = author.split(',').next().unwrap_or("ref").trim().to_ascii_lowercase();
        let fam: String = fam.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        format!("{}{year}", if fam.is_empty() { "ref".to_string() } else { fam })
    });
    Some(crate::sources::BibEntry {
        key,
        entry_type,
        author,
        title,
        year,
        journal: s("container-title"),
        volume: s("volume"),
        number: s("issue"),
        pages: s("page"),
        publisher: s("publisher"),
        url: s("URL"),
        doi: s("DOI"),
        isbn: s("ISBN"),
        abstract_: s("abstract"),
        ..Default::default()
    })
}

/// R3-B/D — write a SOURCES-1 `BibEntry` into the Sources book under a "Research"
/// chapter (dedup by cite key). Returns `true` when a new entry was created.
pub(crate) fn add_bibentry(
    store: &Store,
    cfg: &Config,
    entry: &crate::sources::BibEntry,
) -> anyhow::Result<bool> {
    use crate::store::{InsertPosition, NodeKind};
    if !entry.is_valid() {
        return Ok(false);
    }
    let hier = crate::store::hierarchy::Hierarchy::load(store)?;
    let book = hier
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_SOURCES)
        })
        .ok_or_else(|| anyhow::anyhow!("Sources book missing"))?
        .clone();
    let chapter = match hier
        .children_of(Some(book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Research"))
        .cloned()
    {
        Some(c) => c,
        None => store.create_node(cfg, &hier, NodeKind::Chapter, "Research", Some(&book), None, InsertPosition::End)?,
    };
    // Dedup by cite key (case-insensitive) within the chapter.
    let key_lc = entry.key.to_lowercase();
    if hier.children_of(Some(chapter.id)).into_iter().any(|n| n.title.to_lowercase() == key_lc) {
        return Ok(false);
    }
    let hier = crate::store::hierarchy::Hierarchy::load(store)?;
    let mut node =
        store.create_node(cfg, &hier, NodeKind::Paragraph, &entry.key, Some(&chapter), None, InsertPosition::End)?;
    node.content_type = Some("hjson".to_string());
    let body = entry.to_hjson();
    if let Some(rel) = &node.file {
        let _ = crate::io_atomic::write(&store.project_root().join(rel), body.as_bytes());
    }
    store.update_paragraph_content(&mut node, body.as_bytes())?;
    Ok(true)
}

/// RESRCH-5 (R5-D) — collect the `BibEntry`s under the Sources book's Research
/// chapter (the auto-cited + imported entries), for `/bibliography`.
pub(crate) fn collect_research_bibentries(store: &Store, h: &Hierarchy) -> Vec<crate::sources::BibEntry> {
    use crate::store::NodeKind;
    let Some(book) = h.iter().find(|n| {
        n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_SOURCES)
    }) else {
        return Vec::new();
    };
    let Some(chapter) = h
        .children_of(Some(book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Research"))
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for pid in h.collect_subtree(chapter.id) {
        let Some(node) = h.get(pid) else { continue };
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let body = match store.get_content(pid) {
            Ok(Some(b)) => String::from_utf8_lossy(&b).into_owned(),
            _ => continue,
        };
        if let Some(e) = crate::sources::BibEntry::from_hjson(&body) {
            if e.is_valid() {
                out.push(e);
            }
        }
    }
    out
}

/// Embed one file as a research source (thread-global); returns the chunk count.
fn embed_source_file(layout: &ProjectLayout, cfg: &Config, store: &Store, p: &std::path::Path) -> Result<usize> {
    use super::imports;
    let text = imports::read_source(p)?;
    let chunks = imports::chunk_text(&text, cfg.research.import_chunk_chars.max(200));
    if chunks.is_empty() {
        anyhow::bail!("no text extracted from {}", p.display());
    }
    let name = imports::source_name(p);
    let abs = std::fs::canonicalize(p)
        .map(|c| c.to_string_lossy().into_owned())
        .unwrap_or_else(|_| p.to_string_lossy().into_owned());
    let now = chrono::Utc::now().to_rfc3339();
    let mut doc_ids = Vec::new();
    for (i, chunk) in chunks.iter().enumerate() {
        let meta = serde_json::json!({
            "kind": imports::SOURCE_KIND, "source": abs, "name": name,
            "thread": "", "chunk": i, "imported_at": now,
        });
        let id = store.raw().add_document(meta, chunk.as_bytes()).map_err(|e| anyhow::anyhow!("{e}"))?;
        doc_ids.push(id.to_string());
    }
    let mut imports_store = imports::Imports::load(layout);
    if let Some(old) = imports_store.sources.get(&name) {
        for id in &old.doc_ids {
            if let Ok(u) = uuid::Uuid::parse_str(id) {
                let _ = store.raw().delete_document(u);
            }
        }
    }
    let chunks_n = doc_ids.len();
    imports_store.sources.insert(
        name.clone(),
        imports::ImportedSource { name, path: abs, doc_ids, thread: String::new(), imported_at: now, chunks: chunks_n },
    );
    imports_store.save(layout)?;
    Ok(chunks_n)
}

pub(crate) fn list_threads_cli(layout: &ProjectLayout, format: Option<&str>) -> Result<()> {
    let summaries = thread::list_threads(layout);
    if format == Some("json") {
        println!("{}", serde_json::to_string_pretty(&summaries)?);
        return Ok(());
    }
    println!("{:<15} {:<13} {:>5}  {}", "Thread", "Last active", "Turns", "Cost");
    if summaries.is_empty() {
        println!("(no research threads yet)");
    }
    for s in &summaries {
        let date = s.last_active.get(0..10).unwrap_or(&s.last_active);
        println!("{:<15} {:<13} {:>5}  ${:.2}", s.name, date, s.turns, s.cost);
    }
    Ok(())
}

/// `inkhaven research --export-thread <name>`. (R-P19 fills in md/json bodies.)
pub(crate) fn export_thread_cli(
    layout: &ProjectLayout,
    name: &str,
    format: Option<&str>,
    out: Option<&str>,
) -> Result<()> {
    let slug = thread::thread_slug(name);
    let thread = ResearchThread::load(layout, &slug)
        .ok_or_else(|| anyhow::anyhow!("thread `{name}` not found"))?;
    let body = match format {
        Some("json") => serde_json::to_string_pretty(&thread)?,
        _ => export_markdown(&thread),
    };
    match out {
        Some(path) => std::fs::write(path, body)?,
        None => println!("{body}"),
    }
    Ok(())
}

/// A thread's history as Markdown (queries, responses, insertions).
fn export_markdown(thread: &ResearchThread) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(s, "# Research thread: {}\n", thread.display_name);
    let _ = writeln!(s, "_Created {} · last active {}_\n", thread.created_at, thread.last_active);
    for turn in &thread.turns {
        match turn.kind {
            super::thread::TurnKind::Query => {
                if let Some(p) = &turn.prompt {
                    let _ = writeln!(s, "## {p}\n");
                }
                if let Some(r) = &turn.response {
                    let _ = writeln!(s, "{r}\n");
                }
            }
            super::thread::TurnKind::FactInsertion | super::thread::TurnKind::NoteInsertion => {
                let book = turn.target_book.as_deref().unwrap_or("Facts");
                let title = turn.extracted_title.as_deref().unwrap_or("");
                let text = turn.extracted_text.as_deref().unwrap_or("");
                let path = turn.insertion_path.as_deref().unwrap_or("");
                let _ = writeln!(s, "> **[{book}] {title}** → `{path}`\n>\n> {text}\n");
            }
        }
    }
    s
}

/// R2-E — the char index where a completable Facts slug path begins in `text`:
/// just after `→` / `->` (the insertion path of `/fact`/`/note`), or after the
/// `/goto ` command word. `None` when there's no completable path token.
fn completable_path_start(text: &str) -> Option<usize> {
    // Arrow form first — it appears after a quoted/bare prompt.
    let arrow = text
        .find('→')
        .map(|i| (i, '→'.len_utf8()))
        .or_else(|| text.find("->").map(|i| (i, 2)));
    if let Some((bi, w)) = arrow {
        let after = &text[bi + w..];
        let lead = after.len() - after.trim_start().len();
        return Some(text[..bi + w + lead].chars().count());
    }
    // `/goto <path>` — require the trailing space so `/gotoX` isn't treated as one.
    let lower = text.to_ascii_lowercase();
    if lower.starts_with("/goto") {
        let after = &text[5..];
        if !after.is_empty() && !after.starts_with(char::is_whitespace) {
            return None;
        }
        let lead = after.len() - after.trim_start().len();
        return Some(text[..5 + lead].chars().count());
    }
    None
}

/// The longest common prefix shared by every string (by `char`).
fn longest_common_prefix(items: &[String]) -> String {
    let Some(first) = items.first() else { return String::new() };
    let mut end = first.chars().count();
    for s in &items[1..] {
        let common = first.chars().zip(s.chars()).take_while(|(a, b)| a == b).count();
        end = end.min(common);
    }
    first.chars().take(end).collect()
}

#[cfg(test)]
mod completion_tests {
    use super::{completable_path_start, longest_common_prefix};

    #[test]
    fn path_start_after_goto() {
        let t = "/goto facts/rom";
        let start = completable_path_start(t).unwrap();
        let typed: String = t.chars().skip(start).collect();
        assert_eq!(typed, "facts/rom");
    }

    #[test]
    fn path_start_after_arrow() {
        for t in ["/fact \"x\" → facts/ro", "/fact \"x\" -> facts/ro"] {
            let start = completable_path_start(t).unwrap();
            let typed: String = t.chars().skip(start).collect();
            assert_eq!(typed, "facts/ro");
        }
    }

    #[test]
    fn non_path_commands_have_no_completion() {
        assert!(completable_path_start("/diff").is_none());
        assert!(completable_path_start("just a question").is_none());
        assert!(completable_path_start("/gotoX").is_none());
    }

    #[test]
    fn lcp_of_candidates() {
        let c = vec!["engineering".to_string(), "engineers".to_string(), "england".to_string()];
        assert_eq!(longest_common_prefix(&c), "eng");
        let d = vec!["rome".to_string(), "ravenna".to_string()];
        assert_eq!(longest_common_prefix(&d), "r");
        assert_eq!(longest_common_prefix(&["solo".to_string()]), "solo");
    }
}

/// R5-E — from the `/upgrade` corroboration judgement, the origin tier of the
/// best corroborating source (highest tier that SUPPORTS with **no** source
/// CONTRADICTING), or `None`. Priority: wikidata > openalex > arxiv.
fn pick_corroborator(buf: &str) -> Option<String> {
    let mut supports: Vec<&str> = Vec::new();
    let mut any_contradict = false;
    for line in buf.lines() {
        let u = line.to_ascii_uppercase();
        let head = u.split('—').next().unwrap_or("");
        if head.contains("CONTRADICT") {
            any_contradict = true;
        } else if head.contains("SUPPORT") {
            if head.contains("WIKIDATA") {
                supports.push("wikidata");
            } else if head.contains("OPENALEX") {
                supports.push("openalex");
            } else if head.contains("ARXIV") {
                supports.push("arxiv");
            }
        }
    }
    if any_contradict {
        return None;
    }
    ["wikidata", "openalex", "arxiv"]
        .into_iter()
        .find(|tier| supports.contains(tier))
        .map(str::to_string)
}

/// R3-E — parse the triangulation judge's output into a one-line verdict and a
/// pass flag. Pass = at least one source SUPPORTS and none CONTRADICTS; a weak or
/// contradicted verdict requires a second confirm.
fn summarize_triangulation(buf: &str) -> (String, bool) {
    let mut supports = 0usize;
    let mut contradicts = 0usize;
    let mut agreement: Option<String> = None;
    for line in buf.lines() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        if l.to_ascii_lowercase().starts_with("agreement") {
            agreement = Some(l.to_string());
            continue;
        }
        // The verdict token sits before the em-dash / hyphen reason separator.
        let head = l.split('—').next().unwrap_or(l);
        let head = head.split(" - ").next().unwrap_or(head).to_ascii_uppercase();
        if head.contains("CONTRADICT") {
            contradicts += 1;
        } else if head.contains("SUPPORT") {
            supports += 1;
        }
    }
    let verdict =
        agreement.unwrap_or_else(|| format!("{supports} support · {contradicts} contradict"));
    (verdict, supports >= 1 && contradicts == 0)
}

#[cfg(test)]
mod triangulation_tests {
    use super::summarize_triangulation;

    #[test]
    fn passes_on_support_without_contradiction() {
        let buf = "Wikidata: SUPPORTS — matches the triple\narXiv: SILENT — not addressed\nAgreement: 1/2 support";
        let (v, pass) = summarize_triangulation(buf);
        assert!(pass);
        assert!(v.contains("Agreement"));
    }

    #[test]
    fn fails_on_contradiction() {
        let buf = "Wikidata: CONTRADICTS — the date differs\nOpenAlex: SUPPORTS — consistent";
        let (_, pass) = summarize_triangulation(buf);
        assert!(!pass);
    }

    #[test]
    fn fails_when_all_silent() {
        let buf = "Wikidata: SILENT — nothing\narXiv: SILENT — nothing\nAgreement: 0/2 support";
        let (_, pass) = summarize_triangulation(buf);
        assert!(!pass);
    }

    #[test]
    fn corroborator_prefers_highest_tier_and_respects_contradiction() {
        use super::pick_corroborator;
        // Wikidata supports, arXiv silent → wikidata wins (top tier).
        assert_eq!(
            pick_corroborator("Wikidata: SUPPORTS — matches\narXiv: SILENT — n/a").as_deref(),
            Some("wikidata")
        );
        // OpenAlex supports, no wikidata → openalex.
        assert_eq!(
            pick_corroborator("OpenAlex: SUPPORTS — consistent").as_deref(),
            Some("openalex")
        );
        // Any contradiction → no corroboration.
        assert!(pick_corroborator("Wikidata: SUPPORTS — x\narXiv: CONTRADICTS — y").is_none());
        assert!(pick_corroborator("Wikidata: SILENT — nothing").is_none());
    }
}

#[cfg(test)]
mod csl_tests {
    use super::parse_csl_json;

    #[test]
    fn parses_csl_json_array() {
        let text = r#"[
          {"id":"vaswani2017","type":"paper-conference","title":"Attention Is All You Need",
           "author":[{"family":"Vaswani","given":"Ashish"},{"family":"Shazeer","given":"Noam"}],
           "issued":{"date-parts":[[2017]]},"DOI":"10.5555/x","container-title":"NeurIPS"}
        ]"#;
        let v = parse_csl_json(text).unwrap();
        assert_eq!(v.len(), 1);
        let e = &v[0];
        assert_eq!(e.key, "vaswani2017");
        assert_eq!(e.entry_type, "inproceedings");
        assert_eq!(e.author, "Vaswani, Ashish and Shazeer, Noam");
        assert_eq!(e.year, "2017");
        assert_eq!(e.doi.as_deref(), Some("10.5555/x"));
        assert!(e.is_valid());
    }

    #[test]
    fn derives_key_when_absent_and_rejects_non_array() {
        let text = r#"[{"type":"book","title":"T","author":[{"family":"Roe","given":"J"}],"issued":{"date-parts":[[1999]]}}]"#;
        let v = parse_csl_json(text).unwrap();
        assert_eq!(v[0].key, "roe1999");
        assert!(parse_csl_json("{}").is_err()); // object, not array
    }
}
