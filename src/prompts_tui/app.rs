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
use std::time::Duration;

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
use tui_textarea::TextArea;

use crate::ai::prompts::{Prompt, PromptLibrary};
use crate::config::DEFAULT_PROMPTS;

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
        terminal.draw(|f| render(f, &mut app))?;
        if event::poll(Duration::from_millis(250))? {
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
    // Forward to tui-textarea without the emacs-
    // style Ctrl shortcuts so global chords like
    // Ctrl+S / Ctrl+Q stay live.  Matches the
    // main TUI's `input_without_shortcuts`
    // policy.
    let input: tui_textarea::Input = key.into();
    let changed = app.editor.input_without_shortcuts(input);
    if changed {
        // Don't compute dirty per keystroke; that
        // happens lazily in stash_editor at focus-
        // change / save / cursor-switch time.  Just
        // surface that something happened.
        app.first_launch = false;
    }
}

fn dispatch_ai_prompt_keys(app: &mut App, _key: KeyEvent) {
    // Phase 2: input is still inert.  Phase 3 wires
    // it to spawn_chat_stream.
    app.status =
        "AI prompt input wires to the LLM in Phase 3".into();
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
        "                     movement (tui-textarea defaults)",
        "   Shift+arrows      extend selection",
        "   Backspace / Del   delete character",
        "   Type to insert.",
        "",
        " Note: emacs-style Ctrl shortcuts are deliberately NOT",
        " forwarded to tui-textarea — Ctrl+S / Ctrl+Q / Ctrl+H are",
        " app-global chords.  Plain typing + Tab/Enter/Backspace",
        " work as expected.",
        "",
        " Template variables (recognised in Phase 3's send pipeline):",
        "   {{selection}}    replaced with the AI prompt input",
        "   {{context}}      replaced with empty (no hierarchical",
        "                    context inside this standalone editor)",
    ]
    .join("\n")
}

fn ai_prompt_help_body() -> String {
    [
        " AI prompt input — chord summary",
        "",
        "   (Phase 1: inert — Phase 3 wires the send pipeline)",
        "",
        " Phase 3 will support:",
        "   type to edit · Backspace deletes",
        "   Up / Down     history walk",
        "   Enter         SEND",
        "   Ctrl+L        clear input",
        "   Ctrl+K        clear input + history",
        "",
        " Send pipeline:",
        "   1. Render the editor body as a template.",
        "   2. Replace {{selection}} with this input.",
        "   3. Replace {{context}} with empty.",
        "   4. Send the rendered text as a USER message to the",
        "      configured LLM (no system prompt).",
        "   5. Stream the response into the AI pane.",
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

fn draw_ai_pane(f: &mut ratatui::Frame, area: Rect, _app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" AI response ")
        .border_style(border_style(false));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let dim = Style::default().add_modifier(Modifier::DIM);
    let body = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  (Phase 3 streams LLM responses here)",
            dim,
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Send semantics:",
            dim,
        )),
        Line::from(Span::styled(
            "    1. Render editor as a template.",
            dim,
        )),
        Line::from(Span::styled(
            "    2. {{selection}} ← AI prompt input.",
            dim,
        )),
        Line::from(Span::styled(
            "    3. Send rendered text as user message.",
            dim,
        )),
    ];
    f.render_widget(Paragraph::new(body).wrap(Wrap { trim: false }), inner);
}

fn draw_ai_prompt(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let focused = app.focus == Focus::AiPrompt;
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Test prompt — Phase 3 sends to LLM ")
        .border_style(border_style(focused));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let dim = Style::default().add_modifier(Modifier::DIM);
    let body = vec![Line::from(Span::styled(
        "  (Phase 3 wires this single-line input)",
        dim,
    ))];
    f.render_widget(Paragraph::new(body), inner);
}

fn draw_status(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let dim = Style::default().add_modifier(Modifier::DIM);
    let hints = match app.focus {
        Focus::List => {
            " ↑↓ · a add · d delete · Ctrl+S save · Tab next · ? help · Ctrl+Q quit"
        }
        Focus::Editor => {
            " type to edit · Ctrl+S save · Tab next · Ctrl+H help · Ctrl+Q quit"
        }
        Focus::AiPrompt => {
            " (Phase 3 wires this) · Ctrl+S save · Tab next · Ctrl+H help · Ctrl+Q quit"
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
        };
        assert!(app.current_prompt().is_none());
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
        };
        let p = app.current_prompt().expect("cursor points at a prompt");
        assert_eq!(p.name, "beta");
    }
}
