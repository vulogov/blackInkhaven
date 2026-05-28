//! 1.2.10+ — config-TUI event loop + render.
//!
//! Phase 2: typed widgets (bool / int / float /
//! string) + comment-preserving save (surgical splice
//! via `hjson_index` + `save`) + timestamped backups +
//! confirmation modal + restart-required overlay.
//!
//! No widgets that mutate the disk file outside of the
//! save path — `Ctrl+S` is the only thing that writes.
//! Esc / Ctrl+Q exit immediately; if there are unsaved
//! edits, a discard-confirm modal pops first.

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
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
use serde_json::Value;

use crate::config::Config;
use crate::config_tui::annotations::Annotations;
use crate::config_tui::backup::{self, BackupEntry};
use crate::config_tui::help::HelpIndex;
use crate::config_tui::hjson_index::{self, HjsonIndex};
use crate::config_tui::save::{self, Edit, EditKind};
use crate::config_tui::schema::{self, ConfigType, SchemaNode, ValueSource};
use crate::config_tui::widgets::{EditOutcome, Widget};

/// Two-step entry: install panic hook + raw-mode +
/// alt-screen, run the event loop, restore the
/// terminal in every exit path.
pub fn run(project: &Path) -> Result<()> {
    let cfg_path = project.join("inkhaven.hjson");
    let app = App::load(project.to_path_buf(), &cfg_path)?;

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

struct App {
    /// Project root — carries the `.config-backups/`
    /// path for save snapshots.
    project_root: PathBuf,
    cfg_path: PathBuf,
    /// Schema tree.  `current` values mutate when the
    /// user commits a widget edit.
    schema: SchemaNode,
    /// Source-text + span index of `inkhaven.hjson` at
    /// load time.  Rebuilt after each successful save.
    /// `None` when the file didn't exist at load.
    index: Option<HjsonIndex>,
    /// Original on-disk values keyed by path — frozen
    /// at load time, used to detect "changed since
    /// load" vs "matches disk".  Phase 2's edit
    /// detection compares the schema tree's `current`
    /// against this.
    original_by_path: HashMap<String, Value>,
    /// Set of paths whose `current` differs from
    /// `original_by_path`.  Drives the `[changed]`
    /// chip + the save confirmation modal.
    changed_paths: HashSet<String>,
    /// Unknown fields detected in the live HJSON.  See
    /// proposal §6.6.
    unknowns: Vec<(String, Value)>,
    collapsed: HashSet<String>,
    cursor: usize,
    scroll: usize,
    help: HelpIndex,
    modal: Modal,
    status: String,
    /// `true` once a save has succeeded in this
    /// session — used to flag the restart-required
    /// overlay on exit.
    saved_at_least_once: bool,
    /// Phase 3 — sidecar annotation store.
    annotations: Annotations,
    /// Phase 5 — map-entry paths staged for
    /// addition.  Each is a full path like
    /// `llm.providers.<name>`.  The entry's value
    /// lives in the schema tree (added as a child of
    /// the parent map); the path here is the
    /// "needs append on save" marker.
    added_map_entries: HashSet<String>,
    /// Phase 5 — map-entry paths staged for
    /// deletion.  The tree still renders them
    /// (struck-through) until save.
    removed_map_entries: HashSet<String>,
}

enum Modal {
    None,
    Help {
        body: String,
    },
    Edit {
        path: String,
        widget: Widget,
    },
    SaveConfirm {
        edits: Vec<Edit>,
    },
    Saved {
        message: String,
    },
    DiscardConfirm {
        unsaved: usize,
    },
    /// Phase 3 — `Ctrl+R` rollback picker.
    Rollback {
        entries: Vec<BackupEntry>,
        cursor: usize,
    },
    /// Phase 3 — confirm before deleting a backup.
    RollbackDelete {
        entry: BackupEntry,
    },
    /// Phase 3 — preview a backup file's contents.
    RollbackPreview {
        entry: BackupEntry,
        body: String,
        scroll: usize,
    },
    /// Phase 3 — `Ctrl+I` comment inspector.
    Inspector {
        title: String,
        comments: Option<String>,
        annotation: Option<String>,
    },
    /// Phase 3 — `Ctrl+A` annotation editor.
    Annotate {
        path: String,
        buffer: String,
    },
    /// Phase 5 — `a` add map entry.  Prompts for a
    /// new entry name; `parent` is the map path.
    AddMapEntry {
        parent: String,
        buffer: String,
        error: Option<String>,
    },
    /// Phase 5 — `d` delete confirm.  The targeted
    /// entry's full path is `path`.
    DeleteMapEntryConfirm {
        path: String,
    },
}

impl App {
    fn load(project_root: PathBuf, cfg_path: &Path) -> Result<Self> {
        let defaults_value: Value = serde_json::to_value(Config::default())
            .context("serialise Config::default() to JSON")?;

        let (live_value, index): (Value, Option<HjsonIndex>) =
            if cfg_path.exists() {
                let raw = std::fs::read_to_string(cfg_path)
                    .with_context(|| format!("read {}", cfg_path.display()))?;
                let parsed_value =
                    serde_hjson::from_str::<Value>(&raw).unwrap_or_else(|e| {
                        tracing::warn!(
                            target: "inkhaven::config_tui",
                            "{} parse failed: {e}",
                            cfg_path.display()
                        );
                        Value::Object(serde_json::Map::new())
                    });
                let idx = hjson_index::parse(&raw).ok();
                (parsed_value, idx)
            } else {
                (Value::Object(serde_json::Map::new()), None)
            };

        let (schema, unknowns) = schema::build(&defaults_value, &live_value);
        let help = HelpIndex::build();

        // Snapshot original values so we can detect
        // changes.
        let mut original_by_path: HashMap<String, Value> = HashMap::new();
        snapshot_originals(&schema, &mut original_by_path);

        let status = if cfg_path.exists() {
            format!(
                "{} loaded · {} stanzas · {} unknown",
                cfg_path.display(),
                schema.children.len(),
                unknowns.len(),
            )
        } else {
            format!(
                "{} not found · showing defaults · save will create the file",
                cfg_path.display(),
            )
        };

        let annotations = Annotations::load(&project_root);

        Ok(Self {
            project_root,
            cfg_path: cfg_path.to_path_buf(),
            schema,
            index,
            original_by_path,
            changed_paths: HashSet::new(),
            unknowns,
            collapsed: HashSet::new(),
            cursor: 0,
            scroll: 0,
            help,
            modal: Modal::None,
            status,
            saved_at_least_once: false,
            annotations,
            added_map_entries: HashSet::new(),
            removed_map_entries: HashSet::new(),
        })
    }

    fn rows(&self) -> Vec<(usize, &SchemaNode)> {
        let mut out: Vec<(usize, &SchemaNode)> = Vec::new();
        self.schema.flatten(&self.collapsed, &mut out, 0);
        out.into_iter().skip(1).collect()
    }

    fn current_node(&self) -> Option<&SchemaNode> {
        self.rows().get(self.cursor).map(|(_, n)| *n)
    }

    /// Convenience accessor; reserved for the
    /// Phase 3 annotations workflow.
    #[allow(dead_code)]
    fn current_path(&self) -> Option<String> {
        self.current_node().map(|n| n.path.clone())
    }

    /// True when ANY kind of unsaved change is
    /// pending: scalar edits, map-entry additions, or
    /// map-entry deletions.
    fn has_unsaved(&self) -> bool {
        !self.changed_paths.is_empty()
            || !self.added_map_entries.is_empty()
            || !self.removed_map_entries.is_empty()
    }

    fn unsaved_count(&self) -> usize {
        self.changed_paths.len()
            + self.added_map_entries.len()
            + self.removed_map_entries.len()
    }

    /// Stage a new value at `path` in the schema tree.
    /// Updates `changed_paths` based on whether the
    /// new value differs from `original_by_path`.
    fn stage(&mut self, path: &str, new_value: Value) {
        if let Some(node) = find_mut(&mut self.schema, path) {
            node.current = new_value.clone();
            // The source rolls up: a configured leaf
            // sets its branch to Configured.
            node.source = ValueSource::Configured;
            // Mark ancestor stanzas as Configured so
            // the tree paints green for the changed
            // path.
        }
        mark_ancestors_configured(&mut self.schema, path);
        let original = self.original_by_path.get(path);
        if original.map(|v| v == &new_value).unwrap_or(false) {
            self.changed_paths.remove(path);
        } else {
            self.changed_paths.insert(path.to_string());
        }
    }
}

fn snapshot_originals(node: &SchemaNode, out: &mut HashMap<String, Value>) {
    if !node.path.is_empty() && node.is_leaf() {
        out.insert(node.path.clone(), node.current.clone());
    }
    for child in &node.children {
        snapshot_originals(child, out);
    }
}

fn find_mut<'a>(node: &'a mut SchemaNode, path: &str) -> Option<&'a mut SchemaNode> {
    if node.path == path {
        return Some(node);
    }
    for child in &mut node.children {
        if path.starts_with(&child.path) && !child.path.is_empty() {
            if let Some(found) = find_mut(child, path) {
                return Some(found);
            }
        }
    }
    None
}

