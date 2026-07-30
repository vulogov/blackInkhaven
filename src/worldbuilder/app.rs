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

    // — World-fact research (WB-P7) ————————————————————————————————————
    /// The last `/research` query + its retrieved Facts passages, shown in the
    /// Research right-pane. `◎` marks passages already tagged `fact:world`.
    pub(super) research_query: Option<String>,
    pub(super) research_hits: Vec<crate::book_rag::RetrievedPassage>,

    // — Interview (WB-P8) ——————————————————————————————————————————————
    /// The active guided interview, if the author started one (`/interview` or
    /// `--interview`). While `Some`, plain Query input answers the current step.
    pub(super) interview: Option<super::interview::Interview>,

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
            pending_ops: Vec::new(),
            hjson_preview: None,
            plausibility_score: None,
            compiled_summary: None,
            compiled_layers: None,
            research_query: None,
            research_hits: Vec::new(),
            interview: None,
            plausibility_prev: None,
            plausibility_warnings: Vec::new(),
            session,
            should_quit: false,
            status: "worldbuilder — Tab cycles panes · Ctrl+Q quits".to_string(),
            show_hints: true,
            theme,
        };
        app.refresh_plausibility();
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
            Command::Unknown(msg) => self.status = msg,
        }
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
                let layers = crate::world::plausibility::compile_layers(&def);
                let summary = crate::world::plausibility::summarise_compiled(&def, &layers);
                self.compiled_summary = Some(summary.clone());
                self.compiled_layers = Some(layers);
                self.push_turn("/compile".into(), format!("Compiled world state —\n{summary}"));
                self.status =
                    "compiled — chat reasons over the simulated world · Ctrl+R → Map".into();
            }
            None => {
                self.status = "no world to compile — declare one first (interview or /set)".into();
            }
        }
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
            }
            Err(e) => self.status = format!("write failed: {e}"),
        }
    }

    /// `/undo` — drop the last pending op and rescore.
    fn undo_pending(&mut self) {
        if self.pending_ops.pop().is_some() {
            self.refresh_plausibility();
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
        // change also invalidates the cached `/compile` summary + map grids.
        self.compiled_summary = None;
        self.compiled_layers = None;
        let def = self.current_world_def();
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
