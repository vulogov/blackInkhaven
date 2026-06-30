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

/// Default cap on pinned Facts nodes (RFC §16); the `research:` config block
/// (R-P20) will override it.
pub(crate) const DEFAULT_MAX_PINNED: usize = 3;

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

/// R-P10/R-P11 — an in-flight extraction stream feeding the confirmation overlay.
pub(super) struct ExtractState {
    rx: mpsc::UnboundedReceiver<StreamMsg>,
    buf: String,
    book: TargetBook,
    book_id: Uuid,
    target: Option<Uuid>,
    command: String,
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

/// R-P15 — `/chain`: a sequential research pipeline. Each step's response is
/// accumulated as context for the next.
pub(super) struct ChainState {
    steps: Vec<String>,
    current: usize,
    accumulated: Vec<String>,
    rx: Option<mpsc::UnboundedReceiver<StreamMsg>>,
    turn_idx: usize,
}

/// Facts-tree / AI-chat split: tree columns out of 10 (4 = 40% tree, 60% chat).
/// Hard-coded until the `research:` config block lands (R-P20).
pub(crate) const DEFAULT_SPLIT_RATIO: u32 = 4;

pub(crate) struct ResearchApp {
    pub(super) layout: ProjectLayout,
    pub(super) cfg: Config,
    pub(super) store: Store,
    pub(super) hierarchy: Hierarchy,

    /// The open, persistent research thread (R-P2).
    pub(super) thread: ResearchThread,

    /// The Facts tree pane (R-P4).
    pub(super) facts_tree: FactsTree,
    /// Pinned Facts nodes (G4); RAG injection lands in R-P8.
    pub(super) pinned_nodes: Vec<Uuid>,
    /// The inline manual-entry overlay, when active (G7).
    pub(super) manual: Option<ManualEntry>,

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
    /// The editable insertion confirmation overlay (G1/G2).
    pub(super) confirmation: Option<ConfirmationState>,
    /// Accumulated session cost estimate (USD).
    pub(super) session_cost: f64,

    pub(super) focus: Focus,
    pub(super) show_hints: bool,
    pub(super) split_ratio: u32,
    pub(super) status_message: Option<String>,

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
        Ok(ResearchApp {
            layout,
            cfg,
            store,
            hierarchy,
            thread,
            facts_tree,
            pinned_nodes,
            manual: None,
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
            confirmation: None,
            session_cost: 0.0,
            focus: Focus::QueryPrompt,
            show_hints: true,
            split_ratio: DEFAULT_SPLIT_RATIO,
            status_message: None,
            should_quit: false,
        })
    }

    /// Cap on pinned nodes (config in R-P20; default for now).
    fn max_pinned(&self) -> usize {
        DEFAULT_MAX_PINNED
    }