fn mark_ancestors_configured(node: &mut SchemaNode, descendant_path: &str) {
    if !node.path.is_empty()
        && descendant_path.starts_with(&node.path)
        && descendant_path.len() > node.path.len()
    {
        node.source = ValueSource::Configured;
    }
    for child in &mut node.children {
        if !child.path.is_empty() && descendant_path.starts_with(&child.path) {
            mark_ancestors_configured(child, descendant_path);
        }
    }
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
    if let Modal::Edit { path, widget } = &mut app.modal {
        match widget.handle_key(key) {
            EditOutcome::Continue => {}
            EditOutcome::Cancel => {
                app.modal = Modal::None;
                app.status = "edit cancelled".into();
            }
            EditOutcome::Commit(new_value) => {
                let path = path.clone();
                app.stage(&path, new_value);
                app.modal = Modal::None;
                app.status = format!("staged {path}");
            }
        }
        return Ok(false);
    }
    if matches!(
        app.modal,
        Modal::Help { .. } | Modal::Saved { .. } | Modal::Inspector { .. }
    ) {
        app.modal = Modal::None;
        return Ok(false);
    }
    if matches!(app.modal, Modal::AddMapEntry { .. }) {
        // Handle in two stages so we can drop the
        // modal borrow before calling staging
        // helpers that need `&mut app`.
        #[derive(Default)]
        struct Outcome {
            cancel: bool,
            commit_parent_name: Option<(String, String)>,
            set_error: Option<String>,
            clear_error: bool,
            push_char: Option<char>,
            backspace: bool,
        }
        let outcome = if let Modal::AddMapEntry { parent, buffer, .. } = &app.modal {
            let parent = parent.clone();
            let buffer = buffer.clone();
            let mut out = Outcome::default();
            match key.code {
                KeyCode::Esc => out.cancel = true,
                KeyCode::Enter => {
                    let name = buffer.trim().to_string();
                    if name.is_empty() {
                        out.set_error = Some("name is required".into());
                    } else if !is_valid_map_entry_name(&name) {
                        out.set_error = Some(
                            "name must start with a letter or `_` and contain only letters / digits / `_`".into(),
                        );
                    } else {
                        let new_path = format!("{parent}.{name}");
                        if find_in_tree(&app.schema, &new_path).is_some() {
                            out.set_error =
                                Some(format!("`{new_path}` already exists"));
                        } else {
                            out.commit_parent_name = Some((parent, name));
                        }
                    }
                }
                KeyCode::Backspace => {
                    out.backspace = true;
                    out.clear_error = true;
                }
                KeyCode::Char(c)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT) =>
                {
                    out.push_char = Some(c);
                    out.clear_error = true;
                }
                _ => {}
            }
            out
        } else {
            Outcome::default()
        };
        if outcome.cancel {
            app.modal = Modal::None;
            app.status = "add map entry: cancelled".into();
            return Ok(false);
        }
        if let Some((parent, name)) = outcome.commit_parent_name {
            let new_path = format!("{parent}.{name}");
            match stage_new_map_entry(app, &parent, &name) {
                Ok(()) => {
                    app.modal = Modal::None;
                    app.status = format!("staged new map entry {new_path}");
                }
                Err(e) => {
                    if let Modal::AddMapEntry { error, .. } = &mut app.modal {
                        *error = Some(format!("failed: {e:#}"));
                    }
                }
            }
            return Ok(false);
        }
        if let Modal::AddMapEntry { buffer, error, .. } = &mut app.modal {
            if outcome.backspace {
                buffer.pop();
            }
            if let Some(c) = outcome.push_char {
                buffer.push(c);
            }
            if outcome.clear_error {
                *error = None;
            }
            if let Some(err) = outcome.set_error {
                *error = Some(err);
            }
        }
        return Ok(false);
    }
    if let Modal::DeleteMapEntryConfirm { path } = &app.modal {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                let path = path.clone();
                if app.added_map_entries.contains(&path) {
                    // Newly-staged entry — just drop
                    // it from the tree + staging set
                    // without bothering the save
                    // pipeline.
                    app.added_map_entries.remove(&path);
                    remove_subtree(&mut app.schema, &path);
                    // Drop any staged-changed leaves
                    // under this path.
                    app.changed_paths
                        .retain(|p| !(p == &path || p.starts_with(&format!("{path}."))));
                    app.status = format!("dropped unsaved map entry {path}");
                } else {
                    app.removed_map_entries.insert(path.clone());
                    app.changed_paths.insert(path.clone());
                    app.status = format!("staged deletion of {path} — Ctrl+S to commit");
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
    if let Modal::Annotate { path, buffer } = &mut app.modal {
        match key.code {
            KeyCode::Esc => {
                app.modal = Modal::None;
                app.status = "annotation cancelled".into();
            }
            KeyCode::Enter => {
                let path = path.clone();
                let text = buffer.clone();
                app.annotations.set(&path, &text);
                let outcome = app.annotations.save(&app.project_root);
                app.modal = Modal::None;
                app.status = match outcome {
                    Ok(()) => {
                        if text.trim().is_empty() {
                            format!("annotation cleared for {path}")
                        } else {
                            format!("annotation saved for {path}")
                        }
                    }
                    Err(e) => format!("annotation save FAILED: {e:#}"),
                };
            }
            KeyCode::Backspace => {
                buffer.pop();
            }
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                buffer.push(c);
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
                    Ok(staged) => {
                        app.modal = Modal::None;
                        app.status = format!(
                            "rollback staged {staged} change{} from {} — Ctrl+S to commit",
                            if staged == 1 { "" } else { "s" },
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
                        app.status = format!("deleted backup {}", entry.filename);
                    }
                    Err(e) => {
                        app.status = format!("delete failed: {e:#}");
                    }
                }
                // Refresh the picker.
                match backup::list(&app.project_root) {
                    Ok(entries) if !entries.is_empty() => {
                        app.modal = Modal::Rollback { entries, cursor: 0 };
                    }
                    _ => {
                        app.modal = Modal::None;
                    }
                }
            }
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                // Return to the picker.
                match backup::list(&app.project_root) {
                    Ok(entries) if !entries.is_empty() => {
                        app.modal = Modal::Rollback { entries, cursor: 0 };
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
                // Back to the picker.
                match backup::list(&app.project_root) {
                    Ok(entries) if !entries.is_empty() => {
                        app.modal = Modal::Rollback { entries, cursor: 0 };
                    }
                    _ => {
                        app.modal = Modal::None;
                    }
                }
            }
            KeyCode::Up => {
                *scroll = scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                if *scroll + 1 < total {
                    *scroll += 1;
                }
            }
            KeyCode::PageUp => {
                *scroll = scroll.saturating_sub(20);
            }
            KeyCode::PageDown => {
                *scroll = (*scroll + 20).min(total.saturating_sub(1));
            }
            KeyCode::Home => {
                *scroll = 0;
            }
            KeyCode::End => {
                *scroll = total.saturating_sub(1);
            }
            _ => {}
        }
        return Ok(false);
    }
    if let Modal::SaveConfirm { edits } = &app.modal {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                let edits = edits.clone();
                let outcome = perform_save(app, &edits);
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
    if let Modal::DiscardConfirm { .. } = &app.modal {
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

    // Save chord.
    if key.code == KeyCode::Char('s')
        && key.modifiers.contains(KeyModifiers::CONTROL)
    {
        open_save_confirm(app);
        return Ok(false);
    }

    // Rollback picker.
    if key.code == KeyCode::Char('r')
        && key.modifiers.contains(KeyModifiers::CONTROL)
    {
        open_rollback(app);
        return Ok(false);
    }

    // Comment inspector.
    if key.code == KeyCode::Char('i')
        && key.modifiers.contains(KeyModifiers::CONTROL)
    {
        open_inspector(app);
        return Ok(false);
    }

    // Annotation editor.
    if key.code == KeyCode::Char('a')
        && key.modifiers.contains(KeyModifiers::CONTROL)
    {
        open_annotation_editor(app);
        return Ok(false);
    }

    static_chord_dispatch(app, key);
    Ok(false)
}

fn open_rollback(app: &mut App) {
    match backup::list(&app.project_root) {
        Ok(entries) if !entries.is_empty() => {
            app.modal = Modal::Rollback { entries, cursor: 0 };
        }
        Ok(_) => {
            app.status = format!(
                "rollback: no backups yet · save once to populate {}/.config-backups/",
                app.project_root.display(),
            );
        }
        Err(e) => {
            app.status = format!("rollback list failed: {e:#}");
        }
    }
}

fn open_inspector(app: &mut App) {
    let Some(node) = app.current_node() else {
        return;
    };
    let path = node.path.clone();
    let title = if path.is_empty() {
        "<root>".to_string()
    } else {
        path.clone()
    };
    // Pull comments span from the byte-range index
    // (Phase 2 foundation).
    let comments_text = app.index.as_ref().and_then(|idx| {
        // Prefer leaf comments; fall back to stanza.
        if let Some(leaf) = idx.leaves.get(&path) {
            leaf.leading_comments_range
                .clone()
                .map(|r| idx.source[r].to_string())
        } else if let Some(stanza) = idx.stanzas.get(&path) {
            stanza
                .leading_comments_range
                .clone()
                .map(|r| idx.source[r].to_string())
        } else {
            None
        }
    });
    let annotation = app.annotations.get(&path).map(str::to_owned);
    app.modal = Modal::Inspector {
        title,
        comments: comments_text,
        annotation,
    };
}

fn open_annotation_editor(app: &mut App) {
    let Some(node) = app.current_node() else {
        return;
    };
    let path = node.path.clone();
    if path.is_empty() {
        app.status = "annotation: select a field first".into();
        return;
    }
    let buffer = app
        .annotations
        .get(&path)
        .map(str::to_owned)
        .unwrap_or_default();
    app.modal = Modal::Annotate { path, buffer };
}

fn stage_rollback(app: &mut App, entry: &BackupEntry) -> Result<usize> {
    let raw = backup::read(entry)?;
    let backup_value =
        serde_hjson::from_str::<Value>(&raw).context("parse backup HJSON")?;
    let backup_index = hjson_index::parse(&raw).ok();

    // Walk every leaf in the backup and stage it into
    // the working schema.  Anything in the backup but
    // not in the current schema is reported but not
    // staged (unknown fields can't be edited).
    let mut count: usize = 0;
    stage_from_value(app, "", &backup_value, &mut count);
    // Refresh original-source comparisons: backup's
    // own index becomes the basis for the next save
    // diff so unchanged-from-backup leaves don't show
    // as "changed" against the live file.
    if let Some(idx) = backup_index {
        app.index = Some(idx);
    }
    Ok(count)
}

fn stage_from_value(app: &mut App, prefix: &str, value: &Value, count: &mut usize) {
    if let Value::Object(map) = value {
        for (key, child) in map {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            stage_from_value(app, &path, child, count);
        }
        return;
    }
    // Scalar / array leaf — stage if the schema knows
    // the path.
    if find_mut(&mut app.schema, prefix).is_some() {
        app.stage(prefix, value.clone());
        *count += 1;
    }
}

fn static_chord_dispatch(app: &mut App, key: KeyEvent) {
    use KeyCode::*;
    match key.code {
        Up => {
            if app.cursor > 0 {
                app.cursor -= 1;
            }
        }
        Down => {
            let n = app.rows().len();
            if app.cursor + 1 < n {
                app.cursor += 1;
            }
        }
        PageUp => {
            app.cursor = app.cursor.saturating_sub(10);
        }
        PageDown => {
            let n = app.rows().len();
            app.cursor = (app.cursor + 10).min(n.saturating_sub(1));
        }
        Home => {
            app.cursor = 0;
        }
        End => {
            let n = app.rows().len();
            app.cursor = n.saturating_sub(1);
        }
        Enter | Char(' ') => {
            // For stanzas: expand/collapse.  For
            // leaves: open the widget.
            let (is_leaf, path, current, ty_label) = match app.current_node() {
                Some(n) => (
                    n.is_leaf(),
                    n.path.clone(),
                    n.current.clone(),
                    n.ty.label().to_string(),
                ),
                None => return,
            };
            if is_leaf {
                let ty = app
                    .current_node()
                    .map(|n| n.ty.clone())
                    .unwrap_or(crate::config_tui::schema::ConfigType::String);
                let widget =
                    Widget::start_for_typed(&current, &ty, &ty_label, &path);
                app.modal = Modal::Edit { path, widget };
            } else if let Some(node) = app.current_node() {
                let p = node.path.clone();
                if app.collapsed.contains(&p) {
                    app.collapsed.remove(&p);
                } else {
                    app.collapsed.insert(p);
                }
            }
        }
        Char('e') => {
            // Explicit "edit leaf" — same as Enter on
            // a leaf but unambiguous when the user is
            // mid-navigation.
            if let Some(node) = app.current_node() {
                if node.is_leaf() {
                    let widget = Widget::start_for_typed(
                        &node.current,
                        &node.ty,
                        node.ty.label(),
                        &node.path,
                    );
                    let path = node.path.clone();
                    app.modal = Modal::Edit { path, widget };
                }
            }
        }
        Char('r') => {
            // Reset selected leaf to its default.
            let (path, default) = match app.current_node() {
                Some(n) if n.is_leaf() => (n.path.clone(), n.default.clone()),
                _ => return,
            };
            app.stage(&path, default);
            app.status = format!("reset {path} to default");
        }
        Char('a') => {
            // Add map entry — focused node must be a
            // known map stanza.
            let parent = match app.current_node() {
                Some(n) if schema::is_known_map_path(&n.path) => n.path.clone(),
                Some(n) if !n.path.is_empty()
                    && schema::is_known_map_path(parent_path_str(&n.path)) =>
                {
                    parent_path_str(&n.path).to_string()
                }
                _ => {
                    app.status = "add: focus a map-shaped stanza (e.g. `llm.providers`)".into();
                    return;
                }
            };
            app.modal = Modal::AddMapEntry {
                parent,
                buffer: String::new(),
                error: None,
            };
        }
        Char('d') => {
            // Delete map entry — focused node must
            // be a map entry (direct child of a known
            // map path).
            let entry_path = match app.current_node() {
                Some(n) if !n.path.is_empty()
                    && schema::is_known_map_path(parent_path_str(&n.path)) =>
                {
                    n.path.clone()
                }
                _ => {
                    app.status =
                        "delete: focus an entry of a map-shaped stanza".into();
                    return;
                }
            };
            app.modal = Modal::DeleteMapEntryConfirm { path: entry_path };
        }
        Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            open_help(app);
        }
        Char('?') => {
            open_help(app);
        }
        _ => {}
    }
}

fn open_help(app: &mut App) {
    let Some(node) = app.current_node() else { return; };
    let mut body = String::new();

    // ── Section 1: structural metadata.  Always
    // present — the field's path, type, default value,
    // current value (when divergent), and where it
    // came from.  Useful even when the user hasn't
    // documented the field in CONFIGURATION.md.
    body.push_str(" Structural\n");
    if node.path.is_empty() {
        body.push_str("   path     <root>\n");
    } else {
        body.push_str(&format!("   path     {}\n", node.path));
    }
    body.push_str(&format!("   type     {}\n", node.ty.label()));
    body.push_str(&format!(
        "   default  {}\n",
        help_value_summary(&node.default),
    ));
    if node.current != node.default {
        body.push_str(&format!(
            "   current  {}\n",
            help_value_summary(&node.current),
        ));
    }
    body.push_str(&format!(
        "   source   {}\n",
        source_label(node.source),
    ));

    // ── Section 2: contextual hints.  Map semantics,
    // staging state, and any user annotation get
    // their own bullet so the writer sees them
    // without paging into the docs section below.
    let mut hints: Vec<String> = Vec::new();
    if schema::is_known_map_path(&node.path) {
        hints.push(
            "this is a map of named entries; `a` to add a new one, `d` to remove the selected one"
                .into(),
        );
    } else if !node.path.is_empty()
        && schema::is_known_map_path(parent_path_str(&node.path))
    {
        hints.push(
            "this is a map entry; press `d` on this row to stage its deletion"
                .into(),
        );
    }
    if app.added_map_entries.contains(&node.path) {
        hints.push("STAGED — new map entry (Ctrl+S to write)".into());
    }
    if app.removed_map_entries.contains(&node.path) {
        hints.push("STAGED — deletion (Ctrl+S to write)".into());
    } else if app.changed_paths.contains(&node.path) && !app.added_map_entries.contains(&node.path) {
        hints.push("STAGED — value edit (Ctrl+S to write)".into());
    }
    if let Some(ann) = app.annotations.get(&node.path) {
        hints.push(format!("annotation: {ann}"));
    }
    if !hints.is_empty() {
        body.push('\n');
        body.push_str(" Notes\n");
        for h in &hints {
            body.push_str("   • ");
            body.push_str(h);
            body.push('\n');
        }
    }

    // ── Section 3: CONFIGURATION.md row.  When the
    // path matches a documented row, the row body
    // lands here verbatim.  When it doesn't, we
    // surface a clear path-forward instruction.
    body.push('\n');
    body.push_str(" Documentation\n");
    match app.help.lookup(&node.path) {
        Some(text) if !text.trim().is_empty() => {
            for line in text.lines() {
                body.push_str("   ");
                body.push_str(line);
                body.push('\n');
            }
        }
        _ => {
            body.push_str(
                "   No CONFIGURATION.md row matched this path yet.\n",
            );
            body.push_str(
                "   The structural section above is always available.\n",
            );
            body.push_str(
                "   To enrich this help pane, add a row to\n",
            );
            body.push_str(
                "   `Documentation/CONFIGURATION.md` keyed by this field's path.\n",
            );
        }
    }

    app.modal = Modal::Help { body };
}

/// Compact one-line render of a JSON value for the
/// help pane's `default` / `current` summary.  Long
/// objects / arrays get a shape label rather than a
/// dump — the user can browse them via the tree.
fn help_value_summary(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            let mut chars: Vec<char> = s.chars().collect();
            if chars.len() > 80 {
                chars.truncate(79);
                let mut out: String = chars.into_iter().collect();
                out.push('…');
                format!("\"{out}\"")
            } else {
                format!("\"{s}\"")
            }
        }
        Value::Array(arr) => {
            if arr.is_empty() {
                "[]".into()
            } else if arr.iter().all(|v| v.is_string()) {
                format!("[{} strings]", arr.len())
            } else {
                format!("[{} items]", arr.len())
            }
        }
        Value::Object(map) => {
            if map.is_empty() {
                "{}".into()
            } else {
                format!("{{ {} fields }}", map.len())
            }
        }
    }
}

