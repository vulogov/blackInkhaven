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

/// A declared landmark resolved to a source-grid cell, for the map overlay (P2).
pub(super) struct MapMarker {
    pub x: usize,
    pub y: usize,
    pub name: String,
    pub kind: String,
}

/// What a map name-entry will place once a name is given (P2/P3).
pub(super) enum MapPlacement {
    /// A `geography.landmarks[]` entry of `geo_kind` at `(x, y)`.
    Landmark { geo_kind: &'static str, x: usize, y: usize },
    /// A `hydrology.rivers[]` entry from `from` to `to` (P3).
    River { from: (usize, usize), to: (usize, usize) },
    /// A `geography.regions[]` entry anchored at `(x, y)`, biome taken from the
    /// compiled cell under the cursor (P4).
    Region { x: usize, y: usize, biome: String },
}

/// An in-progress name entry for a feature being placed (P2/P3).
pub(super) struct MapInput {
    pub label: &'static str,
    pub placement: MapPlacement,
    pub buffer: String,
}

/// A multi-step map tool in progress (P3). `r` starts a river; the first Enter
/// sets its source, the second its mouth (then a name is requested).
pub(super) enum MapTool {
    River { source: Option<(usize, usize)> },
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

    // — World shaping (WB-P4) ——————————————————————————————————————————
    /// Accepted-but-uncommitted edits; `/write` folds them into world.hjson.
    pub(super) pending_ops: Vec<super::commands::Op>,
    /// A shaping delta awaiting the author's y/n confirmation.
    pub(super) hjson_preview: Option<(String, Vec<super::commands::Op>)>,

    // — Plausibility (WB-P3) ——————————————————————————————————————————
    pub(super) plausibility_score: Option<u8>,
    plausibility_prev: Option<u8>,
    plausibility_warnings: Vec<crate::world::plausibility::Warning>,

    // — Compiled world state (WB-P5) ——————————————————————————————————
    /// The last `/compile` result: a deterministic prose summary of the compiled
    /// layer chain. Fed to the chat system prompt instead of the declaration-only
    /// summary when present. Invalidated whenever the world changes.
    compiled_summary: Option<String>,
    /// The compiled layer grids from the last `/compile`, rendered as an ASCII
    /// biome minimap in the Map right-pane (WB-P6). Invalidated with the summary.
    pub(super) compiled_layers: Option<crate::world::plausibility::CompiledLayers>,

    // — Raster map (WS-P2) —————————————————————————————————————————————
    /// The terminal's image protocol, if it can display images and
    /// `images.preview_enabled` is on. `None` → the Map pane is always ASCII.
    image_picker: Option<ratatui_image::picker::Picker>,
    /// The last `/map` plakat raster, ready to paint in the Map pane. `RefCell`
    /// because the stateful image protocol mutates on render but `TuiHost::render`
    /// is `&self`. Invalidated whenever the world changes.
    pub(super) map_raster: Option<std::cell::RefCell<ratatui_image::protocol::StatefulProtocol>>,

    // — Map editor (MAPED-P1/P2) ———————————————————————————————————————
    /// Whether the Map pane is in edit mode (a movable grid cursor). Requires a
    /// compiled map (the ASCII grid); the raster is suppressed while editing.
    pub(super) map_edit: bool,
    /// The edit cursor in *source-grid* coordinates (`geology.width × height`).
    pub(super) map_cursor: (usize, usize),
    /// Declared landmarks resolved to source cells, for the map overlay (P2).
    /// Refreshed with `compiled_layers`; cleared when the world changes.
    pub(super) map_landmarks: Vec<MapMarker>,
    /// Declared regions with an anchor cell, for the map overlay (P4).
    pub(super) map_regions: Vec<MapMarker>,
    /// An open name-entry prompt for a feature being placed (P2/P3).
    pub(super) map_input: Option<MapInput>,
    /// A multi-step map tool in progress (P3 rivers).
    pub(super) map_tool: Option<MapTool>,
    /// The last `/mapcheck` findings — flagged on the map (`!`) and jumpable with
    /// `f` (P5). Cleared when the world changes.
    pub(super) map_findings: Vec<super::map::MapFinding>,
    /// Cursor into the spatial findings for `f` (P5).
    map_finding_idx: usize,

    // — World-fact research (WB-P7) ————————————————————————————————————
    /// The last `/research` query + its retrieved Facts passages, shown in the
    /// Research right-pane. `◎` marks passages already tagged `fact:world`.
    pub(super) research_query: Option<String>,
    pub(super) research_hits: Vec<crate::book_rag::RetrievedPassage>,
    /// Cursor into `research_hits` for the accept-as-fact keystroke (WS-P3).
    pub(super) research_cursor: usize,

    // — Interview (WB-P8) ——————————————————————————————————————————————
    /// The active guided interview, if the author started one (`/interview` or
    /// `--interview`). While `Some`, plain Query input answers the current step.
    pub(super) interview: Option<super::interview::Interview>,

    // — Magic ledger (WB-P9) ———————————————————————————————————————————
    /// The magic ledger of the current world (disk + pending), refreshed on every
    /// world change. Rendered + linted in the Ledger right-pane. `None` = no magic.
    pub(super) ledger_snapshot: Option<crate::world::types::MagicLedger>,

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
        let cfg_images_preview = cfg.images.preview_enabled;
        let mut query = TextArea::default();
        query.set_cursor_line_style(ratatui::style::Style::default());

        let left_split = session.left_split.clamp(2, 8);
        let split_ratio = session.split_ratio.clamp(2, 8);

        let mut app = WorldbuilderApp {
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
            // WB-P10 — restore the accepted-but-uncommitted delta from the session.
            pending_ops: session.pending_ops.clone(),
            hjson_preview: None,
            plausibility_score: None,
            compiled_summary: None,
            compiled_layers: None,
            // WS-P2 — query the terminal once for image support (gated by config).
            image_picker: if cfg_images_preview {
                ratatui_image::picker::Picker::from_query_stdio().ok()
            } else {
                None
            },
            map_raster: None,
            map_edit: false,
            map_cursor: (0, 0),
            map_landmarks: Vec::new(),
            map_regions: Vec::new(),
            map_input: None,
            map_tool: None,
            map_findings: Vec::new(),
            map_finding_idx: 0,
            research_query: None,
            research_hits: Vec::new(),
            research_cursor: 0,
            interview: None,
            ledger_snapshot: None,
            plausibility_prev: None,
            plausibility_warnings: Vec::new(),
            session,
            should_quit: false,
            status: "worldbuilder — Tab cycles panes · Ctrl+Q quits".to_string(),
            show_hints: true,
            theme,
        };
        app.refresh_plausibility();
        if !app.pending_ops.is_empty() {
            app.status = format!(
                "restored {} pending delta(s) from session · /diff to review · /write to commit",
                app.pending_ops.len()
            );
        }
        // `--interview` (and, until the plakat map-first flow lands, `--from-map`)
        // opens straight into the guided interview.
        if inv.interview || inv.from_map {
            app.start_interview();
        }
        Ok(app)
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

