use std::io;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use tui_textarea::{CursorMove, TextArea};
use uuid::Uuid;

use crate::ai::AiClient;
use crate::ai::prompts::{Prompt, PromptLibrary};
use crate::ai::stream::{StreamMsg, spawn_chat_stream};
use crate::config::Config;
use crate::error::{Error, Result as InkResult};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};
use crate::store::{InsertPosition, Snapshot};

use super::file_picker::{FilePicker, PickerContext};
use super::focus::Focus;
use super::highlight::{
    BlockSelection, TypstHighlighter, build_row_spans, build_visual_row_spans, diff_added,
    wrap_line,
};
use super::input::TextInput;
use super::keymap::KeyChord;
use super::search_results::SearchHit;

pub fn run(project: &Path) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized().map_err(anyhow::Error::from)?;

    let cfg = Config::load(&layout.config_path()).map_err(anyhow::Error::from)?;
    let store = Store::open(layout.clone(), &cfg).map_err(anyhow::Error::from)?;

    let mut app = App::new(layout, cfg, store)?;

    // Install a panic hook that restores the terminal before re-panicking
    // through the user's hook. Otherwise a panic inside the TUI leaves the
    // shell in raw-mode + alt-screen and the message is invisible.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = app.run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    // Restore the hook we replaced.
    let _ = std::panic::take_hook();

    result
}

struct Keymap {
    next_pane: KeyChord,
    prev_pane: KeyChord,
    search: KeyChord,
    ai_prompt: KeyChord,
    save: KeyChord,
    page_up: KeyChord,
    page_down: KeyChord,
    add_book: KeyChord,
    add_chapter: KeyChord,
    add_subchapter: KeyChord,
    add_paragraph: KeyChord,
    delete_node: KeyChord,
    move_up: KeyChord,
    move_down: KeyChord,
}

