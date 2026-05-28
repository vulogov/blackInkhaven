//! 1.2.11+ — prompts-editor event loop + render.
//!
//! Phase 1: read-only walk-through.  CLI plumbing,
//! the four-pane shell, list navigation, show-on-
//! focus editor display, help pane.  No mutation,
//! no save, no AI send.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Local;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent,
    KeyEventKind, KeyModifiers,
};
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
use tokio::sync::mpsc::UnboundedReceiver;
use tui_textarea::TextArea;

use crate::ai::AiClient;
use crate::ai::prompts::{Prompt, PromptLibrary};
use crate::ai::stream::{StreamMsg, spawn_chat_stream};
use crate::config::{Config, DEFAULT_PROMPTS};
use crate::prompts_tui::backup::{self, BackupEntry};
use crate::tui::input::TextInput;

/// Two-step entry: install panic hook + raw-mode +
/// alt-screen, run the event loop, restore the
/// terminal in every exit path.
pub fn run(project: &Path) -> Result<()> {
    let prompts_path = project.join("prompts.hjson");
    let app = App::load(project.to_path_buf(), &prompts_path)?;

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
}

/// Three editable panes the user can Tab between.
/// AI response is display-only and not in the Tab
/// cycle (scroll via Ctrl+↑ / Ctrl+↓ from any focus
/// — wiring lands in Phase 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    List,
    Editor,
    AiPrompt,
}

impl Focus {
    fn next(self) -> Self {
        match self {
            Self::List => Self::Editor,
            Self::Editor => Self::AiPrompt,
            Self::AiPrompt => Self::List,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::List => Self::AiPrompt,
            Self::Editor => Self::List,
            Self::AiPrompt => Self::Editor,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Editor => "editor",
            Self::AiPrompt => "ai prompt",
        }
    }
}

enum Modal {
    None,
    Help { body: String },
    SaveConfirm { summary: SaveSummary },
    Saved { message: String },
    AddPrompt { buffer: String, error: Option<String> },
    DeletePromptConfirm { name: String },
    DiscardConfirm { unsaved: usize },
    /// Phase 4 — Ctrl+R rollback picker.
    Rollback { entries: Vec<BackupEntry>, cursor: usize },
    /// Confirm before deleting a backup file.
    RollbackDelete { entry: BackupEntry },
    /// Preview a backup's contents.
    RollbackPreview { entry: BackupEntry, body: String, scroll: usize },
}

#[derive(Debug, Clone)]
struct SaveSummary {
    added: Vec<String>,
    modified: Vec<String>,
    removed: Vec<String>,
}

impl SaveSummary {
    fn total(&self) -> usize {
        self.added.len() + self.modified.len() + self.removed.len()
    }
}

struct App {
    project_root: PathBuf,
    prompts_path: PathBuf,
    library: PromptLibrary,
    cursor: usize,
    list_scroll: usize,
    /// `tui-textarea` view of the currently-loaded
    /// prompt's body.  Switching prompts in the
    /// list pane stashes this back into the library
    /// before swapping in the new prompt's body.
    editor: TextArea<'static>,
    /// Names of prompts whose `template` differs
    /// from what's on disk (or in the
    /// embedded-defaults baseline).
    dirty: HashSet<String>,
    /// Names of prompts newly created this session
    /// (not yet written to disk).  These also live
    /// in `library.prompts` so the editor pane sees
    /// them, but they need a special "add new
    /// entry" event in the save summary.
    added: HashSet<String>,
    /// Names of prompts staged for deletion.  Still
    /// in `library.prompts` (so the list pane
    /// renders them struck-through) until save
    /// filters them out.
    removed: HashSet<String>,
    /// Per-prompt baseline — the on-disk
    /// `template` text the dirty-set is compared
    /// against.  Refreshed after each save.
    baseline: std::collections::HashMap<String, String>,
    first_launch: bool,
    /// `true` when the on-disk `prompts.hjson` was
    /// missing at load and the embedded defaults
    /// were materialised in-memory.  Status bar
    /// flags this so the user knows what's loaded;
    /// the FIRST Ctrl+S writes the file from those
    /// in-memory defaults.
    loaded_from_defaults: bool,
    focus: Focus,
    modal: Modal,
    status: String,
    /// Phase 3 — AI client + model resolved at load
    /// time.  `None` when the project's
    /// `inkhaven.hjson` isn't readable / the default
    /// provider isn't configured.  Send path stays
    /// inert in that case; everything else works.
    ai: Option<AiRuntime>,
    /// Phase 3 — AI prompt input (single-line).
    ai_input: TextInput,
    /// Phase 3 — in-session prompt history; Up/Down
    /// walks while AI prompt pane is focused.
    ai_history: Vec<String>,
    /// `None` = editing fresh; `Some(i)` = walking
    /// history.  Cleared on any text edit.
    ai_history_cursor: Option<usize>,
    /// Phase 3 — most recent (or in-flight) send.
    /// Single-shot per send per Q5; replaced wholesale
    /// every Enter.
    last_send: Option<Send>,
    /// Live streaming inference handle (token receiver
    /// + start time).  Set on send, cleared when the
    /// stream finishes.
    inference: Option<Inference>,
    /// 1.2.11+ — meta-prefix state.  `true` after the
    /// user presses Ctrl+B; the next keystroke is
    /// interpreted as a chord suffix.  Reset after
    /// processing or on Esc.  Mirrors the main TUI's
    /// Ctrl+B chord scheme so terminal-level Ctrl
    /// intercepts (Ctrl+G as ASCII BEL is the typical
    /// offender) can't eat our chords.
    meta_pending: bool,
}

#[derive(Clone)]
struct AiRuntime {
    client: AiClient,
    model: String,
    provider: String,
}

#[derive(Debug)]
pub(super) struct Send {
    /// Name of the prompt being analysed at send
    /// time.
    pub prompt_name: Option<String>,
    /// Snapshot of the editor body — the template
    /// being put under review.  Placeholders like
    /// `{{selection}}` are NOT substituted; the
    /// LLM sees them verbatim so it can comment on
    /// their use.
    pub template_under_review: String,
    /// The analysis instruction — either what the
    /// user typed into the AI prompt input or the
    /// embedded `DEFAULT_ANALYSIS_REQUEST` when
    /// the input was empty.
    pub analysis_request: String,
    pub response: String,
    pub started_at: Instant,
    pub duration: Option<Duration>,
    pub failed: bool,
}

pub(super) struct Inference {
    rx: UnboundedReceiver<StreamMsg>,
    /// Reserved — the Send struct currently owns
    /// the canonical timer; this field is kept for
    /// future "abandoned stream older than X
    /// seconds" cleanup logic.
    #[allow(dead_code)]
    started_at: Instant,
}

impl App {
    fn load(project_root: PathBuf, prompts_path: &Path) -> Result<Self> {
        let (library, loaded_from_defaults) = if prompts_path.exists() {
            let lib = PromptLibrary::load(prompts_path)
                .with_context(|| format!("load {}", prompts_path.display()))?;
            (lib, false)
        } else {
            // Q4 — auto-populate from inkhaven's
            // embedded defaults when the file is
            // missing.  In-memory; first Ctrl+S
            // commits.
            let lib: PromptLibrary = serde_hjson::from_str(DEFAULT_PROMPTS)
                .context("parse embedded DEFAULT_PROMPTS")?;
            (lib, true)
        };

        // Baseline = current library snapshot.  When
        // loaded_from_defaults is true the baseline
        // marks every prompt as "modified vs disk"
        // so Ctrl+S writes the whole library.
        let baseline = build_baseline(&library);

        let mut added: HashSet<String> = HashSet::new();
        if loaded_from_defaults {
            // Every default counts as "added vs
            // disk" — the file isn't on disk yet.
            for p in &library.prompts {
                added.insert(p.name.clone());
            }
        }

        let editor = build_editor(library.prompts.first());

        let status = if loaded_from_defaults {
            format!(
                "{} missing · {} embedded defaults staged · Ctrl+S to write",
                prompts_path.display(),
                library.prompts.len(),
            )
        } else {
            format!(
                "{} loaded · {} prompts",
                prompts_path.display(),
                library.prompts.len(),
            )
        };

        // Phase 3 — build the AI runtime if the
        // project's inkhaven.hjson is readable AND
        // its llm.default provider is configured.
        // Any failure flags the runtime as disabled;
        // the rest of the TUI still works.
        let ai = build_ai_runtime(&project_root);
        let status = match (&ai, status.as_str()) {
            (Some(rt), s) => {
                if loaded_from_defaults {
                    format!("{s} · LLM: {} · {}", rt.provider, rt.model)
                } else {
                    format!("{s} · LLM: {} · {}", rt.provider, rt.model)
                }
            }
            (None, s) => format!(
                "{s} · LLM: (not configured — send is inert)",
            ),
        };

        Ok(Self {
            project_root,
            prompts_path: prompts_path.to_path_buf(),
            library,
            cursor: 0,
            list_scroll: 0,
            editor,
            dirty: HashSet::new(),
            added,
            removed: HashSet::new(),
            baseline,
            first_launch: true,
            loaded_from_defaults,
            focus: Focus::List,
            modal: Modal::None,
            status,
            ai,
            ai_input: TextInput::new(),
            ai_history: Vec::new(),
            ai_history_cursor: None,
            last_send: None,
            inference: None,
            meta_pending: false,
        })
    }

    fn current_prompt(&self) -> Option<&Prompt> {
        self.library.prompts.get(self.cursor)
    }

    /// Stash the editor's current text back into the
    /// library entry pointed at by `cursor`.  Called
    /// before switching prompts so an in-progress
    /// edit isn't lost.  Updates the dirty set when
    /// the new text differs from the baseline.
    fn stash_editor(&mut self) {
        let Some(prompt) = self.library.prompts.get_mut(self.cursor) else {
            return;
        };
        let body = self.editor.lines().join("\n");
        if body == prompt.template {
            // No change since last load — nothing to
            // stash.
            return;
        }
        prompt.template = body;
        let baseline = self.baseline.get(&prompt.name).cloned();
        let name = prompt.name.clone();
        // Need to drop the mut borrow before touching
        // self.dirty.
        let _ = prompt;
        match baseline {
            Some(base) if base == self.library.prompts[self.cursor].template => {
                // Edited back to baseline — clear
                // dirty.
                self.dirty.remove(&name);
            }
            _ => {
                self.dirty.insert(name);
            }
        }
    }

