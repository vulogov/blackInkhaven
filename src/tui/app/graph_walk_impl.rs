//! GRAPHMIND GM-P8 — the in-editor graph walk: GM-P5's multi-turn traversal,
//! streamed and non-blocking, driven across render frames instead of a blocking
//! loop. All graph access stays on the UI thread (no background thread, no second
//! `Store` handle — the 1.2.15 concurrency guarantee); the walk rides the same
//! `spawn_chat_stream` → `Inference` → `pump_inference` machinery a normal chat
//! uses, one exploration turn per streamed inference.
//!
//! Lifecycle: `start_graph_walk` (from the graph hub) kicks turn 1;
//! `pump_inference`'s finalize point calls `advance_graph_walk` when a turn
//! completes, which parses the reply, runs the graph query on the UI thread,
//! and either kicks the next exploration turn or the terminal **prose synthesis**
//! turn. The synthesis turn commits through the ordinary chat-history path (its
//! streamed prose *is* the answer). `Esc` cancels the whole session.

use super::super::focus::Focus;
use super::super::inference::{Inference, InferenceStatus};
use crate::ai::stream::spawn_chat_stream;
use crate::graph_rag::ask::{AskSession, AskStep};
use crate::graph_rag::oracle::StoreOracle;

/// An in-flight graph walk: the resumable session plus the bookkeeping the TUI
/// driver needs. Held in `App::graph_walk` while a walk runs.
pub(in crate::tui::app) struct GraphWalk {
    /// The resumable traversal core (handles, observations, transcript, step).
    session: AskSession,
    /// The original question — paired with the streamed answer when the walk
    /// commits its single chat turn.
    question: String,
    /// True once the terminal synthesis turn has been issued: its completion
    /// commits normally rather than advancing the loop.
    synthesizing: bool,
}

impl GraphWalk {
    /// The live exploration transcript (one step line per action taken) — what
    /// the AI pane renders while the walk unfolds.
    pub(in crate::tui::app) fn transcript(&self) -> &[String] {
        self.session.transcript()
    }

    /// Whether the terminal answer turn is streaming (vs. still exploring).
    pub(in crate::tui::app) fn synthesizing(&self) -> bool {
        self.synthesizing
    }
}

impl super::App {
    /// Whether a graph walk is currently running.
    pub(in crate::tui::app) fn graph_walk_active(&self) -> bool {
        self.graph_walk.is_some()
    }

    /// The active walk (for the renderer).
    pub(in crate::tui::app) fn graph_walk(&self) -> Option<&GraphWalk> {
        self.graph_walk.as_ref()
    }

    /// GM-P8 — start a graph walk from the current AI-prompt text (the graph hub
    /// `Ctrl+B z → w`). Takes the prompt as the question, kicks the first
    /// exploration turn, and moves focus to the AI pane so the walk is visible.
    pub(in crate::tui::app) fn start_graph_walk(&mut self) {
        let question = self.ai_input.as_str().trim().to_string();
        if question.is_empty() {
            self.status = "graph walk: type a question in the AI prompt first".into();
            return;
        }
        if self.graph_walk.is_some() {
            self.status = "graph walk: one already running (Esc to stop it)".into();
            return;
        }
        if self.ai.resolve_provider(&self.cfg.llm, None).is_err() {
            self.status = "graph walk: no LLM provider configured".into();
            return;
        }
        let max_steps = self.cfg.graph.ask_max_steps.max(1);
        let width = self.cfg.graph.ask_search_width.max(1);
        let session = AskSession::new(question.clone(), max_steps, width);
        let prompt = session.next_prompt();
        self.graph_walk = Some(GraphWalk { session, question, synthesizing: false });
        self.ai_input.clear();
        self.change_focus(Focus::Ai);
        self.status = format!("graph walk · turn 1/{max_steps} · Esc to stop");
        self.kick_graph_walk_turn(prompt, self.graph_walk_explore_system());
    }

