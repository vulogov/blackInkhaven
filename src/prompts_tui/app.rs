//! 1.2.11+ — prompts-editor event loop + render.
//!
//! Phase 1: read-only walk-through.  CLI plumbing,
//! the four-pane shell, list navigation, show-on-
//! focus editor display, help pane.  No mutation,
//! no save, no AI send.

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

use crate::ai::prompts::PromptLibrary;
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
}

struct App {
    /// Project root — reserved for Phase 2's backup
    /// directory.
    #[allow(dead_code)]
    project_root: PathBuf,
    prompts_path: PathBuf,
    library: PromptLibrary,
    cursor: usize,
    /// Vertical-scroll offset for the list pane.
    list_scroll: usize,
    /// Scroll offset for the editor pane (read-only
    /// in Phase 1; Phase 2 replaces this with a
    /// tui-textarea).
    editor_scroll: usize,
    /// First-launch banner — flips off after the
    /// user opens the help pane or moves the
    /// cursor once.
    first_launch: bool,
    /// `true` when the on-disk `prompts.hjson` was
    /// missing at load and the embedded defaults
    /// were materialised in-memory.  Status bar
    /// flags this so the user knows what's loaded.
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
            // missing.  Stays in-memory until the
            // user hits `Ctrl+S` in Phase 2 (which
            // doesn't exist yet — Phase 1 is read-
            // only, so this is "see what defaults
            // ship").
            let lib: PromptLibrary = serde_hjson::from_str(DEFAULT_PROMPTS)
                .context("parse embedded DEFAULT_PROMPTS")?;
            (lib, true)
        };

        let status = if loaded_from_defaults {
            format!(
                "{} not found · {} embedded default prompts loaded (Phase 1 is read-only — save lands in Phase 2)",
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
            editor_scroll: 0,
            first_launch: true,
            loaded_from_defaults,
            focus: Focus::List,
            modal: Modal::None,
            status,
        })
    }

    fn current_prompt(&self) -> Option<&crate::ai::prompts::Prompt> {
        self.library.prompts.get(self.cursor)
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
    if matches!(app.modal, Modal::Help { .. }) {
        // Any key dismisses help.
        app.modal = Modal::None;
        app.first_launch = false;
        return Ok(false);
    }

    // Global exit chords.
    if key.code == KeyCode::Char('q')
        && key.modifiers.contains(KeyModifiers::CONTROL)
    {
        return Ok(true);
    }
    if key.code == KeyCode::Esc {
        return Ok(true);
    }

    // Help — Ctrl+H or ?.
    if (key.code == KeyCode::Char('h')
        && key.modifiers.contains(KeyModifiers::CONTROL))
        || key.code == KeyCode::Char('?')
    {
        open_help(app);
        return Ok(false);
    }

    // Tab / Shift+Tab cycles focus.
    if key.code == KeyCode::Tab && !key.modifiers.contains(KeyModifiers::SHIFT) {
        app.focus = app.focus.next();
        app.first_launch = false;
        app.status = format!("focus → {}", app.focus.label());
        return Ok(false);
    }
    if key.code == KeyCode::BackTab
        || (key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT))
    {
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

fn dispatch_list_keys(app: &mut App, key: KeyEvent) {
    let n = app.library.prompts.len();
    match key.code {
        KeyCode::Up => {
            if app.cursor > 0 {
                app.cursor -= 1;
                app.editor_scroll = 0;
                app.first_launch = false;
            }
        }
        KeyCode::Down => {
            if app.cursor + 1 < n {
                app.cursor += 1;
                app.editor_scroll = 0;
                app.first_launch = false;
            }
        }
        KeyCode::PageUp => {
            app.cursor = app.cursor.saturating_sub(10);
            app.editor_scroll = 0;
        }
        KeyCode::PageDown => {
            app.cursor = (app.cursor + 10).min(n.saturating_sub(1));
            app.editor_scroll = 0;
        }
        KeyCode::Home => {
            app.cursor = 0;
            app.editor_scroll = 0;
        }
        KeyCode::End => {
            app.cursor = n.saturating_sub(1);
            app.editor_scroll = 0;
        }
        _ => {}
    }
}

fn dispatch_editor_keys(app: &mut App, key: KeyEvent) {
    // Phase 1: read-only.  Editor pane responds only
    // to scroll keys.  Phase 2 plugs in tui-textarea.
    match key.code {
        KeyCode::Up => {
            app.editor_scroll = app.editor_scroll.saturating_sub(1);
        }
        KeyCode::Down => {
            app.editor_scroll = app.editor_scroll.saturating_add(1);
        }
        KeyCode::PageUp => {
            app.editor_scroll = app.editor_scroll.saturating_sub(10);
        }
        KeyCode::PageDown => {
            app.editor_scroll = app.editor_scroll.saturating_add(10);
        }
        KeyCode::Home => {
            app.editor_scroll = 0;
        }
        _ => {
            app.status =
                "editor is read-only in Phase 1 — Phase 2 plugs in tui-textarea".into();
        }
    }
}

fn dispatch_ai_prompt_keys(app: &mut App, _key: KeyEvent) {
    // Phase 1: input is inert.  Phase 3 wires it to
    // spawn_chat_stream.
    app.status =
        "AI prompt is inert in Phase 1 — Phase 3 wires it to the LLM".into();
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
        "   Tab / Shift+Tab   cycle pane focus",
        "   Esc / Ctrl+Q      quit",
        "",
        " Phase 2 adds: a add prompt · d delete prompt · Ctrl+S save",
        " Phase 3 adds: live LLM evaluation in the AI pane",
    ]
    .join("\n")
}