fn open_save_confirm(app: &mut App) {
    if app.changed_paths.is_empty()
        && app.added_map_entries.is_empty()
        && app.removed_map_entries.is_empty()
    {
        app.status = "nothing to save".into();
        return;
    }
    let edits = compute_all_edits(app);
    if edits.is_empty() {
        app.status = "nothing to save (no diff)".into();
        return;
    }
    app.modal = Modal::SaveConfirm { edits };
}

/// Compose all four edit kinds: structural deletes
/// + structural adds + scalar splice/append (only
/// for paths NOT already covered by a map-entry
/// add/delete).
fn compute_all_edits(app: &mut App) -> Vec<Edit> {
    let mut out: Vec<Edit> = Vec::new();
    // Map-entry deletions first.
    for path in &app.removed_map_entries {
        out.push(Edit {
            path: path.clone(),
            new_value: Value::Null,
            kind: EditKind::DeleteMapEntry,
        });
    }
    // Map-entry additions next.  Each addition
    // carries the full stanza body (the schema tree's
    // `current` at that path).
    for path in &app.added_map_entries {
        if let Some(node) = find_in_tree(&app.schema, path) {
            out.push(Edit {
                path: path.clone(),
                new_value: collect_subtree_value(node),
                kind: EditKind::AddMapEntry,
            });
        }
    }
    // Scalar diffs — but skip any leaf living under
    // a staged add or delete (those are handled by
    // the structural edits above).
    let index_source = match app.index.as_ref() {
        Some(i) => i.clone(),
        None => match hjson_index::parse("{}") {
            Ok(idx) => idx,
            Err(_) => return out,
        },
    };
    let leaf_edits = save::compute_edits(&app.schema, &index_source);
    for edit in leaf_edits {
        if app
            .added_map_entries
            .iter()
            .any(|p| edit.path == *p || edit.path.starts_with(&format!("{p}.")))
        {
            continue;
        }
        if app
            .removed_map_entries
            .iter()
            .any(|p| edit.path == *p || edit.path.starts_with(&format!("{p}.")))
        {
            continue;
        }
        out.push(edit);
    }
    out
}