    /// Load the library entry at `cursor` into the
    /// editor view.  Used after cursor movement +
    /// after add/delete/save flows.
    fn reload_editor(&mut self) {
        self.editor = build_editor(self.library.prompts.get(self.cursor));
    }

    fn has_unsaved(&self) -> bool {
        !self.dirty.is_empty()
            || !self.added.is_empty()
            || !self.removed.is_empty()
            || self.loaded_from_defaults
    }

    fn unsaved_count(&self) -> usize {
        self.dirty.len() + self.added.len() + self.removed.len()
    }

    fn save_summary(&self) -> SaveSummary {
        let mut added: Vec<String> = self.added.iter().cloned().collect();
        added.sort();
        let mut removed: Vec<String> = self.removed.iter().cloned().collect();
        removed.sort();
        // Modified = dirty entries that aren't
        // newly-added (otherwise they'd appear in
        // both buckets and confuse the summary).
        let mut modified: Vec<String> = self
            .dirty
            .iter()
            .filter(|name| !self.added.contains(*name))
            .cloned()
            .collect();
        modified.sort();
        SaveSummary {
            added,
            modified,
            removed,
        }
    }
}

/// Best-effort: load `<project>/inkhaven.hjson`,
/// resolve the default LLM provider, build an
/// AiClient.  Any failure → return `None` so the
/// prompts editor still launches with a disabled
/// send path.  This keeps the TUI useful for users
/// who haven't set up AI yet.
fn build_ai_runtime(project_root: &Path) -> Option<AiRuntime> {
    let cfg_path = project_root.join("inkhaven.hjson");
    let cfg = if cfg_path.exists() {
        Config::load(&cfg_path).ok()?
    } else {
        // No inkhaven.hjson — use defaults.  Default
        // provider is whatever Config::default()
        // declares; without an API key in the
        // environment the resolve will fail
        // gracefully below.
        Config::default()
    };
    let client = AiClient::from_config(&cfg.llm).ok()?;
    let (model, _env) = client.resolve_provider(&cfg.llm, None).ok()?;
    let model = model.to_string();
    let provider = client.default_provider.clone();
    Some(AiRuntime {
        client,
        model,
        provider,
    })
}

/// 1.2.11+ — system prompt that frames the LLM as
/// a prompt-engineering reviewer.  The user pane
/// sends a template + an analysis request; the
/// LLM does NOT execute the template — it reviews
/// it as prompt-engineering work.
const ANALYSIS_SYSTEM_PROMPT: &str = "\
You are a prompt-engineering reviewer.  The user is editing prompt \
templates that another LLM will execute later, and they're asking \
you to analyze, critique, or improve their drafts.

Templates may contain placeholders like `{{selection}}` and \
`{{context}}` — these are substituted at runtime by the inkhaven \
editor (with the user's selected prose and surrounding hierarchical \
context, respectively).  Do NOT try to execute the template yourself; \
review it as a piece of prompt-engineering work.

Be specific.  Quote phrases from the template when you critique them.  \
Suggest concrete improvements.  When the user asks a yes/no question \
about the template, answer it directly first, then justify.";

/// Default analysis request used when the AI prompt
/// input is empty — so the user can press Enter on
/// any prompt and get a baseline critique.
const DEFAULT_ANALYSIS_REQUEST: &str = "\
Critique this prompt template.  Identify its strengths and weaknesses, \
comment on whether the placeholders are used effectively, and suggest \
one or two concrete improvements.";

/// Compose the user message the LLM receives.  The
/// template body is presented as a review target
/// inside fenced markers; the analysis request
/// follows.  Placeholders are passed through
/// untouched so the reviewer can comment on them.
fn build_analysis_request(template_body: &str, instruction: &str) -> String {
    let instruction = if instruction.trim().is_empty() {
        DEFAULT_ANALYSIS_REQUEST
    } else {
        instruction.trim()
    };
    format!(
        "--- PROMPT TEMPLATE UNDER REVIEW ---\n\
         {template_body}\n\
         --- END TEMPLATE ---\n\
         \n\
         Analysis request:\n\
         {instruction}",
    )
}

fn build_baseline(library: &PromptLibrary) -> std::collections::HashMap<String, String> {
    library
        .prompts
        .iter()
        .map(|p| (p.name.clone(), p.template.clone()))
        .collect()
}

fn build_editor(prompt: Option<&Prompt>) -> TextArea<'static> {
    let lines: Vec<String> = match prompt {
        Some(p) if !p.template.is_empty() => {
            p.template.lines().map(|s| s.to_string()).collect()
        }
        Some(_) => vec![String::new()],
        None => vec![String::new()],
    };
    TextArea::new(lines)
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut app: App,
) -> Result<()> {
    loop {
        // Drain any AI tokens that arrived since the
        // last frame BEFORE we draw so the response
        // pane is always up-to-date.
        pump_inference(&mut app);
        terminal.draw(|f| render(f, &mut app))?;
        // Shorter poll while streaming so the spinner
        // animates and tokens flow into the pane
        // promptly.
        let poll_ms = if app.inference.is_some() { 80 } else { 250 };
        if event::poll(Duration::from_millis(poll_ms))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if handle_key(&mut app, key)? {
                        return Ok(());
                    }
                }
            }
        }
    }
}

