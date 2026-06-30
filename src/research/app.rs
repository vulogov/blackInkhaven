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

use tui_textarea::TextArea;
use uuid::Uuid;

use super::Focus;
use super::chat::ChatTurn;
use super::facts_tree::FactsTree;
use super::render;
use super::thread::{self, ResearchThread, TurnKind};

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

    /// R-P6 — chat pane scroll keys. Returns whether consumed.
    fn chat_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
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

        if text.starts_with('/') {
            self.status_message = Some("commands arrive in R-P9".to_string());
            return;
        }
        // Placeholder response until the streaming LLM lands (R-P7).
        self.chat_history
            .push(ChatTurn::with_response(text, "(LLM streaming wiring lands in R-P7)".to_string()));
        self.rebuild_prompt_history();
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
