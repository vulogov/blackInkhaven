//! L-P0 — the Linguistic companion's application state + rendering.
//!
//! Two panes on the shared [`crate::tui_host`] loop: the Languages tree on the
//! left, and a right pane that toggles (Tab) between a node preview and a
//! grounded AI chat. The chat streams over the shared [`crate::ai::stream`]
//! machinery and is grounded, via `book_rag`, on the language sub-book that
//! contains the current selection — the same retrieval the Research companion
//! uses over the Facts book.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::Backend;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::ai::stream::{ChatTurn as AiTurn, StreamMsg, spawn_chat_stream};
use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::{Store, SYSTEM_TAG_LANGUAGES};
use crate::system_tree::SystemBookTree;
use crate::tui_host::TuiHost;

use super::session::{Session, SessionTurn};
use super::LinguisticInvocation;

/// AI-usage category for the cost dashboard.
const CATEGORY: &str = "linguistic";
/// How many characters of a node body to preview before eliding.
const PREVIEW_LIMIT: usize = 8_000;
/// How many retrieved passages ground a chat answer.
const RAG_TOP_N: usize = 6;
/// Bound on replayed prior turns (context, not the whole transcript).
const MAX_REPLAY_TURNS: usize = 10;
const MAX_REPLAY_CHARS: usize = 20_000;

/// One chat exchange in the Linguist's transcript.
struct Turn {
    prompt: String,
    response: String,
    streaming: bool,
    /// The language sub-book (title) that grounded this answer, if any.
    scope: Option<String>,
}

impl Turn {
    fn new(prompt: String, scope: Option<String>) -> Turn {
        Turn { prompt, response: String::new(), streaming: true, scope }
    }
}

/// Which view the right pane shows.
#[derive(Clone, Copy, PartialEq)]
enum RightPane {
    Chat,
    Preview,
    Metrics,
    Universals,
    Pairs,
}

impl RightPane {
    fn next(self) -> RightPane {
        match self {
            RightPane::Chat => RightPane::Preview,
            RightPane::Preview => RightPane::Metrics,
            RightPane::Metrics => RightPane::Universals,
            RightPane::Universals => RightPane::Pairs,
            RightPane::Pairs => RightPane::Chat,
        }
    }
}

pub struct LinguisticApp {
    layout: ProjectLayout,
    cfg: Config,
    store: Store,
    hierarchy: Hierarchy,
    /// Left-pane tree over the `Language` system book.
    tree: SystemBookTree,
    /// Cached preview text for the selected node (recomputed on move).
    preview: String,
    /// Cached metrics report for the current language (recomputed on `m`).
    metrics_text: String,
    /// Cached typology report for the current language (recomputed on `u`).
    universals_text: String,
    /// Cached minimal-pairs report for the current language (recomputed on `p`).
    pairs_text: String,
    right: RightPane,
    /// The chat transcript.
    chat: Vec<Turn>,
    chat_scroll: u16,
    /// The prompt being composed; `input_focused` gates keystrokes to it.
    input: String,
    input_focused: bool,
    /// In-flight response stream + the turn it feeds.
    stream_rx: Option<mpsc::UnboundedReceiver<StreamMsg>>,
    streaming_turn: Option<usize>,
    /// Transient status line (hints, errors).
    status: String,
    /// The persisted chat session (`--session`); transcript is saved on each
    /// completed turn and replayed on open.
    session: Session,
    should_quit: bool,
}