/// Returns `Ok(true)` when the loop should exit.
fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    // Meta-prefix dispatch first.  After the user
    // pressed `Ctrl+B`, the next keystroke is a
    // chord suffix.  Most chords here are
    // duplicates of the bare-Ctrl chords above so
    // users can pick whichever their terminal
    // doesn't eat — Ctrl+G is the canonical
    // offender (ASCII BEL).
    if app.meta_pending {
        app.meta_pending = false;
        match key.code {
            KeyCode::Char('g') | KeyCode::Char('G') => {
                insert_ai_response_into_editor(app);
                // Focus-agnostic — the user can fire
                // this from any pane and the
                // response still lands in the
                // editor.  If they were focused on
                // the AI prompt or list pane,
                // they'll usually want to switch to
                // the editor right after.
                app.focus = Focus::Editor;
            }
            KeyCode::Esc => {
                app.status = "meta: cancelled".into();
            }
            other => {
                app.status = format!(
                    "meta: unknown chord (got {other:?}) — Ctrl+B G to insert AI response",
                );
            }
        }
        return Ok(false);
    }
    // Ctrl+B alone (no other modifier) starts the
    // meta prefix.  Only fires when no modal is
    // open — otherwise the modal handlers see the
    // key first.
    if matches!(app.modal, Modal::None)
        && key.code == KeyCode::Char('b')
        && key.modifiers == KeyModifiers::CONTROL
    {
        app.meta_pending = true;
        app.status = "META — next key is a chord suffix · Esc cancels".into();
        return Ok(false);
    }

    // Modal-first dispatch.
    if matches!(
        app.modal,
        Modal::Help { .. } | Modal::Saved { .. }
    ) {
        app.modal = Modal::None;
        app.first_launch = false;
        return Ok(false);
    }
    if matches!(app.modal, Modal::SaveConfirm { .. }) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                let outcome = perform_save(app);
                app.modal = match outcome {
                    Ok(msg) => Modal::Saved { message: msg },
                    Err(e) => Modal::Saved {
                        message: format!("save FAILED: {e:#}"),
                    },
                };
            }
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                app.modal = Modal::None;
                app.status = "save cancelled".into();
            }
            _ => {}
        }
        return Ok(false);
    }
    if let Modal::AddPrompt { buffer, error } = &mut app.modal {
        match key.code {
            KeyCode::Esc => {
                app.modal = Modal::None;
                app.status = "add prompt: cancelled".into();
            }
            KeyCode::Enter => {
                let name = buffer.trim().to_string();
                if name.is_empty() {
                    *error = Some("name is required".into());
                } else if !is_valid_prompt_name(&name) {
                    *error = Some(
                        "name must start with a letter / `_` and contain only letters / digits / `_` / `-`"
                            .into(),
                    );
                } else if app
                    .library
                    .prompts
                    .iter()
                    .any(|p| p.name == name)
                {
                    *error = Some(format!("`{name}` already exists"));
                } else {
                    // Stage the addition.
                    let new_prompt = Prompt {
                        name: name.clone(),
                        description: String::new(),
                        template: String::new(),
                    };
                    app.library.prompts.push(new_prompt);
                    app.added.insert(name.clone());
                    app.dirty.insert(name.clone());
                    // Re-sort so the new entry lands
                    // alphabetically.
                    app.library.prompts.sort_by(|a, b| a.name.cmp(&b.name));
                    // Move cursor to the new entry +
                    // load it into the editor.
                    if let Some(idx) =
                        app.library.prompts.iter().position(|p| p.name == name)
                    {
                        app.cursor = idx;
                    }
                    app.reload_editor();
                    app.focus = Focus::Editor;
                    app.modal = Modal::None;
                    app.status =
                        format!("staged new prompt `{name}` — Ctrl+S to commit");
                }
            }
            KeyCode::Backspace => {
                buffer.pop();
                *error = None;
            }
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                buffer.push(c);
                *error = None;
            }
            _ => {}
        }
        return Ok(false);
    }
    if let Modal::DeletePromptConfirm { name } = &app.modal {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                let name = name.clone();
                if app.added.contains(&name) {
                    // Newly-added entry — drop
                    // entirely without bothering the
                    // save pipeline.
                    app.library.prompts.retain(|p| p.name != name);
                    app.added.remove(&name);
                    app.dirty.remove(&name);
                    if app.cursor >= app.library.prompts.len()
                        && app.cursor > 0
                    {
                        app.cursor -= 1;
                    }
                    app.reload_editor();
                    app.status = format!("dropped unsaved prompt `{name}`");
                } else {
                    // Existing entry — mark for
                    // removal but keep in tree so
                    // it renders struck-through.
                    app.removed.insert(name.clone());
                    app.status =
                        format!("staged deletion of `{name}` — Ctrl+S to commit");
                }
                app.modal = Modal::None;
            }
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                app.modal = Modal::None;
            }
            _ => {}
        }
        return Ok(false);
    }
    if matches!(app.modal, Modal::DiscardConfirm { .. }) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                return Ok(true);
            }
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                app.modal = Modal::None;
            }
            _ => {}
        }
        return Ok(false);
    }
    if let Modal::Rollback { entries, cursor } = &mut app.modal {
        match key.code {
            KeyCode::Esc => {
                app.modal = Modal::None;
            }
            KeyCode::Up => {
                if *cursor > 0 {
                    *cursor -= 1;
                }
            }
            KeyCode::Down => {
                if *cursor + 1 < entries.len() {
                    *cursor += 1;
                }
            }
            KeyCode::PageUp => {
                *cursor = cursor.saturating_sub(5);
            }
            KeyCode::PageDown => {
                *cursor = (*cursor + 5).min(entries.len().saturating_sub(1));
            }
            KeyCode::Home => {
                *cursor = 0;
            }
            KeyCode::End => {
                *cursor = entries.len().saturating_sub(1);
            }
            KeyCode::Enter => {
                let Some(entry) = entries.get(*cursor).cloned() else {
                    app.modal = Modal::None;
                    return Ok(false);
                };
                let outcome = stage_rollback(app, &entry);
                match outcome {
                    Ok(count) => {
                        app.modal = Modal::None;
                        app.status = format!(
                            "rollback staged {count} change{} from {} — Ctrl+S to commit",
                            if count == 1 { "" } else { "s" },
                            entry.filename,
                        );
                    }
                    Err(e) => {
                        app.status = format!("rollback failed: {e:#}");
                    }
                }
            }
            KeyCode::Char('v') | KeyCode::Char('V') => {
                let Some(entry) = entries.get(*cursor).cloned() else {
                    return Ok(false);
                };
                match backup::read(&entry) {
                    Ok(body) => {
                        app.modal = Modal::RollbackPreview {
                            entry,
                            body,
                            scroll: 0,
                        };
                    }
                    Err(e) => {
                        app.status = format!("preview failed: {e:#}");
                    }
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                let Some(entry) = entries.get(*cursor).cloned() else {
                    return Ok(false);
                };
                app.modal = Modal::RollbackDelete { entry };
            }
            _ => {}
        }
        return Ok(false);
    }
    if let Modal::RollbackDelete { entry } = &app.modal {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                let entry = entry.clone();
                match backup::delete(&entry) {
                    Ok(()) => {
                        app.status =
                            format!("deleted backup {}", entry.filename);
                    }
                    Err(e) => {
                        app.status = format!("delete failed: {e:#}");
                    }
                }
                // Refresh the picker.
                match backup::list(&app.project_root) {
                    Ok(es) if !es.is_empty() => {
                        app.modal = Modal::Rollback {
                            entries: es,
                            cursor: 0,
                        };
                    }
                    _ => {
                        app.modal = Modal::None;
                    }
                }
            }
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                match backup::list(&app.project_root) {
                    Ok(es) if !es.is_empty() => {
                        app.modal = Modal::Rollback {
                            entries: es,
                            cursor: 0,
                        };
                    }
                    _ => {
                        app.modal = Modal::None;
                    }
                }
            }
            _ => {}
        }
        return Ok(false);
    }
    if let Modal::RollbackPreview { body, scroll, .. } = &mut app.modal {
        let total = body.lines().count();
        match key.code {
            KeyCode::Esc => {
                match backup::list(&app.project_root) {
                    Ok(es) if !es.is_empty() => {
                        app.modal = Modal::Rollback {
                            entries: es,
                            cursor: 0,
                        };
                    }
                    _ => {
                        app.modal = Modal::None;
                    }
                }
            }
            KeyCode::Up => *scroll = scroll.saturating_sub(1),
            KeyCode::Down => {
                if *scroll + 1 < total {
                    *scroll += 1;
                }
            }
            KeyCode::PageUp => *scroll = scroll.saturating_sub(20),
            KeyCode::PageDown => {
                *scroll = (*scroll + 20).min(total.saturating_sub(1))
            }
            KeyCode::Home => *scroll = 0,
            KeyCode::End => *scroll = total.saturating_sub(1),
            _ => {}
        }
        return Ok(false);
    }

    // Global exit chords.
    if key.code == KeyCode::Char('q')
        && key.modifiers.contains(KeyModifiers::CONTROL)
    {
        if app.has_unsaved() {
            app.modal = Modal::DiscardConfirm {
                unsaved: app.unsaved_count(),
            };
            return Ok(false);
        }
        return Ok(true);
    }
    if key.code == KeyCode::Esc {
        if app.has_unsaved() {
            app.modal = Modal::DiscardConfirm {
                unsaved: app.unsaved_count(),
            };
            return Ok(false);
        }
        return Ok(true);
    }

    // Save chord — works from any focus.
    if key.code == KeyCode::Char('s')
        && key.modifiers.contains(KeyModifiers::CONTROL)
    {
        // Stash any in-progress editor edit before
        // computing the save summary.
        app.stash_editor();
        if !app.has_unsaved() {
            app.status = "nothing to save".into();
            return Ok(false);
        }
        let summary = app.save_summary();
        app.modal = Modal::SaveConfirm { summary };
        return Ok(false);
    }

    // Rollback picker — global Ctrl+R.
    if key.code == KeyCode::Char('r')
        && key.modifiers.contains(KeyModifiers::CONTROL)
    {
        open_rollback(app);
        return Ok(false);
    }

    // Help — Ctrl+H or ? (only when AI prompt isn't
    // focused, since `?` is a printable char people
    // type into prompts).
    if (key.code == KeyCode::Char('h')
        && key.modifiers.contains(KeyModifiers::CONTROL))
        || (key.code == KeyCode::Char('?') && app.focus != Focus::AiPrompt)
    {
        open_help(app);
        return Ok(false);
    }

    // Tab / Shift+Tab cycles focus.
    if key.code == KeyCode::Tab && !key.modifiers.contains(KeyModifiers::SHIFT) {
        // Stash on focus change so a half-typed edit
        // commits to the library before we drop the
        // textarea-keystroke routing.
        if app.focus == Focus::Editor {
            app.stash_editor();
        }
        app.focus = app.focus.next();
        app.first_launch = false;
        app.status = format!("focus → {}", app.focus.label());
        return Ok(false);
    }
    if key.code == KeyCode::BackTab
        || (key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT))
    {
        if app.focus == Focus::Editor {
            app.stash_editor();
        }
        app.focus = app.focus.prev();
        app.first_launch = false;
        app.status = format!("focus → {}", app.focus.label());
        return Ok(false);
    }

    // Focus-routed keys.
    match app.focus {
        Focus::List => dispatch_list_keys(app, key),
        Focus::Editor => dispatch_editor_keys(app, key),
        Focus::AiPrompt => dispatch_ai_prompt_keys(app, key),
    }
    Ok(false)
}