    /// WB-P10 — mirror the live pending delta + world name into the session and
    /// persist. Called after every change to `pending_ops`, so an accepted-but-
    /// uncommitted delta survives a quit.
    fn persist_pending(&mut self) {
        self.session.pending_ops = self.pending_ops.clone();
        if self.session.world_name.is_empty() {
            self.session.world_name = self.world_name().to_string();
        }
        if let Err(e) = self.session.save(&self.layout) {
            self.status = format!("session save failed: {e}");
        }
    }

    /// WB-P10 — append a turn to the session timeline (the "Worldbuilding
    /// Journey") and persist. `facts` are Facts-book node ids created this turn.
    fn record_turn(&mut self, user: String, summary: String, facts: Vec<String>) {
        let seq = self.session.turns.len() as u64 + 1;
        let at = chrono::Utc::now().to_rfc3339();
        self.session.turns.push(super::session::SessionTurn {
            seq,
            at,
            user,
            assistant_summary: summary,
            plausibility_before: self.plausibility_prev,
            plausibility_after: self.plausibility_score,
            facts_inserted: facts,
            ..Default::default()
        });
        // Keep the pending delta persisted in the same write.
        self.session.pending_ops = self.pending_ops.clone();
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
        // MAPED-P2 — a landmark name-entry prompt swallows all input.
        if self.map_input.is_some() {
            match key.code {
                KeyCode::Esc => {
                    self.map_input = None;
                    self.status = "placement cancelled".into();
                }
                KeyCode::Enter => {
                    if let Some(mi) = self.map_input.take() {
                        let name = mi.buffer.trim().to_string();
                        if name.is_empty() {
                            self.status = "name required — placement cancelled".into();
                        } else {
                            match mi.placement {
                                MapPlacement::Landmark { geo_kind, x, y } => {
                                    self.place_landmark(x, y, name, geo_kind)
                                }
                                MapPlacement::River { from, to } => {
                                    self.place_river(from, to, name)
                                }
                                MapPlacement::Region { x, y, biome } => {
                                    self.place_region(x, y, name, biome)
                                }
                            }
                        }
                    }
                }
                KeyCode::Backspace => {
                    if let Some(mi) = self.map_input.as_mut() {
                        mi.buffer.pop();
                    }
                }
                KeyCode::Char(c) => {
                    if let Some(mi) = self.map_input.as_mut() {
                        mi.buffer.push(c);
                    }
                }
                _ => {}
            }
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
                    // Esc leaves an active interview first; then clears the line;
                    // then steps focus back to where we came from.
                    if self.interview.is_some() {
                        self.interview = None;
                        self.query.select_all();
                        self.query.cut();
                        self.status = "interview left — pending edits kept (/diff to review)".into();
                    } else if self.query.is_empty() {
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
                    // While interviewing, plain text answers the current step; a
                    // `/command` still runs (so /diff, /write work mid-interview).
                    if self.interview.is_some() && !trimmed.starts_with('/') {
                        self.submit_interview_answer(trimmed);
                    } else if trimmed.is_empty() {
                        // nothing to send
                    } else if trimmed.starts_with('/') {
                        self.dispatch_command(&text);
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
                } else if self.right_pane == RightPane::Map {
                    // MAPED-P1/P2/P3 — grid cursor, placement tools, river tool.
                    if self.map_edit {
                        if self.try_map_move(key) {
                            // cursor moved
                        } else if self.map_tool.is_some() {
                            // A multi-step tool is active: Enter advances it.
                            match key.code {
                                KeyCode::Esc => {
                                    self.map_tool = None;
                                    self.status = "tool cancelled".into();
                                }
                                KeyCode::Enter => self.river_pick(),
                                _ => {}
                            }
                        } else {
                            match key.code {
                                KeyCode::Esc | KeyCode::Char('e') => {
                                    self.map_edit = false;
                                    self.status = "left map edit".into();
                                }
                                KeyCode::Char('t') => self.open_map_input("Town name", "city"),
                                KeyCode::Char('n') => self.open_map_input("Landmark name", "landmark"),
                                KeyCode::Char('g') => self.open_region_input(),
                                KeyCode::Char('f') => self.jump_next_finding(),
                                KeyCode::Char('d') | KeyCode::Char('x') => {
                                    self.delete_feature_at_cursor()
                                }
                                KeyCode::Char('r') => {
                                    self.map_tool = Some(MapTool::River { source: None });
                                    self.status =
                                        "river — move to the source, Enter to set · Esc cancel".into();
                                }
                                _ => {}
                            }
                        }
                    } else if key.code == KeyCode::Char('e') {
                        self.enter_map_edit();
                    }
                } else if self.right_pane == RightPane::Research {
                    // WS-P3 — move over hits; `a` promotes one to a world fact.
                    match key.code {
                        KeyCode::Char('j') | KeyCode::Down => {
                            let n = self.research_hits.len();
                            if n > 0 {
                                self.research_cursor = (self.research_cursor + 1).min(n - 1);
                            }
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            self.research_cursor = self.research_cursor.saturating_sub(1);
                        }
                        KeyCode::Char('a') => self.accept_research_hit(),
                        _ => {}
                    }
                }
            }
            Focus::ConfirmationOverlay => match key.code {
                KeyCode::Char('y') => self.accept_pending(),
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.hjson_preview = None;
                    self.focus = Focus::QueryPrompt;
                    self.status = "delta discarded".into();
                }
                _ => {}
            },
        }
    }

    /// Route a `/command` from the Query prompt (WB-P4).
    fn dispatch_command(&mut self, input: &str) {
        use super::commands::Command;
        match super::commands::parse(input) {
            Command::Shape { label, ops } => {
                // Preview before it enters the pending delta.
                self.hjson_preview = Some((label, ops));
                self.focus = Focus::ConfirmationOverlay;
                self.status = "review the delta — y accept · n discard".into();
            }
            Command::Write => self.write_pending(),
            Command::Undo => self.undo_pending(),
            Command::Reset => {
                let n = self.pending_ops.len();
                self.pending_ops.clear();
                self.refresh_plausibility();
                self.persist_pending();
                self.status = format!("reset — discarded {n} pending delta(s)");
            }
            Command::Diff => {
                self.status = if self.pending_ops.is_empty() {
                    "no pending deltas".into()
                } else {
                    format!(
                        "{} pending delta(s): {}",
                        self.pending_ops.len(),
                        self.pending_ops.iter().map(|o| o.preview()).collect::<Vec<_>>().join(" · ")
                    )
                };
            }
            Command::Compile => self.run_compile(),
            Command::Validate => self.run_validate(),
            Command::Wfact(text) => self.run_wfact(&text),
            Command::Research(query) => self.run_research(&query),
            Command::Interview => self.start_interview(),
            Command::Journey => self.run_journey(),
            Command::Sessions => self.run_sessions(),
            Command::Export { pdf } => self.run_export(pdf),
            Command::Switch(name) => self.run_switch(&name),
            Command::Roll(n) => self.run_roll(n),
            Command::Map => self.run_map(),
            Command::MapCheck => self.run_mapcheck(),
            Command::Unknown(msg) => self.status = msg,
        }
    }