impl Keymap {
    fn from_config(cfg: &Config) -> InkResult<Self> {
        let parse = |label: &str, s: &str| -> InkResult<KeyChord> {
            KeyChord::parse(s).map_err(|e| Error::Config(format!("keys.{label}: {e}")))
        };
        Ok(Self {
            next_pane: parse("next_pane", &cfg.keys.next_pane)?,
            prev_pane: parse("prev_pane", &cfg.keys.prev_pane)?,
            search: parse("search", &cfg.keys.search)?,
            ai_prompt: parse("ai_prompt", &cfg.keys.ai_prompt)?,
            save: parse("save", &cfg.keys.save)?,
            page_up: parse("page_up", &cfg.keys.page_up)?,
            page_down: parse("page_down", &cfg.keys.page_down)?,
            add_book: parse("add_book", &cfg.keys.add_book)?,
            add_chapter: parse("add_chapter", &cfg.keys.add_chapter)?,
            add_subchapter: parse("add_subchapter", &cfg.keys.add_subchapter)?,
            add_paragraph: parse("add_paragraph", &cfg.keys.add_paragraph)?,
            delete_node: parse("delete_node", &cfg.keys.delete_node)?,
            move_up: parse("move_up", &cfg.keys.move_up)?,
            move_down: parse("move_down", &cfg.keys.move_down)?,
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum MoveDir {
    Up,
    Down,
}

#[derive(Default)]
struct ImportCounts {
    /// Any branch created during import: chapter, subchapter, or book.
    branches: usize,
    paragraphs: usize,
}

fn derive_paragraph_title_from_path(path: &std::path::Path) -> String {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("imported");
    let pretty: String = stem
        .replace('_', " ")
        .replace('-', " ")
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(c).collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    if pretty.is_empty() {
        "Imported".into()
    } else {
        pretty
    }
}

#[derive(Debug, Clone, Copy)]
enum InferenceAction {
    Replace,
    Insert,
    Top,
    Bottom,
    CopyOnly,
}

impl InferenceAction {
    fn label(&self) -> &'static str {
        match self {
            InferenceAction::Replace => "replaced",
            InferenceAction::Insert => "inserted at cursor",
            InferenceAction::Top => "prepended to top",
            InferenceAction::Bottom => "appended to bottom",
            InferenceAction::CopyOnly => "copied",
        }
    }
}

enum Modal {
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
    FilePicker(FilePicker),
    SnapshotPicker {
        /// Kept for potential refresh ops after future snapshot mutations.
        #[allow(dead_code)]
        paragraph_id: Uuid,
        paragraph_title: String,
        snapshots: Vec<Snapshot>,
        cursor: usize,
    },
}

struct App {
    layout: ProjectLayout,
    store: Store,
    keymap: Keymap,
    cfg: Config,
    ai: AiClient,
    prompts: PromptLibrary,

    hierarchy: Hierarchy,
    rows: Vec<(Uuid, usize)>,
    modal: Modal,

    focus: Focus,
    tree_cursor: usize,
    tree_scroll: usize,

    search_input: TextInput,
    ai_input: TextInput,

    opened: Option<OpenedDoc>,
    status: String,
    show_results_overlay: bool,
    results: Vec<SearchHit>,
    results_cursor: usize,

    /// System clipboard handle. May be None on headless systems or when init
    /// fails; in that case copy/cut/paste use tui-textarea's internal yank
    /// buffer only.
    clipboard: Option<arboard::Clipboard>,

    highlighter: TypstHighlighter,

    inference: Option<Inference>,
    show_prompt_picker: bool,
    prompt_picker_cursor: usize,
}

#[derive(Debug)]
struct Inference {
    provider: String,
    /// Kept for diagnostics on the Debug impl; not displayed in the UI.
    #[allow(dead_code)]
    model: String,
    response: String,
    status: InferenceStatus,
    rx: tokio::sync::mpsc::UnboundedReceiver<StreamMsg>,
    started_at: std::time::Instant,
}

#[derive(Debug, Clone)]
enum InferenceStatus {
    Streaming,
    Done,
    Error(String),
}

struct OpenedDoc {
    id: Uuid,
    title: String,
    rel_path: String,
    textarea: TextArea<'static>,
    dirty: bool,
    /// Custom scroll state. tui-textarea v0.7 does not expose its viewport, so
    /// we maintain our own and never call `textarea.scroll()`.
    scroll_row: usize,
    scroll_col: usize,
    /// Anchor of a vertical-block selection (entered with Alt+arrows).
    /// While Some, the cursor's current position plus this anchor define a
    /// rectangular selection drawn with REVERSED style.
    block_anchor: Option<(usize, usize)>,
    /// Wall-clock of the last key event handled by the editor. Idle autosave
    /// fires when (now - last_activity) >= editor.autosave_seconds.
    last_activity: std::time::Instant,
    /// Snapshot of `textarea.lines()` at the most recent save / load. Used to
    /// bold characters added since then.
    saved_lines: Vec<String>,
}

impl App {
    fn new(layout: ProjectLayout, cfg: Config, store: Store) -> Result<Self> {
        let keymap = Keymap::from_config(&cfg).map_err(anyhow::Error::from)?;
        let hierarchy = Hierarchy::load(&store).map_err(anyhow::Error::from)?;
        let rows: Vec<(Uuid, usize)> = hierarchy
            .flatten()
            .into_iter()
            .map(|(n, d)| (n.id, d))
            .collect();

        let ai = AiClient::from_config(&cfg.llm).map_err(anyhow::Error::from)?;

        let prompts_path = layout.prompts_path(&cfg);
        let prompts = if prompts_path.is_file() {
            PromptLibrary::load(&prompts_path).map_err(anyhow::Error::from)?
        } else {
            PromptLibrary::default()
        };

        Ok(Self {
            layout,
            store,
            keymap,
            cfg,
            ai,
            prompts,
            hierarchy,
            rows,
            modal: Modal::None,
            focus: Focus::Tree,
            tree_cursor: 0,
            tree_scroll: 0,
            search_input: TextInput::new(),
            ai_input: TextInput::new(),
            opened: None,
            status: String::from(
                "Tab=panes · Enter=open · Ctrl+S=save · Ctrl+Shift+B/C/S/P=add · Ctrl+Shift+D=delete · Ctrl+Q=quit",
            ),
            show_results_overlay: false,
            results: Vec::new(),
            results_cursor: 0,
            clipboard: arboard::Clipboard::new().ok(),
            highlighter: TypstHighlighter::new()
                .map_err(|e| anyhow::anyhow!("typst highlighter init: {e}"))?,
            inference: None,
            show_prompt_picker: false,
            prompt_picker_cursor: 0,
        })
    }

    fn run<B: ratatui::backend::Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            self.pump_inference();
            self.tick_autosave();
            terminal.draw(|f| self.draw(f))?;
            // Shorter poll interval while streaming so tokens render with low
            // latency without burning CPU when idle.
            let timeout = if self.is_streaming() {
                Duration::from_millis(40)
            } else {
                Duration::from_millis(200)
            };
            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    if self.handle_key(key)? {
                        return Ok(());
                    }
                }
            }
        }
    }

    fn tick_autosave(&mut self) {
        let secs = self.cfg.editor.autosave_seconds;
        if secs == 0 {
            return;
        }
        let due = match self.opened.as_ref() {
            Some(doc) if doc.dirty => {
                doc.last_activity.elapsed().as_secs() >= secs
            }
            _ => false,
        };
        if due {
            let _ = self.save_current();
        }
    }

    fn is_streaming(&self) -> bool {
        matches!(
            self.inference.as_ref().map(|i| &i.status),
            Some(InferenceStatus::Streaming)
        )
    }

    fn pump_inference(&mut self) {
        let Some(inf) = self.inference.as_mut() else {
            return;
        };
        if !matches!(inf.status, InferenceStatus::Streaming) {
            return;
        }
        loop {
            match inf.rx.try_recv() {
                Ok(StreamMsg::Token(t)) => inf.response.push_str(&t),
                Ok(StreamMsg::Done) => {
                    inf.status = InferenceStatus::Done;
                    let elapsed = inf.started_at.elapsed();
                    self.status = format!(
                        "{} responded in {:.1}s",
                        inf.provider,
                        elapsed.as_secs_f32()
                    );
                    break;
                }
                Ok(StreamMsg::Error(e)) => {
                    inf.status = InferenceStatus::Error(e.clone());
                    self.status = format!("inference error: {e}");
                    break;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    // Task ended without a final message — treat as done.
                    if matches!(inf.status, InferenceStatus::Streaming) {
                        inf.status = InferenceStatus::Done;
                    }
                    break;
                }
            }
        }
    }

    // -------- key dispatch ------------------------------------------------

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        // Hard quit works from anywhere, including inside a modal.
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
        {
            return Ok(self.request_quit());
        }

        // Modal eats every other key.
        if !matches!(self.modal, Modal::None) {
            return self.handle_modal_key(key);
        }

        // Ctrl+1..5 — direct focus jumps. Most terminals send these as
        // Char(digit) + CONTROL. But Ctrl+2 on US-layout terminals is often
        // re-encoded as Ctrl+@ → KeyCode::Char('@') with CONTROL, or as
        // KeyCode::Null. Try every variant we've seen in the wild.
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && !key
                .modifiers
                .intersects(KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            let target = match key.code {
                KeyCode::Char('1') => Some(Focus::Editor),
                // Ctrl+2 alternates:
                KeyCode::Char('2') | KeyCode::Char('@') => Some(Focus::Tree),
                KeyCode::Char('3') => Some(Focus::Ai),
                KeyCode::Char('4') => Some(Focus::SearchBar),
                KeyCode::Char('5') => Some(Focus::AiPrompt),
                // Ctrl+T also focuses Tree — mnemonic and not terminal-eaten.
                KeyCode::Char('t') | KeyCode::Char('T') => Some(Focus::Tree),
                _ => None,
            };
            if let Some(focus) = target {
                self.focus = focus;
                return Ok(false);
            }
        }
        // KeyCode::Null with no modifiers is what some terminals report for
        // Ctrl+2 / Ctrl+Space. Catch that separately because the inner block
        // requires the CONTROL modifier flag.
        if matches!(key.code, KeyCode::Null) {
            self.focus = Focus::Tree;
            return Ok(false);
        }

        // Tree-management chords. Work from any focus so the user doesn't
        // have to defocus the editor to add a chapter.
        if self.keymap.add_book.matches(&key) {
            self.open_add_modal(NodeKind::Book);
            return Ok(false);
        }
        if self.keymap.add_chapter.matches(&key) {
            self.open_add_modal(NodeKind::Chapter);
            return Ok(false);
        }
        if self.keymap.add_subchapter.matches(&key) {
            self.open_add_modal(NodeKind::Subchapter);
            return Ok(false);
        }
        if self.keymap.add_paragraph.matches(&key) {
            self.open_add_modal(NodeKind::Paragraph);
            return Ok(false);
        }
        if self.keymap.delete_node.matches(&key) {
            self.open_delete_modal();
            return Ok(false);
        }
        if self.keymap.move_up.matches(&key) {
            self.move_current(MoveDir::Up);
            return Ok(false);
        }
        if self.keymap.move_down.matches(&key) {
            self.move_current(MoveDir::Down);
            return Ok(false);
        }

        // Save works from anywhere as long as a doc is open.
        if self.keymap.save.matches(&key) && self.opened.is_some() {
            self.save_current()?;
            return Ok(false);
        }

        // Focus jumps from anywhere.
        if self.keymap.search.matches(&key) {
            self.focus = Focus::SearchBar;
            return Ok(false);
        }
        if self.keymap.ai_prompt.matches(&key) {
            self.focus = Focus::AiPrompt;
            return Ok(false);
        }

        // Tab cycling everywhere except when typing into a buffer.
        let in_editor_with_doc = self.focus == Focus::Editor && self.opened.is_some();
        let cycling_blocked = self.focus.is_input() || in_editor_with_doc;
        if !cycling_blocked {
            if self.keymap.next_pane.matches(&key) {
                self.focus = self.focus.next();
                return Ok(false);
            }
            if self.keymap.prev_pane.matches(&key) {
                self.focus = self.focus.prev();
                return Ok(false);
            }
        } else if in_editor_with_doc
            && (self.keymap.next_pane.matches(&key) || self.keymap.prev_pane.matches(&key))
        {
            // Inside an active editor, Tab cycles focus too — but only when
            // the user really meant to (no other modifiers were on). If we
            // didn't intercept here, Tab would insert a literal tab via
            // tui-textarea.
            self.focus = if self.keymap.next_pane.matches(&key) {
                self.focus.next()
            } else {
                self.focus.prev()
            };
            return Ok(false);
        }

        match self.focus {
            Focus::Tree => self.handle_tree_key(key),
            Focus::Editor => self.handle_editor_key(key),
            Focus::Ai => self.handle_passive_key(key),
            Focus::SearchBar => self.handle_input_key(key, true),
            Focus::AiPrompt => self.handle_input_key(key, false),
        }
    }

    fn handle_tree_key(&mut self, key: KeyEvent) -> Result<bool> {
        // For plain-letter / punctuation shortcuts we ignore the SHIFT modifier
        // (uppercase letters require Shift on most layouts) but reject Ctrl /
        // Alt / Super so e.g. Ctrl+A doesn't accidentally trigger Add-subchapter.
        let plain = !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') if plain => return Ok(self.request_quit()),
            KeyCode::F(2) => self.open_rename_modal(),
            // F3 in Tree: import a file (becomes new paragraph after cursor)
            // or a directory tree (dirs → subchapters, files → paragraphs).
            KeyCode::F(3) => self.open_file_picker(PickerContext::TreeInsertOrImport),
            KeyCode::Up => self.move_cursor(-1),
            KeyCode::Down => self.move_cursor(1),
            KeyCode::Home => self.tree_cursor = 0,
            KeyCode::End => {
                if !self.rows.is_empty() {
                    self.tree_cursor = self.rows.len() - 1;
                }
            }
            KeyCode::Enter => self.open_selected()?,

            // Tree-pane add shortcuts. These exist alongside the global
            // Ctrl+Shift+* chords because terminals and multiplexers
            // commonly eat those (Ctrl+S = XOFF, tmux prefix, etc.).
            KeyCode::Char('B') | KeyCode::Char('b') if plain => {
                self.open_add_modal(NodeKind::Book);
            }
            // C/A/+ append at the end of the parent's children;
            // V/S/P insert immediately after the cursor's same-kind ancestor.
            KeyCode::Char('C') | KeyCode::Char('c') if plain => {
                self.open_add_modal(NodeKind::Chapter);
            }
            KeyCode::Char('V') | KeyCode::Char('v') if plain => {
                self.open_add_modal_after(NodeKind::Chapter);
            }
            KeyCode::Char('A') | KeyCode::Char('a') if plain => {
                self.open_add_modal(NodeKind::Subchapter);
            }
            KeyCode::Char('S') | KeyCode::Char('s') if plain => {
                self.open_add_modal_after(NodeKind::Subchapter);
            }
            KeyCode::Char('+') if plain => {
                self.open_add_modal(NodeKind::Paragraph);
            }
            KeyCode::Char('P') | KeyCode::Char('p') if plain => {
                self.open_add_modal_after(NodeKind::Paragraph);
            }

            // Kind-specific delete: D for branches, - for paragraphs. This is
            // a safety feature — `-` won't nuke a whole chapter by accident.
            KeyCode::Char('D') | KeyCode::Char('d') if plain => self.delete_branch_only(),
            KeyCode::Char('-') if plain => self.delete_paragraph_only(),

            _ if self.keymap.page_up.matches(&key) => self.move_cursor(-10),
            _ if self.keymap.page_down.matches(&key) => self.move_cursor(10),
            _ => {}
        }
        Ok(false)
    }

    fn delete_branch_only(&mut self) {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            self.status = "nothing selected".into();
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            return;
        };
        if node.kind == NodeKind::Paragraph {
            self.status = format!(
                "`{}` is a paragraph — press `-` (or Ctrl+Shift+D) to delete it",
                node.title
            );
            return;
        }
        self.open_delete_modal();
    }

    fn delete_paragraph_only(&mut self) {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            self.status = "nothing selected".into();
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            return;
        };
        if node.kind != NodeKind::Paragraph {
            self.status = format!(
                "`{}` is a {} — press `D` (or Ctrl+Shift+D) to delete it",
                node.title,
                node.kind.as_str()
            );
            return;
        }
        self.open_delete_modal();
    }

    fn handle_editor_key(&mut self, key: KeyEvent) -> Result<bool> {
        if self.opened.is_none() {
            if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q')) {
                return Ok(self.request_quit());
            }
            return Ok(false);
        }

        // Stamp last_activity so idle autosave only fires after the user
        // actually pauses. Done first so every branch below benefits.
        if let Some(doc) = self.opened.as_mut() {
            doc.last_activity = std::time::Instant::now();
        }

        // F5 creates a snapshot of the current paragraph; F6 opens the picker.
        // Function keys are rarely intercepted by terminals or multiplexers.
        if matches!(key.code, KeyCode::F(5)) {
            self.create_snapshot_of_current();
            return Ok(false);
        }
        if matches!(key.code, KeyCode::F(6)) {
            self.open_snapshot_picker();
            return Ok(false);
        }
        // F3 opens the file-load dialog: pick a file, its content replaces
        // the editor buffer (and marks dirty).
        if matches!(key.code, KeyCode::F(3)) {
            self.open_file_picker(PickerContext::EditorLoad);
            return Ok(false);
        }

        // Alt+arrows enter / extend vertical-block selection. Alt+C copies it.
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        if alt {
            match key.code {
                KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
                    if let Some(doc) = self.opened.as_mut() {
                        if doc.block_anchor.is_none() {
                            doc.block_anchor = Some(doc.textarea.cursor());
                        }
                        match key.code {
                            KeyCode::Up => doc.textarea.move_cursor(CursorMove::Up),
                            KeyCode::Down => doc.textarea.move_cursor(CursorMove::Down),
                            KeyCode::Left => doc.textarea.move_cursor(CursorMove::Back),
                            KeyCode::Right => doc.textarea.move_cursor(CursorMove::Forward),
                            _ => {}
                        }
                    }
                    return Ok(false);
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    self.block_copy();
                    return Ok(false);
                }
                _ => {}
            }
        }

        if matches!(key.code, KeyCode::Esc) {
            if let Some(doc) = self.opened.as_mut() {
                if doc.block_anchor.is_some() {
                    doc.block_anchor = None;
                    return Ok(false);
                }
            }
            self.focus = Focus::Tree;
            return Ok(false);
        }

        // Any key other than Alt+arrows clears block selection state.
        if let Some(doc) = self.opened.as_mut() {
            if doc.block_anchor.is_some() && !alt {
                doc.block_anchor = None;
            }
        }

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        // Plain arrow keys + Home/End/PageUp/PageDown. `input_without_shortcuts`
        // (which we use to silence tui-textarea's emacs defaults) also treats
        // these as shortcuts and drops them — so we route them ourselves.
        // Shift extends a linear selection; plain cancels any selection.
        if !alt && !ctrl {
            let cmove = match key.code {
                KeyCode::Up => Some(CursorMove::Up),
                KeyCode::Down => Some(CursorMove::Down),
                KeyCode::Left => Some(CursorMove::Back),
                KeyCode::Right => Some(CursorMove::Forward),
                KeyCode::Home => Some(CursorMove::Head),
                KeyCode::End => Some(CursorMove::End),
                KeyCode::PageUp => Some(CursorMove::ParagraphBack),
                KeyCode::PageDown => Some(CursorMove::ParagraphForward),
                _ => None,
            };
            if let Some(cmove) = cmove {
                if let Some(doc) = self.opened.as_mut() {
                    if shift {
                        if doc.textarea.selection_range().is_none() {
                            doc.textarea.start_selection();
                        }
                    } else {
                        doc.textarea.cancel_selection();
                    }
                    doc.textarea.move_cursor(cmove);
                }
                return Ok(false);
            }
        }

        // Modern conventional shortcuts: intercept before reaching the
        // textarea so emacs-style defaults don't fire.
        if ctrl {
            // Ctrl+Shift+Z → redo. Also accept Ctrl+Y.
            if shift && matches!(key.code, KeyCode::Char('z') | KeyCode::Char('Z')) {
                if let Some(doc) = self.opened.as_mut() {
                    doc.textarea.redo();
                }
                return Ok(false);
            }
            match key.code {
                KeyCode::Char('z') | KeyCode::Char('Z') if !shift => {
                    if let Some(doc) = self.opened.as_mut() {
                        doc.textarea.undo();
                    }
                    return Ok(false);
                }
                KeyCode::Char('y') | KeyCode::Char('Y') if !shift => {
                    if let Some(doc) = self.opened.as_mut() {
                        doc.textarea.redo();
                    }
                    return Ok(false);
                }
                KeyCode::Char('x') | KeyCode::Char('X') if !shift => {
                    self.editor_cut();
                    return Ok(false);
                }
                KeyCode::Char('c') | KeyCode::Char('C') if !shift => {
                    self.editor_copy();
                    return Ok(false);
                }
                KeyCode::Char('v') | KeyCode::Char('V') if !shift => {
                    self.editor_paste();
                    return Ok(false);
                }
                KeyCode::Char('a') | KeyCode::Char('A') if !shift => {
                    self.editor_select_all();
                    return Ok(false);
                }
                KeyCode::Home => {
                    if let Some(doc) = self.opened.as_mut() {
                        doc.textarea.move_cursor(CursorMove::Top);
                    }
                    return Ok(false);
                }
                KeyCode::End => {
                    if let Some(doc) = self.opened.as_mut() {
                        doc.textarea.move_cursor(CursorMove::Bottom);
                    }
                    return Ok(false);
                }
                KeyCode::Left => {
                    if let Some(doc) = self.opened.as_mut() {
                        doc.textarea.move_cursor(CursorMove::WordBack);
                    }
                    return Ok(false);
                }
                KeyCode::Right => {
                    if let Some(doc) = self.opened.as_mut() {
                        doc.textarea.move_cursor(CursorMove::WordForward);
                    }
                    return Ok(false);
                }
                KeyCode::Backspace => {
                    if let Some(doc) = self.opened.as_mut() {
                        if doc.textarea.delete_word() {
                            doc.dirty = true;
                        }
                    }
                    return Ok(false);
                }
                _ => {}
            }
        }

        // Everything else: pass to textarea WITHOUT its emacs-style defaults,
        // so plain typing/arrows/Home/End/PageUp/PageDown/Shift+arrows still
        // work but Ctrl+letter combinations don't get hijacked.
        if let Some(doc) = self.opened.as_mut() {
            let input: tui_textarea::Input = key.into();
            if doc.textarea.input_without_shortcuts(input) {
                doc.dirty = true;
            }
        }
        Ok(false)
    }

    fn editor_copy(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        doc.textarea.copy();
        let text = doc.textarea.yank_text();
        if !text.is_empty() {
            if let Some(cb) = self.clipboard.as_mut() {
                let _ = cb.set_text(text);
            }
        }
    }

    fn editor_cut(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        if doc.textarea.cut() {
            let text = doc.textarea.yank_text();
            if !text.is_empty() {
                if let Some(cb) = self.clipboard.as_mut() {
                    let _ = cb.set_text(text);
                }
            }
            doc.dirty = true;
        }
    }

    fn editor_paste(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        if let Some(cb) = self.clipboard.as_mut() {
            if let Ok(text) = cb.get_text() {
                doc.textarea.set_yank_text(text);
            }
        }
        if doc.textarea.paste() {
            doc.dirty = true;
        }
    }

    /// Copy the current rectangular selection to the system clipboard
    /// (falling back to tui-textarea's yank buffer). The rectangle is the
    /// inclusive range from `block_anchor` to the current cursor.
    fn block_copy(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            return;
        };
        let Some(anchor) = doc.block_anchor else {
            return;
        };
        let block = BlockSelection::from_anchor_and_cursor(anchor, doc.textarea.cursor());
        let lines = doc.textarea.lines();
        let mut out = String::new();
        for r in block.row_min..=block.row_max {
            if let Some(line) = lines.get(r) {
                let chars: Vec<char> = line.chars().collect();
                let s = block.col_min.min(chars.len());
                let e = (block.col_max + 1).min(chars.len());
                let piece: String = chars[s..e].iter().collect();
                out.push_str(&piece);
            }
            if r < block.row_max {
                out.push('\n');
            }
        }
        // Push to system clipboard if available.
        if let Some(cb) = self.clipboard.as_mut() {
            let _ = cb.set_text(out.clone());
        }
        if let Some(doc) = self.opened.as_mut() {
            doc.textarea.set_yank_text(out);
            doc.block_anchor = None;
        }
        self.status = "copied rectangular block to clipboard".into();
    }

    fn current_block(&self) -> Option<BlockSelection> {
        let doc = self.opened.as_ref()?;
        let anchor = doc.block_anchor?;
        Some(BlockSelection::from_anchor_and_cursor(
            anchor,
            doc.textarea.cursor(),
        ))
    }

    fn editor_select_all(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        doc.textarea.move_cursor(CursorMove::Top);
        doc.textarea.start_selection();
        doc.textarea.move_cursor(CursorMove::Bottom);
        // CursorMove::Bottom lands at (last_row, 0). Without this End move
        // the selection would exclude the last line's content entirely.
        doc.textarea.move_cursor(CursorMove::End);
    }

    fn handle_passive_key(&mut self, key: KeyEvent) -> Result<bool> {
        // When the AI pane has a completed inference and is focused, single-
        // letter keys apply the result to the editor.
        if self.focus == Focus::Ai && self.inference_done_with_text() {
            match key.code {
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    self.apply_inference(InferenceAction::Replace);
                    return Ok(false);
                }
                KeyCode::Char('i') | KeyCode::Char('I') => {
                    self.apply_inference(InferenceAction::Insert);
                    return Ok(false);
                }
                KeyCode::Char('t') | KeyCode::Char('T') => {
                    self.apply_inference(InferenceAction::Top);
                    return Ok(false);
                }
                KeyCode::Char('b') | KeyCode::Char('B') => {
                    self.apply_inference(InferenceAction::Bottom);
                    return Ok(false);
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    self.apply_inference(InferenceAction::CopyOnly);
                    return Ok(false);
                }
                _ => {}
            }
        }

        if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q')) {
            return Ok(self.request_quit());
        }
        Ok(false)
    }

    /// Centralized exit gate. If the open paragraph is dirty, autosave it
    /// before returning true. If the save fails, abort the exit and leave a
    /// status message so the user can recover. Called from every quit chord
    /// (Ctrl+Q, plain q in Tree / Editor-empty / AI).
    fn request_quit(&mut self) -> bool {
        if self.opened.as_ref().is_some_and(|d| d.dirty) {
            // save_current writes its own status. If it can't save, doc.dirty
            // stays true and we refuse to quit so the user can see the error
            // and recover.
            let _ = self.save_current();
            if self.opened.as_ref().is_some_and(|d| d.dirty) {
                return false;
            }
        }
        true
    }

    fn inference_done_with_text(&self) -> bool {
        matches!(
            self.inference.as_ref().map(|i| (&i.status, i.response.is_empty())),
            Some((InferenceStatus::Done, false))
        )
    }

    fn apply_inference(&mut self, action: InferenceAction) {
        let Some(inf) = self.inference.as_ref() else {
            return;
        };
        let text = inf.response.clone();
        if matches!(action, InferenceAction::CopyOnly) {
            if let Some(cb) = self.clipboard.as_mut() {
                let _ = cb.set_text(text.clone());
            }
            self.status = "copied AI result to clipboard".into();
            return;
        }
        let Some(doc) = self.opened.as_mut() else {
            self.status = "no paragraph open — apply needs a focused paragraph".into();
            return;
        };
        match action {
            InferenceAction::Replace => {
                if doc.textarea.selection_range().is_some() {
                    doc.textarea.cut();
                } else {
                    // No selection: replace the whole document.
                    use tui_textarea::CursorMove;
                    doc.textarea.move_cursor(CursorMove::Top);
                    doc.textarea.start_selection();
                    doc.textarea.move_cursor(CursorMove::Bottom);
                    doc.textarea.cut();
                }
                doc.textarea.set_yank_text(text);
                doc.textarea.paste();
            }
            InferenceAction::Insert => {
                doc.textarea.set_yank_text(text);
                doc.textarea.paste();
            }
            InferenceAction::Top => {
                use tui_textarea::CursorMove;
                doc.textarea.move_cursor(CursorMove::Top);
                doc.textarea.move_cursor(CursorMove::Head);
                doc.textarea.set_yank_text(format!("{text}\n\n"));
                doc.textarea.paste();
            }
            InferenceAction::Bottom => {
                use tui_textarea::CursorMove;
                doc.textarea.move_cursor(CursorMove::Bottom);
                doc.textarea.move_cursor(CursorMove::End);
                doc.textarea.set_yank_text(format!("\n\n{text}"));
                doc.textarea.paste();
            }
            InferenceAction::CopyOnly => unreachable!(),
        }
        doc.dirty = true;
        self.status = format!("applied AI result ({})", action.label());
        self.focus = Focus::Editor;
    }

    fn handle_input_key(&mut self, key: KeyEvent, is_search: bool) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                if is_search && self.show_results_overlay {
                    self.show_results_overlay = false;
                } else if !is_search && self.show_prompt_picker {
                    self.show_prompt_picker = false;
                } else {
                    self.show_results_overlay = false;
                    self.show_prompt_picker = false;
                    // Return to the Editor pane when a paragraph is open,
                    // otherwise fall back to Tree. Saves a Tab press in the
                    // common write-search-write workflow.
                    self.focus = if self.opened.is_some() {
                        Focus::Editor
                    } else {
                        Focus::Tree
                    };
                }
            }
            KeyCode::Enter => {
                if is_search {
                    if self.show_results_overlay && !self.results.is_empty() {
                        let id = self.results[self.results_cursor].id;
                        self.open_search_result(id);
                    } else {
                        self.run_search();
                    }
                } else if self.show_prompt_picker {
                    self.commit_prompt_pick();
                } else {
                    self.start_inference();
                }
            }
            KeyCode::Up if is_search && self.show_results_overlay => {
                if !self.results.is_empty() && self.results_cursor > 0 {
                    self.results_cursor -= 1;
                }
            }
            KeyCode::Down if is_search && self.show_results_overlay => {
                if !self.results.is_empty()
                    && self.results_cursor + 1 < self.results.len()
                {
                    self.results_cursor += 1;
                }
            }
            KeyCode::Up if !is_search && self.show_prompt_picker => {
                if self.prompt_picker_cursor > 0 {
                    self.prompt_picker_cursor -= 1;
                }
            }
            KeyCode::Down if !is_search && self.show_prompt_picker => {
                let n = self.prompt_picker_matches().len();
                if n > 0 && self.prompt_picker_cursor + 1 < n {
                    self.prompt_picker_cursor += 1;
                }
            }
            KeyCode::Tab if !is_search && self.show_prompt_picker => {
                self.commit_prompt_pick();
            }
            KeyCode::Backspace => {
                self.current_input(is_search).backspace();
                if is_search {
                    self.show_results_overlay = false;
                }
            }
            KeyCode::Delete => {
                self.current_input(is_search).delete();
                if is_search {
                    self.show_results_overlay = false;
                }
            }
            KeyCode::Left => self.current_input(is_search).move_left(),
            KeyCode::Right => self.current_input(is_search).move_right(),
            KeyCode::Home => self.current_input(is_search).move_home(),
            KeyCode::End => self.current_input(is_search).move_end(),
            KeyCode::Char(c) => {
                let mut residual = key.modifiers;
                residual.remove(KeyModifiers::SHIFT);
                if residual.is_empty() {
                    let final_c = if key.modifiers.contains(KeyModifiers::SHIFT)
                        && c.is_ascii_alphabetic()
                    {
                        c.to_ascii_uppercase()
                    } else {
                        c
                    };
                    self.current_input(is_search).insert_char(final_c);
                    if is_search {
                        self.show_results_overlay = false;
                    } else {
                        self.refresh_prompt_picker();
                    }
                }
            }
            _ => {}
        }
        // After Backspace/Delete in the AI input, also refresh the picker.
        if !is_search {
            self.refresh_prompt_picker();
        }
        Ok(false)
    }

    fn refresh_prompt_picker(&mut self) {
        let s = self.ai_input.as_str();
        self.show_prompt_picker = s.starts_with('/');
        self.prompt_picker_cursor = self
            .prompt_picker_cursor
            .min(self.prompt_picker_matches().len().saturating_sub(1));
    }

    fn prompt_picker_matches(&self) -> Vec<&Prompt> {
        let q = self.ai_input.as_str();
        let filter = q.strip_prefix('/').unwrap_or("").trim().to_lowercase();
        self.prompts
            .prompts
            .iter()
            .filter(|p| {
                filter.is_empty()
                    || p.name.to_lowercase().contains(&filter)
                    || p.description.to_lowercase().contains(&filter)
            })
            .collect()
    }

    fn commit_prompt_pick(&mut self) {
        let matches = self.prompt_picker_matches();
        let Some(picked) = matches.get(self.prompt_picker_cursor).copied().cloned() else {
            self.status = "no matching prompt".into();
            return;
        };
        // Render template now so the user sees what's going.
        let body = self.render_template(&picked.template);
        self.ai_input.clear();
        for c in body.chars() {
            self.ai_input.insert_char(c);
        }
        self.show_prompt_picker = false;
        self.status = format!("loaded prompt `{}` — Enter to send", picked.name);
    }

    fn render_template(&self, template: &str) -> String {
        let selection = self.current_selection_or_paragraph();
        let context = self.current_context_breadcrumb();
        template
            .replace("{{selection}}", &selection)
            .replace("{{context}}", &context)
            .trim()
            .to_string()
    }

    fn current_selection_or_paragraph(&self) -> String {
        let Some(doc) = self.opened.as_ref() else {
            return String::new();
        };
        if let Some(((r1, c1), (r2, c2))) = doc.textarea.selection_range() {
            let lines = doc.textarea.lines();
            return slice_lines(lines, r1, c1, r2, c2);
        }
        doc.textarea.lines().join("\n")
    }

    fn current_context_breadcrumb(&self) -> String {
        let Some(doc) = self.opened.as_ref() else {
            return String::new();
        };
        let Some(node) = self.hierarchy.get(doc.id) else {
            return String::new();
        };
        let mut parts: Vec<String> = self
            .hierarchy
            .ancestors(node)
            .into_iter()
            .map(|n| n.title.clone())
            .collect();
        parts.push(node.title.clone());
        parts.join(" › ")
    }

    fn start_inference(&mut self) {
        let raw = self.ai_input.as_str().trim().to_string();
        if raw.is_empty() {
            self.status = "empty prompt".into();
            return;
        }
        let prompt_text = if raw.starts_with('/') {
            // Resolve `/name [extra args]` form by name lookup.
            let after = raw.trim_start_matches('/').trim();
            match self.prompts.find(after) {
                Some(p) => self.render_template(&p.template.clone()),
                None => {
                    self.status =
                        format!("no prompt `{after}` — type `/` to see the list");
                    return;
                }
            }
        } else {
            raw
        };

        let (model, _env_var) = match self.ai.resolve_provider(&self.cfg.llm, None) {
            Ok(pair) => pair,
            Err(e) => {
                self.status = e.to_string();
                return;
            }
        };
        let model = model.to_string();
        let provider = self.ai.default_provider.clone();

        let rx = spawn_chat_stream(self.ai.client.clone(), model.clone(), None, prompt_text);

        self.inference = Some(Inference {
            provider: provider.clone(),
            model,
            response: String::new(),
            status: InferenceStatus::Streaming,
            rx,
            started_at: std::time::Instant::now(),
        });
        self.focus = Focus::Ai;
        self.status = format!("streaming from {provider}…");
        // Clear the prompt so the next inference starts fresh.
        self.ai_input.clear();
    }

    fn run_search(&mut self) {
        let query = self.search_input.as_str().trim().to_string();
        if query.is_empty() {
            self.show_results_overlay = false;
            self.results.clear();
            self.status = "empty query".into();
            return;
        }
        match self.store.search_text(&query, 10) {
            Ok(raw) => {
                self.results = raw.iter().filter_map(SearchHit::parse).collect();
                self.results_cursor = 0;
                self.show_results_overlay = true;
                self.status = format!(
                    "`{}` → {} result(s) · ↑/↓ to navigate · Enter to open · Esc to close",
                    query,
                    self.results.len()
                );
            }
            Err(e) => {
                self.status = format!("search failed: {e}");
            }
        }
    }

    fn open_search_result(&mut self, id: Uuid) {
        if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == id) {
            self.tree_cursor = i;
        }
        let node = match self.hierarchy.get(id).cloned() {
            Some(n) => n,
            None => {
                self.status = format!("hit {id} no longer in hierarchy — try a fresh search");
                self.show_results_overlay = false;
                return;
            }
        };
        self.show_results_overlay = false;
        if matches!(node.kind, NodeKind::Paragraph) {
            if let Err(e) = self.load_paragraph(&node) {
                self.status = format!("open failed: {e}");
            }
        } else {
            self.focus = Focus::Tree;
            self.status = format!(
                "`{}` is a {} — its paragraph children are editable",
                node.title,
                node.kind.as_str()
            );
        }
    }

    fn current_input(&mut self, is_search: bool) -> &mut TextInput {
        if is_search {
            &mut self.search_input
        } else {
            &mut self.ai_input
        }
    }

    fn move_cursor(&mut self, delta: i32) {
        if self.rows.is_empty() {
            return;
        }
        let new =
            (self.tree_cursor as i32 + delta).clamp(0, self.rows.len() as i32 - 1) as usize;
        self.tree_cursor = new;
    }

    // -------- modal -------------------------------------------------------

    fn open_add_modal(&mut self, kind: NodeKind) {
        self.open_add_modal_inner(kind, InsertPosition::End);
    }

    /// Insert-after variant: walks up from the tree cursor to find a node of
    /// the same `kind` as the one being added; if found, the new node will be
    /// placed immediately after it. Falls back to append-at-end if no
    /// same-kind ancestor exists (e.g. pressing P with cursor on a book).
    fn open_add_modal_after(&mut self, kind: NodeKind) {
        let cursor_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
        let anchor = cursor_id.and_then(|id| {
            let mut cur = Some(id);
            while let Some(c) = cur {
                let node = self.hierarchy.get(c)?;
                if node.kind == kind {
                    return Some(node.id);
                }
                cur = node.parent_id;
            }
            None
        });
        let position = match anchor {
            Some(id) => InsertPosition::After(id),
            None => InsertPosition::End,
        };
        self.open_add_modal_inner(kind, position);
    }

    fn open_add_modal_inner(&mut self, kind: NodeKind, position: InsertPosition) {
        // For After(anchor), the parent is anchor.parent_id (always valid
        // because the anchor's a same-kind node). For End, walk up to find a
        // valid parent.
        let parent_node = match position {
            InsertPosition::After(anchor_id) => match self.hierarchy.get(anchor_id) {
                Some(anchor) => anchor.parent_id.and_then(|pid| self.hierarchy.get(pid)),
                None => {
                    self.status = format!("anchor for insert-after vanished from hierarchy");
                    return;
                }
            },
            InsertPosition::End => {
                let cursor_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
                match self.hierarchy.pick_parent_for(&self.cfg, cursor_id, kind) {
                    Ok(p) => p,
                    Err(e) => {
                        self.status = format!("can't add {}: {e}", kind.as_str());
                        return;
                    }
                }
            }
        };
        let parent_label = parent_node.map_or_else(
            || "<books root>".to_string(),
            |n| self.hierarchy.slug_path(n),
        );
        let parent_id = parent_node.map(|n| n.id);
        self.modal = Modal::Adding {
            kind,
            parent_id,
            parent_label,
            input: TextInput::new(),
            position,
        };
    }

    fn move_current(&mut self, dir: MoveDir) {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            self.status = "nothing selected".into();
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            return;
        };
        let siblings = self.hierarchy.children_of(node.parent_id);
        let pos = match siblings.iter().position(|n| n.id == id) {
            Some(p) => p,
            None => return,
        };
        let other_pos = match dir {
            MoveDir::Up => {
                if pos == 0 {
                    self.status = format!("`{}` is already first", node.slug);
                    return;
                }
                pos - 1
            }
            MoveDir::Down => {
                if pos + 1 >= siblings.len() {
                    self.status = format!("`{}` is already last", node.slug);
                    return;
                }
                pos + 1
            }
        };
        let other_id = siblings[other_pos].id;
        match self.store.swap_siblings(&self.hierarchy, id, other_id) {
            Ok(()) => {
                self.status = format!(
                    "moved `{}` {}",
                    node.slug,
                    match dir {
                        MoveDir::Up => "up",
                        MoveDir::Down => "down",
                    }
                );
                self.reload_hierarchy();
                // Keep the cursor on the same node after the swap.
                if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == id) {
                    self.tree_cursor = i;
                }
            }
            Err(e) => {
                self.status = format!("move failed: {e}");
            }
        }
    }

    fn create_snapshot_of_current(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "no paragraph open".into();
            return;
        };
        let body = doc.textarea.lines().join("\n");
        let id = doc.id;
        let Some(node) = self.hierarchy.get(id).cloned() else {
            self.status = "node missing from hierarchy".into();
            return;
        };
        match self.store.create_snapshot(&node, body.as_bytes()) {
            Ok(snap_id) => {
                let n_snaps = self
                    .store
                    .list_snapshots(id)
                    .map(|v| v.len())
                    .unwrap_or(0);
                self.status = format!(
                    "snapshot {} created ({} total) — F6 to view",
                    snap_id.simple(),
                    n_snaps
                );
            }
            Err(e) => {
                self.status = format!("snapshot failed: {e}");
            }
        }
    }

    fn open_file_picker(&mut self, context: PickerContext) {
        let root = std::env::current_dir().unwrap_or_else(|_| self.layout.root.clone());
        self.modal = Modal::FilePicker(FilePicker::new(root, context));
    }

    fn commit_file_pick(&mut self) {
        let (path, is_dir, context) = match &self.modal {
            Modal::FilePicker(p) => match p.current() {
                Some(entry) => (entry.path.clone(), entry.is_dir, p.context),
                None => {
                    self.modal = Modal::None;
                    return;
                }
            },
            _ => return,
        };

        match (context, is_dir) {
            (PickerContext::EditorLoad, true) => {
                self.status =
                    "Editor F3 needs a file, not a directory — Enter on a file".into();
            }
            (PickerContext::EditorLoad, false) => {
                self.load_file_into_editor(&path);
                self.modal = Modal::None;
            }
            (PickerContext::TreeInsertOrImport, false) => {
                self.import_single_file(&path);
                self.modal = Modal::None;
            }
            (PickerContext::TreeInsertOrImport, true) => {
                self.import_directory_tree(&path);
                self.modal = Modal::None;
            }
        }
    }

    fn load_file_into_editor(&mut self, path: &std::path::Path) {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.status = format!("read {}: {e}", path.display());
                return;
            }
        };
        let body = String::from_utf8_lossy(&bytes).into_owned();
        let Some(doc) = self.opened.as_mut() else {
            self.status =
                "no paragraph open — open one first, then F3 to replace its body".into();
            return;
        };
        let lines: Vec<String> = if body.is_empty() {
            vec![String::new()]
        } else {
            body.split('\n').map(String::from).collect()
        };
        let mut new_textarea = TextArea::new(lines);
        new_textarea.set_cursor_line_style(Style::default().add_modifier(Modifier::REVERSED));
        new_textarea.set_line_number_style(Style::default().fg(Color::DarkGray));
        doc.textarea = new_textarea;
        doc.dirty = true;
        doc.scroll_row = 0;
        doc.scroll_col = 0;
        doc.last_activity = std::time::Instant::now();
        self.focus = Focus::Editor;
        self.status = format!("loaded `{}` — bold marks the change vs saved", path.display());
    }

    fn import_single_file(&mut self, path: &std::path::Path) {
        // Place the new paragraph after the tree cursor's same-kind ancestor
        // if there is one (so it's "insert after current"). Falls back to
        // append-at-end under the nearest valid parent.
        let cursor_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
        let anchor = cursor_id.and_then(|id| {
            let mut cur = Some(id);
            while let Some(c) = cur {
                let node = self.hierarchy.get(c)?;
                if node.kind == NodeKind::Paragraph {
                    return Some(node.id);
                }
                cur = node.parent_id;
            }
            None
        });
        let position = match anchor {
            Some(id) => InsertPosition::After(id),
            None => InsertPosition::End,
        };
        let parent = match position {
            InsertPosition::After(anchor_id) => self
                .hierarchy
                .get(anchor_id)
                .and_then(|n| n.parent_id)
                .and_then(|pid| self.hierarchy.get(pid))
                .cloned(),
            InsertPosition::End => {
                match self
                    .hierarchy
                    .pick_parent_for(&self.cfg, cursor_id, NodeKind::Paragraph)
                {
                    Ok(p) => p.cloned(),
                    Err(e) => {
                        self.status = format!("can't import here: {e}");
                        return;
                    }
                }
            }
        };

        let title = derive_paragraph_title_from_path(path);
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.status = format!("read {}: {e}", path.display());
                return;
            }
        };

        let created = match self.store.create_node(
            &self.cfg,
            &self.hierarchy,
            NodeKind::Paragraph,
            &title,
            parent.as_ref(),
            None,
            position,
        ) {
            Ok(n) => n,
            Err(e) => {
                self.status = format!("create paragraph: {e}");
                return;
            }
        };

        // Replace the templated body with the actual file content.
        let Some(rel) = &created.file else {
            self.status = "created paragraph has no file path — bug?".into();
            return;
        };
        let abs = self.layout.root.join(rel);
        if let Err(e) = std::fs::write(&abs, &bytes) {
            self.status = format!("write {}: {e}", abs.display());
            return;
        }
        let mut node = created.clone();
        if let Err(e) = self.store.update_paragraph_content(&mut node, &bytes) {
            self.status = format!("update: {e}");
            return;
        }
        let _ = self.store.sync();
        self.status = format!("imported `{}` as paragraph", path.display());
        self.reload_hierarchy();
        if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == created.id) {
            self.tree_cursor = i;
        }
    }

    fn import_directory_tree(&mut self, root: &std::path::Path) {
        // The top-level dir's "kind" adapts to where the tree cursor sits:
        //   Book      → top dir becomes a Chapter
        //   Chapter   → top dir becomes a Subchapter
        //   Subchapter→ top dir becomes a Subchapter (requires unbounded)
        //   Paragraph → reject
        // Same rule applies at every recursion level: the new branch's kind
        // is "one level deeper than its parent", so a nested directory tree
        // walks down the hierarchy with it.
        let cursor_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
        let parent_id = cursor_id.and_then(|id| {
            // If cursor is on a paragraph we walk up to its enclosing branch
            // — easier than telling the user "move first".
            let mut cur = Some(id);
            while let Some(c) = cur {
                let node = self.hierarchy.get(c)?;
                if node.kind != NodeKind::Paragraph {
                    return Some(node.id);
                }
                cur = node.parent_id;
            }
            None
        });

        let mut counts = ImportCounts::default();
        let result = self.import_dir_recursive(root, parent_id, &mut counts);

        // Always reload — even on partial failure the new branches/paragraphs
        // that DID get created should be visible in the tree.
        self.reload_hierarchy();

        match result {
            Ok(()) => {
                self.status = format!(
                    "imported {}: {} branch(es), {} paragraph(s)",
                    root.display(),
                    counts.branches,
                    counts.paragraphs
                );
            }
            Err(e) => {
                self.status = format!(
                    "partial import of {}: {} branch(es), {} paragraph(s) — stopped at: {e}",
                    root.display(),
                    counts.branches,
                    counts.paragraphs
                );
            }
        }
    }

    fn import_dir_recursive(
        &mut self,
        source: &std::path::Path,
        parent_id: Option<Uuid>,
        counts: &mut ImportCounts,
    ) -> InkResult<()> {
        // Resolve the parent against a freshly loaded hierarchy so prior
        // creates in this import are visible.
        let hierarchy = Hierarchy::load(&self.store)?;
        let parent = parent_id.and_then(|id| hierarchy.get(id).cloned());

        let kind = match parent.as_ref().map(|p| p.kind) {
            None => NodeKind::Book,
            Some(NodeKind::Book) => NodeKind::Chapter,
            Some(NodeKind::Chapter) => NodeKind::Subchapter,
            Some(NodeKind::Subchapter) => {
                if self.cfg.hierarchy.unbounded_subchapters {
                    NodeKind::Subchapter
                } else {
                    return Err(Error::Store(format!(
                        "max hierarchy depth reached at `{}` — enable `hierarchy.unbounded_subchapters: true` in inkhaven.hjson to allow deeper nesting",
                        source.file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("(unnamed)")
                    )));
                }
            }
            Some(NodeKind::Paragraph) => {
                return Err(Error::Store(
                    "can't import under a paragraph — move cursor to a branch first".into(),
                ));
            }
        };

        let title = source
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("imported")
            .to_string();
        let created = self.store.create_node(
            &self.cfg,
            &hierarchy,
            kind,
            &title,
            parent.as_ref(),
            None,
            InsertPosition::End,
        )?;
        counts.branches += 1;
        let created_id = created.id;

        let mut children: Vec<std::path::PathBuf> = {
            let Ok(rd) = std::fs::read_dir(source) else {
                return Ok(());
            };
            let mut entries: Vec<_> = rd
                .filter_map(Result::ok)
                .filter(|e| {
                    e.file_name()
                        .to_str()
                        .map(|s| !s.starts_with('.'))
                        .unwrap_or(true)
                })
                .collect();
            entries.sort_by(|a, b| {
                let a_dir = a.path().is_dir();
                let b_dir = b.path().is_dir();
                match (a_dir, b_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.file_name().cmp(&b.file_name()),
                }
            });
            entries.into_iter().map(|e| e.path()).collect()
        };

        for child_path in children.drain(..) {
            if child_path.is_dir() {
                self.import_dir_recursive(&child_path, Some(created_id), counts)?;
            } else {
                self.import_file_as_paragraph_by_id(&child_path, created_id, counts)?;
            }
        }
        Ok(())
    }

    fn import_file_as_paragraph_by_id(
        &mut self,
        file: &std::path::Path,
        parent_id: Uuid,
        counts: &mut ImportCounts,
    ) -> InkResult<()> {
        let title = derive_paragraph_title_from_path(file);
        let bytes = std::fs::read(file).map_err(Error::Io)?;
        let hierarchy = Hierarchy::load(&self.store)?;
        let parent = hierarchy
            .get(parent_id)
            .cloned()
            .ok_or_else(|| Error::Store(format!("import: parent {parent_id} vanished")))?;
        let created = self.store.create_node(
            &self.cfg,
            &hierarchy,
            NodeKind::Paragraph,
            &title,
            Some(&parent),
            None,
            InsertPosition::End,
        )?;
        if let Some(rel) = &created.file {
            let abs = self.layout.root.join(rel);
            std::fs::write(&abs, &bytes).map_err(Error::Io)?;
            let mut node = created.clone();
            self.store.update_paragraph_content(&mut node, &bytes)?;
        }
        counts.paragraphs += 1;
        Ok(())
    }

    fn open_rename_modal(&mut self) {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            self.status = "nothing selected to rename".into();
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            return;
        };
        let mut input = TextInput::new();
        for c in node.title.chars() {
            input.insert_char(c);
        }
        self.modal = Modal::Renaming {
            node_id: id,
            kind: node.kind,
            input,
        };
    }

    fn commit_rename(&mut self) {
        let (node_id, new_title) = match &self.modal {
            Modal::Renaming { node_id, input, .. } => {
                (*node_id, input.as_str().trim().to_string())
            }
            _ => return,
        };
        if new_title.is_empty() {
            self.status = "rename: title can't be empty — type one or Esc to cancel".into();
            return;
        }
        match self.store.rename_node(&self.hierarchy, node_id, &new_title) {
            Ok(()) => {
                // Refresh editor's title if the renamed node is the open one.
                if let Some(doc) = self.opened.as_mut() {
                    if doc.id == node_id {
                        doc.title = new_title.clone();
                    }
                }
                self.modal = Modal::None;
                self.status = format!("renamed to `{new_title}`");
                self.reload_hierarchy();
            }
            Err(e) => {
                self.status = format!("rename failed: {e}");
            }
        }
    }

    fn open_snapshot_picker(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "no paragraph open".into();
            return;
        };
        let id = doc.id;
        let title = doc.title.clone();
        match self.store.list_snapshots(id) {
            Ok(snapshots) => {
                if snapshots.is_empty() {
                    self.status =
                        format!("no snapshots yet for `{title}` — press F5 to create one");
                    return;
                }
                self.modal = Modal::SnapshotPicker {
                    paragraph_id: id,
                    paragraph_title: title,
                    snapshots,
                    cursor: 0,
                };
            }
            Err(e) => {
                self.status = format!("snapshot list failed: {e}");
            }
        }
    }

    fn commit_snapshot_load(&mut self) {
        let (snap_id, when) = match &self.modal {
            Modal::SnapshotPicker {
                snapshots, cursor, ..
            } => {
                let Some(snap) = snapshots.get(*cursor) else {
                    self.modal = Modal::None;
                    return;
                };
                (snap.id, snap.created_at)
            }
            _ => return,
        };
        let content = match self.store.snapshot_content(snap_id) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => {
                self.status = "snapshot has no body".into();
                self.modal = Modal::None;
                return;
            }
            Err(e) => {
                self.status = format!("snapshot load failed: {e}");
                self.modal = Modal::None;
                return;
            }
        };

        let body = String::from_utf8_lossy(&content).into_owned();
        let Some(doc) = self.opened.as_mut() else {
            self.modal = Modal::None;
            return;
        };
        let lines: Vec<String> = if body.is_empty() {
            vec![String::new()]
        } else {
            body.split('\n').map(String::from).collect()
        };
        let mut new_textarea = TextArea::new(lines);
        new_textarea.set_cursor_line_style(Style::default().add_modifier(Modifier::REVERSED));
        new_textarea.set_line_number_style(Style::default().fg(Color::DarkGray));
        doc.textarea = new_textarea;
        doc.dirty = true;
        doc.scroll_row = 0;
        doc.scroll_col = 0;
        doc.last_activity = std::time::Instant::now();
        // saved_lines stays at the previously-saved on-disk version, so the
        // snapshot text shows as "added" (bold) until the user accepts it
        // by hitting Ctrl+S.
        self.modal = Modal::None;
        self.focus = Focus::Editor;
        self.status = format!(
            "loaded snapshot from {} — bold marks the change vs saved",
            when.format("%Y-%m-%d %H:%M:%S")
        );
    }

    fn open_delete_modal(&mut self) {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            self.status = "nothing selected to delete".into();
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            return;
        };
        let ids = self.hierarchy.collect_subtree(id);
        let descendant_count = ids.len().saturating_sub(1);
        self.modal = Modal::Deleting {
            root_id: id,
            root_kind: node.kind,
            title: node.title.clone(),
            descendant_count,
            ids,
        };
    }

    fn handle_modal_key(&mut self, key: KeyEvent) -> Result<bool> {
        if matches!(key.code, KeyCode::Esc) {
            self.modal = Modal::None;
            return Ok(false);
        }

        let is_adding = matches!(self.modal, Modal::Adding { .. });
        let is_deleting = matches!(self.modal, Modal::Deleting { .. });
        let is_snapshot = matches!(self.modal, Modal::SnapshotPicker { .. });
        let is_renaming = matches!(self.modal, Modal::Renaming { .. });
        let is_file_picker = matches!(self.modal, Modal::FilePicker(_));

        if is_file_picker {
            let mut commit = false;
            if let Modal::FilePicker(picker) = &mut self.modal {
                match key.code {
                    KeyCode::Up => picker.move_up(),
                    KeyCode::Down => picker.move_down(),
                    KeyCode::PageUp => picker.page_up(10),
                    KeyCode::PageDown => picker.page_down(10),
                    KeyCode::Home => picker.jump_first(),
                    KeyCode::End => picker.jump_last(),
                    KeyCode::Right => picker.expand(),
                    KeyCode::Left => picker.collapse_or_step_out(),
                    KeyCode::Enter => commit = true,
                    _ => {}
                }
            }
            if commit {
                self.commit_file_pick();
            }
            return Ok(false);
        }

        if is_renaming {
            let mut commit = false;
            if let Modal::Renaming { input, .. } = &mut self.modal {
                match key.code {
                    KeyCode::Enter => commit = true,
                    KeyCode::Backspace => input.backspace(),
                    KeyCode::Delete => input.delete(),
                    KeyCode::Left => input.move_left(),
                    KeyCode::Right => input.move_right(),
                    KeyCode::Home => input.move_home(),
                    KeyCode::End => input.move_end(),
                    KeyCode::Char(c) => {
                        let mut residual = key.modifiers;
                        residual.remove(KeyModifiers::SHIFT);
                        if residual.is_empty() {
                            let final_c = if key.modifiers.contains(KeyModifiers::SHIFT)
                                && c.is_ascii_alphabetic()
                            {
                                c.to_ascii_uppercase()
                            } else {
                                c
                            };
                            input.insert_char(final_c);
                        }
                    }
                    _ => {}
                }
            }
            if commit {
                self.commit_rename();
            }
            return Ok(false);
        }

        if is_snapshot {
            let mut commit = false;
            if let Modal::SnapshotPicker {
                snapshots, cursor, ..
            } = &mut self.modal
            {
                match key.code {
                    KeyCode::Up => {
                        if *cursor > 0 {
                            *cursor -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if *cursor + 1 < snapshots.len() {
                            *cursor += 1;
                        }
                    }
                    KeyCode::Home => *cursor = 0,
                    KeyCode::End => {
                        *cursor = snapshots.len().saturating_sub(1);
                    }
                    KeyCode::Enter => commit = true,
                    _ => {}
                }
            }
            if commit {
                self.commit_snapshot_load();
            }
            return Ok(false);
        }

        if is_adding {
            let mut commit = false;
            if let Modal::Adding { input, .. } = &mut self.modal {
                match key.code {
                    KeyCode::Enter => commit = true,
                    KeyCode::Backspace => input.backspace(),
                    KeyCode::Delete => input.delete(),
                    KeyCode::Left => input.move_left(),
                    KeyCode::Right => input.move_right(),
                    KeyCode::Home => input.move_home(),
                    KeyCode::End => input.move_end(),
                    KeyCode::Char(c) => {
                        let mut residual = key.modifiers;
                        residual.remove(KeyModifiers::SHIFT);
                        if residual.is_empty() {
                            // Some terminals report Shift+letter as lowercase
                            // char + SHIFT modifier; others as uppercase char
                            // + SHIFT. Normalize so capital letters always go
                            // into the buffer when Shift was held.
                            let final_c = if key.modifiers.contains(KeyModifiers::SHIFT)
                                && c.is_ascii_alphabetic()
                            {
                                c.to_ascii_uppercase()
                            } else {
                                c
                            };
                            input.insert_char(final_c);
                        }
                    }
                    _ => {}
                }
            }
            if commit {
                self.commit_add();
            }
        } else if is_deleting {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => self.commit_delete(),
                KeyCode::Char('n') | KeyCode::Char('N') => self.modal = Modal::None,
                _ => {}
            }
        }
        Ok(false)
    }

    fn commit_add(&mut self) {
        let (kind, parent_id, raw_title, position) = match &self.modal {
            Modal::Adding {
                kind,
                parent_id,
                input,
                position,
                ..
            } => (
                *kind,
                *parent_id,
                input.as_str().trim().to_string(),
                *position,
            ),
            _ => return,
        };

        // Paragraphs can be added with an empty title — they'll be given a
        // placeholder ("Untitled paragraph") that the next save replaces with
        // the first sentence of the body. Branches still require a title
        // because they have no content from which to derive one.
        let title = if raw_title.is_empty() {
            if kind == NodeKind::Paragraph {
                PARAGRAPH_PLACEHOLDER_TITLE.to_string()
            } else {
                self.status =
                    format!("a {} needs a title — type one or Esc to cancel", kind.as_str());
                return;
            }
        } else {
            raw_title
        };

        let parent = parent_id.and_then(|id| self.hierarchy.get(id)).cloned();
        match self.store.create_node(
            &self.cfg,
            &self.hierarchy,
            kind,
            &title,
            parent.as_ref(),
            None,
            position,
        ) {
            Ok(node) => {
                let new_id = node.id;
                self.modal = Modal::None;
                self.status = format!("added {} `{}`", kind.as_str(), node.title);
                self.reload_hierarchy();
                if let Some(i) = self.rows.iter().position(|(id, _)| *id == new_id) {
                    self.tree_cursor = i;
                }
            }
            Err(e) => {
                self.status = format!("add failed: {e}");
            }
        }
    }

    fn commit_delete(&mut self) {
        let (root_id, root_kind, ids, title) = match &self.modal {
            Modal::Deleting {
                root_id,
                root_kind,
                ids,
                title,
                ..
            } => (*root_id, *root_kind, ids.clone(), title.clone()),
            _ => return,
        };
        let root_node = match self.hierarchy.get(root_id) {
            Some(n) => n.clone(),
            None => {
                self.modal = Modal::None;
                self.status = "node already gone".into();
                return;
            }
        };
        let parent_id = root_node.parent_id;

        let fs_rel = match root_kind {
            NodeKind::Paragraph => root_node
                .file
                .as_ref()
                .map(std::path::PathBuf::from)
                .unwrap_or_default(),
            _ => self.hierarchy.fs_path(&root_node, &self.layout),
        };

        if let Err(e) = self.store.delete_subtree(&fs_rel, &ids) {
            self.status = format!("delete failed: {e}");
            return;
        }

        // Close editor if its open doc was inside the deleted subtree.
        if let Some(doc) = &self.opened {
            if ids.contains(&doc.id) {
                self.opened = None;
            }
        }

        self.modal = Modal::None;
        self.status = format!(
            "deleted {} `{}` ({} other node{} removed)",
            root_kind.as_str(),
            title,
            ids.len() - 1,
            if ids.len() == 2 { "" } else { "s" }
        );
        self.reload_hierarchy();
        if let Some(pid) = parent_id {
            if let Some(i) = self.rows.iter().position(|(id, _)| *id == pid) {
                self.tree_cursor = i;
            }
        }
    }

    // -------- open / save -------------------------------------------------

    fn open_selected(&mut self) -> Result<()> {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            return Ok(());
        };
        let Some(node) = self.hierarchy.get(id).cloned() else {
            return Ok(());
        };

        match node.kind {
            NodeKind::Paragraph => self.load_paragraph(&node)?,
            _ => {
                self.status = format!(
                    "`{}` is a {} (Enter opens paragraphs)",
                    node.title,
                    node.kind.as_str()
                );
            }
        }
        Ok(())
    }

    fn load_paragraph(&mut self, node: &Node) -> Result<()> {
        if let Some(prev) = &self.opened {
            if prev.id == node.id {
                self.focus = Focus::Editor;
                return Ok(());
            }
            // Auto-save any pending edits in the previous paragraph before
            // swapping to the new one. If the save fails (disk error etc.),
            // keep the old doc open so the user can see and fix it.
            if prev.dirty {
                let _ = self.save_current();
                if self.opened.as_ref().is_some_and(|d| d.dirty) {
                    self.status = format!(
                        "couldn't autosave `{}` — opening blocked. Fix the error or Ctrl+S manually.",
                        self.opened.as_ref().map(|d| d.title.as_str()).unwrap_or("")
                    );
                    return Ok(());
                }
            }
        }

        let Some(rel) = node.file.as_ref() else {
            self.status = format!("paragraph `{}` has no file on disk", node.title);
            return Ok(());
        };
        let abs = self.layout.root.join(rel);
        let body = match std::fs::read_to_string(&abs) {
            Ok(b) => b,
            Err(e) => {
                self.status = format!("read {}: {e}", abs.display());
                return Ok(());
            }
        };

        let lines: Vec<String> = if body.is_empty() {
            vec![String::new()]
        } else {
            body.split('\n').map(String::from).collect()
        };
        let saved_lines = lines.clone();
        let mut textarea = TextArea::new(lines);
        textarea.set_cursor_line_style(Style::default().add_modifier(Modifier::REVERSED));
        textarea.set_line_number_style(Style::default().fg(Color::DarkGray));

        self.opened = Some(OpenedDoc {
            id: node.id,
            title: node.title.clone(),
            rel_path: rel.clone(),
            textarea,
            dirty: false,
            scroll_row: 0,
            scroll_col: 0,
            block_anchor: None,
            last_activity: std::time::Instant::now(),
            saved_lines,
        });
        self.focus = Focus::Editor;
        self.status = format!("opened {}", abs.display());
        Ok(())
    }

    fn save_current(&mut self) -> Result<()> {
        let Some(doc) = self.opened.as_mut() else {
            return Ok(());
        };
        let body = doc.textarea.lines().join("\n");
        let abs = self.layout.root.join(&doc.rel_path);

        // Filesystem write first; if that fails, abort before touching the store.
        if let Err(e) = std::fs::write(&abs, body.as_bytes()) {
            self.status = format!("write {}: {e}", abs.display());
            return Ok(());
        }

        let id = doc.id;
        let Some(mut node) = self.hierarchy.get(id).cloned() else {
            self.status = format!("node {id} missing from hierarchy — try reopening the TUI");
            return Ok(());
        };

        // If this paragraph still has the placeholder title, derive a real one
        // from the body's first sentence and stamp it onto the node — that
        // becomes the displayed name in the tree pane.
        let title_was_placeholder = node.title == PARAGRAPH_PLACEHOLDER_TITLE;
        if title_was_placeholder {
            if let Some(derived) = extract_first_sentence(&body) {
                node.title = derived.clone();
                doc.title = derived;
            }
        }

        if let Err(e) = self
            .store
            .update_paragraph_content(&mut node, body.as_bytes())
        {
            self.status = format!("bdslib update failed: {e}");
            return Ok(());
        }
        if let Err(e) = self.store.sync() {
            self.status = format!("bdslib sync failed: {e}");
            return Ok(());
        }

        doc.dirty = false;
        // Refresh the saved-lines snapshot so the bold-new-additions overlay
        // resets, and stamp last_activity to "now" so idle autosave restarts.
        doc.saved_lines = doc.textarea.lines().to_vec();
        doc.last_activity = std::time::Instant::now();
        let words = node.word_count;
        if title_was_placeholder && node.title != PARAGRAPH_PLACEHOLDER_TITLE {
            self.status = format!(
                "saved {} ({} words) · named `{}` from first sentence",
                abs.display(),
                words,
                node.title
            );
        } else {
            self.status = format!("saved {} ({} words, re-embedded)", abs.display(), words);
        }
        self.reload_hierarchy();
        Ok(())
    }

    /// Re-read the hierarchy from bdslib and rebuild the flattened tree-row
    /// list, preserving the cursor on the same UUID if it still exists.
    fn reload_hierarchy(&mut self) {
        let prev_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
        match Hierarchy::load(&self.store) {
            Ok(h) => {
                self.hierarchy = h;
                self.rows = self
                    .hierarchy
                    .flatten()
                    .into_iter()
                    .map(|(n, d)| (n.id, d))
                    .collect();
                if let Some(id) = prev_id {
                    if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == id) {
                        self.tree_cursor = i;
                    }
                }
                if !self.rows.is_empty() {
                    self.tree_cursor = self.tree_cursor.min(self.rows.len() - 1);
                }
            }
            Err(e) => {
                self.status = format!("hierarchy reload failed: {e}");
            }
        }
    }

    // -------- drawing -----------------------------------------------------

    fn draw(&mut self, f: &mut ratatui::Frame) {
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(f.area());

        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(22),
                Constraint::Percentage(56),
                Constraint::Percentage(22),
            ])
            .split(outer[1]);

        self.draw_search_bar(f, outer[0]);
        self.draw_tree(f, body[0]);
        self.draw_editor(f, body[1]);
        self.draw_ai(f, body[2]);
        self.draw_ai_prompt(f, outer[2]);
        self.draw_status(f, outer[3]);

        if self.show_results_overlay {
            self.draw_search_overlay(f, outer[1]);
        }
        if self.show_prompt_picker {
            self.draw_prompt_picker(f, f.area());
        }

        // Modal renders last so it floats over everything.
        if !matches!(self.modal, Modal::None) {
            self.draw_modal(f, f.area());
        }
    }


    fn draw_file_picker_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
        picker: &FilePicker,
    ) {
        // Roomy panel — most of the screen, leaving a margin on all sides.
        let width = area.width.saturating_sub(8).max(40);
        let height = area.height.saturating_sub(4).max(10);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect {
            x,
            y,
            width,
            height,
        };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = match picker.context {
            PickerContext::EditorLoad => format!(" Load file into editor — {} ", picker.root.display()),
            PickerContext::TreeInsertOrImport => {
                format!(" Import into tree — {} ", picker.root.display())
            }
        };

        // The block reserves 2 rows (borders); a footer hint takes 1 more.
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let list_height = inner.height.saturating_sub(1) as usize;
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let list_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };

        // Scroll: keep cursor in view.
        let mut scroll = 0;
        if picker.cursor >= list_height && list_height > 0 {
            scroll = picker.cursor + 1 - list_height;
        }

        let mut lines: Vec<Line> = Vec::with_capacity(list_height);
        for (i, entry) in picker
            .entries
            .iter()
            .enumerate()
            .skip(scroll)
            .take(list_height)
        {
            let indent = "  ".repeat(entry.depth);
            let glyph = if entry.is_dir {
                if entry.expanded { "▾ 📁 " } else { "▸ 📁 " }
            } else {
                "  📄 "
            };
            let name = entry
                .path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("?")
                .to_string();
            let mut style = if entry.is_dir {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };
            if i == picker.cursor {
                style = style.add_modifier(Modifier::REVERSED);
            }
            lines.push(Line::from(Span::styled(
                format!("{indent}{glyph}{name}"),
                style,
            )));
        }

        f.render_widget(Paragraph::new(lines), list_rect);

        let hint = Line::from(Span::styled(
            " ↑↓ navigate · → expand · ← collapse/parent · Enter pick · Esc cancel ",
            Style::default().add_modifier(Modifier::DIM),
        ));
        f.render_widget(Paragraph::new(hint), footer_rect);
    }

    fn draw_modal(&self, f: &mut ratatui::Frame, area: Rect) {
        // The file picker needs a much larger panel than the fixed
        // 80-wide / 8-high box used for confirms — give it its own renderer.
        if let Modal::FilePicker(picker) = &self.modal {
            self.draw_file_picker_modal(f, area, picker);
            return;
        }

        let width = area.width.saturating_sub(8).clamp(30, 80);
        let height: u16 = 8;
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let (title, border_color, body): (String, Color, Vec<Line<'_>>) = match &self.modal {
            Modal::None => return,
            Modal::FilePicker(_) => unreachable!("file picker handled above"),
            Modal::Adding {
                kind,
                parent_label,
                input,
                position,
                ..
            } => {
                let header = match position {
                    InsertPosition::End => format!(" Add {} ", kind.as_str()),
                    InsertPosition::After(_) => format!(" Insert {} after current ", kind.as_str()),
                };
                let parent = format!(" Parent: {}", parent_label);
                let where_line = match position {
                    InsertPosition::End => "    Where: append at end".to_string(),
                    InsertPosition::After(anchor_id) => {
                        let anchor_name = self
                            .hierarchy
                            .get(*anchor_id)
                            .map(|n| n.title.clone())
                            .unwrap_or_else(|| "<gone>".to_string());
                        format!("    Where: after `{anchor_name}`")
                    }
                };
                let title_line = format!(" Title : {}", input.render_with_cursor('│'));
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(parent, Style::default().add_modifier(Modifier::DIM))),
                    Line::from(Span::styled(
                        where_line,
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                    Line::from(title_line),
                    Line::from(Span::styled(
                        " Enter to confirm · Esc to cancel ",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (header, Color::Green, body)
            }
            Modal::Deleting {
                root_kind,
                title,
                descendant_count,
                ..
            } => {
                let prompt = if *descendant_count > 0 {
                    format!(
                        " Delete {} `{}` and {} descendant{}?",
                        root_kind.as_str(),
                        title,
                        descendant_count,
                        if *descendant_count == 1 { "" } else { "s" }
                    )
                } else {
                    format!(" Delete {} `{}`?", root_kind.as_str(), title)
                };
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        prompt,
                        Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        " Removes files from disk AND records from bdslib.",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                    Line::from(Span::styled(
                        " y / Enter to confirm · n / Esc to cancel ",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (" Confirm delete ".into(), Color::Red, body)
            }
            Modal::Renaming { kind, input, .. } => {
                let header = format!(" Rename {} ", kind.as_str());
                let title_line = format!(" Title : {}", input.render_with_cursor('│'));
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("    Renaming a {} — its slug and filesystem entry don't change.", kind.as_str()),
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                    Line::from(""),
                    Line::from(title_line),
                    Line::from(""),
                    Line::from(Span::styled(
                        " Enter to confirm · Esc to cancel ",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (header, Color::Blue, body)
            }
            Modal::SnapshotPicker {
                paragraph_title,
                snapshots,
                cursor,
                ..
            } => {
                let header = format!(" Snapshots — {} ", paragraph_title);
                let mut body: Vec<Line> = Vec::with_capacity(snapshots.len() + 2);
                body.push(Line::from(""));
                for (i, snap) in snapshots.iter().enumerate() {
                    let selected = i == *cursor;
                    let ts = snap.created_at.format("%Y-%m-%d %H:%M:%S");
                    let head = format!(
                        " {ts}   {}w   {}",
                        snap.word_count,
                        if snap.preview.is_empty() {
                            "(no body yet)"
                        } else {
                            snap.preview.as_str()
                        }
                    );
                    let style = if selected {
                        Style::default()
                            .add_modifier(Modifier::REVERSED | Modifier::BOLD)
                            .fg(Color::Cyan)
                    } else {
                        Style::default()
                    };
                    body.push(Line::from(Span::styled(head, style)));
                }
                body.push(Line::from(""));
                body.push(Line::from(Span::styled(
                    " ↑↓ navigate · Enter loads (current edits become dirty) · Esc cancel ",
                    Style::default().add_modifier(Modifier::DIM),
                )));
                (header, Color::Cyan, body)
            }
        };

        f.render_widget(
            Paragraph::new(body).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD)),
            ),
            rect,
        );
    }

    fn pane_block<'a>(&self, title: &'a str, focus: Focus) -> Block<'a> {
        let mut block = Block::default().borders(Borders::ALL).title(title);
        if self.focus == focus {
            block = block
                .border_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        }
        block
    }

    /// Editor pane block. Border color carries the document's clean/dirty
    /// state: green when saved, yellow when there are unsaved changes. The
    /// focus chip in the status bar still indicates which pane has focus, so
    /// the border can be dedicated to dirty signaling.
    fn editor_block<'a>(&self, title: &'a str) -> Block<'a> {
        let dirty = self.opened.as_ref().is_some_and(|d| d.dirty);
        let color = if dirty { Color::Yellow } else { Color::Green };
        let mut style = Style::default().fg(color);
        if self.focus == Focus::Editor {
            style = style.add_modifier(Modifier::BOLD);
        }
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(style)
    }

    fn draw_search_bar(&self, f: &mut ratatui::Frame, area: Rect) {
        let text = if self.focus == Focus::SearchBar {
            self.search_input.render_with_cursor('│')
        } else if self.search_input.is_empty() {
            String::from("(press Ctrl+/ to search)")
        } else {
            self.search_input.as_str().to_string()
        };
        let style = if self.focus == Focus::SearchBar {
            Style::default()
        } else {
            Style::default().add_modifier(Modifier::DIM)
        };
        let p = Paragraph::new(text)
            .style(style)
            .block(self.pane_block("Search", Focus::SearchBar));
        f.render_widget(p, area);
    }

    fn draw_ai_prompt(&self, f: &mut ratatui::Frame, area: Rect) {
        let text = if self.focus == Focus::AiPrompt {
            self.ai_input.render_with_cursor('│')
        } else if self.ai_input.is_empty() {
            String::from("(press Ctrl+I for AI; `/` lists prompts)")
        } else {
            self.ai_input.as_str().to_string()
        };
        let style = if self.focus == Focus::AiPrompt {
            Style::default()
        } else {
            Style::default().add_modifier(Modifier::DIM)
        };
        let p = Paragraph::new(text)
            .style(style)
            .block(self.pane_block("AI prompt", Focus::AiPrompt));
        f.render_widget(p, area);
    }

    fn draw_tree(&self, f: &mut ratatui::Frame, area: Rect) {
        let block = self.pane_block("Tree", Focus::Tree);
        let inner = block.inner(area);
        f.render_widget(block, area);

        if self.rows.is_empty() {
            let hint = Paragraph::new("(empty project — `inkhaven add book \"…\"` from the CLI)")
                .style(Style::default().add_modifier(Modifier::DIM));
            f.render_widget(hint, inner);
            return;
        }

        let height = inner.height as usize;
        let mut scroll = self.tree_scroll;
        if self.tree_cursor < scroll {
            scroll = self.tree_cursor;
        } else if self.tree_cursor >= scroll + height && height > 0 {
            scroll = self.tree_cursor + 1 - height;
        }

        let open_id: Option<Uuid> = self.opened.as_ref().map(|d| d.id);

        let mut lines: Vec<Line> = Vec::new();
        for (i, (id, depth)) in self.rows.iter().enumerate().skip(scroll).take(height) {
            let Some(node) = self.hierarchy.get(*id) else {
                continue;
            };
            let indent = "  ".repeat(*depth);
            // When this row is the paragraph currently open in the editor,
            // swap its kind glyph for a "►" arrow so it's obvious at a
            // glance which paragraph the editor pane is showing.
            let is_open = open_id.is_some_and(|o| o == node.id);
            let marker = if is_open {
                "►"
            } else {
                match node.kind {
                    NodeKind::Paragraph => "¶ ",
                    NodeKind::Book => "📖 ",
                    NodeKind::Chapter => "▸ ",
                    NodeKind::Subchapter => "▹ ",
                }
            };
            let mut row_style = Style::default();
            if is_open {
                // Green + bold matches the editor's clean-state border so the
                // user can mentally connect the two. Even when the editor is
                // dirty (yellow border), this stays green — it's marking the
                // "what is loaded", not the dirty state.
                row_style = row_style
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD);
            }
            if i == self.tree_cursor {
                row_style = row_style.add_modifier(Modifier::REVERSED);
            }

            // Truncate very long titles (typical for paragraphs whose name
            // was auto-derived from the first sentence) so they don't push
            // the word count off the pane.
            let display_title = truncate_title(&node.title, TITLE_MAX_DISPLAY);
            let mut spans = vec![Span::styled(
                format!("{indent}{marker}{display_title}"),
                row_style,
            )];
            if matches!(node.kind, NodeKind::Paragraph) {
                let count_style = if i == self.tree_cursor || is_open {
                    row_style
                } else {
                    Style::default().add_modifier(Modifier::DIM)
                };
                spans.push(Span::styled(format!("  {}w", node.word_count), count_style));
            }
            lines.push(Line::from(spans));
        }

        let p = Paragraph::new(lines);
        f.render_widget(p, inner);
    }

    fn draw_editor(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let title = match &self.opened {
            Some(d) if d.dirty => format!("Editor — {} [modified]", d.title),
            Some(d) => format!("Editor — {}", d.title),
            None => String::from("Editor"),
        };
        let block = self.editor_block(&title);
        let inner = block.inner(area);
        f.render_widget(block, area);

        if self.opened.is_none() {
            let hint = Paragraph::new(
                "(no paragraph open — select one in the Tree pane and press Enter)",
            )
            .style(Style::default().add_modifier(Modifier::DIM))
            .wrap(Wrap { trim: false });
            f.render_widget(hint, inner);
            return;
        }

        if self.cfg.editor.wrap {
            self.draw_editor_wrapped(f, inner);
        } else {
            self.draw_editor_unwrapped(f, inner);
        }
    }

    fn draw_editor_unwrapped(&mut self, f: &mut ratatui::Frame, inner: Rect) {
        let block = self.current_block();
        let opened = self.opened.as_mut().expect("opened checked above");
        let highlighter = &mut self.highlighter;
        let current_lines: Vec<String> = opened.textarea.lines().to_vec();
        let source = current_lines.join("\n");
        let highlighted = highlighter.highlight_lines(&source);

        // Precompute "added since last save" bitmaps per source row.
        let saved = &opened.saved_lines;
        let added_per_row: Vec<Vec<bool>> = current_lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let saved_line = saved.get(i).map(String::as_str).unwrap_or("");
                if saved.get(i).is_none() {
                    // Line beyond the saved snapshot: everything is new.
                    vec![true; line.chars().count()]
                } else {
                    diff_added(saved_line, line)
                }
            })
            .collect();

        let (cur_row, cur_col) = opened.textarea.cursor();
        let selection = opened.textarea.selection_range();

        let total_lines = highlighted.len().max(1);
        let lineno_chars = digit_count(total_lines);
        let gutter_width = (lineno_chars + 1) as u16;

        let h = inner.height as usize;
        let w = inner.width.saturating_sub(gutter_width) as usize;

        if h > 0 {
            if cur_row < opened.scroll_row {
                opened.scroll_row = cur_row;
            } else if cur_row >= opened.scroll_row + h {
                opened.scroll_row = cur_row + 1 - h;
            }
        }
        if w > 0 {
            if cur_col < opened.scroll_col {
                opened.scroll_col = cur_col;
            } else if cur_col >= opened.scroll_col + w {
                opened.scroll_col = cur_col + 1 - w;
            }
        }

        let lineno_style = Style::default().fg(Color::DarkGray);
        let current_bg = Color::Indexed(236);

        let mut visible_lines: Vec<Line> = Vec::with_capacity(h);
        let row_end = (opened.scroll_row + h).min(highlighted.len());
        for row in opened.scroll_row..row_end {
            let is_current = row == cur_row;
            let lineno_text = format!("{:>chars$} ", row + 1, chars = lineno_chars);
            let mut lineno_span_style = lineno_style;
            if is_current {
                lineno_span_style = lineno_span_style
                    .bg(current_bg)
                    .add_modifier(Modifier::BOLD);
            }

            let added_flags = added_per_row.get(row).map(Vec::as_slice);
            let mut text_spans = build_row_spans(
                &highlighted[row],
                row,
                opened.scroll_col,
                w,
                selection,
                block,
                added_flags,
            );
            if is_current {
                for s in &mut text_spans {
                    if s.style.bg.is_none() {
                        s.style = s.style.bg(current_bg);
                    }
                }
            }

            let text_chars: usize = text_spans.iter().map(|s| s.content.chars().count()).sum();
            let mut spans = vec![Span::styled(lineno_text, lineno_span_style)];
            spans.extend(text_spans);
            if is_current && text_chars < w {
                spans.push(Span::styled(
                    " ".repeat(w - text_chars),
                    Style::default().bg(current_bg),
                ));
            }
            visible_lines.push(Line::from(spans));
        }
        f.render_widget(Paragraph::new(visible_lines), inner);

        if self.focus == Focus::Editor
            && h > 0
            && w > 0
            && cur_row >= opened.scroll_row
            && cur_row < opened.scroll_row + h
            && cur_col >= opened.scroll_col
            && cur_col < opened.scroll_col + w
        {
            let x = inner.x + gutter_width + (cur_col - opened.scroll_col) as u16;
            let y = inner.y + (cur_row - opened.scroll_row) as u16;
            f.set_cursor_position((x, y));
        }
    }

    fn draw_editor_wrapped(&mut self, f: &mut ratatui::Frame, inner: Rect) {
        let block = self.current_block();
        let opened = self.opened.as_mut().expect("opened checked above");
        let highlighter = &mut self.highlighter;
        let current_lines: Vec<String> = opened.textarea.lines().to_vec();
        let source = current_lines.join("\n");
        let highlighted = highlighter.highlight_lines(&source);

        let saved = &opened.saved_lines;
        let added_per_row: Vec<Vec<bool>> = current_lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                if saved.get(i).is_none() {
                    vec![true; line.chars().count()]
                } else {
                    diff_added(&saved[i], line)
                }
            })
            .collect();

        let (cur_row, cur_col) = opened.textarea.cursor();
        let selection = opened.textarea.selection_range();

        let total_lines = highlighted.len().max(1);
        let lineno_chars = digit_count(total_lines);
        let gutter_width = (lineno_chars + 1) as u16;

        let h = inner.height as usize;
        let w = inner.width.saturating_sub(gutter_width) as usize;

        let mut visual: Vec<super::highlight::VisualRow> = Vec::new();
        for (src_row, runs) in highlighted.iter().enumerate() {
            for vr in wrap_line(runs, src_row, w) {
                visual.push(vr);
            }
        }

        let cursor_visual = find_cursor_visual(&visual, cur_row, cur_col);

        if h > 0 {
            if cursor_visual.0 < opened.scroll_row {
                opened.scroll_row = cursor_visual.0;
            } else if cursor_visual.0 >= opened.scroll_row + h {
                opened.scroll_row = cursor_visual.0 + 1 - h;
            }
        }
        opened.scroll_col = 0;

        let lineno_style = Style::default().fg(Color::DarkGray);
        let current_bg = Color::Indexed(236);

        let mut lines: Vec<Line> = Vec::with_capacity(h);
        let row_end = (opened.scroll_row + h).min(visual.len());
        for (i, v) in visual[opened.scroll_row..row_end].iter().enumerate() {
            let visual_row_idx = opened.scroll_row + i;
            let is_current = visual_row_idx == cursor_visual.0;

            // Line number only on the first visual row of each source row.
            let lineno_text = if v.src_col_start == 0 {
                format!("{:>chars$} ", v.src_row + 1, chars = lineno_chars)
            } else {
                format!("{:>chars$} ", "", chars = lineno_chars)
            };
            let mut lineno_span_style = lineno_style;
            if is_current {
                lineno_span_style = lineno_span_style
                    .bg(current_bg)
                    .add_modifier(Modifier::BOLD);
            }

            let added_flags = added_per_row.get(v.src_row).map(Vec::as_slice);
            let mut text_spans = build_visual_row_spans(v, selection, block, added_flags);
            if is_current {
                for s in &mut text_spans {
                    if s.style.bg.is_none() {
                        s.style = s.style.bg(current_bg);
                    }
                }
            }

            let text_chars: usize = text_spans.iter().map(|s| s.content.chars().count()).sum();
            let mut spans = vec![Span::styled(lineno_text, lineno_span_style)];
            spans.extend(text_spans);
            if is_current && text_chars < w {
                spans.push(Span::styled(
                    " ".repeat(w - text_chars),
                    Style::default().bg(current_bg),
                ));
            }
            lines.push(Line::from(spans));
        }
        f.render_widget(Paragraph::new(lines), inner);

        if self.focus == Focus::Editor
            && h > 0
            && w > 0
            && cursor_visual.0 >= opened.scroll_row
            && cursor_visual.0 < opened.scroll_row + h
            && cursor_visual.1 < w
        {
            let x = inner.x + gutter_width + cursor_visual.1 as u16;
            let y = inner.y + (cursor_visual.0 - opened.scroll_row) as u16;
            f.set_cursor_position((x, y));
        }
    }

    fn draw_ai(&self, f: &mut ratatui::Frame, area: Rect) {
        let title = match &self.inference {
            None => String::from("AI"),
            Some(inf) => match &inf.status {
                InferenceStatus::Streaming => format!("AI — {} · streaming…", inf.provider),
                InferenceStatus::Done => format!("AI — {} · done", inf.provider),
                InferenceStatus::Error(_) => format!("AI — {} · error", inf.provider),
            },
        };
        let block = self.pane_block(&title, Focus::Ai);
        let inner = block.inner(area);
        f.render_widget(block, area);

        match &self.inference {
            None => {
                let hint = Paragraph::new(
                    "(focus AI prompt with Ctrl+I, type a query and press Enter\n\n type `/` to pick from the prompt library)",
                )
                .style(Style::default().add_modifier(Modifier::DIM))
                .wrap(Wrap { trim: false });
                f.render_widget(hint, inner);
            }
            Some(inf) => {
                // Reserve the last line for action hints when done.
                let show_hints = matches!(inf.status, InferenceStatus::Done) && !inf.response.is_empty();
                let body_height = if show_hints {
                    inner.height.saturating_sub(2)
                } else {
                    inner.height
                };
                let body_rect = Rect {
                    x: inner.x,
                    y: inner.y,
                    width: inner.width,
                    height: body_height,
                };
                let body_text: &str = match &inf.status {
                    InferenceStatus::Error(e) => e,
                    _ => inf.response.as_str(),
                };
                let style = match &inf.status {
                    InferenceStatus::Error(_) => Style::default().fg(Color::Red),
                    InferenceStatus::Streaming => Style::default(),
                    InferenceStatus::Done => Style::default(),
                };
                f.render_widget(
                    Paragraph::new(body_text).style(style).wrap(Wrap { trim: false }),
                    body_rect,
                );
                if show_hints && inner.height >= 2 {
                    let hints_rect = Rect {
                        x: inner.x,
                        y: inner.y + inner.height - 1,
                        width: inner.width,
                        height: 1,
                    };
                    let hints = Line::from(vec![
                        Span::styled(" r ", reverse_chip(Color::Yellow)),
                        Span::raw("replace  "),
                        Span::styled(" i ", reverse_chip(Color::Yellow)),
                        Span::raw("insert  "),
                        Span::styled(" t ", reverse_chip(Color::Yellow)),
                        Span::raw("top  "),
                        Span::styled(" b ", reverse_chip(Color::Yellow)),
                        Span::raw("bottom  "),
                        Span::styled(" c ", reverse_chip(Color::Yellow)),
                        Span::raw("copy"),
                    ]);
                    f.render_widget(Paragraph::new(hints), hints_rect);
                }
            }
        }
    }

    fn draw_prompt_picker(&self, f: &mut ratatui::Frame, area: Rect) {
        let width = (area.width * 6 / 10).max(40).min(area.width.saturating_sub(4));
        let matches = self.prompt_picker_matches();
        let row_count = matches.len() as u16;
        let height = (row_count * 2 + 2).max(4).min(area.height.saturating_sub(4));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        // Anchor near the bottom (above the AI prompt bar).
        let y = area.height.saturating_sub(height + 4) + area.y;
        let rect = Rect {
            x,
            y,
            width,
            height,
        };
        f.render_widget(ratatui::widgets::Clear, rect);

        let mut lines: Vec<Line> = Vec::new();
        if matches.is_empty() {
            lines.push(Line::from(Span::styled(
                "(no matching prompts)",
                Style::default().add_modifier(Modifier::DIM),
            )));
        } else {
            for (i, p) in matches.iter().enumerate() {
                let selected = i == self.prompt_picker_cursor;
                let name_style = if selected {
                    Style::default()
                        .add_modifier(Modifier::REVERSED | Modifier::BOLD)
                        .fg(Color::Magenta)
                } else {
                    Style::default().fg(Color::Magenta)
                };
                let desc_style = if selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default().add_modifier(Modifier::DIM)
                };
                lines.push(Line::from(Span::styled(format!(" /{}", p.name), name_style)));
                lines.push(Line::from(Span::styled(
                    format!("    {}", p.description),
                    desc_style,
                )));
            }
        }

        f.render_widget(
            Paragraph::new(lines).wrap(Wrap { trim: false }).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Prompts ")
                    .border_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            ),
            rect,
        );
    }

    fn draw_status(&self, f: &mut ratatui::Frame, area: Rect) {
        let dirty = self.opened.as_ref().is_some_and(|d| d.dirty);
        let mut spans: Vec<Span<'_>> = Vec::new();
        if dirty {
            spans.push(Span::styled(
                " ● ",
                Style::default()
                    .bg(Color::Red)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        spans.push(Span::styled(
            format!(" [{}] ", self.focus.label()),
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw("  "));
        spans.push(Span::raw(self.status.clone()));
        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    fn draw_search_overlay(&self, f: &mut ratatui::Frame, area: Rect) {
        let width = area.width.saturating_sub(6).max(40);
        // Each result takes 3 lines (header / title / snippet); +2 for borders;
        // +1 for an "(no results)" hint when empty.
        let body_rows = if self.results.is_empty() {
            1
        } else {
            (self.results.len() as u16) * 3
        };
        let height = (body_rows + 2).min(area.height.saturating_sub(2)).max(5);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + 1;
        let rect = Rect {
            x,
            y,
            width,
            height,
        };

        f.render_widget(ratatui::widgets::Clear, rect);

        let title = format!(
            " Results for `{}` ({}) ",
            self.search_input.as_str(),
            self.results.len()
        );

        let mut lines: Vec<Line> = Vec::new();
        if self.results.is_empty() {
            lines.push(Line::from(Span::styled(
                "(no results)",
                Style::default().add_modifier(Modifier::DIM),
            )));
        } else {
            for (i, hit) in self.results.iter().enumerate() {
                let selected = i == self.results_cursor;
                let header_style = if selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::REVERSED | Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Yellow)
                };
                let title_style = if selected {
                    Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
                } else {
                    Style::default().add_modifier(Modifier::BOLD)
                };
                let snippet_style = if selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default().add_modifier(Modifier::DIM)
                };

                let header = format!(
                    " {:>5.3}  [{:<10}] {} ",
                    hit.score,
                    hit.kind.as_str(),
                    hit.slug_path
                );
                lines.push(Line::from(Span::styled(header, header_style)));
                lines.push(Line::from(Span::styled(
                    format!("         {}", hit.title),
                    title_style,
                )));
                let snip = if hit.snippet.is_empty() {
                    "         (no body yet)".to_string()
                } else {
                    format!("         {}", hit.snippet)
                };
                lines.push(Line::from(Span::styled(snip, snippet_style)));
            }
        }

        let body = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
        );
        f.render_widget(body, rect);
    }
}