impl LinguisticApp {
    pub(crate) fn new(
        layout: ProjectLayout,
        cfg: Config,
        store: Store,
        hierarchy: Hierarchy,
        inv: LinguisticInvocation,
    ) -> Result<LinguisticApp> {
        let mut tree = SystemBookTree::new(&hierarchy, SYSTEM_TAG_LANGUAGES);
        if let Some(name) = inv.language.as_deref() {
            if let Some(id) = find_language_node(&hierarchy, name) {
                tree.reveal(&hierarchy, id);
            }
        }

        // Open (or create) the named session and replay its transcript.
        let now = chrono::Utc::now().to_rfc3339();
        let session_name = inv.session.as_deref().unwrap_or("default");
        let session = Session::open_or_create(&layout, session_name, now)?;
        let chat: Vec<Turn> = session
            .turns
            .iter()
            .map(|t| Turn {
                prompt: t.prompt.clone(),
                response: t.response.clone(),
                streaming: false,
                scope: t.scope.clone(),
            })
            .collect();

        let status = if tree.root.is_none() {
            "No Language system book yet — open the editor and create a language first."
                .to_string()
        } else if tree.is_empty() {
            "No languages yet. `inkhaven language init <name>` scaffolds one.".to_string()
        } else {
            "i ask · m metrics · u universals · p pairs · Tab cycle · jk move · q quit".to_string()
        };

        let mut app = LinguisticApp {
            layout,
            cfg,
            store,
            hierarchy,
            tree,
            preview: String::new(),
            metrics_text: String::new(),
            universals_text: String::new(),
            pairs_text: String::new(),
            right: RightPane::Chat,
            chat,
            chat_scroll: u16::MAX, // open scrolled to the latest turn
            input: String::new(),
            input_focused: false,
            stream_rx: None,
            streaming_turn: None,
            status,
            session,
            should_quit: false,
        };
        app.refresh_preview();
        Ok(app)
    }

