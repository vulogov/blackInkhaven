//! WBLD-1 (WB-P0) — the `WorldbuilderApp` shell.
//!
//! A third [`crate::tui_host::TuiHost`] consumer beside `research::ResearchApp`
//! and `linguistic::LinguisticApp`. WB-P0 stands up the frame: two (empty) left
//! panes over the Facts and World books, a full-width Query prompt, a right pane
//! (Chat | Research | Map | Ledger), a status bar, the focus/Tab model, and the
//! session sidecar. Later phases fill the panes with behaviour.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::Backend;
use tui_textarea::TextArea;

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::{SYSTEM_TAG_FACTS, SYSTEM_TAG_WORLD, Store};
use crate::system_tree::SystemBookTree;
use crate::tui::theme::Theme;
use crate::tui_host::TuiHost;
use uuid::Uuid;

use super::WorldbuilderInvocation;
use super::focus::{Focus, RightPane};
use super::session::WorldbuilderSession;

pub(crate) struct WorldbuilderApp {
    pub(super) layout: ProjectLayout,
    // Read by WB-P2 (system prompt), WB-P3 (plausibility weights), etc.
    #[allow(dead_code)]
    pub(super) cfg: Config,
    // Read by WB-P2+ (retrieval, world state, fact insert).
    #[allow(dead_code)]
    pub(super) store: Store,
    #[allow(dead_code)]
    pub(super) hierarchy: Hierarchy,

    // — Left pane A: Facts book (rendered + navigated in WB-P1) ————————
    #[allow(dead_code)]
    pub(super) facts_tree: SystemBookTree,
    #[allow(dead_code)]
    pub(super) facts_pins: Vec<Uuid>,

    // — Left pane B: World book (WB-P1) ————————————————————————————————
    #[allow(dead_code)]
    pub(super) world_tree: SystemBookTree,
    #[allow(dead_code)]
    pub(super) world_pins: Vec<Uuid>,

    // — Sizing (persisted to the session) ——————————————————————————————
    pub(super) left_split: u8,  // Facts / World vertical split   (2–8)
    pub(super) split_ratio: u8, // left column / right pane ratio (2–8)

    // — Focus + right pane ————————————————————————————————————————————
    pub(super) focus: Focus,
    pub(super) prev_focus: Focus,
    pub(super) right_pane: RightPane,

    // — Query prompt (full width) ——————————————————————————————————————
    pub(super) query: TextArea<'static>,

    // — Session ————————————————————————————————————————————————————————
    pub(super) session: WorldbuilderSession,

    pub(super) should_quit: bool,
    pub(super) status: String,
    pub(super) show_hints: bool,
    pub(super) theme: Theme,
}

impl WorldbuilderApp {
    pub(crate) fn new(
        layout: ProjectLayout,
        cfg: Config,
        store: Store,
        hierarchy: Hierarchy,
        inv: WorldbuilderInvocation,
    ) -> Result<WorldbuilderApp> {
        let facts_tree = SystemBookTree::new(&hierarchy, SYSTEM_TAG_FACTS);
        let world_tree = SystemBookTree::new(&hierarchy, SYSTEM_TAG_WORLD);

        let now = chrono::Utc::now().to_rfc3339();
        let session_name = inv.session.as_deref().unwrap_or("default");
        let session = WorldbuilderSession::open_or_create(&layout, session_name, now)?;

        let theme = Theme::from_config(&cfg.theme);
        let mut query = TextArea::default();
        query.set_cursor_line_style(ratatui::style::Style::default());

        let left_split = session.left_split.clamp(2, 8);
        let split_ratio = session.split_ratio.clamp(2, 8);

        Ok(WorldbuilderApp {
            layout,
            cfg,
            store,
            hierarchy,
            facts_tree,
            facts_pins: Vec::new(),
            world_tree,
            world_pins: Vec::new(),
            left_split,
            split_ratio,
            focus: Focus::QueryPrompt,
            prev_focus: Focus::QueryPrompt,
            right_pane: RightPane::Chat,
            query,
            session,
            should_quit: false,
            status: "worldbuilder — WB-P0 shell (Tab cycles panes · Ctrl+Q quits)".to_string(),
            show_hints: true,
            theme,
        })
    }

    pub(crate) fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        crate::tui_host::run_loop(self, terminal)
    }

    /// The `world_name` for the title bar (session, else placeholder).
    pub(super) fn world_name(&self) -> &str {
        if self.session.world_name.is_empty() {
            "untitled world"
        } else {
            &self.session.world_name
        }
    }

    /// Persist pane sizing back to the session (best-effort).
    fn persist_sizing(&mut self) {
        self.session.left_split = self.left_split;
        self.session.split_ratio = self.split_ratio;
        if let Err(e) = self.session.save(&self.layout) {
            self.status = format!("session save failed: {e}");
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        // Universal quit.
        if ctrl && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q')) {
            self.should_quit = true;
            return;
        }
        // Ctrl+R cycles the right-pane view from anywhere.
        if ctrl && key.code == KeyCode::Char('r') {
            self.right_pane = self.right_pane.next();
            self.status = format!("right pane: {}", self.right_pane.title());
            return;
        }
        // Focus movement (the overlay, once it exists, is sticky via Focus::next).
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
            Focus::QueryPrompt => match key.code {
                KeyCode::Esc => {
                    // Clear the line, then step focus back to where we came from.
                    if self.query.is_empty() {
                        self.focus = self.prev_focus;
                    } else {
                        self.query.select_all();
                        self.query.cut();
                    }
                }
                // Enter is wired to command dispatch in WB-P4; a no-op for now.
                KeyCode::Enter => {}
                _ => {
                    self.query.input(key);
                }
            },
            // Left/right pane key routing lands in WB-P1/P2. WB-P0: a couple of
            // shell-wide conveniences reachable from the navigable panes.
            Focus::FactsPane | Focus::WorldPane | Focus::RightPane => match key.code {
                KeyCode::Char('?') => self.show_hints = !self.show_hints,
                KeyCode::Char('q') => self.should_quit = true,
                // Resize gestures (WB-P1 refines per-pane; the shell honours them now).
                KeyCode::Char('{') => {
                    self.left_split = (self.left_split - 1).clamp(2, 8);
                    self.persist_sizing();
                }
                KeyCode::Char('}') => {
                    self.left_split = (self.left_split + 1).clamp(2, 8);
                    self.persist_sizing();
                }
                KeyCode::Char('[') => {
                    self.split_ratio = (self.split_ratio - 1).clamp(2, 8);
                    self.persist_sizing();
                }
                KeyCode::Char(']') => {
                    self.split_ratio = (self.split_ratio + 1).clamp(2, 8);
                    self.persist_sizing();
                }
                _ => {}
            },
            Focus::ConfirmationOverlay => {}
        }
    }
}

impl TuiHost for WorldbuilderApp {
    fn should_quit(&self) -> bool {
        self.should_quit
    }

    fn render(&self, frame: &mut Frame) {
        super::render::render(frame, self);
    }

    fn on_key(&mut self, key: KeyEvent) {
        // Remember where non-query focus was, so Esc from the prompt returns there.
        if self.focus != Focus::QueryPrompt {
            self.prev_focus = self.focus;
        }
        self.handle_key(key);
    }
}