fn is_valid_prompt_name(name: &str) -> bool {
    let mut chars = name.chars();
    let first = match chars.next() {
        Some(c) => c,
        None => return false,
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn dispatch_list_keys(app: &mut App, key: KeyEvent) {
    let n = app.library.prompts.len();
    match key.code {
        KeyCode::Up => {
            if app.cursor > 0 {
                app.stash_editor();
                app.cursor -= 1;
                app.reload_editor();
                app.first_launch = false;
            }
        }
        KeyCode::Down => {
            if app.cursor + 1 < n {
                app.stash_editor();
                app.cursor += 1;
                app.reload_editor();
                app.first_launch = false;
            }
        }
        KeyCode::PageUp => {
            app.stash_editor();
            app.cursor = app.cursor.saturating_sub(10);
            app.reload_editor();
        }
        KeyCode::PageDown => {
            app.stash_editor();
            app.cursor = (app.cursor + 10).min(n.saturating_sub(1));
            app.reload_editor();
        }
        KeyCode::Home => {
            app.stash_editor();
            app.cursor = 0;
            app.reload_editor();
        }
        KeyCode::End => {
            app.stash_editor();
            app.cursor = n.saturating_sub(1);
            app.reload_editor();
        }
        KeyCode::Enter => {
            // Cursor-driven autoload already
            // happens on ↑↓, but pressing Enter on
            // a row commits to working on that
            // entry — jump focus into the editor
            // so the user can start typing
            // immediately.
            if app.library.prompts.get(app.cursor).is_some() {
                app.focus = Focus::Editor;
                app.first_launch = false;
                app.status = "loaded into editor".into();
            }
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            app.modal = Modal::AddPrompt {
                buffer: String::new(),
                error: None,
            };
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            if let Some(prompt) = app.library.prompts.get(app.cursor) {
                let name = prompt.name.clone();
                if app.removed.contains(&name) {
                    // Already staged for deletion —
                    // a second `d` revokes it.
                    app.removed.remove(&name);
                    app.status = format!("revoked deletion of `{name}`");
                } else {
                    app.modal = Modal::DeletePromptConfirm { name };
                }
            }
        }
        _ => {}
    }
}

fn dispatch_editor_keys(app: &mut App, key: KeyEvent) {
    // (Ctrl+G as a direct chord was reported as
    // intercepted by the terminal — Ctrl+G is ASCII
    // BEL — so the "Get response" action moved to
    // the meta-prefix chord `Ctrl+B G`, handled
    // globally in handle_key.)

    // Forward to tui-textarea with its default key
    // map — arrows, Home/End, PgUp/PgDn, Shift+arrow
    // selection, plus the emacs-style readline
    // shortcuts (Ctrl+A start-of-line, Ctrl+E end-
    // of-line, Ctrl+K kill-to-end, Ctrl+W
    // delete-word-backward, Ctrl+U/Y undo/redo).
    //
    // The Ctrl chords we DO reserve globally
    // (Ctrl+S save, Ctrl+Q quit, Ctrl+H help) are
    // intercepted by `handle_key` before this
    // dispatcher runs, so there's no conflict
    // letting tui-textarea handle everything else.
    //
    // `input()` returns true when the buffer
    // mutated; navigation returns false but still
    // moves the cursor — both cases are fine.  The
    // dirty bookkeeping happens lazily in
    // stash_editor at focus / cursor / save time.
    let input: tui_textarea::Input = key.into();
    let _ = app.editor.input(input);
    app.first_launch = false;
}

/// 1.2.11+ — Ctrl+R handler.  Build the rollback
/// picker modal from `.prompts-backups/`, or surface
/// "no backups yet" on the status bar when the
/// directory is empty / missing.
fn open_rollback(app: &mut App) {
    match backup::list(&app.project_root) {
        Ok(entries) if !entries.is_empty() => {
            app.modal = Modal::Rollback { entries, cursor: 0 };
        }
        Ok(_) => {
            app.status = format!(
                "rollback: no backups yet · save once to populate {}/{}/",
                app.project_root.display(),
                backup::BACKUP_DIR,
            );
        }
        Err(e) => {
            app.status = format!("rollback list failed: {e:#}");
        }
    }
}

/// 1.2.11+ — load a backup file into the working
/// schema, staging every leaf diff vs the current
/// library.  No disk write — the user reviews then
/// Ctrl+S to commit (which writes a fresh backup
/// of the pre-rollback state on the way).
fn stage_rollback(app: &mut App, entry: &BackupEntry) -> Result<usize> {
    let raw = backup::read(entry)?;
    let restored: PromptLibrary = serde_hjson::from_str(&raw)
        .with_context(|| format!("parse {}", entry.path.display()))?;

    // Compute the diff between the restored library
    // and the current in-memory one.  For
    // book-keeping:
    //   * Names in restored but not in current →
    //     ADDED.
    //   * Names in current but not in restored →
    //     REMOVED.
    //   * Names in both → swap templates / desc;
    //     mark dirty if anything changed.
    let mut current_by_name: std::collections::HashMap<String, &Prompt> =
        std::collections::HashMap::new();
    for p in &app.library.prompts {
        current_by_name.insert(p.name.clone(), p);
    }
    let restored_names: std::collections::HashSet<String> =
        restored.prompts.iter().map(|p| p.name.clone()).collect();

    let mut staged: usize = 0;

    // Apply additions + modifications.
    let mut new_library = PromptLibrary::default();
    for restored_prompt in &restored.prompts {
        let name = restored_prompt.name.clone();
        let was_present = current_by_name.contains_key(&name);
        if !was_present {
            // Brand-new vs current library.
            app.added.insert(name.clone());
            app.dirty.insert(name.clone());
            staged += 1;
        } else {
            // Compare bodies; if different, mark
            // dirty.
            let live = current_by_name.get(&name).copied().unwrap();
            if live.template != restored_prompt.template
                || live.description != restored_prompt.description
            {
                app.dirty.insert(name.clone());
                staged += 1;
            }
            // If the user had already staged this
            // for deletion, the rollback un-stages
            // it.
            app.removed.remove(&name);
        }
        new_library.prompts.push(restored_prompt.clone());
    }
    // Names in current but missing from restored
    // get staged for deletion.  Include them in
    // new_library so the list still renders them
    // (struck-through) until save.
    for current_prompt in &app.library.prompts {
        if !restored_names.contains(&current_prompt.name) {
            app.removed.insert(current_prompt.name.clone());
            // Keep the existing body for the
            // delete-confirm strike-through render.
            new_library.prompts.push((*current_prompt).clone());
            staged += 1;
        }
    }

    // Swap the library + sort + reload editor + reset
    // cursor to clamp.
    new_library
        .prompts
        .sort_by(|a, b| a.name.cmp(&b.name));
    app.library = new_library;
    if app.cursor >= app.library.prompts.len() && app.cursor > 0 {
        app.cursor = app.library.prompts.len().saturating_sub(1);
    }
    app.reload_editor();
    Ok(staged)
}

/// 1.2.11+ — Ctrl+G handler.  Inserts
/// `last_send.response` at the editor cursor.
/// No-op (with a status message) when there's no
/// response yet or it's empty.
fn insert_ai_response_into_editor(app: &mut App) {
    let response_text = match app.last_send.as_ref() {
        Some(send) if !send.response.trim().is_empty() => send.response.clone(),
        Some(_) => {
            app.status =
                "AI response is empty — wait for it to finish, then Ctrl+G".into();
            return;
        }
        None => {
            app.status =
                "no AI response yet — Tab to the AI prompt input and press Enter first".into();
            return;
        }
    };
    if app.inference.is_some() {
        app.status =
            "AI response still streaming — wait for it to finish".into();
        return;
    }
    app.editor.insert_str(&response_text);
    // Inserting into a previously-saved entry
    // means it's now dirty.  Stash so the dirty
    // set picks up the change.
    app.stash_editor();
    app.status = format!(
        "inserted {} chars of AI response into editor",
        response_text.chars().count(),
    );
}

fn dispatch_ai_prompt_keys(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => {
            send_ai_prompt(app);
        }
        KeyCode::Up => {
            if app.ai_history.is_empty() {
                return;
            }
            let new_cursor = match app.ai_history_cursor {
                None => Some(app.ai_history.len() - 1),
                Some(0) => Some(0),
                Some(i) => Some(i - 1),
            };
            app.ai_history_cursor = new_cursor;
            if let Some(i) = new_cursor {
                let text = app.ai_history[i].clone();
                let len = text.chars().count();
                app.ai_input.set_with_cursor(text, len);
            }
        }
        KeyCode::Down => {
            let Some(i) = app.ai_history_cursor else {
                return;
            };
            if i + 1 < app.ai_history.len() {
                app.ai_history_cursor = Some(i + 1);
                let text = app.ai_history[i + 1].clone();
                let len = text.chars().count();
                app.ai_input.set_with_cursor(text, len);
            } else {
                // Past the most-recent entry — clear
                // back to a fresh empty buffer.
                app.ai_history_cursor = None;
                app.ai_input.clear();
            }
        }
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.ai_input.clear();
            app.ai_history_cursor = None;
        }
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.ai_input.clear();
            app.ai_history.clear();
            app.ai_history_cursor = None;
            app.status = "ai prompt: input + history cleared".into();
        }
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.ai_input.move_home();
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.ai_input.move_end();
        }
        KeyCode::Home => {
            app.ai_input.move_home();
        }
        KeyCode::End => {
            app.ai_input.move_end();
        }
        KeyCode::Left => {
            app.ai_input.move_left();
        }
        KeyCode::Right => {
            app.ai_input.move_right();
        }
        KeyCode::Backspace => {
            app.ai_input.backspace();
            app.ai_history_cursor = None;
        }
        KeyCode::Delete => {
            app.ai_input.delete();
            app.ai_history_cursor = None;
        }
        KeyCode::Char(c)
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            app.ai_input.insert_char(c);
            app.ai_history_cursor = None;
        }
        _ => {}
    }
}

fn send_ai_prompt(app: &mut App) {
    let Some(rt) = app.ai.clone() else {
        app.status =
            "LLM not configured — set llm.default in inkhaven.hjson + provide its API key".into();
        return;
    };
    // Stash the editor so the template rendered
    // below sees the live in-progress edit.
    app.stash_editor();
    let editor_body = app
        .library
        .prompts
        .get(app.cursor)
        .map(|p| p.template.clone())
        .unwrap_or_default();
    if editor_body.trim().is_empty() {
        app.status =
            "prompt body is empty — focus the editor pane and write a prompt first".into();
        return;
    }
    let prompt_name = app
        .library
        .prompts
        .get(app.cursor)
        .map(|p| p.name.clone());
    let raw_input = app.ai_input.as_str().to_string();
    let instruction = raw_input.trim();
    let analysis_request = if instruction.is_empty() {
        DEFAULT_ANALYSIS_REQUEST.to_string()
    } else {
        instruction.to_string()
    };
    let rendered = build_analysis_request(&editor_body, &analysis_request);

    // Push the typed input onto the history
    // (deduped against the previous most-recent
    // entry).  Empty inputs don't get a row.
    if !instruction.is_empty()
        && app.ai_history.last().map(String::as_str) != Some(instruction)
    {
        app.ai_history.push(instruction.to_string());
    }
    app.ai_history_cursor = None;
    app.ai_input.clear();

    let rx = spawn_chat_stream(
        rt.client.client.clone(),
        rt.model.clone(),
        Some(ANALYSIS_SYSTEM_PROMPT.to_string()),
        Vec::new(),
        rendered,
    );
    app.last_send = Some(Send {
        prompt_name,
        template_under_review: editor_body,
        analysis_request,
        response: String::new(),
        started_at: Instant::now(),
        duration: None,
        failed: false,
    });
    app.inference = Some(Inference {
        rx,
        started_at: Instant::now(),
    });
    app.status =
        format!("analysing prompt via {} ({})…", rt.provider, rt.model);
}

/// Drain any StreamMsg events that arrived since the
/// last frame.  Mutates `app.last_send.response` /
/// `.duration` / `.failed` based on what's flowed
/// through.  Called from the event loop just before
/// `terminal.draw`.
fn pump_inference(app: &mut App) {
    // Pump in a scope so the &mut borrow on
    // `app.inference` drops before we try to set it
    // back to `None`.
    let done = {
        let Some(inf) = app.inference.as_mut() else {
            return;
        };
        let mut finished = false;
        loop {
            match inf.rx.try_recv() {
                Ok(StreamMsg::Token(chunk)) => {
                    if let Some(send) = app.last_send.as_mut() {
                        send.response.push_str(&chunk);
                    }
                }
                Ok(StreamMsg::Done) => {
                    if let Some(send) = app.last_send.as_mut() {
                        send.duration = Some(send.started_at.elapsed());
                    }
                    finished = true;
                    break;
                }
                Ok(StreamMsg::Error(e)) => {
                    if let Some(send) = app.last_send.as_mut() {
                        send.failed = true;
                        if !send.response.is_empty() {
                            send.response.push_str("\n\n");
                        }
                        send.response.push_str(&format!("⚠ ERROR: {e}"));
                        send.duration = Some(send.started_at.elapsed());
                    }
                    finished = true;
                    break;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    if let Some(send) = app.last_send.as_mut() {
                        if send.duration.is_none() {
                            send.duration = Some(send.started_at.elapsed());
                        }
                    }
                    finished = true;
                    break;
                }
            }
        }
        finished
    };
    if done {
        app.inference = None;
        if let Some(send) = app.last_send.as_ref() {
            if send.failed {
                app.status = "AI response: FAILED".into();
            } else {
                let secs = send
                    .duration
                    .map(|d| d.as_secs_f32())
                    .unwrap_or(0.0);
                app.status = format!("AI response ready · {secs:.1}s");
            }
        }
    }
}