    pub(crate) fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        crate::tui_host::run_loop(self, terminal)
    }

    fn refresh_preview(&mut self) {
        self.preview = match self.tree.selected() {
            Some(id) => self.node_preview(id),
            None => String::new(),
        };
    }

    fn node_preview(&self, id: Uuid) -> String {
        let Some(node) = self.hierarchy.get(id) else {
            return String::new();
        };
        match self.store.get_content(id) {
            Ok(Some(bytes)) if !bytes.is_empty() => {
                let body = String::from_utf8_lossy(&bytes);
                let mut s = body.chars().take(PREVIEW_LIMIT).collect::<String>();
                if body.chars().count() > PREVIEW_LIMIT {
                    s.push_str("\n\n… (truncated)");
                }
                s
            }
            _ => {
                let kids = self.hierarchy.children_of(Some(id));
                if kids.is_empty() {
                    format!("{}\n\n(empty)", node.title)
                } else {
                    let mut s = format!("{}\n\n", node.title);
                    for k in kids {
                        s.push_str(&format!("  • {}\n", k.title));
                    }
                    s
                }
            }
        }
    }

    /// The language sub-book (direct child of the Languages root) that contains
    /// the current selection — the grounding scope. `None` at/above that level.
    fn current_language_book(&self) -> Option<Uuid> {
        language_book_of(&self.hierarchy, self.tree.root?, self.tree.selected()?)
    }

    /// Compute (and cache) the L-P2 metrics report for the current language.
    fn compute_metrics(&mut self) {
        let Some(book_id) = self.current_language_book() else {
            self.metrics_text = "Select a language to see its metrics.".to_string();
            return;
        };
        let Some(book) = self.hierarchy.get(book_id).cloned() else {
            self.metrics_text = "Select a language to see its metrics.".to_string();
            return;
        };
        let phon = crate::cli::language::load_phonology(&self.store, &self.hierarchy, &book)
            .ok()
            .flatten()
            .unwrap_or_default();
        let entries = crate::cli::language::load_dictionary(&self.store, &self.hierarchy, &book)
            .unwrap_or_default();
        let m = crate::conlang::metrics::metrics(&phon, &entries);
        let nat = crate::conlang::naturalness::naturalness(&phon);
        self.metrics_text = format!("{}\n\n{}", format_metrics(&book.title, &m), format_naturalness(&nat));
    }

    /// Compute (and cache) the L-P3 typology report for the current language.
    fn compute_universals(&mut self) {
        let Some(book_id) = self.current_language_book() else {
            self.universals_text = "Select a language to see its typology.".to_string();
            return;
        };
        let Some(book) = self.hierarchy.get(book_id).cloned() else {
            self.universals_text = "Select a language to see its typology.".to_string();
            return;
        };
        let spec = crate::cli::language::load_grammar_spec(&self.store, &self.hierarchy, &book)
            .map(|(s, _)| s)
            .unwrap_or_default();
        let report = crate::conlang::universals::survey(&spec);
        self.universals_text = format_universals(&book.title, &report);
    }

    /// Compute (and cache) the Wave-2 minimal-pairs report for the current language.
    fn compute_pairs(&mut self) {
        let Some(book_id) = self.current_language_book() else {
            self.pairs_text = "Select a language to see its minimal pairs.".to_string();
            return;
        };
        let Some(book) = self.hierarchy.get(book_id).cloned() else {
            self.pairs_text = "Select a language to see its minimal pairs.".to_string();
            return;
        };
        let phon = crate::cli::language::load_phonology(&self.store, &self.hierarchy, &book)
            .ok()
            .flatten()
            .unwrap_or_default();
        let entries = crate::cli::language::load_dictionary(&self.store, &self.hierarchy, &book)
            .unwrap_or_default();
        let report = crate::conlang::pairs::minimal_pairs(&phon, &entries, 20);
        self.pairs_text = format_pairs(&book.title, &report);
    }

    /// Send `query` to the model, grounded on the current language sub-book.
    fn send_query(&mut self, query: String) {
        let query = query.trim().to_string();
        if query.is_empty() {
            return;
        }
        if self.stream_rx.is_some() {
            self.status = "a response is still streaming…".to_string();
            return;
        }

        let ai = match crate::ai::AiClient::from_config(&self.cfg.llm) {
            Ok(a) => a,
            Err(e) => {
                self.push_error_turn(query, format!("[no LLM provider: {e}]"));
                return;
            }
        };
        let (model, _env) = match ai.resolve_provider(&self.cfg.llm, None) {
            Ok(m) => m,
            Err(e) => {
                self.push_error_turn(query, format!("[provider error: {e}]"));
                return;
            }
        };

        // Grounding scope: the current language book (else the Languages root).
        let scope_id = self.current_language_book().or(self.tree.root);
        let scope_title = scope_id.and_then(|id| self.hierarchy.get(id)).map(|n| n.title.clone());
        let rag = scope_id.and_then(|bid| {
            crate::book_rag::retrieval::retrieve(
                &self.store,
                &self.hierarchy,
                &self.cfg.book_rag,
                bid,
                &query,
            )
            .ok()
            .filter(|p| !p.is_empty())
            .map(|mut p| {
                p.truncate(RAG_TOP_N);
                crate::book_rag::compose_context_prefix(&p)
            })
        });

        let system = self.system_prompt(scope_title.as_deref(), rag.as_deref());
        let history = self.replay_history();

        self.chat.push(Turn::new(query.clone(), scope_title));
        self.streaming_turn = Some(self.chat.len() - 1);
        self.chat_scroll = u16::MAX; // pin to bottom while streaming
        self.status = "asking the Inner Linguist…".to_string();

        let rx = spawn_chat_stream(
            ai.client.clone(),
            model.to_string(),
            Some(system),
            history,
            query,
            CATEGORY,
        );
        self.stream_rx = Some(rx);
    }

    /// `/trace <rule>` — preview a pending sound change over the current
    /// language and show the diff inline as a chat turn (no AI call).
    fn run_trace(&mut self, rule: String) {
        let prompt = format!("/trace {rule}");
        if rule.is_empty() {
            self.push_error_turn(prompt, "usage: /trace <rule>, e.g. /trace s > ʃ / _ i".into());
            return;
        }
        let Some(book) = self.current_language_book().and_then(|id| self.hierarchy.get(id).cloned())
        else {
            self.push_error_turn(prompt, "select a language first".into());
            return;
        };
        let phon = crate::cli::language::load_phonology(&self.store, &self.hierarchy, &book)
            .ok()
            .flatten()
            .unwrap_or_default();
        let entries = crate::cli::language::load_dictionary(&self.store, &self.hierarchy, &book)
            .unwrap_or_default();
        let report = crate::conlang::trace::trace_sound_change(&phon, &entries, &rule, 12);
        self.chat.push(Turn {
            prompt,
            response: format_trace(&report),
            streaming: false,
            scope: Some(book.title.clone()),
        });
        self.chat_scroll = u16::MAX;
        self.right = RightPane::Chat;
    }

    fn push_error_turn(&mut self, prompt: String, response: String) {
        self.chat.push(Turn {
            prompt,
            response,
            streaming: false,
            scope: None,
        });
        self.chat_scroll = u16::MAX;
    }

    /// The multilingual, grounded system prompt. Answers follow the project
    /// language; the RAG block (when present) is the sole source of fact.
    fn system_prompt(&self, scope: Option<&str>, rag: Option<&str>) -> String {
        let (plang, _) = crate::prose::resolve_prose_language(None, &self.cfg.language);
        let answer_lang = language_name(&plang);
        let subject = match scope {
            Some(name) => format!("the constructed language **{name}**"),
            None => "the project's constructed languages".to_string(),
        };
        let mut s = format!(
            "You are the Inner Linguist, a companion for developing, analysing and \
             documenting {subject}. Answer precisely and concretely, as a working \
             linguist would; use standard terminology (phonology, morphology, syntax, \
             typology). When the material below is relevant, ground your answer in it and \
             do not invent forms, glosses or rules that it does not support. If you are \
             unsure, say so. Write in {answer_lang}."
        );
        if let Some(ctx) = rag {
            if !ctx.trim().is_empty() {
                s.push_str("\n\n--- Material from the language book ---\n");
                s.push_str(ctx.trim());
            }
        }
        s
    }

    /// Prior genuine Q&A turns, most-recent-first within a budget, replayed
    /// oldest-first so the model has conversational context.
    fn replay_history(&self) -> Vec<AiTurn> {
        let mut picked: Vec<&Turn> = Vec::new();
        let mut budget = MAX_REPLAY_CHARS;
        for t in self.chat.iter().rev() {
            if t.streaming || t.response.trim().is_empty() || t.response.starts_with('[') {
                continue;
            }
            let cost = t.prompt.len() + t.response.len();
            if picked.len() >= MAX_REPLAY_TURNS || cost > budget {
                break;
            }
            budget -= cost;
            picked.push(t);
        }
        picked.reverse();
        picked
            .iter()
            .flat_map(|t| [AiTurn::User(t.prompt.clone()), AiTurn::Assistant(t.response.clone())])
            .collect()
    }
}

