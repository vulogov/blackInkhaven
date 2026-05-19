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

use super::focus::Focus;
use super::highlight::{
    BlockSelection, TypstHighlighter, build_row_spans, build_visual_row_spans, wrap_line,
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
    },
    Deleting {
        root_id: Uuid,
        root_kind: NodeKind,
        title: String,
        descendant_count: usize,
        ids: Vec<Uuid>,
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
            return Ok(true);
        }

        // Modal eats every other key.
        if !matches!(self.modal, Modal::None) {
            return self.handle_modal_key(key);
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
            KeyCode::Char('q') | KeyCode::Char('Q') if plain => return Ok(true),
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
            KeyCode::Char('C') | KeyCode::Char('c') if plain => {
                self.open_add_modal(NodeKind::Chapter);
            }
            KeyCode::Char('A') | KeyCode::Char('a') if plain => {
                self.open_add_modal(NodeKind::Subchapter);
            }
            KeyCode::Char('+') if plain => {
                self.open_add_modal(NodeKind::Paragraph);
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
                return Ok(true);
            }
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
            return Ok(true);
        }
        Ok(false)
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
                    self.focus = Focus::Tree;
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
                let mut mods = key.modifiers;
                mods.remove(KeyModifiers::SHIFT);
                if mods.is_empty() {
                    self.current_input(is_search).insert_char(c);
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
        let cursor_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
        let parent_node = match self.hierarchy.pick_parent_for(&self.cfg, cursor_id, kind) {
            Ok(p) => p,
            Err(e) => {
                self.status = format!("can't add {}: {e}", kind.as_str());
                return;
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
                        let mut mods = key.modifiers;
                        mods.remove(KeyModifiers::SHIFT);
                        if mods.is_empty() {
                            input.insert_char(c);
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
        let (kind, parent_id, raw_title) = match &self.modal {
            Modal::Adding {
                kind,
                parent_id,
                input,
                ..
            } => (*kind, *parent_id, input.as_str().trim().to_string()),
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
            if prev.dirty {
                self.status = format!(
                    "unsaved changes in `{}` — Ctrl+S to save, then reopen",
                    prev.title
                );
                return Ok(());
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


    fn draw_modal(&self, f: &mut ratatui::Frame, area: Rect) {
        let width = area.width.saturating_sub(8).clamp(30, 80);
        let height: u16 = 7;
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let (title, border_color, body): (String, Color, Vec<Line<'_>>) = match &self.modal {
            Modal::None => return,
            Modal::Adding {
                kind,
                parent_label,
                input,
                ..
            } => {
                let header = format!(" Add {} ", kind.as_str());
                let parent = format!(" Parent: {}", parent_label);
                let title_line = format!(" Title : {}│", input.as_str());
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(parent, Style::default().add_modifier(Modifier::DIM))),
                    Line::from(title_line),
                    Line::from(""),
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

    fn draw_search_bar(&self, f: &mut ratatui::Frame, area: Rect) {
        let mut text = self.search_input.as_str().to_string();
        if self.focus == Focus::SearchBar {
            text.push('│');
        } else if text.is_empty() {
            text = String::from("(press Ctrl+/ to search)");
        }
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
        let mut text = self.ai_input.as_str().to_string();
        if self.focus == Focus::AiPrompt {
            text.push('│');
        } else if text.is_empty() {
            text = String::from("(press Ctrl+I for AI; `/` lists prompts)");
        }
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

        let mut lines: Vec<Line> = Vec::new();
        for (i, (id, depth)) in self.rows.iter().enumerate().skip(scroll).take(height) {
            let Some(node) = self.hierarchy.get(*id) else {
                continue;
            };
            let indent = "  ".repeat(*depth);
            let marker = match node.kind {
                NodeKind::Paragraph => "¶ ",
                NodeKind::Book => "📖 ",
                NodeKind::Chapter => "▸ ",
                NodeKind::Subchapter => "▹ ",
            };
            let mut row_style = Style::default();
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
                let count_style = if i == self.tree_cursor {
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
        let block = self.pane_block(&title, Focus::Editor);
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
        let source = opened.textarea.lines().join("\n");
        let highlighted = highlighter.highlight_lines(&source);

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

            let mut text_spans =
                build_row_spans(&highlighted[row], row, opened.scroll_col, w, selection, block);
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
        let source = opened.textarea.lines().join("\n");
        let highlighted = highlighter.highlight_lines(&source);

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

            let mut text_spans = build_visual_row_spans(v, selection, block);
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
        let line = Line::from(vec![
            Span::styled(
                format!(" [{}] ", self.focus.label()),
                Style::default()
                    .bg(Color::Cyan)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::raw(self.status.clone()),
        ]);
        f.render_widget(Paragraph::new(line), area);
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