/// Map a source-coordinate cursor (row, col) to its visual-coordinate
/// position inside a wrapped layout. Linear in number of visual rows up to
/// the cursor, which is bounded by the source size — cheap at literary scale.
fn find_cursor_visual(
    visual: &[super::highlight::VisualRow],
    cur_row: usize,
    cur_col: usize,
) -> (usize, usize) {
    // Locate the cursor row's last visual row so end-of-line cursor lands
    // there even when the line wraps.
    let mut chosen: Option<usize> = None;
    for (i, v) in visual.iter().enumerate() {
        if v.src_row != cur_row {
            if chosen.is_some() {
                break;
            }
            continue;
        }
        let row_end = v.src_col_start + v.width_chars;
        if cur_col < row_end {
            return (i, cur_col - v.src_col_start);
        }
        chosen = Some(i);
    }
    if let Some(i) = chosen {
        let v = &visual[i];
        return (i, cur_col.saturating_sub(v.src_col_start));
    }
    (0, 0)
}

/// Placeholder title for paragraphs added without one. The next save replaces
/// it with the first sentence of the body.
const PARAGRAPH_PLACEHOLDER_TITLE: &str = "Untitled paragraph";

/// Maximum number of characters a node title is allowed to occupy in the
/// tree pane. Beyond that, the title is truncated with an ellipsis so the
/// `Nw` word-count suffix stays visible on a single row.
const TITLE_MAX_DISPLAY: usize = 60;