/// Find a language sub-book by (case-insensitive) title under the Languages book.
fn find_language_node(hierarchy: &Hierarchy, name: &str) -> Option<Uuid> {
    let root = hierarchy.iter().find(|n| {
        n.kind == crate::store::NodeKind::Book
            && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
    })?;
    hierarchy
        .children_of(Some(root.id))
        .into_iter()
        .find(|n| n.title.eq_ignore_ascii_case(name))
        .map(|n| n.id)
}

/// Walk up from `node` to the direct child of `root` (the language sub-book that
/// contains it) — the grounding scope. `None` if `node` is `root` itself, or not
/// under it.
fn language_book_of(hierarchy: &Hierarchy, root: Uuid, node: Uuid) -> Option<Uuid> {
    let mut cur = node;
    loop {
        let n = hierarchy.get(cur)?;
        match n.parent_id {
            Some(p) if p == root => return Some(cur),
            Some(p) => cur = p,
            None => return None,
        }
    }
}

/// Render the L-P2 metrics as a plain-text report for the right pane.
fn format_metrics(language: &str, m: &crate::conlang::metrics::LanguageMetrics) -> String {
    if m.analyzable_words == 0 {
        return format!(
            "{language}\n\nNo analyzable words yet.\n\nAdd dictionary entries that parse as \
             the language's phonemes, and define a phoneme inventory in the Phonology chapter."
        );
    }
    format!(
        "{language}\n\n\
         corpus     {} analyzable words, {} segments\n\n\
         entropy    {:.2} bits (max {:.2})\n\
         evenness   {:.0}%\n\
         perplexity {:.1}\n\n\
         zipf slope {:.2}  (≈ −1 is Zipfian)\n\
         zipf fit   R² {:.2}\n\n\
         syllables  {} attested / {} possible\n\
         saturation {:.0}%\n\n\
         moras/word {:.2}\n\
         heavy      {:.0}% of syllables",
        m.analyzable_words,
        m.total_segments,
        m.phoneme_entropy,
        m.phoneme_entropy_max,
        m.phoneme_evenness * 100.0,
        m.phoneme_perplexity,
        m.zipf_slope,
        m.zipf_r2,
        m.attested_syllables,
        m.possible_syllables,
        m.syllable_saturation * 100.0,
        m.mean_moras,
        m.heavy_ratio * 100.0,
    )
}