    /// Collect every `fact:world` paragraph as `(title, body)`, in tree order.
    fn collect_world_facts(&self) -> Vec<(String, String)> {
        let Some(book_id) = self.facts_tree.root else { return Vec::new() };
        let mut out = Vec::new();
        // Walk the Facts tree depth-first so facts export in reading order.
        let mut stack: Vec<Uuid> = self
            .hierarchy
            .children_of(Some(book_id))
            .into_iter()
            .rev()
            .map(|n| n.id)
            .collect();
        while let Some(id) = stack.pop() {
            if let Some(node) = self.hierarchy.get(id) {
                if node.kind == NodeKind::Paragraph
                    && node.tags.iter().any(|t| t == FACT_WORLD_TAG)
                {
                    let body = self
                        .store
                        .get_content(id)
                        .ok()
                        .flatten()
                        .map(|b| String::from_utf8_lossy(&b).into_owned())
                        .unwrap_or_default();
                    out.push((node.title.clone(), body));
                }
                for child in self.hierarchy.children_of(Some(id)).into_iter().rev() {
                    stack.push(child.id);
                }
            }
        }
        out
    }

    /// `/export` — assemble a readable Markdown dossier (compiled state,
    /// plausibility, ledger, `fact:world` facts, and the Journey) and write it
    /// atomically under `exports/`. Compiles the world fresh so it works even
    /// `/roll [n]` (WS-P1) — compile `n` candidate worlds from the current
    /// declaration on derived seeds (`base + i`) and report a comparison into
    /// Chat. Pure and deterministic — the same declaration under different seeds
    /// yields decorrelated worlds (the compiler keys its SplitMix64 on the seed),
    /// so this explores the space of worlds the physics implies. `/adopt <seed>`
    /// then sets one as a pending edit.
    fn run_roll(&mut self, n: usize) {
        let Some(base) = self.current_world_def() else {
            self.status = "no world to roll — declare one first (interview or /set)".into();
            return;
        };
        let base_seed = base.seed_u64();
        let mut body = format!("Seed roll — {n} candidate(s), base 0x{base_seed:x}\n");
        body.push_str("  seed              ★   cont  sea%    pop     °C\n");
        for i in 0..n {
            let seed = base_seed.wrapping_add(i as u64);
            let mut def = base.clone();
            def.seed = crate::world::types::SeedValue::Str(format!("0x{seed:x}"));
            let layers = crate::world::plausibility::compile_layers(&def);
            let warns = crate::world::plausibility::run_fast(&def);
            let score = crate::world::plausibility::compute_plausibility_score(&warns);
            let (g, c, d) = (&layers.geology, &layers.climate, &layers.demographics);
            let marker = if i == 0 { "*" } else { " " };
            body.push_str(&format!(
                "{marker} 0x{seed:<13x} {score:>3}  {:>4}  {:>4.0}  {:>7}  {:>5.1}\n",
                g.continents,
                g.sea_coverage_pct,
                Self::compact_pop(d.total_population),
                c.mean_land_temp_c,
            ));
        }
        body.push_str("(* = current) · /adopt <seed> to set one (a pending edit → /write)");
        self.push_turn(format!("/roll {n}"), body);
        self.status = format!("rolled {n} candidate world(s) · /adopt <seed>");
    }

    /// `/map` (WS-P2) — render the world map with plakat and show the raster in
    /// the Map pane on image-capable terminals; otherwise the ASCII biome map
    /// stands in. Graceful at every step: no image support, no plakat, or a render
    /// failure each fall back to ASCII with a status note.
    fn run_map(&mut self) {
        self.right_pane = RightPane::Map;
        // Keep the ASCII fallback populated so the pane always shows something.
        if self.compiled_layers.is_none() {
            if let Some(def) = self.current_world_def() {
                self.populate_map_caches(&def);
            }
        }
        if self.image_picker.is_none() {
            self.status = "Map: this terminal can't display images — showing the ASCII map \
                           (needs kitty/iTerm2/sixel + images.preview_enabled)"
                .into();
            return;
        }
        let Some(def) = self.current_world_def() else {
            self.status = "no world to map — declare one first (interview or /set)".into();
            return;
        };
        let art = match crate::cli::realworld::render_world_map(&self.layout.root, &def, None) {
            Ok(a) => a,
            Err(e) => {
                self.map_raster = None;
                self.status = format!("map: {e} — showing the ASCII map");
                return;
            }
        };
        let img = std::fs::read(&art.png_path)
            .ok()
            .and_then(|bytes| image::load_from_memory(&bytes).ok());
        match img {
            Some(img) => {
                // Borrow the picker only for the protocol build, then store.
                let proto = self.image_picker.as_ref().unwrap().new_resize_protocol(img);
                self.map_raster = Some(std::cell::RefCell::new(proto));
                self.status = "rendered the world map (plakat raster)".into();
            }
            None => {
                self.map_raster = None;
                self.status = "map rendered but its PNG could not be read — ASCII map".into();
            }
        }
    }

    /// The source-grid dimensions of the compiled map, if any (MAPED-P1).
    fn map_source_dims(&self) -> Option<(usize, usize)> {
        self.compiled_layers.as_ref().map(|l| (l.geology.width, l.geology.height))
    }

    /// `/mapcheck` (MAPED-P5) — check the map layer against the compiled world.
    /// Reports the plausibility score, the deterministic physics warnings, and the
    /// spatial map findings (a town in the sea, an off-map coord) into Chat, flags
    /// the offending cells on the map (`!`), and jumps the cursor to the first.
    fn run_mapcheck(&mut self) {
        self.refresh_plausibility();
        let Some(def) = self.current_world_def() else {
            self.status = "no world to check — declare one first (interview or /set)".into();
            return;
        };
        self.populate_map_caches(&def);
        let Some(layers) = self.compiled_layers.as_ref() else {
            self.status = "no world to check".into();
            return;
        };
        let findings = super::map::lint_map(&def, layers);

        let mut body = format!(
            "Map check — plausibility {}/100\n",
            self.plausibility_score.unwrap_or(100)
        );
        if findings.is_empty() {
            body.push_str("Map layer: no problems found.\n");
        } else {
            body.push_str(&format!("Map layer — {} issue(s):\n", findings.len()));
            for f in &findings {
                body.push_str(&format!("  ! {}\n", f.text));
            }
        }
        if !self.plausibility_warnings.is_empty() {
            body.push_str("World checks:\n");
            for w in self.plausibility_warnings.iter().take(8) {
                body.push_str(&format!("  · {}\n", w.text));
            }
        }

        let spatial = findings.iter().filter(|f| f.at.is_some()).count();
        self.map_findings = findings;
        self.map_finding_idx = 0;
        self.right_pane = RightPane::Map;
        if let Some(at) = self.map_findings.iter().find_map(|f| f.at) {
            self.map_cursor = at;
            self.enter_map_edit();
        }
        self.push_turn("/mapcheck".into(), body);
        self.status = format!("map check — {spatial} spatial issue(s) · f: jump to next");
    }