// ── save pipeline ─────────────────────────────────────

fn perform_save(app: &mut App) -> Result<String> {
    // Stash the current editor text one more time
    // (the Ctrl+S path stashed already, but a
    // belt-and-braces call here protects against
    // mid-modal edits in the future).
    app.stash_editor();

    // Build the on-disk library: clone everything,
    // then filter out the staged-for-deletion
    // entries.
    let mut library = app.library.clone();
    if !app.removed.is_empty() {
        library
            .prompts
            .retain(|p| !app.removed.contains(&p.name));
    }
    let body = serde_hjson::to_string(&library)
        .context("serialise library to HJSON")?;

    // Atomic write: write to tmp + rename.
    let mut tmp_path = app.prompts_path.clone();
    let mut tmp_name = app
        .prompts_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    tmp_name.push_str(".tmp");
    tmp_path.set_file_name(&tmp_name);
    fs::write(&tmp_path, &body)
        .with_context(|| format!("write {}", tmp_path.display()))?;
    fs::rename(&tmp_path, &app.prompts_path).with_context(|| {
        format!(
            "rename {} → {}",
            tmp_path.display(),
            app.prompts_path.display()
        )
    })?;

    // Timestamped backup of the just-written file.
    let backup_dir = app.project_root.join(".prompts-backups");
    fs::create_dir_all(&backup_dir)
        .with_context(|| format!("create {}", backup_dir.display()))?;
    let ts = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_path = backup_dir.join(format!("prompts_{ts}.hjson"));
    fs::write(&backup_path, &body)
        .with_context(|| format!("write {}", backup_path.display()))?;

    // Sync app state with what's now on disk.
    let summary = app.save_summary();
    app.library
        .prompts
        .retain(|p| !app.removed.contains(&p.name));
    app.removed.clear();
    app.added.clear();
    app.dirty.clear();
    app.loaded_from_defaults = false;
    app.baseline = build_baseline(&app.library);
    if app.cursor >= app.library.prompts.len() && app.cursor > 0 {
        app.cursor = app.library.prompts.len().saturating_sub(1);
    }
    app.reload_editor();
    app.status = format!(
        "saved · {} prompts · backup {}",
        app.library.prompts.len(),
        backup_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default(),
    );
    let mut msg = String::new();
    msg.push_str(&format!(
        "Saved {} entries to {}.\n",
        app.library.prompts.len(),
        app.prompts_path.display(),
    ));
    msg.push_str(&format!("Backup: {}\n\n", backup_path.display()));
    if !summary.added.is_empty() {
        msg.push_str(&format!("Added: {}\n", summary.added.join(", ")));
    }
    if !summary.modified.is_empty() {
        msg.push_str(&format!("Modified: {}\n", summary.modified.join(", ")));
    }
    if !summary.removed.is_empty() {
        msg.push_str(&format!("Removed: {}\n", summary.removed.join(", ")));
    }
    Ok(msg)
}

fn open_help(app: &mut App) {
    let body = match app.focus {
        Focus::List => list_help_body(),
        Focus::Editor => editor_help_body(),
        Focus::AiPrompt => ai_prompt_help_body(),
    };
    app.modal = Modal::Help { body };
    app.first_launch = false;
}

fn list_help_body() -> String {
    [
        " Prompts list — chord summary",
        "",
        "   ↑↓                navigate entries",
        "   PgUp / PgDn       jump 10 entries",
        "   Home / End        first / last entry",
        "   a                 add new prompt (name prompt)",
        "   d                 delete focused prompt (confirm)",
        "                       second `d` on a deleted entry revokes",
        "   Tab / Shift+Tab   cycle pane focus",
        "   Ctrl+S            save library (confirm modal)",
        "   Ctrl+R            rollback picker (list .prompts-backups/)",
        "   Esc / Ctrl+Q      quit (confirm if unsaved)",
        "",
        " Status chips:",
        "   ✱ unsaved edit",
        "   ✚ newly added (staged)",
        "   ✗ staged for deletion (strike-through)",
        "",
        " Phase 3 adds: live LLM evaluation in the AI pane.",
    ]
    .join("\n")
}

fn editor_help_body() -> String {
    [
        " Prompt editor — chord summary",
        "",
        "   Arrows / Home / End / PgUp / PgDn",
        "                     movement",
        "   Shift+arrows      extend selection",
        "   Backspace / Del   delete character",
        "   Ctrl+A / Ctrl+E   start / end of line",
        "   Ctrl+B / Ctrl+F   cursor left / right",
        "   Ctrl+N / Ctrl+P   cursor down / up",
        "   Ctrl+K            kill to end of line",
        "   Ctrl+W            delete previous word",
        "   Ctrl+U / Ctrl+Y   undo / redo",
        "   Type to insert.",
        "",
        " AI-response insertion (works from any pane):",
        "   Ctrl+B G          \"Get response\" — insert the latest",
        "                     AI pane response at the editor cursor",
        "                     and jump focus to the editor.  Used",
        "                     to be plain Ctrl+G but the terminal",
        "                     eats Ctrl+G as ASCII BEL on most",
        "                     setups, so it moved to the meta",
        "                     prefix.  No-op (with status message)",
        "                     when the response is missing or",
        "                     still streaming.",
        "",
        " App-global chords (intercepted before the editor sees them):",
        "   Ctrl+S            save library (confirm modal)",
        "   Ctrl+H / ?        help (this pane)",
        "   Ctrl+Q / Esc      quit (confirm if unsaved)",
        "   Tab / Shift+Tab   cycle pane focus",
        "",
        " Template variables — DOCUMENTATION ONLY in this editor.",
        " These placeholders are NOT substituted when you send the",
        " template to the reviewer LLM; the reviewer sees them as",
        " literal text and comments on them as part of the critique.",
        "",
        "   {{selection}}    inkhaven substitutes the selected",
        "                    prose at runtime in the main editor",
        "   {{context}}      inkhaven substitutes the hierarchical",
        "                    book/chapter/subchapter context",
    ]
    .join("\n")
}

fn ai_prompt_help_body() -> String {
    [
        " AI prompt input — chord summary",
        "",
        "   type to edit · Backspace / Delete remove",
        "   Left / Right / Home / End / Ctrl+A / Ctrl+E",
        "                  cursor movement",
        "   Up / Down       history walk (in-session)",
        "   Enter           SEND for analysis",
        "   Ctrl+L          clear input",
        "   Ctrl+K          clear input + clear history",
        "",
        " What gets sent — the LLM acts as a prompt",
        " reviewer, NOT as an executor of your template.",
        "",
        "   system  → a fixed framing that tells the LLM",
        "            it's reviewing a prompt template,",
        "            not running one.  Placeholders like",
        "            {{selection}} are explained as",
        "            runtime substitutions inkhaven",
        "            handles later.",
        "   user    → fenced template body (your editor",
        "            pane verbatim) + your typed",
        "            analysis request.  Placeholders are",
        "            NOT substituted — the reviewer sees",
        "            them as-is so it can comment on",
        "            their use.",
        "",
        " Empty AI prompt input — Enter still works.  A",
        " sensible default request kicks in:",
        "   \"Critique this prompt template.  Identify",
        "    strengths and weaknesses, comment on",
        "    placeholder use, suggest improvements.\"",
        "",
        " Single-shot per send — each Enter is an",
        " independent review; there's no conversation",
        " history between sends.",
    ]
    .join("\n")
}

// ── render ────────────────────────────────────────────

fn render(f: &mut ratatui::Frame, app: &mut App) {
    let size = f.area();
    // Vertical split: top body (panes) + bottom AI
    // prompt input (3 rows incl. its border) + status
    // bar (1 row).
    let v_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // top bar
            Constraint::Min(0),     // body
            Constraint::Length(3),  // AI prompt input
            Constraint::Length(1),  // status bar
        ])
        .split(size);
    draw_top_bar(f, v_chunks[0], app);
    draw_body(f, v_chunks[1], app);
    draw_ai_prompt(f, v_chunks[2], app);
    draw_status(f, v_chunks[3], app);

    match &app.modal {
        Modal::None => {
            if app.first_launch {
                draw_welcome_overlay(f, size, app);
            }
        }
        Modal::Help { body } => draw_help_modal(f, size, body),
        Modal::Saved { message } => draw_saved_overlay(f, size, message),
        Modal::SaveConfirm { summary } => {
            draw_save_confirm(f, size, &app.prompts_path, summary);
        }
        Modal::AddPrompt { buffer, error } => {
            draw_add_prompt(f, size, buffer, error.as_deref());
        }
        Modal::DeletePromptConfirm { name } => {
            draw_delete_prompt_confirm(f, size, name, app.added.contains(name));
        }
        Modal::DiscardConfirm { unsaved } => {
            draw_discard_confirm(f, size, *unsaved);
        }
        Modal::Rollback { entries, cursor } => {
            draw_rollback_picker(f, size, entries, *cursor);
        }
        Modal::RollbackDelete { entry } => {
            draw_rollback_delete_confirm(f, size, entry);
        }
        Modal::RollbackPreview { entry, body, scroll } => {
            draw_rollback_preview(f, size, entry, body, *scroll);
        }
    }
}