/// Walk a SchemaNode and rebuild its JSON shape from
/// the `current` values.  Used to gather an added
/// map entry's full stanza body for the
/// `AddMapEntry` edit.
fn collect_subtree_value(node: &SchemaNode) -> Value {
    if node.is_leaf() {
        return node.current.clone();
    }
    let mut map = serde_json::Map::new();
    for child in &node.children {
        map.insert(child.display.clone(), collect_subtree_value(child));
    }
    Value::Object(map)
}

fn find_in_tree<'a>(node: &'a SchemaNode, path: &str) -> Option<&'a SchemaNode> {
    if node.path == path {
        return Some(node);
    }
    for child in &node.children {
        if !child.path.is_empty()
            && (path.starts_with(&child.path)
                || child.path.starts_with(path))
        {
            if let Some(found) = find_in_tree(child, path) {
                return Some(found);
            }
        }
    }
    None
}

fn remove_subtree(node: &mut SchemaNode, target: &str) {
    let target_parent = parent_path_str(target);
    if node.path == target_parent {
        node.children.retain(|c| c.path != target);
        return;
    }
    for child in &mut node.children {
        if !child.path.is_empty() && target.starts_with(&child.path) {
            remove_subtree(child, target);
        }
    }
}

fn parent_path_str(path: &str) -> &str {
    match path.rfind('.') {
        Some(idx) => &path[..idx],
        None => "",
    }
}