    /// `f` in the Map pane — jump the cursor to the next spatial `/mapcheck`
    /// finding and echo its message (P5).
    fn jump_next_finding(&mut self) {
        let spatial: Vec<(usize, usize, String)> = self
            .map_findings
            .iter()
            .filter_map(|f| f.at.map(|at| (at.0, at.1, f.text.clone())))
            .collect();
        if spatial.is_empty() {
            self.status = "no map-check findings — run /mapcheck".into();
            return;
        }
        self.map_finding_idx = (self.map_finding_idx + 1) % spatial.len();
        let (x, y, text) = &spatial[self.map_finding_idx];
        self.map_cursor = (*x, *y);
        self.status = format!("[{}/{}] {text}", self.map_finding_idx + 1, spatial.len());
    }

    /// Enter Map edit mode. Auto-compiles the map if needed. Clamps the cursor
    /// into the current grid.
    fn enter_map_edit(&mut self) {
        if self.compiled_layers.is_none() {
            if let Some(def) = self.current_world_def() {
                self.populate_map_caches(&def);
            }
        }
        let Some((w, h)) = self.map_source_dims() else {
            self.status = "no world to map — declare one first (interview or /set)".into();
            return;
        };
        self.map_edit = true;
        let (cx, cy) = self.map_cursor;
        self.map_cursor = (cx.min(w.saturating_sub(1)), cy.min(h.saturating_sub(1)));
        self.status = "map edit — hjkl move · t town · n name · d delete · Esc leave".into();
    }

    /// Open the name-entry prompt for placing a landmark of `geo_kind` at the
    /// cursor (P2).
    fn open_map_input(&mut self, label: &'static str, geo_kind: &'static str) {
        let (x, y) = self.map_cursor;
        self.map_input = Some(MapInput {
            label,
            placement: MapPlacement::Landmark { geo_kind, x, y },
            buffer: String::new(),
        });
        self.status = format!("{label} at ({x},{y}) — type a name, Enter to place, Esc to cancel");
    }

    /// Open the region name-entry, auto-filling the biome from the compiled cell
    /// under the cursor (P4).
    fn open_region_input(&mut self) {
        let (x, y) = self.map_cursor;
        let biome = self
            .compiled_layers
            .as_ref()
            .and_then(|l| {
                let idx = y.min(l.climate.height.saturating_sub(1)) * l.climate.width
                    + x.min(l.climate.width.saturating_sub(1));
                l.climate.biome.get(idx).map(|b| b.as_str().to_string())
            })
            .unwrap_or_default();
        self.map_input = Some(MapInput {
            label: "Region name",
            placement: MapPlacement::Region { x, y, biome: biome.clone() },
            buffer: String::new(),
        });
        self.status = format!("Region at ({x},{y}) · biome {biome} — type a name, Enter to place");
    }

    /// Place a region anchor into `geography.regions[]` as a pending edit (P4).
    fn place_region(&mut self, x: usize, y: usize, name: String, biome: String) {
        let value = serde_json::json!({ "name": name, "biome": biome, "x": x, "y": y });
        self.pending_ops.push(super::commands::Op::Push {
            path: vec!["geography".into(), "regions".into()],
            value,
        });
        self.refresh_plausibility();
        if let Some(def) = self.current_world_def() {
            self.populate_map_caches(&def);
        }
        self.record_turn(format!("map: region {name}"), format!("{biome} at ({x},{y})"), Vec::new());
        self.status = format!("placed region '{name}' ({biome}) at ({x},{y}) · /write to commit");
    }

    /// Advance the river tool (P3): the first Enter fixes the source, the second
    /// fixes the mouth and asks for a name.
    fn river_pick(&mut self) {
        let cursor = self.map_cursor;
        match self.map_tool {
            Some(MapTool::River { source: None }) => {
                self.map_tool = Some(MapTool::River { source: Some(cursor) });
                self.status = format!(
                    "river source ({},{}) — move to the mouth, Enter to set · Esc cancel",
                    cursor.0, cursor.1
                );
            }
            Some(MapTool::River { source: Some(src) }) => {
                self.map_tool = None;
                self.map_input = Some(MapInput {
                    label: "River name",
                    placement: MapPlacement::River { from: src, to: cursor },
                    buffer: String::new(),
                });
                self.status = "name the river — Enter to place, Esc to cancel".into();
            }
            None => {}
        }
    }

    /// Place a river into `hydrology.rivers[]` as a pending edit, re-populate the
    /// map (the compiled hydrology honours the declared course, so it renders),
    /// and surface `lint_rivers` immediately (P3).
    fn place_river(&mut self, from: (usize, usize), to: (usize, usize), name: String) {
        let value = serde_json::json!({
            "name": name,
            "from": [from.0, from.1],
            "to": [to.0, to.1],
        });
        self.pending_ops.push(super::commands::Op::Push {
            path: vec!["hydrology".into(), "rivers".into()],
            value,
        });
        self.refresh_plausibility();
        if let Some(def) = self.current_world_def() {
            self.populate_map_caches(&def);
        }
        self.record_turn(
            format!("map: river {name}"),
            format!("({},{}) → ({},{})", from.0, from.1, to.0, to.1),
            Vec::new(),
        );
        let lint = self.river_lint_summary();
        if lint.is_empty() {
            self.status = format!("placed river '{name}' · /write to commit");
        } else {
            self.push_turn(format!("river {name}"), format!("⚠ {lint}"));
            self.status = format!("river '{name}' placed — check: {lint}");
        }
    }