/// Render the L-P3 typology report as a plain-text report for the right pane.
fn format_universals(language: &str, r: &crate::conlang::universals::TypologyReport) -> String {
    use crate::conlang::universals::{Branch, Verdict};
    let branch = match r.branch {
        Branch::HeadInitial => "head-initial",
        Branch::HeadFinal => "head-final",
        Branch::Mixed => "mixed / disharmonic",
        Branch::Unknown => "unknown",
    };
    let mut s = format!(
        "{language}\n\n\
         order      {}\n\
         branching  {}\n",
        r.word_order.as_deref().unwrap_or("(unspecified)"),
        branch,
    );
    if let Some(mt) = &r.morphological_type {
        s.push_str(&format!("morphology {mt}\n"));
    }
    let measured = r.harmonic + r.disharmonic.len();
    if measured > 0 {
        s.push_str(&format!(
            "harmony    {}/{} aligned ({:.0}%)\n",
            r.harmonic,
            measured,
            r.harmony_score * 100.0
        ));
        if !r.disharmonic.is_empty() {
            s.push_str(&format!("disharmony {}\n", r.disharmonic.join(", ")));
        }
    }
    s.push_str("\nuniversals\n");
    for c in &r.checks {
        let g = match c.verdict {
            Verdict::Satisfied => "✓",
            Verdict::Violated => "✗",
            Verdict::NotApplicable => "·",
        };
        s.push_str(&format!("  {g} {}\n     {}\n", c.statement, c.detail));
    }
    s
}

/// Format a Consequence-Tracer report for the chat transcript.
fn format_trace(r: &crate::conlang::trace::TraceReport) -> String {
    if !r.rule_valid {
        return "could not parse the rule. Use `X > Y / A _ B`, e.g. `s > ʃ / _ i`.".to_string();
    }
    if r.analyzable_words == 0 {
        return "no analyzable words to trace.".to_string();
    }
    let mut s = format!("affects {} of {} words\n", r.affected, r.analyzable_words);
    for c in r.changes.iter().take(10) {
        s.push_str(&format!("  {} > {}\n", c.from, c.to));
    }
    if r.new_homophones.is_empty() {
        s.push_str("no new homophones — merges nothing.");
    } else {
        s.push_str(&format!(
            "⚠ {} new homophone set(s), {} words merged:\n",
            r.new_homophones.len(),
            r.merged_words
        ));
        for h in r.new_homophones.iter().take(6) {
            s.push_str(&format!("  {} ← {}\n", h.form, h.words.join(", ")));
        }
    }
    s
}

/// A compact inventory-naturalness summary, appended to the Phonology view.
fn format_naturalness(r: &crate::conlang::naturalness::NaturalnessReport) -> String {
    use crate::conlang::naturalness::SizeClass;
    if r.phoneme_count == 0 {
        return "naturalness\n  (no phoneme inventory)".to_string();
    }
    let size = match r.size_class {
        SizeClass::Small => "small",
        SizeClass::Typical => "typical",
        SizeClass::Large => "large",
    };
    let mut s = format!("naturalness  score {:.2} · {size} inventory\n", r.score);
    if !r.voicing_gaps.is_empty() {
        s.push_str(&format!("  voicing gap  {}\n", r.voicing_gaps.join(" ")));
    }
    if !r.missing_common.is_empty() {
        s.push_str(&format!("  missing      {} (near-universal)\n", r.missing_common.join(" ")));
    }
    if !r.unknown_segments.is_empty() {
        s.push_str(&format!("  outside      {}\n", r.unknown_segments.join(" ")));
    }
    if r.voicing_gaps.is_empty() && r.missing_common.is_empty() {
        s.push_str("  typologically ordinary\n");
    }
    s
}