    /// GM-P8 — called from `pump_inference` when a walk turn completes (and it is
    /// an *exploration* turn, not the terminal synthesis). Parses the reply, runs
    /// the graph query on the UI thread, and kicks the next turn: another
    /// exploration turn while the model keeps exploring, or the terminal prose
    /// synthesis once it answers / the step budget is spent.
    pub(in crate::tui::app) fn advance_graph_walk(&mut self) {
        let reply = self
            .inference
            .as_ref()
            .map(|i| i.response.clone())
            .unwrap_or_default();
        // Take the walk out so the `&mut session` and the `&self` oracle borrow
        // don't overlap; put it back (unless the walk ends here).
        let Some(mut walk) = self.graph_walk.take() else {
            return;
        };
        let step = {
            let oracle = StoreOracle { store: &self.store, hierarchy: &self.hierarchy };
            walk.session.on_reply(&reply, &oracle)
        };
        match step {
            AskStep::Continue => {
                let (k, n) = walk.session.turn();
                let prompt = walk.session.next_prompt();
                self.status = format!("graph walk · turn {k}/{n} · Esc to stop");
                self.graph_walk = Some(walk);
                self.kick_graph_walk_turn(prompt, self.graph_walk_explore_system());
            }
            // Either the model chose to answer or the budget is spent: issue ONE
            // terminal turn that streams a grounded PROSE answer. Setting
            // `pending_chat_user_msg` means the ordinary finalize path (once this
            // turn completes) commits `(question → streamed answer)` as one chat
            // turn — see the `synthesizing` branch in `pump_inference`.
            AskStep::Answer(_) | AskStep::Synthesize => {
                let prompt = walk.session.synthesize_prompt();
                walk.synthesizing = true;
                self.pending_chat_user_msg = Some(walk.question.clone());
                self.status = "graph walk · writing the grounded answer · Esc to stop".into();
                let system = self.graph_walk_answer_system();
                self.graph_walk = Some(walk);
                self.kick_graph_walk_turn(prompt, system);
            }
        }
    }

    /// GM-P8 — abort the whole walk (any `Esc` while one is running): drop the
    /// session and the in-flight turn, clear any half-set commit state.
    pub(in crate::tui::app) fn cancel_graph_walk(&mut self) {
        self.graph_walk = None;
        self.inference = None;
        self.pending_chat_user_msg = None;
        self.status = "graph walk cancelled".into();
    }

    /// Spawn one streamed turn of the walk: `system` is the turn's contract
    /// (JSON-action for exploration, prose-grounding for synthesis); `user_prompt`
    /// is the session-built turn prompt. History is empty — the prompt carries
    /// all state. Tagged `graph_rag` for the cost dashboard.
    fn kick_graph_walk_turn(&mut self, user_prompt: String, system: String) {
        let (model, _env) = match self.ai.resolve_provider(&self.cfg.llm, None) {
            Ok(pair) => pair,
            Err(e) => {
                self.status = format!("graph walk: {e}");
                self.graph_walk = None;
                return;
            }
        };
        let rx = spawn_chat_stream(
            self.ai.client.clone(),
            model.to_string(),
            Some(system),
            Vec::new(),
            user_prompt,
            "graph_rag",
        );
        self.inference = Some(Inference {
            provider: self.ai.default_provider.clone(),
            model: model.to_string(),
            response: String::new(),
            status: InferenceStatus::Streaming,
            rx,
            started_at: std::time::Instant::now(),
        });
    }

    /// The exploration-turn system prompt (the JSON tool contract).
    fn graph_walk_explore_system(&self) -> String {
        let iso = crate::ai::prompts::iso_from_long(&self.cfg.language);
        crate::graph_rag::ask::system_prompt(iso).to_string()
    }

    /// The synthesis-turn system prompt (the P4 prose grounding contract — cite
    /// labels, be honest about the graph's limits), so the final answer streams
    /// as prose instead of a JSON action.
    fn graph_walk_answer_system(&self) -> String {
        let iso = crate::ai::prompts::iso_from_long(&self.cfg.language);
        crate::graph_rag::system_prompt(iso).to_string()
    }
}