/// Reserved — leaf-key helper kept for future use
/// (e.g. surfacing the entry name in confirm modals
/// without re-parsing the dotted path on the
/// renderer side).
#[allow(dead_code)]
fn leaf_key_str(path: &str) -> &str {
    match path.rfind('.') {
        Some(idx) => &path[idx + 1..],
        None => path,
    }
}

fn is_valid_map_entry_name(name: &str) -> bool {
    let mut chars = name.chars();
    let first = match chars.next() {
        Some(c) => c,
        None => return false,
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Stage a new map entry: build its template stanza
/// from any existing default + insert a SchemaNode
/// into the tree so the user can immediately edit
/// its fields.  Marks every leaf in the new entry
/// as "changed" (vs the disk file, where the entry
/// doesn't exist).
fn stage_new_map_entry(
    app: &mut App,
    parent_path: &str,
    name: &str,
) -> Result<()> {
    let new_path = format!("{parent_path}.{name}");
    // Find the parent stanza in the tree.
    let parent_node = find_in_tree(&app.schema, parent_path)
        .ok_or_else(|| anyhow::anyhow!("parent stanza `{parent_path}` not in tree"))?;
    // Use the first existing child as the template
    // for the new entry's structure.
    let template = parent_node
        .children
        .iter()
        .find(|c| !c.path.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "cannot add new entry to `{parent_path}` — no existing entry to use as template"
            )
        })?
        .clone();
    // Rebuild the template as a fresh subtree under
    // the new path.  Default values come from the
    // template's `default`; `current` initialises to
    // the same.
    let new_node = clone_template(&template, &new_path, name);
    // Insert into the parent.
    let parent_node_mut = find_mut(&mut app.schema, parent_path)
        .ok_or_else(|| anyhow::anyhow!("parent stanza `{parent_path}` vanished mid-add"))?;
    parent_node_mut.children.push(new_node);
    parent_node_mut
        .children
        .sort_by(|a, b| a.display.cmp(&b.display));
    parent_node_mut.source = ValueSource::Configured;
    app.added_map_entries.insert(new_path.clone());
    // Mark every leaf in the new entry as changed so
    // the save pipeline sees structural work pending.
    // (Even default-valued leaves still need
    // emitting since the entry itself is new.)
    app.changed_paths.insert(new_path);
    Ok(())
}