fn editor_help_body() -> String {
    [
        " Prompt editor — chord summary",
        "",
        "   ↑↓ / PgUp / PgDn  scroll (read-only in Phase 1)",
        "   Home              top",
        "   Tab / Shift+Tab   cycle pane focus",
        "",
        " Phase 2 plugs in tui-textarea so this pane gains the full",
        " main-editor chord set: arrow movement, Ctrl+Left/Right word",
        " motion, Shift+arrow selection, Ctrl+C / Ctrl+K clipboard,",
        " Ctrl+U undo, Ctrl+Y redo.",
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

    if let Modal::Help { body } = &app.modal {
        draw_help_modal(f, size, body);
    } else if app.first_launch {
        draw_welcome_overlay(f, size, app);
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
    draw_list_pane(f, h_chunks[0], app);
    draw_editor_pane(f, h_chunks[1], app);
    draw_ai_pane(f, h_chunks[2], app);
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
        let style = if selected {
            Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        } else {
            Style::default()
        };
        let marker = if selected { "▶" } else { " " };
        lines.push(Line::from(vec![
            Span::styled(format!(" {marker}  "), Style::default()),
            Span::styled(prompt.name.clone(), style),
        ]));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_editor_pane(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let focused = app.focus == Focus::Editor;
    let title = match app.current_prompt() {
        Some(p) => format!(" Editor — `{}` (read-only) ", p.name),
        None => " Editor ".to_string(),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border_style(focused));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(prompt) = app.current_prompt() else {
        let body = vec![Line::from(Span::styled(
            "  (no prompt selected)",
            Style::default().add_modifier(Modifier::DIM),
        ))];
        f.render_widget(Paragraph::new(body), inner);
        return;
    };

    let dim = Style::default().add_modifier(Modifier::DIM);
    let bold = Style::default().add_modifier(Modifier::BOLD);
    let mut lines: Vec<Line<'_>> = Vec::new();
    if !prompt.description.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(" description ", dim),
            Span::styled(format!(" {}", prompt.description), bold),
        ]));
        lines.push(Line::from(""));
    }
    let body_lines: Vec<Line<'_>> = prompt
        .template
        .lines()
        .skip(app.editor_scroll)
        .map(|l| Line::from(Span::raw(l.to_string())))
        .collect();
    lines.extend(body_lines);
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        inner,
    );
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
        Focus::List => " ↑↓ navigate · Tab next pane · ? help · Ctrl+Q quit",
        Focus::Editor => " ↑↓ scroll · Tab next pane · ? help · Ctrl+Q quit",
        Focus::AiPrompt => " Tab next pane · ? help · Ctrl+Q quit",
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
            editor_scroll: 0,
            first_launch: false,
            loaded_from_defaults: false,
            focus: Focus::List,
            modal: Modal::None,
            status: String::new(),
        };
        assert!(app.current_prompt().is_none());
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
            editor_scroll: 0,
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