    /// The synchronous event loop: draw, then block up to 100 ms for a key.
    /// Later phases also drain the `StreamMsg` receiver here each tick (R-P7).
    pub(crate) fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        while !self.should_quit {
            self.poll_stream();
            self.poll_extraction();
            self.poll_verify();
            self.poll_chain();
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
        // Ctrl+C always exits.
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
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
        // Tab / Shift+Tab always cycle focus (even from the text prompt).
        match key.code {
            KeyCode::Tab => {
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

    /// R-P5 — the query prompt. Enter submits, ↑/↓ recall history, Esc
    /// clears-then-defocuses, everything else types into the textarea.
    fn query_prompt_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => self.submit_query(),
            KeyCode::Up => self.history_back(),
            KeyCode::Down => self.history_forward(),
            KeyCode::Esc => {
                if self.query_text().trim().is_empty() {
                    self.focus = Focus::FactsTree;
                } else {
                    self.query = TextArea::default();
                    self.prompt_history_idx = None;
                }
            }
            _ => {
                // History navigation resets once the user edits the draft.
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
            Command::Chain(steps) => self.start_chain(steps),
        }
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

        let rag = super::rag::build_context(
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
                Ok(StreamMsg::Done) | Err(mpsc::error::TryRecvError::Disconnected) => {
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
            let cost = llm::estimate_cost(&turn.prompt, &turn.response);
            turn.cost = cost;
            self.session_cost += cost;
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

        // R-P8 — assemble Facts RAG context (pins + semantic), gated by mode.
        let rag = super::rag::build_context(
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
                Ok(StreamMsg::Done) => {
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
        let (prompt, response) = match self.chat_history.get_mut(idx) {
            Some(turn) => {
                turn.streaming = false;
                let cost = llm::estimate_cost(&turn.prompt, &turn.response);
                turn.cost = cost;
                self.session_cost += cost;
                (turn.prompt.clone(), turn.response.clone())
            }
            None => return,
        };
        let now = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::now_v7().to_string();
        let cost = llm::estimate_cost(&prompt, &response);
        let _ = self.thread.push_turn(
            ResearchTurn::query(id, prompt, response, cost, now),
            &self.layout,
        );
        self.rebuild_prompt_history();
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
        const DIFF_TOP_N: usize = 3;
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
            .take(DIFF_TOP_N)
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
            format!("[/diff — top {DIFF_TOP_N} similar facts]"),
            out,
        ));
        self.chat_scroll = 0;
    }

    /// R-P14 — `/verify`: probe the model's confidence in the specific claims of
    /// the last response. Extracts checkable claims, then asks for a HIGH /
    /// MEDIUM / LOW assessment of each.
    fn run_verify(&mut self) {
        const MIN_WORDS: usize = 8;
        if self.verify_rx.is_some() {
            self.status_message = Some("a verification is already running".to_string());
            return;
        }
        let Some(response) = self.chat_history.last().map(|t| t.response.clone()) else {
            self.status_message = Some("no research response to verify".to_string());
            return;
        };
        let claims = super::verify::extract_claims(&response, MIN_WORDS);
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
                Ok(StreamMsg::Done) | Err(mpsc::error::TryRecvError::Disconnected) => {
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
    fn start_extraction(
        &mut self,
        book: TargetBook,
        prompt: Option<String>,
        path: Option<String>,
        command_name: &str,
    ) {
        if self.extracting.is_some() || self.confirmation.is_some() {
            self.status_message = Some("finish the current extraction first".to_string());
            return;
        }
        let Some(research) = self.chat_history.last().map(|t| t.response.clone()) else {
            self.status_message = Some("no research response yet — ask a question first".to_string());
            return;
        };
        if research.trim().is_empty() {
            self.status_message = Some("the last response is empty — nothing to extract".to_string());
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
        let system = extract::system_prompt(book, &instruction, &research);
        let rx = spawn_chat_stream(
            ai.client.clone(),
            model.to_string(),
            Some(system),
            Vec::new(),
            "Produce the entry as specified.".to_string(),
            llm::CATEGORY,
        );
        self.extracting = Some(ExtractState {
            rx,
            buf: String::new(),
            book,
            book_id,
            target,
            command: format!("{command_name} \"{instruction}\""),
        });
        self.status_message = Some(format!("Extracting {}…", book.label()));
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
        let mut done = false;
        loop {
            match ex.rx.try_recv() {
                Ok(StreamMsg::Token(t)) => ex.buf.push_str(&t),
                Ok(StreamMsg::Done) | Err(mpsc::error::TryRecvError::Disconnected) => {
                    done = true;
                    break;
                }
                Ok(StreamMsg::Error(e)) => {
                    self.status_message = Some(format!("extraction error: {e}"));
                    self.extracting = None;
                    return;
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
            }
        }
        if done {
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
        let Some(c) = self.confirmation.take() else { return };
        let title = c.title.lines().join(" ");
        let body = c.body.lines().join("\n");
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
                self.rebuild_prompt_history();
                self.status_message = Some(format!("✓ Inserted: '{}' → {path}", title.trim()));
            }
            Err(e) => self.status_message = Some(format!("insert failed: {e}")),
        }
        self.focus = Focus::QueryPrompt;
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
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => {
                self.facts_tree.step_in(&self.hierarchy)
            }
            KeyCode::Left | KeyCode::Char('h') => self.facts_tree.step_out(&self.hierarchy),
            KeyCode::Char('n') => {
                self.manual = Some(ManualEntry {
                    stage: ManualStage::Title,
                    title: String::new(),
                    body: String::new(),
                });
            }
            // Ctrl+P pins / unpins the cursor node (G4).
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => self.toggle_pin(),
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

    /// Save the current pin set onto the thread.
    fn persist_pins(&mut self) {
        self.thread.pinned_nodes = self.pinned_nodes.iter().map(|u| u.to_string()).collect();
        let _ = self.thread.save(&self.layout);
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
                self.status_message = Some(format!("✓ added fact: {}", m.title.trim()));
            }
            Err(e) => self.status_message = Some(format!("insert failed: {e}")),
        }
    }

    /// Reload the hierarchy after a store mutation and rebuild the Facts tree.
    pub(super) fn reload_hierarchy(&mut self) {
        if let Ok(h) = Hierarchy::load(&self.store) {
            self.hierarchy = h;
            self.facts_tree.rebuild(&self.hierarchy);
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