    /// The current declared-river lint findings, joined for a status/chat line.
    fn river_lint_summary(&self) -> String {
        let Some(def) = self.current_world_def() else { return String::new() };
        let Some(layers) = self.compiled_layers.as_ref() else { return String::new() };
        let Some(hydro_def) = def.hydrology.as_ref() else { return String::new() };
        crate::world::compile::hydrology_layer::lint_rivers(hydro_def, &layers.geology)
            .iter()
            .map(|w| w.text.clone())
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// Place a named landmark into `geography.landmarks[]` as a pending edit, then
    /// re-populate the map caches so it appears immediately (P2).
    fn place_landmark(&mut self, x: usize, y: usize, name: String, geo_kind: &str) {
        let value = serde_json::json!({ "name": name, "kind": geo_kind, "x": x, "y": y });
        self.pending_ops.push(super::commands::Op::Push {
            path: vec!["geography".into(), "landmarks".into()],
            value,
        });
        self.refresh_plausibility();
        if let Some(def) = self.current_world_def() {
            self.populate_map_caches(&def);
        }
        self.record_turn(format!("map: {geo_kind} {name}"), format!("placed at ({x},{y})"), Vec::new());
        self.status = format!("placed {geo_kind} '{name}' at ({x},{y}) · /write to commit");
    }

    /// Delete the declared feature under the cursor — a landmark first, else a
    /// region — via `Op::RemoveAt` (index computed against the current disk +
    /// pending array, which is the array state at write time) (P2/P4).
    fn delete_feature_at_cursor(&mut self) {
        let Some((w, h)) = self.map_source_dims() else { return };
        let cursor = self.map_cursor;
        let Some(def) = self.current_world_def() else { return };
        let geo = def.geography.as_ref();
        // Landmark under the cursor?
        if let Some(i) =
            geo.and_then(|g| g.landmarks.iter().position(|lm| lm.grid(w, h) == Some(cursor)))
        {
            self.remove_feature("landmarks", i, "landmark", cursor);
            return;
        }
        // Else a region anchored on this cell?
        if let Some(i) = geo.and_then(|g| {
            g.regions.iter().position(|r| match (r.x, r.y) {
                (Some(x), Some(y)) => (x.min(w - 1), y.min(h - 1)) == cursor,
                _ => false,
            })
        }) {
            self.remove_feature("regions", i, "region", cursor);
            return;
        }
        self.status = "no landmark or region at the cursor".into();
    }

    /// Push a `RemoveAt` for `geography.<array>[index]`, re-populate, and report.
    fn remove_feature(&mut self, array: &str, index: usize, label: &str, at: (usize, usize)) {
        self.pending_ops.push(super::commands::Op::RemoveAt {
            path: vec!["geography".into(), array.into()],
            index,
        });
        self.refresh_plausibility();
        if let Some(def) = self.current_world_def() {
            self.populate_map_caches(&def);
        }
        self.status = format!("removed {label} at ({},{}) · /write to commit", at.0, at.1);
    }

    /// Route a movement key to the cursor, returning whether it was one. Handles
    /// coarse (hjkl / arrows) and fine (HJKL / Shift+arrows) steps. Shared by all
    /// map tools so navigation always works.
    fn try_map_move(&mut self, key: KeyEvent) -> bool {
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::Left => self.move_map_cursor(-1, 0, shift),
            KeyCode::Right => self.move_map_cursor(1, 0, shift),
            KeyCode::Up => self.move_map_cursor(0, -1, shift),
            KeyCode::Down => self.move_map_cursor(0, 1, shift),
            KeyCode::Char('h') => self.move_map_cursor(-1, 0, false),
            KeyCode::Char('l') => self.move_map_cursor(1, 0, false),
            KeyCode::Char('k') => self.move_map_cursor(0, -1, false),
            KeyCode::Char('j') => self.move_map_cursor(0, 1, false),
            KeyCode::Char('H') => self.move_map_cursor(-1, 0, true),
            KeyCode::Char('L') => self.move_map_cursor(1, 0, true),
            KeyCode::Char('K') => self.move_map_cursor(0, -1, true),
            KeyCode::Char('J') => self.move_map_cursor(0, 1, true),
            _ => return false,
        }
        true
    }

    /// Move the edit cursor in source-grid cells. `fine` steps a single cell;
    /// otherwise a coarse step (~1/40 of the grid) for quick travel. Clamped.
    fn move_map_cursor(&mut self, dx: i32, dy: i32, fine: bool) {
        let Some((w, h)) = self.map_source_dims() else { return };
        let step_x = if fine { 1 } else { (w / 40).max(1) } as i32;
        let step_y = if fine { 1 } else { (h / 40).max(1) } as i32;
        let (cx, cy) = self.map_cursor;
        let nx = (cx as i32 + dx * step_x).clamp(0, w as i32 - 1) as usize;
        let ny = (cy as i32 + dy * step_y).clamp(0, h as i32 - 1) as usize;
        self.map_cursor = (nx, ny);
    }

    /// A compact population string (`4.1M`, `820k`, `512`) for the roll table.
    fn compact_pop(p: u64) -> String {
        if p >= 1_000_000 {
            format!("{:.1}M", p as f64 / 1_000_000.0)
        } else if p >= 1_000 {
            format!("{:.0}k", p as f64 / 1_000.0)
        } else {
            p.to_string()
        }
    }

    /// before `/compile`.
    fn run_export(&mut self, pdf: bool) {
        let compiled = self
            .current_world_def()
            .map(|def| {
                let layers = crate::world::plausibility::compile_layers(&def);
                crate::world::plausibility::summarise_compiled(&def, &layers)
            });
        let facts = self.collect_world_facts();
        let at = chrono::Utc::now().to_rfc3339();
        let world_name = self.world_name().to_string();
        let input = super::export::DossierInput {
            world_name: &world_name,
            generated_at: &at,
            compiled: compiled.as_deref(),
            score: self.plausibility_score,
            warnings: &self.plausibility_warnings,
            ledger: self.ledger_snapshot.as_ref(),
            facts: &facts,
            journey: &self.session.turns,
        };
        let md = super::export::build_dossier(&input);
        let dir = self.layout.root.join("exports");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.status = format!("export failed: {e}");
            return;
        }
        let md_path = dir.join(format!("dossier-{}.md", self.session.slug));
        if let Err(e) = crate::io_atomic::write(&md_path, md.as_bytes()) {
            self.status = format!("export failed: {e}");
            return;
        }
        let md_rel = md_path.strip_prefix(&self.layout.root).unwrap_or(&md_path).display().to_string();

        if !pdf {
            self.push_turn("/export".into(), format!("Wrote world dossier → {md_rel}"));
            self.status = format!("exported → {md_rel}");
            return;
        }