/// Recursively clone a template SchemaNode under a
/// new path prefix + entry name.  Re-uses the
/// template's defaults but sets `source` to
/// `Configured` (the entry is user-added).
fn clone_template(template: &SchemaNode, new_path: &str, display: &str) -> SchemaNode {
    if template.is_leaf() {
        return SchemaNode {
            path: new_path.to_string(),
            display: display.to_string(),
            ty: template.ty.clone(),
            default: template.default.clone(),
            current: template.default.clone(),
            source: ValueSource::Configured,
            children: Vec::new(),
        };
    }
    let mut children = Vec::with_capacity(template.children.len());
    for child in &template.children {
        let child_path = format!("{new_path}.{}", child.display);
        children.push(clone_template(child, &child_path, &child.display));
    }
    SchemaNode {
        path: new_path.to_string(),
        display: display.to_string(),
        ty: template.ty.clone(),
        default: template.default.clone(),
        current: template.current.clone(),
        source: ValueSource::Configured,
        children,
    }
}

fn perform_save(app: &mut App, edits: &[Edit]) -> Result<String> {
    // If there's no file yet, write a minimal `{}`
    // wrapper first so the surgical-splice pipeline has
    // something to operate on.
    let working_source: String = match app.index.as_ref() {
        Some(idx) => idx.source.clone(),
        None => "{\n}\n".to_string(),
    };
    let working_index = hjson_index::parse(&working_source)
        .context("re-parse working source")?;
    // Re-derive the edit list against the working
    // index (the empty-file case re-classifies splices
    // as appends; map-entry adds/deletes also depend
    // on the freshly-parsed index).  Caller's `edits`
    // is now an informational preview only — the
    // authoritative plan re-derives here.
    let _ = edits;
    let prior_app_index = app.index.clone();
    if app.index.is_none() {
        app.index = Some(working_index.clone());
    }
    let edits = compute_all_edits(app);
    if prior_app_index.is_none() {
        app.index = None;
    } else {
        app.index = prior_app_index;
    }
    let new_source = save::apply_edits(&working_index, &edits)?;
    let written = save::write_atomic(&app.cfg_path, &new_source)?;
    let backup = save::write_backup(&app.project_root, &new_source)?;
    app.saved_at_least_once = true;

    // Refresh load state so subsequent saves diff
    // against the new on-disk values.
    let parsed_value = serde_hjson::from_str::<Value>(&new_source)
        .unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
    let defaults_value =
        serde_json::to_value(Config::default()).unwrap_or(Value::Null);
    let (schema, unknowns) = schema::build(&defaults_value, &parsed_value);
    app.schema = schema;
    app.unknowns = unknowns;
    let new_index = hjson_index::parse(&new_source).ok();
    app.index = new_index;
    let mut original_by_path: HashMap<String, Value> = HashMap::new();
    snapshot_originals(&app.schema, &mut original_by_path);
    app.original_by_path = original_by_path;
    app.changed_paths.clear();
    app.added_map_entries.clear();
    app.removed_map_entries.clear();
    app.status = format!(
        "saved {} · backup {}",
        written.display(),
        backup
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default(),
    );
    Ok(format!(
        "Saved {} edits to {}.\nBackup: {}\n\nRESTART REQUIRED — relaunch inkhaven to pick up the new values.",
        edits.len(),
        written.display(),
        backup.display(),
    ))
}

// ── render ────────────────────────────────────────────

fn render(f: &mut ratatui::Frame, app: &mut App) {
    let size = f.area();
    let v_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(size);
    draw_top_bar(f, v_chunks[0], app);
    draw_body(f, v_chunks[1], app);
    draw_status(f, v_chunks[2], app);

    match &app.modal {
        Modal::None => {}
        Modal::Help { body } => draw_help_modal(f, size, body),
        Modal::Edit { path, widget } => {
            draw_edit_modal(f, size, path, widget);
        }
        Modal::SaveConfirm { edits } => {
            draw_save_confirm(f, size, &app.cfg_path, edits);
        }
        Modal::Saved { message } => draw_saved_overlay(f, size, message),
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
        Modal::Inspector {
            title,
            comments,
            annotation,
        } => {
            draw_inspector(f, size, title, comments.as_deref(), annotation.as_deref());
        }
        Modal::Annotate { path, buffer } => {
            draw_annotation_editor(f, size, path, buffer);
        }
        Modal::AddMapEntry { parent, buffer, error } => {
            draw_add_map_entry(f, size, parent, buffer, error.as_deref());
        }
        Modal::DeleteMapEntryConfirm { path } => {
            draw_delete_map_entry_confirm(f, size, path);
        }
    }
}