/// Render the Wave-2 minimal-pairs report for the right pane.
fn format_pairs(language: &str, r: &crate::conlang::pairs::PairsReport) -> String {
    if r.analyzable_words == 0 {
        return format!("{language}\n\nNo analyzable words yet.");
    }
    let mut s = format!(
        "{language}\n\n{} minimal pair(s) · {} words\n",
        r.pair_count, r.analyzable_words
    );
    if !r.contrast_load.is_empty() {
        s.push_str("\nfunctional load\n");
        for (feat, count) in &r.contrast_load {
            s.push_str(&format!("  {feat:<12} {count}\n"));
        }
    }
    if !r.pairs.is_empty() {
        s.push_str("\nexamples\n");
        for p in &r.pairs {
            let c = if p.features.is_empty() {
                String::new()
            } else {
                format!("[{}]", p.features.join(", "))
            };
            s.push_str(&format!("  {} / {}  {}~{} {}\n", p.a, p.b, p.seg_a, p.seg_b, c));
        }
    }
    s
}

/// Project-language → its English name, for the "write in X" instruction.
fn language_name(lang: &crate::prose::ProseLanguage) -> &'static str {
    use crate::prose::ProseLanguage::*;
    match lang {
        En => "English",
        Ru => "Russian",
        De => "German",
        Fr => "French",
        Es => "Spanish",
        Other(_) => "English",
    }
}

impl TuiHost for LinguisticApp {
    fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Drain the in-flight response stream each frame.
    fn poll_async(&mut self) {
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
            let mut to_record = None;
            if let Some(turn) = self.chat.get_mut(idx) {
                turn.streaming = false;
                // Persist genuine answers; skip empty or error-placeholder turns.
                let ok = !turn.response.trim().is_empty() && !turn.response.starts_with('[');
                if ok {
                    to_record = Some(SessionTurn {
                        prompt: turn.prompt.clone(),
                        response: turn.response.clone(),
                        scope: turn.scope.clone(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    });
                }
            }
            self.status = if let Some(t) = to_record {
                match self.session.record(t, &self.layout) {
                    Ok(()) => "i ask · m metrics · u universals · p pairs · Tab cycle · jk move · q quit".to_string(),
                    Err(e) => format!("answer received, but the session didn't save: {e}"),
                }
            } else {
                "i ask · m metrics · u universals · p pairs · Tab cycle · jk move · q quit".to_string()
            };
        }
    }

    fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1)])
            .split(area);
        self.render_title(frame, chunks[0]);
        self.render_body(frame, chunks[1]);
        self.render_status(frame, chunks[2]);
    }

    fn on_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q'))
        {
            self.should_quit = true;
            return;
        }

        // Compose mode: keystrokes build the prompt.
        if self.input_focused {
            match key.code {
                KeyCode::Esc => self.input_focused = false,
                KeyCode::Enter => {
                    let q = std::mem::take(&mut self.input);
                    self.input_focused = false;
                    // `/trace <rule>` previews a pending sound change locally (no
                    // AI call); anything else is a grounded chat query.
                    if let Some(rule) = q.strip_prefix("/trace ") {
                        self.run_trace(rule.trim().to_string());
                    } else {
                        self.send_query(q);
                    }
                }
                KeyCode::Backspace => {
                    self.input.pop();
                }
                KeyCode::Char(c) => self.input.push(c),
                _ => {}
            }
            return;
        }

        // Navigation mode.
        let mut moved = false;
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('i') | KeyCode::Char('/') => {
                self.right = RightPane::Chat;
                self.input_focused = true;
            }
            KeyCode::Tab => {
                self.right = self.right.next();
                self.refresh_right_pane();
            }
            KeyCode::Char('m') => {
                self.right = RightPane::Metrics;
                self.compute_metrics();
            }
            KeyCode::Char('u') => {
                self.right = RightPane::Universals;
                self.compute_universals();
            }
            KeyCode::Char('p') => {
                self.right = RightPane::Pairs;
                self.compute_pairs();
            }
            KeyCode::PageDown => self.chat_scroll = self.chat_scroll.saturating_add(5),
            KeyCode::PageUp => self.chat_scroll = self.chat_scroll.saturating_sub(5),
            KeyCode::Down | KeyCode::Char('j') => {
                self.tree.move_down();
                moved = true;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.tree.move_up();
                moved = true;
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.tree.step_in(&self.hierarchy);
                moved = true;
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.tree.step_out(&self.hierarchy);
                moved = true;
            }
            KeyCode::Enter => {
                self.tree.toggle(&self.hierarchy);
                moved = true;
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.tree.to_top();
                moved = true;
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.tree.to_bottom();
                moved = true;
            }
            _ => {}
        }
        if moved {
            self.refresh_preview();
            self.refresh_right_pane();
        }
    }
}

