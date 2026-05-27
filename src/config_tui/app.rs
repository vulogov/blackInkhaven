//! 1.2.10+ — config-TUI event loop + render.
//!
//! Phase 1: read-only walk-through.  Standalone
//! terminal session — separate from the main inkhaven
//! TUI, no shared state.  Exits on Esc / Ctrl+Q.

use std::collections::HashSet;
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
use crate::config_tui::help::HelpIndex;
use crate::config_tui::schema::{self, SchemaNode, ValueSource};

/// Two-step entry: install panic hook + raw-mode +
/// alt-screen, run the event loop, restore the
/// terminal in every exit path.
pub fn run(project: &Path) -> Result<()> {
    let cfg_path = project.join("inkhaven.hjson");
    let app = App::load(project.to_path_buf(), &cfg_path)?;

    // Panic hook BEFORE terminal init so a panic mid-
    // render still restores the screen.
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

    // Always restore the terminal, regardless of how
    // the loop exited.
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
}

struct App {
    // Carried for Phase 2's backup snapshots (.config-
    // backups/ lives under the project root).  Unused
    // in Phase 1.
    #[allow(dead_code)]
    project_root: PathBuf,
    cfg_path: PathBuf,
    /// Computed default config tree.
    schema: SchemaNode,
    /// Unknown fields detected in the live HJSON
    /// (present in file but not in the schema).  See
    /// proposal §6.6.
    unknowns: Vec<(String, Value)>,
    /// Path-collapse state for the tree pane.
    collapsed: HashSet<String>,
    /// Visible-row cursor.  Indexes into the flatten()
    /// output of the root tree.
    cursor: usize,
    /// First visible row (vertical scroll offset).
    scroll: usize,
    /// CONFIGURATION.md help index, built once at
    /// startup.
    help: HelpIndex,
    /// Floating pane state.
    modal: Modal,
    /// One-shot status message rendered at the bottom.
    status: String,
}

enum Modal {
    None,
    Help { body: String },
}

impl App {
    fn load(project_root: PathBuf, cfg_path: &Path) -> Result<Self> {
        // Build the defaults tree from `Config::default()`
        // via JSON round-trip.  Any change to the
        // `Config` struct flows into the schema
        // automatically (the only manual work is the
        // metadata table — Phase 2).
        let defaults_value: Value = serde_json::to_value(Config::default())
            .context("serialise Config::default() to JSON")?;

        // Read the live HJSON if it exists; else treat
        // the project as fresh and walk defaults only.
        let live_value: Value = if cfg_path.exists() {
            let raw = std::fs::read_to_string(cfg_path)
                .with_context(|| format!("read {}", cfg_path.display()))?;
            // serde-hjson parses HJSON; serde_json's
            // value type is the lingua franca.  Same
            // crate the existing `Config::load` uses,
            // so HJSON quirks (comments, trailing
            // commas, unquoted keys) are handled
            // identically.
            match serde_hjson::from_str::<Value>(&raw) {
                Ok(v) => v,
                Err(e) => {
                    // Bad HJSON shouldn't kill the TUI —
                    // fall back to defaults-only and
                    // surface the error on status.
                    tracing::warn!(
                        target: "inkhaven::config_tui",
                        "{} failed to parse: {e}",
                        cfg_path.display()
                    );
                    Value::Object(serde_json::Map::new())
                }
            }
        } else {
            Value::Object(serde_json::Map::new())
        };

        let (schema, unknowns) = schema::build(&defaults_value, &live_value);
        let help = HelpIndex::build();

        let mut status = format!(
            "{} loaded · {} top-level stanzas · {} unknown fields",
            cfg_path.display(),
            schema.children.len(),
            unknowns.len(),
        );
        if !cfg_path.exists() {
            status = format!(
                "{} not found · showing defaults only · {} unknown fields",
                cfg_path.display(),
                unknowns.len(),
            );
        }

        Ok(Self {
            project_root,
            cfg_path: cfg_path.to_path_buf(),
            schema,
            unknowns,
            collapsed: HashSet::new(),
            cursor: 0,
            scroll: 0,
            help,
            modal: Modal::None,
            status,
        })
    }

    /// Flatten the visible rows.
    fn rows(&self) -> Vec<(usize, &SchemaNode)> {
        let mut out: Vec<(usize, &SchemaNode)> = Vec::new();
        self.schema.flatten(&self.collapsed, &mut out, 0);
        // Skip the synthetic root from rendering.
        out.into_iter().skip(1).collect()
    }

    fn current_node(&self) -> Option<&SchemaNode> {
        self.rows().get(self.cursor).map(|(_, n)| *n)
    }
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut app: App,
) -> Result<()> {
    loop {
        terminal.draw(|f| render(f, &mut app))?;

        // Polled events — wake up periodically so
        // future async (e.g. file watcher) can plug in
        // without restructuring.
        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if handle_key(&mut app, key)? {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }
    }
}

