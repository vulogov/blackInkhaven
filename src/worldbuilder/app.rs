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

use tokio::sync::mpsc;

use crate::ai::stream::{ChatTurn as AiTurn, StreamMsg, spawn_chat_stream};
use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::{NodeKind, SYSTEM_TAG_FACTS, SYSTEM_TAG_WORLD, Store};
use crate::system_tree::SystemBookTree;
use crate::tui::theme::Theme;
use crate::tui_host::TuiHost;
use uuid::Uuid;

use super::WorldbuilderInvocation;
use super::focus::{Focus, RightPane};
use super::session::WorldbuilderSession;

/// The tag that marks a Facts-book paragraph as an *invented-world* fact — the
/// `/wfact` output and the `Ctrl+T` toggle. Rendered `◎` in the Facts pane.
pub(super) const FACT_WORLD_TAG: &str = "fact:world";

/// AI cost-dashboard category for worldbuilder chat inferences.
const WB_CATEGORY: &str = "worldbuilder";

/// One chat exchange in the worldbuilder conversation.
pub(super) struct WorldbuilderTurn {
    pub prompt: String,
    pub response: String,
    pub streaming: bool,
}

/// World-book chapters owned by `realworld compile` — read-only in the
/// worldbuilder (change them by editing `world.hjson` + recompiling, not by hand).
/// Matched case-insensitively against a node's title or an ancestor's.
const WORLD_COMPILER_CHAPTERS: &[&str] =
    &["astronomy", "geology", "climate", "hydrology", "demographics"];

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
    /// `z` zooms one left pane to fill the left column; `Some(pane)` while zoomed.
    pub(super) zoom: Option<Focus>,
    /// `Shift+F` in the Facts pane: emphasise only `fact:world` paragraphs.
    pub(super) facts_filter_world: bool,

    // — Query prompt (full width) ——————————————————————————————————————
    pub(super) query: TextArea<'static>,

    // — Chat (RightPane::Chat) + streaming ————————————————————————————
    pub(super) chat: Vec<WorldbuilderTurn>,
    pub(super) chat_scroll: u16,
    stream_rx: Option<mpsc::UnboundedReceiver<StreamMsg>>,
    streaming_turn: Option<usize>,

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
            zoom: None,
            facts_filter_world: false,
            query,
            chat: Vec::new(),
            chat_scroll: 0,
            stream_rx: None,
            streaming_turn: None,
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
                KeyCode::Enter => {
                    let text = self.query.lines().join("\n");
                    self.query.select_all();
                    self.query.cut();
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        // nothing to send
                    } else if trimmed.starts_with('/') {
                        // Slash-command dispatch lands in WB-P4.
                        self.status = format!("commands (`{trimmed}`) land in WB-P4");
                    } else {
                        self.send_chat(text);
                    }
                }
                _ => {
                    self.query.input(key);
                }
            },
            Focus::FactsPane => self.left_pane_key(key, true),
            Focus::WorldPane => self.left_pane_key(key, false),
            Focus::RightPane => {
                if self.common_pane_key(key) {
                    return;
                }
                if self.right_pane == RightPane::Chat {
                    match key.code {
                        // `u16::MAX` pins to the bottom (also while streaming).
                        KeyCode::Char('G') => self.chat_scroll = u16::MAX,
                        KeyCode::Char('g') => self.chat_scroll = 0,
                        KeyCode::Char('k') | KeyCode::Up => {
                            if self.chat_scroll != u16::MAX {
                                self.chat_scroll = self.chat_scroll.saturating_sub(1);
                            }
                        }
                        KeyCode::Char('j') | KeyCode::Down => {
                            if self.chat_scroll != u16::MAX {
                                self.chat_scroll = self.chat_scroll.saturating_add(1);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Focus::ConfirmationOverlay => {}
        }
    }

    /// Shell-wide keys reachable from any navigable pane. Returns whether the key
    /// was consumed.
    fn common_pane_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('?') => {
                self.show_hints = !self.show_hints;
                true
            }
            KeyCode::Char('q') => {
                self.should_quit = true;
                true
            }
            KeyCode::Char('{') => {
                self.left_split = self.left_split.saturating_sub(1).clamp(2, 8);
                self.persist_sizing();
                true
            }
            KeyCode::Char('}') => {
                self.left_split = (self.left_split + 1).clamp(2, 8);
                self.persist_sizing();
                true
            }
            KeyCode::Char('[') => {
                self.split_ratio = self.split_ratio.saturating_sub(1).clamp(2, 8);
                self.persist_sizing();
                true
            }
            KeyCode::Char(']') => {
                self.split_ratio = (self.split_ratio + 1).clamp(2, 8);
                self.persist_sizing();
                true
            }
            _ => false,
        }
    }

    /// Cursor / fold navigation on the focused tree. Returns whether consumed.
    fn tree_nav_key(&mut self, key: KeyEvent) -> bool {
        let (tree, h) = match self.focus {
            Focus::WorldPane => (&mut self.world_tree, &self.hierarchy),
            _ => (&mut self.facts_tree, &self.hierarchy),
        };
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                tree.move_down();
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                tree.move_up();
                true
            }
            KeyCode::Char('g') => {
                tree.to_top();
                true
            }
            KeyCode::Char('G') => {
                tree.to_bottom();
                true
            }
            KeyCode::Char('l') | KeyCode::Right => {
                tree.step_in(h);
                true
            }
            KeyCode::Char('h') | KeyCode::Left => {
                tree.step_out(h);
                true
            }
            KeyCode::Enter => {
                tree.toggle(h);
                true
            }
            _ => false,
        }
    }

    /// Keys for a left (Facts / World) pane. `is_facts` selects Facts-only keys.
    fn left_pane_key(&mut self, key: KeyEvent, is_facts: bool) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl {
            match key.code {
                KeyCode::Char('p') => {
                    self.pin_toggle();
                    return;
                }
                KeyCode::Char('t') if is_facts => {
                    self.toggle_world_tag();
                    return;
                }
                _ => {}
            }
        }
        if self.common_pane_key(key) {
            return;
        }
        if self.tree_nav_key(key) {
            return;
        }
        match key.code {
            KeyCode::Char('z') => {
                self.zoom = if self.zoom == Some(self.focus) { None } else { Some(self.focus) };
            }
            KeyCode::Char('F') if is_facts => {
                self.facts_filter_world = !self.facts_filter_world;
                self.status = if self.facts_filter_world {
                    "filter: fact:world only".into()
                } else {
                    "filter off".into()
                };
            }
            // Tree editing: WB-P1 rejects compiler-owned World nodes; author edits
            // (add / edit / rename / delete modals) land in a later phase.
            KeyCode::Char('n') | KeyCode::Char('e') | KeyCode::Char('r') | KeyCode::Char('d') => {
                self.reject_or_stub_edit(is_facts);
            }
            _ => {}
        }
    }

    /// `Ctrl+P` — toggle the selected node in the focused pane's pin list.
    fn pin_toggle(&mut self) {
        let sel = match self.focus {
            Focus::WorldPane => self.world_tree.selected(),
            _ => self.facts_tree.selected(),
        };
        let Some(id) = sel else { return };
        let pins = match self.focus {
            Focus::WorldPane => &mut self.world_pins,
            _ => &mut self.facts_pins,
        };
        if let Some(pos) = pins.iter().position(|p| *p == id) {
            pins.remove(pos);
            self.status = "unpinned".into();
        } else {
            pins.push(id);
            self.status = "pinned".into();
        }
    }

    /// `Ctrl+T` (Facts pane) — add/remove the `fact:world` tag on the selected
    /// paragraph, persisted to the store.
    fn toggle_world_tag(&mut self) {
        let Some(id) = self.facts_tree.selected() else { return };
        let Some(node) = self.hierarchy.get(id) else { return };
        if node.kind != NodeKind::Paragraph {
            self.status = "select a fact (a paragraph) to tag fact:world".into();
            return;
        }
        let mut updated = node.clone();
        let now_tagged = match updated.tags.iter().position(|t| t == FACT_WORLD_TAG) {
            Some(pos) => {
                updated.tags.remove(pos);
                false
            }
            None => {
                updated.tags.push(FACT_WORLD_TAG.to_string());
                true
            }
        };
        if let Err(e) = self.store.raw().update_metadata(id, updated.to_json()) {
            self.status = format!("tag write failed: {e}");
            return;
        }
        self.reload_hierarchy();
        self.status = if now_tagged {
            "◎ tagged fact:world".into()
        } else {
            "untagged fact:world".into()
        };
    }

    fn reject_or_stub_edit(&mut self, is_facts: bool) {
        if !is_facts {
            if let Some(id) = self.world_tree.selected() {
                if self.is_world_compiler_owned(id) {
                    self.status =
                        "Managed by realworld compile — edit world.hjson, then /compile".into();
                    return;
                }
            }
        }
        self.status = "tree editing (add/edit/rename/delete) lands in a later phase".into();
    }

    /// Send a plain-language question to the World Builder AI, grounding it in the
    /// world state, pins, and retrieved world facts (WB-P2). Slash commands are
    /// routed elsewhere (WB-P4).
    fn send_chat(&mut self, query: String) {
        let query = query.trim().to_string();
        if query.is_empty() {
            return;
        }
        if self.stream_rx.is_some() {
            self.status = "a response is still streaming…".into();
            return;
        }
        let ai = match crate::ai::AiClient::from_config(&self.cfg.llm) {
            Ok(a) => a,
            Err(e) => {
                self.push_turn(query, format!("[no LLM provider: {e}]"));
                return;
            }
        };
        let (model, _env) = match ai.resolve_provider(&self.cfg.llm, None) {
            Ok(m) => m,
            Err(e) => {
                self.push_turn(query, format!("[provider error: {e}]"));
                return;
            }
        };

        // Assemble the four context sections (a leading warnings section arrives
        // in WB-P3). All immutable borrows — build the owned `system` before we
        // mutate the chat below.
        let world_state = super::prompt::world_declaration_summary(&self.layout.root).unwrap_or_default();
        let pinned_world = super::prompt::pinned_nodes_text(&self.store, &self.hierarchy, &self.world_pins);
        let world_facts = self
            .facts_tree
            .root
            .map(|bid| {
                super::prompt::retrieve_world_facts(&self.store, &self.hierarchy, &self.cfg, bid, &query)
            })
            .unwrap_or_default();
        let pinned_facts = super::prompt::pinned_nodes_text(&self.store, &self.hierarchy, &self.facts_pins);
        let system = super::prompt::build_system_prompt(
            self.world_name(),
            &self.cfg.language,
            "", // warnings — WB-P3
            &world_state,
            &pinned_world,
            &world_facts,
            &pinned_facts,
        );
        let history = self.replay_history();

        self.chat.push(WorldbuilderTurn {
            prompt: query.clone(),
            response: String::new(),
            streaming: true,
        });
        self.streaming_turn = Some(self.chat.len() - 1);
        self.chat_scroll = u16::MAX; // pin to bottom while streaming
        self.right_pane = RightPane::Chat; // surface the answer
        self.status = "asking the World Builder…".into();

        let rx = spawn_chat_stream(
            ai.client.clone(),
            model.to_string(),
            Some(system),
            history,
            query,
            WB_CATEGORY,
        );
        self.stream_rx = Some(rx);
    }

    fn push_turn(&mut self, prompt: String, response: String) {
        self.chat.push(WorldbuilderTurn { prompt, response, streaming: false });
        self.chat_scroll = u16::MAX;
    }

    /// Prior completed turns, replayed to the model for follow-up context.
    fn replay_history(&self) -> Vec<AiTurn> {
        let mut h = Vec::new();
        for t in &self.chat {
            if t.streaming {
                continue;
            }
            h.push(AiTurn::User(t.prompt.clone()));
            if !t.response.trim().is_empty() && !t.response.starts_with('[') {
                h.push(AiTurn::Assistant(t.response.clone()));
            }
        }
        h
    }

    /// Drain the streaming channel into the in-flight turn (called each frame).
    fn drain_stream(&mut self) {
        let Some(rx) = self.stream_rx.as_mut() else { return };
        let Some(idx) = self.streaming_turn else { return };
        let mut done = false;
        loop {
            match rx.try_recv() {
                Ok(StreamMsg::Token(t)) => {
                    if let Some(turn) = self.chat.get_mut(idx) {
                        turn.response.push_str(&t);
                    }
                }
                Ok(StreamMsg::Done(_)) => {
                    done = true;
                    break;
                }
                Ok(StreamMsg::Error(e)) => {
                    if let Some(turn) = self.chat.get_mut(idx) {
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
            self.stream_rx = None;
            self.streaming_turn = None;
            if let Some(turn) = self.chat.get_mut(idx) {
                turn.streaming = false;
            }
            self.status = "ready".into();
        }
    }

    /// Whether a World node is owned by `realworld compile` (one of the compiled
    /// layers, or a descendant of one) — read-only in the worldbuilder.
    pub(super) fn is_world_compiler_owned(&self, id: Uuid) -> bool {
        node_is_compiler_owned(&self.hierarchy, id)
    }

    fn reload_hierarchy(&mut self) {
        if let Ok(h) = Hierarchy::load(&self.store) {
            self.hierarchy = h;
            self.facts_tree.rebuild(&self.hierarchy);
            self.world_tree.rebuild(&self.hierarchy);
        }
    }
}

impl TuiHost for WorldbuilderApp {
    fn should_quit(&self) -> bool {
        self.should_quit
    }

    fn poll_async(&mut self) {
        self.drain_stream();
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

/// Whether a World-book node is owned by `realworld compile` — it *is* one of the
/// compiled layer chapters (Astronomy/…/Demographics), or descends from one.
/// Free function so it's testable against a `Hierarchy` without a live store.
pub(super) fn node_is_compiler_owned(h: &Hierarchy, id: Uuid) -> bool {
    let Some(node) = h.get(id) else { return false };
    let is_layer = |n: &crate::store::node::Node| {
        WORLD_COMPILER_CHAPTERS.contains(&n.title.trim().to_lowercase().as_str())
    };
    is_layer(node) || h.ancestors(node).iter().any(|a| is_layer(a))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::node::Node;

    fn wnode(
        id: Uuid,
        kind: &str,
        title: &str,
        parent: Option<Uuid>,
        order: u32,
        system_tag: Option<&str>,
        tags: &[&str],
    ) -> Node {
        let mut raw = serde_json::json!({
            "id": id,
            "kind": kind,
            "title": title,
            "slug": title.to_lowercase(),
            "path": [],
            "parent_id": parent,
            "order": order,
            "file": null,
            "modified_at": "2026-01-01T00:00:00Z",
            "tags": tags,
        });
        if let Some(t) = system_tag {
            raw["system_tag"] = serde_json::json!(t);
        }
        serde_json::from_value(raw).expect("test node deserialises")
    }

    #[test]
    fn compiler_owned_covers_layer_chapters_and_their_descendants() {
        let world = Uuid::now_v7();
        let astronomy = Uuid::now_v7(); // compiled layer chapter
        let calendar = Uuid::now_v7(); // paragraph under Astronomy → owned
        let culture = Uuid::now_v7(); // author chapter
        let ethos = Uuid::now_v7(); // paragraph under Culture → author-owned
        let nodes = vec![
            wnode(world, "book", "World", None, 1, Some("world"), &[]),
            wnode(astronomy, "chapter", "Astronomy", Some(world), 1, None, &[]),
            wnode(calendar, "paragraph", "Calendar", Some(astronomy), 1, None, &[]),
            wnode(culture, "chapter", "Culture", Some(world), 2, None, &[]),
            wnode(ethos, "paragraph", "Ethos", Some(culture), 1, None, &[]),
        ];
        let h = Hierarchy::from_nodes_for_test(nodes);
        assert!(node_is_compiler_owned(&h, astronomy), "the layer chapter itself");
        assert!(node_is_compiler_owned(&h, calendar), "a descendant of a layer");
        assert!(!node_is_compiler_owned(&h, culture), "an author chapter");
        assert!(!node_is_compiler_owned(&h, ethos), "an author paragraph");
        assert!(!node_is_compiler_owned(&h, Uuid::now_v7()), "an unknown id");
    }

    #[test]
    fn fact_world_tag_marks_a_world_fact() {
        let tagged = wnode(Uuid::now_v7(), "paragraph", "Velmari harbours", None, 1, None, &[FACT_WORLD_TAG]);
        let plain = wnode(Uuid::now_v7(), "paragraph", "Bronze ports", None, 2, None, &[]);
        assert!(tagged.tags.iter().any(|t| t == FACT_WORLD_TAG));
        assert!(!plain.tags.iter().any(|t| t == FACT_WORLD_TAG));
    }
}