impl LinguisticApp {
    /// Recompute the active right-pane's cached content (after a selection move
    /// or a Tab). Preview refreshes separately; Chat holds live state.
    fn refresh_right_pane(&mut self) {
        match self.right {
            RightPane::Metrics => self.compute_metrics(),
            RightPane::Universals => self.compute_universals(),
            RightPane::Pairs => self.compute_pairs(),
            RightPane::Preview | RightPane::Chat => {}
        }
    }
}

impl LinguisticApp {
    fn render_title(&self, frame: &mut Frame, area: Rect) {
        let scope = self
            .current_language_book()
            .and_then(|id| self.hierarchy.get(id))
            .map(|n| n.title.clone())
            .unwrap_or_else(|| "Languages".to_string());
        let line = Line::from(vec![
            Span::styled(" ⟐ Inner Linguist ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("· "),
            Span::styled(scope, Style::default().add_modifier(Modifier::DIM)),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    }

    fn render_body(&self, frame: &mut Frame, area: Rect) {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
            .split(area);
        self.render_tree(frame, cols[0]);
        match self.right {
            RightPane::Preview => self.render_preview(frame, cols[1]),
            RightPane::Chat => self.render_chat(frame, cols[1]),
            RightPane::Metrics => self.render_metrics(frame, cols[1]),
            RightPane::Universals => self.render_universals(frame, cols[1]),
            RightPane::Pairs => self.render_pairs(frame, cols[1]),
        }
    }

    fn render_pairs(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default().borders(Borders::ALL).title(" Minimal pairs ");
        frame.render_widget(
            Paragraph::new(self.pairs_text.as_str()).block(block).wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_metrics(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default().borders(Borders::ALL).title(" Phonology ");
        frame.render_widget(
            Paragraph::new(self.metrics_text.as_str()).block(block).wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_universals(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default().borders(Borders::ALL).title(" Universals ");
        frame.render_widget(
            Paragraph::new(self.universals_text.as_str()).block(block).wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_tree(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default().borders(Borders::ALL).title(" Tree ");
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let mut lines: Vec<Line> = Vec::new();
        for (i, row) in self.tree.rows().iter().enumerate() {
            let fold = if row.has_children {
                if row.expanded { "▾ " } else { "▸ " }
            } else {
                "  "
            };
            let title = self
                .hierarchy
                .get(row.id)
                .map(|n| n.title.clone())
                .unwrap_or_else(|| "?".to_string());
            let indent = "  ".repeat(row.depth);
            let text = format!("{indent}{fold}{title}");
            let style = if i == self.tree.cursor {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(text, style)));
        }
        if lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "(no languages)",
                Style::default().add_modifier(Modifier::DIM),
            )));
        }
        frame.render_widget(Paragraph::new(lines).scroll((self.tree.scroll as u16, 0)), inner);
    }

    fn render_preview(&self, frame: &mut Frame, area: Rect) {
        let title = self
            .tree
            .selected()
            .and_then(|id| self.hierarchy.get(id))
            .map(|n| format!(" {} ", n.title))
            .unwrap_or_else(|| " Preview ".to_string());
        let block = Block::default().borders(Borders::ALL).title(title);
        let para = Paragraph::new(self.preview.as_str()).block(block).wrap(Wrap { trim: false });
        frame.render_widget(para, area);
    }

    fn render_chat(&self, frame: &mut Frame, area: Rect) {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(3)])
            .split(area);

        // Transcript.
        let block = Block::default().borders(Borders::ALL).title(" Chat ");
        let mut lines: Vec<Line> = Vec::new();
        if self.chat.is_empty() {
            lines.push(Line::from(Span::styled(
                "Press i to ask the Inner Linguist, or /trace <rule> to preview a sound change.",
                Style::default().add_modifier(Modifier::DIM),
            )));
        }
        for turn in &self.chat {
            let mut header = format!("❯ {}", turn.prompt);
            if let Some(scope) = &turn.scope {
                header.push_str(&format!("  ({scope})"));
            }
            lines.push(Line::from(Span::styled(
                header,
                Style::default().add_modifier(Modifier::BOLD),
            )));
            let body = if turn.response.is_empty() && turn.streaming {
                "…".to_string()
            } else {
                turn.response.clone()
            };
            for l in body.lines() {
                lines.push(Line::from(l.to_string()));
            }
            lines.push(Line::from(""));
        }
        // Clamp scroll so "pin to bottom" (u16::MAX) lands on the last screenful.
        let inner_h = rows[0].height.saturating_sub(2).max(1);
        let total = lines.len() as u16;
        let max_scroll = total.saturating_sub(inner_h);
        let scroll = self.chat_scroll.min(max_scroll);
        frame.render_widget(
            Paragraph::new(lines).block(block).wrap(Wrap { trim: false }).scroll((scroll, 0)),
            rows[0],
        );

        // Input box.
        let (title, content, style) = if self.input_focused {
            (" Ask ", format!("{}▏", self.input), Style::default())
        } else {
            (" Ask ", "press i to compose".to_string(), Style::default().add_modifier(Modifier::DIM))
        };
        frame.render_widget(
            Paragraph::new(Span::styled(content, style))
                .block(Block::default().borders(Borders::ALL).title(title)),
            rows[1],
        );
    }

    fn render_status(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!(" {}", self.status),
                Style::default().add_modifier(Modifier::DIM),
            )),
            area,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::node::Node;

    fn node(id: Uuid, kind: &str, title: &str, parent: Option<Uuid>, lang_root: bool) -> Node {
        let mut raw = serde_json::json!({
            "id": id,
            "kind": kind,
            "title": title,
            "slug": title.to_lowercase(),
            "path": [],
            "parent_id": parent,
            "order": 1,
            "file": null,
            "modified_at": "2026-01-01T00:00:00Z",
        });
        if lang_root {
            raw["system_tag"] = serde_json::json!(SYSTEM_TAG_LANGUAGES);
        }
        serde_json::from_value(raw).expect("test node deserialises")
    }

    #[test]
    fn find_language_node_matches_case_insensitively() {
        let root = Uuid::now_v7();
        let quenya = Uuid::now_v7();
        let sindarin = Uuid::now_v7();
        let h = Hierarchy::from_nodes_for_test(vec![
            node(root, "book", "Languages", None, true),
            node(quenya, "book", "Quenya", Some(root), false),
            node(sindarin, "book", "Sindarin", Some(root), false),
        ]);
        assert_eq!(find_language_node(&h, "quenya"), Some(quenya));
        assert_eq!(find_language_node(&h, "SINDARIN"), Some(sindarin));
        assert_eq!(find_language_node(&h, "Adûnaic"), None);
    }

    #[test]
    fn find_language_node_is_none_without_a_languages_book() {
        let ch = Uuid::now_v7();
        let h = Hierarchy::from_nodes_for_test(vec![node(ch, "book", "Draft", None, false)]);
        assert_eq!(find_language_node(&h, "Quenya"), None);
    }

    #[test]
    fn language_book_of_resolves_the_grounding_scope() {
        let root = Uuid::now_v7();
        let quenya = Uuid::now_v7();
        let phon = Uuid::now_v7();
        let para = Uuid::now_v7();
        let h = Hierarchy::from_nodes_for_test(vec![
            node(root, "book", "Languages", None, true),
            node(quenya, "book", "Quenya", Some(root), false),
            node(phon, "chapter", "Phonology", Some(quenya), false),
            node(para, "paragraph", "vowels", Some(phon), false),
        ]);
        // A deep node resolves up to its language book.
        assert_eq!(language_book_of(&h, root, para), Some(quenya));
        assert_eq!(language_book_of(&h, root, phon), Some(quenya));
        // The language book itself resolves to itself.
        assert_eq!(language_book_of(&h, root, quenya), Some(quenya));
        // The root has no containing language book.
        assert_eq!(language_book_of(&h, root, root), None);
    }

    #[test]
    fn language_name_covers_the_project_languages() {
        use crate::prose::ProseLanguage::*;
        assert_eq!(language_name(&Ru), "Russian");
        assert_eq!(language_name(&Fr), "French");
        assert_eq!(language_name(&Other("xx".into())), "English");
    }
}