        // `--pdf`: render the same dossier through Typst, in-process.
        let typ = super::export::build_dossier_typst(&input);
        let settings = crate::typst_world::WorldSettings::from_cfg(&self.cfg.typst_compile);
        match crate::typst_inprocess::compile_source_to_pdf(&self.layout.root, typ, settings) {
            Ok(bytes) => {
                let pdf_path = dir.join(format!("dossier-{}.pdf", self.session.slug));
                match crate::io_atomic::write(&pdf_path, &bytes) {
                    Ok(()) => {
                        let pdf_rel = pdf_path.strip_prefix(&self.layout.root).unwrap_or(&pdf_path).display();
                        self.push_turn("/export --pdf".into(), format!("Wrote {md_rel} and {pdf_rel}"));
                        self.status = format!("exported → {pdf_rel}");
                    }
                    Err(e) => self.status = format!("wrote {md_rel}; PDF write failed: {e}"),
                }
            }
            Err(e) => {
                let first = e.lines().next().unwrap_or("compile error");
                self.push_turn("/export --pdf".into(), format!("Wrote {md_rel}; PDF compile failed: {first}"));
                self.status = format!("wrote {md_rel}; PDF failed (see Chat)");
            }
        }
    }

    /// `/journey` — render the session timeline (the Worldbuilding Journey) into
    /// the Chat pane: each recorded turn with its plausibility arc.
    fn run_journey(&mut self) {
        let count = self.session.turns.len();
        if count == 0 {
            self.push_turn("/journey".into(), "No journey yet — shape the world and it fills in.".into());
            self.status = "journey — empty".into();
            return;
        }
        let mut body = format!(
            "Worldbuilding Journey · {} · {count} step(s)\n",
            self.session.name,
        );
        for t in &self.session.turns {
            // Trim the timestamp to the date+minute for a compact line.
            let when = t.at.get(..16).unwrap_or(&t.at);
            let arc = match (t.plausibility_before, t.plausibility_after) {
                (Some(b), Some(a)) if a != b => format!("  ★{b}→{a}"),
                (_, Some(a)) => format!("  ★{a}"),
                _ => String::new(),
            };
            let facts = if t.facts_inserted.is_empty() {
                String::new()
            } else {
                format!("  ◎{}", t.facts_inserted.len())
            };
            body.push_str(&format!(
                "{:>3}. {when}  {} → {}{arc}{facts}\n",
                t.seq,
                t.user.trim(),
                t.assistant_summary.trim(),
            ));
        }
        self.push_turn("/journey".into(), body);
        self.status = format!("journey — {count} step(s)");
    }

    /// `/sessions` — list the project's worldbuilder sessions (the current one
    /// marked). Session switching is a later refinement; this surfaces what exists.
    fn run_sessions(&mut self) {
        let slugs = super::session::WorldbuilderSession::list(&self.layout);
        if slugs.is_empty() {
            self.status = "no sessions".into();
            return;
        }
        let list = slugs
            .iter()
            .map(|s| if *s == self.session.slug { format!("• {s} (current)") } else { format!("  {s}") })
            .collect::<Vec<_>>()
            .join("\n");
        self.push_turn("/sessions".into(), format!("Sessions:\n{list}"));
        self.status = format!("{} session(s) — /switch <name> or --session <name>", slugs.len());
    }

    /// `/switch <name>` (WS-P3) — persist the current session, then open (or
    /// create) the named one and swap in its pending delta. `world.hjson` is
    /// shared across a project's sessions, so switching changes only the pending
    /// edits, timeline, and pane sizing — and starts a fresh conversation.
    fn run_switch(&mut self, name: &str) {
        // Flush the current session (pending + turns are mirrored on change;
        // capture the latest sizing too).
        self.session.left_split = self.left_split;
        self.session.split_ratio = self.split_ratio;
        self.session.pending_ops = self.pending_ops.clone();
        let _ = self.session.save(&self.layout);

        let now = chrono::Utc::now().to_rfc3339();
        let target = match super::session::WorldbuilderSession::open_or_create(&self.layout, name, now) {
            Ok(s) => s,
            Err(e) => {
                self.status = format!("switch failed: {e}");
                return;
            }
        };
        let slug = target.slug.clone();
        self.left_split = target.left_split.clamp(2, 8);
        self.split_ratio = target.split_ratio.clamp(2, 8);
        self.pending_ops = target.pending_ops.clone();
        self.session = target;
        // Fresh conversation + research state for the new session.
        self.chat.clear();
        self.chat_scroll = 0;
        self.research_query = None;
        self.research_hits.clear();
        self.research_cursor = 0;
        self.interview = None;
        self.hjson_preview = None;
        self.refresh_plausibility(); // rescores pending; clears compiled/map caches
        self.status = format!("switched to session '{slug}' · {} pending delta(s)", self.pending_ops.len());
    }

    /// Start (or restart) the guided interview: focus the Query prompt, surface
    /// the Chat pane, and post the first question. Answers accumulate into the
    /// pending delta (reviewable with `/diff`, committed with `/write`).
    fn start_interview(&mut self) {
        let iv = super::interview::Interview::new();
        self.interview = Some(iv);
        self.right_pane = RightPane::Chat;
        self.focus = Focus::QueryPrompt;
        self.push_turn(
            String::new(),
            "Interview — I'll ask about the sky, land, people, and rules. Answer in your own \
             words (blank to skip a question, Esc to leave). Your answers become pending edits; \
             review them with /diff and commit with /write, then /compile."
                .to_string(),
        );
        self.post_interview_question();
        self.status = "interview started — answer in the Query prompt · Esc to leave".into();
    }

    /// Post the current interview step as a chat turn (pinned to the bottom, so it
    /// stays visible as the conversation grows).
    fn post_interview_question(&mut self) {
        let q = self.interview.as_ref().and_then(|iv| {
            iv.current().map(|s| {
                let (n, total) = iv.progress();
                format!("[{} · {n}/{total}] {}", s.stage.label(), s.prompt)
            })
        });
        if let Some(q) = q {
            self.push_turn(String::new(), q);
        }
    }

    /// Feed one answer to the active interview: fill the current step's command
    /// template, record its ops into the pending delta, and advance. A blank
    /// answer skips; a malformed one is reported and the step is retried.
    fn submit_interview_answer(&mut self, answer: &str) {
        let answer = answer.trim();
        let Some(step) = self.interview.as_ref().and_then(|iv| iv.current()) else {
            self.interview = None;
            return;
        };
        // Blank → skip this question.
        if answer.is_empty() {
            if let Some(iv) = self.interview.as_mut() {
                iv.advance();
            }
            self.push_turn(String::new(), "(skipped)".into());
            self.after_interview_step();
            return;
        }

        let cmd = step.template.replace("{}", answer);
        match super::commands::parse(&cmd) {
            super::commands::Command::Shape { label, ops } => {
                self.pending_ops.extend(ops);
                self.refresh_plausibility();
                let d = self.plausibility_delta_chip();
                let note = if d.is_empty() {
                    format!("recorded · {label}")
                } else {
                    format!("recorded · {label}  (★ {d})")
                };
                self.push_turn(answer.to_string(), note);
                self.record_turn(format!("interview: {answer}"), label, Vec::new());
                if let Some(iv) = self.interview.as_mut() {
                    iv.advance();
                }
                self.after_interview_step();
            }
            super::commands::Command::Unknown(msg) => {
                // Keep the step; let the author retry.
                self.push_turn(answer.to_string(), format!("didn't take that — {msg}"));
            }
            _ => {
                // A template that parses to a session command shouldn't happen;
                // skip defensively rather than loop.
                if let Some(iv) = self.interview.as_mut() {
                    iv.advance();
                }
                self.after_interview_step();
            }
        }
    }

    /// Post the next question, or close the interview when the script is done.
    fn after_interview_step(&mut self) {
        let done = self.interview.as_ref().map(|iv| iv.done()).unwrap_or(true);
        if done {
            self.interview = None;
            let n = self.pending_ops.len();
            self.push_turn(
                String::new(),
                format!(
                    "That's the frame — {n} pending edit(s). Review with /diff, commit with \
                     /write, then /compile to see the world your choices imply."
                ),
            );
            self.status = format!("interview complete — {n} pending edit(s) · /diff · /write");
        } else {
            self.post_interview_question();
            self.status = "interview — answer in the Query prompt · Esc to leave".into();
        }
    }

    /// `/wfact <statement>` — record an author-decided world fact into the Facts
    /// book, tagged `fact:world`, using the shared research insertion primitive
    /// (auto-reembeds → immediately retrievable). No AI: the author decides, the
    /// worldbuilder records. Inserts near the tree cursor when it is inside Facts,
    /// else at the end of the book; then reveals the new fact.
    fn run_wfact(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let Some(book_id) = self.facts_tree.root else {
            self.status = "no Facts book — create one in the editor first".into();
            return;
        };
        // A short title from the first clause/line; the full statement is the body.
        let title: String = text.split(['.', '\n']).next().unwrap_or(text).trim().chars().take(60).collect();
        let target = self.facts_tree.selected();
        let new_id = match crate::research::insert::insert_paragraph(
            &self.store,
            &self.cfg,
            &self.hierarchy,
            book_id,
            target,
            &title,
            text,
        ) {
            Ok(id) => id,
            Err(e) => {
                self.status = format!("fact write failed: {e}");
                return;
            }
        };
        // Tag it fact:world (the whole point — it must show as ◎ and feed the
        // world-fact RAG). Reload first so the node is in the hierarchy.
        self.reload_hierarchy();
        if let Some(node) = self.hierarchy.get(new_id) {
            let mut updated = node.clone();
            if !updated.tags.iter().any(|t| t == FACT_WORLD_TAG) {
                updated.tags.push(FACT_WORLD_TAG.to_string());
                if let Err(e) = self.store.raw().update_metadata(new_id, updated.to_json()) {
                    self.status = format!("recorded, but tag write failed: {e}");
                    return;
                }
            }
        }
        self.reload_hierarchy();
        self.facts_tree.reveal(&self.hierarchy, new_id);
        self.record_turn(format!("/wfact {title}"), "◎ recorded fact:world".into(), vec![new_id.to_string()]);
        self.status = "◎ recorded fact:world".into();
    }

    /// `/research <query>` — semantically retrieve related Facts (the whole Facts
    /// book, not only `fact:world`, so the author can discover material to promote)
    /// and surface them in the Research right-pane. Pure retrieval, no generation.
    fn run_research(&mut self, query: &str) {
        let query = query.trim();
        if query.is_empty() {
            return;
        }
        let Some(book_id) = self.facts_tree.root else {
            self.status = "no Facts book to research".into();
            return;
        };
        match crate::book_rag::retrieval::retrieve(
            &self.store,
            &self.hierarchy,
            &self.cfg.book_rag,
            book_id,
            query,
        ) {
            Ok(hits) => {
                let n = hits.len();
                self.research_hits = hits;
                self.research_cursor = 0;
                self.research_query = Some(query.to_string());
                self.right_pane = RightPane::Research;
                self.status = format!("research · {n} passage(s) for “{query}” · Ctrl+R → Research");
            }
            Err(e) => self.status = format!("research failed: {e}"),
        }
    }

    /// `/compile` — run the pure layer chain over the current world (disk +
    /// pending), cache a compiled-state summary for the chat prompt, and echo it
    /// into the conversation as a Simulation turn. No LLM, no disk writes.
    fn run_compile(&mut self) {
        match self.current_world_def() {
            Some(def) => {
                self.populate_map_caches(&def);
                let summary = self.compiled_summary.clone().unwrap_or_default();
                self.push_turn("/compile".into(), format!("Compiled world state —\n{summary}"));
                self.status =
                    "compiled — chat reasons over the simulated world · Ctrl+R → Map".into();
            }
            None => {
                self.status = "no world to compile — declare one first (interview or /set)".into();
            }
        }
    }

    /// Compile the world and refresh the Map caches together — `compiled_layers`
    /// (the biome/height grid), `compiled_summary`, and `map_landmarks` (declared
    /// landmarks resolved to source cells for the overlay). Landmark/river/region
    /// edits don't change the grid, so the editor re-runs this after a placement
    /// to keep the map live (refresh_plausibility having cleared the caches).
    fn populate_map_caches(&mut self, def: &crate::world::types::WorldDefinition) {
        let layers = crate::world::plausibility::compile_layers(def);
        self.compiled_summary =
            Some(crate::world::plausibility::summarise_compiled(def, &layers));
        let (w, h) = (layers.geology.width, layers.geology.height);
        self.map_landmarks = def
            .geography
            .as_ref()
            .map(|g| {
                g.landmarks
                    .iter()
                    .filter_map(|lm| {
                        lm.grid(w, h).map(|(x, y)| MapMarker {
                            x,
                            y,
                            name: lm.name.clone(),
                            kind: lm.kind.clone(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.map_regions = def
            .geography
            .as_ref()
            .map(|g| {
                g.regions
                    .iter()
                    .filter_map(|r| match (r.x, r.y) {
                        (Some(x), Some(y)) => Some(MapMarker {
                            x: x.min(w.saturating_sub(1)),
                            y: y.min(h.saturating_sub(1)),
                            name: r.name.clone(),
                            kind: if r.biome.is_empty() { "region".into() } else { r.biome.clone() },
                        }),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.compiled_layers = Some(layers);
    }

    /// `/validate` — run the deterministic plausibility lints over the current
    /// world and report the score + warnings into the conversation. Reuses the
    /// already-computed `plausibility_warnings` (kept fresh by every delta).
    fn run_validate(&mut self) {
        match self.plausibility_score {
            Some(score) => {
                let mut body = format!("Plausibility {score}/100");
                if self.plausibility_warnings.is_empty() {
                    body.push_str(" — no warnings.");
                } else {
                    body.push_str(&format!(" — {} warning(s):\n", self.plausibility_warnings.len()));
                    for w in &self.plausibility_warnings {
                        let sev = match w.severity {
                            crate::world::plausibility::Severity::High => "HIGH",
                            crate::world::plausibility::Severity::Medium => "MED ",
                            crate::world::plausibility::Severity::Low => "LOW ",
                        };
                        body.push_str(&format!("  [{sev}] {}\n", w.text));
                    }
                }
                self.push_turn("/validate".into(), body);
                self.status = format!("validated — plausibility {score}/100");
            }
            None => {
                self.status = "no world to validate — declare one first (interview or /set)".into();
            }
        }
    }

    /// `y` in the preview — fold the previewed ops into the pending delta and
    /// rescore (the score responds before `/write` commits to disk).
    fn accept_pending(&mut self) {
        if let Some((label, ops)) = self.hjson_preview.take() {
            self.pending_ops.extend(ops);
            self.refresh_plausibility();
            let d = self.plausibility_delta_chip();
            self.status = if d.is_empty() {
                format!("✓ {label} · /write to commit")
            } else {
                format!("✓ {label}  ★ {d} · /write to commit")
            };
            self.record_turn(label, "shaping delta accepted".into(), Vec::new());
        }
        self.focus = Focus::QueryPrompt;
    }

    /// `/write` — fold every pending op into `world.hjson` atomically.
    fn write_pending(&mut self) {
        if self.pending_ops.is_empty() {
            self.status = "nothing to write".into();
            return;
        }
        let path = self.layout.root.join("world.hjson");
        let mut value = std::fs::read_to_string(&path)
            .ok()
            .and_then(|r| serde_hjson::from_str::<serde_json::Value>(&r).ok())
            .unwrap_or_else(|| serde_json::json!({}));
        for op in &self.pending_ops {
            op.apply(&mut value);
        }
        let json = serde_json::to_string_pretty(&value).unwrap_or_default();
        match crate::io_atomic::write(&path, json.as_bytes()) {
            Ok(()) => {
                let n = self.pending_ops.len();
                self.pending_ops.clear();
                self.refresh_plausibility();
                self.status = format!("✓ wrote {n} delta(s) to world.hjson");
                self.record_turn("/write".into(), format!("committed {n} delta(s)"), Vec::new());
            }
            Err(e) => self.status = format!("write failed: {e}"),
        }
    }

    /// `/undo` — drop the last pending op and rescore.
    fn undo_pending(&mut self) {
        if self.pending_ops.pop().is_some() {
            self.refresh_plausibility();
            self.persist_pending();
            self.status = format!("undone — {} pending delta(s) left", self.pending_ops.len());
        } else {
            self.status = "nothing to undo".into();
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

    /// WS-P3 — promote the research hit under the cursor to a world fact: tag its
    /// (existing) Facts paragraph `fact:world`, so it joins the `◎` set and the
    /// world-fact RAG.
    fn accept_research_hit(&mut self) {
        let Some(hit) = self.research_hits.get(self.research_cursor) else {
            return;
        };
        let id = hit.id;
        let Some(node) = self.hierarchy.get(id) else {
            self.status = "that passage is no longer available".into();
            return;
        };
        if node.tags.iter().any(|t| t == FACT_WORLD_TAG) {
            self.status = "already a ◎ world fact".into();
            return;
        }
        let mut updated = node.clone();
        updated.tags.push(FACT_WORLD_TAG.to_string());
        if let Err(e) = self.store.raw().update_metadata(id, updated.to_json()) {
            self.status = format!("tag write failed: {e}");
            return;
        }
        self.reload_hierarchy();
        self.status = "◎ promoted to world fact".into();
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
        // Prefer the compiled world state (from `/compile`) — it reports what the
        // physics produced, not just what was declared — falling back to the
        // declaration summary until the author runs `/compile`.
        let world_state = self
            .compiled_summary
            .clone()
            .or_else(|| super::prompt::world_declaration_summary(&self.layout.root))
            .unwrap_or_default();
        let pinned_world = super::prompt::pinned_nodes_text(&self.store, &self.hierarchy, &self.world_pins);
        let world_facts = self
            .facts_tree
            .root
            .map(|bid| {
                super::prompt::retrieve_world_facts(&self.store, &self.hierarchy, &self.cfg, bid, &query)
            })
            .unwrap_or_default();
        let pinned_facts = super::prompt::pinned_nodes_text(&self.store, &self.hierarchy, &self.facts_pins);
        let warnings = self.plausibility_warnings_text();
        let system = super::prompt::build_system_prompt(
            self.world_name(),
            &self.cfg.language,
            &warnings,
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

    /// WB-P3 — recompute the deterministic plausibility score + warnings from
    /// `world.hjson`. `None` when there is no world yet. Called on load; WB-P4
    /// deltas will re-trigger it. No LLM — `run_fast` compiles + lints.
    /// The world as it currently stands: `world.hjson` on disk PLUS the pending
    /// (accepted-but-uncommitted) deltas. `None` when there is no parseable world
    /// yet. Shared by the plausibility score, `/compile`, and `/validate` so they
    /// all reason over the same in-progress world.
    pub(super) fn current_world_def(&self) -> Option<crate::world::types::WorldDefinition> {
        let mut value = std::fs::read_to_string(self.layout.root.join("world.hjson"))
            .ok()
            .and_then(|r| serde_hjson::from_str::<serde_json::Value>(&r).ok())
            .unwrap_or_else(|| serde_json::json!({}));
        for op in &self.pending_ops {
            op.apply(&mut value);
        }
        serde_json::from_value::<crate::world::types::WorldDefinition>(value).ok()
    }

    pub(super) fn refresh_plausibility(&mut self) {
        // Score the world.hjson on disk PLUS the pending (accepted-but-uncommitted)
        // deltas, so the score responds the moment a delta is accepted. Any world
        // change also invalidates the cached `/compile` summary + map grids +
        // the plakat raster (all stale once the world moves).
        self.compiled_summary = None;
        self.compiled_layers = None;
        self.map_raster = None;
        self.map_landmarks.clear();
        self.map_regions.clear();
        self.map_findings.clear();
        let def = self.current_world_def();
        self.ledger_snapshot = def.as_ref().and_then(|d| d.magic.clone());
        self.plausibility_prev = self.plausibility_score;
        match def {
            Some(def) => {
                let warnings = crate::world::plausibility::run_fast(&def);
                self.plausibility_score =
                    Some(crate::world::plausibility::compute_plausibility_score(&warnings));
                self.plausibility_warnings = warnings;
            }
            None => {
                self.plausibility_score = None;
                self.plausibility_warnings.clear();
            }
        }
    }

    /// The score change since the previous recompute, as a status chip.
    pub(super) fn plausibility_delta_chip(&self) -> String {
        match (self.plausibility_prev, self.plausibility_score) {
            (Some(p), Some(c)) if c > p => format!("▲{}", c - p),
            (Some(p), Some(c)) if c < p => format!("▼{}", p - c),
            _ => String::new(),
        }
    }

    /// The plausibility warnings formatted for the chat system prompt's WARNINGS
    /// section (top N; `run_fast` order already groups by layer).
    fn plausibility_warnings_text(&self) -> String {
        let mut s = String::new();
        for w in self.plausibility_warnings.iter().take(8) {
            s.push_str(&format!("! {}\n", w.text));
        }
        s
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