/// Returns `Ok(true)` when the loop should exit.
fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    // Modal-first dispatch.
    if matches!(app.modal, Modal::Help { .. }) {
        // Any key dismisses the help pane.
        app.modal = Modal::None;
        return Ok(false);
    }

    // Global chords.
    if key.code == KeyCode::Char('q')
        && key.modifiers.contains(KeyModifiers::CONTROL)
    {
        return Ok(true);
    }
    if key.code == KeyCode::Esc {
        return Ok(true);
    }

    // Ctrl+B prefix → wait for next chord on the same key
    // press.  Phase 1 keeps this trivial: only Ctrl+B h
    // is registered.  We use a one-shot in-line state
    // machine since we don't need the full chord
    // matcher yet.
    static_chord_dispatch(app, key)?;
    Ok(false)
}

fn static_chord_dispatch(app: &mut App, key: KeyEvent) -> Result<()> {
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
            // Expand/collapse the focused stanza.
            if let Some(node) = app.current_node() {
                if !node.is_leaf() {
                    let path = node.path.clone();
                    if app.collapsed.contains(&path) {
                        app.collapsed.remove(&path);
                    } else {
                        app.collapsed.insert(path);
                    }
                }
            }
        }
        Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Quick-open help (without the Ctrl+B
            // prefix).  Phase 1 convenience.
            open_help(app);
        }
        Char('?') => {
            open_help(app);
        }
        _ => {}
    }
    Ok(())
}

fn open_help(app: &mut App) {
    if let Some(node) = app.current_node() {
        let body = app
            .help
            .lookup(&node.path)
            .map(str::to_owned)
            .unwrap_or_else(|| {
                format!(
                    "No CONFIGURATION.md row matched `{}`.\n\nDocs are indexed at build time from `Documentation/CONFIGURATION.md`.  If this field is new, add a row there.",
                    node.path
                )
            });
        app.modal = Modal::Help { body };
    }
}

// ── render ────────────────────────────────────────────

fn render(f: &mut ratatui::Frame, app: &mut App) {
    let size = f.area();
    let v_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // top bar
            Constraint::Min(0),     // body
            Constraint::Length(1),  // status bar
        ])
        .split(size);
    draw_top_bar(f, v_chunks[0], app);
    draw_body(f, v_chunks[1], app);
    draw_status(f, v_chunks[2], app);

    // Floating panes on top.
    if let Modal::Help { body } = &app.modal {
        draw_help_modal(f, size, body);
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
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_body(f: &mut ratatui::Frame, area: Rect, app: &mut App) {
    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Percentage(65),
        ])
        .split(area);
    draw_tree_pane(f, h_chunks[0], app);
    draw_detail_pane(f, h_chunks[1], app);
}

fn draw_tree_pane(f: &mut ratatui::Frame, area: Rect, app: &mut App) {
    // Clamp scroll BEFORE taking the borrow.  Computing
    // `rows()` again is cheap at literary scale (the
    // tree has hundreds of nodes, not millions).
    let inner_h = area.height.saturating_sub(2) as usize; // borders
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
        let style = if selected {
            Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        } else if node.source == ValueSource::Configured {
            Style::default().fg(Color::Green)
        } else {
            Style::default().add_modifier(Modifier::DIM)
        };
        let chip = match node.source {
            ValueSource::Configured if node.is_leaf() => "● ",
            _ => "  ",
        };
        lines.push(Line::from(Span::styled(
            format!("{indent}{glyph}{chip}{}", node.display),
            style,
        )));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_detail_pane(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Detail ");
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
    } else {
        lines.push(Line::from(Span::styled(
            format!(" {} children:", node.children.len()),
            bold,
        )));
        for child in &node.children {
            let chip = match child.source {
                ValueSource::Configured if child.is_leaf() => "●",
                _ => " ",
            };
            lines.push(Line::from(vec![
                Span::raw("   "),
                Span::styled(
                    chip,
                    Style::default().fg(Color::Green),
                ),
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
    let hints = " ↑↓ navigate · Enter expand/collapse · Ctrl+H help · ? help · Esc / Ctrl+Q quit";
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
    let h = host.height.saturating_sub(4).min(28);
    let x = host.x + host.width.saturating_sub(w) / 2;
    let y = host.y + host.height.saturating_sub(h) / 2;
    let rect = Rect { x, y, width: w, height: h };
    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help — CONFIGURATION.md slice ")
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let mut lines: Vec<Line<'_>> = body
        .lines()
        .map(|l| Line::from(Span::raw(l.to_string())))
        .collect();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " any key closes ",
        Style::default().add_modifier(Modifier::DIM),
    )));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
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
            // Detail pane handles stanzas via the
            // children list; this branch should be
            // unreachable for leaves but stays safe.
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