fn draw_top_bar(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let mut spans: Vec<Span<'_>> = Vec::new();
    spans.push(Span::styled(
        " inkhaven config ",
        Style::default()
            .bg(Color::Cyan)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        format!("{}", app.cfg_path.display()),
        Style::default().add_modifier(Modifier::BOLD),
    ));
    if !app.changed_paths.is_empty() {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!(" {} changed ", app.changed_paths.len()),
            Style::default()
                .bg(Color::Red)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if !app.unknowns.is_empty() {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!(" {} unknown ", app.unknowns.len()),
            Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if app.saved_at_least_once {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            " restart required ",
            Style::default()
                .bg(Color::Magenta)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_body(f: &mut ratatui::Frame, area: Rect, app: &mut App) {
    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);
    draw_tree_pane(f, h_chunks[0], app);
    draw_detail_pane(f, h_chunks[1], app);
}

fn draw_tree_pane(f: &mut ratatui::Frame, area: Rect, app: &mut App) {
    let inner_h = area.height.saturating_sub(2) as usize;
    if app.cursor < app.scroll {
        app.scroll = app.cursor;
    } else if inner_h > 0 && app.cursor >= app.scroll + inner_h {
        app.scroll = app.cursor + 1 - inner_h;
    }
    let rows = app.rows();
    let total = rows.len();
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Config tree ({total}) "));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line<'_>> = Vec::with_capacity(inner_h);
    for (i, (depth, node)) in rows.iter().enumerate().skip(app.scroll).take(inner_h) {
        let glyph = if node.is_leaf() {
            "  "
        } else if app.collapsed.contains(&node.path) {
            "▸ "
        } else {
            "▾ "
        };
        let indent = "  ".repeat(*depth);
        let selected = i == app.cursor;
        let changed = app.changed_paths.contains(&node.path);
        let annotated = app.annotations.get(&node.path).is_some();
        let added = app.added_map_entries.contains(&node.path);
        let removed = app.removed_map_entries.contains(&node.path);
        // Two-character chip: state + annotation
        // marker.  Stage / add / remove / configured
        // win the first slot in that order.
        let state = if removed {
            "✗"
        } else if added {
            "✚"
        } else if changed {
            "✱"
        } else if node.is_leaf() && node.source == ValueSource::Configured {
            "●"
        } else {
            " "
        };
        let ann = if annotated { "+" } else { " " };
        let chip = format!("{state}{ann}");
        let style = if selected {
            Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        } else if removed {
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::CROSSED_OUT | Modifier::DIM)
        } else if added {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else if changed {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else if node.source == ValueSource::Configured {
            Style::default().fg(Color::Green)
        } else {
            Style::default().add_modifier(Modifier::DIM)
        };
        lines.push(Line::from(Span::styled(
            format!("{indent}{glyph}{chip}{}", node.display),
            style,
        )));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_detail_pane(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let block = Block::default().borders(Borders::ALL).title(" Detail ");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let Some(node) = app.current_node() else {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  (empty tree)",
                Style::default().add_modifier(Modifier::DIM),
            ))),
            inner,
        );
        return;
    };

    let dim = Style::default().add_modifier(Modifier::DIM);
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let mut lines: Vec<Line<'_>> = Vec::new();

    lines.push(Line::from(vec![
        Span::styled(" path ", dim),
        Span::styled(format!(" {}", node.path.as_str()), bold),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" type ", dim),
        Span::raw(format!(" {}", node.ty.label())),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" source ", dim),
        Span::raw(format!(" {}", source_label(node.source))),
    ]));
    if app.changed_paths.contains(&node.path) {
        lines.push(Line::from(Span::styled(
            "  ✱ STAGED — unsaved",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
    }
    lines.push(Line::from(""));

    if node.is_leaf() {
        lines.push(Line::from(Span::styled(" current value:", bold)));
        for chunk in pretty(&node.current) {
            lines.push(Line::from(Span::raw(format!("   {chunk}"))));
        }
        if node.current != node.default {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(" default value:", dim)));
            for chunk in pretty(&node.default) {
                lines.push(Line::from(Span::styled(
                    format!("   {chunk}"),
                    dim,
                )));
            }
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " Enter / e to edit · r to reset to default ",
            Style::default().fg(Color::Cyan),
        )));
    } else {
        if schema::is_known_map_path(&node.path) {
            lines.push(Line::from(Span::styled(
                format!(
                    " map of named entries · {} entr{}:",
                    node.children.len(),
                    if node.children.len() == 1 { "y" } else { "ies" },
                ),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::styled(
                "   user-added entries in the live HJSON appear here too",
                dim,
            )));
            lines.push(Line::from(Span::styled(
                "   (add / delete via Ctrl+B 0 HJSON editor for now)",
                dim,
            )));
            lines.push(Line::from(""));
        } else {
            lines.push(Line::from(Span::styled(
                format!(" {} children:", node.children.len()),
                bold,
            )));
        }
        for child in &node.children {
            let chip = match child.source {
                ValueSource::Configured if child.is_leaf() => "●",
                _ => " ",
            };
            lines.push(Line::from(vec![
                Span::raw("   "),
                Span::styled(chip, Style::default().fg(Color::Green)),
                Span::raw(" "),
                Span::raw(child.display.clone()),
                Span::styled(
                    format!("   ({})", child.ty.label()),
                    dim,
                ),
            ]));
        }
    }

    let unknown_for_path: Vec<&(String, Value)> = app
        .unknowns
        .iter()
        .filter(|(p, _)| p.starts_with(&node.path) && !node.path.is_empty())
        .collect();
    if !unknown_for_path.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(
                " {} unknown sub-fields preserved as-is:",
                unknown_for_path.len()
            ),
            Style::default().fg(Color::Yellow),
        )));
        for (p, v) in unknown_for_path.iter().take(10) {
            lines.push(Line::from(Span::styled(
                format!("   {p} = {}", trim_value(v, 60)),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::DIM),
            )));
        }
    }

    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn draw_status(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let n = app.rows().len();
    let pos = format!("{} / {}", app.cursor + 1, n.max(1));
    let hints = " ↑↓ · Enter · e edit · r reset · a add-entry · d del-entry · Ctrl+S save · Ctrl+R rollback · Ctrl+I inspect · Ctrl+A annotate · ? help · Esc";
    let dim = Style::default().add_modifier(Modifier::DIM);
    let spans = vec![
        Span::styled(format!(" {pos} "), dim),
        Span::raw("  "),
        Span::raw(app.status.clone()),
        Span::raw("   "),
        Span::styled(hints, dim),
    ];
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_help_modal(f: &mut ratatui::Frame, host: Rect, body: &str) {
    let w = host.width.saturating_sub(8).min(96);
    let h = host.height.saturating_sub(4).min(32);
    let x = host.x + host.width.saturating_sub(w) / 2;
    let y = host.y + host.height.saturating_sub(h) / 2;
    let rect = Rect { x, y, width: w, height: h };
    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help — config field ")
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let dim = Style::default().add_modifier(Modifier::DIM);
    // The structural body uses ` Title\n` lines as
    // section headers and `   key  value\n` lines for
    // body content.  Boldify the headers (lines that
    // start with a space + uppercase letter and end
    // with no trailing colon) so the at-a-glance read
    // works.  Lines starting with `   •` get a cyan
    // bullet.  Everything else renders plain.
    let mut lines: Vec<Line<'_>> = Vec::new();
    for raw in body.lines() {
        let trimmed = raw.trim_start();
        // Section header: starts with a single space,
        // first non-space is uppercase, no leading
        // bullet.
        if raw.starts_with(' ')
            && !raw.starts_with("  ")
            && trimmed
                .chars()
                .next()
                .map(|c| c.is_ascii_uppercase())
                .unwrap_or(false)
        {
            lines.push(Line::from(Span::styled(
                raw.to_string(),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )));
        } else if trimmed.starts_with("• ") {
            lines.push(Line::from(Span::raw(raw.to_string())));
        } else if let Some((k, v)) = split_kv_line(raw) {
            lines.push(Line::from(vec![
                Span::styled(k, bold),
                Span::raw("  "),
                Span::raw(v),
            ]));
        } else {
            lines.push(Line::from(Span::raw(raw.to_string())));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " any key closes ",
        dim,
    )));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

/// Helper for the help modal: split a 3-space-indent
/// `   key  value` line into `(key, value)` so the
/// renderer can bold the key.  Returns `None` when
/// the line doesn't match the shape.
fn split_kv_line(line: &str) -> Option<(String, String)> {
    if !line.starts_with("   ") {
        return None;
    }
    let body = &line[3..];
    // Two-or-more consecutive spaces separate key
    // from value.
    let mut idx = None;
    let bytes = body.as_bytes();
    for i in 0..bytes.len().saturating_sub(1) {
        if bytes[i] == b' ' && bytes[i + 1] == b' ' {
            idx = Some(i);
            break;
        }
    }
    let i = idx?;
    let key = body[..i].to_string();
    let value = body[i..].trim_start().to_string();
    if key.is_empty() {
        return None;
    }
    Some((format!("   {key}"), value))
}

fn draw_edit_modal(
    f: &mut ratatui::Frame,
    host: Rect,
    path: &str,
    widget: &Widget,
) {
    let w = host.width.saturating_sub(8).min(80);
    // List + Color + Enum need extra vertical space
    // for their richer content; everything else fits
    // in 9 rows.
    let h: u16 = match widget {
        Widget::List(_) => 16,
        Widget::Color(_) => 14,
        Widget::Enum(_) => 12,
        _ => 9,
    };
    let x = host.x + host.width.saturating_sub(w) / 2;
    let y = host.y + host.height.saturating_sub(h) / 2;
    let rect = Rect { x, y, width: w, height: h };
    f.render_widget(ratatui::widgets::Clear, rect);
    widget.render(f, rect, path);
}

fn draw_save_confirm(
    f: &mut ratatui::Frame,
    host: Rect,
    path: &Path,
    edits: &[Edit],
) {
    let max_rows = edits.len().min(15).max(1) as u16;
    let w = host.width.saturating_sub(8).min(96);
    let h = (max_rows + 7).min(host.height.saturating_sub(4));
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
            Span::styled(format!("{}", edits.len()), bold),
            Span::raw(" change"),
            Span::raw(if edits.len() == 1 { "" } else { "s" }),
            Span::raw(" to "),
            Span::styled(format!("{}", path.display()), bold),
            Span::raw("?"),
        ]),
        Line::from(""),
    ];
    for edit in edits.iter().take(15) {
        let kind = match edit.kind {
            EditKind::Splice => "splice",
            EditKind::Append => "append",
            EditKind::AddMapEntry => "+entry",
            EditKind::DeleteMapEntry => "-entry",
        };
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(format!("{kind:>6}"), dim),
            Span::raw("  "),
            Span::styled(edit.path.clone(), bold),
            Span::raw(" = "),
            Span::raw(trim_value(&edit.new_value, 60)),
        ]));
    }
    if edits.len() > 15 {
        lines.push(Line::from(Span::styled(
            format!("    … and {} more", edits.len() - 15),
            dim,
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "    A timestamped backup will be written to <project>/.config-backups/",
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
    let h: u16 = 10;
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
                format!("{unsaved} staged edit{}", if unsaved == 1 { "" } else { "s" }),
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

// ── Phase 3 modal painters ────────────────────────────

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
        .title(format!(" Config rollback — {} backups ", entries.len()))
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
    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("    "),
            Span::styled(entry.filename.clone(), Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "    This cannot be undone.",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "    y / Enter delete · n / Esc cancel",
            Style::default().add_modifier(Modifier::DIM),
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
    let total = body.lines().count();
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

fn draw_inspector(
    f: &mut ratatui::Frame,
    host: Rect,
    title: &str,
    comments: Option<&str>,
    annotation: Option<&str>,
) {
    let w = host.width.saturating_sub(8).min(96);
    let h = host.height.saturating_sub(4).min(28);
    let x = host.x + host.width.saturating_sub(w) / 2;
    let y = host.y + host.height.saturating_sub(h) / 2;
    let rect = Rect { x, y, width: w, height: h };
    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Inspector — {title} "))
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let dim = Style::default().add_modifier(Modifier::DIM);
    let mut lines: Vec<Line<'_>> = Vec::new();
    lines.push(Line::from(Span::styled(" Comments in inkhaven.hjson:", bold)));
    match comments {
        Some(text) if !text.trim().is_empty() => {
            for line in text.lines() {
                lines.push(Line::from(Span::raw(format!("   {line}"))));
            }
        }
        _ => {
            lines.push(Line::from(Span::styled(
                "   (no comments attached to this field)",
                dim,
            )));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(" Annotation:", bold)));
    match annotation {
        Some(text) if !text.trim().is_empty() => {
            for line in text.lines() {
                lines.push(Line::from(Span::raw(format!("   {line}"))));
            }
        }
        _ => {
            lines.push(Line::from(Span::styled(
                "   (no annotation — Ctrl+A to add one)",
                dim,
            )));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " any key closes ",
        dim,
    )));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn draw_add_map_entry(
    f: &mut ratatui::Frame,
    host: Rect,
    parent: &str,
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
        .title(format!(" Add map entry to {parent} "))
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
        Line::from(Span::styled(
            "    Name for the new entry:",
            bold,
        )),
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
            "    Identifier: letter or `_` to start, then letters / digits / `_`",
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

fn draw_delete_map_entry_confirm(
    f: &mut ratatui::Frame,
    host: Rect,
    path: &str,
) {
    let w = host.width.saturating_sub(8).min(72);
    let h: u16 = 9;
    let x = host.x + host.width.saturating_sub(w) / 2;
    let y = host.y + host.height.saturating_sub(h) / 2;
    let rect = Rect { x, y, width: w, height: h };
    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Delete map entry? ")
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
            Span::raw("    Stage deletion of "),
            Span::styled(path.to_string(), bold),
            Span::raw(" ?"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "    The entry stays in the tree (struck-through) until you Ctrl+S.",
            dim,
        )),
        Line::from(""),
        Line::from(Span::styled(
            "    y / Enter confirm · n / Esc cancel",
            dim,
        )),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_annotation_editor(
    f: &mut ratatui::Frame,
    host: Rect,
    path: &str,
    buffer: &str,
) {
    let w = host.width.saturating_sub(8).min(96);
    let h: u16 = 8;
    let x = host.x + host.width.saturating_sub(w) / 2;
    let y = host.y + host.height.saturating_sub(h) / 2;
    let rect = Rect { x, y, width: w, height: h };
    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Annotate — {path} "))
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let dim = Style::default().add_modifier(Modifier::DIM);
    let lines = vec![
        Line::from(""),
        Line::from(format!("    {buffer}│")),
        Line::from(""),
        Line::from(Span::styled(
            "    Free-text note attached to this field.  Empty input clears.",
            dim,
        )),
        Line::from(Span::styled(
            "    Enter saves · Esc cancels",
            dim,
        )),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

// ── helpers ───────────────────────────────────────────

fn source_label(s: ValueSource) -> &'static str {
    match s {
        ValueSource::Default => "default",
        ValueSource::Configured => "configured (in HJSON)",
        ValueSource::Unknown => "unknown (not in schema)",
    }
}

fn pretty(value: &Value) -> Vec<String> {
    match value {
        Value::Array(arr) if arr.is_empty() => vec!["[]".into()],
        Value::Array(arr) => arr
            .iter()
            .map(|v| format!("- {}", trim_value(v, 80)))
            .collect(),
        Value::Object(_) => {
            vec![serde_json::to_string_pretty(value).unwrap_or_default()]
        }
        v => vec![trim_value(v, 80)],
    }
}

fn trim_value(value: &Value, max_chars: usize) -> String {
    let s = match value {
        Value::String(s) => s.clone(),
        v => v.to_string(),
    };
    let mut chars: Vec<char> = s.chars().collect();
    if chars.len() > max_chars {
        chars.truncate(max_chars.saturating_sub(1));
        let mut out: String = chars.into_iter().collect();
        out.push('…');
        out
    } else {
        s
    }
}

// Unused-but-kept import (compile-time anchor for the
// Phase 1 schema/help imports + Phase 2's ConfigType
// label lookup).
#[allow(dead_code)]
const _: fn() = || {
    let _ = ConfigType::Bool.label();
};