fn draw_top_bar(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let mut spans: Vec<Span<'_>> = Vec::new();
    spans.push(Span::styled(
        " inkhaven prompts-editor ",
        Style::default()
            .bg(Color::Cyan)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        format!("{}", app.prompts_path.display()),
        Style::default().add_modifier(Modifier::BOLD),
    ));
    if app.loaded_from_defaults {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            " from embedded defaults ",
            Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if app.meta_pending {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            " META ",
            Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ));
    }
    let unsaved = app.unsaved_count();
    if unsaved > 0 {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!(" {unsaved} unsaved "),
            Style::default()
                .bg(Color::Red)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
    }
    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        format!(" focus: {} ", app.focus.label()),
        Style::default()
            .bg(Color::Magenta)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    ));
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_body(f: &mut ratatui::Frame, area: Rect, app: &mut App) {
    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(22),
            Constraint::Percentage(43),
            Constraint::Percentage(35),
        ])
        .split(area);
    let list_rect = h_chunks[0];
    let editor_rect = h_chunks[1];
    let ai_rect = h_chunks[2];
    draw_list_pane(f, list_rect, app);
    draw_editor_pane(f, editor_rect, app);
    draw_ai_pane(f, ai_rect, app);
}

fn draw_list_pane(f: &mut ratatui::Frame, area: Rect, app: &mut App) {
    let focused = app.focus == Focus::List;
    let n = app.library.prompts.len();
    let inner_h = area.height.saturating_sub(2) as usize;
    if app.cursor < app.list_scroll {
        app.list_scroll = app.cursor;
    } else if inner_h > 0 && app.cursor >= app.list_scroll + inner_h {
        app.list_scroll = app.cursor + 1 - inner_h;
    }
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Prompts ({n}) "))
        .border_style(border_style(focused));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if n == 0 {
        let body = vec![Line::from(Span::styled(
            "  (empty — Phase 2 ships `a` to add)",
            Style::default().add_modifier(Modifier::DIM),
        ))];
        f.render_widget(Paragraph::new(body), inner);
        return;
    }

    let mut lines: Vec<Line<'_>> = Vec::with_capacity(inner_h);
    for (i, prompt) in app
        .library
        .prompts
        .iter()
        .enumerate()
        .skip(app.list_scroll)
        .take(inner_h)
    {
        let selected = i == app.cursor;
        let removed = app.removed.contains(&prompt.name);
        let added = app.added.contains(&prompt.name);
        let dirty = app.dirty.contains(&prompt.name);
        let chip = if removed {
            "✗"
        } else if added {
            "✚"
        } else if dirty {
            "✱"
        } else {
            " "
        };
        let style = if selected {
            Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        } else if removed {
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::CROSSED_OUT | Modifier::DIM)
        } else if added {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else if dirty {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let marker = if selected { "▶" } else { " " };
        lines.push(Line::from(vec![
            Span::raw(format!(" {marker} ")),
            Span::raw(format!("{chip} ")),
            Span::styled(prompt.name.clone(), style),
        ]));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_editor_pane(f: &mut ratatui::Frame, area: Rect, app: &mut App) {
    let focused = app.focus == Focus::Editor;
    let title = match app.current_prompt() {
        Some(p) => {
            let name = p.name.clone();
            let dirty_marker = if app.dirty.contains(&name) {
                " · ✱ unsaved"
            } else if app.removed.contains(&name) {
                " · ✗ DELETING"
            } else if app.added.contains(&name) {
                " · ✚ NEW"
            } else {
                ""
            };
            format!(" Editor — `{name}`{dirty_marker} ")
        }
        None => " Editor ".to_string(),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border_style(focused));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.library.prompts.is_empty() {
        let body = vec![Line::from(Span::styled(
            "  (no prompts — focus the list and press `a` to add one)",
            Style::default().add_modifier(Modifier::DIM),
        ))];
        f.render_widget(Paragraph::new(body), inner);
        return;
    }

    // Style the textarea so the focused-vs-not state
    // matches the surrounding pane's border + the
    // cursor is visible only when focused.
    let cursor_style = if focused {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };
    let line_style = Style::default();
    app.editor.set_cursor_style(cursor_style);
    app.editor.set_cursor_line_style(line_style);
    app.editor.set_block(Block::default());
    f.render_widget(&app.editor, inner);
}

fn draw_ai_pane(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let title = ai_pane_title(app);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border_style(false));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let dim = Style::default().add_modifier(Modifier::DIM);
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let mut lines: Vec<Line<'_>> = Vec::new();

    if app.ai.is_none() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  ⚠ LLM not configured",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Set `llm.default` in inkhaven.hjson and",
            dim,
        )));
        lines.push(Line::from(Span::styled(
            "  provide its API-key env var, then relaunch.",
            dim,
        )));
        f.render_widget(
            Paragraph::new(lines).wrap(Wrap { trim: false }),
            inner,
        );
        return;
    }

    let Some(send) = app.last_send.as_ref() else {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  (no analysis yet)",
            dim,
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Tab to the AI prompt input.  Press Enter to ask",
            dim,
        )));
        lines.push(Line::from(Span::styled(
            "  the LLM to critique the focused prompt template.",
            dim,
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Empty input → default critique request.",
            dim,
        )));
        lines.push(Line::from(Span::styled(
            "  Typed input → that text becomes the analysis",
            dim,
        )));
        lines.push(Line::from(Span::styled(
            "  request (e.g. \"is this clear?\", \"shorten\").",
            dim,
        )));
        f.render_widget(
            Paragraph::new(lines).wrap(Wrap { trim: false }),
            inner,
        );
        return;
    };

    // Three sections — what the reviewer LLM was
    // shown.  Template under review (the editor
    // body at send time) + analysis request (the
    // user's instruction, or the embedded default).
    let header = match send.prompt_name.as_deref() {
        Some(n) => format!(" ▸ template under review · `{n}`"),
        None => " ▸ template under review".to_string(),
    };
    lines.push(Line::from(Span::styled(header, bold)));
    if send.template_under_review.trim().is_empty() {
        lines.push(Line::from(Span::styled(
            "   (empty)",
            dim,
        )));
    } else {
        for body in send.template_under_review.lines() {
            lines.push(Line::from(format!("   {body}")));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(" ▸ analysis request", bold)));
    for body in send.analysis_request.lines() {
        lines.push(Line::from(format!("   {body}")));
    }
    lines.push(Line::from(""));

    // Assistant response section.
    let header_style = if send.failed {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        bold
    };
    lines.push(Line::from(Span::styled(" ▸ assistant", header_style)));
    if send.response.is_empty() && send.duration.is_none() {
        let elapsed = send.started_at.elapsed();
        let secs = elapsed.as_secs_f32();
        let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let idx = (elapsed.as_millis() / 100) as usize % spinner_frames.len();
        lines.push(Line::from(vec![
            Span::raw("   "),
            Span::styled(
                spinner_frames[idx],
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("streaming · {secs:.1}s"),
                dim,
            ),
        ]));
    } else if send.response.is_empty() {
        lines.push(Line::from(Span::styled(
            "   (empty response)",
            dim,
        )));
    } else {
        for body in send.response.lines() {
            lines.push(Line::from(format!("   {body}")));
        }
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn ai_pane_title(app: &App) -> String {
    let Some(rt) = app.ai.as_ref() else {
        return " AI response ".to_string();
    };
    let model = rt.model.as_str();
    let provider = rt.provider.as_str();
    match (app.inference.as_ref(), app.last_send.as_ref()) {
        (Some(_), _) => format!(" AI · {provider} · {model} · streaming "),
        (None, Some(send)) => {
            if send.failed {
                format!(" AI · {provider} · {model} · FAILED ")
            } else if let Some(d) = send.duration {
                format!(
                    " AI · {provider} · {model} · {:.1}s ",
                    d.as_secs_f32()
                )
            } else {
                format!(" AI · {provider} · {model} ")
            }
        }
        (None, None) => format!(" AI · {provider} · {model} "),
    }
}

fn draw_ai_prompt(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let focused = app.focus == Focus::AiPrompt;
    let title = if app.inference.is_some() {
        " Test prompt · sending… ".to_string()
    } else if app.ai.is_none() {
        " Test prompt · (LLM disabled) ".to_string()
    } else {
        match &app.ai_history_cursor {
            Some(i) => format!(
                " Test prompt · history {}/{} ",
                i + 1,
                app.ai_history.len(),
            ),
            None => " Test prompt ".to_string(),
        }
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border_style(focused));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let dim = Style::default().add_modifier(Modifier::DIM);
    let rendered = app.ai_input.render_with_cursor(if focused { '│' } else { ' ' });
    let hint = if app.ai.is_none() {
        "  (LLM not configured — see AI pane)"
    } else if app.inference.is_some() {
        "  (Esc cancels by ending the session; Enter ignored while streaming)"
    } else if focused {
        "  Enter sends · Up/Down history · Ctrl+L clear · Ctrl+K clear+history"
    } else {
        "  Tab to focus · type a test input, Enter sends"
    };
    let lines = vec![
        Line::from(format!(" › {rendered}")),
        Line::from(Span::styled(hint, dim)),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_status(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let dim = Style::default().add_modifier(Modifier::DIM);
    let hints = match app.focus {
        Focus::List => {
            " ↑↓ · a add · d delete · Ctrl+S save · Ctrl+R rollback · Tab next · ? help"
        }
        Focus::Editor => {
            " type · Ctrl+B G insert AI response · Ctrl+S save · Tab next · Ctrl+H help"
        }
        Focus::AiPrompt => {
            " type · Enter send · Up/Down history · Ctrl+L clear · Tab next · Ctrl+H help"
        }
    };
    let pos = format!(" {}/{} ", app.cursor + 1, app.library.prompts.len().max(1));
    let spans = vec![
        Span::styled(pos, dim),
        Span::raw("  "),
        Span::raw(app.status.clone()),
        Span::raw("   "),
        Span::styled(hints, dim),
    ];
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_help_modal(f: &mut ratatui::Frame, host: Rect, body: &str) {
    let w = host.width.saturating_sub(8).min(96);
    let h = host.height.saturating_sub(4).min(28);
    let x = host.x + host.width.saturating_sub(w) / 2;
    let y = host.y + host.height.saturating_sub(h) / 2;
    let rect = Rect { x, y, width: w, height: h };
    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help ")
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let mut lines: Vec<Line<'_>> = Vec::new();
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let cyan_bold =
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    for raw in body.lines() {
        let trimmed = raw.trim_start();
        if raw.starts_with(' ')
            && !raw.starts_with("  ")
            && trimmed
                .chars()
                .next()
                .map(|c| c.is_ascii_uppercase())
                .unwrap_or(false)
        {
            lines.push(Line::from(Span::styled(raw.to_string(), cyan_bold)));
        } else {
            lines.push(Line::from(Span::raw(raw.to_string())));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " any key closes ",
        Style::default().add_modifier(Modifier::DIM),
    )));
    let _ = bold;
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn draw_save_confirm(
    f: &mut ratatui::Frame,
    host: Rect,
    path: &Path,
    summary: &SaveSummary,
) {
    let entry_count = summary.total();
    let max_rows = (entry_count.min(20) as u16).max(1);
    let w = host.width.saturating_sub(8).min(96);
    let h = (max_rows + 8).min(host.height.saturating_sub(4));
    let x = host.x + host.width.saturating_sub(w) / 2;
    let y = host.y + host.height.saturating_sub(h) / 2;
    let rect = Rect { x, y, width: w, height: h };
    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Save? ")
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let dim = Style::default().add_modifier(Modifier::DIM);
    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  Save "),
            Span::styled(format!("{entry_count}"), bold),
            Span::raw(" pending change"),
            Span::raw(if entry_count == 1 { "" } else { "s" }),
            Span::raw(" to "),
            Span::styled(format!("{}", path.display()), bold),
            Span::raw("?"),
        ]),
        Line::from(""),
    ];
    if !summary.added.is_empty() {
        lines.push(Line::from(Span::styled(" ✚ added:", bold)));
        for name in &summary.added {
            lines.push(Line::from(format!("     {name}")));
        }
    }
    if !summary.modified.is_empty() {
        lines.push(Line::from(Span::styled(" ✱ modified:", bold)));
        for name in &summary.modified {
            lines.push(Line::from(format!("     {name}")));
        }
    }
    if !summary.removed.is_empty() {
        lines.push(Line::from(Span::styled(" ✗ removed:", bold)));
        for name in &summary.removed {
            lines.push(Line::from(format!("     {name}")));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "    A timestamped copy will land in <project>/.prompts-backups/",
        dim,
    )));
    lines.push(Line::from(Span::styled(
        "    y / Enter confirm · n / Esc cancel",
        dim,
    )));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn draw_saved_overlay(f: &mut ratatui::Frame, host: Rect, message: &str) {
    let w = host.width.saturating_sub(8).min(96);
    let h: u16 = 12;
    let x = host.x + host.width.saturating_sub(w) / 2;
    let y = host.y + host.height.saturating_sub(h) / 2;
    let rect = Rect { x, y, width: w, height: h };
    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Saved ")
        .border_style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let mut lines: Vec<Line<'_>> = vec![Line::from("")];
    for line in message.lines() {
        lines.push(Line::from(Span::raw(format!("  {line}"))));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  any key dismisses",
        Style::default().add_modifier(Modifier::DIM),
    )));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn draw_add_prompt(
    f: &mut ratatui::Frame,
    host: Rect,
    buffer: &str,
    error: Option<&str>,
) {
    let w = host.width.saturating_sub(8).min(72);
    let h: u16 = 10;
    let x = host.x + host.width.saturating_sub(w) / 2;
    let y = host.y + host.height.saturating_sub(h) / 2;
    let rect = Rect { x, y, width: w, height: h };
    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Add prompt ")
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let dim = Style::default().add_modifier(Modifier::DIM);
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled("    Name:", bold)),
        Line::from(format!("    {buffer}│")),
        Line::from(""),
    ];
    if let Some(err) = error {
        lines.push(Line::from(Span::styled(
            format!("  ⚠ {err}"),
            Style::default().fg(Color::Red),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "    Identifier: letter or `_` to start, then letters/digits/`_`/`-`",
            dim,
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "    Enter commits (stages — Ctrl+S to write) · Esc cancels",
        dim,
    )));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn draw_delete_prompt_confirm(
    f: &mut ratatui::Frame,
    host: Rect,
    name: &str,
    is_newly_added: bool,
) {
    let w = host.width.saturating_sub(8).min(72);
    let h: u16 = 9;
    let x = host.x + host.width.saturating_sub(w) / 2;
    let y = host.y + host.height.saturating_sub(h) / 2;
    let rect = Rect { x, y, width: w, height: h };
    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Delete prompt? ")
        .border_style(
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let dim = Style::default().add_modifier(Modifier::DIM);
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let detail = if is_newly_added {
        "    Drops the unsaved addition entirely."
    } else {
        "    Stages deletion (struck-through until Ctrl+S commits)."
    };
    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("    Delete "),
            Span::styled(name.to_string(), bold),
            Span::raw(" ?"),
        ]),
        Line::from(""),
        Line::from(Span::styled(detail, dim)),
        Line::from(""),
        Line::from(Span::styled(
            "    y / Enter confirm · n / Esc cancel",
            dim,
        )),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_discard_confirm(f: &mut ratatui::Frame, host: Rect, unsaved: usize) {
    let w = host.width.saturating_sub(8).min(72);
    let h: u16 = 8;
    let x = host.x + host.width.saturating_sub(w) / 2;
    let y = host.y + host.height.saturating_sub(h) / 2;
    let rect = Rect { x, y, width: w, height: h };
    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Discard unsaved changes? ")
        .border_style(
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("    "),
            Span::styled(
                format!("{unsaved} pending change{}", if unsaved == 1 { "" } else { "s" }),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(" will be lost."),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "    y / Enter discard + quit · n / Esc keep editing",
            Style::default().add_modifier(Modifier::DIM),
        )),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_rollback_picker(
    f: &mut ratatui::Frame,
    host: Rect,
    entries: &[BackupEntry],
    cursor: usize,
) {
    let w = host.width.saturating_sub(8).min(96);
    let h = (entries.len() as u16 + 6).min(host.height.saturating_sub(4));
    let x = host.x + host.width.saturating_sub(w) / 2;
    let y = host.y + host.height.saturating_sub(h) / 2;
    let rect = Rect { x, y, width: w, height: h };
    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Prompts rollback — {} backups ", entries.len()))
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let now = chrono::Local::now();
    let dim = Style::default().add_modifier(Modifier::DIM);
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let mut lines: Vec<Line<'_>> = Vec::with_capacity(entries.len() + 2);
    for (i, entry) in entries.iter().enumerate() {
        let selected = i == cursor;
        let style = if selected {
            Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        } else {
            Style::default()
        };
        let marker = if selected { "▶" } else { " " };
        let rel = backup::relative_time(entry, now);
        let abs = entry
            .timestamp
            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| entry.filename.clone());
        let size = if entry.size_bytes < 1024 {
            format!("{} B", entry.size_bytes)
        } else {
            format!("{:.1} KB", entry.size_bytes as f64 / 1024.0)
        };
        lines.push(Line::from(vec![
            Span::styled(format!(" {marker}  "), bold),
            Span::styled(abs, style),
            Span::styled(format!("   ({rel}, {size})"), dim),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " Enter restore (stages — Ctrl+S to commit) · v preview · d delete · Esc back",
        dim,
    )));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn draw_rollback_delete_confirm(
    f: &mut ratatui::Frame,
    host: Rect,
    entry: &BackupEntry,
) {
    let w = host.width.saturating_sub(8).min(72);
    let h: u16 = 8;
    let x = host.x + host.width.saturating_sub(w) / 2;
    let y = host.y + host.height.saturating_sub(h) / 2;
    let rect = Rect { x, y, width: w, height: h };
    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Delete backup? ")
        .border_style(
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let dim = Style::default().add_modifier(Modifier::DIM);
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("    "),
            Span::styled(entry.filename.clone(), bold),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "    This cannot be undone.",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "    y / Enter delete · n / Esc cancel",
            dim,
        )),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_rollback_preview(
    f: &mut ratatui::Frame,
    host: Rect,
    entry: &BackupEntry,
    body: &str,
    scroll: usize,
) {
    let w = host.width.saturating_sub(4).min(120);
    let h = host.height.saturating_sub(2).min(40);
    let x = host.x + host.width.saturating_sub(w) / 2;
    let y = host.y + host.height.saturating_sub(h) / 2;
    let rect = Rect { x, y, width: w, height: h };
    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Preview — {} ", entry.filename))
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let visible = inner.height.saturating_sub(2) as usize;
    let total = body.lines().count();
    let mut lines: Vec<Line<'_>> = body
        .lines()
        .skip(scroll)
        .take(visible)
        .map(|l| Line::from(Span::raw(l.to_string())))
        .collect();
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (empty file)",
            Style::default().add_modifier(Modifier::DIM),
        )));
    }
    let last_line = (scroll + visible).min(total);
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!(
            " lines {}-{} of {} · ↑↓ PgUp PgDn Home End scroll · Esc back",
            scroll + 1,
            last_line.max(scroll + 1),
            total,
        ),
        Style::default().add_modifier(Modifier::DIM),
    )));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn draw_welcome_overlay(f: &mut ratatui::Frame, host: Rect, app: &App) {
    let w = host.width.saturating_sub(8).min(72);
    let h: u16 = 12;
    let x = host.x + host.width.saturating_sub(w) / 2;
    let y = host.y + host.height.saturating_sub(h) / 2;
    let rect = Rect { x, y, width: w, height: h };
    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Welcome — prompts editor · Phase 1 ")
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let dim = Style::default().add_modifier(Modifier::DIM);
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let mut lines: Vec<Line<'_>> = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("{} prompts loaded", app.library.prompts.len()),
                bold,
            ),
            Span::raw("  from  "),
            Span::styled(
                if app.loaded_from_defaults {
                    "(embedded defaults)".to_string()
                } else {
                    app.prompts_path.display().to_string()
                },
                bold,
            ),
        ]),
        Line::from(""),
        Line::from(Span::raw("  Phase 1 is a read-only walk-through:")),
        Line::from(Span::raw("    · ↑↓ to navigate the prompts list")),
        Line::from(Span::raw("    · Tab to cycle between the three editable panes")),
        Line::from(Span::raw("    · ? or Ctrl+H for focus-aware help")),
        Line::from(""),
        Line::from(Span::styled(
            "  any key dismisses this banner",
            dim,
        )),
    ];
    while (lines.len() as u16) < h.saturating_sub(2) {
        lines.push(Line::from(""));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn border_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::prompts::Prompt;

    #[test]
    fn focus_next_cycles_three_panes() {
        assert_eq!(Focus::List.next(), Focus::Editor);
        assert_eq!(Focus::Editor.next(), Focus::AiPrompt);
        assert_eq!(Focus::AiPrompt.next(), Focus::List);
    }

    #[test]
    fn focus_prev_reverses() {
        assert_eq!(Focus::List.prev(), Focus::AiPrompt);
        assert_eq!(Focus::AiPrompt.prev(), Focus::Editor);
        assert_eq!(Focus::Editor.prev(), Focus::List);
    }

    #[test]
    fn embedded_defaults_parse_cleanly() {
        // Q4 guard: the auto-populate-on-first-launch
        // path depends on DEFAULT_PROMPTS staying valid
        // HJSON.  If the embedded file ever drifts,
        // this test catches it before the user
        // launches the TUI.
        let lib: PromptLibrary = serde_hjson::from_str(DEFAULT_PROMPTS)
            .expect("embedded DEFAULT_PROMPTS must be valid HJSON");
        assert!(
            !lib.prompts.is_empty(),
            "embedded defaults should ship at least one prompt"
        );
        // Spot-check expected entries — fine-grained
        // enough to catch a typo, loose enough to
        // survive normal default-list churn.
        let names: Vec<&str> =
            lib.prompts.iter().map(|p| p.name.as_str()).collect();
        assert!(
            names.iter().any(|n| n.contains("tighten") || n.contains("darker")),
            "expected at least one of the bundled default prompts, got {names:?}"
        );
    }

    #[test]
    fn help_bodies_have_expected_headings() {
        // Loose smoke — the cyan-bold pass in
        // draw_help_modal keys on the leading-space
        // + uppercase-first-char convention.  If
        // the bodies drift away from it, the help
        // pane stops looking like a structured doc.
        assert!(list_help_body().starts_with(" Prompts list"));
        assert!(editor_help_body().starts_with(" Prompt editor"));
        assert!(ai_prompt_help_body().starts_with(" AI prompt input"));
    }

    #[test]
    fn current_prompt_handles_empty_library() {
        let app = App {
            project_root: PathBuf::from("/tmp"),
            prompts_path: PathBuf::from("/tmp/prompts.hjson"),
            library: PromptLibrary::default(),
            cursor: 0,
            list_scroll: 0,
            editor: TextArea::default(),
            dirty: HashSet::new(),
            added: HashSet::new(),
            removed: HashSet::new(),
            baseline: std::collections::HashMap::new(),
            first_launch: false,
            loaded_from_defaults: false,
            focus: Focus::List,
            modal: Modal::None,
            status: String::new(),
            ai: None,
            ai_input: TextInput::new(),
            ai_history: Vec::new(),
            ai_history_cursor: None,
            last_send: None,
            inference: None,
            meta_pending: false,
        };
        assert!(app.current_prompt().is_none());
    }

    #[test]
    fn analysis_request_includes_template_verbatim_with_placeholders() {
        // Placeholders MUST appear in the output —
        // the reviewer LLM needs to see them to
        // comment on their use.
        let body = "Tighten:\n\n{{selection}}\n\nContext: {{context}}";
        let out = build_analysis_request(body, "is this clear?");
        assert!(out.contains("{{selection}}"));
        assert!(out.contains("{{context}}"));
        assert!(out.contains("Tighten:"));
        assert!(out.contains("is this clear?"));
    }

    #[test]
    fn analysis_request_uses_fenced_markers() {
        let out = build_analysis_request("body text", "instruction");
        assert!(out.contains("--- PROMPT TEMPLATE UNDER REVIEW ---"));
        assert!(out.contains("--- END TEMPLATE ---"));
        assert!(out.contains("Analysis request:"));
    }

    #[test]
    fn analysis_request_falls_back_to_default_when_input_empty() {
        let out = build_analysis_request("body", "");
        assert!(out.contains(DEFAULT_ANALYSIS_REQUEST));
    }

    #[test]
    fn analysis_request_falls_back_to_default_on_whitespace_only_input() {
        let out = build_analysis_request("body", "    \t  \n  ");
        assert!(out.contains(DEFAULT_ANALYSIS_REQUEST));
    }

    #[test]
    fn analysis_request_trims_user_instruction() {
        // Surrounding whitespace gets stripped so the
        // instruction lands cleanly under the
        // "Analysis request:" header.
        let out = build_analysis_request("body", "  rewrite to be concise  ");
        assert!(out.contains("Analysis request:\nrewrite to be concise"));
        assert!(!out.contains("Analysis request:\n  rewrite"));
    }

    #[test]
    fn system_prompt_frames_reviewer_role() {
        // Sanity check: the system prompt should
        // explicitly tell the LLM not to execute the
        // template — that's the whole shift in
        // workflow this fix is about.
        assert!(ANALYSIS_SYSTEM_PROMPT.contains("Do NOT try to execute"));
        assert!(ANALYSIS_SYSTEM_PROMPT.contains("reviewer"));
    }

    #[test]
    fn is_valid_prompt_name_accepts_typical_idents() {
        assert!(is_valid_prompt_name("critique-edit"));
        assert!(is_valid_prompt_name("show_dont_tell"));
        assert!(is_valid_prompt_name("_internal"));
        assert!(is_valid_prompt_name("Foo123"));
    }

    #[test]
    fn is_valid_prompt_name_rejects_leading_digit_and_empty() {
        assert!(!is_valid_prompt_name(""));
        assert!(!is_valid_prompt_name("1foo"));
        assert!(!is_valid_prompt_name("-leading-dash"));
        assert!(!is_valid_prompt_name("has space"));
        assert!(!is_valid_prompt_name("dotted.name"));
    }

    #[test]
    fn save_summary_partitions_added_modified_removed() {
        let mut lib = PromptLibrary::default();
        lib.prompts.push(Prompt {
            name: "alpha".into(),
            description: "".into(),
            template: "a".into(),
        });
        lib.prompts.push(Prompt {
            name: "beta".into(),
            description: "".into(),
            template: "b".into(),
        });
        let mut app = App {
            project_root: PathBuf::from("/tmp"),
            prompts_path: PathBuf::from("/tmp/prompts.hjson"),
            library: lib,
            cursor: 0,
            list_scroll: 0,
            editor: TextArea::default(),
            dirty: HashSet::new(),
            added: HashSet::new(),
            removed: HashSet::new(),
            baseline: std::collections::HashMap::new(),
            first_launch: false,
            loaded_from_defaults: false,
            focus: Focus::List,
            modal: Modal::None,
            status: String::new(),
            ai: None,
            ai_input: TextInput::new(),
            ai_history: Vec::new(),
            ai_history_cursor: None,
            last_send: None,
            inference: None,
            meta_pending: false,
        };
        // alpha modified, beta unchanged, gamma added, beta also removed.
        app.dirty.insert("alpha".into());
        app.added.insert("gamma".into());
        app.dirty.insert("gamma".into()); // newly-added is also dirty by definition
        app.removed.insert("beta".into());

        let s = app.save_summary();
        assert_eq!(s.added, vec!["gamma".to_string()]);
        assert_eq!(s.modified, vec!["alpha".to_string()]);
        assert_eq!(s.removed, vec!["beta".to_string()]);
        // gamma must NOT appear in modified (it's an
        // addition, not an edit-of-existing).
        assert!(!s.modified.contains(&"gamma".to_string()));
    }

    #[test]
    fn has_unsaved_covers_all_three_buckets_and_defaults_load() {
        let mut app = App {
            project_root: PathBuf::from("/tmp"),
            prompts_path: PathBuf::from("/tmp/prompts.hjson"),
            library: PromptLibrary::default(),
            cursor: 0,
            list_scroll: 0,
            editor: TextArea::default(),
            dirty: HashSet::new(),
            added: HashSet::new(),
            removed: HashSet::new(),
            baseline: std::collections::HashMap::new(),
            first_launch: false,
            loaded_from_defaults: false,
            focus: Focus::List,
            modal: Modal::None,
            status: String::new(),
            ai: None,
            ai_input: TextInput::new(),
            ai_history: Vec::new(),
            ai_history_cursor: None,
            last_send: None,
            inference: None,
            meta_pending: false,
        };
        assert!(!app.has_unsaved());
        app.dirty.insert("x".into());
        assert!(app.has_unsaved());
        app.dirty.clear();
        app.added.insert("y".into());
        assert!(app.has_unsaved());
        app.added.clear();
        app.removed.insert("z".into());
        assert!(app.has_unsaved());
        app.removed.clear();
        // Loaded-from-defaults counts as unsaved
        // (the file doesn't exist on disk yet).
        app.loaded_from_defaults = true;
        assert!(app.has_unsaved());
    }

    #[test]
    fn current_prompt_returns_indexed_entry() {
        let mut lib = PromptLibrary::default();
        lib.prompts.push(Prompt {
            name: "alpha".into(),
            description: "first".into(),
            template: "alpha body".into(),
        });
        lib.prompts.push(Prompt {
            name: "beta".into(),
            description: "second".into(),
            template: "beta body".into(),
        });
        let app = App {
            project_root: PathBuf::from("/tmp"),
            prompts_path: PathBuf::from("/tmp/prompts.hjson"),
            library: lib,
            cursor: 1,
            list_scroll: 0,
            editor: TextArea::default(),
            dirty: HashSet::new(),
            added: HashSet::new(),
            removed: HashSet::new(),
            baseline: std::collections::HashMap::new(),
            first_launch: false,
            loaded_from_defaults: false,
            focus: Focus::List,
            modal: Modal::None,
            status: String::new(),
            ai: None,
            ai_input: TextInput::new(),
            ai_history: Vec::new(),
            ai_history_cursor: None,
            last_send: None,
            inference: None,
            meta_pending: false,
        };
        let p = app.current_prompt().expect("cursor points at a prompt");
        assert_eq!(p.name, "beta");
    }
}