fn truncate_title(title: &str, max_chars: usize) -> String {
    let chars: Vec<char> = title.chars().collect();
    if chars.len() <= max_chars {
        return title.to_string();
    }
    let mut out: String = chars.iter().take(max_chars - 1).collect();
    out.push('…');
    out
}

/// Try to derive a usable title from a paragraph body. Skips Typst heading
/// lines (`= …`), comments (`// …`), and blank lines. Looks for the first
/// `.`, `!`, or `?` followed by whitespace or end-of-text. Truncates the
/// result to fit `TITLE_MAX_DISPLAY` chars (with ellipsis if cut). Returns
/// None if no usable text is found.
fn extract_first_sentence(content: &str) -> Option<String> {
    let prose: String = content
        .lines()
        .filter_map(|l| {
            let t = l.trim();
            if t.is_empty() || t.starts_with("=") || t.starts_with("//") {
                None
            } else {
                Some(t.to_string())
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    if prose.is_empty() {
        return None;
    }

    let chars: Vec<char> = prose.chars().collect();
    let mut end = chars.len();
    for (i, c) in chars.iter().enumerate() {
        if matches!(*c, '.' | '!' | '?') {
            let next_is_space_or_end = i + 1 >= chars.len() || chars[i + 1].is_whitespace();
            if next_is_space_or_end {
                end = i + 1;
                break;
            }
        }
    }
    let sentence: String = chars.iter().take(end).collect();
    let sentence = sentence.trim();
    if sentence.is_empty() {
        return None;
    }

    let s_chars: Vec<char> = sentence.chars().collect();
    if s_chars.len() > TITLE_MAX_DISPLAY {
        let mut out: String = s_chars.iter().take(TITLE_MAX_DISPLAY - 1).collect();
        out.push('…');
        Some(out)
    } else {
        Some(sentence.to_string())
    }
}

fn digit_count(n: usize) -> usize {
    let mut x = n.max(1);
    let mut d = 0;
    while x > 0 {
        d += 1;
        x /= 10;
    }
    d
}

fn reverse_chip(fg: Color) -> Style {
    Style::default()
        .bg(fg)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD)
}

/// Extract the substring spanning `(start_row, start_col)..(end_row, end_col)`
/// from a slice of lines, where both ends are char-indexed. Used for pulling
/// the editor selection out for `{{selection}}` substitution.
fn slice_lines(lines: &[String], r1: usize, c1: usize, r2: usize, c2: usize) -> String {
    if r1 >= lines.len() {
        return String::new();
    }
    if r1 == r2 {
        let chars: Vec<char> = lines[r1].chars().collect();
        let s = c1.min(chars.len());
        let e = c2.min(chars.len());
        return chars[s..e].iter().collect();
    }
    let mut out = String::new();
    let first: String = lines[r1].chars().skip(c1).collect();
    out.push_str(&first);
    out.push('\n');
    for r in (r1 + 1)..r2 {
        if let Some(line) = lines.get(r) {
            out.push_str(line);
            out.push('\n');
        }
    }
    if let Some(last_line) = lines.get(r2) {
        let chars: Vec<char> = last_line.chars().collect();
        let e = c2.min(chars.len());
        out.extend(chars[..e].iter());
    }
    out
}
