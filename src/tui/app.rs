use std::io;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent,
    KeyEventKind, KeyModifiers, KeyboardEnhancementFlags, MouseButton,
    MouseEvent, MouseEventKind, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
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
use tui_textarea::{CursorMove, TextArea};
use uuid::Uuid;

use crate::ai::AiClient;
use crate::ai::prompts::PromptLibrary;
use crate::ai::stream::{ChatTurn, StreamMsg, spawn_chat_stream};
use crate::config::Config;
use crate::error::{Error, Result as InkResult};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};
use crate::store::{InsertPosition, Snapshot};

use super::file_picker::{FilePicker, PickerContext};
use super::focus::Focus;
use super::quickref;
use super::search_replace::{RowMatch, SearchState, row_matches};
use super::session::{EditorSession, ParagraphCursor, SessionState, TreeSession};
use super::highlight::{
    BlockSelection, RowHit, TypstHighlighter, build_row_spans, build_visual_row_spans,
    diff_added, wrap_line,
};
use super::input::TextInput;
use super::keymap::KeyChord;
use super::search_results::SearchHit;

enum StartupError {
    UserAborted,
    Store(anyhow::Error),
}

/// Spawn `Store::open` on a worker thread and animate a "Please wait" splash
/// while it runs. Returns the opened store, or `UserAborted` if Ctrl+Q is
/// pressed during the splash.
fn open_store_with_splash<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    layout: ProjectLayout,
    cfg: Config,
) -> std::result::Result<Store, StartupError> {
    use std::sync::mpsc;

    let project_display = layout.root.display().to_string();
    let (tx, rx) = mpsc::channel::<crate::error::Result<Store>>();
    let layout_for_thread = layout.clone();
    let cfg_for_thread = cfg.clone();
    let _ = std::thread::Builder::new()
        .name("store-open".into())
        .spawn(move || {
            let result = Store::open(layout_for_thread, &cfg_for_thread);
            let _ = tx.send(result);
        });

    let started_at = std::time::Instant::now();
    let spinner_frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let mut spinner_idx: usize = 0;

    loop {
        let elapsed = started_at.elapsed().as_secs();
        let frame = spinner_frames[spinner_idx % spinner_frames.len()];
        spinner_idx = spinner_idx.wrapping_add(1);

        terminal
            .draw(|f| draw_splash(f, &project_display, frame, elapsed))
            .map_err(|e| StartupError::Store(anyhow::anyhow!("draw splash: {e}")))?;

        // Honor Ctrl+Q during the splash so a stuck DB load can be cancelled.
        if event::poll(std::time::Duration::from_millis(80))
            .map_err(|e| StartupError::Store(anyhow::anyhow!("event poll: {e}")))?
        {
            if let Event::Key(key) = event::read()
                .map_err(|e| StartupError::Store(anyhow::anyhow!("event read: {e}")))?
            {
                if key.kind == KeyEventKind::Press
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
                {
                    return Err(StartupError::UserAborted);
                }
            }
        }

        match rx.try_recv() {
            Ok(Ok(store)) => return Ok(store),
            Ok(Err(e)) => return Err(StartupError::Store(anyhow::Error::from(e))),
            Err(mpsc::TryRecvError::Empty) => continue,
            Err(mpsc::TryRecvError::Disconnected) => {
                return Err(StartupError::Store(anyhow::anyhow!(
                    "store-open worker thread died without sending a result"
                )));
            }
        }
    }
}

fn draw_splash(f: &mut ratatui::Frame, project_display: &str, spinner: char, elapsed_s: u64) {
    let area = f.area();
    // Center a small panel.
    let width = area.width.saturating_sub(8).clamp(40, 80);
    let height: u16 = 9;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let rect = Rect { x, y, width, height };

    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Inkhaven ")
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    let body = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {} Please wait for database to open…", spinner),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("  Project: {project_display}"),
            Style::default().add_modifier(Modifier::DIM),
        )),
        Line::from(Span::styled(
            format!("  Elapsed: {elapsed_s}s (first-run model download can take a minute)"),
            Style::default().add_modifier(Modifier::DIM),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Ctrl+Q to abort startup",
            Style::default().add_modifier(Modifier::DIM),
        )),
    ];
    f.render_widget(Paragraph::new(body).wrap(Wrap { trim: false }), inner);
}

/// 1.2.5+ — `logo.png` from the repo root embedded directly in
/// the binary via `include_bytes!`. Decoded lazily the first
/// time the credits modal opens so the cost is paid once per
/// session, not every Ctrl+B V press. The PNG's size on disk
/// is the binary-size delta; keep the source PNG appropriately
/// sized (~1–2 MB is a sensible upper bound).
static EMBEDDED_LOGO: &[u8] = include_bytes!("../../logo.png");

static DECODED_LOGO: std::sync::OnceLock<Option<image::DynamicImage>> =
    std::sync::OnceLock::new();

fn embedded_logo_image() -> Option<&'static image::DynamicImage> {
    DECODED_LOGO
        .get_or_init(|| image::load_from_memory(EMBEDDED_LOGO).ok())
        .as_ref()
}

/// 1.2.4+: project-pulse splash shown right after project open.
/// Renders for up to `STARTUP_SPLASH_SECS` seconds or until a key
/// press; the key press is consumed so it doesn't leak into the
/// editor's first frame. Failures (`terminal.draw` returning Err)
/// silently fall through — we'd rather skip the splash than block
/// the editor from launching.
fn run_startup_pulse_splash<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &App,
) {
    const STARTUP_SPLASH_SECS: u64 = 7;
    let started = std::time::Instant::now();
    let snap = app.progress_cache.clone();
    let project_display = app
        .layout
        .root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project")
        .to_string();
    let total_paragraphs = app
        .hierarchy
        .iter()
        .filter(|n| {
            n.kind == NodeKind::Paragraph
                && !app
                    .hierarchy
                    .ancestors(n)
                    .into_iter()
                    .any(|a| a.kind == NodeKind::Book && a.system_tag.is_some())
        })
        .count();
    let mut by_status: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for n in app.hierarchy.iter() {
        if n.kind != NodeKind::Paragraph {
            continue;
        }
        if app
            .hierarchy
            .ancestors(n)
            .into_iter()
            .any(|a| a.kind == NodeKind::Book && a.system_tag.is_some())
        {
            continue;
        }
        let key = n.status.as_deref().unwrap_or("None").to_string();
        *by_status.entry(key).or_insert(0) += 1;
    }

    loop {
        let elapsed = started.elapsed().as_secs();
        if elapsed >= STARTUP_SPLASH_SECS {
            break;
        }
        let remaining = STARTUP_SPLASH_SECS - elapsed;
        let _ = terminal.draw(|f| {
            draw_pulse_splash(
                f,
                &project_display,
                snap.as_ref(),
                total_paragraphs,
                &by_status,
                remaining,
            )
        });
        // Poll for keys with a short timeout. Any keystroke
        // dismisses the splash; consume it so the editor's
        // first frame doesn't see it as input.
        if crossterm::event::poll(std::time::Duration::from_millis(100))
            .unwrap_or(false)
        {
            if let Ok(crossterm::event::Event::Key(_)) = crossterm::event::read() {
                break;
            }
        }
    }
}

/// Render the startup project-pulse splash. See
/// `run_startup_pulse_splash` for the lifecycle.
fn draw_pulse_splash(
    f: &mut ratatui::Frame,
    project_display: &str,
    snap: Option<&crate::progress::ProgressSnapshot>,
    total_paragraphs: usize,
    by_status: &std::collections::BTreeMap<String, usize>,
    remaining_secs: u64,
) {
    let area = f.area();
    let width = area.width.saturating_sub(8).clamp(50, 90);
    let height: u16 = 15;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let rect = Rect { x, y, width, height };

    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Inkhaven · {project_display} "))
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
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(" Today's pulse", bold)));
    if let Some(s) = snap {
        let today_line = match s.project.daily_goal {
            Some(goal) => {
                let pct = if goal > 0 {
                    (s.project.today_words.max(0) * 100 / goal).clamp(0, 999)
                } else {
                    0
                };
                format!(
                    "   words: {}/{} ({}%)",
                    s.project.today_words, goal, pct
                )
            }
            None => format!("   words: {}", s.project.today_words),
        };
        lines.push(Line::from(today_line));
        lines.push(Line::from(format!(
            "   streak: {}d · active: {}",
            s.streak.days,
            format_active_duration(s.active_seconds_today)
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "   (progress tracking disabled or no data yet)",
            dim,
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(" Project", bold)));
    lines.push(Line::from(format!("   {total_paragraphs} paragraphs total")));
    if !by_status.is_empty() {
        let summary = by_status
            .iter()
            .map(|(k, v)| format!("{v} {k}"))
            .collect::<Vec<_>>()
            .join(" · ");
        lines.push(Line::from(format!("   by status: {summary}")));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("  any key dismisses · auto-close in {remaining_secs}s"),
        dim,
    )));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

pub fn run(project: &Path) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized().map_err(anyhow::Error::from)?;

    let cfg = Config::load(&layout.config_path()).map_err(anyhow::Error::from)?;

    // 1.2.5+: log the typst engine at startup so users can confirm
    // their HJSON setting took effect. Both engines are always
    // available — the in-process compiler ships in every 1.2.5
    // build — but the default stays `external` to match prior
    // behaviour exactly. The same one-liner also lands on the
    // status bar after first paint, and persistently in the
    // Ctrl+B V credits pane.
    let engine_summary = crate::typst_compile::engine_summary(&cfg);
    tracing::info!(
        target: "inkhaven::typst",
        "typst engine: {engine_summary}",
    );

    // Install the panic hook BEFORE we touch the terminal so a panic during
    // DB load (or anywhere later) still restores the screen.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    // Try to enable the kitty keyboard protocol. Without it, legacy
    // terminal encoding can't distinguish e.g. `Ctrl+1` from a bare `1`
    // (the TTY only has dedicated bytes for Ctrl+A..Z + a handful of
    // punctuation), so `Ctrl+1` ends up inserting "1" into the AI prompt.
    // Best-effort: terminals that don't support it just ignore the CSI
    // sequence and we run with reduced functionality.
    let kbd_enhanced = execute!(
        io::stdout(),
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS,
        )
    )
    .is_ok();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Open the document store on a worker thread so the TUI can draw a
    // "Please wait" splash immediately. First-time runs of fastembed download
    // a ~120 MB model, which takes long enough to look like a hang otherwise.
    let store_result = open_store_with_splash(&mut terminal, layout.clone(), cfg.clone());

    let store = match store_result {
        Ok(s) => s,
        Err(StartupError::UserAborted) => {
            disable_raw_mode()?;
            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
            terminal.show_cursor()?;
            return Ok(());
        }
        Err(StartupError::Store(e)) => {
            disable_raw_mode()?;
            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
            terminal.show_cursor()?;
            return Err(e);
        }
    };

    // Keep a copy of the config and layout for the auto-backup hook below;
    // App takes ownership of the originals.
    let cfg_for_exit = cfg.clone();
    let layout_for_exit = layout.clone();

    // Scripting layer (policy + active store) was armed inside
    // Store::open via scripting::configure. Force eager Adam
    // construction here so the bootstrap script runs and hook
    // lambdas are registered before the first store mutation
    // can fire a hook.
    if let Err(e) = crate::scripting::init_adam() {
        tracing::warn!("scripting init failed: {e}");
    }

    let mut app = App::new(layout, cfg, store)?;
    app.restore_session();
    app.install_progress();

    // 1.2.4+: project-pulse startup splash. Renders today /
    // streak / active-time / by-status counts; auto-closes
    // after 7 seconds or on any key press. Disabled via
    // `editor.startup_splash = false` in HJSON.
    if app.cfg.editor.startup_splash {
        run_startup_pulse_splash(&mut terminal, &app);
    }

    let result = app.run(&mut terminal);

    // Explicit final flush — HNSW save + DuckDB CHECKPOINT — while the
    // App still holds the Store. The pool's Drop impl would checkpoint
    // implicitly, but doing it explicitly here lets us log any error
    // and guarantees the auto-backup below sees a fully-drained WAL.
    app.shutdown_flush();
    crate::progress::uninstall();

    // Drop the App (and its Store handle) BEFORE running the auto-backup so
    // duckdb/HNSW checkpoint state is flushed to disk and the zip captures
    // a consistent snapshot rather than mid-write WAL data.
    drop(app);

    if let Err(e) = maybe_auto_backup(&mut terminal, &layout_for_exit, &cfg_for_exit) {
        // Backup failures must not eat the editor's own exit status — just
        // log them to stderr (which routes to .inkhaven.log in TUI mode)
        // and let the user retry with `inkhaven backup` manually.
        tracing::warn!("auto-backup on exit failed: {e}");
    }

    if kbd_enhanced {
        let _ = execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags);
    }
    let _ = execute!(terminal.backend_mut(), DisableMouseCapture);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    // Restore the hook we replaced.
    let _ = std::panic::take_hook();

    result
}

/// Render the centered "Performing database backup" splash with a progress
/// bar. Called from the exit hook each time another batch of files has
/// been zipped so the bar visibly advances. `done`/`total` are file counts.
fn draw_backup_splash(
    f: &mut ratatui::Frame,
    project_display: &str,
    done: usize,
    total: usize,
) {
    let area = f.area();
    let width = area.width.saturating_sub(8).clamp(50, 90);
    let height: u16 = 9;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let rect = Rect { x, y, width, height };
    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Inkhaven · backup ")
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    let bar_width = (inner.width as usize).saturating_sub(8).max(20);
    let pct = if total == 0 {
        0.0
    } else {
        (done as f32 / total as f32).clamp(0.0, 1.0)
    };
    let filled = (pct * bar_width as f32).round() as usize;
    let bar = format!(
        "  [{}{}]  {}/{} ({:>3.0}%)",
        "█".repeat(filled),
        "·".repeat(bar_width.saturating_sub(filled)),
        done,
        total,
        pct * 100.0,
    );

    let body = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Performing database backup…".to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("  Project: {project_display}"),
            Style::default().add_modifier(Modifier::DIM),
        )),
        Line::from(Span::styled(
            bar,
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];
    f.render_widget(Paragraph::new(body).wrap(Wrap { trim: false }), inner);
}

/// Render the centered "Importing directory" splash for the tree-pane
/// directory import. Mirrors `draw_backup_splash` but adds a third line
/// showing the file currently being imported so the user can see the
/// walk advance through the tree.
fn draw_import_splash(
    f: &mut ratatui::Frame,
    source_display: &str,
    done: usize,
    total: usize,
    current: &str,
) {
    let area = f.area();
    let width = area.width.saturating_sub(8).clamp(50, 100);
    let height: u16 = 11;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let rect = Rect { x, y, width, height };
    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Inkhaven · import directory ")
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    let bar_width = (inner.width as usize).saturating_sub(8).max(20);
    let pct = if total == 0 {
        0.0
    } else {
        (done as f32 / total as f32).clamp(0.0, 1.0)
    };
    let filled = (pct * bar_width as f32).round() as usize;
    let bar = format!(
        "  [{}{}]  {}/{} ({:>3.0}%)",
        "█".repeat(filled),
        "·".repeat(bar_width.saturating_sub(filled)),
        done,
        total,
        pct * 100.0,
    );

    // Clip the current-file line so the splash never wraps and shoves
    // the bar off-screen on narrow terminals.
    let label_budget = inner.width.saturating_sub(4) as usize;
    let current_clipped: String = if current.chars().count() > label_budget {
        let mut s: String = current
            .chars()
            .rev()
            .take(label_budget.saturating_sub(1))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        s.insert(0, '…');
        s
    } else {
        current.to_string()
    };

    let body = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Importing directory…".to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("  Source:  {source_display}"),
            Style::default().add_modifier(Modifier::DIM),
        )),
        Line::from(Span::styled(
            format!("  Current: {current_clipped}"),
            Style::default().add_modifier(Modifier::DIM),
        )),
        Line::from(""),
        Line::from(Span::styled(
            bar,
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];
    f.render_widget(Paragraph::new(body).wrap(Wrap { trim: false }), inner);
}

/// Splash for the Book-assembly procedure (Ctrl+B A). Mirrors the
/// import splash visually but the body line is "Assembling …" and the
/// per-file readout is relative to `<artefacts-root>` so the user sees
/// `inkhaven-artefacts/<project>/<book>/book/<chapter>/index.typ`-shape
/// paths scroll past.
fn draw_assembly_splash(
    f: &mut ratatui::Frame,
    book_display: &str,
    done: usize,
    total: usize,
    current: &str,
) {
    let area = f.area();
    let width = area.width.saturating_sub(8).clamp(50, 100);
    let height: u16 = 11;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let rect = Rect { x, y, width, height };
    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Inkhaven · Book assembly ")
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    let bar_width = (inner.width as usize).saturating_sub(8).max(20);
    let pct = if total == 0 {
        0.0
    } else {
        (done as f32 / total as f32).clamp(0.0, 1.0)
    };
    let filled = (pct * bar_width as f32).round() as usize;
    let bar = format!(
        "  [{}{}]  {}/{} ({:>3.0}%)",
        "█".repeat(filled),
        "·".repeat(bar_width.saturating_sub(filled)),
        done,
        total,
        pct * 100.0,
    );

    let label_budget = inner.width.saturating_sub(4) as usize;
    let current_clipped: String = if current.chars().count() > label_budget {
        let mut s: String = current
            .chars()
            .rev()
            .take(label_budget.saturating_sub(1))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        s.insert(0, '…');
        s
    } else {
        current.to_string()
    };

    let body = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Assembling book…".to_string(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("  Book:    {book_display}"),
            Style::default().add_modifier(Modifier::DIM),
        )),
        Line::from(Span::styled(
            format!("  Writing: {current_clipped}"),
            Style::default().add_modifier(Modifier::DIM),
        )),
        Line::from(""),
        Line::from(Span::styled(
            bar,
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];
    f.render_widget(Paragraph::new(body).wrap(Wrap { trim: false }), inner);
}

/// Splash for the `typst compile` step (Ctrl+B B / Ctrl+B O). Static
/// "Please wait" body plus an animated spinner the caller advances
/// each frame so the user can tell the TUI is still alive while the
/// child process churns.
fn draw_typst_compile_splash(
    f: &mut ratatui::Frame,
    book_display: &str,
    engine_label: &str,
    elapsed_secs: u64,
    spinner: char,
) {
    let area = f.area();
    let width = area.width.saturating_sub(8).clamp(50, 100);
    let height: u16 = 11;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let rect = Rect { x, y, width, height };
    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Inkhaven · typst compile ")
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    let body = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {spinner}  Please wait while PDF is generated…"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("  Book:    {book_display}"),
            Style::default().add_modifier(Modifier::DIM),
        )),
        Line::from(Span::styled(
            format!("  Engine:  {engine_label}"),
            Style::default().add_modifier(Modifier::DIM),
        )),
        Line::from(Span::styled(
            format!("  Elapsed: {elapsed_secs}s"),
            Style::default().add_modifier(Modifier::DIM),
        )),
        Line::from(Span::styled(
            "  Press Esc to cancel.",
            Style::default().fg(Color::Gray),
        )),
    ];
    f.render_widget(Paragraph::new(body).wrap(Wrap { trim: false }), inner);
}

const TYPST_COMPILE_SPINNER: &[char] = &[
    '⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏',
];

/// Walk `root` and count regular files that would be imported as
/// paragraphs (mirrors the importer's hidden-entry filter so the total
/// matches the progress callbacks).
fn count_importable_files(root: &Path) -> usize {
    walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            e.file_name()
                .to_str()
                .map(|s| !s.starts_with('.'))
                .unwrap_or(true)
        })
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .count()
}

/// Check whether the project is overdue for a backup and run one if so,
/// streaming progress into the splash drawn over the alternate screen.
/// Returns `Ok(())` when no backup was required OR the backup succeeded;
/// `Err(_)` if the zip failed mid-flight.
fn maybe_auto_backup<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    layout: &ProjectLayout,
    cfg: &Config,
) -> Result<()> {
    // Auto-backup opts out only via `max_age = 0s` now — `out_dir` empty
    // means "use the per-user default" (see `default_user_backup_dir`).
    let bcfg = &cfg.backup;
    if bcfg.max_age.as_secs() == 0 {
        return Ok(());
    }
    // If we already backed up recently, do nothing.
    let now = chrono::Utc::now();
    if let Some(state) = crate::backup::BackupState::load(&layout.root) {
        let age = now.signed_duration_since(state.last_at);
        if age.num_seconds() >= 0
            && (age.num_seconds() as u64) < bcfg.max_age.as_secs()
        {
            return Ok(());
        }
    }

    // Resolve the backup directory. Empty `out_dir` → per-user data
    // location; absolute path → used as-is; relative → resolved against
    // the project root (legacy override for users who explicitly want
    // backups inside the project).
    let out_dir = {
        let raw = bcfg.out_dir.trim();
        if raw.is_empty() {
            crate::store::default_user_backup_dir(&layout.root)
        } else {
            let p = std::path::PathBuf::from(raw);
            if p.is_absolute() {
                p
            } else {
                layout.root.join(p)
            }
        }
    };
    std::fs::create_dir_all(&out_dir).ok();
    let abs_project = std::fs::canonicalize(&layout.root)
        .unwrap_or_else(|_| layout.root.clone());
    let abs_out = std::fs::canonicalize(&out_dir).unwrap_or_else(|_| out_dir.clone());
    let skip = crate::cli::backup::skip_dirs_for(&abs_project, &abs_out);

    let project_display = layout.root.display().to_string();
    // First frame: 0/0 so the bar shows immediately even before file
    // enumeration completes.
    let _ = terminal.draw(|f| draw_backup_splash(f, &project_display, 0, 0));
    let mut last_redraw = std::time::Instant::now();
    let mut progress = |done: usize, total: usize| {
        // Throttle redraws to ~30Hz so a tiny project doesn't drown the
        // terminal in noise on a fast disk.
        if last_redraw.elapsed() < std::time::Duration::from_millis(33) {
            return;
        }
        last_redraw = std::time::Instant::now();
        let _ = terminal.draw(|f| draw_backup_splash(f, &project_display, done, total));
    };

    crate::backup::create_backup(&abs_project, &abs_out, &skip, Some(&mut progress))
        .map_err(anyhow::Error::from)?;
    Ok(())
}

struct Keymap {
    next_pane: KeyChord,
    prev_pane: KeyChord,
    search: KeyChord,
    ai_prompt: KeyChord,
    save: KeyChord,
    page_up: KeyChord,
    page_down: KeyChord,
    meta_prefix: KeyChord,
    /// Bund-meta prefix. `None` when the config sets
    /// `keys.bund_prefix = ""` to disable the chord (some users
    /// reserve Ctrl+Z for their terminal multiplexer).
    bund_prefix: Option<KeyChord>,
    /// View-meta prefix (1.2.4+, default `Ctrl+V`). `None`
    /// disables the layer (some terminals bind Ctrl+V to "verbatim
    /// next" and the user might want it back).
    view_prefix: Option<KeyChord>,
}

impl Keymap {
    fn from_config(cfg: &Config) -> InkResult<Self> {
        let parse = |label: &str, s: &str| -> InkResult<KeyChord> {
            KeyChord::parse(s).map_err(|e| Error::Config(format!("keys.{label}: {e}")))
        };
        let bund_prefix = if cfg.keys.bund_prefix.trim().is_empty() {
            None
        } else {
            Some(parse("bund_prefix", &cfg.keys.bund_prefix)?)
        };
        let view_prefix = if cfg.keys.view_prefix.trim().is_empty() {
            None
        } else {
            Some(parse("view_prefix", &cfg.keys.view_prefix)?)
        };
        Ok(Self {
            next_pane: parse("next_pane", &cfg.keys.next_pane)?,
            prev_pane: parse("prev_pane", &cfg.keys.prev_pane)?,
            search: parse("search", &cfg.keys.search)?,
            ai_prompt: parse("ai_prompt", &cfg.keys.ai_prompt)?,
            save: parse("save", &cfg.keys.save)?,
            page_up: parse("page_up", &cfg.keys.page_up)?,
            page_down: parse("page_down", &cfg.keys.page_down)?,
            meta_prefix: parse("meta_prefix", &cfg.keys.meta_prefix)?,
            bund_prefix,
            view_prefix,
        })
    }
}

/// Which scope the Ctrl+V markdown export targets. Used by
/// `view_export_markdown` to route through the existing
/// per-scope helpers from one binding-table arm.
#[derive(Debug, Clone, Copy)]
enum ViewMdScope {
    Buffer,
    Subchapter,
    Subtree,
}

#[derive(Debug, Clone, Copy)]
enum MoveDir {
    Up,
    Down,
}

/// Direction of a link-pick flow (Ctrl+V A vs Ctrl+V I).
/// `Outgoing` adds the picked target to the open paragraph's
/// outgoing links; `Incoming` adds the open paragraph to the
/// picked target's outgoing links (== an incoming link for
/// current).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkPickDirection {
    Outgoing,
    Incoming,
}

#[derive(Default)]
struct ImportCounts {
    /// Any branch created during import: chapter, subchapter, or book.
    branches: usize,
    paragraphs: usize,
}

/// Read a directory's immediate children, filter hidden entries, sort dirs
/// first then alphabetical. Returns owned paths so the caller doesn't carry
/// a borrow against the DirEntry iterator.
fn read_sorted_children(source: &std::path::Path) -> Vec<std::path::PathBuf> {
    let Ok(rd) = std::fs::read_dir(source) else {
        return Vec::new();
    };
    let mut entries: Vec<_> = rd
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|s| !s.starts_with('.'))
                .unwrap_or(true)
        })
        .collect();
    entries.sort_by(|a, b| {
        let a_dir = a.path().is_dir();
        let b_dir = b.path().is_dir();
        match (a_dir, b_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });
    entries.into_iter().map(|e| e.path()).collect()
}

fn derive_paragraph_title_from_path(path: &std::path::Path) -> String {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("imported");
    let pretty: String = stem
        .replace('_', " ")
        .replace('-', " ")
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(c).collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    if pretty.is_empty() {
        "Imported".into()
    } else {
        pretty
    }
}

/// Return the lowercase extension if `path` looks like an image file
/// we'll route to `import_single_image`. The list is the recognised
/// set Typst's `#image(...)` natively understands: PNG, JPG, JPEG,
/// GIF, WebP, SVG. Anything else stays a paragraph candidate.
fn image_extension_for(path: &std::path::Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" => Some(ext),
        _ => None,
    }
}

/// Map file extension → content_type tag stored on the resulting
/// Paragraph. `.hjson` → "hjson"; anything else (including `.typ` /
/// no extension / plain text files) → `None`, which means "typst
/// default".
fn content_type_for(path: &std::path::Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "hjson" => Some("hjson".into()),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
enum InferenceAction {
    Replace,
    Insert,
    Top,
    Bottom,
    CopyOnly,
    /// Grammar-check-aware replace: lifts ONLY the corrected paragraph
    /// from the response (between `<<<CORRECTED>>>` / `<<<END>>>` markers,
    /// or fenced code, or a "Corrected …" heading) and overwrites the
    /// editor buffer with it. No markdown→typst conversion runs — the
    /// grammar prompt instructs the model to keep Typst markup verbatim.
    ReplaceCorrected,
}

/// Scope of context an AI prompt sweeps in along with the user's query.
/// Cycled by F9: None → Selection → Paragraph → Subchapter → Chapter →
/// Book → None. Each non-None scope prepends the relevant text to the
/// query before sending; after a successful submission the mode auto-
/// resets to None so a follow-up prompt isn't surprised by stale scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AiMode {
    None,
    Selection,
    Paragraph,
    Subchapter,
    Chapter,
    Book,
}

impl AiMode {
    fn label(self) -> &'static str {
        match self {
            AiMode::None => "None",
            AiMode::Selection => "Selection",
            AiMode::Paragraph => "Paragraph",
            AiMode::Subchapter => "Subchapter",
            AiMode::Chapter => "Chapter",
            AiMode::Book => "Book",
        }
    }
    fn next(self) -> Self {
        match self {
            AiMode::None => AiMode::Selection,
            AiMode::Selection => AiMode::Paragraph,
            AiMode::Paragraph => AiMode::Subchapter,
            AiMode::Subchapter => AiMode::Chapter,
            AiMode::Chapter => AiMode::Book,
            AiMode::Book => AiMode::None,
        }
    }
}

/// How aggressively the model is allowed to draw on its own knowledge.
/// F10 toggles between the two values. Help inferences always run as
/// `Local` regardless of the user's current toggle — the Help book is the
/// authoritative source and we don't want the model paraphrasing from
/// general training data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InferenceMode {
    /// Only the supplied RAG / scope context (and prior chat turns) may be
    /// used. The system prompt instructs the model to refuse rather than
    /// fall back on outside knowledge.
    Local,
    /// Context is treated as ground truth where present, but the model
    /// may augment with general knowledge. Default for fresh chats.
    Full,
}

impl InferenceMode {
    fn label(self) -> &'static str {
        match self {
            InferenceMode::Local => "Local",
            InferenceMode::Full => "Full",
        }
    }
    fn toggle(self) -> Self {
        match self {
            InferenceMode::Local => InferenceMode::Full,
            InferenceMode::Full => InferenceMode::Local,
        }
    }
}

impl InferenceAction {
    fn label(&self) -> &'static str {
        match self {
            InferenceAction::Replace => "replaced",
            InferenceAction::Insert => "inserted at cursor",
            InferenceAction::Top => "prepended to top",
            InferenceAction::Bottom => "appended to bottom",
            InferenceAction::CopyOnly => "copied",
            InferenceAction::ReplaceCorrected => "replaced with corrected text",
        }
    }
}

/// Extract only the corrected-paragraph text from a grammar-check
/// response. Tries in order: marker block (preferred), last fenced code
/// block, then everything after a "Corrected …" line. Returns `None` if
/// none of those patterns match so callers can refuse rather than paste
/// commentary by mistake.
fn extract_corrected_text(response: &str) -> Option<String> {
    if let Some(begin) = response.find(CORRECTED_BEGIN) {
        let after = &response[begin + CORRECTED_BEGIN.len()..];
        if let Some(end_offset) = after.find(CORRECTED_END) {
            let inner = &after[..end_offset];
            let cleaned = inner.trim_matches(|c: char| c == '\n' || c == '\r' || c == ' ');
            if !cleaned.is_empty() {
                return Some(cleaned.to_string());
            }
        }
    }
    if let Some(last_close) = response.rfind("```") {
        let before = &response[..last_close];
        if let Some(open) = before.rfind("```") {
            let body = &response[open + 3..last_close];
            let (first_nl, rest) = match body.find('\n') {
                Some(i) => (&body[..i], &body[i + 1..]),
                None => (body, ""),
            };
            // Drop a short alphanumeric language tag on the first line.
            let cleaned = if !first_nl.is_empty()
                && first_nl.len() < 16
                && first_nl.chars().all(|c| c.is_ascii_alphanumeric())
            {
                rest.to_string()
            } else {
                body.to_string()
            };
            let trimmed = cleaned.trim_matches(|c: char| c == '\n' || c == '\r' || c == ' ');
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    let lower = response.to_ascii_lowercase();
    if let Some(idx) = lower.rfind("corrected") {
        if let Some(line_end) = response[idx..].find('\n') {
            let after = response[idx + line_end + 1..]
                .trim_matches(|c: char| c == '\n' || c == '\r' || c == ' ');
            if !after.is_empty() {
                return Some(after.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod corrected_tests {
    use super::*;

    #[test]
    fn marker_block_wins() {
        let r = "Summary: 1 issue.\n\n<<<CORRECTED>>>\n= Heading\n\nThe rain in Spain.\n<<<END>>>\n";
        let got = extract_corrected_text(r).unwrap();
        assert_eq!(got, "= Heading\n\nThe rain in Spain.");
    }

    #[test]
    fn falls_back_to_code_fence() {
        let r = "Summary.\n\n```typst\n= Heading\n\nFixed body.\n```\n";
        let got = extract_corrected_text(r).unwrap();
        assert_eq!(got, "= Heading\n\nFixed body.");
    }

    #[test]
    fn falls_back_to_corrected_heading() {
        let r = "Summary line.\n- issue 1\n\nCorrected paragraph:\n= Heading\n\nFixed body.\n";
        let got = extract_corrected_text(r).unwrap();
        assert_eq!(got, "= Heading\n\nFixed body.");
    }

    #[test]
    fn returns_none_on_empty_or_unmatched() {
        assert!(extract_corrected_text("").is_none());
        assert!(extract_corrected_text("Just commentary, no markers.").is_none());
    }
}

#[cfg(test)]
mod book_info_tests {
    use super::*;

    #[test]
    fn count_sentences_basic_terminators() {
        assert_eq!(count_sentences("Hello. World!"), 2);
        assert_eq!(count_sentences("Why? Because!"), 2);
        assert_eq!(count_sentences("One sentence only"), 0);
    }

    #[test]
    fn count_sentences_collapses_runs() {
        // Repeated terminators ("...", "?!") should count as one sentence
        // each, not three / two.
        assert_eq!(count_sentences("Hmm... interesting?!"), 2);
    }

    #[test]
    fn count_sentences_ignores_headings_and_comments() {
        let body = "= Chapter title\n\n// this comment has a period.\n\
                    First. Second.";
        assert_eq!(count_sentences(body), 2);
    }

    #[test]
    fn count_sentences_handles_blank_and_multiline() {
        let body = "First sentence.\n\nSecond sentence!\nThird?";
        assert_eq!(count_sentences(body), 3);
    }

    #[test]
    fn format_age_subminute() {
        assert_eq!(format_age_humantime(std::time::Duration::from_secs(0)), "0s");
        assert_eq!(format_age_humantime(std::time::Duration::from_secs(45)), "45s");
    }

    #[test]
    fn chat_turn_roundtrips_through_serde() {
        let history = vec![
            ChatTurn::User("What's the weather?".into()),
            ChatTurn::Assistant("I don't have a weather tool.".into()),
        ];
        let json = serde_json::to_string(&history).expect("encode");
        let back: Vec<ChatTurn> = serde_json::from_str(&json).expect("decode");
        assert_eq!(back.len(), 2);
        match &back[0] {
            ChatTurn::User(s) => assert_eq!(s, "What's the weather?"),
            _ => panic!("expected User turn"),
        }
        match &back[1] {
            ChatTurn::Assistant(s) => assert_eq!(s, "I don't have a weather tool."),
            _ => panic!("expected Assistant turn"),
        }
    }

    #[test]
    fn digit_to_status_mapping() {
        assert_eq!(digit_to_status('1'), Some("Ready"));
        assert_eq!(digit_to_status('2'), Some("Final"));
        assert_eq!(digit_to_status('3'), Some("Third"));
        assert_eq!(digit_to_status('4'), Some("Second"));
        assert_eq!(digit_to_status('5'), Some("First"));
        assert_eq!(digit_to_status('6'), Some("Napkin"));
        assert_eq!(digit_to_status('7'), Some("None"));
        // 0, 8, 9 and letters don't map.
        assert_eq!(digit_to_status('0'), None);
        assert_eq!(digit_to_status('8'), None);
        assert_eq!(digit_to_status('a'), None);
    }

    #[test]
    fn status_letter_returns_one_char_or_space() {
        assert_eq!(status_letter("Napkin"), "n");
        assert_eq!(status_letter("First"), "1");
        assert_eq!(status_letter("Second"), "2");
        assert_eq!(status_letter("Third"), "3");
        assert_eq!(status_letter("Final"), "F");
        assert_eq!(status_letter("Ready"), "R");
        assert_eq!(status_letter("None"), " ");
        assert_eq!(status_letter("Unknown"), " ");
    }

    #[test]
    fn next_status_walks_the_ring() {
        assert_eq!(next_status(None), "Napkin");
        assert_eq!(next_status(Some("Napkin")), "First");
        assert_eq!(next_status(Some("First")), "Second");
        assert_eq!(next_status(Some("Second")), "Third");
        assert_eq!(next_status(Some("Third")), "Final");
        assert_eq!(next_status(Some("Final")), "Ready");
        // Wrap.
        assert_eq!(next_status(Some("Ready")), "None");
        // Empty string = same as None.
        assert_eq!(next_status(Some("")), "Napkin");
    }

    #[test]
    fn prev_status_walks_backwards_and_wraps() {
        assert_eq!(prev_status(Some("Napkin")), "None");
        assert_eq!(prev_status(Some("Ready")), "Final");
        assert_eq!(prev_status(Some("Final")), "Third");
        assert_eq!(prev_status(None), "Ready"); // wrap from None backwards
    }

    #[test]
    fn next_status_unknown_value_treated_as_none() {
        assert_eq!(next_status(Some("WeirdCustom")), "Napkin");
    }

    #[test]
    fn display_status_collapses_none_variants() {
        assert_eq!(display_status(None), "None");
        assert_eq!(display_status(Some("")), "None");
        assert_eq!(display_status(Some("   ")), "None");
        assert_eq!(display_status(Some("Napkin")), "Napkin");
    }

    #[test]
    fn byte_offset_for_cursor_basic() {
        let src = "abc\ndef\nghi";
        // Row 0, col 0 → byte 0.
        assert_eq!(byte_offset_for_cursor(src, 0, 0), 0);
        // Row 0, col 3 → end of "abc".
        assert_eq!(byte_offset_for_cursor(src, 0, 3), 3);
        // Row 1, col 0 → after the first newline.
        assert_eq!(byte_offset_for_cursor(src, 1, 0), 4);
        // Row 2, col 2 → "gh|i".
        assert_eq!(byte_offset_for_cursor(src, 2, 2), 10);
    }

    #[test]
    fn byte_offset_for_cursor_handles_multibyte() {
        // "Москва" is 6 chars / 12 bytes.
        let src = "Москва";
        assert_eq!(byte_offset_for_cursor(src, 0, 0), 0);
        assert_eq!(byte_offset_for_cursor(src, 0, 6), 12);
        // Mid-way: 3 chars in is 6 bytes.
        assert_eq!(byte_offset_for_cursor(src, 0, 3), 6);
    }

    #[test]
    fn open_pair_for_known_openers() {
        assert_eq!(open_pair_for('('), Some(')'));
        assert_eq!(open_pair_for('['), Some(']'));
        assert_eq!(open_pair_for('{'), Some('}'));
        assert_eq!(open_pair_for('"'), Some('"'));
        assert_eq!(open_pair_for('\''), Some('\''));
        assert_eq!(open_pair_for('a'), None);
        assert_eq!(open_pair_for(')'), None);
    }

    #[test]
    fn is_close_pair_char_covers_all_closers() {
        for c in [')', ']', '}', '"', '\''] {
            assert!(is_close_pair_char(c), "{c} should be a close pair char");
        }
        for c in ['(', '[', '{', 'a', '#'] {
            assert!(!is_close_pair_char(c), "{c} should not be");
        }
    }

    /// Reproduce the pair-detection rule used by the Enter and
    /// Backspace handlers without going through tui-textarea.
    fn between_pair(line: &str, col: usize) -> bool {
        let chars: Vec<char> = line.chars().collect();
        let before = if col > 0 { chars.get(col - 1).copied() } else { None };
        let after = chars.get(col).copied();
        matches!(
            (before, after),
            (Some('('), Some(')')) | (Some('['), Some(']')) | (Some('{'), Some('}'))
        )
    }

    #[test]
    fn pair_detection_inside_freshly_typed_pair() {
        // `foo(|)` — cursor at col 4, between `(` and `)`.
        assert!(between_pair("foo()", 4));
        // `[]` — cursor at col 1, between `[` and `]`.
        assert!(between_pair("[]", 1));
        // `{}` — cursor at col 1, between `{` and `}`.
        assert!(between_pair("{}", 1));
    }

    #[test]
    fn pair_detection_skips_when_chars_dont_match() {
        // Just before `)` but `(` is not on the immediate left.
        assert!(!between_pair("foo )", 4));
        // Indented prose with parens that DO match — pair logic only
        // cares about the immediately adjacent chars.
        assert!(!between_pair("  abc(def)", 6));
    }

    #[test]
    fn filter_functions_empty_returns_all() {
        let all = filter_functions("").len();
        assert!(all > 50, "expected the baked-in table to have >50 entries, got {all}");
    }

    #[test]
    fn filter_functions_substring_match() {
        let m: Vec<&'static str> = filter_functions("image").iter().map(|f| f.name).collect();
        assert!(m.contains(&"image"), "got: {m:?}");
        assert!(m.contains(&"figure") == false || true);
    }

    #[test]
    fn filter_functions_case_insensitive() {
        let a = filter_functions("Heading");
        let b = filter_functions("heading");
        assert_eq!(a.len(), b.len());
    }

    #[test]
    fn filter_functions_no_match_returns_empty() {
        assert!(filter_functions("zzz-no-such-function").is_empty());
    }

    #[test]
    fn image_call_context_inside_open_string() {
        // Cursor is after the `"` — we're inside the first string arg.
        let line = "#image(\"";
        let ctx = detect_image_call_context(line, line.chars().count())
            .expect("should detect");
        assert!(!ctx.closing_quote_present);
    }

    #[test]
    fn image_call_context_inside_with_closing_quote() {
        let line = "#image(\"\")";
        // Cursor positioned right after the opening quote (col 8).
        let ctx = detect_image_call_context(line, 8).expect("should detect");
        assert!(ctx.closing_quote_present);
    }

    #[test]
    fn image_call_context_not_inside_when_closed() {
        // After the closing `)`.
        let line = "#image(\"cover.png\")";
        let n = line.chars().count();
        assert!(detect_image_call_context(line, n).is_none());
    }

    #[test]
    fn image_call_context_requires_hash_prefix() {
        // `image(` without leading `#` is not a typst function call.
        let line = "image(\"cover.png";
        assert!(detect_image_call_context(line, line.chars().count()).is_none());
    }

    #[test]
    fn image_call_context_not_after_other_function() {
        let line = "#text(\"hello";
        assert!(detect_image_call_context(line, line.chars().count()).is_none());
    }

    #[test]
    fn image_call_context_in_chapter_body() {
        // Realistic editor line — leading indentation + prose, then a
        // `#image(` partway through.
        let line = "  Some prose. #image(\"01-co";
        let ctx = detect_image_call_context(line, line.chars().count())
            .expect("should detect inside the call");
        assert!(!ctx.closing_quote_present);
    }

    #[test]
    fn body_to_lines_strips_crlf() {
        // CRLF (DOS / Windows / RFC dumps): trailing `\r` must not
        // survive into the line list.
        let body = "Network Working Group\r\nRequest for Comments: 1\r\n";
        let lines = body_to_lines(body);
        assert_eq!(lines.len(), 3); // last `\n` produces a trailing "" entry
        assert_eq!(lines[0], "Network Working Group");
        assert_eq!(lines[1], "Request for Comments: 1");
        assert_eq!(lines[2], "");
    }

    #[test]
    fn body_to_lines_strips_bare_cr() {
        // Old-Mac files used bare `\r`. Treat them as line breaks too.
        let body = "first\rsecond\rthird";
        let lines = body_to_lines(body);
        assert_eq!(lines, vec!["first", "second", "third"]);
    }

    #[test]
    fn body_to_lines_unix_passthrough() {
        let body = "alpha\nbeta\ngamma";
        let lines = body_to_lines(body);
        assert_eq!(lines, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn body_to_lines_empty_yields_single_empty() {
        assert_eq!(body_to_lines(""), vec![String::new()]);
    }

    #[test]
    fn set_llm_default_rewrites_value_only() {
        let raw = "\
language: english

llm: {
  // Provider used by default.
  default: gemini
  providers: {
    gemini: { model: gemini-2.5-pro }
    ollama: { model: llama3.2 }
  }
}
";
        let out = set_llm_default_in_hjson(raw, "ollama").unwrap();
        // The default line changed.
        assert!(out.contains("default: ollama"));
        assert!(!out.contains("default: gemini"));
        // Everything else survives byte-for-byte.
        assert!(out.contains("language: english"));
        assert!(out.contains("// Provider used by default."));
        assert!(out.contains("model: gemini-2.5-pro"));
        assert!(out.contains("model: llama3.2"));
    }

    #[test]
    fn set_llm_default_preserves_trailing_comment() {
        let raw = "\
llm: {
  default: gemini    // pick gemini for prose
  providers: { gemini: { model: x } }
}
";
        let out = set_llm_default_in_hjson(raw, "ollama").unwrap();
        // Value swapped, comment retained.
        assert!(
            out.contains("default: ollama"),
            "expected new default; got:\n{out}"
        );
        assert!(
            out.contains("// pick gemini for prose"),
            "expected trailing comment preserved; got:\n{out}"
        );
    }

    #[test]
    fn set_llm_default_quotes_unsafe_values() {
        let raw = "\
llm: {
  default: gemini
  providers: { x: { model: y } }
}
";
        let out = set_llm_default_in_hjson(raw, "weird name").unwrap();
        assert!(
            out.contains("default: \"weird name\""),
            "value with space should be quoted; got:\n{out}"
        );
    }

    #[test]
    fn set_llm_default_inserts_when_missing() {
        // No `default:` key in the llm block — insert one.
        let raw = "\
llm: {
  providers: { gemini: { model: g } }
}
";
        let out = set_llm_default_in_hjson(raw, "gemini").unwrap();
        assert!(
            out.contains("default: gemini"),
            "expected inserted default; got:\n{out}"
        );
        // The providers block survives.
        assert!(out.contains("providers: { gemini: { model: g } }"));
    }

    #[test]
    fn set_llm_default_errors_on_missing_block() {
        let raw = "language: english\n";
        let err = set_llm_default_in_hjson(raw, "gemini").unwrap_err();
        assert!(err.contains("no `llm:` block"), "got: {err}");
    }

    #[test]
    fn set_llm_default_roundtrips_shipped_template() {
        // The annotated default HJSON the project ships with has to
        // survive a switch and still parse cleanly via `Config::load`.
        // This is the regression we care about most — if the in-place
        // edit garbles the file, the user's next launch fails to read
        // their config.
        let raw = crate::config::DEFAULT_PROJECT_CONFIG;
        let edited = set_llm_default_in_hjson(raw, "ollama").unwrap();
        let cfg: crate::config::Config =
            serde_hjson::from_str(&edited).expect("edited HJSON should still parse");
        assert_eq!(cfg.llm.default, "ollama");
        // The two non-llm sections should round-trip unchanged.
        assert_eq!(cfg.language, "english");
        assert_eq!(cfg.editor.tab_width, 2);
        // Comments survive (string match in the raw text).
        assert!(edited.contains("// Provider used by the AI pane"));
    }

    #[test]
    fn set_sound_enabled_rewrites_existing_block() {
        let raw = "\
sound: {
  enabled: false
  volume: 0.6
}
";
        let out = set_sound_enabled_in_hjson(raw, true).unwrap();
        assert!(out.contains("enabled: true"));
        assert!(out.contains("volume: 0.6"));
        // And toggling back.
        let back = set_sound_enabled_in_hjson(&out, false).unwrap();
        assert!(back.contains("enabled: false"));
    }

    #[test]
    fn set_sound_enabled_roundtrips_shipped_template() {
        let raw = crate::config::DEFAULT_PROJECT_CONFIG;
        let edited = set_sound_enabled_in_hjson(raw, true).unwrap();
        let cfg: crate::config::Config =
            serde_hjson::from_str(&edited).expect("edited HJSON should still parse");
        assert!(cfg.sound.enabled);
        // Sound-unrelated stanzas untouched.
        assert_eq!(cfg.language, "english");
        assert!(edited.contains("// Typewriter-style sound effects"));
    }

    #[test]
    fn set_sound_enabled_inserts_block_when_missing() {
        // Older configs without a sound block — the helper inserts a
        // minimal one just inside the root closing brace, so the file
        // stays valid HJSON.
        let raw = "{\n  language: english\n}\n";
        let out = set_sound_enabled_in_hjson(raw, true).unwrap();
        assert!(out.contains("sound: {"), "got:\n{out}");
        assert!(out.contains("enabled: true"));
        let cfg: crate::config::Config =
            serde_hjson::from_str(&out).expect("inserted HJSON should still parse");
        assert!(cfg.sound.enabled);
    }

    #[test]
    fn set_sound_enabled_insertion_lands_before_root_close() {
        // Regression: the previous version appended after the root `}`
        // which made the file invalid HJSON. Verify the new block lives
        // strictly before the root close, and the file round-trips.
        let raw = "{\n  language: english\n  theme: {\n    pane_bg: \"#1e1e2e\"\n  }\n}\n";
        let out = set_sound_enabled_in_hjson(raw, true).unwrap();
        let sound_idx = out.find("sound:").expect("sound block inserted");
        let last_close = out.rfind('}').expect("root close present");
        assert!(
            sound_idx < last_close,
            "sound block must be before root close — got:\n{out}"
        );
        let _: crate::config::Config = serde_hjson::from_str(&out).expect("must parse");
    }

    #[test]
    fn format_reading_time_thresholds() {
        assert_eq!(format_reading_time(0), "<1m");
        assert_eq!(format_reading_time(1), "~1m");          // ceil(1/250) = 1
        assert_eq!(format_reading_time(250), "~1m");
        assert_eq!(format_reading_time(251), "~2m");
        assert_eq!(format_reading_time(250 * 60), "~1h");   // exact hour, no "0m"
        assert_eq!(format_reading_time(250 * 75), "~1h 15m");
    }

    #[test]
    fn set_llm_default_does_not_match_provider_internals() {
        // A `default:` key inside a nested provider block must NOT be
        // mistaken for `llm.default`. Our scanner requires depth==1.
        let raw = "\
llm: {
  default: gemini
  providers: {
    fake: {
      default: should_not_be_touched
      model: x
    }
  }
}
";
        let out = set_llm_default_in_hjson(raw, "ollama").unwrap();
        assert!(out.contains("default: ollama"));
        assert!(
            out.contains("default: should_not_be_touched"),
            "nested `default:` inside a provider must survive untouched; got:\n{out}"
        );
    }

    #[test]
    fn format_age_minutes_hours_days() {
        assert_eq!(
            format_age_humantime(std::time::Duration::from_secs(7 * 60)),
            "7m"
        );
        assert_eq!(
            format_age_humantime(std::time::Duration::from_secs(3 * 3600 + 30 * 60)),
            "3h 30m"
        );
        assert_eq!(
            format_age_humantime(std::time::Duration::from_secs(2 * 86_400 + 4 * 3600)),
            "2d 4h"
        );
        // Whole-hour and whole-day values shouldn't dangle a "0m" / "0h".
        assert_eq!(
            format_age_humantime(std::time::Duration::from_secs(5 * 86_400)),
            "5d"
        );
        assert_eq!(
            format_age_humantime(std::time::Duration::from_secs(2 * 3600)),
            "2h"
        );
    }
}

/// One entry in the `/` prompt picker. Wraps both shipping HJSON prompts
/// (`PromptSource::System`) and user-authored paragraphs under the Prompts
/// book (`PromptSource::Book`). The body is lazily fetched for book
/// paragraphs so we don't hit the store while filtering as the user types.
#[derive(Debug, Clone)]
struct PromptCandidate {
    name: String,
    description: String,
    body: PromptBody,
    source: PromptSource,
}

#[derive(Debug, Clone)]
enum PromptBody {
    Static(String),
    BookParagraph(Uuid),
}

#[derive(Debug, Clone, Copy)]
enum PromptSource {
    System,
    Book,
}

/// Strip a leading Typst heading line (`= Title`) from a paragraph body so
/// it doesn't end up in the LLM prompt. The heading is editor chrome — it
/// describes the prompt for tree-pane navigation, not text the user wants
/// to send. Trims any blank lines that immediately follow the heading.
fn strip_leading_typst_heading(body: &str) -> String {
    let mut lines: Vec<&str> = body.lines().collect();
    if let Some(first) = lines.first() {
        if first.trim_start().starts_with('=') {
            lines.remove(0);
            while lines.first().is_some_and(|l| l.trim().is_empty()) {
                lines.remove(0);
            }
        }
    }
    lines.join("\n")
}

/// Where the `Ctrl+Z ?` script picker is sourcing entries from.
/// `A` inside the modal toggles between the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScriptPickerScope {
    /// Scripts under the cursor's nearest containing branch
    /// (subchapter / chapter / book — whichever is closest).
    Branch,
    /// Scripts under the `Scripts` system book.
    ScriptsBook,
}

/// One row in the script-picker modal.
#[derive(Debug, Clone)]
pub(crate) struct ScriptPickerEntry {
    pub id: Uuid,
    pub title: String,
    pub slug_path: String,
}

/// One row in the similar-paragraph picker modal.
#[derive(Debug, Clone)]
pub(crate) struct SimilarPickerEntry {
    pub id: Uuid,
    pub title: String,
    pub slug_path: String,
    pub score: f64,
    pub snippet: String,
}

/// One aligned row in the snapshot-diff view. `left_*` holds the
/// snapshot side (or `None` for an addition); `right_*` holds the
/// current-buffer side (or `None` for a deletion).
#[derive(Debug, Clone)]
pub(crate) struct SnapshotDiffRow {
    pub left: Option<String>,
    pub right: Option<String>,
    pub kind: SnapshotDiffKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SnapshotDiffKind {
    /// Same line on both sides.
    Equal,
    /// Snapshot had it, buffer dropped it.
    Removed,
    /// Buffer added it.
    Added,
    /// Both sides have a line at this position but they differ.
    Changed,
}

/// One page of a rendered paragraph kept in the preview modal —
/// just enough state for ratatui-image to repaint it and for the
/// title bar to show "page N/M · width×height".
struct RenderedPageProto {
    proto: ratatui_image::protocol::StatefulProtocol,
    width: u32,
    height: u32,
}

/// Save-mode for the SaveRenderedPng picker. Drives whether one
/// page or every page lands on disk.
#[derive(Debug, Clone)]
enum PagesToSave {
    /// Single page at the given 0-based index. File path is the
    /// user's input verbatim.
    Single(usize),
    /// Every page. The user's input is the *base* path
    /// (`/path/to/render` or `/path/to/render.png`); inkhaven
    /// inserts `-page-NNN` before the `.png` extension and
    /// writes one file per page.
    All,
}

enum Modal {
    None,
    Adding {
        kind: NodeKind,
        parent_id: Option<Uuid>,
        parent_label: String,
        input: TextInput,
        /// Where in the parent's children list the new node lands.
        position: InsertPosition,
    },
    Deleting {
        root_id: Uuid,
        root_kind: NodeKind,
        title: String,
        descendant_count: usize,
        ids: Vec<Uuid>,
    },
    Renaming {
        node_id: Uuid,
        kind: NodeKind,
        input: TextInput,
    },
    /// Ctrl+Z E — one-shot Bund eval. The user types an
    /// expression; Enter runs it against Adam and pops the
    /// result onto the status bar. Esc cancels.
    BundEval {
        input: TextInput,
    },
    /// Generic input modal opened by the `ink.input` Bund word.
    /// Shows `prompt`, reads a single line of text, and on Enter
    /// fires the named hook with the typed string on the stack
    /// (`scripting::hooks::fire(hook, [Value::String])`). Esc
    /// closes without firing. Hook-driven rather than synchronous
    /// because a blocking modal would freeze autosave + inference
    /// polling for the duration of the prompt.
    BundInput {
        prompt: String,
        input: TextInput,
        hook: String,
    },
    /// Floating Bund output pane. Opened by `ink.pane.show`,
    /// receives every subsequent `print`/`println` output until
    /// `ink.pane.close` (or Esc). While this is open, the print
    /// buffer that normally drains to the status bar is bypassed
    /// and text lands here instead — letting scripts emit long /
    /// multi-line output without clobbering the status line.
    BundPane {
        title: String,
        lines: Vec<String>,
        scroll: usize,
    },
    /// Ctrl+Z ? — pick + execute a Bund script. `scope`
    /// switches between the cursor's containing branch and the
    /// global `Scripts` system book via the `A` key.
    ScriptPicker {
        scope: ScriptPickerScope,
        entries: Vec<ScriptPickerEntry>,
        cursor: usize,
        scroll: usize,
    },
    /// Ctrl+V G — writing-progress overview. Renders the cached
    /// snapshot (today/streak/per-book/status ladder) plus a
    /// 30-day sparkline. Read-only; refresh on open.
    Progress {
        scroll: usize,
    },
    /// Ctrl+V T — set / clear the per-paragraph word-count goal.
    /// Empty or `0` clears the target. Lives in the same input-
    /// modal family as BundEval / HelpQuery.
    ParagraphTarget {
        input: TextInput,
    },
    /// Ctrl+V 1/2 save-as modal (1.2.4+). Pre-filled with the
    /// default markdown destination — Enter writes; Esc cancels.
    /// `body` is the markdown bytes computed before the modal
    /// opened; `label` is the human-readable name used for any
    /// fallback default path computation.
    SaveMarkdown {
        input: TextInput,
        body: String,
        label: String,
    },
    /// Ctrl+V L (1.2.4+) — linked-paragraphs floating modal.
    /// Lists the open paragraph's outgoing `linked_paragraphs`
    /// metadata entries. `D` on a row removes the link.
    LinkPicker {
        owner: Uuid,
        entries: Vec<ScriptPickerEntry>,
        cursor: usize,
        scroll: usize,
    },
    /// Ctrl+V K (1.2.4+) — backlinks floating modal. Reverse of
    /// LinkPicker: lists paragraphs whose `linked_paragraphs`
    /// contains `target`. `D` removes the source's outgoing
    /// link to `target` (mutates the source paragraph).
    BacklinkPicker {
        target: Uuid,
        entries: Vec<ScriptPickerEntry>,
        cursor: usize,
        scroll: usize,
    },
    /// Ctrl+V M (1.2.4+) — bookmark picker. Lists every
    /// paragraph with `bookmark = true`. Enter opens; D
    /// clears the bookmark flag.
    BookmarkPicker {
        entries: Vec<ScriptPickerEntry>,
        cursor: usize,
        scroll: usize,
    },
    /// Ctrl+V P (1.2.4+) — fuzzy paragraph picker. The
    /// `entries` field is pre-computed from every paragraph
    /// node; the input box narrows the visible list as the
    /// user types.
    FuzzyParagraphPicker {
        input: TextInput,
        entries: Vec<ScriptPickerEntry>,
        cursor: usize,
        scroll: usize,
    },
    /// F6 picker → `V` opens a two-pane diff of the cursor's
    /// snapshot against the open paragraph's current buffer.
    /// Read-only; Esc returns to the snapshot picker.
    SnapshotDiff {
        paragraph_title: String,
        when: String,
        /// Aligned line pairs: `(left_label, left_text,
        /// right_label, right_text, kind)` per row. `kind`
        /// drives the row colour.
        rows: Vec<SnapshotDiffRow>,
        scroll: usize,
        /// Stashed snapshot picker we came from, so `Esc`
        /// restores it intact instead of closing both layers.
        return_to: Box<Modal>,
    },
    /// Ctrl+V S — pick a paragraph similar to the current buffer.
    /// Result list comes from the vector index seeded with the
    /// current paragraph's text; entries always exclude the
    /// current paragraph itself.
    SimilarPicker {
        entries: Vec<SimilarPickerEntry>,
        cursor: usize,
        scroll: usize,
    },
    FilePicker(FilePicker),
    /// Ctrl+H quick reference. Pane-aware: content is fetched from
    /// `quickref::entries_for(focus_when_opened)`.
    QuickRef {
        focus: Focus,
        scroll: usize,
    },
    /// Ctrl+B V — version, author, and credits panel. Scrollable.
    /// Content is rendered fresh each frame so it picks up the current
    /// `CARGO_PKG_VERSION` / `CARGO_PKG_AUTHORS` env vars.
    ///
    /// 1.2.5+: optional `logo` `StatefulProtocol` rendered as a
    /// banner above the text — populated from the embedded
    /// `logo.png` when the host terminal supports ratatui-image
    /// (kitty / iterm2 / sixel / unicode half-blocks). `None`
    /// when image-preview is disabled or the terminal can't
    /// negotiate a graphics protocol.
    Credits {
        scroll: usize,
        logo: Option<ratatui_image::protocol::StatefulProtocol>,
    },
    /// Ctrl+B I — current-book info panel: backup / artefacts paths,
    /// structural counts (chapters / subchapters / paragraphs /
    /// sentences / words), reading-time estimate, and rendered-PDF
    /// status. Content is recomputed each frame so the figures stay
    /// fresh as the user edits.
    BookInfo {
        scroll: usize,
    },
    /// Ctrl+B L — pick a different `llm.default` provider from the set
    /// configured in inkhaven.hjson. On commit we rewrite just the
    /// `default:` line of the HJSON file in place so user comments and
    /// the rest of the config survive.
    LlmPicker {
        providers: Vec<String>,
        cursor: usize,
        initial_default: String,
    },
    /// Ctrl+B P fired with the cursor inside `#image("…")`: pick a
    /// sibling Image node to insert. Filename gets inserted at the
    /// cursor (plus a closing `"` when the call had none).
    ImagePicker {
        entries: Vec<ImagePickerEntry>,
        cursor: usize,
        /// Tells `commit_image_picker` whether to append a `"` after
        /// the filename — true when the `#image(` call was unclosed.
        close_quote: bool,
    },
    /// Enter-on-Image preview using ratatui-image. The `proto` is a
    /// resize-aware StatefulProtocol scoped to one image; it's
    /// re-encoded each frame against the modal's current rect so a
    /// terminal resize Just Works. None when the picker isn't
    /// available — caller falls back to the status-line info path.
    ImagePreview {
        title: String,
        fs_rel: String,
        size_bytes: u64,
        proto: ratatui_image::protocol::StatefulProtocol,
    },
    /// Ctrl+V R (1.2.5+) — float a rasterised PNG of the open
    /// paragraph on top of the editor. `pages` holds one
    /// ratatui-image protocol per page of the compiled
    /// document; `current_page` selects which one renders.
    /// `body` + `settings` are captured so an `S` or `A`
    /// keypress can re-render at high DPI without re-prompting.
    RenderedPreview {
        title: String,
        body: String,
        settings: crate::typst_world::WorldSettings,
        pages: Vec<RenderedPageProto>,
        current_page: usize,
    },
    /// `S` (current page) or `A` (all pages) from
    /// `RenderedPreview` — save-as path picker for the full-DPI
    /// PNG(s). Enter writes the file(s); Esc restores the
    /// underlying `RenderedPreview` so navigation state survives
    /// a cancelled save.
    SaveRenderedPng {
        input: TextInput,
        body: String,
        settings: crate::typst_world::WorldSettings,
        title: String,
        pages: PagesToSave,
        /// Stash of the underlying preview modal so Esc returns
        /// to it. Same `return_to: Box<Modal>` pattern the
        /// snapshot-diff picker uses.
        return_to: Box<Modal>,
    },
    /// Ctrl+B F (editor pane) — Typst function picker. The filter
    /// input narrows the baked-in list as the user types; Enter
    /// inserts `#<name>(|)` at the cursor with the editor cursor
    /// positioned between the parens (Phase 1 = markup-mode default).
    FunctionPicker {
        filter: TextInput,
        cursor: usize,
    },
    /// Ctrl+F in AI-fullscreen — query string entry for the chat-
    /// history search. Enter commits the query into
    /// `App::chat_search`; Esc cancels with no search.
    ChatSearchPrompt {
        input: TextInput,
    },
    /// Ctrl+B 1..7 — list paragraphs whose `status` matches the
    /// chord's target value (1 = Ready, 2 = Final, …, 7 = None),
    /// scoped to the tree cursor's enclosing branch (or the whole
    /// project when the cursor sits at the root). Actions inside the
    /// modal:
    ///   Enter → jump tree cursor + open the paragraph
    ///   r / R → cycle the highlighted paragraph's status forward
    ///           (if it no longer matches, the row disappears from
    ///           the list and the next one slides up)
    ///   - / Backspace → cycle status backward
    StatusFilter {
        status_label: &'static str,
        scope: String,
        entries: Vec<StatusFilterEntry>,
        cursor: usize,
    },
    /// F1 help-manual query. Asks a free-form question against the Help
    /// system book (RAG), then streams the constrained LLM answer into the
    /// AI pane. Esc cancels; Enter submits.
    HelpQuery {
        input: TextInput,
    },
    /// Find / replace prompt. `replace` is None for Ctrl+F search-only mode
    /// and Some for Ctrl+R replace mode (Tab switches focus between the two
    /// input fields when present).
    FindReplace {
        search_input: TextInput,
        replace_input: Option<TextInput>,
        focus_replace: bool,
    },
    SnapshotPicker {
        /// Kept for potential refresh ops after future snapshot mutations.
        #[allow(dead_code)]
        paragraph_id: Uuid,
        paragraph_title: String,
        snapshots: Vec<Snapshot>,
        cursor: usize,
    },
}

pub(crate) struct App {
    layout: ProjectLayout,
    store: Store,
    keymap: Keymap,
    cfg: Config,
    ai: AiClient,
    prompts: PromptLibrary,

    hierarchy: Hierarchy,
    rows: Vec<(Uuid, usize)>,
    /// Branches whose children are hidden in the tree pane. The branch
    /// itself stays visible; only its subtree is collapsed. Left arrow adds
    /// to this set, Right removes from it.
    collapsed_nodes: std::collections::HashSet<Uuid>,
    /// True after the user pressed the meta-prefix chord (default Ctrl+B).
    /// The next key is interpreted as an action selector and clears this.
    meta_pending: bool,
    /// True for one keystroke after the user presses Ctrl+V. The
    /// next key picks an export variant — `1` / `2` whose meaning
    /// depends on the current focus (editor: buffer / subchapter;
    /// tree: current node + descendants). All variants produce
    /// markdown and write to the launch cwd with a deterministic
    /// stem so the user can find the file without a save dialog.
    view_pending: bool,
    /// True after the user pressed the Bund-meta prefix (default
    /// Ctrl+Z). The next key dispatches into `handle_bund_action`
    /// (R run, E eval, N new script).
    bund_pending: bool,
    modal: Modal,

    focus: Focus,
    tree_cursor: usize,
    tree_scroll: usize,

    search_input: TextInput,
    ai_input: TextInput,
    /// Sent AI prompts in chronological order (oldest first).
    /// Up/Down in the AI prompt walks this list when no prompt
    /// picker is showing. Cleared on every send via push_back.
    /// 1.2.4+.
    ai_prompt_history: Vec<String>,
    /// Cursor into `ai_prompt_history`. None when not navigating;
    /// `Some(i)` when the user is stepping through history. Any
    /// edit (typing, backspace, etc.) clears it so the next Up
    /// arrow starts at the end of the list again.
    ai_prompt_history_cursor: Option<usize>,

    opened: Option<OpenedDoc>,
    /// "Similar paragraphs" mode: when `Some`, a second
    /// paragraph is loaded side-by-side with `opened`. The right
    /// editor pane (which normally holds the AI pane) is
    /// repurposed to render this doc. Set by the SimilarPicker
    /// modal; cleared by re-pressing Ctrl+V S, which first saves
    /// both buffers.
    ///
    /// `self.opened` always carries the **focused** doc — Tab in
    /// similar mode swaps `opened` ↔ `secondary` so the existing
    /// editor key handlers keep working unchanged. The
    /// `secondary_in_left_pane` flag tells the renderer which
    /// physical pane currently holds `opened`.
    secondary: Option<OpenedDoc>,
    /// Cached writing-progress snapshot, refreshed on every save +
    /// on project open. Status-bar widget reads from this cache so
    /// per-frame redraws don't trigger a hierarchy walk. `None`
    /// means "progress disabled / not yet computed".
    progress_cache: Option<crate::progress::ProgressSnapshot>,
    /// "Select paragraph to link" mode (1.2.4+). When Some, the
    /// tree pane shows a custom title and `Enter` on a paragraph
    /// links it to the owning UUID stashed here. Esc / tree-focus
    /// loss exits the mode and restores normal Enter semantics.
    ///
    /// Direction:
    /// * `Outgoing` — the Ctrl+V A flow. Open paragraph
    ///   `linked_paragraphs` gains the tree-picked target.
    /// * `Incoming` — the Ctrl+V I flow. The tree-picked
    ///   paragraph's `linked_paragraphs` gains the open one
    ///   (i.e. creates a link FROM the picked paragraph TO
    ///   current).
    link_pick_for: Option<(Uuid, LinkPickDirection)>,
    /// Multi-select set in the tree pane (1.2.4+). Toggled by
    /// `Space` on a row; when non-empty, `Ctrl+B R` (cycle
    /// status) and `Ctrl+V T` (set target) operate on every
    /// marked paragraph instead of just the cursor's. Cleared
    /// by `Esc` in the tree.
    tree_marked: std::collections::HashSet<Uuid>,
    /// In similar-paragraph mode, which pane has keyboard focus.
    /// `false` (default) → left pane = `self.opened` is the
    /// keyboard target. `true` → right pane = `self.secondary`
    /// is the target. Tab inside `Focus::Editor` flips this flag.
    /// All existing editor handlers continue to read/write
    /// `self.opened`; routing happens via a swap performed at
    /// the key-dispatch + save boundaries.
    secondary_focused: bool,
    status: String,
    show_results_overlay: bool,
    results: Vec<SearchHit>,
    results_cursor: usize,

    /// System clipboard handle. May be None on headless systems or when init
    /// fails; in that case copy/cut/paste use tui-textarea's internal yank
    /// buffer only.
    clipboard: Option<arboard::Clipboard>,

    highlighter: TypstHighlighter,
    /// Decoded ratatui colours for the active theme; built once at startup
    /// from `cfg.theme`. Read everywhere the renderer used to hard-code
    /// `Color::Cyan` / `Color::DarkGray` / etc.
    theme: super::theme::Theme,

    /// Place/Character names recompiled into regexes for the editor highlight
    /// overlay. Rebuilt after every save and at startup. None means an empty
    /// lexicon — render path skips work.
    lexicon: super::lexicon::Lexicon,

    inference: Option<Inference>,
    show_prompt_picker: bool,
    prompt_picker_cursor: usize,

    /// Saved (cursor_row, cursor_col, scroll_row, scroll_col) per paragraph
    /// UUID. Updated when the editor loses focus, when the user switches
    /// paragraphs, and at exit. Loaded from `.session.json` on startup so
    /// positions survive across runs.
    paragraph_cursors: std::collections::HashMap<Uuid, ParagraphCursor>,

    /// Cumulative AI chat history for the current session. Each prompt the
    /// user sends from the AI prompt bar appends a User turn, and the
    /// resulting assistant response (when streaming finishes) appends an
    /// Assistant turn. The full history is replayed back to the model on
    /// every follow-up so the conversation is continuous. Cleared by F9 or
    /// the meta-prefix Ctrl+B C.
    ///
    /// Help (F1 / `Help!`) inferences are deliberately *not* added here —
    /// they're one-shot RAG flows with a separate strict system prompt and
    /// don't benefit from carrying chat context.
    chat_history: Vec<ChatTurn>,
    /// User-supplied system-prompt override set via the
    /// `ink.ai.set_system_prompt` Bund stdlib word. When `Some`,
    /// every AI inference (start_inference / start_help_inference
    /// / grammar check) uses this string instead of the
    /// inference-mode-derived default. Cleared by passing empty
    /// string. Volatile — not persisted to HJSON.
    system_prompt_override: Option<String>,
    /// Captures the user message of the currently-streaming chat inference
    /// so we can record the matching Assistant turn into `chat_history`
    /// once the stream finishes. None during one-shot (Help) inferences.
    pending_chat_user_msg: Option<String>,

    /// RAG context block (e.g. a place/character lookup) that the next
    /// AI-prompt submission should prepend to the user's typed query.
    /// Used by the Ctrl+B P / Ctrl+B C editor flows when the AI prompt is
    /// empty: the context is stashed, focus jumps to the AI prompt so the
    /// user can type their question, and `start_inference` lifts the
    /// prefix on Enter.
    pending_rag_prefix: Option<String>,

    /// Per-pane rectangles cached from the most recent `draw()` so the
    /// mouse handler can map a click coordinate to the right pane. Empty
    /// `Rect`s before the first frame is drawn — every handler checks
    /// `contains` so a click during that window safely no-ops.
    layout_search: Rect,
    layout_tree: Rect,
    layout_editor: Rect,
    layout_ai: Rect,
    layout_ai_prompt: Rect,

    /// Active AI "scope" picker. Cycled with F9. When set to anything but
    /// `None`, the next AI-prompt submission prepends the matching context
    /// (selection text, paragraph body, subchapter / chapter / book
    /// concatenation) to the user's query, then auto-resets to `None`.
    ai_mode: AiMode,

    /// How aggressively the model may draw on its own knowledge. Toggled
    /// globally by F10. Help inferences pin this to `Local` regardless of
    /// the current value (see `start_help_inference`).
    inference_mode: InferenceMode,

    /// Set by `commit_file_pick` when the user picks a directory to
    /// import. The main loop picks this up, renders a progress splash,
    /// and runs the (synchronous) import with periodic redraws. None
    /// during normal operation.
    pending_import: Option<std::path::PathBuf>,

    /// Set by Ctrl+B A (`schedule_assembly`). Main loop drives the
    /// synchronous book-assembly procedure with a progress splash.
    pending_assembly: Option<Uuid>,

    /// Set by Ctrl+B B — run assembly + `typst compile`, surface
    /// errors via a fresh AI chat tuned for typst diagnostics.
    pending_build: Option<Uuid>,

    /// Set by Ctrl+B O — Ctrl+B B + copy the resulting PDF into the
    /// inkhaven launch cwd with a timestamped filename.
    pending_take: Option<Uuid>,

    /// Typewriter-style SFX (Enter key + editor-pane focus-out). None
    /// when the host has no audio device — every play call then
    /// silently no-ops, so a headless / SSH-without-audio session
    /// behaves identically to a desktop with `sound.enabled = false`.
    sound: Option<super::sound::SoundPlayer>,

    /// Cached ratatui-image Picker — queried once at startup from the
    /// host terminal. None when the query failed (CI, weird ssh
    /// session, terminals without ANSI graphics support); preview
    /// then falls back to the status-bar info line. Also None when
    /// `images.preview_enabled = false` regardless of capability.
    image_picker: Option<ratatui_image::picker::Picker>,

    /// Ctrl+B W toggles full-screen "typewriter mode" — the editor
    /// pane expands to the whole terminal, every other pane (search
    /// bar, tree, AI, AI prompt, status bar) is hidden. The same
    /// chord returns to the normal layout.
    typewriter_mode: bool,

    /// Ctrl+B K toggles full-screen AI mode — left half = the live
    /// AI pane (streaming inference), right half = scrolling chat
    /// history, bottom = AI prompt. Same chord returns to the
    /// normal layout. Mutually exclusive with `typewriter_mode`.
    ai_fullscreen: bool,

    /// Extra lines to scroll the chat-history pane UP from its auto-
    /// bottom-pin. PageUp adds, PageDown subtracts; the value is
    /// clamped against the total line count so over-scrolling stops
    /// at the top of the history. Reset to 0 each time a new user
    /// message is sent so the streaming reply is visible.
    chat_history_scroll: usize,

    /// Active chat-history search state (Ctrl+F in AI-fullscreen).
    /// While Some, matching lines render with a highlight bg and
    /// the renderer scrolls so the `current` match lands in the
    /// middle of the pane. Ctrl+X advances toward older matches
    /// (the spec: "Start from bottom, going up to older").
    chat_search: Option<ChatSearchState>,

    /// Active chat-selection mode (Ctrl+C in AI-fullscreen). `Some`
    /// when the user is selecting a turn block. Up / Down navigate;
    /// `C` copies the selected turn to the system clipboard, `T`
    /// inserts it into the editor buffer at the cursor.
    chat_selection: Option<ChatSelectionState>,
}

#[derive(Debug)]
struct Inference {
    provider: String,
    /// Kept for diagnostics on the Debug impl; not displayed in the UI.
    #[allow(dead_code)]
    model: String,
    response: String,
    status: InferenceStatus,
    rx: tokio::sync::mpsc::UnboundedReceiver<StreamMsg>,
    started_at: std::time::Instant,
}

#[derive(Debug, Clone)]
enum InferenceStatus {
    Streaming,
    Done,
    Error(String),
}

/// One row in the `Ctrl+B P`-while-inside-`#image(...)` picker. The
/// `fname` is what gets inserted at the cursor — already in
/// `NN-slug.<ext>` form (Node::fs_name).
#[derive(Debug, Clone)]
struct ImagePickerEntry {
    fname: String,
    title: String,
    size_bytes: u64,
}

/// Active search session inside the AI-fullscreen chat-history pane.
/// `matches` is recomputed lazily by `draw_chat_history` whenever the
/// rendered line count changes (terminal resize) — we just track the
/// query + which match we're currently centred on.
#[derive(Debug, Clone)]
struct ChatSearchState {
    query: String,
    /// Index into `matches`. The render hook clamps this against the
    /// freshly-computed match count each frame so terminal resize +
    /// streaming-token arrival can't push it out of range.
    current: usize,
}

/// "Chat selection mode" (Ctrl+C in AI-fullscreen). The cursor
/// points at a single turn in `chat_history`; Up / Down step through
/// turns, `c` / `C` copies the turn text to the clipboard, `t` / `T`
/// inserts it at the editor cursor.
#[derive(Debug, Clone, Copy)]
struct ChatSelectionState {
    /// Index into `chat_history`. Always points at a valid turn —
    /// reset / clamped if the history shrinks while selection is
    /// active.
    turn: usize,
}

/// One row in the `Ctrl+B 1..7` status-filter list. Carries the
/// paragraph id (for opening on Enter) plus a pre-rendered
/// breadcrumb so the user can disambiguate same-titled paragraphs
/// across chapters at a glance.
#[derive(Debug, Clone)]
struct StatusFilterEntry {
    id: Uuid,
    title: String,
    breadcrumb: String,
}

struct OpenedDoc {
    id: Uuid,
    title: String,
    rel_path: String,
    textarea: TextArea<'static>,
    dirty: bool,
    /// Custom scroll state. tui-textarea v0.7 does not expose its viewport, so
    /// we maintain our own and never call `textarea.scroll()`.
    scroll_row: usize,
    scroll_col: usize,
    /// Anchor of a vertical-block selection (entered with Alt+arrows).
    /// While Some, the cursor's current position plus this anchor define a
    /// rectangular selection drawn with REVERSED style.
    block_anchor: Option<(usize, usize)>,
    /// Wall-clock of the last key event handled by the editor. Idle autosave
    /// fires when (now - last_activity) >= editor.autosave_seconds.
    last_activity: std::time::Instant,
    /// Snapshot of `textarea.lines()` at the most recent save / load. Used to
    /// bold characters added since then.
    saved_lines: Vec<String>,
    /// Set when split-edit mode is active. The lower pane shows a read-only
    /// copy of `snapshot_lines`, scrolled independently of the live editor.
    split: Option<SplitView>,
    /// Active find / replace session (Ctrl+F / Ctrl+R). While Some, matches
    /// are highlighted red and Ctrl+G advances or replaces.
    search: Option<SearchState>,
    /// True when this paragraph lives inside the Help book. The editor still
    /// renders it normally (so the user can read it, scroll, search), but
    /// every mutating keystroke is intercepted with a status message.
    read_only: bool,
    /// Picked from the Node's `content_type` at open time. Drives
    /// which syntax highlighter the editor uses (`"hjson"` → the
    /// hand-rolled HJSON lexer; anything else → tree-sitter-typst).
    /// Also reported in the editor header so the user can tell at a
    /// glance which language they're editing.
    content_type: Option<String>,
    /// Pre-correction baseline captured when the AI pane's `T` (grammar-
    /// check apply) overwrites the buffer with the model's corrected text.
    /// Lines that differ from this baseline render in `theme.grammar_change_fg`
    /// so the user can eyeball what changed. Cleared on the next save
    /// (implicit "accept the corrections") or when the user opens a
    /// different paragraph.
    correction_baseline: Option<Vec<String>>,
    /// Cached typst parse-time diagnostics (1.2.5+). Recomputed on
    /// save and on idle when `typst_compile.diagnostics` is on and
    /// the buffer's content type is `None` (default = typst) or
    /// `Some("typst")`. Empty when the buffer parses cleanly OR
    /// when diagnostics are disabled in HJSON. See
    /// `crate::typst_check`.
    typst_diagnostics: Vec<crate::typst_check::TypstDiagnostic>,
    /// Wall-clock of the last typst-syntax recheck. Throttles the
    /// idle re-check against `typst_compile.diagnostics_idle_seconds`.
    typst_diagnostics_checked_at: std::time::Instant,
}

struct SplitView {
    snapshot_lines: Vec<String>,
    scroll_row: usize,
}

impl App {
    fn new(layout: ProjectLayout, cfg: Config, store: Store) -> Result<Self> {
        let keymap = Keymap::from_config(&cfg).map_err(anyhow::Error::from)?;
        // Build the chord-action table from the user's HJSON
        // overlay. Defaults first, then `keys.bindings` rewrites.
        // Install into the process-wide slot so `ink.key.*` Bund
        // stdlib words can mutate the same source of truth the
        // App reads from on every chord dispatch.
        let overrides: Vec<(String, String, Option<String>)> = cfg
            .keys
            .bindings
            .iter()
            .map(|b| (b.chord.clone(), b.action.clone(), b.scope.clone()))
            .collect();
        let keys = super::keybind::KeyBindings::from_overrides(
            keymap.meta_prefix,
            keymap.bund_prefix,
            keymap.view_prefix,
            &overrides,
        )
        .map_err(|e| Error::Config(format!("keys.bindings: {e}")))?;
        super::keybind::install(keys);
        let hierarchy = Hierarchy::load(&store).map_err(anyhow::Error::from)?;
        let lexicon = build_lexicon(&hierarchy, &cfg);
        let collapsed_nodes: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
        let rows: Vec<(Uuid, usize)> = hierarchy
            .flatten_with_collapsed(&collapsed_nodes)
            .into_iter()
            .map(|(n, d)| (n.id, d))
            .collect();

        // Background sync: every `sync_interval_seconds` flush the HNSW
        // index (cheap no-op when clean) and force a DuckDB CHECKPOINT
        // against `metadata.db` + `blobs.db` (cheap when WAL is empty).
        // Both ops short-circuit when there's no real work, so the tick
        // can be generous (default 600s). Store is Send+Sync and cheap to
        // clone (Arc inside). 0 disables.
        if cfg.sync_interval_seconds > 0 {
            let store_for_sync = store.clone();
            let interval_secs = cfg.sync_interval_seconds;
            tokio::spawn(async move {
                let period = std::time::Duration::from_secs(interval_secs);
                let mut ticker = tokio::time::interval(period);
                // Skip the immediate first tick (interval fires at t=0).
                ticker.tick().await;
                loop {
                    ticker.tick().await;
                    if let Err(e) = store_for_sync.sync() {
                        tracing::warn!("background sync failed: {e}");
                    }
                    if let Err(e) = store_for_sync.checkpoint() {
                        tracing::warn!("background checkpoint failed: {e}");
                    }
                }
            });
        }

        let ai = AiClient::from_config(&cfg.llm).map_err(anyhow::Error::from)?;

        let prompts_path = layout.prompts_path(&cfg);
        let prompts = if prompts_path.is_file() {
            PromptLibrary::load(&prompts_path).map_err(anyhow::Error::from)?
        } else {
            PromptLibrary::default()
        };

        let theme = super::theme::Theme::from_config(&cfg.theme);
        // Try to claim the default audio device. None on hosts without
        // one — the player then silently no-ops, mirroring
        // `sound.enabled = false`.
        let sound =
            super::sound::SoundPlayer::try_new(cfg.sound.enabled, cfg.sound.volume);
        // Probe the host terminal for graphics-protocol support so the
        // image-preview modal can pick kitty / sixel / iterm2 / half-
        // block. Errors here just disable the preview pane; the rest
        // of the app behaves identically.
        let image_picker = if cfg.images.preview_enabled {
            match ratatui_image::picker::Picker::from_query_stdio() {
                Ok(p) => Some(p),
                Err(e) => {
                    tracing::info!("image preview disabled — terminal probe: {e}");
                    None
                }
            }
        } else {
            None
        };
        Ok(Self {
            layout,
            store,
            keymap,
            cfg,
            ai,
            prompts,
            hierarchy,
            rows,
            modal: Modal::None,
            collapsed_nodes,
            meta_pending: false,
            bund_pending: false,
            view_pending: false,
            focus: Focus::Tree,
            tree_cursor: 0,
            tree_scroll: 0,
            search_input: TextInput::new(),
            ai_input: TextInput::new(),
            ai_prompt_history: Vec::new(),
            ai_prompt_history_cursor: None,
            opened: None,
            secondary: None,
            secondary_focused: false,
            progress_cache: None,
            link_pick_for: None,
            tree_marked: std::collections::HashSet::new(),
            status: String::from(
                "Tab=panes · Enter=open · Ctrl+S=save · Ctrl+B then B/C/S/P add · D delete · ↑/↓ reorder · Ctrl+Q quit",
            ),
            show_results_overlay: false,
            results: Vec::new(),
            results_cursor: 0,
            clipboard: arboard::Clipboard::new().ok(),
            highlighter: TypstHighlighter::new()
                .map_err(|e| anyhow::anyhow!("typst highlighter init: {e}"))?,
            theme,
            lexicon,
            inference: None,
            show_prompt_picker: false,
            prompt_picker_cursor: 0,
            paragraph_cursors: std::collections::HashMap::new(),
            chat_history: Vec::new(),
            system_prompt_override: None,
            pending_chat_user_msg: None,
            pending_rag_prefix: None,
            layout_search: Rect::default(),
            layout_tree: Rect::default(),
            layout_editor: Rect::default(),
            layout_ai: Rect::default(),
            layout_ai_prompt: Rect::default(),
            ai_mode: AiMode::None,
            inference_mode: InferenceMode::Full,
            pending_import: None,
            pending_assembly: None,
            pending_build: None,
            pending_take: None,
            sound,
            image_picker,
            typewriter_mode: false,
            ai_fullscreen: false,
            chat_history_scroll: 0,
            chat_search: None,
            chat_selection: None,
        })
    }

    /// Open the progress store under the project root and seed
    /// today's baselines. Called from `run` after `App::new` and
    /// before the event loop spins up. Failures are logged + the
    /// store stays uninstalled — progress tracking degrades to
    /// "(disabled)" rather than aborting startup.
    pub(crate) fn install_progress(&mut self) {
        if let Err(e) = crate::progress::install(&self.layout.root) {
            tracing::warn!(target: "inkhaven::progress", "install: {e:#}");
            return;
        }
        // Snapshot today's per-book + project-wide totals so
        // `today_words` has a stable reference point. Idempotent
        // per (day, book) — subsequent calls in the same UTC day
        // are silent no-ops.
        let (per_book, project_total) = self.compute_word_totals_now();
        crate::progress::capture_today_baselines(&per_book, project_total);
        self.refresh_progress_cache();
    }

    /// Rebuild `self.progress_cache` from the current hierarchy +
    /// progress store. Cheap enough to call per save (one hierarchy
    /// walk + a handful of small DuckDB selects). The status bar
    /// always reads from the cache so it doesn't pay the cost on
    /// every redraw.
    ///
    /// 1.2.4+: fires two Bund hooks when the snapshot transitions
    /// across a meaningful threshold — see `fire_progress_hooks`.
    pub(crate) fn refresh_progress_cache(&mut self) {
        let (per_book_vec, project_total) = self.compute_word_totals_now();
        let mut per_book: std::collections::HashMap<Uuid, i64> =
            std::collections::HashMap::new();
        let mut book_titles: std::collections::HashMap<Uuid, String> =
            std::collections::HashMap::new();
        let mut book_slugs: std::collections::HashMap<Uuid, String> =
            std::collections::HashMap::new();
        for (id, total) in per_book_vec {
            per_book.insert(id, total);
            if let Some(node) = self.hierarchy.get(id) {
                book_titles.insert(id, node.title.clone());
                book_slugs.insert(id, node.slug.clone());
            }
        }
        let live = crate::progress::LiveTotals {
            per_book,
            project_total,
            book_titles,
            book_slugs,
        };
        let prev = self.progress_cache.clone();
        let snap = crate::progress::snapshot(&self.cfg.goals, &live);
        self.fire_progress_hooks(prev.as_ref(), &snap);
        self.progress_cache = Some(snap);
    }

    /// Diff the previous progress snapshot against the new one
    /// and fire the matching transition hooks.
    ///
    /// * `hook.on_goal_hit ( today_words daily_goal -- )` — fired
    ///   the first time `today_words` crosses `daily_goal` on a
    ///   given day. Doesn't re-fire while still over the line;
    ///   self-resets if the user deletes back below it.
    /// * `hook.on_streak_break ( prev_streak_days -- )` — fired
    ///   when the streak transitions from positive to zero. The
    ///   argument is the streak length immediately before the
    ///   break so a hook can log/notify "you just broke a 12-day
    ///   streak".
    fn fire_progress_hooks(
        &self,
        prev: Option<&crate::progress::ProgressSnapshot>,
        new: &crate::progress::ProgressSnapshot,
    ) {
        // on_goal_hit
        if let Some(goal) = new.project.daily_goal.filter(|n| *n > 0) {
            let prev_today = prev
                .map(|p| p.project.today_words)
                .unwrap_or(0);
            let new_today = new.project.today_words;
            if prev_today < goal && new_today >= goal {
                crate::scripting::hooks::fire(
                    "hook.on_goal_hit",
                    vec![
                        rust_dynamic::value::Value::from_int(new_today),
                        rust_dynamic::value::Value::from_int(goal),
                    ],
                );
            }
        }
        // on_streak_break — only fires when we had a positive
        // streak previously and now we have zero. First-launch
        // (prev is None) doesn't count as a break.
        let prev_streak = prev.map(|p| p.streak.days).unwrap_or(0);
        if prev_streak > 0 && new.streak.days == 0 {
            crate::scripting::hooks::fire(
                "hook.on_streak_break",
                vec![rust_dynamic::value::Value::from_int(prev_streak)],
            );
        }
        // on_active_goal_hit — same transitional semantics as
        // on_goal_hit but against `goals.active_minutes_daily`.
        let active_goal_secs = self.cfg.goals.active_minutes_daily.max(0) * 60;
        if active_goal_secs > 0 {
            let prev_active = prev.map(|p| p.active_seconds_today).unwrap_or(0);
            let new_active = new.active_seconds_today;
            if prev_active < active_goal_secs && new_active >= active_goal_secs {
                crate::scripting::hooks::fire(
                    "hook.on_active_goal_hit",
                    vec![
                        rust_dynamic::value::Value::from_int(new_active),
                        rust_dynamic::value::Value::from_int(active_goal_secs),
                    ],
                );
            }
        }
    }

    /// Walk the hierarchy and compute current per-book + project
    /// word totals. Touches the filesystem (reads paragraph
    /// bodies) — okay to call once at startup; called from the
    /// progress modal too where the cost amortises across an
    /// interactive open.
    pub(crate) fn compute_word_totals_now(
        &self,
    ) -> (Vec<(Uuid, i64)>, i64) {
        use crate::progress::word_count::count_words;
        let mut per_book: std::collections::HashMap<Uuid, i64> =
            std::collections::HashMap::new();
        for (node, _) in self.hierarchy.flatten() {
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            // Skip protected (system / Help) books — they're not
            // user manuscript content.
            let in_user_book = self
                .hierarchy
                .ancestors(node)
                .into_iter()
                .find(|a| a.kind == NodeKind::Book)
                .filter(|b| b.system_tag.is_none());
            let Some(book) = in_user_book else { continue };
            let Some(rel) = node.file.as_ref() else { continue };
            let abs = self.layout.root.join(rel);
            let body = std::fs::read_to_string(&abs).unwrap_or_default();
            let n = count_words(&body);
            *per_book.entry(book.id).or_insert(0) += n;
        }
        let project_total: i64 = per_book.values().sum();
        let per_book_vec: Vec<(Uuid, i64)> = per_book.into_iter().collect();
        (per_book_vec, project_total)
    }

    /// Final HNSW save + DuckDB CHECKPOINT before the App (and its
    /// `Store` handle, and therefore the duckdb connection pool) are
    /// dropped. Called from the exit sequence in `run(&Path)` so the
    /// `.db.wal` files are drained while we can still surface errors
    /// — the pool's own Drop impl would checkpoint implicitly, but
    /// silently.
    /// Evaluate a Bund script with `self` installed as the
    /// active `App`. Lets `ink.editor.* / ink.ai.* / ink.typst.*`
    /// stdlib words reach App state. Pure wrapper around
    /// `scripting::eval` — sets the global ACTIVE_APP slot via
    /// `AppGuard` before invoking, restores on RAII drop.
    pub(crate) fn scripting_eval(
        &mut self,
        code: &str,
    ) -> anyhow::Result<crate::scripting::EvalOutput> {
        let _guard = crate::scripting::AppGuard::enter(self);
        crate::scripting::eval(code)
    }

    fn shutdown_flush(&self) {
        if let Err(e) = self.store.sync() {
            tracing::warn!("shutdown sync failed: {e}");
        }
        if let Err(e) = self.store.checkpoint() {
            tracing::warn!("shutdown checkpoint failed: {e}");
        }
    }

    fn run<B: ratatui::backend::Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            self.pump_inference();
            self.tick_autosave();
            // Drive any deferred directory import — `commit_file_pick`
            // sets `pending_import` so the splash can be drawn directly
            // via the terminal handle (which `commit_file_pick` doesn't
            // own). Runs synchronously: same UX as the backup splash.
            if let Some(root) = self.pending_import.take() {
                self.run_pending_import(terminal, &root);
            }
            // Same pattern for Book assembly (Ctrl+B A): the dispatcher
            // stashes the book uuid, the main loop runs the procedure
            // and drives the splash redraws.
            if let Some(book_id) = self.pending_assembly.take() {
                self.run_pending_assembly(terminal, book_id);
            }
            if let Some(book_id) = self.pending_build.take() {
                self.run_pending_build(terminal, book_id, false);
            }
            if let Some(book_id) = self.pending_take.take() {
                self.run_pending_build(terminal, book_id, true);
            }
            terminal.draw(|f| self.draw(f))?;
            // Shorter poll interval while streaming so tokens render with low
            // latency without burning CPU when idle.
            let timeout = if self.is_streaming() {
                Duration::from_millis(40)
            } else {
                Duration::from_millis(200)
            };
            if event::poll(timeout)? {
                match event::read()? {
                    Event::Key(key) => {
                        if key.kind != KeyEventKind::Press {
                            continue;
                        }
                        if self.handle_key(key)? {
                            return Ok(());
                        }
                    }
                    Event::Mouse(mouse) => self.handle_mouse(mouse),
                    _ => {}
                }
            }
        }
    }

    fn tick_autosave(&mut self) {
        let secs = self.cfg.editor.autosave_seconds;
        if secs == 0 {
            return;
        }
        let due = match self.opened.as_ref() {
            // Suspend idle autosave while a grammar-correction overlay is
            // active so it doesn't disappear under the user's nose. The
            // overlay is dismissed by Ctrl+S (manual save), focus-out, or
            // Ctrl+B C — each of those resumes normal autosave.
            Some(doc) if doc.dirty && doc.correction_baseline.is_none() => {
                doc.last_activity.elapsed().as_secs() >= secs
            }
            _ => false,
        };
        if due {
            let _ = self.save_current();
        }

        // 1.2.4+: also autosave the secondary editor (similar-
        // paragraph mode's right pane). Before this, the secondary
        // doc was flush-only-on-Ctrl+V-S-exit, which surprised
        // users who typed in the right pane and walked away.
        // Same idle-threshold as the primary; no correction-overlay
        // gate since correction overlays only apply to the primary.
        // `Option::take()` pulls the doc out so we can call
        // `save_doc(&mut self, &mut OpenedDoc)` without aliasing
        // self.secondary against itself; the doc goes straight
        // back regardless of save outcome.
        let sec_due = match self.secondary.as_ref() {
            Some(doc) if doc.dirty => doc.last_activity.elapsed().as_secs() >= secs,
            _ => false,
        };
        if sec_due {
            if let Some(mut doc) = self.secondary.take() {
                if let Err(e) = self.save_doc(&mut doc) {
                    tracing::warn!(
                        target: "inkhaven::autosave",
                        "secondary autosave failed: {e}",
                    );
                }
                self.secondary = Some(doc);
            }
        }

        // 1.2.5+: idle typst-syntax recheck. Independent of save
        // — runs whenever the user has paused for
        // `typst_compile.diagnostics_idle_seconds` and the buffer
        // has moved on since the last check. Save itself already
        // calls `refresh_typst_diagnostics_for_opened`, so this
        // covers the "I'm staring at the buffer wondering why
        // typst errored" case where no save has fired yet.
        if self.cfg.typst_compile.diagnostics {
            let idle = self.cfg.typst_compile.diagnostics_idle_seconds;
            let due = match self.opened.as_ref() {
                Some(doc) => {
                    let idle_ok =
                        doc.last_activity.elapsed().as_secs() >= idle;
                    let stale = doc
                        .typst_diagnostics_checked_at
                        .elapsed()
                        .as_secs()
                        >= idle.max(1);
                    idle_ok && stale && doc.dirty
                }
                None => false,
            };
            if due {
                self.refresh_typst_diagnostics_for_opened();
            }
        }
    }

    fn is_streaming(&self) -> bool {
        matches!(
            self.inference.as_ref().map(|i| &i.status),
            Some(InferenceStatus::Streaming)
        )
    }

    fn pump_inference(&mut self) {
        let Some(inf) = self.inference.as_mut() else {
            return;
        };
        if !matches!(inf.status, InferenceStatus::Streaming) {
            return;
        }
        // Track terminal state of this poll so we can commit to chat history
        // exactly once when the stream completes successfully.
        let mut just_finished = false;
        loop {
            match inf.rx.try_recv() {
                Ok(StreamMsg::Token(t)) => inf.response.push_str(&t),
                Ok(StreamMsg::Done) => {
                    inf.status = InferenceStatus::Done;
                    let elapsed = inf.started_at.elapsed();
                    self.status = format!(
                        "{} responded in {:.1}s",
                        inf.provider,
                        elapsed.as_secs_f32()
                    );
                    just_finished = true;
                    break;
                }
                Ok(StreamMsg::Error(e)) => {
                    inf.status = InferenceStatus::Error(e.clone());
                    self.status = format!("inference error: {e}");
                    break;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    // Task ended without a final message — treat as done so
                    // the assistant turn still gets recorded.
                    if matches!(inf.status, InferenceStatus::Streaming) {
                        inf.status = InferenceStatus::Done;
                        just_finished = true;
                    }
                    break;
                }
            }
        }
        if just_finished {
            // Pair the pending user message with this assistant response
            // and append both to chat_history. Help one-shots leave
            // `pending_chat_user_msg = None`, so they're skipped here.
            let assistant_text = self
                .inference
                .as_ref()
                .map(|i| i.response.clone())
                .unwrap_or_default();
            if let Some(user_msg) = self.pending_chat_user_msg.take() {
                if !assistant_text.trim().is_empty() {
                    self.chat_history.push(ChatTurn::User(user_msg));
                    self.chat_history
                        .push(ChatTurn::Assistant(assistant_text));
                }
            }
        }
    }

    // -------- key dispatch ------------------------------------------------

    /// Dispatch a crossterm mouse event. Left-click moves focus to the
    /// clicked pane and positions the cursor inside it where possible.
    /// Scroll wheel scrolls the pane under the pointer. Other kinds
    /// (middle-click, drag, motion) are ignored for now.
    ///
    /// Modals / overlays (file picker, prompt picker, modal stack) eat
    /// mouse input — clicking through a modal feels wrong and the
    /// keyboard flow for those is well-trodden. We early-return if any
    /// modal is up so the click can't accidentally focus a pane that's
    /// hidden behind the floating panel.
    fn handle_mouse(&mut self, ev: MouseEvent) {
        if !matches!(self.modal, Modal::None) {
            return;
        }
        if self.show_results_overlay || self.show_prompt_picker {
            return;
        }
        let (col, row) = (ev.column, ev.row);
        let pane = self.pane_at(col, row);
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(target) = pane {
                    if self.focus != target {
                        self.change_focus(target);
                    }
                    match target {
                        Focus::Tree => self.mouse_position_tree(row),
                        Focus::Editor => self.mouse_position_editor(col, row),
                        _ => {}
                    }
                }
            }
            MouseEventKind::ScrollUp => match pane {
                Some(Focus::Tree) => self.move_cursor(-3),
                Some(Focus::Editor) => self.mouse_scroll_editor(-3),
                _ => {}
            },
            MouseEventKind::ScrollDown => match pane {
                Some(Focus::Tree) => self.move_cursor(3),
                Some(Focus::Editor) => self.mouse_scroll_editor(3),
                _ => {}
            },
            _ => {}
        }
    }

    /// Map a terminal coordinate to the pane that owns it, if any. Uses
    /// the rectangles cached by the most recent `draw()`.
    fn pane_at(&self, col: u16, row: u16) -> Option<Focus> {
        if rect_contains(self.layout_tree, col, row) {
            return Some(Focus::Tree);
        }
        if rect_contains(self.layout_editor, col, row) {
            return Some(Focus::Editor);
        }
        if rect_contains(self.layout_ai, col, row) {
            return Some(Focus::Ai);
        }
        if rect_contains(self.layout_search, col, row) {
            return Some(Focus::SearchBar);
        }
        if rect_contains(self.layout_ai_prompt, col, row) {
            return Some(Focus::AiPrompt);
        }
        None
    }

    /// Move the tree cursor to whatever row was clicked. Accounts for
    /// the 1-row top border and the current scroll offset.
    fn mouse_position_tree(&mut self, row: u16) {
        if self.rows.is_empty() {
            return;
        }
        let body_y = self.layout_tree.y.saturating_add(1);
        if row < body_y {
            return;
        }
        let row_in_view = (row - body_y) as usize;
        let idx = self.tree_scroll + row_in_view;
        if idx < self.rows.len() {
            self.tree_cursor = idx;
        }
    }

    /// Position the editor cursor under the click. Accounts for the
    /// pane border (1 row top, 1 col left), the line-number gutter, and
    /// the current scroll. Wrapped-line clicks degrade gracefully by
    /// snapping to the closest source row.
    fn mouse_position_editor(&mut self, col: u16, row: u16) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        let inner_x = self.layout_editor.x.saturating_add(1);
        let inner_y = self.layout_editor.y.saturating_add(1);
        if row < inner_y || col < inner_x {
            return;
        }
        // Gutter width: digits in line count + 1 trailing space (see
        // `digit_count` + `gutter_width` in the editor renderer).
        let total_lines = doc.textarea.lines().len().max(1);
        let lineno_chars = digit_count(total_lines);
        let gutter = (lineno_chars + 1) as u16;
        let inner_cursor_x = inner_x.saturating_add(gutter);
        // Click inside the gutter or borders → ignore.
        if col < inner_cursor_x {
            return;
        }
        let rel_row = (row - inner_y) as usize;
        let rel_col = (col - inner_cursor_x) as usize;
        let src_row = (doc.scroll_row + rel_row).min(total_lines - 1);
        let line_len = doc
            .textarea
            .lines()
            .get(src_row)
            .map_or(0, |s| s.chars().count());
        let src_col = (doc.scroll_col + rel_col).min(line_len);
        doc.textarea
            .move_cursor(tui_textarea::CursorMove::Jump(src_row as u16, src_col as u16));
        doc.last_activity = std::time::Instant::now();
    }

    /// Scroll the editor by `delta` source rows (positive = down).
    /// Updates the cursor too so the visible window doesn't snap back on
    /// the next render — the renderer keeps the cursor in view, which
    /// would otherwise undo a pure scroll-only adjustment.
    fn mouse_scroll_editor(&mut self, delta: i32) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        let total = doc.textarea.lines().len().max(1);
        if delta < 0 {
            let n = (-delta) as usize;
            doc.scroll_row = doc.scroll_row.saturating_sub(n);
            use tui_textarea::CursorMove;
            for _ in 0..n {
                doc.textarea.move_cursor(CursorMove::Up);
            }
        } else {
            let n = delta as usize;
            doc.scroll_row = (doc.scroll_row + n).min(total.saturating_sub(1));
            use tui_textarea::CursorMove;
            for _ in 0..n {
                doc.textarea.move_cursor(CursorMove::Down);
            }
        }
        doc.last_activity = std::time::Instant::now();
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        // Hard quit works from anywhere, including inside a modal.
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
        {
            return Ok(self.request_quit());
        }

        // Modal eats every other key.
        if !matches!(self.modal, Modal::None) {
            return self.handle_modal_key(key);
        }

        // Ctrl+1..5 — direct focus jumps. Most terminals send these as
        // Char(digit) + CONTROL. But Ctrl+2 on US-layout terminals is often
        // re-encoded as Ctrl+@ → KeyCode::Char('@') with CONTROL, or as
        // KeyCode::Null. Try every variant we've seen in the wild.
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && !key
                .modifiers
                .intersects(KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            let target = match key.code {
                KeyCode::Char('1') => Some(Focus::Editor),
                // Ctrl+2 alternates:
                KeyCode::Char('2') | KeyCode::Char('@') => Some(Focus::Tree),
                KeyCode::Char('3') => Some(Focus::Ai),
                KeyCode::Char('4') => Some(Focus::SearchBar),
                KeyCode::Char('5') => Some(Focus::AiPrompt),
                // Ctrl+T also focuses Tree — mnemonic and not terminal-eaten.
                KeyCode::Char('t') | KeyCode::Char('T') => Some(Focus::Tree),
                _ => None,
            };
            if let Some(focus) = target {
                self.change_focus(focus);
                return Ok(false);
            }
        }
        // KeyCode::Null with no modifiers is what some terminals report for
        // Ctrl+2 / Ctrl+Space. Catch that separately because the inner block
        // requires the CONTROL modifier flag.
        if matches!(key.code, KeyCode::Null) {
            self.change_focus(Focus::Tree);
            return Ok(false);
        }

        // View-prefix dispatch (Ctrl+V by default, 1.2.4+).
        // Captures the next keystroke. Resolution goes through
        // the binding table now (Layer::ViewSub), so HJSON
        // `keys.bindings` + `ink.key.*` can rebind every chord
        // under the prefix.
        if self.view_pending {
            self.handle_view_action(key);
            return Ok(false);
        }
        if let Some(view_prefix) = self.keymap.view_prefix {
            if view_prefix.matches(&key) {
                self.view_pending = true;
                self.status = super::keybind::read().view_hint(self.focus);
                return Ok(false);
            }
        }

        // Bund-meta dispatch (Ctrl+Z by default). Mirrors the
        // meta-prefix machinery below — same state-machine shape,
        // different action table. Intercept BEFORE tui-textarea
        // sees the key so its default Ctrl+Z=undo binding stays
        // dormant.
        if self.bund_pending {
            self.handle_bund_action(key);
            return Ok(false);
        }
        if let Some(bund_prefix) = self.keymap.bund_prefix {
            if bund_prefix.matches(&key) {
                self.bund_pending = true;
                self.status = super::keybind::read().bund_hint(self.focus);
                return Ok(false);
            }
        }

        // Meta-prefix dispatch. If we're already inside meta mode, the next
        // key is the action selector. Otherwise check whether THIS key is
        // the meta prefix and enter the mode.
        if self.meta_pending {
            self.handle_meta_action(key);
            return Ok(false);
        }
        if self.keymap.meta_prefix.matches(&key) {
            self.meta_pending = true;
            // The meta action table is pane-specific (see dispatch_meta_*),
            // so the hint shown in the status bar should match the focused
            // pane. Generic suffix (· H help · Esc cancel) is shared.
            // Build the meta-hint from the live binding table so
            // user overlays (HJSON + ink.key.*) show up in the
            // status bar automatically.
            self.status = super::keybind::read().meta_hint(self.focus);
            return Ok(false);
        }


        // 1.2.4+: F-keys + every top-level (no-prefix) chord
        // flow through the `top_level` binding table. The table
        // is pane-aware via Scope, so F-keys that only made
        // sense in one pane (F2 rename / F3 file picker /
        // F4-F6 editor) keep their per-pane behaviour without
        // hardcoded match arms. The user can rebind any of
        // them via HJSON `keys.bindings` (single-token chord
        // strings route to TopLevel) or runtime
        // `ink.key.bind`.
        if let Some(action) = super::keybind::read().resolve_top_level(&key, self.focus) {
            if !matches!(action, super::keybind::Action::None) {
                self.run_action(action);
                return Ok(false);
            }
        }

        // AI-fullscreen Ctrl+C toggles "Chat selection mode" — the
        // editor's Ctrl+C still does clipboard-copy via the focus
        // dispatch, but in this layout the editor isn't visible and
        // the chord is reclaimed for navigating chat turns.
        if self.ai_fullscreen
            && key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
            && matches!(self.modal, Modal::None)
        {
            self.toggle_chat_selection_mode();
            return Ok(false);
        }

        // When chat-selection is active: Up / Down step turns, `c` /
        // `C` copies to clipboard, `t` / `T` inserts at the editor
        // cursor, Esc / Enter exits.
        if self.ai_fullscreen && self.chat_selection.is_some() && matches!(self.modal, Modal::None) {
            let plain = !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
            match key.code {
                KeyCode::Up if plain => {
                    self.chat_selection_step(-1);
                    return Ok(false);
                }
                KeyCode::Down if plain => {
                    self.chat_selection_step(1);
                    return Ok(false);
                }
                KeyCode::Home if plain => {
                    self.chat_selection_jump(0);
                    return Ok(false);
                }
                KeyCode::End if plain => {
                    self.chat_selection_jump(usize::MAX);
                    return Ok(false);
                }
                KeyCode::Char('c') | KeyCode::Char('C') if plain => {
                    self.chat_selection_copy();
                    return Ok(false);
                }
                KeyCode::Char('t') | KeyCode::Char('T') if plain => {
                    self.chat_selection_into_editor();
                    return Ok(false);
                }
                KeyCode::Esc | KeyCode::Enter => {
                    self.chat_selection = None;
                    self.status = "chat selection mode off".into();
                    return Ok(false);
                }
                _ => {}
            }
        }

        // AI-fullscreen Esc with an active chat search → clear the
        // search (drop highlights + scroll back to the bottom-pin).
        // Routed before the focus handlers so AI-prompt Esc doesn't
        // grab it.
        if self.ai_fullscreen
            && self.chat_search.is_some()
            && matches!(key.code, KeyCode::Esc)
            && matches!(self.modal, Modal::None)
        {
            self.chat_search = None;
            self.chat_history_scroll = 0;
            self.status = "chat search cleared".into();
            return Ok(false);
        }

        // AI-fullscreen Ctrl+F → chat-history search modal. Intercepted
        // before the editor-pane Ctrl+F handler so the chord behaves
        // contextually based on layout, not focus.
        if self.ai_fullscreen
            && key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('f') | KeyCode::Char('F'))
        {
            self.open_chat_search_prompt();
            return Ok(false);
        }
        // Ctrl+X advances to the next (older) match while a chat
        // search is active. No-op when no search is running.
        if self.ai_fullscreen
            && key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('x') | KeyCode::Char('X'))
            && self.chat_search.is_some()
        {
            self.advance_chat_search();
            return Ok(false);
        }

        // AI-fullscreen-only: PageUp / PageDown scroll the chat history
        // pane regardless of which sub-pane has focus (the user is
        // typically focused on the AI prompt). Up / Down do the same
        // one line at a time for fine-grained reading. Intercepted
        // here so the AI prompt's own input handler never sees these
        // keys in this layout. The non-fullscreen path leaves them
        // untouched.
        //
        // Exception: when the `/` prompt-library picker is open, Up /
        // Down still navigate the picker — that's the picker's only
        // way to move the selection.
        if self.ai_fullscreen {
            if self.keymap.page_up.matches(&key) {
                self.chat_history_scroll = self.chat_history_scroll.saturating_add(10);
                return Ok(false);
            }
            if self.keymap.page_down.matches(&key) {
                self.chat_history_scroll = self.chat_history_scroll.saturating_sub(10);
                return Ok(false);
            }
            if !self.show_prompt_picker {
                if matches!(key.code, KeyCode::Up) {
                    self.chat_history_scroll = self.chat_history_scroll.saturating_add(1);
                    return Ok(false);
                }
                if matches!(key.code, KeyCode::Down) {
                    self.chat_history_scroll = self.chat_history_scroll.saturating_sub(1);
                    return Ok(false);
                }
            }
        }

        // F9 / F10 / F7 / F1 et al. now flow through the
        // top_level binding-table dispatch above. The hardcoded
        // match arms were removed in the 1.2.4 F-key migration.

        // Save works from anywhere as long as a doc is open.
        if self.keymap.save.matches(&key) && self.opened.is_some() {
            self.save_current()?;
            return Ok(false);
        }

        // Focus jumps from anywhere.
        if self.keymap.search.matches(&key) {
            self.change_focus(Focus::SearchBar);
            return Ok(false);
        }
        if self.keymap.ai_prompt.matches(&key) {
            self.change_focus(Focus::AiPrompt);
            return Ok(false);
        }

        // Tab cycling everywhere except when typing into a buffer.
        let in_editor_with_doc = self.focus == Focus::Editor && self.opened.is_some();
        let cycling_blocked = self.focus.is_input() || in_editor_with_doc;
        if !cycling_blocked {
            if self.keymap.next_pane.matches(&key) {
                self.change_focus(self.focus.next());
                return Ok(false);
            }
            if self.keymap.prev_pane.matches(&key) {
                self.change_focus(self.focus.prev());
                return Ok(false);
            }
        } else if in_editor_with_doc
            && (self.keymap.next_pane.matches(&key) || self.keymap.prev_pane.matches(&key))
        {
            // Inside an active editor, Tab cycles focus too — but only when
            // the user really meant to (no other modifiers were on). If we
            // didn't intercept here, Tab would insert a literal tab via
            // tui-textarea.
            //
            // Similar-paragraph mode special case: when the AI pane
            // has been replaced by a second editor (`self.secondary
            // .is_some()`), Tab inside the editor pane toggles
            // keyboard focus between the two editor panes instead
            // of cycling to a non-existent AI pane. Shift+Tab does
            // the same — there's only one "other pane" to flip to.
            if self.secondary.is_some() {
                self.secondary_focused = !self.secondary_focused;
                self.status = if self.secondary_focused {
                    "similar: right editor focused".into()
                } else {
                    "similar: left editor focused".into()
                };
                return Ok(false);
            }
            let next = if self.keymap.next_pane.matches(&key) {
                self.focus.next()
            } else {
                self.focus.prev()
            };
            self.change_focus(next);
            return Ok(false);
        }

        match self.focus {
            Focus::Tree => self.handle_tree_key(key),
            Focus::Editor => self.handle_editor_key(key),
            Focus::Ai => self.handle_passive_key(key),
            Focus::SearchBar => self.handle_input_key(key, true),
            Focus::AiPrompt => self.handle_input_key(key, false),
        }
    }

    fn handle_tree_key(&mut self, key: KeyEvent) -> Result<bool> {
        // For plain-letter / punctuation shortcuts we ignore the SHIFT modifier
        // (uppercase letters require Shift on most layouts) but reject Ctrl /
        // Alt / Super so e.g. Ctrl+A doesn't accidentally trigger Add-subchapter.
        let plain = !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') if plain => return Ok(self.request_quit()),
            // Esc cycles Tree → Search bar (third leg of the
            // Editor → Tree → Search → Editor rotation).
            // 1.2.4+: Esc exits link-pick mode (returning to
            // the editor) instead of cycling focus to the
            // search bar. Also clears any multi-select marks
            // so the next chord operates on the cursor row,
            // not stale marks.
            KeyCode::Esc => {
                if self.link_pick_for.is_some() {
                    self.link_pick_for = None;
                    self.status = "link cancelled".into();
                    self.change_focus(Focus::Editor);
                } else if !self.tree_marked.is_empty() {
                    let n = self.tree_marked.len();
                    self.tree_marked.clear();
                    self.status =
                        format!("multi-select cleared ({n} paragraph(s))");
                } else {
                    self.change_focus(Focus::SearchBar);
                }
            }
            // 1.2.4+: Space toggles multi-select on the
            // cursor row (paragraphs only — branches don't
            // accept the bulk operations).
            KeyCode::Char(' ') if plain => {
                if let Some((id, _)) = self.rows.get(self.tree_cursor) {
                    let id = *id;
                    if let Some(node) = self.hierarchy.get(id) {
                        if node.kind == NodeKind::Paragraph {
                            if self.tree_marked.remove(&id) {
                                self.status = format!(
                                    "unmarked · {} marked",
                                    self.tree_marked.len()
                                );
                            } else {
                                self.tree_marked.insert(id);
                                self.status = format!(
                                    "marked · {} marked",
                                    self.tree_marked.len()
                                );
                            }
                        } else {
                            self.status =
                                "multi-select only marks paragraphs".into();
                        }
                    }
                }
            }
            // F2 (rename) and F3 (file picker) now flow through
            // the top_level binding-table dispatch in handle_key.
            KeyCode::Up => self.move_cursor(-1),
            KeyCode::Down => self.move_cursor(1),
            KeyCode::Home => self.tree_cursor = 0,
            KeyCode::End => {
                if !self.rows.is_empty() {
                    self.tree_cursor = self.rows.len() - 1;
                }
            }
            // 1.2.4+: when link-pick mode is active, Enter on a
            // tree row links the row's paragraph to the
            // pick-mode owner rather than opening it for editing.
            KeyCode::Enter => {
                if self.link_pick_for.is_some() {
                    self.commit_link_pick();
                } else {
                    self.open_selected()?;
                }
            }

            // Right arrow: expand cursor's branch (no-op for leaves).
            // Left arrow: collapse cursor's branch if expanded, else move
            // cursor to its parent in the hierarchy. Same semantics as the
            // F3 file picker (§11 in KEYBINDING.md).
            KeyCode::Right => self.tree_expand_at_cursor(),
            KeyCode::Left => self.tree_collapse_or_step_out(),

            // Tree-pane add shortcuts. These exist alongside the global
            // Ctrl+Shift+* chords because terminals and multiplexers
            // commonly eat those (Ctrl+S = XOFF, tmux prefix, etc.).
            KeyCode::Char('B') | KeyCode::Char('b') if plain => {
                self.open_add_modal(NodeKind::Book);
            }
            // C/A/+ append at the end of the parent's children;
            // V/S/P insert immediately after the cursor's same-kind ancestor.
            KeyCode::Char('C') | KeyCode::Char('c') if plain => {
                self.open_add_modal(NodeKind::Chapter);
            }
            KeyCode::Char('V') | KeyCode::Char('v') if plain => {
                self.open_add_modal_after(NodeKind::Chapter);
            }
            KeyCode::Char('A') | KeyCode::Char('a') if plain => {
                self.open_add_modal(NodeKind::Subchapter);
            }
            KeyCode::Char('S') | KeyCode::Char('s') if plain => {
                self.open_add_modal_after(NodeKind::Subchapter);
            }
            KeyCode::Char('+') if plain => {
                self.open_add_modal(NodeKind::Paragraph);
            }
            KeyCode::Char('P') | KeyCode::Char('p') if plain => {
                self.open_add_modal_after(NodeKind::Paragraph);
            }

            // Kind-specific delete: D for branches, - for paragraphs. This is
            // a safety feature — `-` won't nuke a whole chapter by accident.
            KeyCode::Char('D') | KeyCode::Char('d') if plain => self.delete_branch_only(),
            KeyCode::Char('-') if plain => self.delete_paragraph_only(),

            // Sibling reorder, plain-letter form. Equivalent to the meta-
            // prefix chord `Ctrl+B ↑` / `Ctrl+B ↓` but reachable without a
            // chord — handy when reorganising a long list of paragraphs.
            KeyCode::Char('U') | KeyCode::Char('u') if plain => {
                self.move_current(MoveDir::Up);
            }
            KeyCode::Char('J') | KeyCode::Char('j') if plain => {
                self.move_current(MoveDir::Down);
            }

            // Z collapses the cursor's enclosing subchapter; X collapses
            // every expanded branch in the tree. Both rebuild the row list
            // so the view updates immediately.
            KeyCode::Char('Z') | KeyCode::Char('z') if plain => {
                self.collapse_enclosing_subchapter();
            }
            KeyCode::Char('X') | KeyCode::Char('x') if plain => {
                self.collapse_all_branches();
            }

            // 1.2.4+: tree T cycles the type of the cursor row
            // (or every marked paragraph, when multi-select is
            // active). Same ladder as Ctrl+B M:
            // Paragraph(typst) → Paragraph(hjson) → Script(bund).
            KeyCode::Char('T') | KeyCode::Char('t') if plain => {
                if !self.tree_marked.is_empty() {
                    self.cycle_leaf_type_bulk();
                } else {
                    self.cycle_leaf_type();
                }
            }
            // 1.2.4+: tree O cycles paragraph status. Mirrors
            // Ctrl+B R; honours multi-select for bulk status
            // transitions.
            KeyCode::Char('O') | KeyCode::Char('o') if plain => {
                self.cycle_paragraph_status();
            }

            _ if self.keymap.page_up.matches(&key) => self.move_cursor(-10),
            _ if self.keymap.page_down.matches(&key) => self.move_cursor(10),
            _ => {}
        }
        Ok(false)
    }

    fn tree_expand_at_cursor(&mut self) {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            return;
        };
        if node.kind == NodeKind::Paragraph {
            return;
        }
        if self.collapsed_nodes.remove(&id) {
            self.rebuild_rows_preserving_cursor();
        }
    }

    fn tree_collapse_or_step_out(&mut self) {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            return;
        };

        let is_branch = node.kind != NodeKind::Paragraph;
        let has_children = is_branch && self.hierarchy.has_children(id);
        let is_currently_collapsed = self.collapsed_nodes.contains(&id);

        if is_branch && has_children && !is_currently_collapsed {
            self.collapsed_nodes.insert(id);
            self.rebuild_rows_preserving_cursor();
            return;
        }

        // Otherwise step out to parent.
        if let Some(parent_id) = node.parent_id {
            if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == parent_id) {
                self.tree_cursor = i;
            }
        }
    }

    /// Collapse the cursor's enclosing Subchapter. If the cursor is on a
    /// Subchapter itself, collapse it directly. Walks ancestors otherwise;
    /// no-op if no Subchapter is in scope (e.g. cursor on a chapter or
    /// directly under a book). After collapsing, the tree cursor moves to
    /// the now-folded subchapter row so the user sees what happened.
    fn collapse_enclosing_subchapter(&mut self) {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            self.status = "nothing selected".into();
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            return;
        };
        // Pick the cursor's enclosing subchapter — itself if it IS one,
        // otherwise the nearest ancestor of kind Subchapter.
        let target = if node.kind == NodeKind::Subchapter {
            Some(node.id)
        } else {
            self.hierarchy
                .ancestors(node)
                .into_iter()
                .find(|a| a.kind == NodeKind::Subchapter)
                .map(|a| a.id)
        };
        let Some(target_id) = target else {
            self.status = "no enclosing subchapter to collapse".into();
            return;
        };
        if self.collapsed_nodes.insert(target_id) {
            self.rebuild_rows_preserving_cursor();
            // Land the cursor on the freshly-collapsed subchapter row so
            // the user can see what was folded.
            if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == target_id) {
                self.tree_cursor = i;
            }
            let title = self
                .hierarchy
                .get(target_id)
                .map(|n| n.title.as_str())
                .unwrap_or("?");
            self.status = format!("collapsed subchapter `{title}`");
        } else {
            self.status = "subchapter is already collapsed".into();
        }
    }

    /// Collapse every branch that has children. Paragraphs and empty
    /// branches are untouched (they wouldn't render differently anyway).
    /// The tree cursor stays on the same node if it survives the fold;
    /// otherwise `rebuild_rows_preserving_cursor` snaps it to the nearest
    /// remaining visible row.
    fn collapse_all_branches(&mut self) {
        let mut added = 0usize;
        let candidates: Vec<Uuid> = self
            .hierarchy
            .iter()
            .filter(|n| n.kind != NodeKind::Paragraph && self.hierarchy.has_children(n.id))
            .map(|n| n.id)
            .collect();
        for id in candidates {
            if self.collapsed_nodes.insert(id) {
                added += 1;
            }
        }
        if added == 0 {
            self.status = "all branches already collapsed".into();
            return;
        }
        self.rebuild_rows_preserving_cursor();
        self.status = format!("collapsed {added} branch(es)");
    }

    fn rebuild_rows_preserving_cursor(&mut self) {
        let prev_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
        self.rows = self
            .hierarchy
            .flatten_with_collapsed(&self.collapsed_nodes)
            .into_iter()
            .map(|(n, d)| (n.id, d))
            .collect();
        if let Some(id) = prev_id {
            if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == id) {
                self.tree_cursor = i;
                return;
            }
        }
        if !self.rows.is_empty() {
            self.tree_cursor = self.tree_cursor.min(self.rows.len() - 1);
        } else {
            self.tree_cursor = 0;
        }
    }

    fn delete_branch_only(&mut self) {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            self.status = "nothing selected".into();
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            return;
        };
        if node.kind == NodeKind::Paragraph {
            self.status = format!(
                "`{}` is a paragraph — press `-` (or Ctrl+B then D) to delete it",
                node.title
            );
            return;
        }
        self.open_delete_modal();
    }

    fn delete_paragraph_only(&mut self) {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            self.status = "nothing selected".into();
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            return;
        };
        if node.kind != NodeKind::Paragraph {
            self.status = format!(
                "`{}` is a {} — press `D` (or Ctrl+B then D) to delete it",
                node.title,
                node.kind.as_str()
            );
            return;
        }
        self.open_delete_modal();
    }

    /// Entry point for the editor pane. When in similar-paragraph
    /// mode + the secondary pane has keyboard focus
    /// (`self.secondary_focused`), we swap `opened ↔ secondary` so
    /// every downstream handler — all of which target
    /// `self.opened` — naturally operates on the right-pane doc.
    /// Swap back after the call returns. This keeps the 100+
    /// existing editor key handlers unchanged.
    fn handle_editor_key(&mut self, key: KeyEvent) -> Result<bool> {
        if self.secondary_focused && self.secondary.is_some() {
            std::mem::swap(&mut self.opened, &mut self.secondary);
            let r = self.handle_editor_key_inner(key);
            std::mem::swap(&mut self.opened, &mut self.secondary);
            return r;
        }
        self.handle_editor_key_inner(key)
    }

    fn handle_editor_key_inner(&mut self, key: KeyEvent) -> Result<bool> {
        if self.opened.is_none() {
            if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q')) {
                return Ok(self.request_quit());
            }
            return Ok(false);
        }

        // Stamp last_activity so idle autosave only fires after the user
        // actually pauses. Done first so every branch below benefits.
        if let Some(doc) = self.opened.as_mut() {
            doc.last_activity = std::time::Instant::now();
        }

        // Read-only gate (Help subtree): allow navigation, search, copy, and
        // focus-related chords; refuse anything that would touch the buffer
        // or the store.
        if self
            .opened
            .as_ref()
            .is_some_and(|d| d.read_only)
            && !is_read_only_safe_key(&key)
        {
            self.status = "Help is read-only".into();
            return Ok(false);
        }

        // Typewriter SFX — plain Enter (end-of-line click). Fires after
        // the read-only gate so refused keystrokes in Help don't click.
        // No-op when sound is disabled or the host has no audio device.
        if matches!(key.code, KeyCode::Enter)
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            if let Some(sp) = &self.sound {
                sp.play_enter();
            }
        }

        // F3 / F5 / F6 now resolve through the top_level binding
        // table at the top of handle_key (1.2.4+ migration).
        // Ctrl+F open find, Ctrl+X "repeat" (advance / replace+advance),
        // Ctrl+R open replace dialog or "replace all" when already in
        // replace mode.
        let ctrl_no_shift = key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::SHIFT)
            && !key.modifiers.intersects(KeyModifiers::ALT | KeyModifiers::SUPER);
        if ctrl_no_shift {
            // Ctrl+X is "Repeat" only while a search is active. Otherwise
            // it falls through (currently no-op — was cut, now Ctrl+K).
            if matches!(key.code, KeyCode::Char('x') | KeyCode::Char('X'))
                && self
                    .opened
                    .as_ref()
                    .map_or(false, |d| d.search.is_some())
            {
                self.search_advance_or_replace();
                return Ok(false);
            }
            match key.code {
                KeyCode::Char('f') | KeyCode::Char('F') => {
                    self.open_find_modal(false);
                    return Ok(false);
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    if self
                        .opened
                        .as_ref()
                        .and_then(|d| d.search.as_ref())
                        .is_some_and(|s| s.replace_with.is_some())
                    {
                        self.replace_all_remaining();
                    } else {
                        self.open_find_modal(true);
                    }
                    return Ok(false);
                }
                _ => {}
            }
        }
        // Esc in editor: first press clears an active in-buffer search (the
        // Ctrl+F flow); second press cycles focus → Tree. The Editor/Tree/
        // Search cycle is Editor → Tree → Search → Editor.
        if matches!(key.code, KeyCode::Esc) {
            if self.opened.as_ref().is_some_and(|d| d.search.is_some()) {
                if let Some(doc) = self.opened.as_mut() {
                    doc.search = None;
                }
                self.status = "search cleared".into();
                return Ok(false);
            }
            self.change_focus(Focus::Tree);
            return Ok(false);
        }

        // F4 / Ctrl+F4 resolve through the top_level binding
        // table at the top of handle_key.

        // Ctrl+H / Ctrl+J scroll the lower (read-only) pane while split is
        // open. Without split, they fall through to normal editor handling
        // (tui-textarea / our backspace-word / etc.).
        let split_active = self.opened.as_ref().is_some_and(|d| d.split.is_some());
        if split_active && key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('h') | KeyCode::Char('H') => {
                    self.scroll_split_up();
                    return Ok(false);
                }
                KeyCode::Char('j') | KeyCode::Char('J') => {
                    self.scroll_split_down();
                    return Ok(false);
                }
                _ => {}
            }
        }

        // Alt+arrows enter / extend vertical-block selection. Alt+C copies it.
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        if alt {
            match key.code {
                KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
                    if let Some(doc) = self.opened.as_mut() {
                        if doc.block_anchor.is_none() {
                            doc.block_anchor = Some(doc.textarea.cursor());
                        }
                        match key.code {
                            KeyCode::Up => doc.textarea.move_cursor(CursorMove::Up),
                            KeyCode::Down => doc.textarea.move_cursor(CursorMove::Down),
                            KeyCode::Left => doc.textarea.move_cursor(CursorMove::Back),
                            KeyCode::Right => doc.textarea.move_cursor(CursorMove::Forward),
                            _ => {}
                        }
                    }
                    return Ok(false);
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    self.block_copy();
                    return Ok(false);
                }
                _ => {}
            }
        }

        if matches!(key.code, KeyCode::Esc) {
            if let Some(doc) = self.opened.as_mut() {
                if doc.block_anchor.is_some() {
                    doc.block_anchor = None;
                    return Ok(false);
                }
            }
            self.change_focus(Focus::Tree);
            return Ok(false);
        }

        // Any key other than Alt+arrows clears block selection state.
        if let Some(doc) = self.opened.as_mut() {
            if doc.block_anchor.is_some() && !alt {
                doc.block_anchor = None;
            }
        }

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        // Plain arrow keys + Home/End/PageUp/PageDown. `input_without_shortcuts`
        // (which we use to silence tui-textarea's emacs defaults) also treats
        // these as shortcuts and drops them — so we route them ourselves.
        // Shift extends a linear selection; plain cancels any selection.
        if !alt && !ctrl {
            let cmove = match key.code {
                KeyCode::Up => Some(CursorMove::Up),
                KeyCode::Down => Some(CursorMove::Down),
                KeyCode::Left => Some(CursorMove::Back),
                KeyCode::Right => Some(CursorMove::Forward),
                KeyCode::Home => Some(CursorMove::Head),
                KeyCode::End => Some(CursorMove::End),
                KeyCode::PageUp => Some(CursorMove::ParagraphBack),
                KeyCode::PageDown => Some(CursorMove::ParagraphForward),
                _ => None,
            };
            if let Some(cmove) = cmove {
                if let Some(doc) = self.opened.as_mut() {
                    if shift {
                        if doc.textarea.selection_range().is_none() {
                            doc.textarea.start_selection();
                        }
                    } else {
                        doc.textarea.cancel_selection();
                    }
                    doc.textarea.move_cursor(cmove);
                }
                return Ok(false);
            }
        }

        // Editor key map (intercepted before tui-textarea so its emacs
        // defaults don't fire). Note the rebinds from earlier conventions:
        //   Ctrl+U undo  (was Ctrl+Z)
        //   Ctrl+K cut   (was Ctrl+X — now "repeat" for search/replace)
        //   Ctrl+P paste (was Ctrl+V)
        //   Ctrl+D/E/W/Z delete-line / delete-to-end / delete-to-start /
        //                delete-to-end (Z duplicates E per spec)
        if ctrl {
            match key.code {
                // Undo / Redo
                KeyCode::Char('u') | KeyCode::Char('U') if !shift => {
                    if let Some(doc) = self.opened.as_mut() {
                        doc.textarea.undo();
                    }
                    return Ok(false);
                }
                KeyCode::Char('y') | KeyCode::Char('Y') if !shift => {
                    if let Some(doc) = self.opened.as_mut() {
                        doc.textarea.redo();
                    }
                    return Ok(false);
                }
                // Clipboard
                KeyCode::Char('k') | KeyCode::Char('K') if !shift => {
                    self.editor_cut();
                    return Ok(false);
                }
                KeyCode::Char('c') | KeyCode::Char('C') if !shift => {
                    self.editor_copy();
                    return Ok(false);
                }
                KeyCode::Char('p') | KeyCode::Char('P') if !shift => {
                    self.editor_paste();
                    return Ok(false);
                }
                KeyCode::Char('a') | KeyCode::Char('A') if !shift => {
                    self.editor_select_all();
                    return Ok(false);
                }
                // Line-targeted deletes. All four operations preserve the
                // yank buffer so they don't clobber Ctrl+C / Ctrl+P state.
                KeyCode::Char('d') | KeyCode::Char('D') if !shift => {
                    self.editor_delete_line();
                    return Ok(false);
                }
                KeyCode::Char('e') | KeyCode::Char('E') if !shift => {
                    self.editor_delete_to_eol();
                    return Ok(false);
                }
                KeyCode::Char('w') | KeyCode::Char('W') if !shift => {
                    self.editor_delete_to_bol();
                    return Ok(false);
                }
                // Ctrl+Z is intentionally unbound. Undo is Ctrl+U,
                // delete-to-EOL is Ctrl+E. The key falls through to
                // input_without_shortcuts (which itself ignores it).
                KeyCode::Home => {
                    if let Some(doc) = self.opened.as_mut() {
                        doc.textarea.move_cursor(CursorMove::Top);
                    }
                    return Ok(false);
                }
                KeyCode::End => {
                    if let Some(doc) = self.opened.as_mut() {
                        doc.textarea.move_cursor(CursorMove::Bottom);
                    }
                    return Ok(false);
                }
                KeyCode::Left => {
                    if let Some(doc) = self.opened.as_mut() {
                        doc.textarea.move_cursor(CursorMove::WordBack);
                    }
                    return Ok(false);
                }
                KeyCode::Right => {
                    if let Some(doc) = self.opened.as_mut() {
                        doc.textarea.move_cursor(CursorMove::WordForward);
                    }
                    return Ok(false);
                }
                KeyCode::Backspace => {
                    if let Some(doc) = self.opened.as_mut() {
                        if doc.textarea.delete_word() {
                            doc.dirty = true;
                        }
                    }
                    return Ok(false);
                }
                _ => {}
            }
        }

        // Auto-close pairs (configurable). Only plain keystrokes — Ctrl /
        // Alt / Super combinations fall through to the textarea catch-all
        // below. Each helper returns `true` when it consumed the key,
        // letting us skip the catch-all without disturbing the cursor.
        let plain_no_mods = !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
        if plain_no_mods && self.cfg.editor.auto_close_pairs {
            if let KeyCode::Char(c) = key.code {
                if let Some(close) = open_pair_for(c) {
                    self.editor_auto_open_pair(c, close);
                    return Ok(false);
                }
                if is_close_pair_char(c) && self.editor_try_skip_close(c) {
                    return Ok(false);
                }
            }
            if matches!(key.code, KeyCode::Enter) && self.editor_try_expand_pair_on_enter() {
                return Ok(false);
            }
            if matches!(key.code, KeyCode::Backspace) && self.editor_try_delete_pair() {
                return Ok(false);
            }
        }

        // Everything else: pass to textarea WITHOUT its emacs-style defaults,
        // so plain typing/arrows/Home/End/PageUp/PageDown/Shift+arrows still
        // work but Ctrl+letter combinations don't get hijacked.
        if let Some(doc) = self.opened.as_mut() {
            let input: tui_textarea::Input = key.into();
            if doc.textarea.input_without_shortcuts(input) {
                doc.dirty = true;
            }
        }
        Ok(false)
    }

    /// Insert `open` + `close` at the cursor and step back one
    /// character so the cursor sits between them.
    fn editor_auto_open_pair(&mut self, open: char, close: char) {
        let Some(doc) = self.opened.as_mut() else { return };
        doc.textarea.insert_char(open);
        doc.textarea.insert_char(close);
        doc.textarea.move_cursor(CursorMove::Back);
        doc.dirty = true;
    }

    /// When the next char on the line is the same close character
    /// the user just typed, step over it instead of inserting a
    /// duplicate. Returns `false` when the next char doesn't match
    /// (caller falls through to normal insertion).
    fn editor_try_skip_close(&mut self, close: char) -> bool {
        let Some(doc) = self.opened.as_mut() else { return false };
        let (row, col) = doc.textarea.cursor();
        let line = doc.textarea.lines().get(row).cloned().unwrap_or_default();
        let next_char = line.chars().nth(col);
        if next_char == Some(close) {
            doc.textarea.move_cursor(CursorMove::Forward);
            return true;
        }
        false
    }

    /// Enter pressed with the cursor between matching brackets:
    /// expand to a 3-line indented block. Returns `false` when the
    /// cursor isn't between a pair (caller does the regular Enter).
    fn editor_try_expand_pair_on_enter(&mut self) -> bool {
        let Some(doc) = self.opened.as_ref() else { return false };
        let (row, col) = doc.textarea.cursor();
        let line = doc.textarea.lines().get(row).cloned().unwrap_or_default();
        let chars: Vec<char> = line.chars().collect();
        let before = if col > 0 { chars.get(col - 1).copied() } else { None };
        let after = chars.get(col).copied();
        if !matches!(
            (before, after),
            (Some('('), Some(')')) | (Some('['), Some(']')) | (Some('{'), Some('}'))
        ) {
            return false;
        }
        let base_indent: String = chars
            .iter()
            .take_while(|c| **c == ' ' || **c == '\t')
            .collect();
        let extra = " ".repeat(self.cfg.editor.tab_width.max(1));
        let new_indent = format!("{base_indent}{extra}");
        let Some(doc) = self.opened.as_mut() else { return false };
        // 1. Newline → cursor lands at column 0 of a new line with
        //    the close-bracket as its first char.
        doc.textarea.insert_char('\n');
        // 2. Type the deeper indent, then ANOTHER newline so the
        //    close-bracket slides further down. Cursor lands at
        //    column 0 of the close-bracket line.
        doc.textarea.insert_str(&new_indent);
        doc.textarea.insert_char('\n');
        // 3. Indent the close-bracket line with the base indent.
        doc.textarea.insert_str(&base_indent);
        // 4. Move up to the middle line, end of indent.
        doc.textarea.move_cursor(CursorMove::Up);
        doc.textarea.move_cursor(CursorMove::End);
        doc.dirty = true;
        true
    }

    /// Backspace when cursor sits between a freshly typed pair like
    /// `(|)`: delete BOTH halves. Returns `false` otherwise.
    fn editor_try_delete_pair(&mut self) -> bool {
        let Some(doc) = self.opened.as_ref() else { return false };
        let (row, col) = doc.textarea.cursor();
        let line = doc.textarea.lines().get(row).cloned().unwrap_or_default();
        let chars: Vec<char> = line.chars().collect();
        let before = if col > 0 { chars.get(col - 1).copied() } else { None };
        let after = chars.get(col).copied();
        let is_pair = matches!(
            (before, after),
            (Some('('), Some(')'))
                | (Some('['), Some(']'))
                | (Some('{'), Some('}'))
                | (Some('"'), Some('"'))
                | (Some('\''), Some('\''))
        );
        if !is_pair {
            return false;
        }
        let Some(doc) = self.opened.as_mut() else { return false };
        doc.textarea.delete_next_char();
        doc.textarea.delete_char();
        doc.dirty = true;
        true
    }

    fn editor_copy(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        doc.textarea.copy();
        let text = doc.textarea.yank_text();
        if !text.is_empty() {
            if let Some(cb) = self.clipboard.as_mut() {
                let _ = cb.set_text(text);
            }
        }
    }

    fn editor_cut(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        if doc.textarea.cut() {
            let text = doc.textarea.yank_text();
            if !text.is_empty() {
                if let Some(cb) = self.clipboard.as_mut() {
                    let _ = cb.set_text(text);
                }
            }
            doc.dirty = true;
        }
    }

    fn editor_paste(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        if let Some(cb) = self.clipboard.as_mut() {
            if let Ok(text) = cb.get_text() {
                doc.textarea.set_yank_text(text);
            }
        }
        if doc.textarea.paste() {
            doc.dirty = true;
        }
    }

    /// Copy the current rectangular selection to the system clipboard
    /// (falling back to tui-textarea's yank buffer). The rectangle is the
    /// inclusive range from `block_anchor` to the current cursor.
    fn block_copy(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            return;
        };
        let Some(anchor) = doc.block_anchor else {
            return;
        };
        let block = BlockSelection::from_anchor_and_cursor(anchor, doc.textarea.cursor());
        let lines = doc.textarea.lines();
        let mut out = String::new();
        for r in block.row_min..=block.row_max {
            if let Some(line) = lines.get(r) {
                let chars: Vec<char> = line.chars().collect();
                let s = block.col_min.min(chars.len());
                let e = (block.col_max + 1).min(chars.len());
                let piece: String = chars[s..e].iter().collect();
                out.push_str(&piece);
            }
            if r < block.row_max {
                out.push('\n');
            }
        }
        // Push to system clipboard if available.
        if let Some(cb) = self.clipboard.as_mut() {
            let _ = cb.set_text(out.clone());
        }
        if let Some(doc) = self.opened.as_mut() {
            doc.textarea.set_yank_text(out);
            doc.block_anchor = None;
        }
        self.status = "copied rectangular block to clipboard".into();
    }

    fn current_block(&self) -> Option<BlockSelection> {
        let doc = self.opened.as_ref()?;
        let anchor = doc.block_anchor?;
        Some(BlockSelection::from_anchor_and_cursor(
            anchor,
            doc.textarea.cursor(),
        ))
    }

    /// Delete the current line entirely (content + trailing newline). Cursor
    /// lands on the line that took its place. Preserves the yank buffer.
    fn editor_delete_line(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        let saved_yank = doc.textarea.yank_text();
        // Move to start of line, clear to end, then remove the newline so
        // the next line collapses up.
        doc.textarea.move_cursor(CursorMove::Head);
        doc.textarea.delete_line_by_end();
        // delete_next_char removes the newline; on the last line it's a
        // no-op which leaves an empty line where the deleted one was. That's
        // an acceptable quirk — the user can hit Ctrl+D again to remove it,
        // or move up and delete the previous newline.
        doc.textarea.delete_next_char();
        doc.textarea.set_yank_text(saved_yank);
        doc.dirty = true;
    }

    /// Delete from the cursor to the end of the current line. Used by both
    /// Ctrl+E and Ctrl+Z (the user requested both bindings for the same op).
    fn editor_delete_to_eol(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        let saved_yank = doc.textarea.yank_text();
        if doc.textarea.delete_line_by_end() {
            doc.dirty = true;
        }
        doc.textarea.set_yank_text(saved_yank);
    }

    /// Delete from the cursor back to the beginning of the current line.
    fn editor_delete_to_bol(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        let saved_yank = doc.textarea.yank_text();
        if doc.textarea.delete_line_by_head() {
            doc.dirty = true;
        }
        doc.textarea.set_yank_text(saved_yank);
    }

    fn editor_select_all(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        doc.textarea.move_cursor(CursorMove::Top);
        doc.textarea.start_selection();
        doc.textarea.move_cursor(CursorMove::Bottom);
        // CursorMove::Bottom lands at (last_row, 0). Without this End move
        // the selection would exclude the last line's content entirely.
        doc.textarea.move_cursor(CursorMove::End);
    }

    fn handle_passive_key(&mut self, key: KeyEvent) -> Result<bool> {
        // Esc bounces AI pane → AI prompt so the user can edit / send the
        // next message without an extra Tab. Mirror of the AiPrompt → Ai
        // bounce in handle_input_key.
        if self.focus == Focus::Ai && matches!(key.code, KeyCode::Esc) {
            self.change_focus(Focus::AiPrompt);
            return Ok(false);
        }
        // When the AI pane has a completed inference and is focused, single-
        // letter keys apply the result to the editor.
        if self.focus == Focus::Ai && self.inference_done_with_text() {
            match key.code {
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    self.apply_inference(InferenceAction::Replace);
                    return Ok(false);
                }
                KeyCode::Char('i') | KeyCode::Char('I') => {
                    self.apply_inference(InferenceAction::Insert);
                    return Ok(false);
                }
                // `t` / `T` — prepend the AI response to the top of the
                // paragraph (markdown→Typst conversion applied).
                KeyCode::Char('t') | KeyCode::Char('T') => {
                    self.apply_inference(InferenceAction::Top);
                    return Ok(false);
                }
                // `g` / `G` — grammar-check apply: lift only the
                // corrected paragraph (between `<<<CORRECTED>>>` /
                // `<<<END>>>` markers, or last fenced code, or after a
                // "Corrected …" heading) and overwrite the buffer
                // wholesale. No markdown→Typst conversion runs because
                // the grammar prompt instructs the model to preserve
                // Typst markup verbatim.
                KeyCode::Char('g') | KeyCode::Char('G') => {
                    self.apply_inference(InferenceAction::ReplaceCorrected);
                    return Ok(false);
                }
                KeyCode::Char('b') | KeyCode::Char('B') => {
                    self.apply_inference(InferenceAction::Bottom);
                    return Ok(false);
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    self.apply_inference(InferenceAction::CopyOnly);
                    return Ok(false);
                }
                _ => {}
            }
        }

        if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q')) {
            return Ok(self.request_quit());
        }
        Ok(false)
    }

    /// Centralized exit gate. If the open paragraph is dirty, autosave it
    /// before returning true. If the save fails, abort the exit and leave a
    /// status message so the user can recover. Called from every quit chord
    /// (Ctrl+Q, plain q in Tree / Editor-empty / AI).
    fn request_quit(&mut self) -> bool {
        if self.opened.as_ref().is_some_and(|d| d.dirty) {
            // save_current writes its own status. If it can't save, doc.dirty
            // stays true and we refuse to quit so the user can see the error
            // and recover.
            let _ = self.save_current();
            if self.opened.as_ref().is_some_and(|d| d.dirty) {
                return false;
            }
        }
        // Persist session state regardless of save outcome path above.
        // Failure is silent — sessions are a UX nicety, not correctness.
        let _ = self.save_session();
        true
    }

    fn save_session(&mut self) -> std::io::Result<()> {
        // Snapshot the live paragraph's cursor into the persistent map before
        // we serialise — otherwise an exit (or focus-loss session save) right
        // after a cursor move would lose the latest position.
        self.snapshot_open_paragraph_cursor();

        let cursor_id = self
            .rows
            .get(self.tree_cursor)
            .map(|(id, _)| id.to_string());
        let collapsed: Vec<String> = self
            .collapsed_nodes
            .iter()
            .map(|u| u.to_string())
            .collect();
        let editor_session = self.opened.as_ref().map(|d| {
            let (row, col) = d.textarea.cursor();
            EditorSession {
                opened_id: d.id.to_string(),
                cursor_row: row,
                cursor_col: col,
            }
        });
        let paragraph_cursors: std::collections::HashMap<String, ParagraphCursor> = self
            .paragraph_cursors
            .iter()
            .map(|(id, pc)| (id.to_string(), *pc))
            .collect();
        let state = SessionState {
            tree: TreeSession {
                cursor_id,
                collapsed_nodes: collapsed,
            },
            editor: editor_session,
            focus: format!("{:?}", self.focus),
            paragraph_cursors,
        };
        state.save(&self.layout.root)
    }

    /// Copy the currently-open paragraph's cursor + scroll into the
    /// in-memory `paragraph_cursors` map. Called on focus loss, on
    /// paragraph switch, and right before `save_session` writes to disk.
    /// No-op when no paragraph is open.
    fn snapshot_open_paragraph_cursor(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            return;
        };
        let (row, col) = doc.textarea.cursor();
        self.paragraph_cursors.insert(
            doc.id,
            ParagraphCursor {
                cursor_row: row,
                cursor_col: col,
                scroll_row: doc.scroll_row,
                scroll_col: doc.scroll_col,
            },
        );
    }

    /// Re-apply a saved session on startup. Silently ignores anything that
    /// no longer makes sense (missing UUIDs, corrupt fields). Should be
    /// called after `Hierarchy::load` so the lookups can resolve.
    fn restore_session(&mut self) {
        let Some(state) = SessionState::load(&self.layout.root) else {
            return;
        };

        // Per-paragraph cursor map. We restore this BEFORE opening the
        // last-active paragraph so `load_paragraph` finds an entry and seeds
        // the cursor immediately.
        for (key, pc) in &state.paragraph_cursors {
            if let Ok(id) = Uuid::parse_str(key) {
                self.paragraph_cursors.insert(id, *pc);
            }
        }

        // Collapsed branches.
        for s in &state.tree.collapsed_nodes {
            if let Ok(id) = Uuid::parse_str(s) {
                if self.hierarchy.get(id).is_some() {
                    self.collapsed_nodes.insert(id);
                }
            }
        }
        self.rebuild_rows_preserving_cursor();

        // Tree cursor.
        if let Some(cid) = &state.tree.cursor_id {
            if let Ok(id) = Uuid::parse_str(cid) {
                if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == id) {
                    self.tree_cursor = i;
                }
            }
        }

        // Open paragraph + cursor. The per-paragraph map (above) takes
        // precedence over the legacy single-cursor field — load_paragraph
        // restores from the map automatically.
        if let Some(ed) = &state.editor {
            if let Ok(id) = Uuid::parse_str(&ed.opened_id) {
                if let Some(node) = self.hierarchy.get(id).cloned() {
                    if node.kind == NodeKind::Paragraph {
                        let _ = self.load_paragraph(&node);
                        // If a fresh load didn't find a per-paragraph entry
                        // (older session file), fall back to the legacy
                        // single-cursor coordinates.
                        if !self.paragraph_cursors.contains_key(&id) {
                            if let Some(doc) = self.opened.as_mut() {
                                doc.textarea.move_cursor(CursorMove::Jump(
                                    ed.cursor_row as u16,
                                    ed.cursor_col as u16,
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Focus.
        let restored_focus = match state.focus.as_str() {
            "Tree" => Some(Focus::Tree),
            "Editor" => Some(Focus::Editor),
            "Ai" => Some(Focus::Ai),
            "SearchBar" => Some(Focus::SearchBar),
            "AiPrompt" => Some(Focus::AiPrompt),
            _ => None,
        };
        if let Some(f) = restored_focus {
            self.focus = f;
        }
    }

    fn inference_done_with_text(&self) -> bool {
        matches!(
            self.inference.as_ref().map(|i| (&i.status, i.response.is_empty())),
            Some((InferenceStatus::Done, false))
        )
    }

    fn apply_inference(&mut self, action: InferenceAction) {
        let Some(inf) = self.inference.as_ref() else {
            return;
        };
        let raw = inf.response.clone();
        if matches!(action, InferenceAction::CopyOnly) {
            // Copy keeps the original markdown — the user might paste it
            // somewhere that expects markdown, not Typst.
            if let Some(cb) = self.clipboard.as_mut() {
                let _ = cb.set_text(raw.clone());
            }
            self.status = "copied AI result to clipboard".into();
            return;
        }
        let Some(doc) = self.opened.as_mut() else {
            self.status = "no paragraph open — apply needs a focused paragraph".into();
            return;
        };

        // ReplaceCorrected has its own pipeline: pull just the corrected
        // paragraph (no commentary), skip the markdown→typst conversion
        // because the grammar prompt instructs the model to keep Typst
        // markup verbatim, and overwrite the buffer wholesale. Before
        // overwriting, snapshot the pre-correction buffer into
        // `correction_baseline` so the renderer can highlight what
        // changed in `theme.grammar_change_fg`. The highlight survives
        // saves (autosave or manual) and is dismissed only by switching
        // paragraphs or by Ctrl+B C — saving an accepted correction
        // shouldn't yank the visual diff out from under the user.
        if matches!(action, InferenceAction::ReplaceCorrected) {
            let Some(corrected) = extract_corrected_text(&raw) else {
                self.status =
                    "couldn't find corrected text in the response \
                     (expected `<<<CORRECTED>>>` block or fenced code)"
                        .into();
                return;
            };
            let baseline = doc.textarea.lines().to_vec();
            // Build a fresh TextArea from the corrected text rather than
            // shuffling cursor + selection inside the existing one — the
            // cut/select dance was leaving stray characters in the buffer
            // (the "In _The H" duplication seen in user reports).
            let corrected_lines: Vec<String> = if corrected.is_empty() {
                vec![String::new()]
            } else {
                corrected.split('\n').map(String::from).collect()
            };
            let mut new_ta = TextArea::new(corrected_lines);
            new_ta.set_cursor_line_style(
                Style::default().add_modifier(Modifier::REVERSED),
            );
            new_ta.set_line_number_style(
                Style::default().fg(self.theme.line_number_fg),
            );
            doc.textarea = new_ta;
            doc.correction_baseline = Some(baseline);
            doc.dirty = true;
            // Bump activity so idle autosave doesn't fire on the very
            // next tick (which would otherwise lose the freshness of
            // the diff before the user has had a chance to read it).
            doc.last_activity = std::time::Instant::now();
            self.status = format!(
                "applied AI result ({}) — changes highlighted; Ctrl+B C dismisses",
                action.label()
            );
            self.change_focus(Focus::Editor);
            return;
        }

        // Translate markdown to Typst for editor-bound applies; the AI tends
        // to respond in markdown (`# Heading`, `**bold**`) but our buffer is
        // Typst (`= Heading`, `*bold*`). Conversion is best-effort — anything
        // unrecognised passes through verbatim.
        let text = super::markdown::markdown_to_typst(&raw);
        match action {
            InferenceAction::Replace => {
                if doc.textarea.selection_range().is_some() {
                    doc.textarea.cut();
                } else {
                    // No selection: replace the whole document.
                    use tui_textarea::CursorMove;
                    doc.textarea.move_cursor(CursorMove::Top);
                    doc.textarea.start_selection();
                    doc.textarea.move_cursor(CursorMove::Bottom);
                    doc.textarea.cut();
                }
                doc.textarea.set_yank_text(text);
                doc.textarea.paste();
            }
            InferenceAction::Insert => {
                doc.textarea.set_yank_text(text);
                doc.textarea.paste();
            }
            InferenceAction::Top => {
                use tui_textarea::CursorMove;
                doc.textarea.move_cursor(CursorMove::Top);
                doc.textarea.move_cursor(CursorMove::Head);
                doc.textarea.set_yank_text(format!("{text}\n\n"));
                doc.textarea.paste();
            }
            InferenceAction::Bottom => {
                use tui_textarea::CursorMove;
                doc.textarea.move_cursor(CursorMove::Bottom);
                doc.textarea.move_cursor(CursorMove::End);
                doc.textarea.set_yank_text(format!("\n\n{text}"));
                doc.textarea.paste();
            }
            InferenceAction::CopyOnly | InferenceAction::ReplaceCorrected => unreachable!(),
        }
        doc.dirty = true;
        self.status = format!("applied AI result ({})", action.label());
        self.change_focus(Focus::Editor);
    }

    fn handle_input_key(&mut self, key: KeyEvent, is_search: bool) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                if is_search && self.show_results_overlay {
                    self.show_results_overlay = false;
                } else if !is_search && self.show_prompt_picker {
                    self.show_prompt_picker = false;
                } else {
                    self.show_results_overlay = false;
                    self.show_prompt_picker = false;
                    // Search bar → Editor closes the
                    // Editor → Tree → Search → Editor rotation. AI prompt
                    // bounces to AI pane (separate Ai↔AiPrompt pairing).
                    let target = if is_search {
                        Focus::Editor
                    } else {
                        Focus::Ai
                    };
                    self.change_focus(target);
                }
            }
            KeyCode::Enter => {
                if is_search {
                    if self.show_results_overlay && !self.results.is_empty() {
                        let id = self.results[self.results_cursor].id;
                        self.open_search_result(id);
                    } else {
                        self.run_search();
                    }
                } else if self.show_prompt_picker {
                    self.commit_prompt_pick();
                } else {
                    self.start_inference();
                }
            }
            KeyCode::Up if is_search && self.show_results_overlay => {
                if !self.results.is_empty() && self.results_cursor > 0 {
                    self.results_cursor -= 1;
                }
            }
            KeyCode::Down if is_search && self.show_results_overlay => {
                if !self.results.is_empty()
                    && self.results_cursor + 1 < self.results.len()
                {
                    self.results_cursor += 1;
                }
            }
            KeyCode::Up if !is_search && self.show_prompt_picker => {
                if self.prompt_picker_cursor > 0 {
                    self.prompt_picker_cursor -= 1;
                }
            }
            KeyCode::Down if !is_search && self.show_prompt_picker => {
                let n = self.prompt_picker_matches().len();
                if n > 0 && self.prompt_picker_cursor + 1 < n {
                    self.prompt_picker_cursor += 1;
                }
            }
            // 1.2.4+: Up / Down in the AI prompt (no picker
            // showing) walks `ai_prompt_history`. Shell-style.
            KeyCode::Up if !is_search => {
                if !self.ai_prompt_history.is_empty() {
                    let next = match self.ai_prompt_history_cursor {
                        Some(0) => 0,
                        Some(i) => i - 1,
                        None => self.ai_prompt_history.len() - 1,
                    };
                    self.ai_prompt_history_cursor = Some(next);
                    let entry = self.ai_prompt_history[next].clone();
                    self.ai_input.clear();
                    for c in entry.chars() {
                        self.ai_input.insert_char(c);
                    }
                }
            }
            KeyCode::Down if !is_search => {
                if let Some(cur) = self.ai_prompt_history_cursor {
                    let next = cur + 1;
                    if next >= self.ai_prompt_history.len() {
                        // Past the newest entry → leave history
                        // navigation, clear the input.
                        self.ai_prompt_history_cursor = None;
                        self.ai_input.clear();
                    } else {
                        self.ai_prompt_history_cursor = Some(next);
                        let entry = self.ai_prompt_history[next].clone();
                        self.ai_input.clear();
                        for c in entry.chars() {
                            self.ai_input.insert_char(c);
                        }
                    }
                }
            }
            KeyCode::Tab if !is_search && self.show_prompt_picker => {
                self.commit_prompt_pick();
            }
            KeyCode::Backspace => {
                self.current_input(is_search).backspace();
                if is_search {
                    self.show_results_overlay = false;
                }
                if !is_search {
                    self.ai_prompt_history_cursor = None;
                }
            }
            KeyCode::Delete => {
                self.current_input(is_search).delete();
                if is_search {
                    self.show_results_overlay = false;
                }
                if !is_search {
                    self.ai_prompt_history_cursor = None;
                }
            }
            KeyCode::Left => self.current_input(is_search).move_left(),
            KeyCode::Right => self.current_input(is_search).move_right(),
            KeyCode::Home => self.current_input(is_search).move_home(),
            KeyCode::End => self.current_input(is_search).move_end(),
            KeyCode::Char(c) => {
                // Explicit AI-prompt focus shortcuts. The global Ctrl+1..5
                // block at the top of `handle_key` covers these too, but
                // some terminals re-encode the chords in ways that bypass
                // the global path — handle them locally as a safety net so
                // they always work from the AI prompt.
                if !is_search
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key
                        .modifiers
                        .intersects(KeyModifiers::ALT | KeyModifiers::SUPER)
                {
                    if c == '1' {
                        self.change_focus(Focus::Editor);
                        return Ok(false);
                    }
                    if c == 't' || c == 'T' {
                        self.change_focus(Focus::Tree);
                        return Ok(false);
                    }
                }
                let mut residual = key.modifiers;
                residual.remove(KeyModifiers::SHIFT);
                if residual.is_empty() {
                    let final_c = if key.modifiers.contains(KeyModifiers::SHIFT)
                        && c.is_ascii_alphabetic()
                    {
                        c.to_ascii_uppercase()
                    } else {
                        c
                    };
                    self.current_input(is_search).insert_char(final_c);
                    if is_search {
                        self.show_results_overlay = false;
                    } else {
                        // 1.2.4+: typing in the AI prompt
                        // breaks history-recall navigation —
                        // the next Up arrow starts at the
                        // newest entry again.
                        self.ai_prompt_history_cursor = None;
                        self.refresh_prompt_picker();
                    }
                }
            }
            _ => {}
        }
        // After Backspace/Delete in the AI input, also refresh the picker.
        if !is_search {
            self.refresh_prompt_picker();
        }
        Ok(false)
    }

    fn refresh_prompt_picker(&mut self) {
        let s = self.ai_input.as_str();
        self.show_prompt_picker = s.starts_with('/');
        self.prompt_picker_cursor = self
            .prompt_picker_cursor
            .min(self.prompt_picker_matches().len().saturating_sub(1));
    }

    /// Union of system-level prompts (from prompts.hjson) and paragraphs
    /// nested under the "Prompts" system book. System prompts come first so
    /// the user's mental model — "well-known commands at the top, project-
    /// specific scratch prompts below" — is preserved. Filtered by the
    /// substring after `/` in `ai_input`.
    fn prompt_picker_matches(&self) -> Vec<PromptCandidate> {
        let q = self.ai_input.as_str();
        let filter = q.strip_prefix('/').unwrap_or("").trim().to_lowercase();

        // 1.2.4+: rank candidates so prefix matches beat
        // mid-word substring matches. Empty filter → keep
        // insertion order (system before book). Match scores:
        //   3 = name starts with filter
        //   2 = description starts with filter (after splitting
        //       on whitespace — so "summarize selection" matches
        //       a /sel prefix on the second word)
        //   1 = name or description contains filter
        //   0 = no match (excluded)
        let score = |name: &str, desc: &str| -> i32 {
            if filter.is_empty() {
                return 1;
            }
            let nl = name.to_lowercase();
            let dl = desc.to_lowercase();
            if nl.starts_with(&filter) {
                return 3;
            }
            if dl.split_whitespace().any(|w| w.starts_with(&filter)) {
                return 2;
            }
            if nl.contains(&filter) || dl.contains(&filter) {
                return 1;
            }
            0
        };

        let mut scored: Vec<(i32, PromptCandidate)> = Vec::new();
        // 1) prompts.hjson (system)
        for p in &self.prompts.prompts {
            let s = score(&p.name, &p.description);
            if s > 0 {
                scored.push((s, PromptCandidate {
                    name: p.name.clone(),
                    description: p.description.clone(),
                    body: PromptBody::Static(p.template.clone()),
                    source: PromptSource::System,
                }));
            }
        }
        // 2) Paragraphs under the Prompts system book
        if let Some(book_id) = self.system_book_id(crate::store::SYSTEM_TAG_PROMPTS) {
            for id in self.hierarchy.collect_subtree(book_id) {
                if id == book_id {
                    continue;
                }
                let Some(node) = self.hierarchy.get(id) else {
                    continue;
                };
                if node.kind != NodeKind::Paragraph {
                    continue;
                }
                let name = node.slug.clone();
                let title = node.title.clone();
                let s = score(&name, &title);
                if s > 0 {
                    scored.push((s, PromptCandidate {
                        name,
                        description: title,
                        body: PromptBody::BookParagraph(node.id),
                        source: PromptSource::Book,
                    }));
                }
            }
        }
        // Stable sort by descending score — preserves the
        // "system before book" within-tier ordering.
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        let out: Vec<PromptCandidate> = scored.into_iter().map(|(_, c)| c).collect();
        out
    }

    fn commit_prompt_pick(&mut self) {
        let matches = self.prompt_picker_matches();
        let Some(picked) = matches.into_iter().nth(self.prompt_picker_cursor) else {
            self.status = "no matching prompt".into();
            return;
        };
        let template = match &picked.body {
            PromptBody::Static(t) => t.clone(),
            PromptBody::BookParagraph(id) => {
                match self.store.get_content(*id) {
                    Ok(Some(bytes)) => {
                        let text = String::from_utf8_lossy(&bytes).to_string();
                        // Strip the leading `= Title` Typst heading that the
                        // editor inserts by default so it doesn't pollute the
                        // prompt sent to the LLM.
                        strip_leading_typst_heading(&text)
                    }
                    Ok(None) => {
                        self.status = format!("prompt `{}` has no body", picked.name);
                        return;
                    }
                    Err(e) => {
                        self.status = format!(
                            "loading prompt `{}` from book failed: {e}",
                            picked.name
                        );
                        return;
                    }
                }
            }
        };
        let body = self.render_template(&template);
        self.ai_input.clear();
        for c in body.chars() {
            self.ai_input.insert_char(c);
        }
        self.show_prompt_picker = false;
        let chip = match picked.source {
            PromptSource::System => "system",
            PromptSource::Book => "book",
        };
        self.status = format!(
            "loaded prompt `{}` [{chip}] — Enter to send",
            picked.name
        );
    }

    /// Look up a prompt by name inside the Prompts system book. Returns the
    /// paragraph body with the leading `= Title` heading stripped, ready to
    /// be passed through `render_template`. Returns None if no such
    /// paragraph exists or its body can't be loaded.
    fn lookup_book_prompt_template(&self, name: &str) -> Option<String> {
        let book_id = self.system_book_id(crate::store::SYSTEM_TAG_PROMPTS)?;
        let lower = name.to_lowercase();
        for id in self.hierarchy.collect_subtree(book_id) {
            if id == book_id {
                continue;
            }
            let node = self.hierarchy.get(id)?;
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            if node.slug.to_lowercase() == lower || node.title.to_lowercase() == lower {
                let bytes = self.store.get_content(node.id).ok().flatten()?;
                let text = String::from_utf8_lossy(&bytes).to_string();
                return Some(strip_leading_typst_heading(&text));
            }
        }
        None
    }

    fn render_template(&self, template: &str) -> String {
        let selection = self.current_selection_or_paragraph();
        let context = self.current_context_breadcrumb();
        template
            .replace("{{selection}}", &selection)
            .replace("{{context}}", &context)
            .trim()
            .to_string()
    }

    fn current_selection_or_paragraph(&self) -> String {
        let Some(doc) = self.opened.as_ref() else {
            return String::new();
        };
        if let Some(((r1, c1), (r2, c2))) = doc.textarea.selection_range() {
            let lines = doc.textarea.lines();
            return slice_lines(lines, r1, c1, r2, c2);
        }
        doc.textarea.lines().join("\n")
    }

    fn current_context_breadcrumb(&self) -> String {
        let Some(doc) = self.opened.as_ref() else {
            return String::new();
        };
        let Some(node) = self.hierarchy.get(doc.id) else {
            return String::new();
        };
        let mut parts: Vec<String> = self
            .hierarchy
            .ancestors(node)
            .into_iter()
            .map(|n| n.title.clone())
            .collect();
        parts.push(node.title.clone());
        parts.join(" › ")
    }

    fn start_inference(&mut self) {
        let raw = self.ai_input.as_str().trim().to_string();
        if raw.is_empty() {
            self.status = "empty prompt".into();
            return;
        }
        // 1.2.4+: stash the raw prompt in the history ring for
        // Up/Down recall. Avoids dupes-against-most-recent so
        // the list stays useful when the user re-sends the same
        // prompt repeatedly.
        if self.ai_prompt_history.last() != Some(&raw) {
            self.ai_prompt_history.push(raw.clone());
            // Cap the history so a long session doesn't grow
            // unbounded. 500 entries is past any reasonable
            // recall horizon.
            if self.ai_prompt_history.len() > 500 {
                let drop_n = self.ai_prompt_history.len() - 500;
                self.ai_prompt_history.drain(..drop_n);
            }
        }
        self.ai_prompt_history_cursor = None;
        // "Help!" prefix (case-sensitive) reroutes through the F1 Help-book
        // RAG flow. The rest of the line becomes the question; the AI pane
        // shows the same grounded answer the F1 modal produces.
        if let Some(rest) = raw.strip_prefix("Help!") {
            let question = rest.trim().to_string();
            self.ai_input.clear();
            if question.is_empty() {
                self.status = "Help: type a question after `Help!`".into();
                return;
            }
            self.start_help_inference(&question);
            return;
        }
        let user_query = if raw.starts_with('/') {
            // Resolve `/name [extra args]` form. Search system prompts
            // (prompts.hjson) first, then paragraphs under the Prompts book.
            let after = raw.trim_start_matches('/').trim();
            if let Some(p) = self.prompts.find(after) {
                self.render_template(&p.template.clone())
            } else if let Some(text) = self.lookup_book_prompt_template(after) {
                self.render_template(&text)
            } else {
                self.status =
                    format!("no prompt `{after}` — type `/` to see the list");
                return;
            }
        } else {
            raw
        };

        // Prepend the AI scope context if one is set. Failures (no
        // selection, etc.) abort the submission with a status message; the
        // scope sticks around so the user can fix the cause and re-submit.
        let prompt_text = match self.build_ai_mode_context() {
            Ok(Some(prefix)) => format!("{prefix}\n\n{user_query}"),
            Ok(None) => user_query,
            Err(reason) => {
                self.status = reason;
                return;
            }
        };
        // Lift any pending Place/Character RAG prefix (set by Ctrl+B P / C
        // when the AI prompt was empty). Consumes it — one-shot.
        let prompt_text = match self.pending_rag_prefix.take() {
            Some(rag) => format!("{rag}\n\n{prompt_text}"),
            None => prompt_text,
        };
        let mode_used = self.ai_mode;

        let (model, _env_var) = match self.ai.resolve_provider(&self.cfg.llm, None) {
            Ok(pair) => pair,
            Err(e) => {
                self.status = e.to_string();
                return;
            }
        };
        let model = model.to_string();
        let provider = self.ai.default_provider.clone();

        // Replay the accumulated chat history before this new user message
        // so the model has continuous context across turns.
        let history = self.chat_history.clone();
        // System prompt depends on the inference mode. Local clamps the
        // model to supplied context only; Full lets it augment with
        // general knowledge while still treating context as ground truth.
        // `ink.ai.set_system_prompt` overrides both via a Bund script.
        let system_prompt = self
            .system_prompt_override
            .clone()
            .or_else(|| match self.inference_mode {
                InferenceMode::Local => Some(LOCAL_SYSTEM_PROMPT.to_string()),
                InferenceMode::Full => Some(FULL_SYSTEM_PROMPT.to_string()),
            });
        let rx = spawn_chat_stream(
            self.ai.client.clone(),
            model.clone(),
            system_prompt,
            history,
            prompt_text.clone(),
        );

        self.inference = Some(Inference {
            provider: provider.clone(),
            model,
            response: String::new(),
            status: InferenceStatus::Streaming,
            rx,
            started_at: std::time::Instant::now(),
        });
        // Remember the user message so we can pair it with the assistant
        // turn once the stream finishes.
        self.pending_chat_user_msg = Some(prompt_text);
        // Reset chat-history scroll so the user always sees the
        // streaming reply (if they'd PageUp'd to look at earlier turns
        // before sending, the new turn would otherwise land off-screen).
        self.chat_history_scroll = 0;
        // Stay on the AI prompt pane so follow-up questions are one keystroke
        // away. Esc bounces to the AI pane to read/scroll the answer.
        self.change_focus(Focus::AiPrompt);
        let depth = self.chat_history.len() / 2 + 1;
        let scope_note = if mode_used == AiMode::None {
            String::new()
        } else {
            format!(" · scope={}", mode_used.label())
        };
        self.status = format!(
            "streaming from {provider} (chat turn #{depth}{scope_note})…"
        );
        // Auto-reset the scope so the next prompt isn't surprised by stale
        // context. The user re-cycles with F9 to pick a new scope.
        self.ai_mode = AiMode::None;
        // Clear the prompt so the next inference starts fresh.
        self.ai_input.clear();
    }

    fn run_search(&mut self) {
        let query = self.search_input.as_str().trim().to_string();
        if query.is_empty() {
            self.show_results_overlay = false;
            self.results.clear();
            self.status = "empty query".into();
            return;
        }
        match self.store.search_text(&query, 10) {
            Ok(raw) => {
                self.results = raw.iter().filter_map(SearchHit::parse).collect();
                self.results_cursor = 0;
                self.show_results_overlay = true;
                self.status = format!(
                    "`{}` → {} result(s) · ↑/↓ to navigate · Enter to open · Esc to close",
                    query,
                    self.results.len()
                );
            }
            Err(e) => {
                self.status = format!("search failed: {e}");
            }
        }
    }

    fn open_search_result(&mut self, id: Uuid) {
        if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == id) {
            self.tree_cursor = i;
        }
        let node = match self.hierarchy.get(id).cloned() {
            Some(n) => n,
            None => {
                self.status = format!("hit {id} no longer in hierarchy — try a fresh search");
                self.show_results_overlay = false;
                return;
            }
        };
        self.show_results_overlay = false;
        if matches!(node.kind, NodeKind::Paragraph) {
            if let Err(e) = self.load_paragraph(&node) {
                self.status = format!("open failed: {e}");
            }
        } else {
            self.change_focus(Focus::Tree);
            self.status = format!(
                "`{}` is a {} — its paragraph children are editable",
                node.title,
                node.kind.as_str()
            );
        }
    }

    fn current_input(&mut self, is_search: bool) -> &mut TextInput {
        if is_search {
            &mut self.search_input
        } else {
            &mut self.ai_input
        }
    }

    fn move_cursor(&mut self, delta: i32) {
        if self.rows.is_empty() {
            return;
        }
        let new =
            (self.tree_cursor as i32 + delta).clamp(0, self.rows.len() as i32 - 1) as usize;
        self.tree_cursor = new;
    }

    // -------- modal -------------------------------------------------------

    fn open_add_modal(&mut self, kind: NodeKind) {
        // User-added Books at root always slot ABOVE the system block
        // ([Notes, Research, Prompts, Places, Characters, Help]) so the
        // user's own content stays at the top of the tree.
        if kind == NodeKind::Book {
            if let Some(notes_id) = self.system_book_id(crate::store::SYSTEM_TAG_NOTES) {
                self.open_add_modal_inner(kind, InsertPosition::Before(notes_id));
                return;
            }
        }
        self.open_add_modal_inner(kind, InsertPosition::End);
    }

    /// Build a "Book › Chapter › Subchapter" style breadcrumb of titles
    /// for the node identified by `id`. Used by the search-results overlay
    /// and the Help RAG context block so users see human names rather than
    /// the slug-derived filesystem path. Falls back to the hit's own slug
    /// path if the node has vanished from the hierarchy (e.g. just deleted).
    fn title_breadcrumb(&self, id: Uuid) -> String {
        let Some(node) = self.hierarchy.get(id) else {
            return String::new();
        };
        let mut parts: Vec<String> = self
            .hierarchy
            .ancestors(node)
            .into_iter()
            .map(|n| n.title.clone())
            .collect();
        parts.push(node.title.clone());
        parts.join(" › ")
    }

    /// Look up a system book's UUID by tag. Returns None if the project
    /// pre-dates the system-book feature and the book hasn't been seeded
    /// yet (shouldn't happen in practice — ensure_system_books runs on
    /// every Store::open).
    fn system_book_id(&self, tag: &str) -> Option<Uuid> {
        self.hierarchy
            .iter()
            .find(|n| {
                n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(tag)
            })
            .map(|n| n.id)
    }

    /// Insert-after variant: walks up from the tree cursor to find a node of
    /// the same `kind` as the one being added; if found, the new node will be
    /// placed immediately after it. Falls back to append-at-end if no
    /// same-kind ancestor exists (e.g. pressing P with cursor on a book).
    fn open_add_modal_after(&mut self, kind: NodeKind) {
        let cursor_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
        let anchor = cursor_id.and_then(|id| {
            let mut cur = Some(id);
            while let Some(c) = cur {
                let node = self.hierarchy.get(c)?;
                if node.kind == kind {
                    return Some(node.id);
                }
                cur = node.parent_id;
            }
            None
        });
        let position = match anchor {
            Some(id) => InsertPosition::After(id),
            None => InsertPosition::End,
        };
        self.open_add_modal_inner(kind, position);
    }

    fn open_add_modal_inner(&mut self, kind: NodeKind, position: InsertPosition) {
        // For After(anchor), the parent is anchor.parent_id (always valid
        // because the anchor's a same-kind node). For End, walk up to find a
        // valid parent.
        let parent_node = match position {
            InsertPosition::After(anchor_id) | InsertPosition::Before(anchor_id) => {
                match self.hierarchy.get(anchor_id) {
                    Some(anchor) => anchor.parent_id.and_then(|pid| self.hierarchy.get(pid)),
                    None => {
                        self.status = "anchor for insert-around vanished from hierarchy".into();
                        return;
                    }
                }
            }
            InsertPosition::End => {
                let cursor_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
                match self.hierarchy.pick_parent_for(&self.cfg, cursor_id, kind) {
                    Ok(p) => p,
                    Err(e) => {
                        self.status = format!("can't add {}: {e}", kind.as_str());
                        return;
                    }
                }
            }
        };
        let parent_label = parent_node.map_or_else(
            || "<books root>".to_string(),
            |n| self.hierarchy.slug_path(n),
        );
        let parent_id = parent_node.map(|n| n.id);
        self.modal = Modal::Adding {
            kind,
            parent_id,
            parent_label,
            input: TextInput::new(),
            position,
        };
    }

    fn move_current(&mut self, dir: MoveDir) {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            self.status = "nothing selected".into();
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            return;
        };
        let siblings = self.hierarchy.children_of(node.parent_id);
        let pos = match siblings.iter().position(|n| n.id == id) {
            Some(p) => p,
            None => return,
        };
        let other_pos = match dir {
            MoveDir::Up => {
                if pos == 0 {
                    self.status = format!("`{}` is already first", node.slug);
                    return;
                }
                pos - 1
            }
            MoveDir::Down => {
                if pos + 1 >= siblings.len() {
                    self.status = format!("`{}` is already last", node.slug);
                    return;
                }
                pos + 1
            }
        };
        let other_id = siblings[other_pos].id;
        match self.store.swap_siblings(&self.hierarchy, id, other_id) {
            Ok(()) => {
                self.status = format!(
                    "moved `{}` {}",
                    node.slug,
                    match dir {
                        MoveDir::Up => "up",
                        MoveDir::Down => "down",
                    }
                );
                self.reload_hierarchy();
                // Keep the cursor on the same node after the swap.
                if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == id) {
                    self.tree_cursor = i;
                }
            }
            Err(e) => {
                self.status = format!("move failed: {e}");
            }
        }
    }

    fn open_find_modal(&mut self, with_replace: bool) {
        if self.opened.is_none() {
            self.status = "no paragraph open".into();
            return;
        }
        // Pre-fill with the last pattern if we have one open already.
        let mut search_input = TextInput::new();
        let mut replace_input = TextInput::new();
        if let Some(state) = self.opened.as_ref().and_then(|d| d.search.as_ref()) {
            for c in state.pattern.chars() {
                search_input.insert_char(c);
            }
            if with_replace {
                if let Some(r) = &state.replace_with {
                    for c in r.chars() {
                        replace_input.insert_char(c);
                    }
                }
            }
        }
        self.modal = Modal::FindReplace {
            search_input,
            replace_input: if with_replace {
                Some(replace_input)
            } else {
                None
            },
            focus_replace: false,
        };
    }

    fn commit_find(&mut self) {
        let (pattern, replace_with) = match &self.modal {
            Modal::FindReplace {
                search_input,
                replace_input,
                ..
            } => (
                search_input.as_str().to_string(),
                replace_input.as_ref().map(|i| i.as_str().to_string()),
            ),
            _ => return,
        };
        if pattern.is_empty() {
            self.status = "search pattern is empty".into();
            return;
        }
        let Some(doc) = self.opened.as_mut() else {
            self.modal = Modal::None;
            return;
        };
        let lines = doc.textarea.lines().to_vec();
        match SearchState::build(&pattern, replace_with.clone(), &lines) {
            Ok(state) => {
                let n = state.matches.len();
                doc.search = Some(state);
                self.modal = Modal::None;
                if n == 0 {
                    self.status = format!("no matches for /{pattern}/");
                    return;
                }
                // Jump cursor to first match
                self.jump_to_current_match();
                if replace_with.is_some() {
                    // Replace mode: do the FIRST replacement automatically so
                    // user sees immediate effect; subsequent Ctrl+G keep going.
                    self.do_replace_current();
                    let remaining = self
                        .opened
                        .as_ref()
                        .and_then(|d| d.search.as_ref())
                        .map_or(0, |s| s.matches.len());
                    self.status = format!(
                        "/{pattern}/ → replaced 1 · {remaining} left · Ctrl+G next · Ctrl+R replace all · Esc clear"
                    );
                } else {
                    self.status = format!(
                        "/{pattern}/ → {n} match(es) · Ctrl+G next · Ctrl+R add replacement · Esc clear"
                    );
                }
            }
            Err(e) => {
                self.status = format!("regex error: {e}");
                // Leave the modal open so the user can fix the pattern.
            }
        }
    }

    fn jump_to_current_match(&mut self) {
        let target = self
            .opened
            .as_ref()
            .and_then(|d| d.search.as_ref())
            .and_then(|s| s.current_match())
            .map(|m| (m.row, m.col_start));
        let Some((row, col)) = target else {
            return;
        };
        // Editor viewport height in lines — `layout_editor` is cached
        // from the last draw and includes the two border rows.
        let viewport_h =
            (self.layout_editor.height as usize).saturating_sub(2);
        if let Some(doc) = self.opened.as_mut() {
            doc.textarea
                .move_cursor(CursorMove::Jump(row as u16, col as u16));
            // The editor draws itself — it doesn't render the
            // tui-textarea widget — so the actual viewport top is
            // `doc.scroll_row`, not anything inside tui-textarea. The
            // per-render auto-scroll only nudges `scroll_row` when the
            // cursor falls outside `[scroll_row, scroll_row + h)`,
            // which after a forward Jump lands the cursor at the
            // BOTTOM edge of the viewport. Pre-pinning scroll_row to
            // `target - h/2` keeps the cursor inside the new viewport,
            // so the auto-scroll leaves it alone and the match lands
            // in the middle.
            //
            // Both renderers track scroll_row this way (unwrapped uses
            // source rows verbatim; wrapped uses visual rows but for
            // typical short-line literary content the two agree).
            if viewport_h > 0 {
                let half = viewport_h / 2;
                doc.scroll_row = row.saturating_sub(half);
            }
        }
    }

    fn search_advance_or_replace(&mut self) {
        let is_replace = self
            .opened
            .as_ref()
            .and_then(|d| d.search.as_ref())
            .is_some_and(|s| s.replace_with.is_some());
        if is_replace {
            // Replace the current match, refresh hits, jump to new first.
            self.do_replace_current();
            self.refresh_search_after_edit();
            let remaining = self
                .opened
                .as_ref()
                .and_then(|d| d.search.as_ref())
                .map_or(0, |s| s.matches.len());
            if remaining == 0 {
                if let Some(doc) = self.opened.as_mut() {
                    doc.search = None;
                }
                self.status = "replace done — no more matches".into();
            } else {
                self.jump_to_current_match();
                self.status = format!("replaced · {remaining} left · Ctrl+R replace all");
            }
        } else {
            // Search-only: advance to next match (wraps).
            if let Some(doc) = self.opened.as_mut() {
                if let Some(state) = doc.search.as_mut() {
                    state.advance();
                }
            }
            self.jump_to_current_match();
            let n = self
                .opened
                .as_ref()
                .and_then(|d| d.search.as_ref())
                .map_or(0, |s| s.matches.len());
            let i = self
                .opened
                .as_ref()
                .and_then(|d| d.search.as_ref())
                .map_or(0, |s| s.current);
            if n > 0 {
                self.status = format!("match {} / {}", i + 1, n);
            }
        }
    }

    fn do_replace_current(&mut self) {
        let (row, col_start, col_end, replacement) = {
            let Some(doc) = self.opened.as_ref() else {
                return;
            };
            let Some(state) = &doc.search else {
                return;
            };
            let Some(replacement) = state.replace_with.clone() else {
                return;
            };
            let Some(m) = state.current_match() else {
                return;
            };
            (m.row, m.col_start, m.col_end, replacement)
        };
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        // Select [col_start..col_end] in row, cut, insert replacement.
        doc.textarea
            .move_cursor(CursorMove::Jump(row as u16, col_start as u16));
        doc.textarea.start_selection();
        doc.textarea
            .move_cursor(CursorMove::Jump(row as u16, col_end as u16));
        doc.textarea.cut();
        doc.textarea.insert_str(&replacement);
        doc.dirty = true;
    }

    fn refresh_search_after_edit(&mut self) {
        let lines = self
            .opened
            .as_ref()
            .map(|d| d.textarea.lines().to_vec());
        let Some(lines) = lines else { return };
        if let Some(doc) = self.opened.as_mut() {
            if let Some(state) = doc.search.as_mut() {
                state.refresh(&lines);
            }
        }
    }

    fn replace_all_remaining(&mut self) {
        let mut count = 0;
        loop {
            let has_any = self
                .opened
                .as_ref()
                .and_then(|d| d.search.as_ref())
                .map_or(false, |s| !s.matches.is_empty());
            if !has_any {
                break;
            }
            self.do_replace_current();
            self.refresh_search_after_edit();
            count += 1;
            if count > 100_000 {
                break;
            }
        }
        if let Some(doc) = self.opened.as_mut() {
            doc.search = None;
        }
        self.status = format!("replaced {count} match(es) and cleared search");
    }

    /// Toggle split-edit mode. When entering, capture the current buffer as
    /// the lower-pane snapshot; when leaving, drop the snapshot and restore
    /// full-size editor.
    fn toggle_split(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            self.status = "no paragraph open".into();
            return;
        };
        if doc.split.is_some() {
            doc.split = None;
            self.status = "split closed".into();
        } else {
            let snapshot_lines = doc.textarea.lines().to_vec();
            doc.split = Some(SplitView {
                snapshot_lines,
                scroll_row: 0,
            });
            self.status =
                "split open · upper r/w · lower r/o · Ctrl+H/J scroll · Ctrl+F4 accept · F4 close"
                    .into();
        }
    }

    /// Replace the live buffer with the split snapshot and exit split mode.
    /// Used to "roll back" to the captured version after experimenting.
    fn accept_split_snapshot(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        let Some(split) = doc.split.take() else {
            self.status = "split is not open".into();
            return;
        };
        let lines = if split.snapshot_lines.is_empty() {
            vec![String::new()]
        } else {
            split.snapshot_lines
        };
        let mut new_ta = TextArea::new(lines);
        new_ta.set_cursor_line_style(Style::default().add_modifier(Modifier::REVERSED));
        new_ta.set_line_number_style(Style::default().fg(Color::DarkGray));
        doc.textarea = new_ta;
        doc.dirty = true;
        doc.scroll_row = 0;
        doc.scroll_col = 0;
        doc.last_activity = std::time::Instant::now();
        self.status =
            "split snapshot accepted — buffer replaced; Ctrl+S to commit · bold shows the diff"
                .into();
    }

    fn scroll_split_up(&mut self) {
        if let Some(doc) = self.opened.as_mut() {
            if let Some(split) = &mut doc.split {
                split.scroll_row = split.scroll_row.saturating_sub(1);
            }
        }
    }

    fn scroll_split_down(&mut self) {
        if let Some(doc) = self.opened.as_mut() {
            if let Some(split) = &mut doc.split {
                let max = split.snapshot_lines.len().saturating_sub(1);
                if split.scroll_row < max {
                    split.scroll_row += 1;
                }
            }
        }
    }

    fn create_snapshot_of_current(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "no paragraph open".into();
            return;
        };
        let body = doc.textarea.lines().join("\n");
        let id = doc.id;
        let Some(node) = self.hierarchy.get(id).cloned() else {
            self.status = "node missing from hierarchy".into();
            return;
        };
        match self.store.create_snapshot(&node, body.as_bytes()) {
            Ok(snap_id) => {
                let n_snaps = self
                    .store
                    .list_snapshots(id)
                    .map(|v| v.len())
                    .unwrap_or(0);
                self.status = format!(
                    "snapshot {} created ({} total) — F6 to view",
                    snap_id.simple(),
                    n_snaps
                );
            }
            Err(e) => {
                self.status = format!("snapshot failed: {e}");
            }
        }
    }

    /// Dispatch the keystroke that follows the meta-prefix. Each pane has
    /// its own action table:
    ///   * Tree (and Search bar): hierarchy operations
    ///   * Editor: save / snapshots / file load / split-edit
    ///   * AI (and AI prompt): inference management
    fn handle_meta_action(&mut self, key: KeyEvent) {
        self.meta_pending = false;
        if matches!(key.code, KeyCode::Esc) {
            self.status = "meta cancelled".into();
            return;
        }
        let plain = !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
        if !plain {
            self.status = "meta cancelled".into();
            return;
        }
        let resolved = super::keybind::read().resolve_meta_sub(&key, self.focus);
        match resolved {
            Some(super::keybind::Action::None) => {
                self.status = "meta: chord disabled by config".into();
            }
            Some(action) => self.run_action(action),
            None => {
                self.status = format!(
                    "meta {}: unknown action — use Ctrl+B again to retry",
                    self.focus.label()
                );
            }
        }
    }

    /// Dispatch the Bund-meta chord (Ctrl+Z by default). Three
    /// actions in the v1 chord table:
    ///
    ///   R — run the buffer (eval the open Script's body against Adam)
    ///   N — new script (Add modal under the Scripts system book)
    ///   E — eval one expression (modal prompt, like F1)
    /// Dispatch the view-prefix chord (Ctrl+V): markdown export.
    /// Two suffixes are recognised; their meaning shifts by focus:
    ///
    ///   * Editor pane + `1`  → current paragraph buffer (in-memory,
    ///     includes unsaved edits).
    ///   * Editor pane + `2`  → containing subchapter's subtree.
    ///   * Tree pane   + `1`  → tree-cursor's node and all descendants.
    ///
    /// All variants run the source through `typst_to_markdown`
    /// and write to the launch cwd with a `<title>-<stamp>.md`
    /// filename. Errors land on the status bar; nothing else
    /// changes.
    /// Resolve the tree-cursor row to a paragraph and link it to
    /// the link-pick owner. Always exits pick mode (success or
    /// failure) and returns focus to the editor. Direction is
    /// stashed in `link_pick_for.1`:
    /// * `Outgoing` (Ctrl+V A) — `add_link(owner, picked)`
    /// * `Incoming` (Ctrl+V I) — `add_link(picked, owner)`
    fn commit_link_pick(&mut self) {
        let Some((owner, direction)) = self.link_pick_for else { return };
        let picked = self
            .rows
            .get(self.tree_cursor)
            .map(|(id, _)| *id);
        self.link_pick_for = None;
        let Some(picked) = picked else {
            self.status = "link cancelled: no tree row selected".into();
            self.change_focus(Focus::Editor);
            return;
        };
        let picked_kind = self.hierarchy.get(picked).map(|n| n.kind);
        if !matches!(picked_kind, Some(NodeKind::Paragraph)) {
            self.status =
                "link cancelled: target is not a paragraph".into();
            self.change_focus(Focus::Editor);
            return;
        }
        // Outgoing: owner → picked.  Incoming: picked → owner.
        let (from, to) = match direction {
            LinkPickDirection::Outgoing => (owner, picked),
            LinkPickDirection::Incoming => (picked, owner),
        };
        match self.add_paragraph_link(from, to) {
            Ok(()) => {
                let title = self
                    .hierarchy
                    .get(to)
                    .map(|n| n.title.clone())
                    .unwrap_or_else(|| "?".into());
                self.status = match direction {
                    LinkPickDirection::Outgoing => format!("linked → `{title}`"),
                    LinkPickDirection::Incoming => {
                        let from_title = self
                            .hierarchy
                            .get(from)
                            .map(|n| n.title.clone())
                            .unwrap_or_else(|| "?".into());
                        format!("linked `{from_title}` → current")
                    }
                };
            }
            Err(e) => {
                self.status = format!("link: {e}");
            }
        }
        self.change_focus(Focus::Editor);
    }

    // ── Wiki-links (1.2.4+) ────────────────────────────────

    /// Enter "select paragraph to link" mode. Tree pane gets a
    /// custom title; Enter on a paragraph adds it to the open
    /// paragraph's outgoing links (with a circular-reference
    /// guard). Esc / loss-of-focus cancels.
    fn enter_link_pick_mode(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "view A: no paragraph open".into();
            return;
        };
        self.link_pick_for = Some((doc.id, LinkPickDirection::Outgoing));
        self.change_focus(Focus::Tree);
        self.status =
            "link: select paragraph to link · Enter confirms · Esc cancels".into();
    }

    /// Reverse-direction picker (1.2.4+, Ctrl+V I). The
    /// tree-picked paragraph's outgoing links gains the open
    /// paragraph — same circular guard.
    fn enter_incoming_link_pick_mode(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "view I: no paragraph open".into();
            return;
        };
        self.link_pick_for = Some((doc.id, LinkPickDirection::Incoming));
        self.change_focus(Focus::Tree);
        self.status =
            "incoming link: select paragraph that will link to current · Enter confirms · Esc cancels"
                .into();
    }

    /// Open the linked-paragraphs modal for the open paragraph.
    /// Lists each outgoing link with title + slug path; `D`
    /// removes a row.
    fn open_link_picker_modal(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "view L: no paragraph open".into();
            return;
        };
        let owner = doc.id;
        let entries = self.collect_link_entries(owner);
        if entries.is_empty() {
            self.status =
                "view L: no linked paragraphs (Ctrl+V A adds one)".into();
            return;
        }
        self.modal = Modal::LinkPicker {
            owner,
            entries,
            cursor: 0,
            scroll: 0,
        };
        self.status =
            "links: ↑↓ select · D removes · Esc closes".into();
    }

    /// Toggle bookmark on the open paragraph. Persists via the
    /// existing metadata-update path (`Store::raw().update_metadata`).
    fn toggle_bookmark(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "view B: no paragraph open".into();
            return;
        };
        let id = doc.id;
        let Some(node) = self.hierarchy.get(id).cloned() else {
            self.status = "view B: paragraph not in hierarchy".into();
            return;
        };
        let new_value = !node.bookmark;
        let mut updated = node.clone();
        updated.bookmark = new_value;
        updated.modified_at = chrono::Utc::now();
        if let Err(e) = self
            .store
            .raw()
            .update_metadata(id, updated.to_json())
        {
            self.status = format!("bookmark: store update failed: {e}");
            return;
        }
        self.reload_hierarchy();
        self.status = format!(
            "bookmark {}: `{}`",
            if new_value { "added" } else { "cleared" },
            node.title
        );
    }

    /// Open the bookmark picker — every paragraph whose
    /// `bookmark` flag is true.
    fn open_bookmark_picker_modal(&mut self) {
        let entries = self.collect_bookmark_entries();
        if entries.is_empty() {
            self.status =
                "view M: no bookmarks (Ctrl+V B toggles one on the open paragraph)".into();
            return;
        }
        self.modal = Modal::BookmarkPicker {
            entries,
            cursor: 0,
            scroll: 0,
        };
        self.status =
            "bookmarks: ↑↓ select · Enter opens · D clears bookmark · Esc closes"
                .into();
    }

    /// Open the fuzzy paragraph picker. Pre-builds the full
    /// paragraph list (title + slug-path) so subsequent
    /// keystrokes only filter, never re-walk the hierarchy.
    /// Ctrl+V R (1.2.5+) — render the open paragraph in-process,
    /// pop a floating preview modal on top of the editor. Saves
    /// the current buffer first so the rendered preview matches
    /// the on-disk source the user just edited.
    fn open_rendered_paragraph_preview(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status =
                "render ¶: no paragraph open in the editor".into();
            return;
        };
        // Skip Bund / HJSON / images — only typst sources render
        // through the typst pipeline.
        let is_typst = matches!(
            doc.content_type.as_deref(),
            None | Some("") | Some("typst"),
        );
        if !is_typst {
            self.status = format!(
                "render ¶: `{}` is not a typst source — Ctrl+V R only renders .typ buffers",
                doc.title,
            );
            return;
        }
        // Capture the title before we save (save may renumber /
        // re-derive the title from the first sentence).
        let title_before = doc.title.clone();
        // Save first — the spec says "Save current buffer" before
        // render. If save fails, abort the render (we shouldn't
        // render bytes that aren't on disk).
        if doc.dirty {
            if let Err(e) = self.save_current() {
                self.status =
                    format!("render ¶: autosave failed: {e}");
                return;
            }
        }
        let Some(doc) = self.opened.as_ref() else {
            self.status = "render ¶: editor closed during save".into();
            return;
        };
        let body = doc.textarea.lines().join("\n");
        let title = doc.title.clone();
        if body.trim().is_empty() {
            self.status =
                "render ¶: buffer is empty — nothing to render".into();
            return;
        }
        let _ = title_before;
        // Image picker (ratatui-image) is required to display the
        // PNG. Without it (terminals without graphics support and
        // `images.preview_enabled = false`) fall back to status
        // bar with a hint.
        let Some(picker) = self.image_picker.as_ref() else {
            self.status =
                "render ¶: terminal can't display images (set `images.preview_enabled: true` or use a kitty / iterm2 / sixel-capable terminal)".into();
            return;
        };
        let settings = crate::typst_world::WorldSettings::from_cfg(
            &self.cfg.typst_compile,
        );
        // Preview DPI: 2.0 ppt = ~144 dpi. Good for screen,
        // doesn't blow up memory on long paragraphs. Renders
        // every page up front so Left/Right inside the modal is
        // a pure protocol swap (no re-compile).
        match crate::typst_paragraph_render::render_all(
            &body,
            settings.clone(),
            2.0,
        ) {
            Ok(rendered) => {
                let total = rendered.len();
                let first_w = rendered[0].width;
                let first_h = rendered[0].height;
                let pages: Vec<RenderedPageProto> = rendered
                    .into_iter()
                    .map(|r| RenderedPageProto {
                        proto: picker.new_resize_protocol(r.image),
                        width: r.width,
                        height: r.height,
                    })
                    .collect();
                self.modal = Modal::RenderedPreview {
                    title: title.clone(),
                    body,
                    settings,
                    pages,
                    current_page: 0,
                };
                let pages_note = if total > 1 {
                    format!(" · page 1/{}  · ←/→ navigate", total)
                } else {
                    String::new()
                };
                self.status = format!(
                    "render ¶ `{}` · {}×{}{}  ·  Esc closes · S saves current · A saves all",
                    title, first_w, first_h, pages_note,
                );
            }
            Err(err) => {
                let first_line = err
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("compile failed");
                self.status =
                    format!("render ¶: {first_line}");
            }
        }
    }

    fn open_fuzzy_paragraph_picker(&mut self) {
        let entries = self.collect_all_paragraph_entries();
        if entries.is_empty() {
            self.status =
                "view P: no paragraphs in this project".into();
            return;
        }
        self.modal = Modal::FuzzyParagraphPicker {
            input: TextInput::new(),
            entries,
            cursor: 0,
            scroll: 0,
        };
        self.status =
            "find ¶: type to filter · ↑↓ select · Enter opens · Esc cancels".into();
    }

    /// Collect every paragraph in the project (excluding
    /// system-book content) as picker entries.
    fn collect_all_paragraph_entries(&self) -> Vec<ScriptPickerEntry> {
        let mut out: Vec<ScriptPickerEntry> = Vec::new();
        for (n, _) in self.hierarchy.flatten() {
            if n.kind != NodeKind::Paragraph {
                continue;
            }
            // Skip system-book content — Help reference, Typst
            // reference, etc., aren't manuscript paragraphs.
            let in_system = self
                .hierarchy
                .ancestors(n)
                .into_iter()
                .any(|a| a.kind == NodeKind::Book && a.system_tag.is_some());
            if in_system {
                continue;
            }
            out.push(ScriptPickerEntry {
                id: n.id,
                title: n.title.clone(),
                slug_path: self.hierarchy.slug_path(n),
            });
        }
        out.sort_by(|a, b| a.slug_path.cmp(&b.slug_path));
        out
    }

    /// Walk the hierarchy and collect every paragraph whose
    /// `bookmark` flag is true.
    fn collect_bookmark_entries(&self) -> Vec<ScriptPickerEntry> {
        let mut out: Vec<ScriptPickerEntry> = Vec::new();
        for (n, _) in self.hierarchy.flatten() {
            if n.kind != NodeKind::Paragraph || !n.bookmark {
                continue;
            }
            out.push(ScriptPickerEntry {
                id: n.id,
                title: n.title.clone(),
                slug_path: self.hierarchy.slug_path(n),
            });
        }
        out.sort_by(|a, b| a.title.cmp(&b.title));
        out
    }

    /// Open the backlinks modal for the open paragraph.
    fn open_backlink_picker_modal(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "view K: no paragraph open".into();
            return;
        };
        let target = doc.id;
        let entries = self.collect_backlink_entries(target);
        if entries.is_empty() {
            self.status =
                "view K: no incoming links to this paragraph".into();
            return;
        }
        self.modal = Modal::BacklinkPicker {
            target,
            entries,
            cursor: 0,
            scroll: 0,
        };
        self.status =
            "backlinks: ↑↓ select · Enter opens · D removes source link · Esc closes"
                .into();
    }

    /// Walk every paragraph and collect the ones whose
    /// `linked_paragraphs` contains `target`. O(N) over the
    /// hierarchy — acceptable for typical project sizes; if it
    /// ever bites, the obvious next step is a reverse-index
    /// cached at hierarchy-load time.
    fn collect_backlink_entries(&self, target: Uuid) -> Vec<ScriptPickerEntry> {
        let mut out: Vec<ScriptPickerEntry> = Vec::new();
        for (n, _) in self.hierarchy.flatten() {
            if n.kind != NodeKind::Paragraph {
                continue;
            }
            if !n.linked_paragraphs.contains(&target) {
                continue;
            }
            out.push(ScriptPickerEntry {
                id: n.id,
                title: n.title.clone(),
                slug_path: self.hierarchy.slug_path(n),
            });
        }
        out.sort_by(|a, b| a.slug_path.cmp(&b.slug_path));
        out
    }

    /// Resolve the open paragraph's outgoing links into picker
    /// entries (title + slug path). Stale UUIDs (target deleted)
    /// are silently filtered.
    fn collect_link_entries(&self, owner: Uuid) -> Vec<ScriptPickerEntry> {
        let Some(node) = self.hierarchy.get(owner) else {
            return Vec::new();
        };
        node.linked_paragraphs
            .iter()
            .filter_map(|id| self.hierarchy.get(*id))
            .map(|n| ScriptPickerEntry {
                id: n.id,
                title: n.title.clone(),
                slug_path: self.hierarchy.slug_path(n),
            })
            .collect()
    }

    /// Add `target` to `owner`'s outgoing links. Rejects self-
    /// linking and circular references (walks `target`'s
    /// outgoing closure looking for `owner`). Persists via
    /// `Store::update_metadata`.
    fn add_paragraph_link(
        &mut self,
        owner: Uuid,
        target: Uuid,
    ) -> std::result::Result<(), String> {
        if owner == target {
            return Err("can't link a paragraph to itself".into());
        }
        let owner_node = self
            .hierarchy
            .get(owner)
            .cloned()
            .ok_or_else(|| format!("owner paragraph {owner} not in hierarchy"))?;
        if owner_node.linked_paragraphs.contains(&target) {
            return Err("already linked".into());
        }
        // Circular guard: walk target's outgoing transitive closure
        // depth-first; if owner appears in it, we'd create a loop.
        if self.link_path_exists(target, owner) {
            return Err(
                "You can not create circular references".into(),
            );
        }
        let target_node = self
            .hierarchy
            .get(target)
            .ok_or_else(|| format!("target paragraph {target} not in hierarchy"))?;
        if target_node.kind != NodeKind::Paragraph {
            return Err(format!("`{}` is not a paragraph", target_node.title));
        }
        let mut updated = owner_node.clone();
        updated.linked_paragraphs.push(target);
        updated.modified_at = chrono::Utc::now();
        self.store
            .raw()
            .update_metadata(owner, updated.to_json())
            .map_err(|e| format!("store update: {e}"))?;
        self.reload_hierarchy();
        Ok(())
    }

    /// Remove `target` from `owner`'s outgoing links. Returns
    /// `false` when the link wasn't present (no-op).
    fn remove_paragraph_link(
        &mut self,
        owner: Uuid,
        target: Uuid,
    ) -> std::result::Result<bool, String> {
        let owner_node = self
            .hierarchy
            .get(owner)
            .cloned()
            .ok_or_else(|| format!("owner paragraph {owner} not in hierarchy"))?;
        if !owner_node.linked_paragraphs.contains(&target) {
            return Ok(false);
        }
        let mut updated = owner_node.clone();
        updated.linked_paragraphs.retain(|u| *u != target);
        updated.modified_at = chrono::Utc::now();
        self.store
            .raw()
            .update_metadata(owner, updated.to_json())
            .map_err(|e| format!("store update: {e}"))?;
        self.reload_hierarchy();
        Ok(true)
    }

    /// True when `start`'s outgoing-link transitive closure
    /// reaches `goal`. Used by `add_paragraph_link` to refuse
    /// cycles. Bounded DFS — a malformed graph with a cycle in
    /// it (shouldn't be possible given this very check, but
    /// stay safe) terminates via `visited`.
    fn link_path_exists(&self, start: Uuid, goal: Uuid) -> bool {
        let mut stack: Vec<Uuid> = vec![start];
        let mut visited: std::collections::HashSet<Uuid> =
            std::collections::HashSet::new();
        while let Some(id) = stack.pop() {
            if id == goal {
                return true;
            }
            if !visited.insert(id) {
                continue;
            }
            if let Some(node) = self.hierarchy.get(id) {
                for next in &node.linked_paragraphs {
                    stack.push(*next);
                }
            }
        }
        false
    }

    /// Dispatch one of the three Ctrl+V markdown-export scopes
    /// through the per-scope `prepare_*` helpers, then open the
    /// SaveMarkdown modal pre-filled with the default
    /// destination. Enter on the modal writes; the user can
    /// edit the path before pressing Enter to redirect.
    fn view_export_markdown(&mut self, scope: ViewMdScope) {
        let prepared = match scope {
            ViewMdScope::Buffer => self.prepare_markdown_buffer(),
            ViewMdScope::Subchapter => self.prepare_markdown_subchapter(),
            ViewMdScope::Subtree => self.prepare_markdown_tree_subtree(),
        };
        match prepared {
            Ok((body, default_dest, label)) => {
                self.open_save_markdown_modal(body, default_dest, label);
            }
            Err(e) => self.status = format!("view: {e}"),
        }
    }

    /// Open the save-as modal with the default path pre-filled.
    /// The user can edit; Enter writes; Esc cancels.
    fn open_save_markdown_modal(
        &mut self,
        body: String,
        default_dest: std::path::PathBuf,
        label: String,
    ) {
        let mut input = TextInput::new();
        for c in default_dest.to_string_lossy().chars() {
            input.insert_char(c);
        }
        self.modal = Modal::SaveMarkdown {
            input,
            body,
            label,
        };
        self.status =
            "save as: edit path or just hit Enter to save · Esc cancels".into();
    }

    /// Commit `body` to whatever path the SaveMarkdown modal's
    /// input contains. Empty input falls back to a fresh default
    /// (defensive — pre-fill should make this rare).
    fn commit_save_markdown(&mut self, body: String, label: String, raw: String) {
        let path_str = raw.trim();
        let path = if path_str.is_empty() {
            match self.default_markdown_dest(&label) {
                Ok(p) => p,
                Err(e) => {
                    self.status = format!("save as: {e}");
                    return;
                }
            }
        } else {
            // Expand `~/` to home if present so users can paste a
            // tilde path. No glob / env expansion — kept minimal.
            let expanded = if let Some(rest) = path_str.strip_prefix("~/") {
                match std::env::var_os("HOME") {
                    Some(home) => std::path::PathBuf::from(home).join(rest),
                    None => std::path::PathBuf::from(path_str),
                }
            } else {
                std::path::PathBuf::from(path_str)
            };
            expanded
        };
        match std::fs::write(&path, body.as_bytes()) {
            Ok(()) => {
                self.status = format!("view: wrote {}", path.display());
            }
            Err(e) => {
                self.status = format!("save as: write {}: {e}", path.display());
            }
        }
    }

    fn handle_view_action(&mut self, key: KeyEvent) {
        self.view_pending = false;
        if matches!(key.code, KeyCode::Esc) {
            self.status = "view cancelled".into();
            return;
        }
        let plain = !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
        if !plain {
            self.status = "view cancelled".into();
            return;
        }
        let resolved = super::keybind::read().resolve_view_sub(&key, self.focus);
        match resolved {
            Some(super::keybind::Action::None) => {
                self.status = "view: chord disabled by config".into();
            }
            Some(action) => self.run_action(action),
            None => {
                self.status =
                    "view: unknown chord — 1/2 export · S similar · G progress · T target · Esc".into();
            }
        }
    }

    /// Entry point for the Ctrl+V S chord. Behaviour depends on
    /// whether we're already in similar-paragraph mode:
    ///
    /// * Not in mode → save the current buffer, embed it as a
    ///   similarity query against the vector index, and open the
    ///   SimilarPicker modal with the results (current paragraph
    ///   filtered out, paragraphs only).
    /// * Already in mode → save both buffers and drop the
    ///   secondary doc so the layout returns to tree | editor | AI.
    fn toggle_similar_paragraph_mode(&mut self) {
        if self.secondary.is_some() {
            // Save both, then exit similar mode. If the focused
            // (= `self.opened`) doc fails to save, surface that
            // first and keep the user in similar mode so they can
            // fix it. The unfocused doc's save error is swallowed
            // with a tracing log — it's the secondary buffer,
            // less critical to surface immediately.
            if let Err(e) = self.save_current() {
                self.status = format!("view S: save primary failed — {e:#}");
                return;
            }
            if let Some(mut sec) = self.secondary.take() {
                if sec.dirty {
                    if let Err(e) = self.save_doc(&mut sec) {
                        tracing::warn!(
                            target: "inkhaven::view_similar",
                            "secondary save failed: {e:#}",
                        );
                        self.status = format!(
                            "view S: exited (secondary save warning: {e:#})"
                        );
                        self.secondary_focused = false;
                        return;
                    }
                }
            }
            self.secondary_focused = false;
            self.status = "view S: exited similar-paragraphs mode".into();
            return;
        }
        // Not in mode — open the picker. Need an open paragraph
        // to derive the similarity query from.
        let Some(doc) = self.opened.as_ref() else {
            self.status = "view S: no paragraph open — nothing to compare against".into();
            return;
        };
        // Save the current buffer first so the similarity search
        // sees on-disk text (the vector index is refreshed on
        // save). If save fails, abort — searching against stale
        // bytes would mislead.
        if doc.dirty {
            if let Err(e) = self.save_current() {
                self.status = format!("view S: save failed — {e:#}");
                return;
            }
        }
        let (current_id, query) = match self.opened.as_ref() {
            Some(d) => (d.id, d.textarea.lines().join("\n")),
            None => {
                self.status = "view S: paragraph closed during save".into();
                return;
            }
        };
        if query.trim().is_empty() {
            self.status = "view S: paragraph is empty — nothing to compare".into();
            return;
        }
        match self.find_similar_paragraphs(current_id, &query, 20) {
            Ok(entries) if entries.is_empty() => {
                self.status =
                    "view S: no similar paragraphs found (need more indexed content)".into();
            }
            Ok(entries) => {
                self.modal = Modal::SimilarPicker {
                    entries,
                    cursor: 0,
                    scroll: 0,
                };
                self.status =
                    "similar: ↑↓ select · Enter open side-by-side · Esc cancel".into();
            }
            Err(e) => {
                self.status = format!("view S: search failed — {e:#}");
            }
        }
    }

    /// Run a vector-similarity search seeded with `query` and
    /// turn the raw hits into picker entries. Filters out
    /// `exclude_id` (the current paragraph; would otherwise top
    /// the list with score = 1.0) and any non-Paragraph kind
    /// (Help-book content, Notes/Places/etc. should surface
    /// elsewhere — the user asked for paragraphs).
    fn find_similar_paragraphs(
        &self,
        exclude_id: Uuid,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<SimilarPickerEntry>> {
        use crate::tui::search_results::SearchHit;
        // Over-fetch: the dedup pass inside the store collapses
        // meta/content slots, but our own filters (current id,
        // non-paragraph kinds) still drop rows. Aim for `limit`
        // *survivors*, not `limit` raw hits.
        let raw = self
            .store
            .search_text(query, (limit + 4).max(8))
            .map_err(|e| anyhow::anyhow!("similarity search: {e}"))?;
        let mut out: Vec<SimilarPickerEntry> = Vec::new();
        for v in raw.iter() {
            let Some(hit) = SearchHit::parse(v) else {
                continue;
            };
            if hit.id == exclude_id {
                continue;
            }
            if !matches!(hit.kind, crate::store::node::NodeKind::Paragraph) {
                continue;
            }
            // Only surface paragraphs that still live in the
            // hierarchy (the vector index can lag a fast delete).
            let Some(node) = self.hierarchy.get(hit.id) else {
                continue;
            };
            let slug_path = self.hierarchy.slug_path(node);
            out.push(SimilarPickerEntry {
                id: hit.id,
                title: hit.title.clone(),
                slug_path,
                score: hit.score,
                snippet: hit.snippet.clone(),
            });
            if out.len() >= limit {
                break;
            }
        }
        Ok(out)
    }

    /// Save an arbitrary OpenedDoc to disk. Used by the
    /// similar-paragraph mode toggle to flush the secondary doc
    /// (which lives in `self.secondary`, outside the normal
    /// save_current path). Mirrors save_current's body so the
    /// two stay in sync; refactoring both onto one impl is
    /// future work.
    fn save_doc(
        &mut self,
        doc: &mut OpenedDoc,
    ) -> std::result::Result<(), String> {
        let abs = self.layout.root.join(&doc.rel_path);
        let body = doc.textarea.lines().join("\n");
        let prev_words =
            crate::progress::count_words(&doc.saved_lines.join("\n"));
        std::fs::write(&abs, body.as_bytes())
            .map_err(|e| format!("write {}: {e}", abs.display()))?;
        // Refresh the store so subsequent searches see the new
        // text. We deliberately skip the snapshot machinery —
        // secondary saves are routine + cheap; explicit snapshots
        // go through the F5 / Ctrl+B N flow on the primary doc.
        let mut node = self
            .hierarchy
            .get(doc.id)
            .cloned()
            .ok_or_else(|| format!("paragraph {} not in hierarchy", doc.id))?;
        self.store
            .update_paragraph_content(&mut node, body.as_bytes())
            .map_err(|e| format!("store update: {e}"))?;
        doc.dirty = false;
        doc.saved_lines = doc.textarea.lines().to_vec();
        let new_words = crate::progress::count_words(&body);
        let book_id = self.book_of_node(doc.id);
        crate::progress::record_save(doc.id, book_id, prev_words, new_words);
        self.refresh_progress_cache();
        Ok(())
    }

    /// Compute (markdown body, default destination, status-bar
    /// label) for the open paragraph's buffer. Used by the
    /// save-as flow — `view_export_markdown` opens the
    /// `SaveMarkdown` modal pre-filled with `default_dest`.
    fn prepare_markdown_buffer(
        &self,
    ) -> std::result::Result<(String, std::path::PathBuf, String), String> {
        let doc = self
            .opened
            .as_ref()
            .ok_or_else(|| "no paragraph open".to_string())?;
        let typst_src = doc.textarea.lines().join("\n");
        let md = crate::export::markdown::typst_to_markdown(&typst_src);
        let dest = self.default_markdown_dest(&doc.title)?;
        Ok((md, dest, doc.title.clone()))
    }

    fn prepare_markdown_subchapter(
        &self,
    ) -> std::result::Result<(String, std::path::PathBuf, String), String> {
        let doc = self
            .opened
            .as_ref()
            .ok_or_else(|| "no paragraph open".to_string())?;
        let para = self
            .hierarchy
            .get(doc.id)
            .ok_or_else(|| "paragraph not in hierarchy".to_string())?;
        let root = self
            .hierarchy
            .ancestors(para)
            .into_iter()
            .find(|a| {
                matches!(
                    a.kind,
                    crate::store::node::NodeKind::Subchapter
                        | crate::store::node::NodeKind::Chapter
                )
            })
            .ok_or_else(|| "no containing subchapter".to_string())?;
        self.prepare_markdown_subtree_of(root)
    }

    fn prepare_markdown_tree_subtree(
        &self,
    ) -> std::result::Result<(String, std::path::PathBuf, String), String> {
        let (id, _) = *self
            .rows
            .get(self.tree_cursor)
            .ok_or_else(|| "no tree row under cursor".to_string())?;
        let node = self
            .hierarchy
            .get(id)
            .ok_or_else(|| "node missing from hierarchy".to_string())?;
        self.prepare_markdown_subtree_of(node)
    }

    fn prepare_markdown_subtree_of(
        &self,
        root: &crate::store::node::Node,
    ) -> std::result::Result<(String, std::path::PathBuf, String), String> {
        let layout = crate::project::ProjectLayout::new(self.store.project_root());
        let combined = crate::export::assemble_typst_source(
            &layout,
            &self.hierarchy,
            Some(root.id),
        )
        .map_err(|e| format!("assemble: {e:#}"))?;
        let md = crate::export::markdown::typst_to_markdown(&combined);
        let dest = self.default_markdown_dest(&root.title)?;
        Ok((md, dest, root.title.clone()))
    }

    /// Compute the default markdown destination for a given
    /// title. Format: `<cwd>/<slug>-YYYYDDMM-HHMM.md`. Same
    /// scheme `write_markdown_to_cwd` used before 1.2.4's
    /// save-as picker.
    fn default_markdown_dest(
        &self,
        title: &str,
    ) -> std::result::Result<std::path::PathBuf, String> {
        let cwd = std::env::current_dir().map_err(|e| format!("cwd: {e}"))?;
        let stamp = chrono::Local::now().format("%Y%d%m-%H%M");
        let stem = slug::slugify(title);
        let safe_stem = if stem.is_empty() { "buffer".to_string() } else { stem };
        Ok(cwd.join(format!("{safe_stem}-{stamp}.md")))
    }

    /// 1.2.5+ — default destination for `S` on Modal::RenderedPreview.
    /// Mirrors the markdown dest shape but with `.png` extension.
    fn default_rendered_png_dest(
        &self,
        title: &str,
    ) -> std::result::Result<std::path::PathBuf, String> {
        let cwd = std::env::current_dir().map_err(|e| format!("cwd: {e}"))?;
        let stamp = chrono::Local::now().format("%Y%d%m-%H%M");
        let stem = slug::slugify(title);
        let safe_stem = if stem.is_empty() { "render".to_string() } else { stem };
        Ok(cwd.join(format!("{safe_stem}-{stamp}.png")))
    }

    /// `S` (single) or `A` (all) inside Modal::RenderedPreview —
    /// pop the save-as picker with a sensible default path
    /// pre-filled. `all = true` stamps a multi-page-aware default
    /// (still a single base path; we append `-page-NNN` per page
    /// at write time). The picker stashes the underlying preview
    /// modal in `return_to` so Esc preserves navigation state.
    fn open_save_rendered_png_picker(&mut self, all: bool) {
        let (body, settings, title, current_page) = match &self.modal {
            Modal::RenderedPreview {
                body,
                settings,
                title,
                current_page,
                ..
            } => (
                body.clone(),
                settings.clone(),
                title.clone(),
                *current_page,
            ),
            _ => return,
        };
        let default_dest = match self.default_rendered_png_dest(&title) {
            Ok(p) => p,
            Err(e) => {
                self.status = format!("save PNG as: {e}");
                return;
            }
        };
        let mut input = TextInput::new();
        for c in default_dest.to_string_lossy().chars() {
            input.insert_char(c);
        }
        // Move the current modal into the picker's return_to
        // stash — std::mem::replace avoids cloning the protos.
        let return_to = Box::new(std::mem::replace(&mut self.modal, Modal::None));
        let pages = if all {
            PagesToSave::All
        } else {
            PagesToSave::Single(current_page)
        };
        let mode_label = match &pages {
            PagesToSave::Single(idx) => format!("page {}", idx + 1),
            PagesToSave::All => "all pages".to_string(),
        };
        self.modal = Modal::SaveRenderedPng {
            input,
            body,
            settings,
            title,
            pages,
            return_to,
        };
        self.status = format!(
            "save PNG as ({mode_label}): edit path or hit Enter · Esc returns to preview",
        );
    }

    /// Re-render the paragraph at full DPI (4.0 px/pt) and write
    /// to the picked path. Status-bar reports outcome. For
    /// `PagesToSave::All` we strip a trailing `.png` from the
    /// user's input (if present) and append `-page-NNN.png` per
    /// page; for `Single(idx)` the input is used verbatim.
    fn commit_save_rendered_png(
        &mut self,
        body: &str,
        settings: &crate::typst_world::WorldSettings,
        raw: &str,
        title: &str,
        pages: PagesToSave,
    ) {
        let path_str = raw.trim();
        let base_path = if path_str.is_empty() {
            match self.default_rendered_png_dest(title) {
                Ok(p) => p,
                Err(e) => {
                    self.status = format!("save PNG: {e}");
                    return;
                }
            }
        } else if let Some(rest) = path_str.strip_prefix("~/") {
            match std::env::var_os("HOME") {
                Some(home) => std::path::PathBuf::from(home).join(rest),
                None => std::path::PathBuf::from(path_str),
            }
        } else {
            std::path::PathBuf::from(path_str)
        };
        // 4.0 px/pt ≈ 288 dpi. Print-quality without going
        // wild on memory for chapter-sized paragraphs.
        match pages {
            PagesToSave::Single(idx) => match crate::typst_paragraph_render::render_page(
                body,
                settings.clone(),
                4.0,
                idx,
            ) {
                Ok(rendered) => match std::fs::write(&base_path, &rendered.png_bytes) {
                    Ok(()) => {
                        self.status = format!(
                            "save PNG: wrote {} (page {} · {}×{} · {} bytes)",
                            base_path.display(),
                            idx + 1,
                            rendered.width,
                            rendered.height,
                            rendered.png_bytes.len(),
                        );
                    }
                    Err(e) => {
                        self.status =
                            format!("save PNG: write {}: {e}", base_path.display());
                    }
                },
                Err(e) => {
                    let first =
                        e.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
                    self.status =
                        format!("save PNG: re-render failed: {first}");
                }
            },
            PagesToSave::All => {
                // Strip a trailing .png so `myrender.png` and
                // `myrender` both become `myrender-page-001.png` etc.
                let stem = base_path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "render".to_string());
                let parent = base_path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
                match crate::typst_paragraph_render::render_all(
                    body,
                    settings.clone(),
                    4.0,
                ) {
                    Ok(rendered_pages) => {
                        let total = rendered_pages.len();
                        let pad = total.to_string().len().max(3);
                        let mut written: Vec<String> = Vec::with_capacity(total);
                        for (i, page) in rendered_pages.iter().enumerate() {
                            let fname =
                                format!("{stem}-page-{:0pad$}.png", i + 1, pad = pad);
                            let dest = parent.join(&fname);
                            if let Err(e) = std::fs::write(&dest, &page.png_bytes) {
                                self.status = format!(
                                    "save PNG: write {} failed: {e} (wrote {} of {})",
                                    dest.display(),
                                    written.len(),
                                    total,
                                );
                                return;
                            }
                            written.push(fname);
                        }
                        let in_dir = if parent.as_os_str().is_empty() {
                            "(cwd)".to_string()
                        } else {
                            parent.display().to_string()
                        };
                        self.status = format!(
                            "save PNG: wrote {} pages to {} ({})",
                            total,
                            in_dir,
                            // Show first..last filename for context
                            // — full list would blow the status bar.
                            if total == 1 {
                                written[0].clone()
                            } else {
                                format!("{}…{}", written[0], written[total - 1])
                            },
                        );
                    }
                    Err(e) => {
                        let first =
                            e.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
                        self.status =
                            format!("save PNG (all): re-render failed: {first}");
                    }
                }
            }
        }
    }

    fn handle_bund_action(&mut self, key: KeyEvent) {
        self.bund_pending = false;
        if matches!(key.code, KeyCode::Esc) {
            self.status = "bund cancelled".into();
            return;
        }
        let plain = !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
        if !plain {
            self.status = "bund cancelled".into();
            return;
        }
        let resolved = super::keybind::read().resolve_bund_sub(&key, self.focus);
        match resolved {
            Some(super::keybind::Action::None) => {
                self.status = "bund: chord disabled by config".into();
            }
            Some(action) => self.run_action(action),
            None => {
                self.status =
                    "bund: unknown action — R run · N new · E eval · Esc cancel".into();
            }
        }
    }

    /// Central dispatcher for every chord-action. Stage 1 of the
    /// rebindable-keys roadmap (`Documentation/RELEASE_NOTES/…`):
    /// the table at `self.keys` resolves `KeyEvent` → `Action`,
    /// and this switch is the single point where each variant
    /// hits its concrete handler. Adding a new action means a
    /// new variant in `keybind::Action`, a new arm here, and an
    /// entry in `KeyBindings::defaults()`.
    fn run_action(&mut self, action: super::keybind::Action) {
        use super::keybind::Action as A;
        match action {
            // ── Tree pane ─────────────────────────────────────
            A::AddBook => self.open_add_modal(NodeKind::Book),
            A::AddChapter => self.open_add_modal(NodeKind::Chapter),
            A::AddSubchapter => self.open_add_modal(NodeKind::Subchapter),
            A::AddParagraph => self.open_add_modal(NodeKind::Paragraph),
            A::DeleteNode => self.open_delete_modal(),
            A::MorphType => self.cycle_leaf_type(),
            A::ReorderUp => self.move_current(MoveDir::Up),
            A::ReorderDown => self.move_current(MoveDir::Down),

            // ── Editor pane ───────────────────────────────────
            A::Save => {
                if self.opened.is_some() {
                    let _ = self.save_current();
                } else {
                    self.status = "no paragraph open".into();
                }
            }
            A::CreateSnapshot => self.create_snapshot_of_current(),
            A::CycleStatus => self.cycle_paragraph_status(),
            A::OpenFunctionPicker => self.open_function_picker(),
            A::RenameToFirstSentence => self.rename_paragraph_to_first_sentence(),
            A::LookupPlacesOrImage => {
                if !self.try_open_image_picker() {
                    self.start_lexicon_inference(LexiconKind::Places);
                }
            }
            A::LookupCharacters => self.start_lexicon_inference(LexiconKind::Characters),
            A::LookupNotes => self.start_lexicon_inference(LexiconKind::Notes),
            A::LookupArtefacts => self.start_lexicon_inference(LexiconKind::Artefacts),
            A::OpenQuickref => self.open_quickref(),

            // ── Global ────────────────────────────────────────
            A::OpenCredits => self.open_credits(),
            A::OpenBookInfo => self.open_book_info(),
            A::OpenLlmPicker => self.open_llm_picker(),
            A::ToggleSound => self.toggle_sound(),
            A::ScheduleAssemble => self.schedule_assembly(),
            A::ScheduleBuild => self.schedule_build(),
            A::ScheduleTake => self.schedule_take(),
            A::ToggleTypewriter => self.toggle_typewriter_mode(),
            A::ToggleAiFullscreen => self.toggle_ai_fullscreen(),
            A::StatusFilterReady => self.open_status_filter("Ready"),
            A::StatusFilterFinal => self.open_status_filter("Final"),
            A::StatusFilterThird => self.open_status_filter("Third"),
            A::StatusFilterSecond => self.open_status_filter("Second"),
            A::StatusFilterFirst => self.open_status_filter("First"),
            A::StatusFilterNapkin => self.open_status_filter("Napkin"),
            A::StatusFilterNone => self.open_status_filter("None"),

            // ── AI pane ───────────────────────────────────────
            A::ClearChat => self.clear_chat_history(),

            // ── Bund prefix ───────────────────────────────────
            A::BundRunBuffer => self.bund_run_buffer(),
            A::BundNewScript => self.bund_new_script(),
            A::BundOpenEvalModal => self.bund_open_eval_modal(),
            A::BundOpenScriptPicker => self.bund_open_script_picker(),

            // ── View prefix ───────────────────────────────────
            A::ViewExportMarkdownBuffer => self.view_export_markdown(ViewMdScope::Buffer),
            A::ViewExportMarkdownSubchapter => self.view_export_markdown(ViewMdScope::Subchapter),
            A::ViewExportMarkdownSubtree => self.view_export_markdown(ViewMdScope::Subtree),
            A::ViewToggleSimilarMode => self.toggle_similar_paragraph_mode(),
            A::ViewOpenProgress => self.open_progress_modal(),
            A::ViewOpenParagraphTarget => self.open_paragraph_target_modal(),
            A::ViewAddLink => self.enter_link_pick_mode(),
            A::ViewAddIncomingLink => self.enter_incoming_link_pick_mode(),
            A::ViewListLinks => self.open_link_picker_modal(),
            A::ViewListBacklinks => self.open_backlink_picker_modal(),
            A::ViewToggleBookmark => self.toggle_bookmark(),
            A::ViewListBookmarks => self.open_bookmark_picker_modal(),
            A::ViewFuzzyParagraphPicker => self.open_fuzzy_paragraph_picker(),
            A::ViewRenderParagraph => self.open_rendered_paragraph_preview(),
            A::ViewNextDiagnostic => self.jump_to_next_diagnostic(),

            // ── Top-level F-keys (1.2.4+ migration) ───────────
            A::HelpQuery => self.open_help_query_modal(),
            A::RenameNode => self.open_rename_modal(),
            A::FilePickerTreeImport => {
                self.open_file_picker(PickerContext::TreeInsertOrImport)
            }
            A::FilePickerEditorLoad => {
                self.open_file_picker(PickerContext::EditorLoad)
            }
            A::ToggleSplit => self.toggle_split(),
            A::AcceptSplitSnapshot => self.accept_split_snapshot(),
            A::OpenSnapshotPicker => self.open_snapshot_picker(),
            A::GrammarCheck => self.start_grammar_check(),
            A::CycleAiMode => self.cycle_ai_mode(),
            A::ToggleInferenceMode => self.toggle_inference_mode(),

            // Runtime-bound Bund lambda. Dispatch through the
            // hooks machinery so the recursion-cap + policy-deny
            // semantics already in place apply uniformly. No args
            // pushed; the lambda body sees an empty workbench.
            A::BundLambda(name) => {
                crate::scripting::hooks::fire(name.as_ref(), Vec::new());
            }

            // Explicit "do nothing" — should never reach here
            // because the dispatcher catches it first, but harmless.
            A::None => {}
        }
    }

    /// Ctrl+B T — cycle the selected leaf node's flavour through
    /// `Paragraph(typst) → Paragraph(hjson) → Script(bund)` and
    /// back. Target picked from:
    ///   * the open buffer when the focus is on the editor, or
    ///   * the tree cursor otherwise.
    /// Closes + reopens the buffer (if open on the converted node)
    /// so the new highlighter + content_type take effect immediately.
    /// 1.2.4+: multi-select-aware bulk type cycle. When
    /// `tree_marked` is non-empty, runs `cycle_leaf_type_single`
    /// over every marked id and reports the aggregate.
    fn cycle_leaf_type_bulk(&mut self) {
        if self.tree_marked.is_empty() {
            return;
        }
        let ids: Vec<Uuid> = self.tree_marked.iter().copied().collect();
        let mut ok = 0usize;
        let mut fail = 0usize;
        // If the open buffer is in the set, close it first —
        // its rel_path will change as part of the conversion,
        // so reopening from the fresh hierarchy is safer than
        // trying to keep the live doc in sync mid-loop.
        let reopen_id = self
            .opened
            .as_ref()
            .map(|d| d.id)
            .filter(|id| self.tree_marked.contains(id));
        if reopen_id.is_some() {
            self.opened = None;
        }
        for id in &ids {
            if self.cycle_leaf_type_single(*id).is_ok() {
                ok += 1;
            } else {
                fail += 1;
            }
        }
        self.reload_hierarchy();
        // Reopen if needed.
        if let Some(id) = reopen_id {
            if let Some(node) = self.hierarchy.get(id).cloned() {
                if matches!(node.kind, NodeKind::Paragraph) {
                    let _ = self.load_paragraph(&node);
                }
            }
        }
        self.status = if fail == 0 {
            format!("type cycled on {ok} paragraph(s)")
        } else {
            format!("type cycled on {ok} · {fail} failed")
        };
    }

    /// Single-node type cycle used by both the cursor-row chord
    /// and the multi-select wrapper. No buffer-reopen logic —
    /// callers are responsible for that.
    fn cycle_leaf_type_single(
        &mut self,
        node_id: Uuid,
    ) -> std::result::Result<(), String> {
        let node = self
            .hierarchy
            .get(node_id)
            .cloned()
            .ok_or_else(|| format!("node {node_id} not in hierarchy"))?;
        let (new_kind, new_ct, _label) = match (node.kind, node.content_type.as_deref()) {
            (NodeKind::Paragraph, None | Some("typst")) => {
                (NodeKind::Paragraph, Some("hjson"), "hjson")
            }
            (NodeKind::Paragraph, Some("hjson")) => {
                (NodeKind::Script, Some("bund"), "bund")
            }
            (NodeKind::Script, _) => (NodeKind::Paragraph, None, "typst"),
            _ => return Err("not a text leaf".into()),
        };
        self.store
            .convert_leaf(&self.hierarchy, node_id, new_kind, new_ct)
            .map_err(|e| format!("convert: {e}"))?;
        Ok(())
    }

    fn cycle_leaf_type(&mut self) {
        // Pick the node to convert. From the Editor pane: prefer
        // the open buffer; fall back to the tree cursor when the
        // editor pane has nothing open (so M still does the
        // right thing if the user pressed it from a blank editor).
        // From any other pane: always tree cursor.
        let cursor_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
        let target_id: Option<Uuid> = match self.focus {
            Focus::Editor => self.opened.as_ref().map(|d| d.id).or(cursor_id),
            _ => cursor_id,
        };
        let Some(node_id) = target_id else {
            self.status = "type-cycle: nothing selected".into();
            return;
        };
        let Some(node) = self.hierarchy.get(node_id).cloned() else {
            self.status = "type-cycle: node missing from hierarchy".into();
            return;
        };
        let (new_kind, new_ct, label) = match (node.kind, node.content_type.as_deref()) {
            (NodeKind::Paragraph, None | Some("typst")) => {
                (NodeKind::Paragraph, Some("hjson"), "hjson")
            }
            (NodeKind::Paragraph, Some("hjson")) => {
                (NodeKind::Script, Some("bund"), "bund")
            }
            (NodeKind::Script, _) => (NodeKind::Paragraph, None, "typst"),
            (k, ct) => {
                self.status = format!(
                    "type-cycle: {} ({ct:?}) is not a text leaf — only paragraphs / scripts cycle",
                    k.as_str()
                );
                return;
            }
        };

        // Snapshot whether the buffer is open on this node + the
        // focus we should be on when we return. `load_paragraph`
        // unconditionally focuses Editor at the end, so if the
        // user invoked the cycle from the Tree pane we'd steal
        // their focus otherwise.
        let buffer_was_open = self.opened.as_ref().is_some_and(|d| d.id == node_id);
        let saved_focus = self.focus;
        if buffer_was_open {
            self.opened = None;
        }

        match self
            .store
            .convert_leaf(&self.hierarchy, node_id, new_kind, new_ct)
        {
            Ok(converted) => {
                self.status =
                    format!("type-cycle: `{}` is now {label}", converted.title);
                self.reload_hierarchy();
                if buffer_was_open {
                    let _ = self.load_paragraph(&converted);
                    // `load_paragraph` focuses the editor; restore
                    // the pane the user was actually in.
                    if saved_focus != Focus::Editor {
                        self.change_focus(saved_focus);
                    }
                }
            }
            Err(e) => {
                self.status = format!("type-cycle failed: {e}");
            }
        }
    }

    /// Ctrl+Z R — eval the currently-open Script's body against Adam.
    /// No-ops with a status message when no Script is open.
    fn bund_run_buffer(&mut self) {
        // Snapshot everything we need out of self before invoking
        // scripting_eval — that needs &mut self, so we can't hold
        // any borrow of self.opened / self.hierarchy across the
        // call.
        let (body, title) = {
            let Some(doc) = self.opened.as_ref() else {
                self.status = "bund: no buffer open — Ctrl+Z R needs an open .bund".into();
                return;
            };
            let Some(node) = self.hierarchy.get(doc.id) else {
                self.status = "bund: open buffer's node is missing from hierarchy".into();
                return;
            };
            if node.kind != NodeKind::Script {
                self.status = format!(
                    "bund: open buffer is a {}, not a script",
                    node.kind.as_str()
                );
                return;
            }
            (doc.textarea.lines().join("\n"), node.title.clone())
        };
        // Go through `scripting_eval` so the App-state-accessing
        // `ink.editor.* / ink.ai.* / ink.typst.*` stdlib words can
        // reach `self`.
        match self.scripting_eval(&body) {
            Ok(out) => {
                self.status = format_eval_output(&out, Some(&title));
            }
            Err(e) => {
                self.status = format!("bund: eval failed — {e:#}");
            }
        }
    }

    /// Ctrl+Z N — open the Add modal pre-targeted at the Scripts
    /// system book. Falls back to the standard add path if the
    /// Scripts book hasn't been seeded for some reason.
    fn bund_new_script(&mut self) {
        if let Some(scripts_id) = self.system_book_id(crate::store::SYSTEM_TAG_SCRIPTS) {
            // Same shape as open_add_modal_inner for End, but with
            // the parent forced to the Scripts book rather than
            // derived from cursor position.
            let parent_label = if let Some(scripts) = self.hierarchy.get(scripts_id) {
                self.hierarchy.slug_path(scripts)
            } else {
                "scripts".to_string()
            };
            self.modal = Modal::Adding {
                kind: NodeKind::Script,
                parent_id: Some(scripts_id),
                parent_label,
                input: TextInput::new(),
                position: InsertPosition::End,
            };
            self.status = "bund: new script under Scripts — type a title, Enter to create".into();
        } else {
            // Fall back to the default cursor-based parent picker —
            // user-added Books can also host scripts.
            self.open_add_modal(NodeKind::Script);
        }
    }

    /// Ctrl+Z E — open a modal asking for a one-shot expression.
    /// On Enter, eval against Adam and surface the result (or
    /// error) in the status bar. Reuses the BundEval modal variant.
    fn bund_open_eval_modal(&mut self) {
        self.modal = Modal::BundEval {
            input: TextInput::new(),
        };
        self.status = "bund eval: type an expression, Enter to run, Esc to cancel".into();
    }

    /// Ctrl+Z ? — list executable scripts. Starts in `Branch`
    /// scope (the cursor's nearest containing book / chapter /
    /// subchapter). `A` toggles to `ScriptsBook` scope. Enter
    /// runs the highlighted script.
    fn bund_open_script_picker(&mut self) {
        let scope = ScriptPickerScope::Branch;
        let entries = self.collect_script_entries(scope);
        if entries.is_empty() {
            // Fall back to Scripts book if the branch is empty —
            // saves the user one keystroke and matches the
            // "show me something useful" expectation.
            let fallback = self.collect_script_entries(ScriptPickerScope::ScriptsBook);
            if fallback.is_empty() {
                self.status = "bund: no scripts found (try Ctrl+Z N to create one)".into();
                return;
            }
            self.modal = Modal::ScriptPicker {
                scope: ScriptPickerScope::ScriptsBook,
                entries: fallback,
                cursor: 0,
                scroll: 0,
            };
            self.status =
                "bund: no scripts in current branch — showing Scripts book".into();
            return;
        }
        self.modal = Modal::ScriptPicker {
            scope,
            entries,
            cursor: 0,
            scroll: 0,
        };
        self.status =
            "bund: ↑↓ select · Enter run · A toggle scope · Esc cancel".into();
    }

    /// Walk the requested scope and pull every Script node out.
    /// Returns them in slug-path order so the modal listing is
    /// stable across openings.
    fn collect_script_entries(
        &self,
        scope: ScriptPickerScope,
    ) -> Vec<ScriptPickerEntry> {
        let root_id: Option<Uuid> = match scope {
            ScriptPickerScope::Branch => self.current_branch_root_id(),
            ScriptPickerScope::ScriptsBook => self
                .hierarchy
                .iter()
                .find(|n| {
                    n.kind == NodeKind::Book
                        && n.parent_id.is_none()
                        && n.title.eq_ignore_ascii_case("Scripts")
                })
                .map(|n| n.id),
        };
        let Some(root_id) = root_id else {
            return Vec::new();
        };
        let mut ids = self.hierarchy.collect_subtree(root_id);
        // collect_subtree includes the root — drop it if it's
        // not itself a script.
        let mut entries: Vec<ScriptPickerEntry> = Vec::new();
        for id in ids.drain(..) {
            let Some(node) = self.hierarchy.get(id) else {
                continue;
            };
            if node.kind != NodeKind::Script {
                continue;
            }
            let slug_path = self.hierarchy.slug_path(node);
            entries.push(ScriptPickerEntry {
                id: node.id,
                title: node.title.clone(),
                slug_path,
            });
        }
        entries.sort_by(|a, b| a.slug_path.cmp(&b.slug_path));
        entries
    }

    /// The nearest book / chapter / subchapter ancestor of the
    /// tree cursor (or the cursor itself if it already names a
    /// branch). Returns `None` if no row is selected.
    fn current_branch_root_id(&self) -> Option<Uuid> {
        let (id, _) = *self.rows.get(self.tree_cursor)?;
        let node = self.hierarchy.get(id)?;
        if matches!(
            node.kind,
            NodeKind::Subchapter | NodeKind::Chapter | NodeKind::Book
        ) {
            return Some(node.id);
        }
        // Leaf node — walk ancestors until we hit a branch.
        for anc in self.hierarchy.ancestors(node) {
            if matches!(
                anc.kind,
                NodeKind::Subchapter | NodeKind::Chapter | NodeKind::Book
            ) {
                return Some(anc.id);
            }
        }
        None
    }

    // dispatch_meta_tree was absorbed into keybind::KeyBindings —
    // the meta_sub table now carries every tree-pane chord via
    // Scope::Tree entries, and resolution flows through
    // handle_meta_action → resolve_meta_sub → run_action.

    // dispatch_meta_editor + dispatch_meta_ai were absorbed into
    // keybind::KeyBindings — every chord they handled now lives in
    // the meta_sub table under Scope::Editor / Scope::Ai. See
    // run_action for the action→handler dispatch.

    fn open_quickref(&mut self) {
        self.modal = Modal::QuickRef {
            focus: self.focus,
            scroll: 0,
        };
    }

    fn open_credits(&mut self) {
        // 1.2.5+: build a fresh ratatui-image protocol over the
        // embedded `logo.png` so the credits modal can banner it
        // at the top. The image picker is None on terminals
        // without graphics support; we fall through with no logo
        // and the modal still renders fine.
        let logo = self
            .image_picker
            .as_ref()
            .and_then(|picker| {
                embedded_logo_image().map(|img| picker.new_resize_protocol(img.clone()))
            });
        self.modal = Modal::Credits { scroll: 0, logo };
        self.status = "Credits · ↑↓/PgUp/PgDn scroll · Esc close".into();
    }

    /// Ctrl+B I — open the "current book info" panel. The content is
    /// rendered each frame in `draw_book_info_modal` so figures stay
    /// fresh as the user edits; we only stash the scroll offset here.
    fn open_book_info(&mut self) {
        self.modal = Modal::BookInfo { scroll: 0 };
        self.status =
            "Book info · ↑↓/PgUp/PgDn scroll · Esc close".into();
    }

    /// Scroll handler for the BookInfo modal — mirrors
    /// `credits_handle_key`. Renderer clamps scroll to the actual line
    /// count.
    fn book_info_handle_key(&mut self, key: KeyEvent) -> bool {
        let Modal::BookInfo { scroll } = &mut self.modal else {
            return false;
        };
        match key.code {
            KeyCode::Up => {
                *scroll = scroll.saturating_sub(1);
                true
            }
            KeyCode::Down => {
                *scroll = scroll.saturating_add(1);
                true
            }
            KeyCode::PageUp => {
                *scroll = scroll.saturating_sub(10);
                true
            }
            KeyCode::PageDown => {
                *scroll = scroll.saturating_add(10);
                true
            }
            KeyCode::Home => {
                *scroll = 0;
                true
            }
            KeyCode::End => {
                *scroll = usize::MAX / 2;
                true
            }
            _ => false,
        }
    }

    /// Resolve the "current book" the user is inside: prefer the book
    /// containing the open paragraph (if any), otherwise the book
    /// containing the tree cursor. Returns the cloned node so the
    /// caller can drop the temporary `Hierarchy` it loaded.
    ///
    /// "Current book" walks `parent_id` up to the root: for a node at
    /// any depth this lands on the root Book; for a Book node it
    /// returns the node itself. Returns `None` only when the project
    /// is completely empty (no books seeded yet).
    fn current_book_node(&self, hierarchy: &Hierarchy) -> Option<Node> {
        let start_id = self
            .opened
            .as_ref()
            .map(|d| d.id)
            .or_else(|| self.rows.get(self.tree_cursor).map(|(id, _)| *id))?;

        let mut current_id = start_id;
        loop {
            let node = hierarchy.get(current_id)?;
            match node.parent_id {
                None => return Some(node.clone()),
                Some(pid) => current_id = pid,
            }
        }
    }

    fn draw_book_info_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
        scroll: usize,
    ) {
        let lines = self.build_book_info_lines();
        let total = lines.len();

        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = " Book info · Ctrl+B I ".to_string();
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1) as usize;
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };

        let max_scroll = total.saturating_sub(body_h);
        let scroll = scroll.min(max_scroll);
        let end = (scroll + body_h).min(total);
        let visible: Vec<Line<'_>> = lines[scroll..end].to_vec();
        f.render_widget(Paragraph::new(visible), body_rect);

        let at_end = end >= total;
        let more_hint = if at_end { " " } else { " · more below" };
        let hint = format!(
            " ↑↓ / PgUp/PgDn / Home/End scroll · Esc close{more_hint}    (showing {}–{} of {total}) ",
            scroll + 1,
            end
        );
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    fn build_book_info_lines(&self) -> Vec<Line<'static>> {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let dim = Style::default().add_modifier(Modifier::DIM);
        let label_color = Style::default().fg(Color::Cyan);

        let mut out: Vec<Line<'static>> = Vec::new();
        out.push(Line::from(""));

        let Ok(hierarchy) = Hierarchy::load(&self.store) else {
            out.push(Line::from(Span::styled(
                "  (could not load hierarchy)".to_string(),
                dim,
            )));
            return out;
        };

        let Some(book) = self.current_book_node(&hierarchy) else {
            out.push(Line::from(Span::styled(
                "  No book selected. Move the tree cursor onto a book \
                 (or any node inside one) and press Ctrl+B I again."
                    .to_string(),
                dim,
            )));
            return out;
        };

        out.push(Line::from(vec![
            Span::styled("  Book: ", label_color),
            Span::styled(book.title.clone(), bold),
            Span::styled(
                format!("   ({})", book.slug),
                dim,
            ),
        ]));
        if let Some(tag) = book.system_tag.as_deref() {
            out.push(Line::from(Span::styled(
                format!("    system book · tag={tag}"),
                dim,
            )));
        }
        out.push(Line::from(""));

        // 1. Paths.
        let backup_dir =
            crate::store::default_user_backup_dir(&self.layout.root);
        let artefacts_root = self.store.resolve_artefacts_dir(&self.cfg);
        let artefacts_book = artefacts_root.join(&book.slug);

        out.push(Line::from(Span::styled(
            "  Paths".to_string(),
            label_color.add_modifier(Modifier::BOLD),
        )));
        out.push(Line::from(format!("    backups:    {}", backup_dir.display())));
        out.push(Line::from(format!(
            "    artefacts:  {}",
            artefacts_book.display()
        )));
        out.push(Line::from(""));

        // 2. Stats.
        let stats = compute_book_stats(&hierarchy, &book, &self.layout.root);
        out.push(Line::from(Span::styled(
            "  Structure".to_string(),
            label_color.add_modifier(Modifier::BOLD),
        )));
        out.push(Line::from(format!(
            "    chapters:     {}",
            stats.chapters
        )));
        out.push(Line::from(format!(
            "    subchapters:  {}",
            stats.subchapters
        )));
        out.push(Line::from(format!(
            "    paragraphs:   {}",
            stats.paragraphs
        )));
        out.push(Line::from(""));

        out.push(Line::from(Span::styled(
            "  Prose".to_string(),
            label_color.add_modifier(Modifier::BOLD),
        )));
        out.push(Line::from(format!(
            "    sentences:    {}",
            stats.sentences
        )));
        out.push(Line::from(format!(
            "    words:        {}",
            stats.words
        )));
        // 250 wpm — a standard adult silent-reading speed for prose
        // (educational references typically quote 200–300 wpm). Round
        // up to whole minutes so humantime doesn't print sub-second
        // precision for an estimate that's never that precise.
        let read_pretty = if stats.words == 0 {
            "< 1m".to_string()
        } else {
            let minutes = ((stats.words as f64) / 250.0).ceil() as u64;
            humantime::format_duration(std::time::Duration::from_secs(
                minutes.max(1) * 60,
            ))
            .to_string()
        };
        out.push(Line::from(format!(
            "    reading time: {read_pretty}  (at 250 words/min)"
        )));
        out.push(Line::from(""));

        // 3. PDF status.
        let pdf_path = artefacts_book.join(format!("{}.pdf", book.slug));
        out.push(Line::from(Span::styled(
            "  Rendered PDF".to_string(),
            label_color.add_modifier(Modifier::BOLD),
        )));
        out.push(Line::from(format!(
            "    expected:   {}",
            pdf_path.display()
        )));
        match std::fs::metadata(&pdf_path) {
            Ok(meta) => {
                let created = meta
                    .created()
                    .or_else(|_| meta.modified())
                    .ok()
                    .and_then(|t| {
                        std::time::SystemTime::now()
                            .duration_since(t)
                            .ok()
                    });
                let age = match created {
                    Some(d) => format_age_humantime(d),
                    None => "(timestamp unavailable)".to_string(),
                };
                let size_kb = meta.len() / 1024;
                out.push(Line::from(vec![
                    Span::raw("    status:     "),
                    Span::styled(
                        "present".to_string(),
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!("  ({size_kb} KiB)")),
                ]));
                out.push(Line::from(format!("    created:    {age} ago")));
            }
            Err(_) => {
                out.push(Line::from(vec![
                    Span::raw("    status:     "),
                    Span::styled(
                        "missing".to_string(),
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "   (render the book to create it)".to_string(),
                        dim,
                    ),
                ]));
            }
        }
        out.push(Line::from(""));
        out
    }

    /// Ctrl+B L — open the LLM provider picker. Empty providers map
    /// yields a status-bar diagnostic instead of an empty modal.
    fn open_llm_picker(&mut self) {
        if self.cfg.llm.providers.is_empty() {
            self.status =
                "No LLM providers configured in inkhaven.hjson — add at least one under `llm.providers`."
                    .into();
            return;
        }
        let providers: Vec<String> = self.cfg.llm.providers.keys().cloned().collect();
        // Position the cursor on the currently-active default so Enter
        // is a confirm rather than a switch.
        let cursor = providers
            .iter()
            .position(|p| p == &self.cfg.llm.default)
            .unwrap_or(0);
        let initial_default = self.cfg.llm.default.clone();
        self.modal = Modal::LlmPicker {
            providers,
            cursor,
            initial_default,
        };
        self.status = "Switch LLM provider · ↑↓ / Enter to switch · Esc to cancel".into();
    }

    fn llm_picker_handle_key(&mut self, key: KeyEvent) -> bool {
        let Modal::LlmPicker {
            providers, cursor, ..
        } = &mut self.modal
        else {
            return false;
        };
        let total = providers.len();
        match key.code {
            KeyCode::Up => {
                if *cursor > 0 {
                    *cursor -= 1;
                }
                true
            }
            KeyCode::Down => {
                if *cursor + 1 < total {
                    *cursor += 1;
                }
                true
            }
            KeyCode::Home => {
                *cursor = 0;
                true
            }
            KeyCode::End => {
                *cursor = total.saturating_sub(1);
                true
            }
            KeyCode::Enter => {
                self.commit_llm_picker();
                true
            }
            _ => false,
        }
    }

    fn commit_llm_picker(&mut self) {
        let (chosen, initial_default) = match &self.modal {
            Modal::LlmPicker {
                providers,
                cursor,
                initial_default,
            } => (
                providers.get(*cursor).cloned(),
                initial_default.clone(),
            ),
            _ => return,
        };
        let Some(chosen) = chosen else {
            self.modal = Modal::None;
            return;
        };

        // No-op early-out: picking the same provider just closes the
        // modal without rewriting the file.
        if chosen == initial_default {
            self.modal = Modal::None;
            self.status = format!("LLM provider unchanged · still `{chosen}`");
            return;
        }

        // Persist to inkhaven.hjson with a targeted text edit so user
        // comments + the rest of the config survive the rewrite.
        let config_path = self.layout.config_path();
        let raw = match std::fs::read_to_string(&config_path) {
            Ok(s) => s,
            Err(e) => {
                self.status =
                    format!("LLM switch aborted: read {}: {e}", config_path.display());
                return;
            }
        };
        let updated = match set_llm_default_in_hjson(&raw, &chosen) {
            Ok(s) => s,
            Err(reason) => {
                self.status = format!(
                    "LLM switch aborted: can't rewrite {}: {reason}",
                    config_path.display()
                );
                return;
            }
        };
        if let Err(e) = std::fs::write(&config_path, &updated) {
            self.status = format!("LLM switch aborted: write {}: {e}", config_path.display());
            return;
        }

        // Update the live config + AiClient so subsequent prompts use
        // the new provider without restarting.
        self.cfg.llm.default = chosen.clone();
        match AiClient::from_config(&self.cfg.llm) {
            Ok(ai) => self.ai = ai,
            Err(e) => {
                // The file is already on disk so the next startup will
                // honour the new default — surface the error so the
                // user knows the in-memory client wasn't refreshed.
                self.status = format!(
                    "switched to `{chosen}` on disk, but couldn't refresh in-memory client: {e}"
                );
                self.modal = Modal::None;
                return;
            }
        }

        self.modal = Modal::None;
        self.status = format!(
            "LLM provider switched to `{chosen}` · saved to {}",
            config_path.display()
        );
    }

    /// Ctrl+B E — flip `sound.enabled` in the live config + on disk,
    /// and toggle the live SoundPlayer's enabled flag. No-ops on hosts
    /// without an audio device beyond updating the config (so the
    /// preference persists for a future launch on a host that does).
    fn toggle_sound(&mut self) {
        let new_value = !self.cfg.sound.enabled;
        let config_path = self.layout.config_path();
        let raw = match std::fs::read_to_string(&config_path) {
            Ok(s) => s,
            Err(e) => {
                self.status =
                    format!("sound toggle aborted: read {}: {e}", config_path.display());
                return;
            }
        };
        let updated = match set_sound_enabled_in_hjson(&raw, new_value) {
            Ok(s) => s,
            Err(reason) => {
                self.status = format!(
                    "sound toggle aborted: can't rewrite {}: {reason}",
                    config_path.display()
                );
                return;
            }
        };
        if let Err(e) = std::fs::write(&config_path, &updated) {
            self.status =
                format!("sound toggle aborted: write {}: {e}", config_path.display());
            return;
        }

        self.cfg.sound.enabled = new_value;
        if let Some(sp) = self.sound.as_mut() {
            sp.enabled = new_value;
        }
        let label = if new_value { "ON" } else { "OFF" };
        let audio_note = if self.sound.is_some() {
            ""
        } else {
            " (no audio device — silent, but preference saved)"
        };
        self.status =
            format!("Typewriter sound {label}{audio_note} · saved to {}", config_path.display());
    }

    fn draw_llm_picker_modal(&self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::LlmPicker {
            providers,
            cursor,
            initial_default,
        } = &self.modal
        else {
            return;
        };

        // Build the visible lines so we can size the modal to fit.
        let header_lines = 2; // title + blank
        let footer_lines = 2; // blank + hint
        let body_lines = providers.len();
        let height = (header_lines + body_lines + footer_lines + 2) as u16;
        let height = height.clamp(8, area.height.saturating_sub(2));

        // Widest provider name + model for column alignment.
        let max_name = providers.iter().map(|p| p.chars().count()).max().unwrap_or(8);
        let max_model = providers
            .iter()
            .filter_map(|p| self.cfg.llm.providers.get(p).map(|c| c.model.chars().count()))
            .max()
            .unwrap_or(8);
        let width = (max_name + max_model + 28) as u16;
        let width = width.clamp(50, area.width.saturating_sub(6));

        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Switch LLM provider · Ctrl+B L ")
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(""));
        for (i, name) in providers.iter().enumerate() {
            let prov = self.cfg.llm.providers.get(name);
            let model = prov.map(|p| p.model.as_str()).unwrap_or("?");
            let api_key_state = prov
                .and_then(|p| p.api_key_env.clone())
                .map(|env| {
                    if std::env::var(&env).is_ok() {
                        format!("· {env} set")
                    } else {
                        format!("· {env} MISSING")
                    }
                })
                .unwrap_or_else(|| "· local (no key)".to_string());
            let marker = if i == *cursor { "›" } else { " " };
            let current_tag = if name == initial_default {
                "  (current)"
            } else {
                ""
            };
            let name_padded = format!("{name:<width$}", width = max_name);
            let model_padded = format!("{model:<width$}", width = max_model);
            let row = format!(
                "  {marker} {name_padded}   {model_padded}   {api_key_state}{current_tag}"
            );
            let style = if i == *cursor {
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .add_modifier(Modifier::BOLD)
            } else if name == initial_default {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(row, style)));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  ↑↓ to select · Enter to switch · Esc to cancel".to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )));

        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }

    /// Try the in-`#image(…)` picker; returns false when the cursor
    /// isn't inside an image call (so the caller can fall through to
    /// Places RAG). Returns true even when the picker is empty —
    /// "you're inside `#image()` but this paragraph has no sibling
    /// images" is still better feedback than silently jumping to
    /// Places RAG.
    fn try_open_image_picker(&mut self) -> bool {
        let Some(doc) = self.opened.as_ref() else {
            return false;
        };
        let (row, col) = doc.textarea.cursor();
        let line = doc.textarea.lines().get(row).cloned().unwrap_or_default();
        let ctx = match detect_image_call_context(&line, col) {
            Some(c) => c,
            None => return false,
        };
        // Find sibling Image nodes in the paragraph's parent.
        let entries = self.sibling_image_entries(doc.id);
        if entries.is_empty() {
            self.status = "no sibling images at this level — import one with F3 first".into();
            self.modal = Modal::ImagePicker {
                entries,
                cursor: 0,
                close_quote: !ctx.closing_quote_present,
            };
            return true;
        }
        self.status =
            "↑↓ select · Enter insert · Esc cancel".into();
        self.modal = Modal::ImagePicker {
            entries,
            cursor: 0,
            close_quote: !ctx.closing_quote_present,
        };
        true
    }

    /// Sibling Image nodes of the paragraph identified by `para_id`,
    /// sorted by their `order` field. Each entry carries the filename
    /// (already in `NN-slug.<ext>` form), the display title, and the
    /// file size for the picker readout.
    fn sibling_image_entries(&self, para_id: Uuid) -> Vec<ImagePickerEntry> {
        let Some(para) = self.hierarchy.get(para_id) else {
            return Vec::new();
        };
        let mut out: Vec<ImagePickerEntry> = self
            .hierarchy
            .children_of(para.parent_id)
            .into_iter()
            .filter(|n| n.kind == NodeKind::Image)
            .map(|n| {
                let size_bytes = n
                    .file
                    .as_ref()
                    .map(|rel| {
                        std::fs::metadata(self.layout.root.join(rel))
                            .map(|m| m.len())
                            .unwrap_or(0)
                    })
                    .unwrap_or(0);
                ImagePickerEntry {
                    fname: n.fs_name(),
                    title: n.title.clone(),
                    size_bytes,
                }
            })
            .collect();
        out.sort_by(|a, b| a.fname.cmp(&b.fname));
        out
    }

    fn image_picker_handle_key(&mut self, key: KeyEvent) -> bool {
        let Modal::ImagePicker {
            entries, cursor, ..
        } = &mut self.modal
        else {
            return false;
        };
        let total = entries.len();
        match key.code {
            KeyCode::Up => {
                if *cursor > 0 {
                    *cursor -= 1;
                }
                true
            }
            KeyCode::Down => {
                if *cursor + 1 < total {
                    *cursor += 1;
                }
                true
            }
            KeyCode::Home => {
                *cursor = 0;
                true
            }
            KeyCode::End => {
                *cursor = total.saturating_sub(1);
                true
            }
            KeyCode::Enter => {
                self.commit_image_picker();
                true
            }
            _ => false,
        }
    }

    fn commit_image_picker(&mut self) {
        let (fname, close_quote) = match &self.modal {
            Modal::ImagePicker {
                entries,
                cursor,
                close_quote,
            } => match entries.get(*cursor) {
                Some(e) => (e.fname.clone(), *close_quote),
                None => {
                    self.modal = Modal::None;
                    return;
                }
            },
            _ => return,
        };
        // Insert filename + optional `"` at the cursor in the editor.
        let insert = if close_quote {
            format!("{fname}\"")
        } else {
            fname.clone()
        };
        if let Some(doc) = self.opened.as_mut() {
            doc.textarea.insert_str(&insert);
            doc.dirty = true;
        }
        self.modal = Modal::None;
        self.status = format!("inserted `{fname}` into #image(…)");
    }

    fn draw_image_picker_modal(&self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::ImagePicker {
            entries, cursor, ..
        } = &self.modal
        else {
            return;
        };
        let header_lines = 2usize;
        let footer_lines = 2usize;
        let body_lines = entries.len().max(1);
        let height = ((header_lines + body_lines + footer_lines + 2) as u16)
            .clamp(8, area.height.saturating_sub(2));
        let max_name = entries
            .iter()
            .map(|e| e.fname.chars().count())
            .max()
            .unwrap_or(16);
        let max_title = entries
            .iter()
            .map(|e| e.title.chars().count())
            .max()
            .unwrap_or(16);
        let width = ((max_name + max_title + 24) as u16).clamp(50, area.width.saturating_sub(6));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Pick an image · Ctrl+B P ")
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(""));
        if entries.is_empty() {
            lines.push(Line::from(Span::styled(
                "  No Image siblings at this level. Use F3 to import one,"
                    .to_string(),
                Style::default().add_modifier(Modifier::DIM),
            )));
            lines.push(Line::from(Span::styled(
                "  then re-run Ctrl+B P inside the #image(\"…\") call."
                    .to_string(),
                Style::default().add_modifier(Modifier::DIM),
            )));
        } else {
            for (i, e) in entries.iter().enumerate() {
                let marker = if i == *cursor { "›" } else { " " };
                let name_padded =
                    format!("{n:<width$}", n = e.fname, width = max_name);
                let title_padded =
                    format!("{t:<width$}", t = e.title, width = max_title);
                let size_kib = e.size_bytes / 1024;
                let row = format!("  {marker} {name_padded}   {title_padded}   ({size_kib} KiB)");
                let style = if i == *cursor {
                    Style::default()
                        .add_modifier(Modifier::REVERSED)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(row, style)));
            }
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  ↑↓ select · Enter insert · Esc cancel".to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )));
        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }

    fn open_function_picker(&mut self) {
        if self.opened.is_none() {
            self.status = "function picker needs an open paragraph".into();
            return;
        }
        self.modal = Modal::FunctionPicker {
            filter: TextInput::new(),
            cursor: 0,
        };
        self.status = "Type to filter · ↑↓ select · Enter insert · Esc cancel".into();
    }

    fn function_picker_handle_key(&mut self, key: KeyEvent) -> bool {
        // Recompute the filtered list each keystroke so the cursor
        // stays attached to a visible row.
        let (filter_text, mut cursor_value) = match &self.modal {
            Modal::FunctionPicker { filter, cursor } => (filter.as_str().to_string(), *cursor),
            _ => return false,
        };
        let matches = filter_functions(&filter_text);
        let total = matches.len();
        let mut closed = false;
        let mut commit_now = false;
        let mut filter_dirty = false;
        match key.code {
            KeyCode::Up => {
                if cursor_value > 0 {
                    cursor_value -= 1;
                }
            }
            KeyCode::Down => {
                if cursor_value + 1 < total {
                    cursor_value += 1;
                }
            }
            KeyCode::Home => cursor_value = 0,
            KeyCode::End => cursor_value = total.saturating_sub(1),
            KeyCode::PageUp => cursor_value = cursor_value.saturating_sub(10),
            KeyCode::PageDown => {
                cursor_value = (cursor_value + 10).min(total.saturating_sub(1));
            }
            KeyCode::Enter => commit_now = true,
            KeyCode::Esc => closed = true,
            _ => {
                if let Modal::FunctionPicker { filter, cursor } = &mut self.modal {
                    handle_text_input_key(filter, key);
                    *cursor = 0;
                    filter_dirty = true;
                    let _ = cursor;
                }
            }
        }
        if filter_dirty {
            return true;
        }
        if closed {
            self.modal = Modal::None;
            return true;
        }
        if commit_now {
            self.commit_function_picker(&matches, cursor_value);
            return true;
        }
        if let Modal::FunctionPicker { cursor, .. } = &mut self.modal {
            *cursor = cursor_value;
        }
        true
    }

    fn commit_function_picker(&mut self, matches: &[super::typst_funcs::TypstFn], cursor_value: usize) {
        let Some(picked) = matches.get(cursor_value).copied() else {
            self.modal = Modal::None;
            return;
        };
        // Detect the syntactic mode at the cursor via tree-sitter-
        // typst so we only emit the `#` prefix when the cursor is in
        // markup. Inside `{ code }`, function-call arguments, `let`
        // RHS, or math, the bare identifier is what typst expects.
        let mode = self
            .opened
            .as_ref()
            .map(|doc| {
                let source = doc.textarea.lines().join("\n");
                let (row, col) = doc.textarea.cursor();
                let byte = byte_offset_for_cursor(&source, row, col);
                super::highlight::typst_mode_at(&source, byte)
            })
            .unwrap_or(super::highlight::TypstMode::Markup);
        let prefix = mode.call_prefix();
        let opener = format!("{prefix}{}(", picked.name);
        if let Some(doc) = self.opened.as_mut() {
            doc.textarea.insert_str(&opener);
            doc.textarea.insert_str(")");
            doc.textarea.move_cursor(CursorMove::Back);
            doc.dirty = true;
        }
        self.modal = Modal::None;
        let mode_tag = match mode {
            super::highlight::TypstMode::Markup => "markup",
            super::highlight::TypstMode::Code => "code",
            super::highlight::TypstMode::Math => "math",
        };
        self.status = format!("inserted {prefix}{}( … ) · {mode_tag} mode", picked.name);
    }

    fn draw_function_picker_modal(&self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::FunctionPicker { filter, cursor } = &self.modal else {
            return;
        };
        let matches = filter_functions(filter.as_str());
        let width = area.width.saturating_sub(6).max(60);
        let height = area.height.saturating_sub(4).max(14);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let title = " Typst function · Ctrl+B F ".to_string();
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        // 3 rows of chrome: filter, blank spacer, footer.
        let filter_h: u16 = 2;
        let footer_h: u16 = 2;
        let list_h = inner.height.saturating_sub(filter_h + footer_h);
        let filter_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: filter_h,
        };
        let list_rect = Rect {
            x: inner.x,
            y: inner.y + filter_h,
            width: inner.width,
            height: list_h,
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + filter_h + list_h,
            width: inner.width,
            height: footer_h,
        };

        let cursor_char = '│';
        let filter_lines = vec![
            Line::from(Span::styled(
                format!(" › Filter: {}", filter.render_with_cursor(cursor_char)),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                format!(
                    "   {} match{} of {}",
                    matches.len(),
                    if matches.len() == 1 { "" } else { "es" },
                    super::typst_funcs::all().len()
                ),
                Style::default().add_modifier(Modifier::DIM),
            )),
        ];
        f.render_widget(Paragraph::new(filter_lines), filter_rect);

        // List body — scroll so the cursor is always in view.
        let body_height = list_h as usize;
        let total = matches.len();
        let cursor = (*cursor).min(total.saturating_sub(1));
        let scroll = if cursor >= body_height {
            cursor - body_height + 1
        } else {
            0
        };
        let max_name = matches
            .iter()
            .map(|f| f.name.chars().count())
            .max()
            .unwrap_or(8);

        let mut rows: Vec<Line<'static>> = Vec::new();
        if matches.is_empty() {
            rows.push(Line::from(Span::styled(
                "  (no functions match the filter)".to_string(),
                Style::default().add_modifier(Modifier::DIM),
            )));
        }
        let body_end = (scroll + body_height).min(total);
        for i in scroll..body_end {
            let entry = matches[i];
            let marker = if i == cursor { "›" } else { " " };
            let name_padded =
                format!("{n:<width$}", n = entry.name, width = max_name);
            let line = format!("  {marker} {name_padded}   {desc}", desc = entry.description);
            let style = if i == cursor {
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            rows.push(Line::from(Span::styled(line, style)));
        }
        // Also include the signature underneath the selected entry as
        // a hint row. Kept narrow to avoid pushing the list off-screen.
        f.render_widget(Paragraph::new(rows), list_rect);

        let signature_hint = matches
            .get(cursor)
            .map(|f| format!(" sig: {}", f.signature))
            .unwrap_or_default();
        let hint = format!(
            "{signature_hint}\n ↑↓ select · Enter inserts #name(…) · Esc cancel"
        );
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            )))
            .wrap(Wrap { trim: false }),
            footer_rect,
        );
    }

    fn draw_image_preview_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        // Pull the variant fields out by value (cloning the cheap
        // strings/numbers) and take a `&mut` borrow of `proto` only
        // for the render call — keeps the modal field accessible
        // for read elsewhere if needed.
        let Modal::ImagePreview {
            title,
            fs_rel,
            size_bytes,
            proto,
        } = &mut self.modal
        else {
            return;
        };

        let width = area.width.saturating_sub(4).max(40);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let title_line = format!(
            " 🖼 {title}  ·  {fs_rel}  ·  {size_bytes} bytes "
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title_line)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        // Reserve the last inner row for the hint line.
        let body_h = inner.height.saturating_sub(1);
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: body_h,
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + body_h,
            width: inner.width,
            height: 1,
        };

        let widget = ratatui_image::StatefulImage::new();
        f.render_stateful_widget(widget, body_rect, proto);

        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  Esc closes  ·  resize the terminal to re-fit ".to_string(),
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// Ctrl+V R floating preview. Same plumbing as the image-
    /// preview modal — ratatui-image's StatefulImage widget
    /// repaints on every frame so a terminal resize Just Works.
    /// Multi-page documents: ← / → cycle between page protos.
    fn draw_rendered_preview_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::RenderedPreview {
            title,
            pages,
            current_page,
            ..
        } = &mut self.modal
        else {
            return;
        };
        let total = pages.len();
        let idx = (*current_page).min(total.saturating_sub(1));
        let page = match pages.get_mut(idx) {
            Some(p) => p,
            None => return,
        };
        let preview_width = page.width;
        let preview_height = page.height;

        let width = area.width.saturating_sub(4).max(40);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let pages_note = if total > 1 {
            format!(" · page {}/{}", idx + 1, total)
        } else {
            String::new()
        };
        let title_line = format!(
            " 🖨 {title}  ·  {preview_width}×{preview_height}{pages_note} "
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title_line)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1);
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: body_h,
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + body_h,
            width: inner.width,
            height: 1,
        };

        let widget = ratatui_image::StatefulImage::new();
        f.render_stateful_widget(widget, body_rect, &mut page.proto);

        let hint = if total > 1 {
            "  ← / → navigate  ·  S saves current  ·  A saves all  ·  Esc closes ".to_string()
        } else {
            "  Esc closes  ·  S saves full-DPI PNG  ·  A saves all (same here) ".to_string()
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// Save-as picker triggered by `S` in the rendered preview.
    /// Same dimensions / style as the markdown save-as picker so
    /// the UX is consistent.
    fn draw_save_rendered_png_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::SaveRenderedPng { input, title, .. } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(8).clamp(40, 96);
        let height: u16 = 7;
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Save rendered PNG · {title} "))
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let cursor = '│';
        let body = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!(" Path: {}", input.render_with_cursor(cursor)),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Enter saves · Esc cancels · ~/ expands to home".to_string(),
                Style::default().add_modifier(Modifier::DIM),
            )),
        ];
        f.render_widget(Paragraph::new(body), inner);
    }

    fn cycle_ai_mode(&mut self) {
        self.ai_mode = self.ai_mode.next();
        self.status = match self.ai_mode {
            AiMode::None => "AI scope: None (only the prompt is sent)".into(),
            other => format!(
                "AI scope: {} (will prepend matching context to next prompt)",
                other.label()
            ),
        };
    }

    fn toggle_inference_mode(&mut self) {
        self.inference_mode = self.inference_mode.toggle();
        self.status = match self.inference_mode {
            InferenceMode::Local => {
                "Inference: Local (model uses only supplied context)".into()
            }
            InferenceMode::Full => {
                "Inference: Full (model uses context + its own knowledge)".into()
            }
        };
    }

    /// Assemble the prefix the active AI scope should prepend to the
    /// user's prompt. Returns `Ok(Some(text))` when the scope produced
    /// content, `Ok(None)` for `AiMode::None`, or `Err(reason)` when the
    /// scope was requested but produced nothing (no selection, no open
    /// paragraph, no enclosing branch). The status message in `Err` is
    /// surfaced to the user verbatim.
    fn build_ai_mode_context(&self) -> Result<Option<String>, String> {
        match self.ai_mode {
            AiMode::None => Ok(None),
            AiMode::Selection => {
                let Some(doc) = self.opened.as_ref() else {
                    return Err("AI scope `Selection` needs an open paragraph".into());
                };
                let Some(((r1, c1), (r2, c2))) = doc.textarea.selection_range() else {
                    return Err("AI scope `Selection` needs a non-empty selection in the editor".into());
                };
                let text = slice_lines(doc.textarea.lines(), r1, c1, r2, c2);
                if text.trim().is_empty() {
                    return Err("AI scope `Selection` selection was empty".into());
                }
                Ok(Some(format!(
                    "── Editor selection ──\n{text}\n── end selection ──"
                )))
            }
            AiMode::Paragraph => {
                let Some(doc) = self.opened.as_ref() else {
                    return Err("AI scope `Paragraph` needs an open paragraph".into());
                };
                let live = doc.textarea.lines().join("\n");
                let mut out = String::new();
                if let Some(split) = doc.split.as_ref() {
                    // In split-edit mode the user sees the snapshot AND
                    // the live buffer side by side; include both so the
                    // model can compare.
                    out.push_str("── Paragraph snapshot (split-edit copy) ──\n");
                    out.push_str(&split.snapshot_lines.join("\n"));
                    out.push_str("\n── end snapshot ──\n\n");
                }
                out.push_str("── Paragraph: ");
                out.push_str(&doc.title);
                out.push_str(" ──\n");
                out.push_str(&live);
                out.push_str("\n── end paragraph ──");
                // 1.2.4+: also include every linked paragraph's
                // body so the model has the related-material the
                // user explicitly curated. Direct outgoing only
                // (matches the status-bar "links N" count). Bodies
                // come from disk (saved state) — same source the
                // export / similar-paragraph paths use.
                let linked_ids: Vec<Uuid> = self
                    .hierarchy
                    .get(doc.id)
                    .map(|n| n.linked_paragraphs.clone())
                    .unwrap_or_default();
                for id in linked_ids {
                    let Some(linked) = self.hierarchy.get(id) else { continue };
                    let Some(rel) = linked.file.as_ref() else { continue };
                    let abs = self.layout.root.join(rel);
                    let body = std::fs::read_to_string(&abs).unwrap_or_default();
                    out.push_str("\n\n── Linked paragraph: ");
                    out.push_str(&linked.title);
                    out.push_str(" ──\n");
                    out.push_str(&body);
                    out.push_str("\n── end linked paragraph ──");
                }
                Ok(Some(out))
            }
            AiMode::Subchapter | AiMode::Chapter | AiMode::Book => {
                let scope_kind = match self.ai_mode {
                    AiMode::Subchapter => NodeKind::Subchapter,
                    AiMode::Chapter => NodeKind::Chapter,
                    AiMode::Book => NodeKind::Book,
                    _ => unreachable!(),
                };
                let mode_label = self.ai_mode.label();
                // Anchor on the open paragraph if any, otherwise the tree
                // cursor — gives the user a sensible default whether they
                // were editing or browsing when they cycled to this scope.
                let anchor_id = self
                    .opened
                    .as_ref()
                    .map(|d| d.id)
                    .or_else(|| self.rows.get(self.tree_cursor).map(|(id, _)| *id))
                    .ok_or_else(|| {
                        format!("AI scope `{mode_label}` needs an open paragraph or tree cursor")
                    })?;
                let anchor = self
                    .hierarchy
                    .get(anchor_id)
                    .ok_or_else(|| format!("AI scope `{mode_label}` anchor vanished"))?;
                // Walk up to the enclosing branch of `scope_kind`. The
                // anchor itself counts if it is already that kind.
                let scope_node = if anchor.kind == scope_kind {
                    Some(anchor.clone())
                } else {
                    self.hierarchy
                        .ancestors(anchor)
                        .into_iter()
                        .find(|n| n.kind == scope_kind)
                        .cloned()
                };
                let Some(scope_node) = scope_node else {
                    return Err(format!(
                        "AI scope `{mode_label}` requires the cursor to be inside a {}",
                        scope_kind.as_str()
                    ));
                };
                let mut chunks: Vec<String> = Vec::new();
                for id in self.hierarchy.collect_subtree(scope_node.id) {
                    let Some(node) = self.hierarchy.get(id) else {
                        continue;
                    };
                    if node.kind != NodeKind::Paragraph {
                        continue;
                    }
                    if let Ok(Some(bytes)) = self.store.get_content(node.id) {
                        let body = String::from_utf8_lossy(&bytes).to_string();
                        chunks.push(format!(
                            "── {} ──\n{}",
                            self.title_breadcrumb(node.id),
                            body
                        ));
                    }
                }
                if chunks.is_empty() {
                    return Err(format!(
                        "AI scope `{mode_label}` `{}` has no paragraphs to send",
                        scope_node.title
                    ));
                }
                let header = format!(
                    "── {} context: {} ({} paragraph(s)) ──",
                    mode_label,
                    scope_node.title,
                    chunks.len()
                );
                Ok(Some(format!(
                    "{header}\n\n{}\n── end {} context ──",
                    chunks.join("\n\n"),
                    mode_label.to_lowercase()
                )))
            }
        }
    }

    fn clear_chat_history(&mut self) {
        let turns = self.chat_history.len();
        self.chat_history.clear();
        self.pending_chat_user_msg = None;
        self.inference = None;
        // Reset chat-history scroll / search / selection — there's
        // nothing left to scroll or act on.
        self.chat_history_scroll = 0;
        self.chat_search = None;
        self.chat_selection = None;
        // Also dismiss any active grammar-correction overlay — the AI
        // result it derived from is being discarded, so keeping a
        // baseline tied to a forgotten correction is confusing.
        if let Some(doc) = self.opened.as_mut() {
            doc.correction_baseline = None;
        }
        self.status = if turns == 0 {
            "AI chat history already empty".into()
        } else {
            format!("AI chat cleared ({turns} turn(s) discarded)")
        };
    }

    fn open_help_query_modal(&mut self) {
        self.modal = Modal::HelpQuery {
            input: TextInput::new(),
        };
        self.status = "Help — type a question, Enter to ask, Esc to cancel".into();
    }

    /// Run a Help-book RAG inference for `query`. Builds a constrained
    /// prompt — the model is instructed to answer using ONLY the supplied
    /// Help excerpts and to admit when the context is insufficient — then
    /// streams the result into the AI pane. The AI pane is read-only by
    /// construction (no editor lives there), so the user can scroll the
    /// answer but can't edit it.
    /// Ctrl+B W toggles full-screen "typewriter mode". When on, every
    /// pane except the editor (and any floating modal) is hidden;
    /// focus is forced onto the editor so typing lands in the
    /// buffer. The same chord disables it.
    fn toggle_typewriter_mode(&mut self) {
        self.typewriter_mode = !self.typewriter_mode;
        if self.typewriter_mode {
            self.ai_fullscreen = false; // the two fullscreens are exclusive
            // Force focus to the editor so the user can start typing
            // immediately; the search bar / AI prompt are hidden
            // anyway, so leaving focus on them would be confusing.
            self.change_focus(Focus::Editor);
            self.status = "typewriter mode · Ctrl+B W to exit".into();
        } else {
            self.status = "typewriter mode off".into();
        }
    }

    /// Ctrl+B K toggles full-screen AI mode. Layout: top area split
    /// 50/50 — AI pane on the left, chat history on the right —
    /// over a full-width AI prompt at the bottom. Same chord
    /// returns to the four-pane layout.
    fn toggle_ai_fullscreen(&mut self) {
        self.ai_fullscreen = !self.ai_fullscreen;
        // Always start scrolled to the bottom (newest visible). The
        // user can PageUp to walk back through the history.
        self.chat_history_scroll = 0;
        // Wipe any in-flight chat search / selection; the layout
        // transition is an obvious break in user intent.
        self.chat_search = None;
        self.chat_selection = None;
        if self.ai_fullscreen {
            self.typewriter_mode = false; // exclusive with typewriter
            // Restore previously-saved chat history if the in-memory
            // list is currently empty. The user explicitly asked for
            // "if chat is empty, restore from previous state" — never
            // overwrite a live session.
            if self.chat_history.is_empty() {
                match self.load_chat_history_from_disk() {
                    Ok(turns) if turns > 0 => {
                        self.status = format!(
                            "AI fullscreen · restored {turns} turn(s) · Ctrl+B K to exit · Ctrl+F to search history"
                        );
                    }
                    Ok(_) => {
                        self.status =
                            "AI fullscreen · ↑↓/PgUp/PgDn scrolls history · Ctrl+F search · Ctrl+B K to exit".into();
                    }
                    Err(e) => {
                        tracing::warn!("chat history restore failed: {e}");
                        self.status =
                            "AI fullscreen · ↑↓/PgUp/PgDn scrolls history · Ctrl+F search · Ctrl+B K to exit".into();
                    }
                }
            } else {
                self.status =
                    "AI fullscreen · ↑↓/PgUp/PgDn scrolls history · Ctrl+F search · Ctrl+B K to exit".into();
            }
            // Drop focus onto the AI prompt so the user can start
            // typing the next message immediately — the AI pane has
            // no input role and the editor / tree / search bar are
            // hidden in this layout anyway.
            self.change_focus(Focus::AiPrompt);
        } else {
            // Persist the current chat to disk before leaving the
            // layout. Best-effort: write failures are logged but
            // don't block the toggle.
            match self.save_chat_history_to_disk() {
                Ok(()) => {
                    self.status = format!(
                        "AI fullscreen off · {} turn(s) saved",
                        self.chat_history.len()
                    );
                }
                Err(e) => {
                    tracing::warn!("chat history save failed: {e}");
                    self.status = "AI fullscreen off (chat history save failed — see logs)".into();
                }
            }
        }
    }

    /// Path used by the chat-history persistence hooks. Lives next
    /// to `.inkhaven-backup.json` and `.session.json` inside the
    /// project root.
    fn chat_history_path(&self) -> std::path::PathBuf {
        self.layout.root.join(".inkhaven-chat.json")
    }

    /// Write the in-memory `chat_history` to disk. Empty history
    /// removes the file so a stale list doesn't haunt the next
    /// session.
    fn save_chat_history_to_disk(&self) -> std::io::Result<()> {
        let path = self.chat_history_path();
        if self.chat_history.is_empty() {
            // Nothing to save — clean up any prior file so the next
            // entry doesn't restore a phantom.
            if path.exists() {
                std::fs::remove_file(&path)?;
            }
            return Ok(());
        }
        let json = serde_json::to_string_pretty(&self.chat_history)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(&path, json)
    }

    /// Load the on-disk chat history into `chat_history`. Returns
    /// the number of turns loaded (0 when the file is absent or
    /// empty). Parse / IO errors propagate so the caller can log.
    fn load_chat_history_from_disk(&mut self) -> std::io::Result<usize> {
        let path = self.chat_history_path();
        if !path.exists() {
            return Ok(0);
        }
        let bytes = std::fs::read(&path)?;
        if bytes.is_empty() {
            return Ok(0);
        }
        let history: Vec<ChatTurn> = serde_json::from_slice(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let n = history.len();
        self.chat_history = history;
        Ok(n)
    }

    /// Editor meta `Ctrl+B R`: advance the open paragraph's status one
    /// step through the workflow ring. The cycle wraps back to None
    /// after Ready; pressing R repeatedly walks the whole sequence
    /// without any other UI. Persisted to bdslib so it survives the
    /// next launch.
    /// Single-paragraph status cycle, used by the multi-select
    /// path in `cycle_paragraph_status`. No hook firing — that's
    /// reserved for the explicit one-paragraph chord so a bulk
    /// op doesn't fire 30 hooks in sequence.
    fn cycle_status_single(&mut self, id: Uuid) -> std::result::Result<(), String> {
        let node = self
            .hierarchy
            .get(id)
            .cloned()
            .ok_or_else(|| format!("paragraph {id} not in hierarchy"))?;
        if node.kind != NodeKind::Paragraph {
            return Err(format!("`{}` is not a paragraph", node.title));
        }
        let next = next_status(node.status.as_deref());
        let mut updated = node.clone();
        updated.status = if next == "None" {
            None
        } else {
            Some(next.to_string())
        };
        updated.target_hit_at_status = None;
        self.store
            .raw()
            .update_metadata(id, updated.to_json())
            .map_err(|e| format!("store update: {e}"))?;
        Ok(())
    }

    fn cycle_paragraph_status(&mut self) {
        // 1.2.4+: when the tree has a multi-select set, apply
        // the cycle to every marked paragraph instead of just
        // the open one.
        if !self.tree_marked.is_empty() {
            let ids: Vec<Uuid> = self.tree_marked.iter().copied().collect();
            let mut ok = 0usize;
            let mut fail = 0usize;
            for id in &ids {
                if self.cycle_status_single(*id).is_ok() {
                    ok += 1;
                } else {
                    fail += 1;
                }
            }
            self.reload_hierarchy();
            self.status = if fail == 0 {
                format!("status cycled on {ok} paragraph(s)")
            } else {
                format!("status cycled on {ok} · {fail} failed")
            };
            return;
        }
        // 1.2.4+: when invoked without an open paragraph (e.g.
        // from the tree pane via `O`), fall back to the
        // cursor's row. Lets tree O cycle status without
        // first having to open the paragraph.
        let id = match self.opened.as_ref().map(|d| d.id) {
            Some(id) => id,
            None => match self.rows.get(self.tree_cursor) {
                Some((id, _)) => *id,
                None => {
                    self.status = "no paragraph selected".into();
                    return;
                }
            },
        };
        let Some(node) = self.hierarchy.get(id).cloned() else {
            self.status = "couldn't find the target paragraph in the hierarchy".into();
            return;
        };
        if node.kind != NodeKind::Paragraph {
            self.status =
                format!("status cycle: `{}` is not a paragraph", node.title);
            return;
        }
        let next = next_status(node.status.as_deref());
        let mut updated = node.clone();
        updated.status = if next == "None" {
            None
        } else {
            Some(next.to_string())
        };
        // Manual cycle clears the auto-promote bookkeeping so a
        // future save that's still above target will re-promote
        // from whichever status the user just rolled into.
        updated.target_hit_at_status = None;
        if let Err(e) = self
            .store
            .raw()
            .update_metadata(id, updated.to_json())
        {
            self.status = format!("status update failed: {e}");
            return;
        }
        // Refresh hierarchy so the status reads back next frame.
        self.reload_hierarchy();
        // Progress event log — feeds the status-ladder bar in
        // Ctrl+V G and the "promoted-to-Final this week" counts.
        let from_label = display_status(node.status.as_deref()).to_ascii_lowercase();
        let to_label = next.to_ascii_lowercase();
        let total_words = node.word_count.max(0) as i64;
        let book_id = self.book_of_node(id);
        crate::progress::record_status_change(
            id, book_id, &from_label, &to_label, total_words,
        );
        // hook.on_status_promoted ( uuid from_status to_status -- )
        // Fires on every transition — manual cycles AND the
        // auto-promote path. Scripts that want to act only on
        // promotions (not demotions / wraps to "none") can
        // inspect the labels.
        crate::scripting::hooks::fire(
            "hook.on_status_promoted",
            vec![
                rust_dynamic::value::Value::from_string(id.to_string()),
                rust_dynamic::value::Value::from_string(from_label.clone()),
                rust_dynamic::value::Value::from_string(to_label.clone()),
            ],
        );
        self.status = format!("status: `{}` → `{}`", display_status(node.status.as_deref()), next);
    }

    /// Open the floating Ctrl+B 1..7 status-filter modal for
    /// `target`, scoped to the tree cursor's enclosing branch.
    fn open_status_filter(&mut self, target: &'static str) {
        let (scope_id, scope_label) = self.resolve_status_filter_scope();
        let entries = self.collect_status_entries(target, scope_id);
        let count = entries.len();
        self.modal = Modal::StatusFilter {
            status_label: target,
            scope: scope_label.clone(),
            entries,
            cursor: 0,
        };
        self.status = format!(
            "status filter [{target}] · {scope_label} · {count} paragraph(s) · R/- cycles · Enter opens"
        );
    }

    /// "Current scope" for the status filter: the cursor's node when
    /// it's a branch; otherwise the nearest non-paragraph ancestor.
    /// Returns `(scope_id, breadcrumb)`. None scope = project-wide.
    fn resolve_status_filter_scope(&self) -> (Option<Uuid>, String) {
        let cursor_id = self
            .rows
            .get(self.tree_cursor)
            .map(|(id, _)| *id);
        let mut cur = cursor_id;
        while let Some(id) = cur {
            let Some(node) = self.hierarchy.get(id) else {
                break;
            };
            if node.kind != NodeKind::Paragraph && node.kind != NodeKind::Image {
                return (Some(id), self.title_breadcrumb(id));
            }
            cur = node.parent_id;
        }
        (None, "entire project".to_string())
    }

    /// Walk every paragraph that is (a) inside `scope_id`'s subtree
    /// when given, and (b) tagged with `target`. Sorted by breadcrumb
    /// so paragraphs from the same parent cluster together.
    fn collect_status_entries(
        &self,
        target: &'static str,
        scope_id: Option<Uuid>,
    ) -> Vec<StatusFilterEntry> {
        let allowed_ids: Option<std::collections::HashSet<Uuid>> =
            scope_id.map(|id| self.hierarchy.collect_subtree(id).into_iter().collect());
        let mut entries: Vec<StatusFilterEntry> = Vec::new();
        for node in self.hierarchy.iter() {
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            if let Some(allowed) = &allowed_ids {
                if !allowed.contains(&node.id) {
                    continue;
                }
            }
            if display_status(node.status.as_deref()) != target {
                continue;
            }
            entries.push(StatusFilterEntry {
                id: node.id,
                title: node.title.clone(),
                breadcrumb: self.title_breadcrumb(node.id),
            });
        }
        entries.sort_by(|a, b| a.breadcrumb.cmp(&b.breadcrumb));
        entries
    }

    fn status_filter_handle_key(&mut self, key: KeyEvent) -> bool {
        // Navigation-only branches first; status cycling has its own
        // path because it needs the full `self` borrow.
        let advance: Option<bool> = match key.code {
            KeyCode::Char('r') | KeyCode::Char('R') => Some(true),
            KeyCode::Char('-') | KeyCode::Backspace => Some(false),
            _ => None,
        };
        if let Some(forward) = advance {
            self.cycle_status_in_filter(forward);
            return true;
        }

        let Modal::StatusFilter { entries, cursor, .. } = &mut self.modal else {
            return false;
        };
        let total = entries.len();
        match key.code {
            KeyCode::Up => {
                if *cursor > 0 {
                    *cursor -= 1;
                }
                true
            }
            KeyCode::Down => {
                if *cursor + 1 < total {
                    *cursor += 1;
                }
                true
            }
            KeyCode::Home => {
                *cursor = 0;
                true
            }
            KeyCode::End => {
                *cursor = total.saturating_sub(1);
                true
            }
            KeyCode::PageUp => {
                *cursor = cursor.saturating_sub(10);
                true
            }
            KeyCode::PageDown => {
                *cursor = (*cursor + 10).min(total.saturating_sub(1));
                true
            }
            KeyCode::Enter => {
                self.commit_status_filter();
                true
            }
            _ => false,
        }
    }

    /// Step the highlighted paragraph's status forward or backward
    /// in the workflow ring (without leaving the modal). The list is
    /// re-collected against the original filter — if the paragraph
    /// no longer matches it disappears and the next row slides up.
    /// The cursor index is clamped to stay inside the (possibly
    /// shorter) list.
    fn cycle_status_in_filter(&mut self, forward: bool) {
        let (target_status, paragraph_id, prior_cursor) = match &self.modal {
            Modal::StatusFilter { status_label, entries, cursor, .. } => {
                let Some(entry) = entries.get(*cursor) else {
                    return;
                };
                (*status_label, entry.id, *cursor)
            }
            _ => return,
        };
        let Some(node) = self.hierarchy.get(paragraph_id).cloned() else {
            return;
        };
        let new_label = if forward {
            next_status(node.status.as_deref())
        } else {
            prev_status(node.status.as_deref())
        };
        let mut updated = node.clone();
        updated.status = if new_label == "None" {
            None
        } else {
            Some(new_label.to_string())
        };
        if let Err(e) = self
            .store
            .raw()
            .update_metadata(paragraph_id, updated.to_json())
        {
            self.status = format!("status update failed: {e}");
            return;
        }
        self.reload_hierarchy();
        // Re-collect entries under the same scope + status target.
        let (scope_id, scope_label) = self.resolve_status_filter_scope();
        let entries = self.collect_status_entries(target_status, scope_id);
        let total = entries.len();
        let new_cursor = if total == 0 {
            0
        } else if prior_cursor >= total {
            total - 1
        } else {
            prior_cursor
        };
        self.modal = Modal::StatusFilter {
            status_label: target_status,
            scope: scope_label.clone(),
            entries,
            cursor: new_cursor,
        };
        let direction = if forward { "→" } else { "←" };
        self.status = format!(
            "{} {direction} {new_label} · `{scope_label}` · {total} paragraph(s) remaining",
            node.title
        );
    }

    fn commit_status_filter(&mut self) {
        let target_id = match &self.modal {
            Modal::StatusFilter { entries, cursor, .. } => {
                entries.get(*cursor).map(|e| e.id)
            }
            _ => None,
        };
        let Some(id) = target_id else {
            self.modal = Modal::None;
            return;
        };
        self.modal = Modal::None;
        // Jump tree cursor to the chosen paragraph, then open it via
        // the standard load path so paragraph_cursors / save sessions
        // work the same as Enter from the tree.
        if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == id) {
            self.tree_cursor = i;
        }
        if let Some(node) = self.hierarchy.get(id).cloned() {
            if let Err(e) = self.load_paragraph(&node) {
                self.status = format!("open: {e}");
            }
        }
    }

    fn draw_status_filter_modal(&self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::StatusFilter { status_label, scope, entries, cursor } = &self.modal else {
            return;
        };
        let header_lines = 3usize; // title row inside chrome stays 0; footer grows
        let footer_lines = 3usize;
        let body_lines = entries.len().max(1);
        let height = ((header_lines + body_lines + footer_lines + 2) as u16)
            .clamp(10, area.height.saturating_sub(2));
        let max_title = entries.iter().map(|e| e.title.chars().count()).max().unwrap_or(20);
        let max_crumb = entries.iter().map(|e| e.breadcrumb.chars().count()).max().unwrap_or(30);
        let width = ((max_title + max_crumb + 12) as u16).clamp(60, area.width.saturating_sub(6));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let title = format!(" Paragraphs with status [{status_label}] · scope: {scope} · Ctrl+B {} ",
            match *status_label {
                "Ready" => "1",
                "Final" => "2",
                "Third" => "3",
                "Second" => "4",
                "First" => "5",
                "Napkin" => "6",
                "None" => "7",
                _ => "?",
            });
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(""));
        if entries.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("  No paragraphs tagged [{status_label}]."),
                Style::default().add_modifier(Modifier::DIM),
            )));
        } else {
            let body_h = inner.height.saturating_sub((header_lines + footer_lines) as u16) as usize;
            let body_h = body_h.max(1);
            let cursor = (*cursor).min(entries.len() - 1);
            let scroll = if cursor >= body_h { cursor - body_h + 1 } else { 0 };
            let end = (scroll + body_h).min(entries.len());
            for (i_offset, entry) in entries[scroll..end].iter().enumerate() {
                let i = scroll + i_offset;
                let marker = if i == cursor { "›" } else { " " };
                let title_padded =
                    format!("{t:<width$}", t = entry.title, width = max_title);
                let row =
                    format!("  {marker} {title_padded}   {b}", b = entry.breadcrumb);
                let style = if i == cursor {
                    Style::default()
                        .add_modifier(Modifier::REVERSED)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(row, style)));
            }
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  ↑↓ select · Enter opens · r/R advances status · - / Backspace reverses · Esc cancel"
                .to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )));
        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }

    /// Editor meta `Ctrl+B T`: rerun the placeholder-title derivation on
    /// the currently-open paragraph. Same logic that fires on save when
    /// the title is still `PARAGRAPH_PLACEHOLDER_TITLE`, but exposed
    /// explicitly so the user can refresh the tree-display name after
    /// rewriting the lead. Bails out cleanly if no first sentence can be
    /// extracted (paragraph is empty or only contains headings).
    fn rename_paragraph_to_first_sentence(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "no paragraph open".into();
            return;
        };
        let body = doc.textarea.lines().join("\n");
        let Some(new_title) = extract_first_sentence(&body) else {
            self.status =
                "couldn't derive a title — paragraph is empty or only headings".into();
            return;
        };
        let id = doc.id;
        match self.store.rename_node(&self.hierarchy, id, &new_title) {
            Ok(()) => {
                self.reload_hierarchy();
                if let Some(d) = self.opened.as_mut() {
                    d.title = new_title.clone();
                    if let Some(node) = self.hierarchy.get(id) {
                        if let Some(rel) = node.file.as_ref() {
                            d.rel_path = rel.clone();
                        }
                    }
                }
                self.status = format!("renamed paragraph to `{new_title}`");
            }
            Err(e) => {
                self.status = format!("rename failed: {e}");
            }
        }
    }

    /// Editor meta `Ctrl+B P` (Places) / `Ctrl+B C` (Characters). Treats
    /// the editor's selection (or the word under the cursor) as a lookup
    /// term, sweeps matching paragraphs in the named system book, builds
    /// a RAG context block, and either fires the inference immediately
    /// (if the AI prompt already has a query) or stashes the context as
    /// `pending_rag_prefix` and refocuses the AI prompt for the user to
    /// type a query (item 4 in the spec).
    fn start_lexicon_inference(&mut self, kind: LexiconKind) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = format!("{} RAG needs an open paragraph", kind.label());
            return;
        };
        let lookup = current_word_or_selection(doc);
        if lookup.trim().is_empty() {
            self.status = format!(
                "{} RAG: select a name or place the cursor on one first",
                kind.label()
            );
            return;
        }

        let Some(book_id) = self.system_book_id(kind.system_tag()) else {
            self.status = format!(
                "{} book is missing — re-open the project to seed it",
                kind.label()
            );
            return;
        };

        // Case-insensitive substring match against paragraph titles. A
        // selection of "Москва" finds both "Москва" and "Москва-Сити",
        // which is usually the user's intent.
        let needle = lookup.to_lowercase();
        let mut chunks: Vec<String> = Vec::new();
        for id in self.hierarchy.collect_subtree(book_id) {
            if id == book_id {
                continue;
            }
            let Some(node) = self.hierarchy.get(id) else {
                continue;
            };
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            if !node.title.to_lowercase().contains(&needle) {
                continue;
            }
            let body = match self.store.get_content(node.id) {
                Ok(Some(b)) => String::from_utf8_lossy(&b).to_string(),
                _ => continue,
            };
            chunks.push(format!(
                "── {}: {} ──\n{}\n── end {} ──",
                kind.label(),
                node.title,
                body,
                kind.label().to_lowercase()
            ));
        }
        if chunks.is_empty() {
            self.status = format!(
                "{} RAG: no entry titled like `{lookup}` in the {} book",
                kind.label(),
                kind.label()
            );
            return;
        }
        let prefix = format!(
            "── {} context for `{lookup}` ({} match(es)) ──\n\n{}",
            kind.label(),
            chunks.len(),
            chunks.join("\n\n")
        );

        // Item 4: if the AI prompt is empty, arm the prefix and let the
        // user type their question. Otherwise send immediately with the
        // current prompt as the question.
        let prompt_present = !self.ai_input.as_str().trim().is_empty();
        if prompt_present {
            self.pending_rag_prefix = Some(prefix);
            self.start_inference();
            // start_inference moves focus to AiPrompt; bounce to AI pane
            // so the user can watch the streamed answer per spec.
            self.change_focus(Focus::Ai);
        } else {
            self.pending_rag_prefix = Some(prefix);
            self.change_focus(Focus::AiPrompt);
            self.status = format!(
                "{} RAG armed for `{lookup}` — type your question and Enter",
                kind.label()
            );
        }
    }

    /// Run a grammar check on the currently-open paragraph. Resolves a
    /// "Grammar check" prompt template by precedence:
    ///   1. Paragraph titled / slugged `grammar-check` (or `Grammar check`)
    ///      under the Prompts system book.
    ///   2. Same-named entry in `prompts.hjson` (`name: "grammar-check"`).
    ///   3. Built-in fallback that constrains the LLM to checking syntax
    ///      and punctuation in `cfg.language` while preserving any Typst
    ///      formatting.
    ///
    /// In all three cases the paragraph body is appended verbatim. After
    /// streaming starts focus jumps to the AI pane so the user can watch
    /// the result render in real time.
    fn start_grammar_check(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "grammar check needs an open paragraph".into();
            return;
        };
        let body = doc.textarea.lines().join("\n");
        if body.trim().is_empty() {
            self.status = "grammar check: paragraph is empty".into();
            return;
        }

        // Resolver precedence. `grammar-check` is the canonical slug —
        // case-insensitive match against both slug and title catches the
        // common variants ("Grammar check", "GRAMMAR CHECK", etc.).
        const NAME: &str = "grammar-check";
        const TITLE: &str = "grammar check";
        let template = if let Some(t) = self.lookup_book_prompt_template(NAME) {
            t
        } else if let Some(t) = self.lookup_book_prompt_template(TITLE) {
            t
        } else if let Some(p) = self.prompts.find(NAME) {
            p.template.clone()
        } else if let Some(p) = self.prompts.find(TITLE) {
            p.template.clone()
        } else {
            grammar_check_default_prompt(&self.cfg.language)
        };

        // Render placeholders ({{selection}} / {{context}}) and then
        // append the paragraph body so the model has a single trailing
        // block to work on regardless of whether the template already
        // referenced it.
        let rendered = self.render_template(&template);
        let prompt_text = format!(
            "{rendered}\n\n── Paragraph: {title} ──\n{body}\n── end paragraph ──",
            title = doc.title
        );

        let (model, _env_var) = match self.ai.resolve_provider(&self.cfg.llm, None) {
            Ok(pair) => pair,
            Err(e) => {
                self.status = format!("grammar check: {e}");
                return;
            }
        };
        let model = model.to_string();
        let provider = self.ai.default_provider.clone();
        // Grammar check is a one-shot: don't replay chat history, don't
        // append the turn to history. Behaviour matches Help in that
        // sense.
        let rx = spawn_chat_stream(
            self.ai.client.clone(),
            model.clone(),
            Some(GRAMMAR_CHECK_SYSTEM_PROMPT.to_string()),
            Vec::new(),
            prompt_text,
        );
        self.inference = Some(Inference {
            provider: provider.clone(),
            model,
            response: String::new(),
            status: InferenceStatus::Streaming,
            rx,
            started_at: std::time::Instant::now(),
        });
        self.pending_chat_user_msg = None;
        // Per spec: focus moves to the AI pane so the user can watch the
        // streamed result. Esc bounces back to AiPrompt for follow-ups.
        self.change_focus(Focus::Ai);
        self.status = format!(
            "Grammar check: streaming from {provider} ({})…",
            self.cfg.language
        );
    }

    fn start_help_inference(&mut self, query: &str) {
        let query = query.trim();
        if query.is_empty() {
            self.status = "Help: empty question".into();
            return;
        }

        // Locate the Help book; required as the RAG source.
        let Some(help_id) = self.system_book_id(crate::store::SYSTEM_TAG_HELP) else {
            self.status = "Help book not present — re-open the project to seed it".into();
            return;
        };
        let help_subtree: std::collections::HashSet<Uuid> =
            self.hierarchy.collect_subtree(help_id).into_iter().collect();

        // Search broadly, then filter to nodes inside the Help subtree. We
        // ask for more than we'll actually feed to the LLM so the post-filter
        // doesn't starve us if many hits are outside Help.
        let raw_hits = match self.store.search_text(query, 40) {
            Ok(hits) => hits,
            Err(e) => {
                self.status = format!("Help: search failed: {e}");
                return;
            }
        };
        let mut chosen: Vec<SearchHit> = raw_hits
            .iter()
            .filter_map(SearchHit::parse)
            .filter(|h| help_subtree.contains(&h.id))
            .collect();
        // Keep only paragraphs — branches don't have prose to ground on.
        chosen.retain(|h| h.kind == NodeKind::Paragraph);
        // Cap context size to avoid blowing the model's window.
        const MAX_CONTEXT_PARAGRAPHS: usize = 8;
        const MAX_CHARS_PER_PARAGRAPH: usize = 2000;
        chosen.truncate(MAX_CONTEXT_PARAGRAPHS);

        // Fetch full content for the chosen paragraphs and assemble the
        // grounded context block.
        let mut context = String::new();
        let mut included = 0usize;
        for hit in &chosen {
            let body = match self.store.get_content(hit.id) {
                Ok(Some(bytes)) => String::from_utf8_lossy(&bytes).to_string(),
                _ => continue,
            };
            let trimmed = if body.chars().count() > MAX_CHARS_PER_PARAGRAPH {
                let mut t: String = body.chars().take(MAX_CHARS_PER_PARAGRAPH).collect();
                t.push('…');
                t
            } else {
                body
            };
            let breadcrumb = self.title_breadcrumb(hit.id);
            context.push_str(&format!(
                "── Help excerpt: {} (path: {}) ──\n{}\n\n",
                hit.title, breadcrumb, trimmed
            ));
            included += 1;
        }

        if included == 0 {
            self.status = format!(
                "Help: no entries found for `{}`. Try a different question.",
                query
            );
            return;
        }

        let system_prompt = HELP_SYSTEM_PROMPT.to_string();
        let user_prompt = format!(
            "Question: {query}\n\nContext (Inkhaven Help excerpts — your ONLY allowed source):\n\n{context}\nAnswer using only the context above. If it does not contain the answer, say so plainly and suggest which part of the Help book might be relevant."
        );

        let (model, _env_var) = match self.ai.resolve_provider(&self.cfg.llm, None) {
            Ok(pair) => pair,
            Err(e) => {
                self.status = format!("Help: {e}");
                return;
            }
        };
        let model = model.to_string();
        let provider = self.ai.default_provider.clone();

        // Help is a one-shot RAG inference — no chat history is replayed
        // (so the strict grounding system prompt isn't diluted), and the
        // turn does not accumulate into `chat_history`.
        let rx = spawn_chat_stream(
            self.ai.client.clone(),
            model.clone(),
            Some(system_prompt),
            Vec::new(),
            user_prompt,
        );
        self.inference = Some(Inference {
            provider: provider.clone(),
            model,
            response: String::new(),
            status: InferenceStatus::Streaming,
            rx,
            started_at: std::time::Instant::now(),
        });
        self.pending_chat_user_msg = None;
        // Land on the AI prompt pane so the user can immediately ask a
        // follow-up Help question; Esc flips to the AI pane to read.
        self.change_focus(Focus::AiPrompt);
        self.status = format!(
            "Help: streaming answer from {provider} (grounded on {included} excerpt(s))…"
        );
    }

    fn quickref_handle_key(&mut self, key: KeyEvent) -> bool {
        let Modal::QuickRef { focus, scroll } = &mut self.modal else {
            return false;
        };
        let total = quickref::entries_for(*focus).len();
        match key.code {
            KeyCode::Up => {
                *scroll = scroll.saturating_sub(1);
                true
            }
            KeyCode::Down => {
                if *scroll + 1 < total {
                    *scroll += 1;
                }
                true
            }
            KeyCode::PageUp => {
                *scroll = scroll.saturating_sub(10);
                true
            }
            KeyCode::PageDown => {
                *scroll = (*scroll + 10).min(total.saturating_sub(1));
                true
            }
            KeyCode::Home => {
                *scroll = 0;
                true
            }
            KeyCode::End => {
                *scroll = total.saturating_sub(1);
                true
            }
            _ => false,
        }
    }

    /// Scroll handler for the Credits modal — mirrors `quickref_handle_key`.
    /// `total` is the number of rendered lines (computed in the renderer),
    /// but we don't need a hard upper bound here; clamping happens at
    /// render time so out-of-range scroll just shows a blank tail.
    fn credits_handle_key(&mut self, key: KeyEvent) -> bool {
        let Modal::Credits { scroll, .. } = &mut self.modal else {
            return false;
        };
        match key.code {
            KeyCode::Up => {
                *scroll = scroll.saturating_sub(1);
                true
            }
            KeyCode::Down => {
                *scroll = scroll.saturating_add(1);
                true
            }
            KeyCode::PageUp => {
                *scroll = scroll.saturating_sub(10);
                true
            }
            KeyCode::PageDown => {
                *scroll = scroll.saturating_add(10);
                true
            }
            KeyCode::Home => {
                *scroll = 0;
                true
            }
            KeyCode::End => {
                *scroll = usize::MAX / 2;
                true
            }
            _ => false,
        }
    }

    fn open_file_picker(&mut self, context: PickerContext) {
        let root = std::env::current_dir().unwrap_or_else(|_| self.layout.root.clone());
        self.modal = Modal::FilePicker(FilePicker::new(root, context));
    }

    fn commit_file_pick(&mut self) {
        let (path, is_dir, context) = match &self.modal {
            Modal::FilePicker(p) => match p.current() {
                Some(entry) => (entry.path.clone(), entry.is_dir, p.context),
                None => {
                    self.modal = Modal::None;
                    return;
                }
            },
            _ => return,
        };

        match (context, is_dir) {
            (PickerContext::EditorLoad, true) => {
                self.status =
                    "Editor F3 needs a file, not a directory — Enter on a file".into();
            }
            (PickerContext::EditorLoad, false) => {
                self.load_file_into_editor(&path);
                self.modal = Modal::None;
            }
            (PickerContext::TreeInsertOrImport, false) => {
                self.import_single_file(&path);
                self.modal = Modal::None;
            }
            (PickerContext::TreeInsertOrImport, true) => {
                // Defer the actual import to the main loop so it can run
                // with a progress splash drawn directly via the terminal
                // handle. `commit_file_pick` doesn't own the terminal —
                // see `App::run`.
                self.modal = Modal::None;
                self.pending_import = Some(path);
            }
        }
    }

    fn load_file_into_editor(&mut self, path: &std::path::Path) {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.status = format!("read {}: {e}", path.display());
                return;
            }
        };
        let body = String::from_utf8_lossy(&bytes).into_owned();
        let Some(doc) = self.opened.as_mut() else {
            self.status =
                "no paragraph open — open one first, then F3 to replace its body".into();
            return;
        };
        let mut new_textarea = TextArea::new(body_to_lines(&body));
        new_textarea.set_cursor_line_style(Style::default().add_modifier(Modifier::REVERSED));
        new_textarea.set_line_number_style(Style::default().fg(Color::DarkGray));
        doc.textarea = new_textarea;
        doc.dirty = true;
        doc.scroll_row = 0;
        doc.scroll_col = 0;
        doc.last_activity = std::time::Instant::now();
        self.change_focus(Focus::Editor);
        self.status = format!("loaded `{}` — bold marks the change vs saved", path.display());
    }

    fn import_single_file(&mut self, path: &std::path::Path) {
        // Route image files (PNG / JPG / etc.) to the image-import
        // path; everything else gets the prose treatment.
        if let Some(ext) = image_extension_for(path) {
            self.import_single_image(path, &ext);
            return;
        }
        // Place the new paragraph after the tree cursor's same-kind ancestor
        // if there is one (so it's "insert after current"). Falls back to
        // append-at-end under the nearest valid parent.
        let cursor_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
        let anchor = cursor_id.and_then(|id| {
            let mut cur = Some(id);
            while let Some(c) = cur {
                let node = self.hierarchy.get(c)?;
                if node.kind == NodeKind::Paragraph {
                    return Some(node.id);
                }
                cur = node.parent_id;
            }
            None
        });
        let position = match anchor {
            Some(id) => InsertPosition::After(id),
            None => InsertPosition::End,
        };
        let parent = match position {
            InsertPosition::After(anchor_id) | InsertPosition::Before(anchor_id) => self
                .hierarchy
                .get(anchor_id)
                .and_then(|n| n.parent_id)
                .and_then(|pid| self.hierarchy.get(pid))
                .cloned(),
            InsertPosition::End => {
                match self
                    .hierarchy
                    .pick_parent_for(&self.cfg, cursor_id, NodeKind::Paragraph)
                {
                    Ok(p) => p.cloned(),
                    Err(e) => {
                        self.status = format!("can't import here: {e}");
                        return;
                    }
                }
            }
        };

        let title = derive_paragraph_title_from_path(path);
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.status = format!("read {}: {e}", path.display());
                return;
            }
        };

        let content_type = content_type_for(path);
        let mut created = match self.store.create_node(
            &self.cfg,
            &self.hierarchy,
            NodeKind::Paragraph,
            &title,
            parent.as_ref(),
            None,
            position,
        ) {
            Ok(n) => n,
            Err(e) => {
                self.status = format!("create paragraph: {e}");
                return;
            }
        };
        // For non-default content types, stamp the node + rename the
        // on-disk file so the extension matches (`<NN>-<slug>.hjson`
        // instead of `.typ`). create_node always lays down the file
        // with the typst extension; we move it before writing bytes.
        if let Some(ct) = &content_type {
            created.content_type = Some(ct.clone());
            let new_rel = std::path::PathBuf::from(
                created.file.clone().unwrap_or_default(),
            )
            .with_extension(ct);
            if let Some(old_rel) = &created.file {
                let old_abs = self.layout.root.join(old_rel);
                let new_abs = self.layout.root.join(&new_rel);
                if old_abs.exists() && old_abs != new_abs {
                    let _ = std::fs::rename(&old_abs, &new_abs);
                }
            }
            created.file = Some(new_rel.to_string_lossy().into_owned());
            if let Err(e) = self
                .store
                .raw()
                .update_metadata(created.id, created.to_json())
            {
                self.status = format!("update metadata: {e}");
                return;
            }
        }

        // Replace the templated body with the actual file content.
        let Some(rel) = created.file.clone() else {
            self.status = "created paragraph has no file path — bug?".into();
            return;
        };
        let abs = self.layout.root.join(&rel);
        if let Err(e) = std::fs::write(&abs, &bytes) {
            self.status = format!("write {}: {e}", abs.display());
            return;
        }
        let mut node = created.clone();
        if let Err(e) = self.store.update_paragraph_content(&mut node, &bytes) {
            self.status = format!("update: {e}");
            return;
        }
        let _ = self.store.sync();
        let kind_note = match content_type.as_deref() {
            Some("hjson") => " (hjson)",
            _ => "",
        };
        self.status = format!("imported `{}` as paragraph{kind_note}", path.display());
        self.reload_hierarchy();
        if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == created.id) {
            self.tree_cursor = i;
        }
    }

    /// Sibling to `import_single_file` for image files. The parent
    /// selection mirrors the prose path: insert after the cursor's
    /// nearest leaf sibling, falling back to "append at the end of
    /// the nearest legal branch".
    fn import_single_image(&mut self, path: &std::path::Path, ext: &str) {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.status = format!("read {}: {e}", path.display());
                return;
            }
        };
        let title = derive_paragraph_title_from_path(path);
        let cursor_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
        let anchor = cursor_id.and_then(|id| {
            let mut cur = Some(id);
            while let Some(c) = cur {
                let node = self.hierarchy.get(c)?;
                if node.kind.is_leaf() {
                    return Some(node.id);
                }
                cur = node.parent_id;
            }
            None
        });
        let position = match anchor {
            Some(id) => InsertPosition::After(id),
            None => InsertPosition::End,
        };
        let parent = match position {
            InsertPosition::After(anchor_id) | InsertPosition::Before(anchor_id) => self
                .hierarchy
                .get(anchor_id)
                .and_then(|n| n.parent_id)
                .and_then(|pid| self.hierarchy.get(pid))
                .cloned(),
            InsertPosition::End => {
                match self
                    .hierarchy
                    .pick_parent_for(&self.cfg, cursor_id, NodeKind::Image)
                {
                    Ok(p) => p.cloned(),
                    Err(e) => {
                        self.status = format!("can't import image here: {e}");
                        return;
                    }
                }
            }
        };
        let created = match self.store.create_image_node(
            &self.cfg,
            &self.hierarchy,
            &title,
            ext,
            &bytes,
            parent.as_ref(),
            position,
        ) {
            Ok(n) => n,
            Err(e) => {
                self.status = format!("import image: {e}");
                return;
            }
        };
        self.status = format!(
            "imported `{}` as image ({} bytes)",
            path.display(),
            bytes.len()
        );
        self.reload_hierarchy();
        if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == created.id) {
            self.tree_cursor = i;
        }
    }

    /// Drive a deferred directory import set up by `commit_file_pick`.
    /// Pre-counts files so the splash has a meaningful denominator, then
    /// runs the (synchronous) import with a progress callback that
    /// throttles `terminal.draw` to ~30 Hz. Status bar is updated when
    /// the import finishes; the next mainloop frame paints over the
    /// splash automatically.
    fn run_pending_import<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
        root: &Path,
    ) {
        let total = count_importable_files(root);
        let source_display = root.display().to_string();
        let progress_root = root.to_path_buf();

        // Initial 0/total frame so the splash appears even if the
        // first file takes a moment (e.g. fastembed cold-loading on
        // first import in a session).
        let _ = terminal.draw(|f| {
            draw_import_splash(f, &source_display, 0, total, "scanning…")
        });

        let mut last_redraw = std::time::Instant::now();
        {
            let source_display = source_display.clone();
            let mut progress = move |done: usize, file: &Path| {
                if last_redraw.elapsed() < std::time::Duration::from_millis(33) {
                    return;
                }
                last_redraw = std::time::Instant::now();
                let rel = file
                    .strip_prefix(&progress_root)
                    .unwrap_or(file);
                let label = rel.display().to_string();
                let _ = terminal.draw(|f| {
                    draw_import_splash(f, &source_display, done, total, &label)
                });
            };
            self.import_directory_tree(root, &mut progress);
        }
    }

    /// Ctrl+B A — resolve "current book" the same way Ctrl+B I does
    /// (open paragraph's book, or the tree-cursor's book), validate
    /// it's a user book, then stash the uuid so the main loop drives
    /// the assembly with the splash.
    fn schedule_assembly(&mut self) {
        // 1.2.5+: assembly walks the on-disk .typ files. If the
        // user has unsaved edits in the editor (primary or
        // secondary), assemble would see stale bytes. Save first
        // so what the user sees in the editor is what hits the
        // assembler.
        self.save_all_before_build_step("Book assembly");
        let hierarchy = match Hierarchy::load(&self.store) {
            Ok(h) => h,
            Err(e) => {
                self.status = format!("Book assembly: hierarchy load failed: {e}");
                return;
            }
        };
        let Some(book) = self.current_book_node(&hierarchy) else {
            self.status =
                "Book assembly: move the tree cursor onto a user book (or any node inside one) first."
                    .into();
            return;
        };
        if book.system_tag.is_some() {
            self.status = format!(
                "Book assembly: `{}` is a system book — pick a user book.",
                book.title
            );
            return;
        }
        self.pending_assembly = Some(book.id);
        self.status = format!("Book assembly: assembling `{}`…", book.title);
    }

    /// Drive a deferred Book assembly. Pre-renders the splash at 0%,
    /// runs the synchronous assembler with a 30 Hz-throttled progress
    /// callback, and surfaces the final result (root .typ path or
    /// error) in the status bar.
    fn run_pending_assembly<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
        book_id: Uuid,
    ) {
        let book = match Hierarchy::load(&self.store) {
            Ok(h) => match h.get(book_id).cloned() {
                Some(n) => n,
                None => {
                    self.status = "Book assembly: book vanished from hierarchy".into();
                    return;
                }
            },
            Err(e) => {
                self.status = format!("Book assembly: hierarchy load failed: {e}");
                return;
            }
        };

        let book_display = book.title.clone();
        let initial_total = 0;
        let _ = terminal.draw(|f| {
            draw_assembly_splash(f, &book_display, 0, initial_total, "preparing…")
        });

        let mut last_redraw = std::time::Instant::now();
        let book_display_for_cb = book_display.clone();
        let report = {
            let mut progress = move |done: usize, total: usize, file: &Path| {
                if last_redraw.elapsed() < std::time::Duration::from_millis(33) {
                    return;
                }
                last_redraw = std::time::Instant::now();
                let label = file.display().to_string();
                let _ = terminal.draw(|f| {
                    draw_assembly_splash(f, &book_display_for_cb, done, total, &label)
                });
            };
            crate::assemble::assemble_book(
                &self.store,
                &self.layout,
                &self.cfg,
                &book,
                &mut progress,
            )
        };

        match report {
            Ok(r) => {
                self.status = format!(
                    "Book assembly: wrote {} files · root: {}  (typst compile `{}`)",
                    r.files_written,
                    r.root_typ.display(),
                    r.root_typ.display(),
                );
                // hook.on_assemble ( uuid slug root_typ_path files_written -- )
                // Fired after a successful Ctrl+B A. Lets scripts
                // post-process the assembled tree (e.g. patch the
                // root .typ, copy artefacts, kick off a custom
                // build pipeline).
                crate::scripting::hooks::fire(
                    "hook.on_assemble",
                    vec![
                        rust_dynamic::value::Value::from_string(book.id.to_string()),
                        rust_dynamic::value::Value::from_string(book.slug.clone()),
                        rust_dynamic::value::Value::from_string(
                            r.root_typ.to_string_lossy().into_owned(),
                        ),
                        rust_dynamic::value::Value::from_int(r.files_written as i64),
                    ],
                );
            }
            Err(e) => {
                self.status = format!("Book assembly failed: {e}");
            }
        }
    }

    /// Ctrl+B B — schedule a Book "build": assembly + `typst compile`.
    /// On error the build path opens a fresh AI chat for analysis.
    fn schedule_build(&mut self) {
        // 1.2.5+: flush unsaved edits before assemble fires (see
        // schedule_assembly for the rationale).
        self.save_all_before_build_step("Book build");
        let Some(book_id) = self.resolve_current_user_book("Book build") else {
            return;
        };
        self.pending_build = Some(book_id);
        self.status = "Book build: assembling + compiling…".into();
    }

    /// Ctrl+B O — schedule a Book "take": build, then copy the PDF
    /// into the launch cwd with a timestamped filename.
    fn schedule_take(&mut self) {
        // 1.2.5+: flush unsaved edits before assemble fires (see
        // schedule_assembly for the rationale).
        self.save_all_before_build_step("Take the book");
        let Some(book_id) = self.resolve_current_user_book("Take the book") else {
            return;
        };
        self.pending_take = Some(book_id);
        self.status = "Take the book: assembling + compiling + copying…".into();
    }

    /// 1.2.5+: shared autosave step for Ctrl+B A / B / O. Saves
    /// the primary editor and, when similar-paragraph mode has
    /// a secondary editor open, that one too. Errors are logged
    /// at WARN and stamped on the status bar but never abort
    /// the build — the user can react to a save failure by
    /// dismissing the splash on Esc, which still happens. The
    /// helper is a no-op when neither buffer is dirty.
    fn save_all_before_build_step(&mut self, ctx: &str) {
        if let Some(doc) = self.opened.as_ref() {
            if doc.dirty {
                if let Err(e) = self.save_current() {
                    tracing::warn!(
                        target: "inkhaven::build",
                        "{ctx}: primary autosave failed: {e}",
                    );
                    self.status = format!("{ctx}: autosave failed: {e}");
                }
            }
        }
        // Mirror the autosave loop's `Option::take()` dance so we
        // can call `save_doc(&mut self, &mut OpenedDoc)` without
        // an aliasing borrow.
        if let Some(mut doc) = self.secondary.take() {
            if doc.dirty {
                if let Err(e) = self.save_doc(&mut doc) {
                    tracing::warn!(
                        target: "inkhaven::build",
                        "{ctx}: secondary autosave failed: {e}",
                    );
                }
            }
            self.secondary = Some(doc);
        }
    }

    /// Common preflight for Ctrl+B A / B / O. Returns the uuid of the
    /// user book the cursor is inside, or surfaces an error status and
    /// returns None when the cursor isn't on a user book.
    fn resolve_current_user_book(&mut self, ctx: &str) -> Option<Uuid> {
        let hierarchy = match Hierarchy::load(&self.store) {
            Ok(h) => h,
            Err(e) => {
                self.status = format!("{ctx}: hierarchy load failed: {e}");
                return None;
            }
        };
        let book = self.current_book_node(&hierarchy)?;
        if book.system_tag.is_some() {
            self.status =
                format!("{ctx}: `{}` is a system book — pick a user book.", book.title);
            return None;
        }
        Some(book.id)
    }

    /// Run assembly + typst compile for `book_id`. If `take` is true,
    /// the resulting PDF is also copied into the launch cwd with a
    /// timestamped filename. A typst-error opens a fresh AI chat with
    /// the configured error-system-prompt; the user gets streamed
    /// analysis on the AI pane without any extra keystroke.
    fn run_pending_build<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
        book_id: Uuid,
        take: bool,
    ) {
        // Step 1: assembly (re-uses the existing splash + procedure).
        self.run_pending_assembly(terminal, book_id);
        // run_pending_assembly returns silently on failure — figure
        // out whether it succeeded by re-checking the artefacts path.
        let book = match Hierarchy::load(&self.store) {
            Ok(h) => match h.get(book_id).cloned() {
                Some(n) => n,
                None => {
                    self.status =
                        "Book build aborted: book vanished mid-assembly".into();
                    return;
                }
            },
            Err(e) => {
                self.status = format!("Book build aborted: hierarchy reload: {e}");
                return;
            }
        };
        let artefacts_root = self.store.resolve_artefacts_dir(&self.cfg);
        let root_typ = artefacts_root
            .join(&book.slug)
            .join(format!("{}.typ", book.slug));
        if !root_typ.is_file() {
            // Assembly didn't produce a root .typ — its status message
            // already explains why, leave it in place.
            return;
        }

        // Step 2: spawn `typst compile` and animate the splash while
        // the child runs.
        let book_display = book.title.clone();
        let outcome = match self.run_typst_compile(terminal, &book_display, &root_typ) {
            Some(o) => o,
            None => return, // spawn failed; status already set
        };

        if outcome.success {
            let pdf_msg = format!(
                "Build OK · PDF: {}",
                outcome.pdf_path.display()
            );
            if take {
                match self.take_book_pdf(&book, &outcome.pdf_path) {
                    Ok(dest) => {
                        // Multi-format extras live next to the PDF
                        // with matching stem. Errors are reported
                        // (status bar) but never abort the take —
                        // the PDF the user actually asked for is
                        // already on disk.
                        let extras = self.take_extra_formats(&book, &dest);
                        let extras_msg = if extras.is_empty() {
                            String::new()
                        } else {
                            format!(" · extras: {}", extras.join(", "))
                        };
                        self.status = format!(
                            "Took the book · {}{}  (source PDF: {})",
                            dest.display(),
                            extras_msg,
                            outcome.pdf_path.display()
                        );
                        // hook.on_take ( uuid slug pdf_dest -- )
                        // Fired once the PDF is copied to the
                        // launch cwd (and any configured extra
                        // formats are written alongside). Lets
                        // scripts upload the artefact, post a
                        // chat notification, etc.
                        crate::scripting::hooks::fire(
                            "hook.on_take",
                            vec![
                                rust_dynamic::value::Value::from_string(
                                    book.id.to_string(),
                                ),
                                rust_dynamic::value::Value::from_string(
                                    book.slug.clone(),
                                ),
                                rust_dynamic::value::Value::from_string(
                                    dest.to_string_lossy().into_owned(),
                                ),
                            ],
                        );
                    }
                    Err(e) => {
                        self.status = format!("{pdf_msg} · take failed: {e}");
                    }
                }
            } else {
                self.status = pdf_msg;
            }
            return;
        }

        // Compile failed — surface the stderr through the AI pane and
        // leave a status hint mentioning where to read the answer.
        let error_text = if outcome.stderr.trim().is_empty() {
            outcome.stdout.clone()
        } else {
            outcome.stderr.clone()
        };
        self.start_typst_error_analysis(&book, &root_typ, &error_text);
    }

    /// Drive a typst-compile child to completion with the spinner
    /// splash. Returns the outcome on success-or-failure of the
    /// compile itself; returns None when even spawning the binary
    /// failed (status bar is set in that case).
    fn run_typst_compile<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
        book_display: &str,
        root_typ: &Path,
    ) -> Option<crate::typst_compile::CompileOutcome> {
        let mut handle = match crate::typst_compile::spawn_with_config(&self.cfg, root_typ) {
            Ok(h) => h,
            Err(e) => {
                self.status = format!("typst compile: {e}");
                return None;
            }
        };
        // Animate the splash while the compile runs (external child
        // or in-process worker thread — same loop, same UX).
        // ~80ms per frame keeps the spinner readable without
        // burning CPU. The same loop also polls for Esc so a stuck
        // compile can be interrupted by the user.
        let engine_label = crate::typst_compile::engine_summary(&self.cfg);
        let started = std::time::Instant::now();
        let mut spin_idx: usize = 0;
        let mut cancelled = false;
        loop {
            let elapsed = started.elapsed().as_secs();
            let spinner = TYPST_COMPILE_SPINNER[spin_idx % TYPST_COMPILE_SPINNER.len()];
            let _ = terminal.draw(|f| {
                draw_typst_compile_splash(f, book_display, &engine_label, elapsed, spinner)
            });
            spin_idx = spin_idx.wrapping_add(1);
            // Poll for input WITHOUT consuming non-Esc keys — we
            // re-emit nothing here; any user typing during the
            // compile is just dropped (the alternate-screen
            // splash is modal). Esc → cancel.
            if let Ok(true) = crossterm::event::poll(std::time::Duration::from_millis(0)) {
                if let Ok(crossterm::event::Event::Key(k)) = crossterm::event::read() {
                    if matches!(k.code, crossterm::event::KeyCode::Esc) {
                        handle.kill();
                        cancelled = true;
                        break;
                    }
                }
            }
            match handle.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {
                    std::thread::sleep(std::time::Duration::from_millis(80));
                }
                Err(e) => {
                    self.status = format!("typst compile: try_wait: {e}");
                    return None;
                }
            }
        }
        match crate::typst_compile::finish(handle) {
            Ok(o) => {
                if cancelled {
                    // Promote the outcome's failure state into a
                    // user-visible "cancelled" message rather than
                    // running the AI error-analysis path.
                    self.status = format!(
                        "typst compile cancelled — partial output (if any) discarded · engine: {engine_label}",
                    );
                    return None;
                }
                Some(o)
            }
            Err(e) => {
                self.status = format!("typst compile: {e}");
                None
            }
        }
    }

    /// Write each format in `cfg.output.extra_formats` alongside
    /// `pdf_dest`, sharing the same stem. The combined `.typ`
    /// source is built once from `book`'s subtree and reused for
    /// every format. Per-format errors are logged and surfaced in
    /// the returned brief list (`["markdown", "tex error: …"]`),
    /// never aborting — the PDF the user asked for is already on
    /// disk before this fires.
    fn take_extra_formats(
        &self,
        book: &crate::store::node::Node,
        pdf_dest: &Path,
    ) -> Vec<String> {
        let formats = &self.cfg.output.extra_formats;
        if formats.is_empty() {
            return Vec::new();
        }
        let hierarchy = match crate::store::hierarchy::Hierarchy::load(&self.store) {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!(
                    target: "inkhaven::take",
                    "extra formats: hierarchy load: {e}",
                );
                return vec![format!("hierarchy load failed: {e}")];
            }
        };
        let layout = crate::project::ProjectLayout::new(self.store.project_root());
        let combined = match crate::export::assemble_typst_source(
            &layout,
            &hierarchy,
            Some(book.id),
        ) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    target: "inkhaven::take",
                    "extra formats: assemble: {e}",
                );
                return vec![format!("assemble failed: {e}")];
            }
        };
        let mut produced: Vec<String> = Vec::new();
        for raw in formats {
            let fmt = raw.trim().to_ascii_lowercase();
            let outcome = self.build_one_extra_format(&fmt, &combined, &book.title);
            let artefact = match outcome {
                Some(Ok(art)) => art,
                Some(Err(e)) => {
                    tracing::warn!(
                        target: "inkhaven::take",
                        "extra format {fmt}: {e}",
                    );
                    produced.push(format!("{fmt} error"));
                    continue;
                }
                None => {
                    tracing::warn!(
                        target: "inkhaven::take",
                        "extra format {fmt}: unknown — skipped",
                    );
                    produced.push(format!("{fmt}?"));
                    continue;
                }
            };
            let dest = crate::export::with_artefact_extension(pdf_dest, &artefact);
            if let Err(e) = artefact.write_to(&dest) {
                tracing::warn!(
                    target: "inkhaven::take",
                    "extra format {fmt} write {}: {e}",
                    dest.display(),
                );
                produced.push(format!("{fmt} write error"));
                continue;
            }
            produced.push(dest.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(&fmt)
                .to_string());
        }
        produced
    }

    fn build_one_extra_format(
        &self,
        fmt: &str,
        combined: &str,
        book_title: &str,
    ) -> Option<anyhow::Result<crate::export::Artefact>> {
        match fmt {
            "markdown" | "md" => Some(Ok(crate::export::build_markdown(combined))),
            "tex" | "latex" => Some(Ok(crate::export::build_tex(combined))),
            "epub" => {
                let md = crate::export::markdown::typst_to_markdown(combined);
                Some(crate::export::build_epub(&md, book_title))
            }
            _ => None,
        }
    }

    /// Copy `pdf_src` into the launch cwd as
    /// `<book-slug>-YYYYDDMM-HHMM.pdf`. Returns the destination path
    /// on success. The cwd is inkhaven's `current_dir()` at the
    /// moment of the call — same path the shell-launched binary saw.
    fn take_book_pdf(
        &self,
        book: &Node,
        pdf_src: &Path,
    ) -> std::io::Result<std::path::PathBuf> {
        let cwd = std::env::current_dir()?;
        let now = chrono::Local::now();
        // Match the existing backup filename style: YYYYDDMM_HHMMSS.
        // User asked for YYYYDDMM-HHMM specifically — slight variant.
        let stamp = now.format("%Y%d%m-%H%M");
        let dest = cwd.join(format!("{}-{stamp}.pdf", book.slug));
        std::fs::copy(pdf_src, &dest)?;
        Ok(dest)
    }

    /// Open a fresh AI chat (cleared history, system prompt tuned for
    /// typst errors, inference mode forced to Full) and auto-send the
    /// compile error so the user gets streamed analysis without an
    /// extra keystroke.
    fn start_typst_error_analysis(
        &mut self,
        book: &Node,
        root_typ: &Path,
        error_text: &str,
    ) {
        // Wipe any in-flight chat so the new system prompt isn't
        // diluted by unrelated turns.
        self.chat_history.clear();
        self.inference = None;
        self.pending_chat_user_msg = None;
        // Force Full mode per the user's spec; auto-reset scope.
        self.inference_mode = InferenceMode::Full;
        self.ai_mode = AiMode::None;

        let system_prompt = self.cfg.typst_compile.resolved_error_system_prompt();
        let user_prompt = format!(
            "Book: `{book_title}` (slug `{slug}`)\n\
             Root file: {root}\n\n\
             `typst compile` failed with the following error. Please diagnose \
             it using the inkhaven file-layout knowledge from the system \
             prompt and tell me the smallest concrete fix.\n\n\
             --- typst stderr ---\n{err}",
            book_title = book.title,
            slug = book.slug,
            root = root_typ.display(),
            err = error_text.trim(),
        );

        let (model, _env_var) = match self.ai.resolve_provider(&self.cfg.llm, None) {
            Ok(pair) => pair,
            Err(e) => {
                self.status = format!("typst error: can't reach LLM ({e})");
                return;
            }
        };
        let model = model.to_string();
        let provider = self.ai.default_provider.clone();
        let rx = spawn_chat_stream(
            self.ai.client.clone(),
            model.clone(),
            Some(system_prompt),
            Vec::new(),
            user_prompt.clone(),
        );
        self.inference = Some(Inference {
            provider: provider.clone(),
            model,
            response: String::new(),
            status: InferenceStatus::Streaming,
            rx,
            started_at: std::time::Instant::now(),
        });
        // Record the user turn so the assistant's reply ends up in
        // chat_history when streaming finishes.
        self.pending_chat_user_msg = Some(user_prompt);
        self.change_focus(Focus::Ai);
        self.status = format!(
            "typst compile failed · {provider} is analysing the error in the AI pane…"
        );
    }

    fn import_directory_tree(
        &mut self,
        root: &std::path::Path,
        progress: &mut dyn FnMut(usize, &Path),
    ) {
        // The top-level dir's "kind" adapts to where the tree cursor sits:
        //   Book      → top dir becomes a Chapter
        //   Chapter   → top dir becomes a Subchapter
        //   Subchapter→ top dir becomes a Subchapter (requires unbounded)
        //   Paragraph → reject
        // Same rule applies at every recursion level: the new branch's kind
        // is "one level deeper than its parent", so a nested directory tree
        // walks down the hierarchy with it.
        let cursor_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
        let parent_id = cursor_id.and_then(|id| {
            // If cursor is on a paragraph we walk up to its enclosing branch
            // — easier than telling the user "move first".
            let mut cur = Some(id);
            while let Some(c) = cur {
                let node = self.hierarchy.get(c)?;
                if node.kind != NodeKind::Paragraph {
                    return Some(node.id);
                }
                cur = node.parent_id;
            }
            None
        });

        let mut counts = ImportCounts::default();
        let result = self.import_dir_recursive(root, parent_id, &mut counts, progress);

        // Always reload — even on partial failure the new branches/paragraphs
        // that DID get created should be visible in the tree.
        self.reload_hierarchy();

        match result {
            Ok(()) => {
                self.status = format!(
                    "imported {}: {} branch(es), {} paragraph(s)",
                    root.display(),
                    counts.branches,
                    counts.paragraphs
                );
            }
            Err(e) => {
                self.status = format!(
                    "partial import of {}: {} branch(es), {} paragraph(s) — stopped at: {e}",
                    root.display(),
                    counts.branches,
                    counts.paragraphs
                );
            }
        }
    }

    fn import_dir_recursive(
        &mut self,
        source: &std::path::Path,
        parent_id: Option<Uuid>,
        counts: &mut ImportCounts,
        progress: &mut dyn FnMut(usize, &Path),
    ) -> InkResult<()> {
        // Resolve parent against a freshly loaded hierarchy so prior creates
        // in this import are visible.
        let hierarchy = Hierarchy::load(&self.store)?;
        let parent = parent_id.and_then(|id| hierarchy.get(id).cloned());

        // Decide what kind of branch this directory becomes. None means we've
        // bottomed out under a bounded hierarchy and should flatten files
        // into the current parent instead of failing.
        let kind: Option<NodeKind> = match parent.as_ref().map(|p| p.kind) {
            None => Some(NodeKind::Book),
            Some(NodeKind::Book) => Some(NodeKind::Chapter),
            Some(NodeKind::Chapter) => Some(NodeKind::Subchapter),
            Some(NodeKind::Subchapter) => {
                if self.cfg.hierarchy.unbounded_subchapters {
                    Some(NodeKind::Subchapter)
                } else {
                    None
                }
            }
            Some(NodeKind::Paragraph) | Some(NodeKind::Image) | Some(NodeKind::Script) => {
                return Err(Error::Store(
                    "can't import under a leaf — move cursor to a branch first".into(),
                ));
            }
        };

        let Some(kind) = kind else {
            // Max depth reached. Walk the rest of the subtree and import every
            // file as a paragraph in the current parent. Branches beyond this
            // point are lost (the bounded hierarchy can't represent them),
            // but the prose comes through.
            let pid = parent_id.expect("None parent already handled by kind match");
            return self.flatten_files_into(source, pid, counts, progress);
        };

        let title = source
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("imported")
            .to_string();
        let created = self.store.create_node(
            &self.cfg,
            &hierarchy,
            kind,
            &title,
            parent.as_ref(),
            None,
            InsertPosition::End,
        )?;
        counts.branches += 1;
        let created_id = created.id;

        let children = read_sorted_children(source);

        // Don't bail on the first failing child — record the error but
        // continue so siblings still get imported. The user gets a partial-
        // import status with counts; orphan dirs get reported in the message.
        let mut first_err: Option<Error> = None;
        for child_path in children {
            let res = if child_path.is_dir() {
                self.import_dir_recursive(&child_path, Some(created_id), counts, progress)
            } else {
                self.import_file_as_paragraph_by_id(&child_path, created_id, counts, progress)
            };
            if let Err(e) = res {
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
        match first_err {
            None => Ok(()),
            Some(e) => Err(e),
        }
    }

    /// Walk `source` recursively and import every regular file as a paragraph
    /// under `parent_id`. Used when we've hit the depth limit and can no
    /// longer create deeper branches.
    fn flatten_files_into(
        &mut self,
        source: &std::path::Path,
        parent_id: Uuid,
        counts: &mut ImportCounts,
        progress: &mut dyn FnMut(usize, &Path),
    ) -> InkResult<()> {
        let mut first_err: Option<Error> = None;
        for entry in walkdir::WalkDir::new(source)
            .sort_by_file_name()
            .follow_links(false)
        {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    if first_err.is_none() {
                        first_err = Some(Error::Store(format!("walkdir: {e}")));
                    }
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let name = entry.file_name().to_str().unwrap_or("");
            if name.starts_with('.') {
                continue;
            }
            if let Err(e) = self.import_file_as_paragraph_by_id(
                entry.path(),
                parent_id,
                counts,
                progress,
            ) {
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
        match first_err {
            None => Ok(()),
            Some(e) => Err(e),
        }
    }

    fn import_file_as_paragraph_by_id(
        &mut self,
        file: &std::path::Path,
        parent_id: Uuid,
        counts: &mut ImportCounts,
        progress: &mut dyn FnMut(usize, &Path),
    ) -> InkResult<()> {
        let title = derive_paragraph_title_from_path(file);
        let raw = std::fs::read(file).map_err(Error::Io)?;
        // Normalise line endings so DOS / old-Mac dumps don't keep
        // their `\r` bytes on disk — those survive into the editor
        // load path and render as control glyphs. Only touched when
        // the content decodes as UTF-8; binary payloads are written
        // verbatim and will simply fail to render meaningfully.
        let bytes: Vec<u8> = match std::str::from_utf8(&raw) {
            Ok(text) if text.contains('\r') => text
                .replace("\r\n", "\n")
                .replace('\r', "\n")
                .into_bytes(),
            _ => raw,
        };
        let hierarchy = Hierarchy::load(&self.store)?;
        let parent = hierarchy
            .get(parent_id)
            .cloned()
            .ok_or_else(|| Error::Store(format!("import: parent {parent_id} vanished")))?;
        let created = self.store.create_node(
            &self.cfg,
            &hierarchy,
            NodeKind::Paragraph,
            &title,
            Some(&parent),
            None,
            InsertPosition::End,
        )?;
        if let Some(rel) = &created.file {
            let abs = self.layout.root.join(rel);
            std::fs::write(&abs, &bytes).map_err(Error::Io)?;
            let mut node = created.clone();
            self.store.update_paragraph_content(&mut node, &bytes)?;
        }
        counts.paragraphs += 1;
        progress(counts.paragraphs, file);
        Ok(())
    }

    fn open_rename_modal(&mut self) {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            self.status = "nothing selected to rename".into();
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            return;
        };
        if let Some(reason) = self.protected_block_reason(node) {
            self.status = reason;
            return;
        }
        let mut input = TextInput::new();
        for c in node.title.chars() {
            input.insert_char(c);
        }
        self.modal = Modal::Renaming {
            node_id: id,
            kind: node.kind,
            input,
        };
    }

    fn commit_rename(&mut self) {
        let (node_id, new_title) = match &self.modal {
            Modal::Renaming { node_id, input, .. } => {
                (*node_id, input.as_str().trim().to_string())
            }
            _ => return,
        };
        if new_title.is_empty() {
            self.status = "rename: title can't be empty — type one or Esc to cancel".into();
            return;
        }
        match self.store.rename_node(&self.hierarchy, node_id, &new_title) {
            Ok(()) => {
                self.reload_hierarchy();
                // Refresh editor's title + on-disk rel_path if the
                // renamed node is the open one. 1.2.4+: paragraphs
                // rename their file on disk, so the open doc's
                // rel_path needs to track the new slug.
                if let Some(doc) = self.opened.as_mut() {
                    if doc.id == node_id {
                        doc.title = new_title.clone();
                        if let Some(node) = self.hierarchy.get(node_id) {
                            if let Some(rel) = node.file.as_ref() {
                                doc.rel_path = rel.clone();
                            }
                        }
                    }
                }
                self.modal = Modal::None;
                self.status = format!("renamed to `{new_title}`");
            }
            Err(e) => {
                self.status = format!("rename failed: {e}");
            }
        }
    }

    fn open_snapshot_picker(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "no paragraph open".into();
            return;
        };
        let id = doc.id;
        let title = doc.title.clone();
        match self.store.list_snapshots(id) {
            Ok(snapshots) => {
                if snapshots.is_empty() {
                    self.status =
                        format!("no snapshots yet for `{title}` — press F5 to create one");
                    return;
                }
                self.modal = Modal::SnapshotPicker {
                    paragraph_id: id,
                    paragraph_title: title,
                    snapshots,
                    cursor: 0,
                };
            }
            Err(e) => {
                self.status = format!("snapshot list failed: {e}");
            }
        }
    }

    fn commit_snapshot_load(&mut self) {
        let (snap_id, when) = match &self.modal {
            Modal::SnapshotPicker {
                snapshots, cursor, ..
            } => {
                let Some(snap) = snapshots.get(*cursor) else {
                    self.modal = Modal::None;
                    return;
                };
                (snap.id, snap.created_at)
            }
            _ => return,
        };
        let content = match self.store.snapshot_content(snap_id) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => {
                self.status = "snapshot has no body".into();
                self.modal = Modal::None;
                return;
            }
            Err(e) => {
                self.status = format!("snapshot load failed: {e}");
                self.modal = Modal::None;
                return;
            }
        };

        // Safety net (1.2.4+): before we replace the editor buffer,
        // snapshot whatever is currently in it. Without this, hitting
        // Enter on an old snapshot would silently discard any
        // unsaved typing — "oops! a day of work gone". Now the
        // recoverable history grows by one row instead.
        //
        // The pre-restore snapshot fires `hook.on_snapshot` like any
        // other snapshot. If snapshot creation itself fails, we
        // abort the load: the whole point is data safety, so doing
        // the replace without the safety net would defeat the
        // change. The user can fix the underlying error (disk full,
        // store offline) and retry.
        let pre_restore_id = if let Some(doc) = self.opened.as_ref() {
            let body_now = doc.textarea.lines().join("\n");
            let node = self.hierarchy.get(doc.id).cloned();
            match node {
                Some(n) => match self.store.create_snapshot(&n, body_now.as_bytes()) {
                    Ok(id) => Some(id),
                    Err(e) => {
                        self.status = format!(
                            "snapshot load aborted: safety snapshot failed ({e}) — retry once the store is healthy"
                        );
                        self.modal = Modal::None;
                        return;
                    }
                },
                None => None,
            }
        } else {
            None
        };

        let body = String::from_utf8_lossy(&content).into_owned();
        let Some(doc) = self.opened.as_mut() else {
            self.modal = Modal::None;
            return;
        };
        let mut new_textarea = TextArea::new(body_to_lines(&body));
        new_textarea.set_cursor_line_style(Style::default().add_modifier(Modifier::REVERSED));
        new_textarea.set_line_number_style(Style::default().fg(Color::DarkGray));
        doc.textarea = new_textarea;
        doc.dirty = true;
        doc.scroll_row = 0;
        doc.scroll_col = 0;
        doc.last_activity = std::time::Instant::now();
        // saved_lines stays at the previously-saved on-disk version, so the
        // snapshot text shows as "added" (bold) until the user accepts it
        // by hitting Ctrl+S.
        self.modal = Modal::None;
        self.change_focus(Focus::Editor);
        let safety_msg = match pre_restore_id {
            Some(id) => format!(" · safety snapshot {} created", id.simple()),
            None => String::new(),
        };
        self.status = format!(
            "loaded snapshot from {} — bold marks the change vs saved{}",
            when.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S %z"),
            safety_msg,
        );
    }

    /// Open the snapshot-diff modal against the cursor's snapshot.
    /// Stashes the current `SnapshotPicker` modal inside the new
    /// variant so `Esc` returns to the picker rather than closing
    /// both layers.
    fn open_snapshot_diff(&mut self) {
        let (snap_id, when, paragraph_title) = match &self.modal {
            Modal::SnapshotPicker {
                snapshots,
                cursor,
                paragraph_title,
                ..
            } => {
                let Some(snap) = snapshots.get(*cursor) else {
                    return;
                };
                (snap.id, snap.created_at, paragraph_title.clone())
            }
            _ => return,
        };
        let snapshot_bytes = match self.store.snapshot_content(snap_id) {
            Ok(Some(b)) => b,
            Ok(None) => {
                self.status = "snapshot has no body".into();
                return;
            }
            Err(e) => {
                self.status = format!("snapshot load failed: {e}");
                return;
            }
        };
        let snapshot_text = String::from_utf8_lossy(&snapshot_bytes).into_owned();
        let current_text = self
            .opened
            .as_ref()
            .map(|d| d.textarea.lines().join("\n"))
            .unwrap_or_default();
        let rows = compute_line_diff(&snapshot_text, &current_text);
        let when_str = when
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M:%S %z")
            .to_string();
        let return_to = Box::new(std::mem::replace(&mut self.modal, Modal::None));
        self.modal = Modal::SnapshotDiff {
            paragraph_title,
            when: when_str,
            rows,
            scroll: 0,
            return_to,
        };
        self.status = "diff: snapshot ← left · current → right · ↑↓ scroll · Esc back".into();
    }

    fn delete_current_snapshot(&mut self) {
        let (snap_id, when, paragraph_id, paragraph_title) = match &self.modal {
            Modal::SnapshotPicker {
                snapshots,
                cursor,
                paragraph_id,
                paragraph_title,
            } => {
                let Some(snap) = snapshots.get(*cursor).cloned() else {
                    return;
                };
                (snap.id, snap.created_at, *paragraph_id, paragraph_title.clone())
            }
            _ => return,
        };

        if let Err(e) = self.store.delete_snapshot(snap_id) {
            self.status = format!("delete snapshot failed: {e}");
            return;
        }

        let when_local = when
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M:%S %z");

        match self.store.list_snapshots(paragraph_id) {
            Ok(snapshots) => {
                if snapshots.is_empty() {
                    self.modal = Modal::None;
                    self.status = format!(
                        "deleted snapshot {when_local} — no snapshots left for `{paragraph_title}`"
                    );
                } else {
                    // Keep the cursor on the same row index, clamped
                    // to the new (shorter) list — feels like "the row
                    // below the deleted one slid up".
                    let new_cursor = match &self.modal {
                        Modal::SnapshotPicker { cursor, .. } => {
                            (*cursor).min(snapshots.len() - 1)
                        }
                        _ => 0,
                    };
                    self.modal = Modal::SnapshotPicker {
                        paragraph_id,
                        paragraph_title,
                        snapshots,
                        cursor: new_cursor,
                    };
                    self.status = format!("deleted snapshot {when_local}");
                }
            }
            Err(e) => {
                self.modal = Modal::None;
                self.status =
                    format!("deleted snapshot, but couldn't refresh list: {e}");
            }
        }
    }

    fn open_delete_modal(&mut self) {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            self.status = "nothing selected to delete".into();
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            return;
        };
        if let Some(reason) = self.protected_block_reason(node) {
            self.status = reason;
            return;
        }
        let ids = self.hierarchy.collect_subtree(id);
        let descendant_count = ids.len().saturating_sub(1);
        self.modal = Modal::Deleting {
            root_id: id,
            root_kind: node.kind,
            title: node.title.clone(),
            descendant_count,
            ids,
        };
    }

    /// Returns `Some(reason)` if the given node (or any ancestor) is a
    /// system-protected node — used to block destructive operations from
    /// the UI and surface a status message to the user. `None` means the
    /// node is fully mutable.
    fn protected_block_reason(&self, node: &Node) -> Option<String> {
        if node.protected {
            return Some(format!(
                "“{}” is a system book — it can't be deleted or renamed",
                node.title
            ));
        }
        // Walk ancestors so a paragraph inside Help is also blocked.
        for anc in self.hierarchy.ancestors(node) {
            if anc.protected && anc.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_HELP)
            {
                return Some(format!(
                    "“{}” lives inside the read-only Help book",
                    node.title
                ));
            }
        }
        None
    }

    fn handle_modal_key(&mut self, key: KeyEvent) -> Result<bool> {
        if matches!(key.code, KeyCode::Esc) {
            // SnapshotDiff stashes the picker it opened from in
            // `return_to`; popping that back makes Esc behave
            // like "close the diff, stay in the picker".
            if let Modal::SnapshotDiff { return_to, .. } = &mut self.modal {
                let prev = std::mem::replace(return_to.as_mut(), Modal::None);
                self.modal = prev;
                self.status = "diff closed".into();
                return Ok(false);
            }
            // 1.2.5+ SaveRenderedPng follows the same pattern —
            // restore the underlying RenderedPreview so the user
            // doesn't lose their navigation state when they
            // cancel a save.
            if let Modal::SaveRenderedPng { return_to, .. } = &mut self.modal {
                let prev = std::mem::replace(return_to.as_mut(), Modal::None);
                self.modal = prev;
                self.status = "save PNG: cancelled · preview restored".into();
                return Ok(false);
            }
            self.modal = Modal::None;
            return Ok(false);
        }

        let is_adding = matches!(self.modal, Modal::Adding { .. });
        let is_deleting = matches!(self.modal, Modal::Deleting { .. });
        let is_snapshot = matches!(self.modal, Modal::SnapshotPicker { .. });
        let is_renaming = matches!(self.modal, Modal::Renaming { .. });
        let is_file_picker = matches!(self.modal, Modal::FilePicker(_));
        let is_find = matches!(self.modal, Modal::FindReplace { .. });
        let is_quickref = matches!(self.modal, Modal::QuickRef { .. });
        let is_credits = matches!(self.modal, Modal::Credits { .. });
        let is_book_info = matches!(self.modal, Modal::BookInfo { .. });
        let is_llm_picker = matches!(self.modal, Modal::LlmPicker { .. });
        let is_image_picker = matches!(self.modal, Modal::ImagePicker { .. });
        let is_function_picker = matches!(self.modal, Modal::FunctionPicker { .. });
        let is_status_filter = matches!(self.modal, Modal::StatusFilter { .. });
        let is_help_query = matches!(self.modal, Modal::HelpQuery { .. });
        let is_chat_search_prompt = matches!(self.modal, Modal::ChatSearchPrompt { .. });
        let is_bund_eval = matches!(self.modal, Modal::BundEval { .. });
        let is_bund_pane = matches!(self.modal, Modal::BundPane { .. });
        let is_script_picker = matches!(self.modal, Modal::ScriptPicker { .. });
        let is_bund_input = matches!(self.modal, Modal::BundInput { .. });
        let is_similar_picker = matches!(self.modal, Modal::SimilarPicker { .. });
        let is_progress = matches!(self.modal, Modal::Progress { .. });
        let is_paragraph_target = matches!(self.modal, Modal::ParagraphTarget { .. });
        let is_save_markdown = matches!(self.modal, Modal::SaveMarkdown { .. });
        let is_snapshot_diff = matches!(self.modal, Modal::SnapshotDiff { .. });
        let is_link_picker = matches!(self.modal, Modal::LinkPicker { .. });
        let is_backlink_picker = matches!(self.modal, Modal::BacklinkPicker { .. });
        let is_bookmark_picker = matches!(self.modal, Modal::BookmarkPicker { .. });
        let is_fuzzy_paragraph_picker = matches!(self.modal, Modal::FuzzyParagraphPicker { .. });
        let is_rendered_preview = matches!(self.modal, Modal::RenderedPreview { .. });
        let is_save_rendered_png = matches!(self.modal, Modal::SaveRenderedPng { .. });

        if is_quickref {
            self.quickref_handle_key(key);
            return Ok(false);
        }
        if is_credits {
            self.credits_handle_key(key);
            return Ok(false);
        }
        if is_book_info {
            self.book_info_handle_key(key);
            return Ok(false);
        }
        if is_llm_picker {
            self.llm_picker_handle_key(key);
            return Ok(false);
        }
        if is_image_picker {
            self.image_picker_handle_key(key);
            return Ok(false);
        }
        if is_function_picker {
            self.function_picker_handle_key(key);
            return Ok(false);
        }
        if is_status_filter {
            self.status_filter_handle_key(key);
            return Ok(false);
        }

        if is_help_query {
            // Enter submits, anything else feeds the input box. Esc was
            // already handled at the top of this function.
            if matches!(key.code, KeyCode::Enter) {
                let query = match &self.modal {
                    Modal::HelpQuery { input } => input.as_str().to_string(),
                    _ => String::new(),
                };
                self.modal = Modal::None;
                self.start_help_inference(&query);
                return Ok(false);
            }
            if let Modal::HelpQuery { input } = &mut self.modal {
                handle_text_input_key(input, key);
            }
            return Ok(false);
        }

        if is_bund_eval {
            // Enter runs the expression against Adam; result goes to
            // the status bar. Anything else feeds the input box.
            if matches!(key.code, KeyCode::Enter) {
                let expr = match &self.modal {
                    Modal::BundEval { input } => input.as_str().to_string(),
                    _ => String::new(),
                };
                self.modal = Modal::None;
                match self.scripting_eval(&expr) {
                    Ok(out) => {
                        self.status = format_eval_output(&out, None);
                    }
                    Err(e) => {
                        self.status = format!("bund: eval failed — {e:#}");
                    }
                }
                return Ok(false);
            }
            if let Modal::BundEval { input } = &mut self.modal {
                handle_text_input_key(input, key);
            }
            return Ok(false);
        }

        if is_bund_pane {
            self.bund_pane_handle_key(key);
            return Ok(false);
        }

        if is_script_picker {
            self.script_picker_handle_key(key);
            return Ok(false);
        }

        if is_similar_picker {
            self.similar_picker_handle_key(key);
            return Ok(false);
        }

        if is_progress {
            self.progress_modal_handle_key(key);
            return Ok(false);
        }

        if is_link_picker {
            self.link_picker_handle_key(key);
            return Ok(false);
        }

        if is_backlink_picker {
            self.backlink_picker_handle_key(key);
            return Ok(false);
        }

        if is_bookmark_picker {
            self.bookmark_picker_handle_key(key);
            return Ok(false);
        }

        if is_fuzzy_paragraph_picker {
            self.fuzzy_paragraph_picker_handle_key(key);
            return Ok(false);
        }

        if is_snapshot_diff {
            if let Modal::SnapshotDiff { scroll, rows, .. } = &mut self.modal {
                let total = rows.len();
                let page: usize = 12;
                match key.code {
                    KeyCode::Up => *scroll = scroll.saturating_sub(1),
                    KeyCode::Down => {
                        if *scroll + 1 < total {
                            *scroll += 1;
                        }
                    }
                    KeyCode::PageUp => *scroll = scroll.saturating_sub(page),
                    KeyCode::PageDown => {
                        *scroll = (*scroll + page).min(total.saturating_sub(1));
                    }
                    KeyCode::Home => *scroll = 0,
                    KeyCode::End => *scroll = total.saturating_sub(1),
                    _ => {}
                }
            }
            return Ok(false);
        }

        if is_paragraph_target {
            if matches!(key.code, KeyCode::Enter) {
                let raw = match &self.modal {
                    Modal::ParagraphTarget { input } => input.as_str().trim().to_string(),
                    _ => String::new(),
                };
                self.modal = Modal::None;
                self.commit_paragraph_target(&raw);
                return Ok(false);
            }
            if let Modal::ParagraphTarget { input } = &mut self.modal {
                handle_text_input_key(input, key);
            }
            return Ok(false);
        }

        if is_save_markdown {
            if matches!(key.code, KeyCode::Enter) {
                let (body, label, raw) = match &self.modal {
                    Modal::SaveMarkdown { input, body, label } => (
                        body.clone(),
                        label.clone(),
                        input.as_str().to_string(),
                    ),
                    _ => (String::new(), String::new(), String::new()),
                };
                self.modal = Modal::None;
                self.commit_save_markdown(body, label, raw);
                return Ok(false);
            }
            if let Modal::SaveMarkdown { input, .. } = &mut self.modal {
                handle_text_input_key(input, key);
            }
            return Ok(false);
        }

        if is_rendered_preview {
            // Esc is intercepted by the global modal-close
            // handler at the top of this function. Local keys:
            //   ← / →  — navigate pages
            //   S / s — save current page (open picker, mode Single)
            //   A / a — save every page (open picker, mode All)
            //   anything else — swallowed so the editor doesn't see it
            match key.code {
                KeyCode::Left | KeyCode::Up => {
                    if let Modal::RenderedPreview {
                        pages,
                        current_page,
                        ..
                    } = &mut self.modal
                    {
                        if *current_page > 0 {
                            *current_page -= 1;
                            let total = pages.len();
                            let p = &pages[*current_page];
                            self.status = format!(
                                "render ¶ · page {}/{}  · {}×{}",
                                *current_page + 1,
                                total,
                                p.width,
                                p.height,
                            );
                        }
                    }
                }
                KeyCode::Right | KeyCode::Down => {
                    if let Modal::RenderedPreview {
                        pages,
                        current_page,
                        ..
                    } = &mut self.modal
                    {
                        if *current_page + 1 < pages.len() {
                            *current_page += 1;
                            let total = pages.len();
                            let p = &pages[*current_page];
                            self.status = format!(
                                "render ¶ · page {}/{}  · {}×{}",
                                *current_page + 1,
                                total,
                                p.width,
                                p.height,
                            );
                        }
                    }
                }
                KeyCode::Home => {
                    if let Modal::RenderedPreview { current_page, .. } =
                        &mut self.modal
                    {
                        *current_page = 0;
                    }
                }
                KeyCode::End => {
                    if let Modal::RenderedPreview {
                        pages,
                        current_page,
                        ..
                    } = &mut self.modal
                    {
                        *current_page = pages.len().saturating_sub(1);
                    }
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    self.open_save_rendered_png_picker(false);
                }
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    self.open_save_rendered_png_picker(true);
                }
                _ => {}
            }
            return Ok(false);
        }

        if is_save_rendered_png {
            // Esc → restore the preview, handled at the top of
            // this function via the `return_to` stash pattern.
            if matches!(key.code, KeyCode::Enter) {
                let taken = std::mem::replace(&mut self.modal, Modal::None);
                if let Modal::SaveRenderedPng {
                    input,
                    body,
                    settings,
                    title,
                    pages,
                    return_to: _,
                } = taken
                {
                    let raw = input.as_str().to_string();
                    self.commit_save_rendered_png(
                        &body, &settings, &raw, &title, pages,
                    );
                }
                return Ok(false);
            }
            if let Modal::SaveRenderedPng { input, .. } = &mut self.modal {
                handle_text_input_key(input, key);
            }
            return Ok(false);
        }

        if is_bund_input {
            if matches!(key.code, KeyCode::Enter) {
                let (typed, hook) = match &self.modal {
                    Modal::BundInput { input, hook, .. } => {
                        (input.as_str().to_string(), hook.clone())
                    }
                    _ => (String::new(), String::new()),
                };
                self.modal = Modal::None;
                if !hook.is_empty() {
                    crate::scripting::hooks::fire(
                        &hook,
                        vec![rust_dynamic::value::Value::from_string(typed)],
                    );
                }
                return Ok(false);
            }
            if let Modal::BundInput { input, .. } = &mut self.modal {
                handle_text_input_key(input, key);
            }
            return Ok(false);
        }

        if is_chat_search_prompt {
            // Enter commits the query into `chat_search`; the rendered
            // chat-history pane then auto-centres on the match. Esc
            // closes the modal without starting a search (global Esc
            // handler at the top of this function does the close).
            if matches!(key.code, KeyCode::Enter) {
                let query = match &self.modal {
                    Modal::ChatSearchPrompt { input } => {
                        input.as_str().trim().to_string()
                    }
                    _ => String::new(),
                };
                self.modal = Modal::None;
                self.commit_chat_search(query);
                return Ok(false);
            }
            if let Modal::ChatSearchPrompt { input } = &mut self.modal {
                handle_text_input_key(input, key);
            }
            return Ok(false);
        }

        if is_find {
            let mut commit = false;
            if let Modal::FindReplace {
                search_input,
                replace_input,
                focus_replace,
            } = &mut self.modal
            {
                match key.code {
                    KeyCode::Enter => commit = true,
                    KeyCode::Tab => {
                        if replace_input.is_some() {
                            *focus_replace = !*focus_replace;
                        }
                    }
                    KeyCode::Backspace => {
                        if *focus_replace {
                            if let Some(r) = replace_input.as_mut() {
                                r.backspace();
                            }
                        } else {
                            search_input.backspace();
                        }
                    }
                    KeyCode::Delete => {
                        if *focus_replace {
                            if let Some(r) = replace_input.as_mut() {
                                r.delete();
                            }
                        } else {
                            search_input.delete();
                        }
                    }
                    KeyCode::Left => {
                        if *focus_replace {
                            if let Some(r) = replace_input.as_mut() {
                                r.move_left();
                            }
                        } else {
                            search_input.move_left();
                        }
                    }
                    KeyCode::Right => {
                        if *focus_replace {
                            if let Some(r) = replace_input.as_mut() {
                                r.move_right();
                            }
                        } else {
                            search_input.move_right();
                        }
                    }
                    KeyCode::Home => {
                        if *focus_replace {
                            if let Some(r) = replace_input.as_mut() {
                                r.move_home();
                            }
                        } else {
                            search_input.move_home();
                        }
                    }
                    KeyCode::End => {
                        if *focus_replace {
                            if let Some(r) = replace_input.as_mut() {
                                r.move_end();
                            }
                        } else {
                            search_input.move_end();
                        }
                    }
                    KeyCode::Char(c) => {
                        let mut residual = key.modifiers;
                        residual.remove(KeyModifiers::SHIFT);
                        if residual.is_empty() {
                            let final_c = if key.modifiers.contains(KeyModifiers::SHIFT)
                                && c.is_ascii_alphabetic()
                            {
                                c.to_ascii_uppercase()
                            } else {
                                c
                            };
                            if *focus_replace {
                                if let Some(r) = replace_input.as_mut() {
                                    r.insert_char(final_c);
                                }
                            } else {
                                search_input.insert_char(final_c);
                            }
                        }
                    }
                    _ => {}
                }
            }
            if commit {
                self.commit_find();
            }
            return Ok(false);
        }


        if is_file_picker {
            let mut commit = false;
            if let Modal::FilePicker(picker) = &mut self.modal {
                match key.code {
                    KeyCode::Up => picker.move_up(),
                    KeyCode::Down => picker.move_down(),
                    KeyCode::PageUp => picker.page_up(10),
                    KeyCode::PageDown => picker.page_down(10),
                    KeyCode::Home => picker.jump_first(),
                    KeyCode::End => picker.jump_last(),
                    KeyCode::Right => picker.expand(),
                    KeyCode::Left => picker.collapse_or_step_out(),
                    KeyCode::Enter => commit = true,
                    _ => {}
                }
            }
            if commit {
                self.commit_file_pick();
            }
            return Ok(false);
        }

        if is_renaming {
            let mut commit = false;
            if let Modal::Renaming { input, .. } = &mut self.modal {
                match key.code {
                    KeyCode::Enter => commit = true,
                    KeyCode::Backspace => input.backspace(),
                    KeyCode::Delete => input.delete(),
                    KeyCode::Left => input.move_left(),
                    KeyCode::Right => input.move_right(),
                    KeyCode::Home => input.move_home(),
                    KeyCode::End => input.move_end(),
                    KeyCode::Char(c) => {
                        let mut residual = key.modifiers;
                        residual.remove(KeyModifiers::SHIFT);
                        if residual.is_empty() {
                            let final_c = if key.modifiers.contains(KeyModifiers::SHIFT)
                                && c.is_ascii_alphabetic()
                            {
                                c.to_ascii_uppercase()
                            } else {
                                c
                            };
                            input.insert_char(final_c);
                        }
                    }
                    _ => {}
                }
            }
            if commit {
                self.commit_rename();
            }
            return Ok(false);
        }

        if is_snapshot {
            let mut commit = false;
            let mut delete = false;
            let mut view_diff = false;
            if let Modal::SnapshotPicker {
                snapshots, cursor, ..
            } = &mut self.modal
            {
                match key.code {
                    KeyCode::Up => {
                        if *cursor > 0 {
                            *cursor -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if *cursor + 1 < snapshots.len() {
                            *cursor += 1;
                        }
                    }
                    KeyCode::Home => *cursor = 0,
                    KeyCode::End => {
                        *cursor = snapshots.len().saturating_sub(1);
                    }
                    KeyCode::Enter => commit = true,
                    // D (case-insensitive) or the Delete key removes
                    // the cursor's snapshot. No further confirmation —
                    // snapshots are explicit creations (F5 / Ctrl+B N),
                    // and refreshing the list keeps the cursor sane.
                    KeyCode::Char('D') | KeyCode::Char('d') | KeyCode::Delete => {
                        delete = true;
                    }
                    // V (case-insensitive) opens the snapshot
                    // diff modal against the open paragraph's
                    // current buffer. Read-only — Esc returns to
                    // the picker; closing the picker entirely
                    // requires another Esc.
                    KeyCode::Char('V') | KeyCode::Char('v') => {
                        view_diff = true;
                    }
                    _ => {}
                }
            }
            if commit {
                self.commit_snapshot_load();
            } else if delete {
                self.delete_current_snapshot();
            } else if view_diff {
                self.open_snapshot_diff();
            }
            return Ok(false);
        }

        if is_adding {
            let mut commit = false;
            if let Modal::Adding { input, .. } = &mut self.modal {
                match key.code {
                    KeyCode::Enter => commit = true,
                    KeyCode::Backspace => input.backspace(),
                    KeyCode::Delete => input.delete(),
                    KeyCode::Left => input.move_left(),
                    KeyCode::Right => input.move_right(),
                    KeyCode::Home => input.move_home(),
                    KeyCode::End => input.move_end(),
                    KeyCode::Char(c) => {
                        let mut residual = key.modifiers;
                        residual.remove(KeyModifiers::SHIFT);
                        if residual.is_empty() {
                            // Some terminals report Shift+letter as lowercase
                            // char + SHIFT modifier; others as uppercase char
                            // + SHIFT. Normalize so capital letters always go
                            // into the buffer when Shift was held.
                            let final_c = if key.modifiers.contains(KeyModifiers::SHIFT)
                                && c.is_ascii_alphabetic()
                            {
                                c.to_ascii_uppercase()
                            } else {
                                c
                            };
                            input.insert_char(final_c);
                        }
                    }
                    _ => {}
                }
            }
            if commit {
                self.commit_add();
            }
        } else if is_deleting {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => self.commit_delete(),
                KeyCode::Char('n') | KeyCode::Char('N') => self.modal = Modal::None,
                _ => {}
            }
        }
        Ok(false)
    }

    fn commit_add(&mut self) {
        let (kind, parent_id, raw_title, position) = match &self.modal {
            Modal::Adding {
                kind,
                parent_id,
                input,
                position,
                ..
            } => (
                *kind,
                *parent_id,
                input.as_str().trim().to_string(),
                *position,
            ),
            _ => return,
        };

        // Paragraphs can be added with an empty title — they'll be given a
        // placeholder ("Untitled paragraph") that the next save replaces with
        // the first sentence of the body. Branches still require a title
        // because they have no content from which to derive one.
        let title = if raw_title.is_empty() {
            if kind == NodeKind::Paragraph {
                PARAGRAPH_PLACEHOLDER_TITLE.to_string()
            } else {
                self.status =
                    format!("a {} needs a title — type one or Esc to cancel", kind.as_str());
                return;
            }
        } else {
            raw_title
        };

        let parent = parent_id.and_then(|id| self.hierarchy.get(id)).cloned();
        match self.store.create_node(
            &self.cfg,
            &self.hierarchy,
            kind,
            &title,
            parent.as_ref(),
            None,
            position,
        ) {
            Ok(node) => {
                let new_id = node.id;
                // For root-level user books: provision the artefacts
                // subdirectory and the Typst-book skeleton (chapter +
                // index/settings/globals paragraphs). No-op for other
                // kinds; failure logs to status but doesn't roll back
                // the just-created book.
                if let Err(e) = self.store.provision_user_book(&self.cfg, &node) {
                    self.status = format!(
                        "added {} `{}` — but Typst skeleton failed: {e}",
                        kind.as_str(),
                        node.title
                    );
                } else {
                    self.status = format!("added {} `{}`", kind.as_str(), node.title);
                }
                self.modal = Modal::None;
                self.reload_hierarchy();
                if let Some(i) = self.rows.iter().position(|(id, _)| *id == new_id) {
                    self.tree_cursor = i;
                }
            }
            Err(e) => {
                self.status = format!("add failed: {e}");
            }
        }
    }

    fn commit_delete(&mut self) {
        let (root_id, root_kind, ids, title) = match &self.modal {
            Modal::Deleting {
                root_id,
                root_kind,
                ids,
                title,
                ..
            } => (*root_id, *root_kind, ids.clone(), title.clone()),
            _ => return,
        };
        let root_node = match self.hierarchy.get(root_id) {
            Some(n) => n.clone(),
            None => {
                self.modal = Modal::None;
                self.status = "node already gone".into();
                return;
            }
        };
        let parent_id = root_node.parent_id;

        let fs_rel = match root_kind {
            NodeKind::Paragraph => root_node
                .file
                .as_ref()
                .map(std::path::PathBuf::from)
                .unwrap_or_default(),
            _ => self.hierarchy.fs_path(&root_node, &self.layout),
        };

        if let Err(e) = self.store.delete_subtree(&fs_rel, &ids) {
            self.status = format!("delete failed: {e}");
            return;
        }

        // Close editor if its open doc was inside the deleted subtree.
        if let Some(doc) = &self.opened {
            if ids.contains(&doc.id) {
                self.opened = None;
            }
        }

        self.modal = Modal::None;
        self.status = format!(
            "deleted {} `{}` ({} other node{} removed)",
            root_kind.as_str(),
            title,
            ids.len() - 1,
            if ids.len() == 2 { "" } else { "s" }
        );
        self.reload_hierarchy();
        if let Some(pid) = parent_id {
            if let Some(i) = self.rows.iter().position(|(id, _)| *id == pid) {
                self.tree_cursor = i;
            }
        }
    }

    // -------- open / save -------------------------------------------------

    fn open_selected(&mut self) -> Result<()> {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            return Ok(());
        };
        let Some(node) = self.hierarchy.get(id).cloned() else {
            return Ok(());
        };

        match node.kind {
            // Scripts are text leaves like Paragraphs — same load
            // path, same editor surface. Real Bund syntax
            // highlighting is a follow-up; today they render as
            // plain text (which is still legible because bundcore's
            // syntax is sparse: words + braces + strings).
            NodeKind::Paragraph | NodeKind::Script => self.load_paragraph(&node)?,
            NodeKind::Image => self.show_image_info(&node),
            _ => {
                self.status = format!(
                    "`{}` is a {} (Enter opens paragraphs / images / scripts)",
                    node.title,
                    node.kind.as_str()
                );
            }
        }
        Ok(())
    }

    /// Enter on an Image row: try the ratatui-image preview modal,
    /// fall back to a status-bar info line when the picker isn't
    /// available (preview disabled, terminal lacks graphics protocol,
    /// or the image bytes aren't decodable by the `image` crate).
    fn show_image_info(&mut self, node: &Node) {
        let fs_rel = node.file.clone().unwrap_or_else(|| "<no path>".into());
        let abs = self.layout.root.join(&fs_rel);
        let size = std::fs::metadata(&abs)
            .map(|m| m.len())
            .unwrap_or(0);

        // Preview path: fetch bytes from bdslib (source of truth),
        // decode, build a resize protocol, pop the modal.
        if let Some(picker) = self.image_picker.as_ref() {
            match self.store.image_bytes(node.id) {
                Ok(Some(bytes)) => match image::load_from_memory(&bytes) {
                    Ok(dyn_img) => {
                        let proto = picker.new_resize_protocol(dyn_img);
                        self.modal = Modal::ImagePreview {
                            title: node.title.clone(),
                            fs_rel: fs_rel.clone(),
                            size_bytes: size,
                            proto,
                        };
                        self.status = format!(
                            "🖼 {}  ·  Esc closes  ·  {} bytes",
                            node.title, size
                        );
                        return;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "image decode failed for {}: {e} — falling back to info line",
                            node.title
                        );
                    }
                },
                Ok(None) => {
                    tracing::warn!(
                        "image {} has no bytes in bdslib — info line only",
                        node.title
                    );
                }
                Err(e) => {
                    tracing::warn!("image_bytes({}): {e}", node.title);
                }
            }
        }

        // Fallback: status-bar one-liner.
        let caption_hint = node
            .image_caption
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("(no caption)");
        self.status = format!(
            "🖼 {} · {} · {} bytes · {}",
            node.title, fs_rel, size, caption_hint
        );
    }

    fn load_paragraph(&mut self, node: &Node) -> Result<()> {
        if let Some(prev) = &self.opened {
            if prev.id == node.id {
                self.change_focus(Focus::Editor);
                return Ok(());
            }
            // Auto-save any pending edits in the previous paragraph before
            // swapping to the new one. If the save fails (disk error etc.),
            // keep the old doc open so the user can see and fix it.
            if prev.dirty {
                let _ = self.save_current();
                if self.opened.as_ref().is_some_and(|d| d.dirty) {
                    self.status = format!(
                        "couldn't autosave `{}` — opening blocked. Fix the error or Ctrl+S manually.",
                        self.opened.as_ref().map(|d| d.title.as_str()).unwrap_or("")
                    );
                    return Ok(());
                }
            }
            // Memorise the outgoing paragraph's cursor so re-opening it
            // (now or next session) lands back where the user left it.
            self.snapshot_open_paragraph_cursor();
        }

        let Some(rel) = node.file.as_ref() else {
            self.status = format!("paragraph `{}` has no file on disk", node.title);
            return Ok(());
        };
        let abs = self.layout.root.join(rel);
        let body = match std::fs::read_to_string(&abs) {
            Ok(b) => b,
            Err(e) => {
                self.status = format!("read {}: {e}", abs.display());
                return Ok(());
            }
        };

        let lines = body_to_lines(&body);
        let saved_lines = lines.clone();
        let mut textarea = TextArea::new(lines);
        textarea.set_cursor_line_style(Style::default().add_modifier(Modifier::REVERSED));
        textarea.set_line_number_style(Style::default().fg(Color::DarkGray));

        let read_only = self.hierarchy.ancestors(node).iter().any(|a| {
            a.protected && a.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_HELP)
        });

        // Restore the saved cursor + scroll for this paragraph if we've
        // seen it before. Coords are clamped against the loaded buffer so a
        // shorter post-edit body can't crash the cursor.
        let saved_cursor = self.paragraph_cursors.get(&node.id).copied();
        let (init_row, init_col, init_scroll_row, init_scroll_col) = match saved_cursor {
            Some(pc) => {
                let max_row = textarea.lines().len().saturating_sub(1);
                let row = pc.cursor_row.min(max_row);
                let line_len = textarea
                    .lines()
                    .get(row)
                    .map_or(0, |s| s.chars().count());
                let col = pc.cursor_col.min(line_len);
                (row, col, pc.scroll_row.min(max_row), pc.scroll_col)
            }
            None => (0, 0, 0, 0),
        };
        if init_row > 0 || init_col > 0 {
            textarea.move_cursor(CursorMove::Jump(init_row as u16, init_col as u16));
        }

        self.opened = Some(OpenedDoc {
            id: node.id,
            title: node.title.clone(),
            rel_path: rel.clone(),
            textarea,
            dirty: false,
            scroll_row: init_scroll_row,
            scroll_col: init_scroll_col,
            block_anchor: None,
            last_activity: std::time::Instant::now(),
            saved_lines,
            split: None,
            search: None,
            read_only,
            correction_baseline: None,
            // Script nodes default to the "bund" content_type even
            // if the persisted metadata is missing it — covers
            // scripts created before content_type stamping landed.
            content_type: node
                .content_type
                .clone()
                .or_else(|| match node.kind {
                    NodeKind::Script => Some("bund".to_string()),
                    _ => None,
                }),
            typst_diagnostics: Vec::new(),
            typst_diagnostics_checked_at: std::time::Instant::now(),
        });
        self.refresh_typst_diagnostics_for_opened();
        self.change_focus(Focus::Editor);
        self.status = format!("opened {}", abs.display());
        Ok(())
    }

    fn save_current(&mut self) -> Result<()> {
        let Some(doc) = self.opened.as_mut() else {
            return Ok(());
        };
        if doc.read_only {
            // Quietly clear the dirty bit so the autosave loop doesn't keep
            // retrying. Nothing got mutated anyway — the editor's key handler
            // blocks every write — but a stray block_anchor or focus blur
            // could still flip dirty=true on a corner case.
            doc.dirty = false;
            self.status = "Help is read-only — nothing to save".into();
            return Ok(());
        }
        let body = doc.textarea.lines().join("\n");
        // Capture pre-save word count before we overwrite anything
        // — used by the progress event log to compute word_delta.
        let prev_words = crate::progress::count_words(&doc.saved_lines.join("\n"));
        let abs = self.layout.root.join(&doc.rel_path);

        // Filesystem write first; if that fails, abort before touching the store.
        if let Err(e) = std::fs::write(&abs, body.as_bytes()) {
            self.status = format!("write {}: {e}", abs.display());
            return Ok(());
        }

        let id = doc.id;
        let Some(mut node) = self.hierarchy.get(id).cloned() else {
            self.status = format!("node {id} missing from hierarchy — try reopening the TUI");
            return Ok(());
        };

        // If this paragraph still has the placeholder title, derive a real one
        // from the body's first sentence and stamp it onto the node — that
        // becomes the displayed name in the tree pane.
        //
        // 1.2.4+: when we auto-derive a title here, route through
        // `rename_node` so the on-disk filename + slug track the
        // new title. The body has already been written to the
        // OLD path above; `rename_node` will `fs::rename` it to
        // the new path, so the bytes follow the new name.
        let title_was_placeholder = node.title == PARAGRAPH_PLACEHOLDER_TITLE;
        if title_was_placeholder {
            if let Some(derived) = extract_first_sentence(&body) {
                if let Err(e) =
                    self.store.rename_node(&self.hierarchy, node.id, &derived)
                {
                    tracing::warn!(
                        target: "inkhaven::save",
                        "auto-rename to first sentence failed: {e:#}",
                    );
                } else {
                    // Reload so the local `node` + the open doc
                    // reflect the new slug + file path. If the
                    // hierarchy reload itself fails, leave the
                    // existing one in place — the rename is
                    // already on disk, just no in-memory refresh.
                    if let Ok(h) =
                        crate::store::hierarchy::Hierarchy::load(&self.store)
                    {
                        self.hierarchy = h;
                    }
                    if let Some(refreshed) = self.hierarchy.get(node.id).cloned() {
                        // Sync `doc` (the outstanding &mut borrow
                        // taken at the top of save_current_inner)
                        // so its title + rel_path match the new
                        // on-disk layout.
                        doc.title = refreshed.title.clone();
                        if let Some(rel) = refreshed.file.as_ref() {
                            doc.rel_path = rel.clone();
                        }
                        node = refreshed;
                    }
                }
            }
        }

        if let Err(e) = self
            .store
            .update_paragraph_content(&mut node, body.as_bytes())
        {
            self.status = format!("store update failed: {e}");
            return Ok(());
        }
        if let Err(e) = self.store.sync() {
            self.status = format!("store sync failed: {e}");
            return Ok(());
        }

        doc.dirty = false;
        // Refresh the saved-lines snapshot so the bold-new-additions overlay
        // resets, and stamp last_activity to "now" so idle autosave restarts.
        // Save is the explicit "I've reviewed and accepted the
        // corrections" signal — drop the highlight + resume normal
        // autosave cadence.
        doc.saved_lines = doc.textarea.lines().to_vec();
        doc.correction_baseline = None;
        doc.last_activity = std::time::Instant::now();
        let words = node.word_count;
        if title_was_placeholder && node.title != PARAGRAPH_PLACEHOLDER_TITLE {
            self.status = format!(
                "saved {} ({} words) · named `{}` from first sentence",
                abs.display(),
                words,
                node.title
            );
        } else {
            self.status = format!("saved {} ({} words, re-embedded)", abs.display(), words);
        }
        // Progress event log. The book this paragraph belongs
        // to feeds per-book aggregates; project-wide events drop
        // book_id = None for the same record.
        let new_words = crate::progress::count_words(&body);
        let book_id = self.book_of_node(node.id);
        crate::progress::record_save(node.id, book_id, prev_words, new_words);
        // Auto-promote on goal-hit. Idempotent per
        // (paragraph, status) — see Goals.auto_promote_on_target
        // semantics.
        self.maybe_auto_promote_on_target(node.id, new_words);
        self.reload_hierarchy();
        self.refresh_progress_cache();
        // 1.2.5+: refresh typst-syntax diagnostics on save. Pulls
        // the most-recently-saved body straight from the editor's
        // mutable doc so the next render reflects errors the user
        // just introduced (or fixed).
        self.refresh_typst_diagnostics_for_opened();
        Ok(())
    }

    /// 1.2.5+: re-parse the open paragraph with `typst-syntax` and
    /// cache the resulting diagnostics on `OpenedDoc`. Honors the
    /// `typst_compile.diagnostics` HJSON flag and the buffer's
    /// `content_type` — only typst sources are checked (Bund /
    /// HJSON / others skip out cleanly). Status bar surfaces the
    /// first error so the user sees the line number at a glance;
    /// the rest stay cached on the doc for any future "next error"
    /// chord.
    fn refresh_typst_diagnostics_for_opened(&mut self) {
        if !self.cfg.typst_compile.diagnostics {
            return;
        }
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        // Anything not typst-shaped — Bund scripts, HJSON data
        // nodes, images — should never be fed to the typst
        // parser; just clear stale diagnostics and bail.
        let is_typst = match doc.content_type.as_deref() {
            None | Some("") | Some("typst") => true,
            _ => false,
        };
        if !is_typst {
            doc.typst_diagnostics.clear();
            doc.typst_diagnostics_checked_at = std::time::Instant::now();
            return;
        }
        let body = doc.textarea.lines().join("\n");
        // Phase 1 baseline: parse-only diagnostics via `typst-syntax`.
        // Cheap, always available, no engine dependency.
        let mut diags = crate::typst_check::check(&body);
        // 1.2.5+: when the user has the in-process engine on AND
        // opted into semantic diagnostics, run a full
        // `typst::compile` against the paragraph in isolation and
        // surface semantic errors the parser can't catch
        // (unknown functions, type errors, etc.). We APPEND to
        // the parse diagnostics rather than replace — a
        // syntactically-broken buffer often produces a flurry of
        // confusing semantic errors and the parse error is the
        // root cause to surface first.
        if self.cfg.typst_compile.semantic_diagnostics
            && self.cfg.typst_compile.use_inprocess_engine()
            && diags.is_empty()
        {
            let settings = crate::typst_world::WorldSettings::from_cfg(
                &self.cfg.typst_compile,
            );
            let semantic =
                crate::typst_inprocess::check_semantic(&body, settings);
            diags.extend(semantic);
        }
        doc.typst_diagnostics = diags;
        doc.typst_diagnostics_checked_at = std::time::Instant::now();
        if let Some(first) = doc.typst_diagnostics.first() {
            // Don't blow away a more-recent status (a save's own
            // "wrote N bytes" message etc.) — only stamp the
            // diagnostics line if we have errors to show. The
            // save-path caller is OK with this being the final
            // status because errors-on-save are exactly what the
            // user needs to see next.
            self.status = first.summary();
        }
    }

    /// Ctrl+V N (1.2.5+) — move the editor cursor to the next
    /// typst diagnostic in the open buffer. Wraps around at the
    /// end. Refreshes the diagnostics cache up-front so the user
    /// always navigates against the current buffer state, even
    /// if they haven't paused long enough for the idle recheck
    /// to fire.
    fn jump_to_next_diagnostic(&mut self) {
        if self.opened.is_none() {
            self.status = "next diag: no paragraph open".into();
            return;
        }
        // Force a fresh recheck — keeps the navigation honest
        // when the user has been typing fast.
        self.refresh_typst_diagnostics_for_opened();
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        if doc.typst_diagnostics.is_empty() {
            self.status = "next diag: no typst diagnostics in this buffer".into();
            return;
        }
        // Cursor in tui-textarea is (row, col), both 0-based.
        // TypstDiagnostic.line/col are 1-based; normalise for
        // comparison.
        let (cur_row, cur_col) = doc.textarea.cursor();
        let cur1 = (cur_row + 1, cur_col + 1);
        // Find the first diagnostic strictly past the cursor.
        // Ties on the same line go to the higher column.
        let mut sorted_idxs: Vec<usize> = (0..doc.typst_diagnostics.len()).collect();
        sorted_idxs.sort_by_key(|&i| {
            let d = &doc.typst_diagnostics[i];
            (d.line, d.col)
        });
        let next = sorted_idxs.iter().copied().find(|&i| {
            let d = &doc.typst_diagnostics[i];
            (d.line, d.col) > cur1
        });
        let chosen = match next {
            Some(i) => i,
            None => {
                // Wrap to the first.
                sorted_idxs[0]
            }
        };
        let target = doc.typst_diagnostics[chosen].clone();
        let row = target.line.saturating_sub(1) as u16;
        let col = target.col.saturating_sub(1) as u16;
        doc.textarea
            .move_cursor(tui_textarea::CursorMove::Jump(row, col));
        let total = doc.typst_diagnostics.len();
        let wrapped_note = if next.is_none() && total > 1 {
            " (wrapped)"
        } else {
            ""
        };
        self.status = format!(
            "diag {}/{}{wrapped_note}  line {}:{}  — {}",
            sorted_idxs.iter().position(|&i| i == chosen).unwrap_or(0) + 1,
            total,
            target.line,
            target.col,
            target.message,
        );
    }

    /// Promote the paragraph one ladder step if (a) the project
    /// has `goals.auto_promote_on_target = true`, (b) the
    /// paragraph carries a positive `target_words`, (c) the
    /// current `word_count` meets or exceeds it, and (d) the
    /// last auto-promote isn't already at the current status.
    /// On promotion: bumps `status` via `next_status`, sets
    /// `target_hit_at_status` to the new status, fires the
    /// status-change progress event, persists via
    /// `store.raw().update_metadata`.
    fn maybe_auto_promote_on_target(&mut self, id: Uuid, current_words: i64) {
        if !self.cfg.goals.auto_promote_on_target {
            return;
        }
        let node = match self.hierarchy.get(id) {
            Some(n) => n.clone(),
            None => return,
        };
        if node.kind != NodeKind::Paragraph {
            return;
        }
        let Some(target) = node.target_words.filter(|n| *n > 0) else {
            return;
        };
        if current_words < target as i64 {
            return;
        }
        let current_status = node.status.clone();
        // Idempotent: already promoted at this status → bail.
        if node.target_hit_at_status.as_deref() == current_status.as_deref() {
            return;
        }
        let promoted = next_status(current_status.as_deref()).to_string();
        let new_status = if promoted == "None" {
            None
        } else {
            Some(promoted.clone())
        };
        let mut updated = node.clone();
        updated.status = new_status.clone();
        updated.target_hit_at_status = new_status.clone();
        updated.modified_at = chrono::Utc::now();
        if let Err(e) = self
            .store
            .raw()
            .update_metadata(id, updated.to_json())
        {
            tracing::warn!(
                target: "inkhaven::goal_promote",
                "auto-promote update_metadata failed: {e}"
            );
            return;
        }
        let from_label = display_status(current_status.as_deref())
            .to_ascii_lowercase();
        let to_label = promoted.to_ascii_lowercase();
        let book_id = self.book_of_node(id);
        crate::progress::record_status_change(
            id, book_id, &from_label, &to_label, current_words,
        );
        // hook.on_status_promoted fires from both auto-promote
        // and manual-cycle paths; see `cycle_paragraph_status`
        // for the signature and rationale.
        crate::scripting::hooks::fire(
            "hook.on_status_promoted",
            vec![
                rust_dynamic::value::Value::from_string(id.to_string()),
                rust_dynamic::value::Value::from_string(from_label.clone()),
                rust_dynamic::value::Value::from_string(to_label.clone()),
            ],
        );
        self.status = format!(
            "goal-hit: `{}` promoted {} → {}",
            node.title, from_label, to_label
        );
    }

    /// Resolve the user book a paragraph belongs to. Returns
    /// `None` for system-book content (Help / Scripts / Typst /
    /// …) which doesn't count toward writing goals.
    pub(crate) fn book_of_node(&self, id: Uuid) -> Option<Uuid> {
        let node = self.hierarchy.get(id)?;
        let book = self
            .hierarchy
            .ancestors(node)
            .into_iter()
            .find(|a| a.kind == NodeKind::Book)?;
        if book.system_tag.is_some() {
            return None;
        }
        Some(book.id)
    }

    /// Re-read the hierarchy from bdslib and rebuild the flattened tree-row
    /// list, preserving the cursor on the same UUID if it still exists.
    fn reload_hierarchy(&mut self) {
        let prev_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
        match Hierarchy::load(&self.store) {
            Ok(h) => {
                self.hierarchy = h;
                // Prune collapsed-state for nodes that no longer exist.
                self.collapsed_nodes
                    .retain(|id| self.hierarchy.get(*id).is_some());
                self.rows = self
                    .hierarchy
                    .flatten_with_collapsed(&self.collapsed_nodes)
                    .into_iter()
                    .map(|(n, d)| (n.id, d))
                    .collect();
                if let Some(id) = prev_id {
                    if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == id) {
                        self.tree_cursor = i;
                    }
                }
                if !self.rows.is_empty() {
                    self.tree_cursor = self.tree_cursor.min(self.rows.len() - 1);
                }
                // Lexicon depends on Places/Characters paragraph titles, so
                // any hierarchy change is potentially a lexicon change. The
                // recompute is cheap at literary scale (a few hundred names
                // stemmed once per language).
                self.lexicon = build_lexicon(&self.hierarchy, &self.cfg);
            }
            Err(e) => {
                self.status = format!("hierarchy reload failed: {e}");
            }
        }
    }

    // -------- drawing -----------------------------------------------------

    fn draw(&mut self, f: &mut ratatui::Frame) {
        // Typewriter mode: hide every pane except the editor. The
        // editor's own header still shows L/C, word count, edited
        // ago, and the status badge — so the user gets the writing-
        // critical metadata without the surrounding panes. Modals
        // (Quick-ref, Book info, etc.) still float on top, which
        // makes Ctrl+B H usable mid-flow.
        if self.typewriter_mode {
            let area = f.area();
            // Empty pane rects mean the mouse handler skips everything
            // hidden (clicks fall through to the editor's own rect).
            self.layout_search = Rect::default();
            self.layout_tree = Rect::default();
            self.layout_editor = area;
            self.layout_ai = Rect::default();
            self.layout_ai_prompt = Rect::default();
            self.draw_editor(f, area);
            if !matches!(self.modal, Modal::None) {
                self.draw_modal(f, f.area());
            }
            return;
        }

        if self.ai_fullscreen {
            // Layout: most of the screen split 50/50 (AI pane | chat
            // history); AI prompt at the bottom; one status line.
            // Tree, editor, and search bar are hidden.
            let outer = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(0),
                    Constraint::Length(3),
                    Constraint::Length(1),
                ])
                .split(f.area());
            let top = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ])
                .split(outer[0]);
            self.layout_search = Rect::default();
            self.layout_tree = Rect::default();
            self.layout_editor = Rect::default();
            self.layout_ai = top[0];
            self.layout_ai_prompt = outer[1];
            self.draw_ai(f, top[0]);
            self.draw_chat_history(f, top[1]);
            self.draw_ai_prompt(f, outer[1]);
            self.draw_status(f, outer[2]);
            if !matches!(self.modal, Modal::None) {
                self.draw_modal(f, f.area());
            }
            return;
        }

        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(f.area());

        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(22),
                Constraint::Percentage(56),
                Constraint::Percentage(22),
            ])
            .split(outer[1]);

        // Cache pane rects for the mouse handler. Done before drawing so
        // a draw that bails (e.g. tiny terminal) still leaves the rects
        // self-consistent for whatever pane managed to render.
        self.layout_search = outer[0];
        self.layout_tree = body[0];
        self.layout_editor = body[1];
        self.layout_ai = body[2];
        self.layout_ai_prompt = outer[2];

        self.draw_search_bar(f, outer[0]);
        self.draw_tree(f, body[0]);
        if self.secondary.is_some() {
            // Similar-paragraph mode: AI pane is repurposed as
            // the second editor. Both panes carve off the bottom
            // row of their rect for the full slug-path footer
            // the spec asks for.
            let primary_rect = body[1];
            let footer_h: u16 = 1;
            let primary_editor_rect = Rect {
                x: primary_rect.x,
                y: primary_rect.y,
                width: primary_rect.width,
                height: primary_rect.height.saturating_sub(footer_h),
            };
            let primary_footer_rect = Rect {
                x: primary_rect.x,
                y: primary_rect.y + primary_rect.height.saturating_sub(footer_h),
                width: primary_rect.width,
                height: footer_h,
            };
            self.draw_editor(f, primary_editor_rect);
            self.draw_primary_pane_footer(f, primary_footer_rect);
            self.draw_secondary_editor(f, body[2]);
        } else {
            self.draw_editor(f, body[1]);
            self.draw_ai(f, body[2]);
        }
        self.draw_ai_prompt(f, outer[2]);
        self.draw_status(f, outer[3]);

        if self.show_results_overlay {
            self.draw_search_overlay(f, outer[1]);
        }
        if self.show_prompt_picker {
            self.draw_prompt_picker(f, f.area());
        }

        // Modal renders last so it floats over everything.
        if !matches!(self.modal, Modal::None) {
            self.draw_modal(f, f.area());
        }
    }


    /// Render the Ctrl+B V credits panel. Version + author come from
    /// `CARGO_PKG_*` env vars set by cargo at compile time; the component
    /// list is a hand-curated static (kept here so it stays in sync with
    /// what Cargo.toml actually depends on — automating from Cargo.lock
    /// would dump 200+ transitive crates that no user wants to read).
    fn draw_credits_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let engine_summary = crate::typst_compile::engine_summary(&self.cfg);
        let lines = build_credits_lines(&self.theme, &engine_summary);
        let total = lines.len();

        // Pull scroll + logo out of the modal up front. Logo is
        // taken via `&mut` so the StatefulImage widget can update
        // its protocol state during render.
        let Modal::Credits { scroll, logo } = &mut self.modal else {
            return;
        };
        let scroll_value = *scroll;
        let logo_present = logo.is_some();

        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = format!(
            " Inkhaven v{} · author / credits ",
            env!("CARGO_PKG_VERSION")
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        // Layout: optional logo banner (top), scrollable text body
        // (middle), one-row hint (bottom). When the logo is
        // present, give it the smaller of 1/3 of the inner height
        // or 12 rows — enough for the image to read without
        // crowding out the text.
        let footer_h: u16 = 1;
        let logo_h: u16 = if logo_present {
            (inner.height / 3).min(12).max(4).min(inner.height.saturating_sub(footer_h + 4))
        } else {
            0
        };
        let body_h_rows = inner.height.saturating_sub(logo_h + footer_h);

        let logo_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: logo_h,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y + logo_h,
            width: inner.width,
            height: body_h_rows,
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + logo_h + body_h_rows,
            width: inner.width,
            height: footer_h,
        };

        if let Some(proto) = logo.as_mut() {
            if logo_h > 0 {
                let widget = ratatui_image::StatefulImage::new();
                f.render_stateful_widget(widget, logo_rect, proto);
            }
        }

        let body_h = body_rect.height as usize;
        let max_scroll = total.saturating_sub(body_h);
        let scroll_value = scroll_value.min(max_scroll);
        let end = (scroll_value + body_h).min(total);
        let visible: Vec<Line<'_>> = lines[scroll_value..end].to_vec();
        f.render_widget(Paragraph::new(visible), body_rect);

        let at_end = end >= total;
        let more_hint = if at_end { " " } else { " · more below" };
        let hint = format!(
            " ↑↓ / PgUp/PgDn / Home/End scroll · Esc close{more_hint}    (showing {}–{} of {total}) ",
            scroll_value + 1,
            end
        );
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    fn draw_quickref_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
        focus: Focus,
        scroll: usize,
    ) {
        let entries = quickref::entries_for(focus);
        let total = entries.len();

        // Roomy panel — most of the screen with a margin.
        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect {
            x,
            y,
            width,
            height,
        };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = format!(" Quick reference · {} pane ", focus.label());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let body_h = inner.height.saturating_sub(1) as usize;
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };

        // Two columns. Each column gets half the inner width (with a small
        // gap). Entries fill column 1 top-to-bottom, then column 2.
        let col_w = (inner.width / 2) as usize;
        let visible_per_col = body_h;
        let visible_count = (visible_per_col * 2).min(total.saturating_sub(scroll));

        let left_count = visible_count.min(visible_per_col);
        let right_count = visible_count.saturating_sub(left_count);

        let mut left_lines: Vec<Line> = Vec::with_capacity(left_count);
        let mut right_lines: Vec<Line> = Vec::with_capacity(right_count);

        for i in 0..left_count {
            let e = &entries[scroll + i];
            left_lines.push(format_entry_line(e, col_w));
        }
        for i in 0..right_count {
            let e = &entries[scroll + left_count + i];
            right_lines.push(format_entry_line(e, col_w));
        }

        let left_rect = Rect {
            x: body_rect.x,
            y: body_rect.y,
            width: (body_rect.width / 2),
            height: body_rect.height,
        };
        let right_rect = Rect {
            x: body_rect.x + (body_rect.width / 2),
            y: body_rect.y,
            width: body_rect.width - (body_rect.width / 2),
            height: body_rect.height,
        };
        f.render_widget(Paragraph::new(left_lines), left_rect);
        f.render_widget(Paragraph::new(right_lines), right_rect);

        let at_end = scroll + visible_count >= total;
        let more = if at_end { " " } else { " · more below" };
        let hint = format!(
            " ↑↓ / PgUp/PgDn / Home/End scroll · Esc close{more}    (showing {}–{} of {total}) ",
            scroll + 1,
            scroll + visible_count
        );
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    fn draw_file_picker_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
        picker: &FilePicker,
    ) {
        // Roomy panel — most of the screen, leaving a margin on all sides.
        let width = area.width.saturating_sub(8).max(40);
        let height = area.height.saturating_sub(4).max(10);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect {
            x,
            y,
            width,
            height,
        };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = match picker.context {
            PickerContext::EditorLoad => format!(" Load file into editor — {} ", picker.root.display()),
            PickerContext::TreeInsertOrImport => {
                format!(" Import into tree — {} ", picker.root.display())
            }
        };

        // The block reserves 2 rows (borders); a footer hint takes 1 more.
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let list_height = inner.height.saturating_sub(1) as usize;
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let list_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };

        // Scroll: keep cursor in view.
        let mut scroll = 0;
        if picker.cursor >= list_height && list_height > 0 {
            scroll = picker.cursor + 1 - list_height;
        }

        let mut lines: Vec<Line> = Vec::with_capacity(list_height);
        for (i, entry) in picker
            .entries
            .iter()
            .enumerate()
            .skip(scroll)
            .take(list_height)
        {
            let indent = "  ".repeat(entry.depth);
            let glyph = if entry.is_dir {
                if entry.expanded { "▾ 📁 " } else { "▸ 📁 " }
            } else {
                "  📄 "
            };
            let name = entry
                .path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("?")
                .to_string();
            let mut style = if entry.is_dir {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };
            if i == picker.cursor {
                style = style.add_modifier(Modifier::REVERSED);
            }
            lines.push(Line::from(Span::styled(
                format!("{indent}{glyph}{name}"),
                style,
            )));
        }

        f.render_widget(Paragraph::new(lines), list_rect);

        let hint = Line::from(Span::styled(
            " ↑↓ navigate · → expand · ← collapse/parent · Enter pick · Esc cancel ",
            Style::default().add_modifier(Modifier::DIM),
        ));
        f.render_widget(Paragraph::new(hint), footer_rect);
    }

    fn bund_pane_handle_key(&mut self, key: KeyEvent) {
        // Esc closes the pane (top of handle_modal_key already
        // covers it). Here we just handle scrolling.
        let Modal::BundPane { lines, scroll, .. } = &mut self.modal else {
            return;
        };
        let total = lines.len();
        let page: usize = 12; // approximate visible window
        match key.code {
            KeyCode::Up => {
                *scroll = scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                if *scroll + 1 < total {
                    *scroll += 1;
                }
            }
            KeyCode::PageUp => {
                *scroll = scroll.saturating_sub(page);
            }
            KeyCode::PageDown => {
                let max = total.saturating_sub(page.max(1));
                *scroll = (*scroll + page).min(max);
            }
            KeyCode::Home => {
                *scroll = 0;
            }
            KeyCode::End => {
                *scroll = total.saturating_sub(page);
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+C inside the pane clears its buffer — convenient
                // when the pane keeps catching subsequent script output.
                lines.clear();
                *scroll = 0;
            }
            _ => {}
        }
    }

    fn draw_bund_pane_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::BundPane { title, lines, scroll } = &self.modal else {
            return;
        };
        // Roomy panel — same shape as the quickref modal.
        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let total = lines.len();
        let header = format!(" Bund · {} ({} lines) ", title, total);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.tree_script_fg)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1) as usize;
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };

        let visible: Vec<Line<'_>> = lines
            .iter()
            .skip(*scroll)
            .take(body_h)
            .map(|l| Line::from(l.as_str()))
            .collect();
        f.render_widget(Paragraph::new(visible), body_rect);

        let at_end = scroll + body_h >= total;
        let more = if at_end { " " } else { " · more below" };
        let shown_start = scroll + 1;
        let shown_end = (scroll + body_h).min(total);
        let hint = format!(
            " ↑↓ / PgUp/PgDn / Home/End scroll · Ctrl+C clear · Esc close{more}    ({}–{} of {total}) ",
            shown_start.min(total.max(1)),
            shown_end
        );
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    fn script_picker_handle_key(&mut self, key: KeyEvent) {
        // Snapshot fields we need without holding the borrow
        // across `run_script_by_id` (which takes &mut self).
        let (selected_id, total, was_a_toggle, was_enter): (Option<Uuid>, usize, bool, bool) = {
            let Modal::ScriptPicker {
                entries,
                cursor,
                scroll,
                scope,
                ..
            } = &mut self.modal
            else {
                return;
            };
            let total = entries.len();
            let page: usize = 12;
            let mut a_toggle = false;
            let mut enter = false;
            let mut selected: Option<Uuid> = None;
            match key.code {
                KeyCode::Up => {
                    if *cursor > 0 {
                        *cursor -= 1;
                    }
                }
                KeyCode::Down => {
                    if *cursor + 1 < total {
                        *cursor += 1;
                    }
                }
                KeyCode::PageUp => {
                    *cursor = cursor.saturating_sub(page);
                }
                KeyCode::PageDown => {
                    *cursor = (*cursor + page).min(total.saturating_sub(1).max(0));
                }
                KeyCode::Home => *cursor = 0,
                KeyCode::End => *cursor = total.saturating_sub(1),
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    *scope = match scope {
                        ScriptPickerScope::Branch => ScriptPickerScope::ScriptsBook,
                        ScriptPickerScope::ScriptsBook => ScriptPickerScope::Branch,
                    };
                    a_toggle = true;
                }
                KeyCode::Enter => {
                    if let Some(e) = entries.get(*cursor) {
                        selected = Some(e.id);
                    }
                    enter = true;
                }
                _ => {}
            }
            // Keep cursor visible (cheap: clamp scroll).
            if *cursor < *scroll {
                *scroll = *cursor;
            } else if *cursor >= *scroll + page {
                *scroll = *cursor + 1 - page;
            }
            (selected, total, a_toggle, enter)
        };

        if was_a_toggle {
            // Rebuild entries against the new scope.
            let new_scope = match &self.modal {
                Modal::ScriptPicker { scope, .. } => *scope,
                _ => return,
            };
            let new_entries = self.collect_script_entries(new_scope);
            if let Modal::ScriptPicker {
                entries,
                cursor,
                scroll,
                ..
            } = &mut self.modal
            {
                *entries = new_entries;
                *cursor = 0;
                *scroll = 0;
            }
            self.status = match new_scope {
                ScriptPickerScope::Branch => "bund: branch scope".into(),
                ScriptPickerScope::ScriptsBook => "bund: Scripts book scope".into(),
            };
            return;
        }

        if was_enter {
            self.modal = Modal::None;
            if let Some(id) = selected_id {
                if let Err(e) = self.bund_run_script_by_id(id) {
                    self.status = format!("bund: {e}");
                }
            } else if total == 0 {
                self.status = "bund: no script to run".into();
            }
        }
    }

    /// Load the Script node `id`, eval its body against Adam,
    /// and route the result (or error) to the status bar — or
    /// to the Bund pane if one is open.
    fn bund_run_script_by_id(&mut self, id: Uuid) -> std::result::Result<(), String> {
        let node = self
            .hierarchy
            .get(id)
            .ok_or_else(|| format!("script {id} not found"))?
            .clone();
        if node.kind != NodeKind::Script {
            return Err(format!("“{}” is not a Script node", node.title));
        }
        let bytes = self
            .store
            .get_content(node.id)
            .map_err(|e| format!("load {}: {e}", node.title))?
            .unwrap_or_default();
        let body = String::from_utf8(bytes)
            .map_err(|e| format!("script “{}” not utf-8: {e}", node.title))?;
        if body.trim().is_empty() {
            self.status = format!("bund: “{}” is empty", node.title);
            return Ok(());
        }
        match self.scripting_eval(&body) {
            Ok(out) => {
                self.status = format_eval_output(&out, Some(&node.title));
                Ok(())
            }
            Err(e) => Err(format!("eval “{}” failed — {e:#}", node.title)),
        }
    }

    fn draw_script_picker_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::ScriptPicker {
            scope,
            entries,
            cursor,
            scroll,
        } = &self.modal
        else {
            return;
        };
        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let scope_label = match scope {
            ScriptPickerScope::Branch => "current branch",
            ScriptPickerScope::ScriptsBook => "Scripts book",
        };
        let header = format!(" Bund · pick a script ({}) ", scope_label);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.tree_script_fg)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1) as usize;
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };

        let lines: Vec<Line<'_>> = entries
            .iter()
            .enumerate()
            .skip(*scroll)
            .take(body_h)
            .map(|(i, e)| {
                let glyph = "λ ";
                let text = format!(" {glyph}{}    {}", e.title, e.slug_path);
                let mut style = Style::default();
                if i == *cursor {
                    style = style.add_modifier(Modifier::REVERSED);
                }
                Line::from(Span::styled(text, style))
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        let total = entries.len();
        let hint = if total == 0 {
            " (empty) · A toggle scope · Esc close ".to_string()
        } else {
            format!(
                " ↑↓ select · Enter run · A toggle scope · Esc close    ({}/{}) ",
                cursor + 1,
                total
            )
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    fn link_picker_handle_key(&mut self, key: KeyEvent) {
        // Collect intent; close over the modal's mutable state.
        let (owner, target_to_remove, target_to_open) = {
            let Modal::LinkPicker { owner, entries, cursor, scroll } = &mut self.modal else {
                return;
            };
            let total = entries.len();
            let page: usize = 12;
            let mut target_to_remove: Option<Uuid> = None;
            let mut target_to_open: Option<Uuid> = None;
            match key.code {
                KeyCode::Up => {
                    if *cursor > 0 {
                        *cursor -= 1;
                    }
                }
                KeyCode::Down => {
                    if *cursor + 1 < total {
                        *cursor += 1;
                    }
                }
                KeyCode::PageUp => *cursor = cursor.saturating_sub(page),
                KeyCode::PageDown => {
                    *cursor = (*cursor + page).min(total.saturating_sub(1));
                }
                KeyCode::Home => *cursor = 0,
                KeyCode::End => *cursor = total.saturating_sub(1),
                KeyCode::Char('D') | KeyCode::Char('d') | KeyCode::Delete => {
                    if let Some(e) = entries.get(*cursor) {
                        target_to_remove = Some(e.id);
                    }
                }
                // 1.2.4+: Enter opens the linked paragraph in the
                // editor (autosaving the current buffer first via
                // `open_search_result` → `load_paragraph` →
                // `save_current`). Tree cursor follows. Modal closes.
                KeyCode::Enter => {
                    if let Some(e) = entries.get(*cursor) {
                        target_to_open = Some(e.id);
                    }
                }
                _ => {}
            }
            // Keep cursor visible.
            if *cursor < *scroll {
                *scroll = *cursor;
            } else if *cursor >= *scroll + page {
                *scroll = *cursor + 1 - page;
            }
            (*owner, target_to_remove, target_to_open)
        };

        if let Some(target) = target_to_open {
            // Close the modal first so any status message the
            // load flow sets isn't immediately overwritten by a
            // modal-redraw cycle.
            self.modal = Modal::None;
            // `open_search_result` does exactly what we want:
            // moves the tree cursor onto the target row, then
            // loads the paragraph (which autosaves the previous
            // buffer if it was dirty).
            self.open_search_result(target);
            return;
        }

        if let Some(target) = target_to_remove {
            match self.remove_paragraph_link(owner, target) {
                Ok(true) => {
                    self.status = "link removed".into();
                    // Rebuild the modal entries from the fresh
                    // hierarchy; close the modal if no links remain.
                    let entries = self.collect_link_entries(owner);
                    if entries.is_empty() {
                        self.modal = Modal::None;
                    } else if let Modal::LinkPicker { entries: e, cursor, scroll, .. } =
                        &mut self.modal
                    {
                        *cursor = (*cursor).min(entries.len() - 1);
                        *scroll = (*scroll).min(*cursor);
                        *e = entries;
                    }
                }
                Ok(false) => {
                    self.status = "link not found (stale view)".into();
                }
                Err(e) => {
                    self.status = format!("link remove: {e}");
                }
            }
        }
    }

    fn draw_link_picker_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::LinkPicker { entries, cursor, scroll, .. } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = format!(" Linked paragraphs ({}) ", entries.len());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1) as usize;
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };

        let lines: Vec<Line<'_>> = entries
            .iter()
            .enumerate()
            .skip(*scroll)
            .take(body_h)
            .map(|(i, e)| {
                let head = format!(" → {}", e.title);
                let path_dim = format!("    {}", e.slug_path);
                let mut spans: Vec<Span> = vec![
                    Span::raw(head),
                    Span::styled(
                        path_dim,
                        Style::default().add_modifier(Modifier::DIM),
                    ),
                ];
                let mut line = Line::from(std::mem::take(&mut spans));
                if i == *cursor {
                    line = line.style(Style::default().add_modifier(Modifier::REVERSED));
                }
                line
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        let hint = if entries.is_empty() {
            " (empty) · Esc close ".to_string()
        } else {
            format!(
                " ↑↓ select · Enter opens · D removes · Esc closes    ({}/{}) ",
                cursor + 1,
                entries.len()
            )
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    fn fuzzy_paragraph_picker_handle_key(&mut self, key: KeyEvent) {
        let to_open = {
            let Modal::FuzzyParagraphPicker { input, entries, cursor, scroll } =
                &mut self.modal
            else {
                return;
            };
            let matches = fuzzy_filter_entries(entries, input.as_str());
            let total = matches.len();
            let page: usize = 12;
            let mut to_open: Option<Uuid> = None;
            match key.code {
                KeyCode::Up => {
                    if *cursor > 0 {
                        *cursor -= 1;
                    }
                }
                KeyCode::Down => {
                    if *cursor + 1 < total {
                        *cursor += 1;
                    }
                }
                KeyCode::PageUp => *cursor = cursor.saturating_sub(page),
                KeyCode::PageDown => {
                    *cursor = (*cursor + page).min(total.saturating_sub(1));
                }
                KeyCode::Enter => {
                    if let Some(idx) = matches.get(*cursor).copied() {
                        to_open = Some(entries[idx].id);
                    }
                }
                _ => {
                    handle_text_input_key(input, key);
                    // Reset cursor on input edit; matches list
                    // may have shifted.
                    *cursor = 0;
                    *scroll = 0;
                }
            }
            if *cursor < *scroll {
                *scroll = *cursor;
            } else if *cursor >= *scroll + page {
                *scroll = *cursor + 1 - page;
            }
            to_open
        };

        if let Some(id) = to_open {
            self.modal = Modal::None;
            self.open_search_result(id);
        }
    }

    fn draw_fuzzy_paragraph_picker_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::FuzzyParagraphPicker { input, entries, cursor, scroll } = &self.modal
        else {
            return;
        };
        let matches = fuzzy_filter_entries(entries, input.as_str());

        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(14);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = format!(
            " Find paragraph ({}/{}) ",
            matches.len(),
            entries.len()
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        // Top input row, body list, footer hint.
        let input_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: inner.height.saturating_sub(2),
        };

        f.render_widget(
            Paragraph::new(Line::from(format!(
                " › {}",
                input.render_with_cursor('│')
            ))),
            input_rect,
        );

        let body_h = body_rect.height as usize;
        let lines: Vec<Line<'_>> = matches
            .iter()
            .enumerate()
            .skip(*scroll)
            .take(body_h)
            .map(|(i, idx)| {
                let e = &entries[*idx];
                let head = format!(" {}", e.title);
                let path = format!("    {}", e.slug_path);
                let spans: Vec<Span> = vec![
                    Span::raw(head),
                    Span::styled(
                        path,
                        Style::default().add_modifier(Modifier::DIM),
                    ),
                ];
                let mut line = Line::from(spans);
                if i == *cursor {
                    line = line.style(Style::default().add_modifier(Modifier::REVERSED));
                }
                line
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        let hint = " ↑↓ select · Enter opens · Esc closes ";
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    fn bookmark_picker_handle_key(&mut self, key: KeyEvent) {
        let (id_to_unbookmark, id_to_open) = {
            let Modal::BookmarkPicker { entries, cursor, scroll } = &mut self.modal else {
                return;
            };
            let total = entries.len();
            let page: usize = 12;
            let mut to_unbookmark: Option<Uuid> = None;
            let mut to_open: Option<Uuid> = None;
            match key.code {
                KeyCode::Up => {
                    if *cursor > 0 {
                        *cursor -= 1;
                    }
                }
                KeyCode::Down => {
                    if *cursor + 1 < total {
                        *cursor += 1;
                    }
                }
                KeyCode::PageUp => *cursor = cursor.saturating_sub(page),
                KeyCode::PageDown => {
                    *cursor = (*cursor + page).min(total.saturating_sub(1));
                }
                KeyCode::Home => *cursor = 0,
                KeyCode::End => *cursor = total.saturating_sub(1),
                KeyCode::Enter => {
                    if let Some(e) = entries.get(*cursor) {
                        to_open = Some(e.id);
                    }
                }
                KeyCode::Char('D') | KeyCode::Char('d') | KeyCode::Delete => {
                    if let Some(e) = entries.get(*cursor) {
                        to_unbookmark = Some(e.id);
                    }
                }
                _ => {}
            }
            if *cursor < *scroll {
                *scroll = *cursor;
            } else if *cursor >= *scroll + page {
                *scroll = *cursor + 1 - page;
            }
            (to_unbookmark, to_open)
        };

        if let Some(id) = id_to_open {
            self.modal = Modal::None;
            self.open_search_result(id);
            return;
        }

        if let Some(id) = id_to_unbookmark {
            if let Some(node) = self.hierarchy.get(id).cloned() {
                let mut updated = node.clone();
                updated.bookmark = false;
                updated.modified_at = chrono::Utc::now();
                if let Err(e) = self
                    .store
                    .raw()
                    .update_metadata(id, updated.to_json())
                {
                    self.status = format!("bookmark clear: {e}");
                    return;
                }
                self.reload_hierarchy();
                let entries = self.collect_bookmark_entries();
                if entries.is_empty() {
                    self.modal = Modal::None;
                    self.status = "bookmark cleared · no bookmarks left".into();
                } else if let Modal::BookmarkPicker { entries: e, cursor, scroll } =
                    &mut self.modal
                {
                    *cursor = (*cursor).min(entries.len() - 1);
                    *scroll = (*scroll).min(*cursor);
                    *e = entries;
                    self.status = "bookmark cleared".into();
                }
            }
        }
    }

    fn draw_bookmark_picker_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::BookmarkPicker { entries, cursor, scroll } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = format!(" Bookmarks ({}) ", entries.len());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1) as usize;
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };

        let lines: Vec<Line<'_>> = entries
            .iter()
            .enumerate()
            .skip(*scroll)
            .take(body_h)
            .map(|(i, e)| {
                let head = format!(" ★ {}", e.title);
                let path_dim = format!("    {}", e.slug_path);
                let spans: Vec<Span> = vec![
                    Span::raw(head),
                    Span::styled(
                        path_dim,
                        Style::default().add_modifier(Modifier::DIM),
                    ),
                ];
                let mut line = Line::from(spans);
                if i == *cursor {
                    line = line.style(Style::default().add_modifier(Modifier::REVERSED));
                }
                line
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        let hint = if entries.is_empty() {
            " (empty) · Esc close ".to_string()
        } else {
            format!(
                " ↑↓ select · Enter opens · D removes bookmark · Esc closes    ({}/{}) ",
                cursor + 1,
                entries.len()
            )
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    fn backlink_picker_handle_key(&mut self, key: KeyEvent) {
        let (target, source_to_unlink, target_to_open) = {
            let Modal::BacklinkPicker { target, entries, cursor, scroll } = &mut self.modal else {
                return;
            };
            let total = entries.len();
            let page: usize = 12;
            let mut source_to_unlink: Option<Uuid> = None;
            let mut target_to_open: Option<Uuid> = None;
            match key.code {
                KeyCode::Up => {
                    if *cursor > 0 {
                        *cursor -= 1;
                    }
                }
                KeyCode::Down => {
                    if *cursor + 1 < total {
                        *cursor += 1;
                    }
                }
                KeyCode::PageUp => *cursor = cursor.saturating_sub(page),
                KeyCode::PageDown => {
                    *cursor = (*cursor + page).min(total.saturating_sub(1));
                }
                KeyCode::Home => *cursor = 0,
                KeyCode::End => *cursor = total.saturating_sub(1),
                KeyCode::Char('D') | KeyCode::Char('d') | KeyCode::Delete => {
                    if let Some(e) = entries.get(*cursor) {
                        source_to_unlink = Some(e.id);
                    }
                }
                KeyCode::Enter => {
                    if let Some(e) = entries.get(*cursor) {
                        target_to_open = Some(e.id);
                    }
                }
                _ => {}
            }
            if *cursor < *scroll {
                *scroll = *cursor;
            } else if *cursor >= *scroll + page {
                *scroll = *cursor + 1 - page;
            }
            (*target, source_to_unlink, target_to_open)
        };

        if let Some(to_open) = target_to_open {
            self.modal = Modal::None;
            self.open_search_result(to_open);
            return;
        }

        if let Some(source) = source_to_unlink {
            // Remove the source's outgoing link to `target`. That's
            // what makes this the symmetric "delete" for backlinks:
            // the link metadata lives on the source paragraph, so
            // mutating it from the backlinks view is honest about
            // what changes on disk.
            match self.remove_paragraph_link(source, target) {
                Ok(true) => {
                    self.status = "backlink removed (source's outgoing link to current)".into();
                    let entries = self.collect_backlink_entries(target);
                    if entries.is_empty() {
                        self.modal = Modal::None;
                    } else if let Modal::BacklinkPicker { entries: e, cursor, scroll, .. } =
                        &mut self.modal
                    {
                        *cursor = (*cursor).min(entries.len() - 1);
                        *scroll = (*scroll).min(*cursor);
                        *e = entries;
                    }
                }
                Ok(false) => {
                    self.status = "backlink not found (stale view)".into();
                }
                Err(e) => {
                    self.status = format!("backlink remove: {e}");
                }
            }
        }
    }

    fn draw_backlink_picker_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::BacklinkPicker { entries, cursor, scroll, .. } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = format!(" Backlinks ({}) ", entries.len());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1) as usize;
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };

        let lines: Vec<Line<'_>> = entries
            .iter()
            .enumerate()
            .skip(*scroll)
            .take(body_h)
            .map(|(i, e)| {
                // "←" arrow signals incoming direction (vs the
                // "→" used by the outgoing-links picker).
                let head = format!(" ← {}", e.title);
                let path_dim = format!("    {}", e.slug_path);
                let spans: Vec<Span> = vec![
                    Span::raw(head),
                    Span::styled(
                        path_dim,
                        Style::default().add_modifier(Modifier::DIM),
                    ),
                ];
                let mut line = Line::from(spans);
                if i == *cursor {
                    line = line.style(Style::default().add_modifier(Modifier::REVERSED));
                }
                line
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        let hint = if entries.is_empty() {
            " (empty) · Esc close ".to_string()
        } else {
            format!(
                " ↑↓ select · Enter opens · D removes source link · Esc closes    ({}/{}) ",
                cursor + 1,
                entries.len()
            )
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    fn similar_picker_handle_key(&mut self, key: KeyEvent) {
        let (selected_id, total, was_enter) = {
            let Modal::SimilarPicker { entries, cursor, scroll } = &mut self.modal else {
                return;
            };
            let total = entries.len();
            let page: usize = 12;
            let mut enter = false;
            let mut selected: Option<Uuid> = None;
            match key.code {
                KeyCode::Up => {
                    if *cursor > 0 {
                        *cursor -= 1;
                    }
                }
                KeyCode::Down => {
                    if *cursor + 1 < total {
                        *cursor += 1;
                    }
                }
                KeyCode::PageUp => *cursor = cursor.saturating_sub(page),
                KeyCode::PageDown => {
                    *cursor = (*cursor + page).min(total.saturating_sub(1).max(0));
                }
                KeyCode::Home => *cursor = 0,
                KeyCode::End => *cursor = total.saturating_sub(1),
                KeyCode::Enter => {
                    if let Some(e) = entries.get(*cursor) {
                        selected = Some(e.id);
                    }
                    enter = true;
                }
                _ => {}
            }
            if *cursor < *scroll {
                *scroll = *cursor;
            } else if *cursor >= *scroll + page {
                *scroll = *cursor + 1 - page;
            }
            (selected, total, enter)
        };

        if was_enter {
            self.modal = Modal::None;
            if let Some(id) = selected_id {
                if let Err(e) = self.load_secondary_paragraph(id) {
                    self.status = format!("similar: {e}");
                }
            } else if total == 0 {
                self.status = "similar: nothing to open".into();
            }
        }
    }

    /// Materialise the picked paragraph as a `secondary` OpenedDoc
    /// rendered in the right pane (replacing AI while in similar
    /// mode). Mirrors `load_paragraph`'s body construction; cursor
    /// memory is honoured so re-opening lands where the user left
    /// it (consistent with primary-pane behaviour).
    fn load_secondary_paragraph(
        &mut self,
        id: Uuid,
    ) -> std::result::Result<(), String> {
        let node = self
            .hierarchy
            .get(id)
            .cloned()
            .ok_or_else(|| format!("paragraph {id} not in hierarchy"))?;
        if node.kind != NodeKind::Paragraph {
            return Err(format!("`{}` is not a paragraph", node.title));
        }
        let rel = node
            .file
            .as_ref()
            .ok_or_else(|| format!("paragraph `{}` has no file on disk", node.title))?;
        let abs = self.layout.root.join(rel);
        let body = std::fs::read_to_string(&abs)
            .map_err(|e| format!("read {}: {e}", abs.display()))?;
        let lines = body_to_lines(&body);
        let saved_lines = lines.clone();
        let mut textarea = TextArea::new(lines);
        textarea.set_cursor_line_style(Style::default().add_modifier(Modifier::REVERSED));
        textarea.set_line_number_style(Style::default().fg(Color::DarkGray));
        let read_only = self.hierarchy.ancestors(&node).iter().any(|a| {
            a.protected && a.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_HELP)
        });
        let saved_cursor = self.paragraph_cursors.get(&node.id).copied();
        let (init_row, init_col, init_scroll_row, init_scroll_col) = match saved_cursor {
            Some(pc) => {
                let max_row = textarea.lines().len().saturating_sub(1);
                let row = pc.cursor_row.min(max_row);
                let line_len = textarea
                    .lines()
                    .get(row)
                    .map_or(0, |s| s.chars().count());
                let col = pc.cursor_col.min(line_len);
                (row, col, pc.scroll_row.min(max_row), pc.scroll_col)
            }
            None => (0, 0, 0, 0),
        };
        if init_row > 0 || init_col > 0 {
            textarea.move_cursor(CursorMove::Jump(init_row as u16, init_col as u16));
        }
        self.secondary = Some(OpenedDoc {
            id: node.id,
            title: node.title.clone(),
            rel_path: rel.clone(),
            textarea,
            dirty: false,
            scroll_row: init_scroll_row,
            scroll_col: init_scroll_col,
            block_anchor: None,
            last_activity: std::time::Instant::now(),
            saved_lines,
            split: None,
            search: None,
            read_only,
            correction_baseline: None,
            content_type: node.content_type.clone(),
            typst_diagnostics: Vec::new(),
            typst_diagnostics_checked_at: std::time::Instant::now(),
        });
        self.secondary_focused = false;
        self.status = format!(
            "similar: `{}` opened side-by-side (Tab swaps focus · Ctrl+V S exits)",
            node.title
        );
        Ok(())
    }

    fn draw_similar_picker_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::SimilarPicker { entries, cursor, scroll } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = format!(" Similar paragraphs ({} hits) ", entries.len());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1) as usize;
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };

        let lines: Vec<Line<'_>> = entries
            .iter()
            .enumerate()
            .skip(*scroll)
            .take(body_h)
            .map(|(i, e)| {
                let score_pct = (e.score * 100.0).round() as i64;
                let head = format!(" {:>3}%  {}", score_pct, e.title);
                let path_dim = format!("    {}", e.slug_path);
                let snippet_dim = if e.snippet.is_empty() {
                    String::new()
                } else {
                    format!("    {}", e.snippet)
                };
                let mut spans: Vec<Span> = vec![
                    Span::raw(head),
                    Span::raw("   "),
                    Span::styled(path_dim, Style::default().add_modifier(Modifier::DIM)),
                ];
                if !snippet_dim.is_empty() {
                    spans.push(Span::raw("  · "));
                    spans.push(Span::styled(
                        snippet_dim,
                        Style::default().add_modifier(Modifier::DIM),
                    ));
                }
                let mut line = Line::from(spans);
                if i == *cursor {
                    line = line.style(Style::default().add_modifier(Modifier::REVERSED));
                }
                line
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        let hint = if entries.is_empty() {
            " (empty) · Esc close ".to_string()
        } else {
            format!(
                " ↑↓ select · Enter open side-by-side · Esc cancel    ({}/{}) ",
                cursor + 1,
                entries.len()
            )
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// Open the per-paragraph goal-setting modal. Pre-fills the
    /// input box with the current `target_words` (if any) so
    /// editing a goal is one keystroke; empty / `0` on Enter
    /// clears the goal.
    fn open_paragraph_target_modal(&mut self) {
        // 1.2.4+: when the tree has multi-select active, the
        // modal opens for ALL marked paragraphs and the commit
        // applies the same target to each. Prefill is empty
        // (no single "current" to display across a set).
        if !self.tree_marked.is_empty() {
            self.modal = Modal::ParagraphTarget {
                input: TextInput::new(),
            };
            self.status = format!(
                "paragraph target × {}: type a number, Enter sets all, Esc cancels",
                self.tree_marked.len()
            );
            return;
        }
        let Some(doc) = self.opened.as_ref() else {
            self.status = "view T: no paragraph open".into();
            return;
        };
        let current = self
            .hierarchy
            .get(doc.id)
            .and_then(|n| n.target_words)
            .filter(|n| *n > 0);
        let mut input = TextInput::new();
        if let Some(n) = current {
            for c in n.to_string().chars() {
                input.insert_char(c);
            }
        }
        self.modal = Modal::ParagraphTarget { input };
        self.status =
            "paragraph target: type a number, Enter to set, empty/0 to clear, Esc to cancel"
                .into();
    }

    /// Commit `raw` as the open paragraph's `target_words`. Empty
    /// or `"0"` clears the goal (sets to None). Non-numeric input
    /// surfaces an error and leaves the existing value untouched.
    ///
    /// 1.2.4+: when `tree_marked` is non-empty, the same target
    /// is applied to every marked paragraph instead of the open
    /// one.
    fn commit_paragraph_target(&mut self, raw: &str) {
        let new_target: Option<i32> = if raw.is_empty() || raw == "0" {
            None
        } else {
            match raw.parse::<i32>() {
                Ok(n) if n > 0 => Some(n),
                Ok(_) => {
                    self.status = "view T: target must be > 0".into();
                    return;
                }
                Err(_) => {
                    self.status = format!("view T: `{raw}` is not a number");
                    return;
                }
            }
        };
        // Multi-select path: apply the same target to every
        // marked paragraph.
        if !self.tree_marked.is_empty() {
            let ids: Vec<Uuid> = self.tree_marked.iter().copied().collect();
            let mut ok = 0usize;
            let mut fail = 0usize;
            for id in &ids {
                if self.set_paragraph_target_now(*id, new_target).is_ok() {
                    ok += 1;
                } else {
                    fail += 1;
                }
            }
            self.status = match new_target {
                Some(n) => format!(
                    "target {n} set on {ok} paragraph(s){}",
                    if fail > 0 { format!(" · {fail} failed") } else { String::new() }
                ),
                None => format!(
                    "target cleared on {ok} paragraph(s){}",
                    if fail > 0 { format!(" · {fail} failed") } else { String::new() }
                ),
            };
            return;
        }
        let Some(doc) = self.opened.as_ref() else {
            self.status = "view T: paragraph closed during input".into();
            return;
        };
        let id = doc.id;
        match self.set_paragraph_target_now(id, new_target) {
            Ok(()) => {
                self.status = match new_target {
                    Some(n) => format!("paragraph target: {} words", n),
                    None => "paragraph target: cleared".into(),
                };
            }
            Err(e) => self.status = format!("view T: {e}"),
        }
    }

    /// Mutate `target_words` on the node `id` and persist via
    /// `store.raw().update_metadata`. Used by both the Ctrl+V T
    /// modal and the `ink.paragraph.set_target` Bund word.
    /// Setting target to None also clears `target_hit_at_status`
    /// so re-enabling the goal starts fresh.
    pub(crate) fn set_paragraph_target_now(
        &mut self,
        id: Uuid,
        target: Option<i32>,
    ) -> std::result::Result<(), String> {
        let node = self
            .hierarchy
            .get(id)
            .ok_or_else(|| format!("paragraph {id} not in hierarchy"))?;
        if node.kind != NodeKind::Paragraph {
            return Err(format!("`{}` is not a paragraph", node.title));
        }
        let mut updated = node.clone();
        updated.target_words = target;
        if target.is_none() {
            updated.target_hit_at_status = None;
        }
        self.store
            .raw()
            .update_metadata(id, updated.to_json())
            .map_err(|e| format!("store update: {e}"))?;
        self.reload_hierarchy();
        Ok(())
    }

    /// Open the writing-progress modal. Forces a cache refresh
    /// so the user always sees fresh numbers (the per-redraw
    /// path stays cheap by reading the cache).
    fn open_progress_modal(&mut self) {
        self.refresh_progress_cache();
        self.modal = Modal::Progress { scroll: 0 };
        self.status = "progress: ↑↓ scroll · Esc close · r refresh".into();
    }

    fn progress_modal_handle_key(&mut self, key: KeyEvent) {
        let Modal::Progress { scroll } = &mut self.modal else {
            return;
        };
        match key.code {
            KeyCode::Up => *scroll = scroll.saturating_sub(1),
            KeyCode::Down => *scroll += 1,
            KeyCode::PageUp => *scroll = scroll.saturating_sub(8),
            KeyCode::PageDown => *scroll += 8,
            KeyCode::Home => *scroll = 0,
            KeyCode::End => *scroll += 100, // clamped by renderer
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.refresh_progress_cache();
                self.status = "progress: refreshed".into();
            }
            _ => {}
        }
    }

    fn draw_progress_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let scroll = match &self.modal {
            Modal::Progress { scroll } => *scroll,
            _ => return,
        };
        let snap = match self.progress_cache.as_ref() {
            Some(s) => s.clone(),
            None => {
                self.refresh_progress_cache();
                self.progress_cache.clone().unwrap_or_else(|| {
                    crate::progress::ProgressSnapshot::empty()
                })
            }
        };

        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(20);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = " Writing progress ".to_string();
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        // Two-column body: text on left (2/3), 30-day sparkline
        // + bar chart on right (1/3). Footer row.
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(60),
                Constraint::Percentage(40),
            ])
            .split(body_rect);
        let text_rect = split[0];
        let chart_rect = split[1];

        // ── Text panel ────────────────────────────────────────
        let mut lines: Vec<Line> = Vec::new();
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let dim = Style::default().add_modifier(Modifier::DIM);

        // Today + streak
        lines.push(Line::from(Span::styled(" Today", bold)));
        let today_line = match snap.project.daily_goal {
            Some(goal) => {
                let pct = if goal > 0 {
                    (snap.project.today_words.max(0) * 100 / goal).clamp(0, 999)
                } else {
                    0
                };
                format!(
                    "   words: {}/{} ({}%)",
                    snap.project.today_words, goal, pct
                )
            }
            None => format!("   words: {} (no daily goal set)", snap.project.today_words),
        };
        lines.push(Line::from(today_line));
        lines.push(Line::from(format!(
            "   streak: {}d (grace {}/{} per week)",
            snap.streak.days, snap.streak.grace_used, snap.streak.grace_per_week
        )));
        lines.push(Line::from(format!(
            "   active: {} today · {} this week",
            format_active_duration(snap.active_seconds_today),
            format_active_duration(snap.active_seconds_week),
        )));
        lines.push(Line::from(""));

        // Per-book breakdown
        lines.push(Line::from(Span::styled(" Books", bold)));
        if snap.books.is_empty() {
            lines.push(Line::from(Span::styled(
                "   (no user books)",
                dim,
            )));
        }
        for b in &snap.books {
            let header = match (b.target_words, b.required_pace, b.days_to_deadline) {
                (Some(t), Some(p), Some(dd)) => format!(
                    "   {}: {}w · target {}w · pace {}w/d · {} day(s)",
                    b.label, b.total_words, t, p, dd
                ),
                (Some(t), _, _) => {
                    format!("   {}: {}w · target {}w", b.label, b.total_words, t)
                }
                _ => format!("   {}: {}w", b.label, b.total_words),
            };
            lines.push(Line::from(header));
            lines.push(Line::from(Span::styled(
                format!("      today: {}w", b.today_words),
                dim,
            )));
        }
        lines.push(Line::from(""));

        // Status ladder
        lines.push(Line::from(Span::styled(
            " Status ladder · last 7 days",
            bold,
        )));
        if snap.status.recent.is_empty() && snap.status.goals.is_empty() {
            lines.push(Line::from(Span::styled(
                "   (no status promotions recorded yet)",
                dim,
            )));
        } else {
            // Display each goal alongside its recent count.
            let mut by_status: std::collections::HashMap<String, i64> =
                std::collections::HashMap::new();
            for (s, n) in &snap.status.recent {
                by_status.insert(s.clone(), *n);
            }
            let mut shown: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for (s, goal) in &snap.status.goals {
                let n = by_status.get(s).copied().unwrap_or(0);
                lines.push(Line::from(format!(
                    "   → {}: {}/{} this week",
                    s, n, goal
                )));
                shown.insert(s.clone());
            }
            for (s, n) in &snap.status.recent {
                if shown.contains(s) {
                    continue;
                }
                lines.push(Line::from(format!("   → {}: {}", s, n)));
            }
        }

        // Apply scroll. The renderer truncates after the visible
        // height; out-of-range scroll is clamped here so End +
        // PageDown saturate at "show the bottom".
        let total = lines.len();
        let body_h = text_rect.height as usize;
        let max_scroll = total.saturating_sub(body_h.max(1));
        let scroll = scroll.min(max_scroll);
        let visible: Vec<Line> = lines.into_iter().skip(scroll).take(body_h).collect();
        f.render_widget(Paragraph::new(visible), text_rect);

        // ── Chart column ───────────────────────────────────────
        // Top half: 30-day daily-words sparkline.
        // Bottom half: per-book progress bar chart (current %
        // of target, capped at 100 for the bar height; bars
        // can overshoot in the label).
        let chart_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(chart_rect);
        let sparkline_rect = chart_split[0];
        let bars_rect = chart_split[1];

        let data: Vec<u64> = snap
            .sparkline
            .iter()
            .map(|n| (*n).max(0) as u64)
            .collect();
        if !data.is_empty() && sparkline_rect.height > 4 {
            let sparkline = ratatui::widgets::Sparkline::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" 30d words/day "),
                )
                .data(&data)
                .style(Style::default().fg(self.theme.tree_script_fg));
            f.render_widget(sparkline, sparkline_rect);
        } else {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    " (not enough history)",
                    dim,
                )))
                .block(Block::default().borders(Borders::ALL).title(" 30d ")),
                sparkline_rect,
            );
        }

        // Per-book BarChart (1.2.4+). Each user book with a
        // target shows one bar = pct of target, capped at 100.
        // The labels are short slugs so multiple books fit in
        // the narrow chart column.
        let book_bars: Vec<(String, u64)> = snap
            .books
            .iter()
            .filter_map(|b| {
                let target = b.target_words?;
                if target <= 0 {
                    return None;
                }
                let pct = (b.total_words.max(0) * 100 / target).clamp(0, 100) as u64;
                // Slugify the label so a wide book title doesn't
                // truncate the bar.
                let label = slug::slugify(&b.label);
                Some((label, pct))
            })
            .collect();
        if !book_bars.is_empty() && bars_rect.height > 4 {
            let data: Vec<(&str, u64)> =
                book_bars.iter().map(|(s, n)| (s.as_str(), *n)).collect();
            let max_label_w = data
                .iter()
                .map(|(s, _)| s.len())
                .max()
                .unwrap_or(8)
                .max(6) as u16;
            let bar_chart = ratatui::widgets::BarChart::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" books: % of target "),
                )
                .data(&data)
                .max(100)
                .bar_width(max_label_w)
                .bar_gap(1)
                .bar_style(Style::default().fg(self.theme.tree_script_fg))
                .value_style(
                    Style::default()
                        .fg(self.theme.modal_fg)
                        .add_modifier(Modifier::BOLD),
                );
            f.render_widget(bar_chart, bars_rect);
        } else {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    " (no per-book targets set)",
                    dim,
                )))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" books "),
                ),
                bars_rect,
            );
        }

        // ── Footer ─────────────────────────────────────────────
        let hint = " ↑↓ / PgUp/PgDn scroll · r refresh · Esc close ";
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(hint, dim))),
            footer_rect,
        );
    }

    fn draw_snapshot_diff_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let (paragraph_title, when, rows, scroll) = match &self.modal {
            Modal::SnapshotDiff {
                paragraph_title,
                when,
                rows,
                scroll,
                ..
            } => (
                paragraph_title.clone(),
                when.clone(),
                rows.clone(),
                *scroll,
            ),
            _ => return,
        };

        // Roomy modal — almost full screen so wide lines fit.
        let width = area.width.saturating_sub(4).max(80);
        let height = area.height.saturating_sub(2).max(20);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header =
            format!(" Diff · `{paragraph_title}` · snapshot {when} → current ");
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        // Footer.
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };
        // Split body into two columns: snapshot (left) | current (right).
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(body_rect);
        let left_rect = split[0];
        let right_rect = split[1];

        let body_h = left_rect.height as usize;
        let visible: Vec<&SnapshotDiffRow> =
            rows.iter().skip(scroll).take(body_h).collect();

        let mut left_lines: Vec<Line<'static>> = Vec::with_capacity(visible.len());
        let mut right_lines: Vec<Line<'static>> = Vec::with_capacity(visible.len());

        let removed_style = Style::default()
            .fg(Color::Red)
            .add_modifier(Modifier::BOLD);
        let added_style = Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD);
        let changed_style = Style::default().fg(Color::Yellow);
        let dim = Style::default().add_modifier(Modifier::DIM);

        for row in visible {
            let (l_marker, r_marker, l_style, r_style) = match row.kind {
                SnapshotDiffKind::Equal => (" ", " ", dim, dim),
                SnapshotDiffKind::Removed => ("-", " ", removed_style, dim),
                SnapshotDiffKind::Added => (" ", "+", dim, added_style),
                SnapshotDiffKind::Changed => ("~", "~", changed_style, changed_style),
            };
            let left_text = row.left.clone().unwrap_or_default();
            let right_text = row.right.clone().unwrap_or_default();
            left_lines.push(Line::from(Span::styled(
                format!("{l_marker} {left_text}"),
                l_style,
            )));
            right_lines.push(Line::from(Span::styled(
                format!("{r_marker} {right_text}"),
                r_style,
            )));
        }

        f.render_widget(Paragraph::new(left_lines), left_rect);
        f.render_widget(Paragraph::new(right_lines), right_rect);

        let hint = format!(
            " ↑↓ / PgUp/PgDn / Home/End scroll · Esc back ({}/{}) ",
            scroll + 1,
            rows.len().max(1)
        );
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    fn draw_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        // The file picker needs a much larger panel than the fixed
        // 80-wide / 8-high box used for confirms — give it its own renderer.
        if let Modal::FilePicker(picker) = &self.modal {
            self.draw_file_picker_modal(f, area, picker);
            return;
        }
        if let Modal::QuickRef { focus, scroll } = &self.modal {
            self.draw_quickref_modal(f, area, *focus, *scroll);
            return;
        }
        if matches!(self.modal, Modal::Credits { .. }) {
            self.draw_credits_modal(f, area);
            return;
        }
        if let Modal::BookInfo { scroll } = &self.modal {
            self.draw_book_info_modal(f, area, *scroll);
            return;
        }
        if let Modal::LlmPicker { .. } = &self.modal {
            self.draw_llm_picker_modal(f, area);
            return;
        }
        if let Modal::ImagePicker { .. } = &self.modal {
            self.draw_image_picker_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::ImagePreview { .. }) {
            self.draw_image_preview_modal(f, area);
            return;
        }
        if let Modal::FunctionPicker { .. } = &self.modal {
            self.draw_function_picker_modal(f, area);
            return;
        }
        if let Modal::StatusFilter { .. } = &self.modal {
            self.draw_status_filter_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::BundPane { .. }) {
            self.draw_bund_pane_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::ScriptPicker { .. }) {
            self.draw_script_picker_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::SimilarPicker { .. }) {
            self.draw_similar_picker_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::Progress { .. }) {
            self.draw_progress_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::SnapshotDiff { .. }) {
            self.draw_snapshot_diff_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::LinkPicker { .. }) {
            self.draw_link_picker_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::BacklinkPicker { .. }) {
            self.draw_backlink_picker_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::BookmarkPicker { .. }) {
            self.draw_bookmark_picker_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::FuzzyParagraphPicker { .. }) {
            self.draw_fuzzy_paragraph_picker_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::RenderedPreview { .. }) {
            self.draw_rendered_preview_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::SaveRenderedPng { .. }) {
            self.draw_save_rendered_png_modal(f, area);
            return;
        }

        let width = area.width.saturating_sub(8).clamp(30, 80);
        let height: u16 = 8;
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let (title, border_color, body): (String, Color, Vec<Line<'_>>) = match &self.modal {
            Modal::None => return,
            Modal::FilePicker(_) => unreachable!("file picker handled above"),
            Modal::QuickRef { .. } => unreachable!("quickref handled above"),
            Modal::Credits { .. } => unreachable!("credits handled above"),
            Modal::BookInfo { .. } => unreachable!("book info handled above"),
            Modal::LlmPicker { .. } => unreachable!("llm picker handled above"),
            Modal::ImagePicker { .. } => unreachable!("image picker handled above"),
            Modal::ImagePreview { .. } => unreachable!("image preview handled above"),
            Modal::FunctionPicker { .. } => unreachable!("function picker handled above"),
            Modal::StatusFilter { .. } => unreachable!("status filter handled above"),
            Modal::BundPane { .. } => unreachable!("bund pane handled above"),
            Modal::ScriptPicker { .. } => unreachable!("script picker handled above"),
            Modal::SimilarPicker { .. } => unreachable!("similar picker handled above"),
            Modal::Progress { .. } => unreachable!("progress modal handled above"),
            Modal::SnapshotDiff { .. } => unreachable!("snapshot diff handled above"),
            Modal::LinkPicker { .. } => unreachable!("link picker handled above"),
            Modal::BacklinkPicker { .. } => unreachable!("backlink picker handled above"),
            Modal::BookmarkPicker { .. } => unreachable!("bookmark picker handled above"),
            Modal::FuzzyParagraphPicker { .. } =>
                unreachable!("fuzzy paragraph picker handled above"),
            Modal::RenderedPreview { .. } =>
                unreachable!("rendered preview handled above"),
            Modal::SaveRenderedPng { .. } =>
                unreachable!("save rendered png handled above"),
            Modal::ParagraphTarget { input } => {
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        " Paragraph word-count target:",
                        Style::default()
                            .fg(self.theme.tree_script_fg)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!(" › {}", input.render_with_cursor('│'))),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Enter sets · empty/0 clears · Esc cancels",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (
                    " Per-paragraph goal — Ctrl+V T ".to_string(),
                    self.theme.tree_script_fg,
                    body,
                )
            }
            Modal::SaveMarkdown { input, label, .. } => {
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!(" Save markdown of `{label}` to:"),
                        Style::default()
                            .fg(self.theme.tree_script_fg)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!(" › {}", input.render_with_cursor('│'))),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Enter writes (default path pre-filled) · Esc cancels",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (
                    " Save markdown — Ctrl+V ".to_string(),
                    self.theme.tree_script_fg,
                    body,
                )
            }
            Modal::HelpQuery { input } => {
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        " Ask the Help book:",
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!(" › {}", input.render_with_cursor('│'))),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Enter to ask · Esc to cancel · answer streams into the AI pane",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (" Help — F1 ".to_string(), Color::Cyan, body)
            }
            Modal::BundInput { prompt, input, hook } => {
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!(" {prompt}"),
                        Style::default()
                            .fg(self.theme.tree_script_fg)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!(" › {}", input.render_with_cursor('│'))),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!(
                            "  Enter fires hook `{hook}` with your input · Esc cancels"
                        ),
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (
                    " Bund — ink.input ".to_string(),
                    self.theme.tree_script_fg,
                    body,
                )
            }
            Modal::BundEval { input } => {
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        " Bund — evaluate one expression against Adam:",
                        Style::default()
                            .fg(self.theme.tree_script_fg)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!(" › {}", input.render_with_cursor('│'))),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Enter runs · Esc cancels · result lands on the status bar",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (
                    " Bund — Ctrl+Z E ".to_string(),
                    self.theme.tree_script_fg,
                    body,
                )
            }
            Modal::ChatSearchPrompt { input } => {
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        " Search chat history:",
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!(" › {}", input.render_with_cursor('│'))),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Enter starts from the newest match · Ctrl+X advances to older · Esc cancels",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (" Chat search — Ctrl+F ".to_string(), Color::Cyan, body)
            }
            Modal::FindReplace {
                search_input,
                replace_input,
                focus_replace,
            } => {
                let cursor_char = '│';
                let search_marker = if *focus_replace { " " } else { ">" };
                let replace_marker = if *focus_replace { ">" } else { " " };
                let mut body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!(
                            " {} Search:  {}",
                            search_marker,
                            search_input.render_with_cursor(cursor_char)
                        ),
                        if !*focus_replace {
                            Style::default()
                        } else {
                            Style::default().add_modifier(Modifier::DIM)
                        },
                    )),
                ];
                if let Some(r) = replace_input {
                    body.push(Line::from(Span::styled(
                        format!(
                            " {} Replace: {}",
                            replace_marker,
                            r.render_with_cursor(cursor_char)
                        ),
                        if *focus_replace {
                            Style::default()
                        } else {
                            Style::default().add_modifier(Modifier::DIM)
                        },
                    )));
                }
                body.push(Line::from(""));
                let hint = if replace_input.is_some() {
                    " Enter run · Tab switch field · Esc cancel "
                } else {
                    " Enter find · Esc cancel "
                };
                body.push(Line::from(Span::styled(
                    hint,
                    Style::default().add_modifier(Modifier::DIM),
                )));
                let header = if replace_input.is_some() {
                    " Find & Replace (regex) "
                } else {
                    " Find (regex) "
                };
                (header.into(), Color::Magenta, body)
            }
            Modal::Adding {
                kind,
                parent_label,
                input,
                position,
                ..
            } => {
                let header = match position {
                    InsertPosition::End => format!(" Add {} ", kind.as_str()),
                    InsertPosition::After(_) => {
                        format!(" Insert {} after current ", kind.as_str())
                    }
                    InsertPosition::Before(_) => {
                        format!(" Insert {} before anchor ", kind.as_str())
                    }
                };
                let parent = format!(" Parent: {}", parent_label);
                let where_line = match position {
                    InsertPosition::End => "    Where: append at end".to_string(),
                    InsertPosition::After(anchor_id) => {
                        let anchor_name = self
                            .hierarchy
                            .get(*anchor_id)
                            .map(|n| n.title.clone())
                            .unwrap_or_else(|| "<gone>".to_string());
                        format!("    Where: after `{anchor_name}`")
                    }
                    InsertPosition::Before(anchor_id) => {
                        let anchor_name = self
                            .hierarchy
                            .get(*anchor_id)
                            .map(|n| n.title.clone())
                            .unwrap_or_else(|| "<gone>".to_string());
                        format!("    Where: before `{anchor_name}`")
                    }
                };
                let title_line = format!(" Title : {}", input.render_with_cursor('│'));
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(parent, Style::default().add_modifier(Modifier::DIM))),
                    Line::from(Span::styled(
                        where_line,
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                    Line::from(title_line),
                    Line::from(Span::styled(
                        " Enter to confirm · Esc to cancel ",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (header, Color::Green, body)
            }
            Modal::Deleting {
                root_kind,
                title,
                descendant_count,
                ..
            } => {
                let prompt = if *descendant_count > 0 {
                    format!(
                        " Delete {} `{}` and {} descendant{}?",
                        root_kind.as_str(),
                        title,
                        descendant_count,
                        if *descendant_count == 1 { "" } else { "s" }
                    )
                } else {
                    format!(" Delete {} `{}`?", root_kind.as_str(), title)
                };
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        prompt,
                        Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        " Removes files from disk AND records from the store.",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                    Line::from(Span::styled(
                        " y / Enter to confirm · n / Esc to cancel ",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (" Confirm delete ".into(), Color::Red, body)
            }
            Modal::Renaming { kind, input, .. } => {
                let header = format!(" Rename {} ", kind.as_str());
                let title_line = format!(" Title : {}", input.render_with_cursor('│'));
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("    Renaming a {} — its slug and filesystem entry don't change.", kind.as_str()),
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                    Line::from(""),
                    Line::from(title_line),
                    Line::from(""),
                    Line::from(Span::styled(
                        " Enter to confirm · Esc to cancel ",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (header, Color::Blue, body)
            }
            Modal::SnapshotPicker {
                paragraph_title,
                snapshots,
                cursor,
                ..
            } => {
                let header = format!(" Snapshots — {} ", paragraph_title);
                let mut body: Vec<Line> = Vec::with_capacity(snapshots.len() + 2);
                body.push(Line::from(""));
                for (i, snap) in snapshots.iter().enumerate() {
                    let selected = i == *cursor;
                    let ts = snap
                        .created_at
                        .with_timezone(&chrono::Local)
                        .format("%Y-%m-%d %H:%M:%S %z");
                    let head = format!(
                        " {ts}   {}w   {}",
                        snap.word_count,
                        if snap.preview.is_empty() {
                            "(no body yet)"
                        } else {
                            snap.preview.as_str()
                        }
                    );
                    let style = if selected {
                        Style::default()
                            .add_modifier(Modifier::REVERSED | Modifier::BOLD)
                            .fg(Color::Cyan)
                    } else {
                        Style::default()
                    };
                    body.push(Line::from(Span::styled(head, style)));
                }
                body.push(Line::from(""));
                body.push(Line::from(Span::styled(
                    " ↑↓ navigate · Enter loads · V diff vs current · D / Del delete · Esc cancel ",
                    Style::default().add_modifier(Modifier::DIM),
                )));
                (header, Color::Cyan, body)
            }
        };

        // Generic confirms (Adding / Deleting / FindReplace / Renaming /
        // SnapshotPicker) all share this final render. Theme drives the
        // modal background + foreground; per-modal accent colour is the
        // border (passed in via `border_color`).
        f.render_widget(
            Paragraph::new(body).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .style(
                        Style::default()
                            .bg(self.theme.modal_bg)
                            .fg(self.theme.modal_fg),
                    ),
            ),
            rect,
        );
    }

    fn pane_block<'a>(&self, title: &'a str, focus: Focus) -> Block<'a> {
        self.pane_block_line(Line::from(title), focus)
    }

    /// Same as `pane_block` but accepts a pre-built styled `Line` as the
    /// title — used by the AI pane to colourise the `scope=` / `infer=`
    /// mode chips.
    fn pane_block_line<'a>(&self, title: Line<'a>, focus: Focus) -> Block<'a> {
        let border_color = if self.focus == focus {
            self.theme.border_focused
        } else {
            self.theme.border_unfocused
        };
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(
                Style::default()
                    .fg(border_color)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.pane_bg)
                    .fg(self.theme.pane_fg),
            )
    }

    /// Editor pane block. Border colour carries the document's clean/dirty
    /// state when the pane has focus: green when saved, yellow when dirty,
    /// teal when the open paragraph is read-only (Help subtree). Unfocused
    /// uses the theme's neutral border colour.
    #[allow(dead_code)] // string-title convenience; the live renderer
    // uses `editor_block_line` for the coloured cursor read-out, but the
    // string overload stays available for non-styled callers.
    fn editor_block<'a>(&self, title: &'a str) -> Block<'a> {
        self.editor_block_line(Line::from(title))
    }

    /// Variant of `editor_block` that takes a pre-built styled `Line` for
    /// the title. Lets the renderer mix theme colours into the header
    /// (used for the `L… C…` cursor read-out chip).
    fn editor_block_line<'a>(&self, title: Line<'a>) -> Block<'a> {
        let border_color = if self.focus == Focus::Editor {
            let dirty = self.opened.as_ref().is_some_and(|d| d.dirty);
            let ro = self.opened.as_ref().is_some_and(|d| d.read_only);
            if ro {
                self.theme.border_readonly
            } else if dirty {
                self.theme.border_dirty
            } else {
                self.theme.border_saved
            }
        } else {
            self.theme.border_unfocused
        };
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(
                Style::default()
                    .fg(border_color)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.pane_bg)
                    .fg(self.theme.pane_fg),
            )
    }

    /// Centralized focus change. When leaving the Editor pane, autosave the
    /// open paragraph (best-effort) so unsaved edits are persisted even
    /// across Tab cycles, Ctrl+1..5 jumps, or any other focus shift. The
    /// underlying `save_current` writes to disk + bdslib + re-embeds.
    fn change_focus(&mut self, new: Focus) {
        if self.focus == Focus::Editor && new != Focus::Editor {
            // Focus-out also counts as "I'm done reviewing the
            // corrections" — drop the highlight so the next time the
            // user comes back the paragraph reads clean.
            if let Some(doc) = self.opened.as_mut() {
                doc.correction_baseline = None;
            }
            if self.opened.as_ref().is_some_and(|d| d.dirty) {
                let _ = self.save_current();
            }
            // Snapshot the cursor before defocusing so re-opening this
            // paragraph (now or in a future run) lands the cursor back where
            // the user left it. Persisting to disk here makes the position
            // survive a crash or kill, not just a graceful exit.
            self.snapshot_open_paragraph_cursor();
            if let Err(e) = self.save_session() {
                tracing::warn!("focus-loss session save failed: {e}");
            }
            // Typewriter SFX — "remove page from machine" when the
            // editor pane loses focus. No-op when sound is disabled or
            // the host has no audio device.
            if let Some(sp) = &self.sound {
                sp.play_focus_out();
            }
        }
        self.focus = new;
    }

    /// Render the secondary editor pane (right side, replaces AI
    /// when in similar-paragraph mode). Simpler than draw_editor —
    /// no syntax highlighting, no find/replace overlay, no split
    /// view — but supports a moving cursor so the user can edit.
    /// Focus highlight comes from `self.secondary_focused`, which
    /// is independent of `self.focus` (keystrokes get routed to
    /// secondary by the swap-on-dispatch wrapper in
    /// `handle_editor_key`).
    fn draw_secondary_editor(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let Some(doc) = self.secondary.as_ref() else {
            return;
        };
        let focused = self.focus == Focus::Editor && self.secondary_focused;
        let border_color = if focused {
            self.theme.border_focused
        } else {
            self.theme.border_unfocused
        };
        let title = format!(" {}  ·  (similar) ", doc.title);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(
                Style::default()
                    .fg(border_color)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.pane_bg)
                    .fg(self.theme.pane_fg),
            );
        let inner = block.inner(area);
        f.render_widget(block, area);

        // Reserve one row at the bottom for the slug-path footer.
        let footer_h: u16 = 1;
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(footer_h),
            width: inner.width,
            height: footer_h,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(footer_h),
        };

        // Render the textarea via the existing widget so cursor,
        // selection, scroll all behave correctly. tui-textarea
        // honours focus via cursor_line_style which we already
        // configured at load time.
        f.render_widget(&doc.textarea, body_rect);

        // Footer: full slug path (the spec calls for full path on
        // each editor pane in similar mode).
        let path = if let Some(node) = self.hierarchy.get(doc.id) {
            self.hierarchy.slug_path(node)
        } else {
            doc.rel_path.clone()
        };
        let footer = format!(" {}", path);
        let style = Style::default().add_modifier(Modifier::DIM);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(footer, style))),
            footer_rect,
        );
    }

    /// Slug-path footer drawn UNDER the primary editor pane when
    /// in similar-paragraph mode (so both panes show their path).
    /// Carved out of the primary editor's rect by the layout in
    /// `draw()`. No-op when not in similar mode — primary editor
    /// keeps its full area.
    fn draw_primary_pane_footer(&self, f: &mut ratatui::Frame, area: Rect) {
        let Some(doc) = self.opened.as_ref() else {
            return;
        };
        let path = if let Some(node) = self.hierarchy.get(doc.id) {
            self.hierarchy.slug_path(node)
        } else {
            doc.rel_path.clone()
        };
        let footer = format!(" {}", path);
        let style = Style::default().add_modifier(Modifier::DIM);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(footer, style))),
            area,
        );
    }

    fn draw_search_bar(&self, f: &mut ratatui::Frame, area: Rect) {
        let text = if self.focus == Focus::SearchBar {
            self.search_input.render_with_cursor('│')
        } else if self.search_input.is_empty() {
            String::from("(press Ctrl+/ to search)")
        } else {
            self.search_input.as_str().to_string()
        };
        let style = if self.focus == Focus::SearchBar {
            Style::default()
        } else {
            Style::default().add_modifier(Modifier::DIM)
        };
        let p = Paragraph::new(text)
            .style(style)
            .block(self.pane_block("Search", Focus::SearchBar));
        f.render_widget(p, area);
    }

    fn draw_ai_prompt(&self, f: &mut ratatui::Frame, area: Rect) {
        let text = if self.focus == Focus::AiPrompt {
            self.ai_input.render_with_cursor('│')
        } else if self.ai_input.is_empty() {
            String::from("(press Ctrl+I for AI; `/` lists prompts · F9 cycles scope)")
        } else {
            self.ai_input.as_str().to_string()
        };
        let style = if self.focus == Focus::AiPrompt {
            Style::default()
        } else {
            Style::default().add_modifier(Modifier::DIM)
        };
        // Title carries the current AI scope so the user knows what
        // context will be prepended on the next submit. Bright when scope
        // is non-None — easy to spot accidentally-armed scope.
        let title = match self.ai_mode {
            AiMode::None => "AI prompt".to_string(),
            other => format!("AI prompt · scope: {}", other.label()),
        };
        let p = Paragraph::new(text)
            .style(style)
            .block(self.pane_block(&title, Focus::AiPrompt));
        f.render_widget(p, area);
    }

    fn draw_tree(&self, f: &mut ratatui::Frame, area: Rect) {
        let tree_title: String = match self.link_pick_for {
            Some((_, LinkPickDirection::Outgoing)) => {
                " Tree · select paragraph to link · Esc cancels ".into()
            }
            Some((_, LinkPickDirection::Incoming)) => {
                " Tree · select paragraph that will link to current · Esc cancels "
                    .into()
            }
            None => "Tree".into(),
        };
        let block = self.pane_block(&tree_title, Focus::Tree);
        let inner = block.inner(area);
        f.render_widget(block, area);

        if self.rows.is_empty() {
            let hint = Paragraph::new("(empty project — `inkhaven add book \"…\"` from the CLI)")
                .style(Style::default().add_modifier(Modifier::DIM));
            f.render_widget(hint, inner);
            return;
        }

        let height = inner.height as usize;
        let mut scroll = self.tree_scroll;
        if self.tree_cursor < scroll {
            scroll = self.tree_cursor;
        } else if self.tree_cursor >= scroll + height && height > 0 {
            scroll = self.tree_cursor + 1 - height;
        }

        let open_id: Option<Uuid> = self.opened.as_ref().map(|d| d.id);

        let mut lines: Vec<Line> = Vec::new();
        for (i, (id, depth)) in self.rows.iter().enumerate().skip(scroll).take(height) {
            let Some(node) = self.hierarchy.get(*id) else {
                continue;
            };
            let indent = "  ".repeat(*depth);
            // When this row is the paragraph currently open in the editor,
            // swap its kind glyph for a "►" arrow so it's obvious at a
            // glance which paragraph the editor pane is showing.
            let is_open = open_id.is_some_and(|o| o == node.id);
            let is_collapsed = self.collapsed_nodes.contains(&node.id);
            let marker = if is_open {
                "►"
            } else {
                match node.kind {
                    // Paragraphs split by content_type: HJSON data
                    // nodes get curly-brace glyph, plain typst keeps
                    // the pilcrow.
                    NodeKind::Paragraph => match node.content_type.as_deref() {
                        Some("hjson") => "❴ ",
                        _ => "¶ ",
                    },
                    NodeKind::Image => "▣ ",
                    // Bund scripts: lambda glyph signals "executable
                    // code, not prose". Paired with the mauve row
                    // colour from tree_script_fg.
                    NodeKind::Script => "λ ",
                    // For branches, use ▾ (expanded) / ▸ (collapsed)
                    // glyphs so the expand/collapse state is visible
                    // at a glance.
                    _ => {
                        if is_collapsed {
                            "▸ "
                        } else {
                            "▾ "
                        }
                    }
                }
            };
            // Per-kind row colour; override with `tree_open_marker` for
            // the currently-loaded paragraph so the editor pane's clean-
            // state border has a matching visual cue in the tree.
            let kind_fg = match node.kind {
                NodeKind::Book => self.theme.tree_book_fg,
                NodeKind::Chapter => self.theme.tree_chapter_fg,
                NodeKind::Subchapter => self.theme.tree_subchapter_fg,
                NodeKind::Paragraph => self.theme.tree_paragraph_fg,
                NodeKind::Image => self.theme.tree_image_fg,
                NodeKind::Script => self.theme.tree_script_fg,
            };
            let mut row_style = Style::default().fg(kind_fg);
            if matches!(node.kind, NodeKind::Book | NodeKind::Chapter) {
                row_style = row_style.add_modifier(Modifier::BOLD);
            }
            if is_open {
                row_style = row_style
                    .fg(self.theme.tree_open_marker)
                    .add_modifier(Modifier::BOLD);
            }
            if i == self.tree_cursor {
                row_style = row_style.add_modifier(Modifier::REVERSED);
            }

            // Truncate very long titles (typical for paragraphs whose name
            // was auto-derived from the first sentence) so they don't push
            // the trailing status badge off the pane.
            let display_title = truncate_title(&node.title, TITLE_MAX_DISPLAY);
            // Build the prefix: indent + glyph + status-letter badge.
            // The badge is one char (or space) styled with the matching
            // workflow colour — gives every paragraph row a consistent
            // gutter column the eye can scan down.
            let status_label = if matches!(node.kind, NodeKind::Paragraph) {
                display_status(node.status.as_deref())
            } else {
                "None"
            };
            let status_letter = status_letter(status_label);
            let status_badge_style = status_style(status_label, &self.theme);
            let mut spans: Vec<Span<'_>> = Vec::new();
            // 1.2.4+: multi-select marker. A `✓` glyph in
            // front of the kind-marker when this row is in
            // `tree_marked`. Costs one char of indent at most.
            let select_prefix = if self.tree_marked.contains(&node.id) {
                "✓ "
            } else if !self.tree_marked.is_empty()
                && matches!(node.kind, NodeKind::Paragraph)
            {
                // Reserve the column when ANY row is marked so
                // the list visually aligns.
                "  "
            } else {
                ""
            };
            spans.push(Span::styled(
                format!("{indent}{select_prefix}{marker}"),
                row_style,
            ));
            // Always reserve the badge column (a space when None) so
            // titles align across rows regardless of which paragraphs
            // have a status set.
            spans.push(Span::styled(
                format!("{status_letter} "),
                if status_label == "None" {
                    Style::default().add_modifier(Modifier::DIM)
                } else {
                    status_badge_style
                },
            ));
            spans.push(Span::styled(display_title.to_string(), row_style));
            // Per-paragraph progress glyph (1.2.4+). The full gauge
            // lives on the editor pane's bottom border (see
            // `editor_goal_footer_text`); the tree pane just shows
            // a compact 1-char status pip so users can scan which
            // paragraphs carry goals without losing room to a
            // long auto-derived title.
            if matches!(node.kind, NodeKind::Paragraph) {
                if let Some(target) = node.target_words.filter(|n| *n > 0) {
                    let pct =
                        (node.word_count as i64 * 100 / target as i64).clamp(0, 999);
                    let pip = if pct >= 100 {
                        "●"
                    } else if pct >= 75 {
                        "◕"
                    } else if pct >= 50 {
                        "◑"
                    } else if pct >= 25 {
                        "◔"
                    } else {
                        "○"
                    };
                    let style = if pct >= 100 {
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD)
                    } else if pct >= 75 {
                        Style::default().fg(Color::LightGreen)
                    } else if pct >= 50 {
                        Style::default().fg(Color::Yellow)
                    } else if pct >= 25 {
                        Style::default().fg(Color::LightRed)
                    } else {
                        Style::default().fg(Color::Red).add_modifier(Modifier::DIM)
                    };
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(pip.to_string(), style));
                }
            }
            lines.push(Line::from(spans));
        }

        let p = Paragraph::new(lines);
        f.render_widget(p, inner);
    }

    fn draw_editor(&mut self, f: &mut ratatui::Frame, area: Rect) {
        // Build the title as a Line of styled spans so the `L… C…`
        // cursor read-out can carry its own theme colour. ratatui's
        // Block accepts a Line title directly.
        let title_line: Line<'_> = match &self.opened {
            Some(d) => {
                let (row, col) = d.textarea.cursor();
                let dirty = if d.dirty { " [modified]" } else { "" };
                let ro = if d.read_only { " [read-only]" } else { "" };
                // Live word count + reading-time estimate (250 wpm —
                // matches the Ctrl+B I book-info modal). Computed each
                // frame from the textarea so it tracks edits.
                let words: usize = d
                    .textarea
                    .lines()
                    .iter()
                    .map(|l| l.split_whitespace().count())
                    .sum();
                let reading = format_reading_time(words);
                let stats_style = Style::default()
                    .fg(self.theme.editor_position_fg)
                    .add_modifier(Modifier::BOLD);
                let lang_tag = match d.content_type.as_deref() {
                    Some("hjson") => " [hjson]",
                    Some("bund") => " [bund]",
                    _ => "",
                };
                // Status badge: hidden when None to keep the header
                // visually quiet on fresh paragraphs; colour-coded
                // through the workflow when set. The badge wraps in
                // brackets so it reads as metadata, not prose.
                let status_node = self.hierarchy.get(d.id);
                let status_label = status_node
                    .and_then(|n| n.status.as_deref())
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty() && *s != "None");
                // "edited X ago" from the node's `modified_at`. Updated
                // automatically on save (via `update_paragraph_content`),
                // and recomputed on every frame so the value freshens
                // visibly when the user re-opens after a break.
                let edited_ago = status_node.map(|n| {
                    let now = chrono::Utc::now();
                    let delta = now.signed_duration_since(n.modified_at);
                    let secs = delta.num_seconds().max(0) as u64;
                    format_age_humantime(std::time::Duration::from_secs(secs))
                });
                let mut spans: Vec<Span<'_>> = Vec::new();
                spans.push(Span::raw(format!(
                    " Editor — {}{}{}{} · ",
                    d.title, lang_tag, ro, dirty
                )));
                if let Some(label) = status_label {
                    spans.push(Span::styled(
                        format!("[{label}]"),
                        status_style(label, &self.theme),
                    ));
                    spans.push(Span::raw(" · "));
                }
                spans.push(Span::styled(
                    format!("L{} C{} ", row + 1, col + 1),
                    stats_style,
                ));
                spans.push(Span::raw("· "));
                spans.push(Span::styled(format!("{words}w"), stats_style));
                spans.push(Span::raw(" · "));
                spans.push(Span::styled(reading, stats_style));
                if let Some(ago) = edited_ago {
                    spans.push(Span::raw(" · "));
                    spans.push(Span::styled(
                        format!("edited {ago} ago"),
                        Style::default().add_modifier(Modifier::DIM),
                    ));
                }
                spans.push(Span::raw(" "));
                Line::from(spans)
            }
            None => Line::from(" Editor "),
        };
        let block = self.editor_block_line(title_line);
        let inner = block.inner(area);
        f.render_widget(block, area);

        if self.opened.is_none() {
            let hint = Paragraph::new(
                "(no paragraph open — select one in the Tree pane and press Enter)",
            )
            .style(Style::default().add_modifier(Modifier::DIM))
            .wrap(Wrap { trim: false });
            f.render_widget(hint, inner);
            return;
        }

        // Per-paragraph goal footer (1.2.4+). Carve one row off
        // the bottom of the editor area when the open paragraph
        // has a target word-count set. Provides reliable space
        // for the gauge — the tree pane can't fit it for long
        // auto-derived titles.
        let goal_footer = self.editor_goal_footer_text();
        let (editor_rect, footer_rect) = match goal_footer.as_ref() {
            Some(_) => {
                let footer_h: u16 = 1;
                let er = Rect {
                    x: inner.x,
                    y: inner.y,
                    width: inner.width,
                    height: inner.height.saturating_sub(footer_h),
                };
                let fr = Rect {
                    x: inner.x,
                    y: inner.y + inner.height.saturating_sub(footer_h),
                    width: inner.width,
                    height: footer_h,
                };
                (er, Some(fr))
            }
            None => (inner, None),
        };
        let inner = editor_rect;

        // Split-edit mode: divide the editor area into two halves; upper is
        // the live editor, lower is the read-only snapshot.
        let split_active = self.opened.as_ref().is_some_and(|d| d.split.is_some());
        if split_active {
            let halves = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(inner);
            let upper = halves[0];
            let lower = halves[1];
            if self.cfg.editor.wrap {
                self.draw_editor_wrapped(f, upper);
            } else {
                self.draw_editor_unwrapped(f, upper);
            }
            self.draw_split_snapshot(f, lower);
        } else if self.cfg.editor.wrap {
            self.draw_editor_wrapped(f, inner);
        } else {
            self.draw_editor_unwrapped(f, inner);
        }

        // Render the goal footer last so it sits on top of the
        // textarea's bottom row (the carve-out above shrunk the
        // textarea, leaving exactly one free row for us).
        if let (Some((gauge, words, target)), Some(rect)) =
            (goal_footer, footer_rect)
        {
            let pct = (words.max(0) * 100 / target.max(1)).clamp(0, 999);
            let (gauge_str, _pct, gauge_style) =
                format_progress_gauge(words, target);
            let pct_str = format!(" {pct}%");
            let counts =
                format!("  {words}/{target} words");
            let line = Line::from(vec![
                Span::raw(" "),
                Span::styled(gauge_str, gauge_style),
                Span::styled(pct_str, gauge_style),
                Span::styled(
                    counts,
                    Style::default().add_modifier(Modifier::DIM),
                ),
                Span::raw(format!("  · goal: {gauge}")),
            ]);
            f.render_widget(Paragraph::new(line), rect);
        }
    }

    /// Compute the editor-pane goal footer text from the open
    /// doc + its node metadata. Returns `(breadcrumb, words,
    /// target)` when a goal is set, otherwise `None`. The
    /// breadcrumb is the human-readable title chain
    /// ("My book › Chapter one › The morning") rather than the
    /// slug path — slugs are stale after a rename until we
    /// re-derive them, and users think in titles anyway.
    fn editor_goal_footer_text(&self) -> Option<(String, i64, i64)> {
        let doc = self.opened.as_ref()?;
        let node = self.hierarchy.get(doc.id)?;
        let target = node.target_words.filter(|n| *n > 0)? as i64;
        // Count live in-memory text via the same algorithm the
        // save path uses so the footer matches what the save
        // event will record.
        let body = doc.textarea.lines().join("\n");
        let words = crate::progress::count_words(&body);
        let breadcrumb = self.title_breadcrumb(node.id);
        Some((breadcrumb, words, target))
    }

    /// Render the lower (read-only) pane of split-edit mode. No cursor,
    /// no diff/bold, no current-line highlight — it's a frozen view of the
    /// buffer at the moment F4 was pressed.
    fn draw_split_snapshot(&self, f: &mut ratatui::Frame, area: Rect) {
        let Some(doc) = self.opened.as_ref() else {
            return;
        };
        let Some(split) = &doc.split else {
            return;
        };

        // 1 row for the separator/hint header, the rest for content.
        if area.height < 2 {
            return;
        }
        let header_rect = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        let content_rect = Rect {
            x: area.x,
            y: area.y + 1,
            width: area.width,
            height: area.height - 1,
        };

        let header = format!(
            "── snapshot · Ctrl+H/J scroll · Ctrl+F4 accept · F4 close (line {}/{}) ──",
            split.scroll_row + 1,
            split.snapshot_lines.len().max(1)
        );
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                header,
                Style::default().fg(Color::DarkGray),
            ))),
            header_rect,
        );

        let lineno_chars = digit_count(split.snapshot_lines.len().max(1));
        let gutter_width = (lineno_chars + 1) as u16;
        let visible = content_rect.height as usize;
        let body_w = content_rect.width.saturating_sub(gutter_width) as usize;

        let lineno_style = Style::default().fg(Color::DarkGray);
        let body_style = Style::default()
            .fg(Color::Gray)
            .add_modifier(Modifier::DIM);

        let mut lines: Vec<Line> = Vec::with_capacity(visible);
        for (i, line) in split
            .snapshot_lines
            .iter()
            .enumerate()
            .skip(split.scroll_row)
            .take(visible)
        {
            // Clip line to body_w chars so long lines don't overflow into
            // the pane border.
            let chars: Vec<char> = line.chars().collect();
            let shown: String = if chars.len() > body_w {
                chars.iter().take(body_w).collect()
            } else {
                chars.iter().collect()
            };
            let lineno = format!("{:>w$} ", i + 1, w = lineno_chars);
            lines.push(Line::from(vec![
                Span::styled(lineno, lineno_style),
                Span::styled(shown, body_style),
            ]));
        }
        f.render_widget(Paragraph::new(lines), content_rect);
    }

    fn draw_editor_unwrapped(&mut self, f: &mut ratatui::Frame, inner: Rect) {
        let block = self.current_block();
        let lexicon = &self.lexicon;
        let theme = &self.theme;
        let opened = self.opened.as_mut().expect("opened checked above");
        let highlighter = &mut self.highlighter;
        let current_lines: Vec<String> = opened.textarea.lines().to_vec();
        let source = current_lines.join("\n");
        let highlighted = highlight_for_content(highlighter, &source, theme, opened.content_type.as_deref());

        // Precompute "added since last save" bitmaps per source row.
        let saved = &opened.saved_lines;
        let added_per_row: Vec<Vec<bool>> = current_lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let saved_line = saved.get(i).map(String::as_str).unwrap_or("");
                if saved.get(i).is_none() {
                    // Line beyond the saved snapshot: everything is new.
                    vec![true; line.chars().count()]
                } else {
                    diff_added(saved_line, line)
                }
            })
            .collect();

        // Grammar-correction changes: same diff function against the
        // pre-correction baseline (set by `T` apply). Empty when no
        // correction is pending — the renderer then short-circuits.
        let correction_per_row: Vec<Vec<bool>> = match opened.correction_baseline.as_ref() {
            Some(base) => current_lines
                .iter()
                .enumerate()
                .map(|(i, line)| match base.get(i) {
                    Some(b) => diff_added(b, line),
                    None => vec![true; line.chars().count()],
                })
                .collect(),
            None => Vec::new(),
        };

        // Per-row regex hits for the match-highlight overlay.
        let matches_per_row: Vec<Vec<RowHit>> = (0..current_lines.len())
            .map(|row| match &opened.search {
                Some(state) => row_matches(state, row)
                    .into_iter()
                    .map(|h: RowMatch| RowHit {
                        col_start: h.col_start,
                        col_end: h.col_end,
                        is_current: h.is_current,
                    })
                    .collect(),
                None => Vec::new(),
            })
            .collect();

        // Per-row Place/Character matches.
        let lex_per_row: Vec<Vec<super::lexicon::LexHit>> = current_lines
            .iter()
            .map(|line| {
                if lexicon.is_empty() {
                    Vec::new()
                } else {
                    lexicon.row_hits(line)
                }
            })
            .collect();

        let (cur_row, cur_col) = opened.textarea.cursor();
        let selection = opened.textarea.selection_range();

        let total_lines = highlighted.len().max(1);
        let lineno_chars = digit_count(total_lines);
        let gutter_width = (lineno_chars + 1) as u16;

        let h = inner.height as usize;
        let w = inner.width.saturating_sub(gutter_width) as usize;

        if h > 0 {
            if cur_row < opened.scroll_row {
                opened.scroll_row = cur_row;
            } else if cur_row >= opened.scroll_row + h {
                opened.scroll_row = cur_row + 1 - h;
            }
        }
        if w > 0 {
            if cur_col < opened.scroll_col {
                opened.scroll_col = cur_col;
            } else if cur_col >= opened.scroll_col + w {
                opened.scroll_col = cur_col + 1 - w;
            }
        }

        let lineno_style = Style::default().fg(theme.line_number_fg);
        let current_bg = theme.current_line_bg;

        let mut visible_lines: Vec<Line> = Vec::with_capacity(h);
        let row_end = (opened.scroll_row + h).min(highlighted.len());
        for row in opened.scroll_row..row_end {
            let is_current = row == cur_row;
            let lineno_text = format!("{:>chars$} ", row + 1, chars = lineno_chars);
            let mut lineno_span_style = lineno_style;
            if is_current {
                lineno_span_style = lineno_span_style
                    .bg(current_bg)
                    .add_modifier(Modifier::BOLD);
            }

            let added_flags = added_per_row.get(row).map(Vec::as_slice);
            let correction_flags = correction_per_row.get(row).map(Vec::as_slice);
            let row_hits = matches_per_row
                .get(row)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let lex_hits = lex_per_row
                .get(row)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let mut text_spans = build_row_spans(
                &highlighted[row],
                row,
                opened.scroll_col,
                w,
                selection,
                block,
                added_flags,
                row_hits,
                lex_hits,
                correction_flags,
                theme,
            );
            if is_current {
                for s in &mut text_spans {
                    if s.style.bg.is_none() {
                        s.style = s.style.bg(current_bg);
                    }
                }
            }

            let text_chars: usize = text_spans.iter().map(|s| s.content.chars().count()).sum();
            let mut spans = vec![Span::styled(lineno_text, lineno_span_style)];
            spans.extend(text_spans);
            if is_current && text_chars < w {
                spans.push(Span::styled(
                    " ".repeat(w - text_chars),
                    Style::default().bg(current_bg),
                ));
            }
            visible_lines.push(Line::from(spans));
        }
        f.render_widget(Paragraph::new(visible_lines), inner);

        if self.focus == Focus::Editor
            && h > 0
            && w > 0
            && cur_row >= opened.scroll_row
            && cur_row < opened.scroll_row + h
            && cur_col >= opened.scroll_col
            && cur_col < opened.scroll_col + w
        {
            let x = inner.x + gutter_width + (cur_col - opened.scroll_col) as u16;
            let y = inner.y + (cur_row - opened.scroll_row) as u16;
            f.set_cursor_position((x, y));
        }
    }

    fn draw_editor_wrapped(&mut self, f: &mut ratatui::Frame, inner: Rect) {
        let block = self.current_block();
        let lexicon = &self.lexicon;
        let theme = &self.theme;
        let opened = self.opened.as_mut().expect("opened checked above");
        let highlighter = &mut self.highlighter;
        let current_lines: Vec<String> = opened.textarea.lines().to_vec();
        let source = current_lines.join("\n");
        let highlighted = highlight_for_content(highlighter, &source, theme, opened.content_type.as_deref());

        let saved = &opened.saved_lines;
        let added_per_row: Vec<Vec<bool>> = current_lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                if saved.get(i).is_none() {
                    vec![true; line.chars().count()]
                } else {
                    diff_added(&saved[i], line)
                }
            })
            .collect();

        let correction_per_row: Vec<Vec<bool>> = match opened.correction_baseline.as_ref() {
            Some(base) => current_lines
                .iter()
                .enumerate()
                .map(|(i, line)| match base.get(i) {
                    Some(b) => diff_added(b, line),
                    None => vec![true; line.chars().count()],
                })
                .collect(),
            None => Vec::new(),
        };

        let matches_per_row: Vec<Vec<RowHit>> = (0..current_lines.len())
            .map(|row| match &opened.search {
                Some(state) => row_matches(state, row)
                    .into_iter()
                    .map(|h: RowMatch| RowHit {
                        col_start: h.col_start,
                        col_end: h.col_end,
                        is_current: h.is_current,
                    })
                    .collect(),
                None => Vec::new(),
            })
            .collect();

        let lex_per_row: Vec<Vec<super::lexicon::LexHit>> = current_lines
            .iter()
            .map(|line| {
                if lexicon.is_empty() {
                    Vec::new()
                } else {
                    lexicon.row_hits(line)
                }
            })
            .collect();

        let (cur_row, cur_col) = opened.textarea.cursor();
        let selection = opened.textarea.selection_range();

        let total_lines = highlighted.len().max(1);
        let lineno_chars = digit_count(total_lines);
        let gutter_width = (lineno_chars + 1) as u16;

        let h = inner.height as usize;
        let w = inner.width.saturating_sub(gutter_width) as usize;

        let mut visual: Vec<super::highlight::VisualRow> = Vec::new();
        for (src_row, runs) in highlighted.iter().enumerate() {
            for vr in wrap_line(runs, src_row, w) {
                visual.push(vr);
            }
        }

        let cursor_visual = find_cursor_visual(&visual, cur_row, cur_col);

        if h > 0 {
            if cursor_visual.0 < opened.scroll_row {
                opened.scroll_row = cursor_visual.0;
            } else if cursor_visual.0 >= opened.scroll_row + h {
                opened.scroll_row = cursor_visual.0 + 1 - h;
            }
        }
        opened.scroll_col = 0;

        let lineno_style = Style::default().fg(theme.line_number_fg);
        let current_bg = theme.current_line_bg;

        let mut lines: Vec<Line> = Vec::with_capacity(h);
        let row_end = (opened.scroll_row + h).min(visual.len());
        for (i, v) in visual[opened.scroll_row..row_end].iter().enumerate() {
            let visual_row_idx = opened.scroll_row + i;
            let is_current = visual_row_idx == cursor_visual.0;

            // Line number only on the first visual row of each source row.
            let lineno_text = if v.src_col_start == 0 {
                format!("{:>chars$} ", v.src_row + 1, chars = lineno_chars)
            } else {
                format!("{:>chars$} ", "", chars = lineno_chars)
            };
            let mut lineno_span_style = lineno_style;
            if is_current {
                lineno_span_style = lineno_span_style
                    .bg(current_bg)
                    .add_modifier(Modifier::BOLD);
            }

            let added_flags = added_per_row.get(v.src_row).map(Vec::as_slice);
            let correction_flags =
                correction_per_row.get(v.src_row).map(Vec::as_slice);
            let row_hits = matches_per_row
                .get(v.src_row)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let lex_hits = lex_per_row
                .get(v.src_row)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let mut text_spans = build_visual_row_spans(
                v,
                selection,
                block,
                added_flags,
                row_hits,
                lex_hits,
                correction_flags,
                theme,
            );
            if is_current {
                for s in &mut text_spans {
                    if s.style.bg.is_none() {
                        s.style = s.style.bg(current_bg);
                    }
                }
            }

            let text_chars: usize = text_spans.iter().map(|s| s.content.chars().count()).sum();
            let mut spans = vec![Span::styled(lineno_text, lineno_span_style)];
            spans.extend(text_spans);
            if is_current && text_chars < w {
                spans.push(Span::styled(
                    " ".repeat(w - text_chars),
                    Style::default().bg(current_bg),
                ));
            }
            lines.push(Line::from(spans));
        }
        f.render_widget(Paragraph::new(lines), inner);

        if self.focus == Focus::Editor
            && h > 0
            && w > 0
            && cursor_visual.0 >= opened.scroll_row
            && cursor_visual.0 < opened.scroll_row + h
            && cursor_visual.1 < w
        {
            let x = inner.x + gutter_width + cursor_visual.1 as u16;
            let y = inner.y + (cursor_visual.0 - opened.scroll_row) as u16;
            f.set_cursor_position((x, y));
        }
    }

    fn draw_ai(&self, f: &mut ratatui::Frame, area: Rect) {
        // Title carries the inference state plus mode chips so the user
        // can see at a glance:
        //   - provider + streaming/done/error status
        //   - chat history depth (N turns) when non-empty
        //   - active AI scope (Selection/Paragraph/...) when non-None
        //   - active InferenceMode (Local/Full) — always shown so F10's
        //     effect is visible
        let chat_turns = self.chat_history.len() / 2;
        // Build the title as a styled Line so the scope= / infer= chips
        // can carry their own theme colours (F9 / F10 effects are
        // visible at a glance).
        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.push(Span::raw(" AI".to_string()));
        if let Some(inf) = &self.inference {
            let status_text = match &inf.status {
                InferenceStatus::Streaming => format!(" — {} · streaming…", inf.provider),
                InferenceStatus::Done => format!(" — {} · done", inf.provider),
                InferenceStatus::Error(_) => format!(" — {} · error", inf.provider),
            };
            spans.push(Span::raw(status_text));
        }
        if self.ai_mode != AiMode::None {
            spans.push(Span::raw(" · scope="));
            spans.push(Span::styled(
                self.ai_mode.label().to_string(),
                Style::default()
                    .fg(self.theme.ai_scope_fg)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        spans.push(Span::raw(" · infer="));
        spans.push(Span::styled(
            self.inference_mode.label().to_string(),
            Style::default()
                .fg(self.theme.ai_infer_fg)
                .add_modifier(Modifier::BOLD),
        ));
        if chat_turns > 0 {
            spans.push(Span::raw(format!(" · {chat_turns} turn(s)")));
        }
        spans.push(Span::raw(" "));
        let title_line = Line::from(spans);
        let block = self.pane_block_line(title_line, Focus::Ai);
        let inner = block.inner(area);
        f.render_widget(block, area);

        match &self.inference {
            None => {
                let hint = Paragraph::new(
                    "(focus AI prompt with Ctrl+I, type a query and press Enter\n\n type `/` to pick from the prompt library)",
                )
                .style(Style::default().add_modifier(Modifier::DIM))
                .wrap(Wrap { trim: false });
                f.render_widget(hint, inner);
            }
            Some(inf) => {
                // Reserve the last line for action hints when done.
                let show_hints = matches!(inf.status, InferenceStatus::Done) && !inf.response.is_empty();
                let body_height = if show_hints {
                    inner.height.saturating_sub(2)
                } else {
                    inner.height
                };
                let body_rect = Rect {
                    x: inner.x,
                    y: inner.y,
                    width: inner.width,
                    height: body_height,
                };
                let widget = match &inf.status {
                    InferenceStatus::Error(e) => Paragraph::new(e.clone())
                        .style(Style::default().fg(Color::Red))
                        .wrap(Wrap { trim: false }),
                    InferenceStatus::Streaming | InferenceStatus::Done => {
                        // Render the response as markdown — bold/italic/
                        // headings/code/lists all light up. Partial input
                        // during streaming is tolerated by the renderer.
                        let lines = super::markdown::render(&inf.response);
                        Paragraph::new(lines).wrap(Wrap { trim: false })
                    }
                };
                f.render_widget(widget, body_rect);
                if show_hints && inner.height >= 2 {
                    let hints_rect = Rect {
                        x: inner.x,
                        y: inner.y + inner.height - 1,
                        width: inner.width,
                        height: 1,
                    };
                    let hints = Line::from(vec![
                        Span::styled(" r ", reverse_chip(Color::Yellow)),
                        Span::raw("replace  "),
                        Span::styled(" i ", reverse_chip(Color::Yellow)),
                        Span::raw("insert  "),
                        Span::styled(" t ", reverse_chip(Color::Yellow)),
                        Span::raw("top  "),
                        Span::styled(" b ", reverse_chip(Color::Yellow)),
                        Span::raw("bottom  "),
                        Span::styled(" c ", reverse_chip(Color::Yellow)),
                        Span::raw("copy  "),
                        Span::styled(" g ", reverse_chip(Color::Green)),
                        Span::raw("grammar"),
                    ]);
                    f.render_widget(Paragraph::new(hints), hints_rect);
                }
            }
        }
    }

    fn toggle_chat_selection_mode(&mut self) {
        if self.chat_selection.is_some() {
            self.chat_selection = None;
            self.status = "chat selection mode off".into();
            return;
        }
        if self.chat_history.is_empty() {
            self.status = "chat history is empty — nothing to select".into();
            return;
        }
        // Start at the newest turn — the assistant reply you just
        // received is what most users want to copy.
        let turn = self.chat_history.len() - 1;
        self.chat_selection = Some(ChatSelectionState { turn });
        // Dismiss the search highlights so the selection's block bg
        // isn't fighting for visibility.
        self.chat_search = None;
        // Reset scroll — the renderer auto-centres on the selected
        // turn anyway.
        self.chat_history_scroll = 0;
        self.status =
            "chat selection mode · ↑↓ navigate · c=copy · t=insert into editor · Esc to exit".into();
    }

    fn chat_selection_step(&mut self, delta: isize) {
        let Some(sel) = self.chat_selection else { return };
        let total = self.chat_history.len();
        if total == 0 {
            self.chat_selection = None;
            return;
        }
        let new_turn = if delta < 0 {
            sel.turn.saturating_sub(delta.unsigned_abs())
        } else {
            (sel.turn + delta as usize).min(total - 1)
        };
        if let Some(s) = self.chat_selection.as_mut() {
            s.turn = new_turn;
        }
        let label = self.chat_turn_label(new_turn);
        self.status = format!("chat selection: {} {}/{total}", label, new_turn + 1);
    }

    fn chat_selection_jump(&mut self, target: usize) {
        let Some(_sel) = self.chat_selection else { return };
        let total = self.chat_history.len();
        if total == 0 {
            self.chat_selection = None;
            return;
        }
        let new_turn = target.min(total - 1);
        if let Some(s) = self.chat_selection.as_mut() {
            s.turn = new_turn;
        }
        let label = self.chat_turn_label(new_turn);
        self.status = format!("chat selection: {} {}/{total}", label, new_turn + 1);
    }

    fn chat_turn_label(&self, idx: usize) -> &'static str {
        match self.chat_history.get(idx) {
            Some(ChatTurn::User(_)) => "User",
            Some(ChatTurn::Assistant(_)) => "Assistant",
            None => "?",
        }
    }

    /// `c` / `C` action: copy the selected turn's text to the system
    /// clipboard. Silently no-op when no clipboard is available
    /// (headless host); status bar reports the outcome either way.
    fn chat_selection_copy(&mut self) {
        let Some(sel) = self.chat_selection else { return };
        let Some(turn) = self.chat_history.get(sel.turn) else { return };
        let text = match turn {
            ChatTurn::User(s) | ChatTurn::Assistant(s) => s.clone(),
        };
        match self.clipboard.as_mut() {
            Some(cb) => match cb.set_text(text.clone()) {
                Ok(()) => {
                    self.status = format!(
                        "copied {} turn ({} chars)",
                        self.chat_turn_label(sel.turn),
                        text.chars().count()
                    );
                }
                Err(e) => {
                    self.status = format!("clipboard copy failed: {e}");
                }
            },
            None => {
                self.status =
                    "no system clipboard available — copy unavailable on this host".into();
            }
        }
    }

    /// `t` / `T` action: insert the selected turn's text at the
    /// editor cursor. Useful when an Assistant reply is the right
    /// next paragraph or when a User question becomes the new
    /// prompt body. Requires an open paragraph in the editor.
    fn chat_selection_into_editor(&mut self) {
        let Some(sel) = self.chat_selection else { return };
        let Some(turn) = self.chat_history.get(sel.turn) else { return };
        let text = match turn {
            ChatTurn::User(s) | ChatTurn::Assistant(s) => s.clone(),
        };
        let label = self.chat_turn_label(sel.turn);
        if self.opened.is_none() {
            self.status =
                "no paragraph open — switch off AI fullscreen (Ctrl+B K) and pick one".into();
            return;
        }
        if let Some(doc) = self.opened.as_mut() {
            doc.textarea.insert_str(&text);
            doc.dirty = true;
        }
        self.status = format!(
            "inserted {label} turn into editor ({} chars)",
            text.chars().count()
        );
    }

    /// Open the chat-history search query modal. Pre-populates the
    /// input with the previous query (if any) so re-search is a
    /// single Enter.
    fn open_chat_search_prompt(&mut self) {
        if self.chat_history.is_empty() {
            self.status = "chat history is empty — nothing to search".into();
            return;
        }
        let mut input = TextInput::new();
        if let Some(prev) = &self.chat_search {
            for c in prev.query.chars() {
                input.insert_char(c);
            }
        }
        self.modal = Modal::ChatSearchPrompt { input };
        self.status =
            "Search chat history · Enter to start (newest first) · Ctrl+X next (older) · Esc cancel".into();
    }

    /// Apply the just-submitted query. Empty query clears any active
    /// search. The `current` index starts at the LAST match — the
    /// most recent / closest-to-the-bottom hit — per the spec.
    fn commit_chat_search(&mut self, query: String) {
        if query.is_empty() {
            self.chat_search = None;
            self.status = "chat search: empty query — cleared".into();
            return;
        }
        let total = self.chat_search_matches(&query).len();
        if total == 0 {
            self.chat_search = None;
            self.status = format!("chat search: no match for `{query}`");
            return;
        }
        self.chat_search = Some(ChatSearchState {
            query: query.clone(),
            current: total - 1, // newest match (last in match order)
        });
        // Recompute scroll so the centre lands on the match.
        // draw_chat_history handles this from the state.
        self.chat_history_scroll = 0;
        self.status = format!("chat search: `{query}` · 1/{total} (newest)");
    }

    /// Step the chat-search cursor one match toward older history.
    /// Wraps from oldest back to newest. Matches are recomputed each
    /// call to handle terminal resize / streaming-token arrival.
    fn advance_chat_search(&mut self) {
        let Some((query, current)) = self
            .chat_search
            .as_ref()
            .map(|s| (s.query.clone(), s.current))
        else {
            return;
        };
        let total = self.chat_search_matches(&query).len();
        if total == 0 {
            // Live history may have lost the matches we had — clear.
            self.chat_search = None;
            self.status = "chat search: no matches in current history".into();
            return;
        }
        let new_current = (current + total - 1) % total;
        if let Some(search) = self.chat_search.as_mut() {
            search.current = new_current;
        }
        self.status = format!(
            "chat search: `{query}` · {}/{}",
            new_current + 1,
            total
        );
    }

    /// Find every line index in the rendered chat-history pane
    /// whose text contains `query` (case-insensitive). Render runs
    /// against the same shape `draw_chat_history` produces so the
    /// indices map 1-1 to rendered rows.
    fn chat_search_matches(&self, query: &str) -> Vec<usize> {
        if query.is_empty() {
            return Vec::new();
        }
        let needle = query.to_lowercase();
        let (lines, _) = self.build_chat_history_lines();
        let mut out: Vec<usize> = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            if text.to_lowercase().contains(&needle) {
                out.push(i);
            }
        }
        out
    }

    /// Build the chat-history pane's Lines exactly as
    /// `draw_chat_history` does — same iteration, same markdown
    /// rendering, same headers. Returns both the lines and per-turn
    /// `(line_start..line_end)` ranges so the chat-selection mode
    /// can highlight the active turn as a block.
    fn build_chat_history_lines(&self) -> (Vec<Line<'static>>, Vec<std::ops::Range<usize>>) {
        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut turn_ranges: Vec<std::ops::Range<usize>> = Vec::new();
        let user_style = Style::default()
            .fg(self.theme.ai_scope_fg)
            .add_modifier(Modifier::BOLD);
        let assistant_style = Style::default()
            .fg(self.theme.ai_infer_fg)
            .add_modifier(Modifier::BOLD);
        for (i, turn) in self.chat_history.iter().enumerate() {
            let turn_start = lines.len();
            match turn {
                ChatTurn::User(text) => {
                    if i > 0 {
                        lines.push(Line::from(""));
                    }
                    lines.push(Line::from(Span::styled(
                        "❯ User".to_string(),
                        user_style,
                    )));
                    for line in text.lines() {
                        lines.push(Line::from(format!("  {line}")));
                    }
                }
                ChatTurn::Assistant(text) => {
                    lines.push(Line::from(Span::styled(
                        "← Assistant".to_string(),
                        assistant_style,
                    )));
                    let rendered = super::markdown::render(text);
                    if rendered.is_empty() {
                        for line in text.lines() {
                            lines.push(Line::from(format!("  {line}")));
                        }
                    } else {
                        for l in rendered {
                            lines.push(l);
                        }
                    }
                }
            }
            turn_ranges.push(turn_start..lines.len());
        }
        (lines, turn_ranges)
    }

    /// Render the accumulated chat history (User / Assistant turns).
    /// Used by the `Ctrl+B K` AI-fullscreen layout. The newest turn is
    /// pinned to the bottom of the pane — old history scrolls up off-
    /// screen, matching the natural chat-window UX. `Paragraph::scroll`
    /// handles the offset so we don't have to track per-pane state.
    fn draw_chat_history(&self, f: &mut ratatui::Frame, area: Rect) {
        let scroll_tag = if self.chat_history_scroll > 0 {
            format!(" · ↑ {} line(s)", self.chat_history_scroll)
        } else {
            String::new()
        };
        let block = self.pane_block_line(
            Line::from(format!(
                " Chat history · {} turn(s){scroll_tag} · ↑↓ / PgUp / PgDn ",
                self.chat_history.len()
            )),
            // Use the AI focus colouring so the two AI-related panes
            // visually group together when the layout is active.
            Focus::Ai,
        );
        let inner = block.inner(area);
        f.render_widget(block, area);

        if self.chat_history.is_empty() {
            let hint = Paragraph::new(
                "(no chat turns yet — send a query from the AI prompt below)",
            )
            .style(Style::default().add_modifier(Modifier::DIM))
            .wrap(Wrap { trim: false });
            f.render_widget(hint, inner);
            return;
        }

        let (mut lines, turn_ranges) = self.build_chat_history_lines();

        // Chat-selection mode: paint the selected turn's lines with
        // a block bg + clamp the turn index against the live
        // history (so a deletion / wipe doesn't leave the highlight
        // dangling).
        let centred_selection: Option<usize> = if let Some(sel) = self.chat_selection {
            let total_turns = self.chat_history.len();
            if total_turns == 0 {
                None
            } else {
                let turn = sel.turn.min(total_turns - 1);
                match turn_ranges.get(turn).cloned() {
                    Some(range) => {
                        let block_style = ratatui::style::Style::default()
                            .bg(self.theme.current_line_bg);
                        for i in range.clone() {
                            if let Some(line) = lines.get_mut(i) {
                                for span in line.spans.iter_mut() {
                                    span.style = span.style.patch(block_style);
                                }
                            }
                        }
                        Some((range.start + range.end) / 2)
                    }
                    None => None,
                }
            }
        } else {
            None
        };

        // If a search is active, highlight ONLY the matched substring
        // on each hit line (not the whole line) and pin the
        // centred match's line index for the scroll math. Matches
        // the editor's per-token search highlight visually: the
        // matched word reads dark text on a light pink bg, so the
        // characters stay legible.
        let body_h = inner.height as usize;
        let centred_match: Option<usize> = if let Some(search) = &self.chat_search {
            let needle = search.query.to_lowercase();
            let mut match_indices: Vec<usize> = Vec::new();
            for (i, line) in lines.iter().enumerate() {
                let text: String =
                    line.spans.iter().map(|s| s.content.as_ref()).collect();
                if text.to_lowercase().contains(&needle) {
                    match_indices.push(i);
                }
            }
            let total = match_indices.len();
            let cursor = if total == 0 {
                0
            } else {
                search.current.min(total - 1)
            };
            for (mi, idx) in match_indices.iter().enumerate() {
                let is_current = mi == cursor;
                highlight_substring_in_line(
                    &mut lines[*idx],
                    &needle,
                    is_current,
                    &self.theme,
                );
            }
            match_indices.get(cursor).copied()
        } else {
            None
        };

        // Scroll: search-centred mode wins over manual / auto when
        // active. Otherwise the existing auto-bottom-pin minus the
        // user's PageUp delta still drives.
        let total = lines.len();
        let auto_scroll = total.saturating_sub(body_h);
        // Centring precedence: a live search trumps selection (the
        // user is presumably hunting for a phrase); otherwise the
        // chat-selection focal point; otherwise the user's manual
        // PageUp delta over the auto-pin.
        let centre_line = centred_match.or(centred_selection);
        let scroll_offset = if let Some(line_idx) = centre_line {
            line_idx.saturating_sub(body_h / 2).min(auto_scroll.max(0))
        } else {
            auto_scroll.saturating_sub(self.chat_history_scroll)
        };
        let p = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll_offset as u16, 0));
        f.render_widget(p, inner);
    }

    fn draw_prompt_picker(&self, f: &mut ratatui::Frame, area: Rect) {
        let width = (area.width * 6 / 10).max(40).min(area.width.saturating_sub(4));
        let matches = self.prompt_picker_matches();
        let row_count = matches.len() as u16;
        let height = (row_count * 2 + 2).max(4).min(area.height.saturating_sub(4));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        // Anchor near the bottom (above the AI prompt bar).
        let y = area.height.saturating_sub(height + 4) + area.y;
        let rect = Rect {
            x,
            y,
            width,
            height,
        };
        f.render_widget(ratatui::widgets::Clear, rect);

        let mut lines: Vec<Line> = Vec::new();
        if matches.is_empty() {
            lines.push(Line::from(Span::styled(
                "(no matching prompts)",
                Style::default().add_modifier(Modifier::DIM),
            )));
        } else {
            for (i, p) in matches.iter().enumerate() {
                let selected = i == self.prompt_picker_cursor;
                let name_style = if selected {
                    Style::default()
                        .add_modifier(Modifier::REVERSED | Modifier::BOLD)
                        .fg(Color::Magenta)
                } else {
                    Style::default().fg(Color::Magenta)
                };
                let desc_style = if selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default().add_modifier(Modifier::DIM)
                };
                let (chip_text, chip_color) = match p.source {
                    PromptSource::System => (" system ", Color::Cyan),
                    PromptSource::Book => (" book ", Color::Green),
                };
                let chip_style = Style::default()
                    .bg(chip_color)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD);
                lines.push(Line::from(vec![
                    Span::styled(chip_text.to_string(), chip_style),
                    Span::styled(format!(" /{}", p.name), name_style),
                ]));
                lines.push(Line::from(Span::styled(
                    format!("        {}", p.description),
                    desc_style,
                )));
            }
        }

        f.render_widget(
            Paragraph::new(lines).wrap(Wrap { trim: false }).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Prompts ")
                    .border_style(
                        Style::default()
                            .fg(self.theme.modal_border)
                            .add_modifier(Modifier::BOLD),
                    )
                    .style(
                        Style::default()
                            .bg(self.theme.modal_bg)
                            .fg(self.theme.modal_fg),
                    ),
            ),
            rect,
        );
    }

    fn draw_status(&self, f: &mut ratatui::Frame, area: Rect) {
        let dirty = self.opened.as_ref().is_some_and(|d| d.dirty);
        let mut spans: Vec<Span<'_>> = Vec::new();
        if dirty {
            spans.push(Span::styled(
                " ● ",
                Style::default()
                    .bg(Color::Red)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        if self.meta_pending {
            spans.push(Span::styled(
                " META ",
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(
            format!(" [{}] ", self.focus.label()),
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw("  "));
        spans.push(Span::raw(self.status.clone()));

        // Right-aligned progress widget — drawn on its own
        // Paragraph with right alignment so it can't be pushed
        // off-screen by a long status message; the left part
        // truncates if the terminal is narrow.
        let progress_text = self.progress_widget_text();
        if !progress_text.is_empty() {
            let right = Paragraph::new(Line::from(Span::styled(
                progress_text,
                Style::default().add_modifier(Modifier::DIM),
            )))
            .alignment(ratatui::layout::Alignment::Right);
            f.render_widget(right, area);
        }
        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    /// One-line writing-progress summary for the right edge of
    /// the status bar. Empty when progress tracking is disabled
    /// (no store installed or no goals configured).
    fn progress_widget_text(&self) -> String {
        let Some(snap) = self.progress_cache.as_ref() else {
            return String::new();
        };
        let mut parts: Vec<String> = Vec::new();
        let today = snap.project.today_words;
        match snap.project.daily_goal {
            Some(goal) => parts.push(format!("today {today}/{goal}w")),
            None => parts.push(format!("today {today}w")),
        }
        // Active-time chip — bare duration when no goal is set,
        // `<spent>/<goal>` when there is one.
        let active_goal_secs = self.cfg.goals.active_minutes_daily.max(0) * 60;
        if active_goal_secs > 0 {
            parts.push(format!(
                "{}/{}",
                format_active_duration(snap.active_seconds_today),
                format_active_duration(active_goal_secs),
            ));
        } else if snap.active_seconds_today > 0 {
            parts.push(format_active_duration(snap.active_seconds_today));
        }
        if snap.streak.days > 0 {
            parts.push(format!("streak {}d", snap.streak.days));
        }
        // Outgoing-link count for the open paragraph (1.2.4+).
        // Only shown when > 0 to keep the line short on the
        // common "no links yet" case.
        if let Some(doc) = self.opened.as_ref() {
            if let Some(node) = self.hierarchy.get(doc.id) {
                let n = node.linked_paragraphs.len();
                if n > 0 {
                    parts.push(format!("links {n}"));
                }
            }
        }
        // Surface the focused book's pace if a deadline is set.
        if let Some(book) = snap.books.iter().find(|b| {
            self.opened
                .as_ref()
                .and_then(|d| self.book_of_node(d.id))
                .map_or(false, |bid| {
                    // Compare by title since BookProgress doesn't
                    // carry the uuid; book labels are unique
                    // within a project.
                    self.hierarchy
                        .get(bid)
                        .map_or(false, |n| n.title == b.label)
                })
        }) {
            if let (Some(target), Some(pace)) =
                (book.target_words, book.required_pace)
            {
                parts.push(format!(
                    "{} {}/{}w (pace {}w/d)",
                    book.label, book.total_words, target, pace
                ));
            }
        }
        format!("{} ", parts.join(" · "))
    }

    fn draw_search_overlay(&self, f: &mut ratatui::Frame, area: Rect) {
        let width = area.width.saturating_sub(6).max(40);
        // Each result takes 3 lines (header / title / snippet); +2 for borders;
        // +1 for an "(no results)" hint when empty.
        let body_rows = if self.results.is_empty() {
            1
        } else {
            (self.results.len() as u16) * 3
        };
        let height = (body_rows + 2).min(area.height.saturating_sub(2)).max(5);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + 1;
        let rect = Rect {
            x,
            y,
            width,
            height,
        };

        f.render_widget(ratatui::widgets::Clear, rect);

        let title = format!(
            " Results for `{}` ({}) ",
            self.search_input.as_str(),
            self.results.len()
        );

        let mut lines: Vec<Line> = Vec::new();
        if self.results.is_empty() {
            lines.push(Line::from(Span::styled(
                "(no results)",
                Style::default().add_modifier(Modifier::DIM),
            )));
        } else {
            for (i, hit) in self.results.iter().enumerate() {
                let selected = i == self.results_cursor;
                let header_style = if selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::REVERSED | Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Yellow)
                };
                let title_style = if selected {
                    Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
                } else {
                    Style::default().add_modifier(Modifier::BOLD)
                };
                let snippet_style = if selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default().add_modifier(Modifier::DIM)
                };

                // Display the human-readable breadcrumb (ancestor titles
                // joined with `›`) instead of the slug-based directory path
                // — book/chapter/subchapter names are what the user
                // recognises.
                let breadcrumb = self.title_breadcrumb(hit.id);
                let header = format!(
                    " {:>5.3}  [{:<10}] {} ",
                    hit.score,
                    hit.kind.as_str(),
                    breadcrumb
                );
                lines.push(Line::from(Span::styled(header, header_style)));
                lines.push(Line::from(Span::styled(
                    format!("         {}", hit.title),
                    title_style,
                )));
                let snip = if hit.snippet.is_empty() {
                    "         (no body yet)".to_string()
                } else {
                    format!("         {}", hit.snippet)
                };
                lines.push(Line::from(Span::styled(snip, snippet_style)));
            }
        }

        let body = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
        );
        f.render_widget(body, rect);
    }

    // ── Bund stdlib bridge ───────────────────────────────────────────
    //
    // Called by `src/scripting/stdlib/app.rs` via `with_active_app`.
    // Each `ink_*` method exposes a single primitive operation; the
    // Bund word handler takes care of argument parsing + status
    // reporting. Methods return Option<…> for queries and Result<…>
    // for mutations so the stdlib can surface a clean error when
    // there's no open buffer / no AI session.

    pub(crate) fn ink_editor_cursor(&self) -> Option<(usize, usize)> {
        self.opened.as_ref().map(|d| d.textarea.cursor())
    }

    pub(crate) fn ink_editor_goto(&mut self, row: usize, col: usize) -> Result<(), String> {
        let Some(doc) = self.opened.as_mut() else {
            return Err("no buffer open".into());
        };
        doc.textarea.move_cursor(tui_textarea::CursorMove::Jump(
            row as u16, col as u16,
        ));
        Ok(())
    }

    pub(crate) fn ink_editor_insert(&mut self, text: &str) -> Result<(), String> {
        let Some(doc) = self.opened.as_mut() else {
            return Err("no buffer open".into());
        };
        doc.textarea.insert_str(text);
        doc.dirty = true;
        Ok(())
    }

    pub(crate) fn ink_editor_scroll(&mut self, delta: i32) -> Result<(), String> {
        let Some(doc) = self.opened.as_mut() else {
            return Err("no buffer open".into());
        };
        let max = doc.textarea.lines().len().saturating_sub(1);
        let new = (doc.scroll_row as i64).saturating_add(delta as i64).max(0) as usize;
        doc.scroll_row = new.min(max);
        Ok(())
    }

    pub(crate) fn ink_editor_delete_line(&mut self) -> Result<(), String> {
        let Some(doc) = self.opened.as_mut() else {
            return Err("no buffer open".into());
        };
        // Kill the whole line: move to start, delete to end.
        doc.textarea.move_cursor(tui_textarea::CursorMove::Head);
        doc.textarea.delete_line_by_end();
        doc.dirty = true;
        Ok(())
    }

    pub(crate) fn ink_editor_delete_to_bol(&mut self) -> Result<(), String> {
        let Some(doc) = self.opened.as_mut() else {
            return Err("no buffer open".into());
        };
        doc.textarea.delete_line_by_head();
        doc.dirty = true;
        Ok(())
    }

    pub(crate) fn ink_editor_delete_to_eol(&mut self) -> Result<(), String> {
        let Some(doc) = self.opened.as_mut() else {
            return Err("no buffer open".into());
        };
        doc.textarea.delete_line_by_end();
        doc.dirty = true;
        Ok(())
    }

    pub(crate) fn ink_editor_text(&self) -> Option<String> {
        self.opened.as_ref().map(|d| d.textarea.lines().join("\n"))
    }

    /// First substring match in the buffer, returned as `(row, col)`.
    pub(crate) fn ink_editor_find(&self, needle: &str) -> Option<(usize, usize)> {
        if needle.is_empty() {
            return None;
        }
        let doc = self.opened.as_ref()?;
        for (row, line) in doc.textarea.lines().iter().enumerate() {
            if let Some(col) = line.find(needle) {
                return Some((row, col));
            }
        }
        None
    }

    pub(crate) fn ink_ai_clear_history(&mut self) {
        self.clear_chat_history();
    }

    pub(crate) fn ink_typst_assemble(&mut self) {
        self.schedule_assembly();
    }

    pub(crate) fn ink_typst_build(&mut self) {
        self.schedule_build();
    }

    pub(crate) fn ink_typst_take(&mut self) {
        self.schedule_take();
    }

    // ── Phase C: AI / theme / editor.replace ─────────────────────────

    /// Send a user prompt through the same AI pipeline the
    /// `Ctrl+I` chord uses: kicks off streaming inference,
    /// returns immediately. Response lands in `chat_history`
    /// once the stream completes. No synchronous result —
    /// Bund scripts that want the response read
    /// `ink.ai.history` later (e.g. in a hook firing on a
    /// subsequent action).
    /// Multi-occurrence find/replace on the open buffer. Returns
    /// the number of replacements made. Empty `find` returns 0
    /// (defends against an infinite loop with `replace.contains(find)`).
    pub(crate) fn ink_editor_replace_all(
        &mut self,
        find: &str,
        replace: &str,
    ) -> std::result::Result<i64, String> {
        if find.is_empty() {
            return Ok(0);
        }
        let doc = self
            .opened
            .as_mut()
            .ok_or_else(|| "no paragraph open".to_string())?;
        if doc.read_only {
            return Err("paragraph is read-only".into());
        }
        // Pull the body, do all replacements at once with String::replace,
        // then re-load the textarea. Avoids cursor-tracking complexity of
        // doing N find/replace passes through tui-textarea.
        let old_body = doc.textarea.lines().join("\n");
        let new_body = old_body.replace(find, replace);
        if new_body == old_body {
            return Ok(0);
        }
        // Count occurrences by counting non-overlapping matches in the
        // original body — same semantics as String::replace.
        let count = old_body.matches(find).count() as i64;
        let lines = body_to_lines(&new_body);
        let mut ta = TextArea::new(lines);
        ta.set_cursor_line_style(Style::default().add_modifier(Modifier::REVERSED));
        ta.set_line_number_style(Style::default().fg(Color::DarkGray));
        doc.textarea = ta;
        doc.dirty = true;
        doc.last_activity = std::time::Instant::now();
        Ok(count)
    }

    /// Open the `index`-th semantic-search hit for `query`.
    /// Returns true when a paragraph was loaded, false when the
    /// search came back empty or `index` is out of bounds.
    pub(crate) fn ink_search_load(
        &mut self,
        query: &str,
        index: usize,
    ) -> std::result::Result<bool, String> {
        if query.trim().is_empty() {
            return Err("empty query".into());
        }
        let raw = self
            .store
            .search_text(query, (index + 1).max(8))
            .map_err(|e| format!("search: {e}"))?;
        let hits: Vec<crate::tui::search_results::SearchHit> = raw
            .iter()
            .filter_map(crate::tui::search_results::SearchHit::parse)
            .collect();
        let Some(hit) = hits.get(index) else {
            return Ok(false);
        };
        let Some(node) = self.hierarchy.get(hit.id).cloned() else {
            return Ok(false);
        };
        if node.kind != crate::store::node::NodeKind::Paragraph {
            return Err(format!("hit `{}` is not a paragraph", node.title));
        }
        self.load_paragraph(&node)
            .map_err(|e| format!("load_paragraph: {e:#}"))?;
        Ok(true)
    }

    /// Snapshot the current AI inference state for `ink.ai.poll`.
    /// Returns `(status, response, elapsed_ms)`; status is one of
    /// `"none"` / `"streaming"` / `"done"` / `"error:<msg>"`.
    /// `response` accumulates as the stream arrives.
    pub(crate) fn ink_ai_poll(&self) -> (String, String, i64) {
        let Some(inf) = self.inference.as_ref() else {
            return ("none".into(), String::new(), 0);
        };
        let status = match &inf.status {
            InferenceStatus::Streaming => "streaming".to_string(),
            InferenceStatus::Done => "done".to_string(),
            InferenceStatus::Error(e) => format!("error: {e}"),
        };
        let elapsed = inf.started_at.elapsed().as_millis() as i64;
        (status, inf.response.clone(), elapsed)
    }

    /// Spawn an inference (same as `ink.ai.send`) and then poll
    /// until it terminates or `timeout_ms` elapses. The TUI does
    /// not redraw while the loop runs — that's the user's
    /// trade-off when picking the blocking variant. Returns the
    /// accumulated response, or `None` if the wait timed out (the
    /// inference itself keeps streaming in the background and
    /// can be read with `ink.ai.poll`).
    pub(crate) fn ink_ai_send_blocking(
        &mut self,
        prompt: &str,
        timeout_ms: i64,
    ) -> std::result::Result<Option<String>, String> {
        self.ink_ai_send(prompt)?;
        let timeout = std::time::Duration::from_millis(timeout_ms.max(0) as u64);
        let started = std::time::Instant::now();
        loop {
            // Pull whatever's accumulated in the stream channel.
            self.pump_inference();
            if let Some(inf) = self.inference.as_ref() {
                match &inf.status {
                    InferenceStatus::Done => {
                        return Ok(Some(inf.response.clone()));
                    }
                    InferenceStatus::Error(e) => {
                        return Err(format!("inference error: {e}"));
                    }
                    InferenceStatus::Streaming => {}
                }
            } else {
                // No inference state — shouldn't happen since we
                // just spawned one, but stay safe.
                return Ok(None);
            }
            if started.elapsed() >= timeout {
                return Ok(None);
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    pub(crate) fn ink_ai_send(&mut self, prompt: &str) -> Result<(), String> {
        if prompt.trim().is_empty() {
            return Err("empty prompt".into());
        }
        // Borrow the existing input-driven path: pre-load
        // `ai_input` then call `start_inference`. Same code
        // path the AI prompt input field uses.
        self.ai_input.clear();
        for c in prompt.chars() {
            self.ai_input.insert_char(c);
        }
        self.start_inference();
        Ok(())
    }

    /// Return the chat history as a Vec of (role, content)
    /// pairs. `role` is `"user"` or `"assistant"`. Most recent
    /// turn is last.
    pub(crate) fn ink_ai_history(&self) -> Vec<(String, String)> {
        self.chat_history
            .iter()
            .map(|t| match t {
                ChatTurn::User(s) => ("user".into(), s.clone()),
                ChatTurn::Assistant(s) => ("assistant".into(), s.clone()),
            })
            .collect()
    }

    /// Set (or clear) the script-supplied system-prompt override.
    /// Empty string clears it; otherwise the inference path
    /// consults the override before falling back to the
    /// inference-mode default. See `system_prompt_override` field.
    pub(crate) fn ink_ai_set_system_prompt(&mut self, text: &str) {
        if text.trim().is_empty() {
            self.system_prompt_override = None;
        } else {
            self.system_prompt_override = Some(text.to_string());
        }
    }

    /// Mutate one theme colour at runtime AND persist the new
    /// value to `inkhaven.hjson` so the change survives restart.
    /// `field` is the theme struct's field name
    /// (`tree_paragraph_fg`, `syntax_keyword`, etc.); `hex` is
    /// `#rrggbb` or a named colour.
    ///
    /// In-memory update happens first; if HJSON write-back fails
    /// (missing `theme:` block, disk error, etc.) the runtime
    /// colour stays applied for the rest of this session — the
    /// caller logs the persistence error but doesn't roll back.
    /// That's deliberate: a hook running mid-session shouldn't
    /// have its visual feedback yanked because the project's
    /// HJSON happens to be locked.
    pub(crate) fn ink_theme_set(
        &mut self,
        field: &str,
        hex: &str,
    ) -> Result<(), String> {
        self.theme.set_by_name(field, hex)?;
        // Persist back to inkhaven.hjson. Failure here is logged
        // at WARN; the in-memory mutation already succeeded.
        let config_path = self.layout.config_path();
        let raw = match std::fs::read_to_string(&config_path) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    target: "inkhaven::theme",
                    "theme persist: read {}: {e}",
                    config_path.display(),
                );
                return Ok(());
            }
        };
        // HJSON colour literal is a double-quoted string.
        let value_lit = format!("\"{}\"", hex);
        match set_key_in_hjson_block(&raw, "theme", field, &value_lit) {
            Ok(updated) => {
                if let Err(e) = std::fs::write(&config_path, &updated) {
                    tracing::warn!(
                        target: "inkhaven::theme",
                        "theme persist: write {}: {e}",
                        config_path.display(),
                    );
                }
            }
            Err(reason) => {
                tracing::warn!(
                    target: "inkhaven::theme",
                    "theme persist: rewrite `{field}` in `theme` block: {reason}",
                );
            }
        }
        Ok(())
    }

    /// Open the BundInput modal. The user's typed string lands
    /// on the Adam workbench via `hooks::fire(hook, [string])`
    /// when they press Enter. Esc closes without firing. The
    /// caller (the `ink.input` word handler) is responsible for
    /// ensuring `hook` names a registered lambda — otherwise
    /// `hooks::fire` silently no-ops.
    pub(crate) fn open_bund_input(&mut self, prompt: &str, hook: &str) {
        self.modal = Modal::BundInput {
            prompt: prompt.to_string(),
            input: TextInput::new(),
            hook: hook.to_string(),
        };
    }

    /// Append text to the active Bund output pane, if any.
    /// Returns `true` if the text was routed to the pane,
    /// `false` if no pane is open (caller falls back to the
    /// print buffer / status bar). When `newline` is true the
    /// text becomes its own line; otherwise it's concatenated
    /// to the last line.
    pub(crate) fn append_to_bund_pane(&mut self, text: &str, newline: bool) -> bool {
        let Modal::BundPane { lines, scroll, .. } = &mut self.modal else {
            return false;
        };
        if newline {
            for chunk in text.split('\n') {
                lines.push(chunk.to_string());
            }
        } else {
            // Split on embedded newlines so a single print of
            // "a\nb" still produces two lines, but the LAST
            // chunk stays append-to-prior.
            let mut parts = text.split('\n');
            if let Some(first) = parts.next() {
                if let Some(last) = lines.last_mut() {
                    last.push_str(first);
                } else {
                    lines.push(first.to_string());
                }
            }
            for chunk in parts {
                lines.push(chunk.to_string());
            }
        }
        // Auto-scroll to bottom so streaming output is visible
        // without manual scrolling. User can scroll back later.
        let visible = 20usize; // approximate; clamped by renderer
        if lines.len() > visible {
            *scroll = lines.len() - visible;
        }
        true
    }

    /// Open (or reuse) the Bund output pane with `title`. If a
    /// pane is already open it's replaced — same as Esc-then-
    /// open. Used by `ink.pane.show`.
    pub(crate) fn open_bund_pane(&mut self, title: &str) {
        self.modal = Modal::BundPane {
            title: title.to_string(),
            lines: Vec::new(),
            scroll: 0,
        };
    }

    /// Close the Bund pane (no-op if not open).
    pub(crate) fn close_bund_pane(&mut self) {
        if matches!(self.modal, Modal::BundPane { .. }) {
            self.modal = Modal::None;
        }
    }

    /// Clear the Bund pane's line buffer while keeping it open.
    pub(crate) fn clear_bund_pane(&mut self) -> bool {
        let Modal::BundPane { lines, scroll, .. } = &mut self.modal else {
            return false;
        };
        lines.clear();
        *scroll = 0;
        true
    }

    /// Replace the first occurrence of `find` in the open
    /// buffer with `replace`. Cursor lands at the start of the
    /// replacement. Returns `true` if a match was found and
    /// replaced; `false` if no match.
    pub(crate) fn ink_editor_replace(
        &mut self,
        find: &str,
        replace: &str,
    ) -> Result<bool, String> {
        let Some(doc) = self.opened.as_mut() else {
            return Err("no buffer open".into());
        };
        if find.is_empty() {
            return Err("find string is empty".into());
        }
        // Linear scan over lines for the first match.
        let target: Option<(usize, usize)> = doc
            .textarea
            .lines()
            .iter()
            .enumerate()
            .find_map(|(r, line)| line.find(find).map(|c| (r, c)));
        let Some((row, col)) = target else {
            return Ok(false);
        };
        doc.textarea
            .move_cursor(tui_textarea::CursorMove::Jump(row as u16, col as u16));
        // Delete `find.len()` chars; tui-textarea's delete_char
        // works one char at a time and there's no bulk delete
        // by length, so iterate.
        for _ in 0..find.chars().count() {
            doc.textarea.delete_next_char();
        }
        doc.textarea.insert_str(replace);
        doc.dirty = true;
        Ok(true)
    }
}

/// Map a source-coordinate cursor (row, col) to its visual-coordinate
/// position inside a wrapped layout. Linear in number of visual rows up to
/// the cursor, which is bounded by the source size — cheap at literary scale.
fn find_cursor_visual(
    visual: &[super::highlight::VisualRow],
    cur_row: usize,
    cur_col: usize,
) -> (usize, usize) {
    // Locate the cursor row's last visual row so end-of-line cursor lands
    // there even when the line wraps.
    let mut chosen: Option<usize> = None;
    for (i, v) in visual.iter().enumerate() {
        if v.src_row != cur_row {
            if chosen.is_some() {
                break;
            }
            continue;
        }
        let row_end = v.src_col_start + v.width_chars;
        if cur_col < row_end {
            return (i, cur_col - v.src_col_start);
        }
        chosen = Some(i);
    }
    if let Some(i) = chosen {
        let v = &visual[i];
        return (i, cur_col.saturating_sub(v.src_col_start));
    }
    (0, 0)
}

/// Placeholder title for paragraphs added without one. The next save replaces
/// it with the first sentence of the body.
const PARAGRAPH_PLACEHOLDER_TITLE: &str = "Untitled paragraph";

/// Maximum number of characters a node title is allowed to occupy in the
/// tree pane. Beyond that, the title is truncated with an ellipsis so the
/// `Nw` word-count suffix stays visible on a single row.
const TITLE_MAX_DISPLAY: usize = 60;

fn truncate_title(title: &str, max_chars: usize) -> String {
    let chars: Vec<char> = title.chars().collect();
    if chars.len() <= max_chars {
        return title.to_string();
    }
    let mut out: String = chars.iter().take(max_chars - 1).collect();
    out.push('…');
    out
}

/// Render a 4-cell Unicode gauge + percent for a per-paragraph
/// word-count goal. Each cell represents 25% of progress; the
/// last partial cell uses a medium-shade glyph when the gauge
/// is between thresholds. Colour buckets:
///   <25% red, <50% yellow, <75% light-green, ≥100% green-bold,
///   between 75 and 100 green-dim. Returns `(gauge_str, percent_int, style)`.
/// Compute aligned line-by-line diff rows for the F6 → V
/// snapshot-diff modal. Uses `similar`'s `TextDiff::from_lines`
/// (Myers algorithm) to get a sequence of `(tag, line)` chunks,
/// then aligns them into side-by-side rows.
///
/// Heuristic for `Changed` rows: when a Delete is immediately
/// followed by an Insert, fuse the pair into one Changed row
/// rather than rendering them as a removal + an addition on
/// different lines. This is what most diff viewers do.
fn compute_line_diff(left: &str, right: &str) -> Vec<SnapshotDiffRow> {
    use similar::{ChangeTag, TextDiff};
    let diff = TextDiff::from_lines(left, right);
    let mut out: Vec<SnapshotDiffRow> = Vec::new();
    // Walk the change list; on a Delete, peek to see if the
    // next change is an Insert and fuse them.
    let changes: Vec<_> = diff.iter_all_changes().collect();
    let mut i = 0;
    while i < changes.len() {
        let c = &changes[i];
        let text = c
            .value()
            .strip_suffix('\n')
            .unwrap_or(c.value())
            .to_string();
        match c.tag() {
            ChangeTag::Equal => {
                out.push(SnapshotDiffRow {
                    left: Some(text.clone()),
                    right: Some(text),
                    kind: SnapshotDiffKind::Equal,
                });
                i += 1;
            }
            ChangeTag::Delete => {
                // Look ahead for a paired Insert to fuse.
                if let Some(next) = changes.get(i + 1) {
                    if next.tag() == ChangeTag::Insert {
                        let next_text = next
                            .value()
                            .strip_suffix('\n')
                            .unwrap_or(next.value())
                            .to_string();
                        out.push(SnapshotDiffRow {
                            left: Some(text),
                            right: Some(next_text),
                            kind: SnapshotDiffKind::Changed,
                        });
                        i += 2;
                        continue;
                    }
                }
                out.push(SnapshotDiffRow {
                    left: Some(text),
                    right: None,
                    kind: SnapshotDiffKind::Removed,
                });
                i += 1;
            }
            ChangeTag::Insert => {
                out.push(SnapshotDiffRow {
                    left: None,
                    right: Some(text),
                    kind: SnapshotDiffKind::Added,
                });
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests_diff {
    use super::*;

    #[test]
    fn identical() {
        let r = compute_line_diff("a\nb\nc\n", "a\nb\nc\n");
        assert_eq!(r.len(), 3);
        assert!(r.iter().all(|x| x.kind == SnapshotDiffKind::Equal));
    }

    #[test]
    fn pure_add() {
        let r = compute_line_diff("a\n", "a\nb\n");
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].kind, SnapshotDiffKind::Equal);
        assert_eq!(r[1].kind, SnapshotDiffKind::Added);
        assert_eq!(r[1].right.as_deref(), Some("b"));
    }

    #[test]
    fn pure_remove() {
        let r = compute_line_diff("a\nb\n", "a\n");
        assert_eq!(r.len(), 2);
        assert_eq!(r[1].kind, SnapshotDiffKind::Removed);
        assert_eq!(r[1].left.as_deref(), Some("b"));
    }

    #[test]
    fn fused_change() {
        // Single-line rewrite fuses to Changed.
        let r = compute_line_diff("foo\n", "bar\n");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].kind, SnapshotDiffKind::Changed);
        assert_eq!(r[0].left.as_deref(), Some("foo"));
        assert_eq!(r[0].right.as_deref(), Some("bar"));
    }
}

/// Fuzzy-rank `entries` against `query`. Returns indices into
/// the original Vec, ordered by score (descending). Scoring:
///   3 — title starts with query (case-insensitive)
///   2 — title contains query as a substring
///   1 — slug path contains query as a substring
///   0 — excluded
/// Empty query keeps the original ordering and returns all
/// indices (the picker treats this as "no filter applied").
fn fuzzy_filter_entries(
    entries: &[ScriptPickerEntry],
    query: &str,
) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return (0..entries.len()).collect();
    }
    let mut scored: Vec<(i32, usize)> = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        let tl = e.title.to_lowercase();
        let sl = e.slug_path.to_lowercase();
        let score = if tl.starts_with(&q) {
            3
        } else if tl.contains(&q) {
            2
        } else if sl.contains(&q) {
            1
        } else {
            0
        };
        if score > 0 {
            scored.push((score, i));
        }
    }
    // Stable sort by descending score preserves the original
    // (slug-path-sorted) order within each tier.
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().map(|(_, i)| i).collect()
}

/// Render an active-time count in shorthand:
///   <60s   → "0m"
///   <60m   → "Nm"
///   else   → "Hh Mm" (zero-padded minutes)
/// Used by the status-bar widget + Ctrl+V G modal.
fn format_active_duration(seconds: i64) -> String {
    let s = seconds.max(0);
    if s < 60 {
        return "0m".to_string();
    }
    let minutes = s / 60;
    if minutes < 60 {
        return format!("{minutes}m");
    }
    let h = minutes / 60;
    let m = minutes % 60;
    format!("{h}h {m:02}m")
}

#[cfg(test)]
mod tests_active_duration {
    use super::format_active_duration;

    #[test]
    fn under_a_minute_is_zero() {
        assert_eq!(format_active_duration(0), "0m");
        assert_eq!(format_active_duration(45), "0m");
    }

    #[test]
    fn minutes_only() {
        assert_eq!(format_active_duration(60), "1m");
        assert_eq!(format_active_duration(3540), "59m");
    }

    #[test]
    fn hours_with_minutes() {
        assert_eq!(format_active_duration(3600), "1h 00m");
        assert_eq!(format_active_duration(3660), "1h 01m");
        assert_eq!(format_active_duration(7325), "2h 02m");
    }
}

fn format_progress_gauge(current: i64, target: i64) -> (String, i64, Style) {
    if target <= 0 {
        return ("[░░░░]".into(), 0, Style::default());
    }
    let pct = (current.max(0) * 100 / target).clamp(0, 999);
    // 4 cells, eighths-resolution per cell would be cleaner but
    // overkill — full / medium / light glyphs are enough.
    let full_cells = (pct / 25).min(4) as usize;
    let remainder = (pct % 25) as usize;
    let mut gauge = String::with_capacity(6);
    gauge.push('[');
    for _ in 0..full_cells {
        gauge.push('█');
    }
    if full_cells < 4 {
        if remainder >= 12 {
            gauge.push('▒');
        } else if remainder > 0 {
            gauge.push('░');
        } else {
            gauge.push('░');
        }
        for _ in (full_cells + 1)..4 {
            gauge.push('░');
        }
    }
    gauge.push(']');
    let style = if pct >= 100 {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else if pct >= 75 {
        Style::default().fg(Color::LightGreen)
    } else if pct >= 50 {
        Style::default().fg(Color::Yellow)
    } else if pct >= 25 {
        Style::default().fg(Color::LightRed)
    } else {
        Style::default()
            .fg(Color::Red)
            .add_modifier(Modifier::DIM)
    };
    (gauge, pct, style)
}

#[cfg(test)]
mod tests_gauge {
    use super::*;

    #[test]
    fn gauge_zero() {
        let (g, p, _) = format_progress_gauge(0, 100);
        assert_eq!(p, 0);
        assert_eq!(g, "[░░░░]");
    }

    #[test]
    fn gauge_partial() {
        let (_, p, _) = format_progress_gauge(60, 100);
        assert_eq!(p, 60);
    }

    #[test]
    fn gauge_full() {
        let (g, p, _) = format_progress_gauge(100, 100);
        assert_eq!(p, 100);
        assert_eq!(g, "[████]");
    }

    #[test]
    fn gauge_over() {
        let (g, p, _) = format_progress_gauge(250, 100);
        assert_eq!(p, 250);
        assert_eq!(g, "[████]");
    }
}

/// Try to derive a usable title from a paragraph body. Skips Typst heading
/// lines (`= …`), comments (`// …`), and blank lines. Looks for the first
/// `.`, `!`, or `?` followed by whitespace or end-of-text. Truncates the
/// result to fit `TITLE_MAX_DISPLAY` chars (with ellipsis if cut). Returns
/// None if no usable text is found.
/// Lift the lookup term for a Ctrl+B P / Ctrl+B C inference: the
/// selection if any, otherwise the (Unicode) word under the cursor. The
/// word boundaries respect Cyrillic / CJK punctuation via
/// `unicode-segmentation`, so a selection of "Москва" works the same as
/// dropping the cursor inside it. Trailing apostrophes / quotes are
/// trimmed so "King's" doesn't pull in an extra quote.
/// Dispatch the per-line highlight call based on `content_type` —
/// typst (default) goes through the cached tree-sitter highlighter;
/// "hjson" runs the lightweight hand-rolled lexer.
pub fn highlight_for_content(
    highlighter: &mut super::highlight::TypstHighlighter,
    source: &str,
    theme: &super::theme::Theme,
    content_type: Option<&str>,
) -> Vec<Vec<super::highlight::StyledRun>> {
    match content_type {
        Some("hjson") => super::hjson_highlight::highlight_hjson_lines(source, theme),
        Some("bund") => super::bund_highlight::highlight_bund_lines(source, theme),
        _ => highlighter.highlight_lines(source, theme),
    }
}

/// Convert a tui-textarea (row, char-col) cursor into a byte offset
/// inside `source = lines.join("\n")`. Used by the mode detector to
/// query tree-sitter at the cursor's position.
pub fn byte_offset_for_cursor(source: &str, row: usize, col: usize) -> usize {
    let mut byte: usize = 0;
    let mut current_row = 0;
    for line in source.split_inclusive('\n') {
        if current_row == row {
            // Find the byte offset for `col` chars into this line
            // (stop before the newline if any).
            let body = line.strip_suffix('\n').unwrap_or(line);
            return byte
                + body
                    .char_indices()
                    .nth(col)
                    .map(|(b, _)| b)
                    .unwrap_or(body.len());
        }
        byte += line.len();
        current_row += 1;
    }
    source.len()
}

/// Map digit chars '1'..'7' to a status label. Empty for any other
/// char. 1 is the most-advanced status so the typical writer query —
/// "what's actually ready to ship?" — is the lowest-effort chord.
/// Map a digit chord (`1`..`7`) to its workflow-status string.
/// Once used inline by the meta dispatcher; the dispatcher now
/// resolves to `StatusFilter*` actions via the binding table, so
/// this stays only as a unit-test helper for the digit→status
/// mapping itself.
#[cfg(test)]
pub fn digit_to_status(c: char) -> Option<&'static str> {
    match c {
        '1' => Some("Ready"),
        '2' => Some("Final"),
        '3' => Some("Third"),
        '4' => Some("Second"),
        '5' => Some("First"),
        '6' => Some("Napkin"),
        '7' => Some("None"),
        _ => None,
    }
}

/// Apply an editor-style "search match" highlight to the first
/// occurrence of `needle` (case-insensitive) inside a chat-history
/// rendered line. The matched substring gets a dark foreground on a
/// pink background (re-using `search_match_bg` / `search_current_bg`
/// from the theme); surrounding text keeps its original styling.
///
/// Only the FIRST occurrence per line is highlighted — multiple
/// matches on the same line is a UX corner case; the user can hit
/// Ctrl+X to walk to the next line's match either way.
pub fn highlight_substring_in_line(
    line: &mut ratatui::text::Line<'static>,
    needle_lower: &str,
    is_current: bool,
    theme: &super::theme::Theme,
) {
    if needle_lower.is_empty() {
        return;
    }
    let full: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    let lower = full.to_lowercase();
    let Some(byte_pos) = lower.find(needle_lower) else { return };
    let byte_end = byte_pos + needle_lower.len();
    let char_start = full[..byte_pos].chars().count();
    // Use `chars().count()` rather than slicing by needle.len() so
    // we get the count in original-cased chars (UTF-8 case-fold can
    // shift byte lengths).
    let char_end = full[..byte_end].chars().count();

    let highlight_bg = if is_current {
        theme.search_current_bg
    } else {
        theme.search_match_bg
    };
    // Dark text on the pink bg — matches the editor's find-modal
    // colour scheme so the matched word stays legible.
    let mut highlight_style = ratatui::style::Style::default()
        .bg(highlight_bg)
        .fg(theme.pane_bg);
    if is_current {
        highlight_style = highlight_style.add_modifier(ratatui::style::Modifier::BOLD);
    }

    let mut new_spans: Vec<ratatui::text::Span<'static>> = Vec::new();
    let mut cursor: usize = 0;
    for span in line.spans.drain(..) {
        let span_text = span.content.to_string();
        let span_len = span_text.chars().count();
        let span_end = cursor + span_len;
        let overlap_start = char_start.max(cursor);
        let overlap_end = char_end.min(span_end);
        if overlap_start >= overlap_end {
            new_spans.push(ratatui::text::Span::styled(span_text, span.style));
            cursor = span_end;
            continue;
        }
        if overlap_start > cursor {
            let pre: String = span_text
                .chars()
                .take(overlap_start - cursor)
                .collect();
            new_spans.push(ratatui::text::Span::styled(pre, span.style));
        }
        let match_text: String = span_text
            .chars()
            .skip(overlap_start - cursor)
            .take(overlap_end - overlap_start)
            .collect();
        new_spans.push(ratatui::text::Span::styled(match_text, highlight_style));
        if overlap_end < span_end {
            let post: String = span_text
                .chars()
                .skip(overlap_end - cursor)
                .collect();
            new_spans.push(ratatui::text::Span::styled(post, span.style));
        }
        cursor = span_end;
    }
    line.spans = new_spans;
}

/// Document-status workflow ring. `Ctrl+B R` advances through this
/// sequence; the ring wraps back to "None" after "Ready". `None` is
/// represented by both the absence of `status` on the Node and the
/// literal "None" string in the ring — the helpers below collapse
/// the two views.
pub const STATUS_CYCLE: &[&str] = &[
    "None", "Napkin", "First", "Second", "Third", "Final", "Ready",
];

pub fn next_status(current: Option<&str>) -> &'static str {
    let cur = display_status(current);
    let idx = STATUS_CYCLE
        .iter()
        .position(|s| *s == cur)
        .unwrap_or(0);
    STATUS_CYCLE[(idx + 1) % STATUS_CYCLE.len()]
}

pub fn prev_status(current: Option<&str>) -> &'static str {
    let cur = display_status(current);
    let idx = STATUS_CYCLE
        .iter()
        .position(|s| *s == cur)
        .unwrap_or(0);
    STATUS_CYCLE[(idx + STATUS_CYCLE.len() - 1) % STATUS_CYCLE.len()]
}

pub fn display_status(current: Option<&str>) -> &str {
    match current {
        None => "None",
        Some(s) if s.trim().is_empty() => "None",
        Some(s) => s,
    }
}

/// Compact one-character badge for the tree-pane row. The colour
/// (from `status_style`) carries the meaning; the letter just gives
/// the row a visual anchor so the user knows that column means
/// status.
pub fn status_letter(label: &str) -> &'static str {
    match label {
        "Napkin" => "n",
        "First" => "1",
        "Second" => "2",
        "Third" => "3",
        "Final" => "F",
        "Ready" => "R",
        _ => " ",
    }
}

/// Colour the editor header uses for each status — picks from the
/// existing theme palette so users with custom themes keep their
/// preferred hues.
pub fn status_style(label: &str, theme: &super::theme::Theme) -> Style {
    let base = match label {
        "None" => return Style::default().add_modifier(Modifier::DIM),
        "Napkin" => theme.grammar_change_fg,           // red — "rough"
        "First" => theme.ai_scope_fg,                  // peach
        "Second" => theme.characters_fg,               // amber
        "Third" => theme.places_fg,                    // cyan
        "Final" => theme.border_saved,                 // green
        "Ready" => theme.border_saved,                 // green + bold
        _ => return Style::default(),
    };
    let mut style = Style::default().fg(base).add_modifier(Modifier::BOLD);
    if label == "Ready" {
        style = style.add_modifier(Modifier::REVERSED);
    }
    style
}

/// Open-bracket → matching close pair the auto-close logic emits.
/// None for any character that isn't an opener we recognise.
fn open_pair_for(c: char) -> Option<char> {
    match c {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        '"' => Some('"'),
        '\'' => Some('\''),
        _ => None,
    }
}

fn is_close_pair_char(c: char) -> bool {
    matches!(c, ')' | ']' | '}' | '"' | '\'')
}

/// Case-insensitive substring filter over the baked-in typst function
/// table. Results are returned sorted alphabetically (the table is
/// already sorted; we just preserve order across filter steps).
pub fn filter_functions(filter: &str) -> Vec<super::typst_funcs::TypstFn> {
    let needle = filter.trim().to_lowercase();
    if needle.is_empty() {
        return super::typst_funcs::all();
    }
    super::typst_funcs::all()
        .into_iter()
        .filter(|f| f.name.to_lowercase().contains(&needle))
        .collect()
}

/// Detection result for "is the cursor sitting inside the first
/// string argument of a `#image(...)` call on this line".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageCallContext {
    /// True when the open `"` has a matching close `"` further along
    /// the same line. The picker uses this to decide whether to
    /// insert a closing quote after the filename or not.
    pub closing_quote_present: bool,
}

/// Inspect the editor line up to the cursor and decide whether we're
/// inside `#image("…<cursor>…")`. Returns `None` when not, so the
/// caller can fall back to the regular `Ctrl+B P` (Places RAG) path.
///
/// Detection rule (line-local, no tree-sitter required):
///   1. Find the LAST occurrence of `#image(` on the line at-or-before
///      the cursor column.
///   2. Between the `(` and the cursor, there must be no balanced `)`
///      (the call is still open).
///   3. Between the `(` and the cursor, there must be exactly one `"`
///      (we are inside the first string literal).
///
/// Multi-line `#image(...)` calls — rare in practice — are not
/// detected. The line-local scope makes this a 50-line function and
/// the failure mode is "Ctrl+B P falls through to Places RAG" rather
/// than a bug.
pub fn detect_image_call_context(line: &str, cursor_col: usize) -> Option<ImageCallContext> {
    let cursor_byte = char_offset_to_byte(line, cursor_col);
    let prefix = &line[..cursor_byte];
    // Walk backward to find the last `#image(`. Allow whitespace
    // between `image` and `(` (e.g. `#image (` is uncommon but legal).
    let open_paren_idx = find_image_open(prefix)?;
    // After the `(`, count parens + quotes up to the cursor.
    let between = &prefix[open_paren_idx + 1..];
    let mut depth: i32 = 1; // we're inside the open paren
    let mut quotes: usize = 0;
    for c in between.chars() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth <= 0 {
                    return None; // call already closed before cursor
                }
            }
            '"' => quotes += 1,
            _ => {}
        }
    }
    if quotes != 1 {
        return None;
    }
    // Look forward for a closing quote on the same line so the picker
    // can decide whether to add one.
    let suffix = &line[cursor_byte..];
    let closing_quote_present = suffix
        .chars()
        .scan(false, |escape, c| {
            if *escape {
                *escape = false;
                return Some((false, c));
            }
            if c == '\\' {
                *escape = true;
                return Some((false, c));
            }
            Some((c == '"', c))
        })
        .any(|(is_close, _)| is_close);
    Some(ImageCallContext {
        closing_quote_present,
    })
}

fn find_image_open(prefix: &str) -> Option<usize> {
    // Iterate in reverse to find the LAST `#image[whitespace]?(`.
    let bytes = prefix.as_bytes();
    let mut i = bytes.len();
    while i > 0 {
        if bytes[i - 1] == b'(' {
            // Look back for "image" with optional whitespace, and a `#`
            // somewhere before that on this scan window.
            let head = &prefix[..i - 1];
            let trimmed = head.trim_end();
            if let Some(stripped) = trimmed.strip_suffix("image") {
                if stripped.ends_with('#') {
                    return Some(i - 1);
                }
            }
        }
        i -= 1;
    }
    None
}

fn char_offset_to_byte(s: &str, char_off: usize) -> usize {
    s.char_indices()
        .nth(char_off)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

fn current_word_or_selection(doc: &OpenedDoc) -> String {
    if let Some(((r1, c1), (r2, c2))) = doc.textarea.selection_range() {
        return slice_lines(doc.textarea.lines(), r1, c1, r2, c2)
            .trim()
            .to_string();
    }
    let (row, col) = doc.textarea.cursor();
    let lines = doc.textarea.lines();
    let Some(line) = lines.get(row) else {
        return String::new();
    };
    use unicode_segmentation::UnicodeSegmentation;
    for (byte_off, w) in line.unicode_word_indices() {
        let start_col = line[..byte_off].chars().count();
        let end_col = start_col + w.chars().count();
        if col >= start_col && col <= end_col {
            return w.trim_matches(|c: char| c == '\'' || c == '"').to_string();
        }
    }
    String::new()
}

/// Aggregate counts for one root Book, computed by walking its subtree.
/// Words come from each Paragraph's stored `word_count` (kept up to date
/// at save time); sentences are derived by re-reading paragraph bodies
/// from disk, which is fine for literary-scale projects (hundreds of
/// short files) but should be reconsidered if a project ever grows past
/// many thousands of paragraphs.
#[derive(Debug, Default)]
struct BookStats {
    chapters: usize,
    subchapters: usize,
    paragraphs: usize,
    images: usize,
    sentences: usize,
    words: u64,
}

fn compute_book_stats(
    hierarchy: &Hierarchy,
    book: &Node,
    project_root: &Path,
) -> BookStats {
    let mut stats = BookStats::default();
    for id in hierarchy.collect_subtree(book.id) {
        let Some(node) = hierarchy.get(id) else { continue };
        match node.kind {
            NodeKind::Book => {} // the book itself — don't count it as a chapter
            NodeKind::Chapter => stats.chapters += 1,
            NodeKind::Subchapter => stats.subchapters += 1,
            NodeKind::Paragraph => {
                stats.paragraphs += 1;
                stats.words += node.word_count;
                if let Some(rel) = node.file.as_ref() {
                    if let Ok(body) =
                        std::fs::read_to_string(project_root.join(rel))
                    {
                        stats.sentences += count_sentences(&body);
                    }
                }
            }
            NodeKind::Image => stats.images += 1,
            // Bund scripts aren't book content; they don't add to
            // word / sentence counts. Tracking them in their own
            // stats slot is a follow-up.
            NodeKind::Script => {}
        }
    }
    stats
}

/// Count sentence-terminators (`. ! ?`) in prose text, ignoring Typst
/// heading lines (`= ...`) and comments (`// ...`). A run of repeated
/// terminators (e.g. `...` or `?!`) counts as one sentence. The result
/// is an estimate — Typst markup like `#image(...)` and inline math can
/// confuse it — but it's good enough for a UI read-out and consistent
/// across runs.
fn count_sentences(content: &str) -> usize {
    let mut count = 0;
    let mut in_run = false;
    for line in content.lines() {
        let t = line.trim_start();
        if t.is_empty() || t.starts_with('=') || t.starts_with("//") {
            in_run = false;
            continue;
        }
        for c in t.chars() {
            if matches!(c, '.' | '!' | '?') {
                if !in_run {
                    count += 1;
                    in_run = true;
                }
            } else {
                in_run = false;
            }
        }
        in_run = false;
    }
    count
}

/// Compact reading-time estimate for the editor header. 250 wpm
/// (educational adult silent-reading baseline) rounded up to whole
/// minutes; short paragraphs collapse to `<1m`. Matches the per-book
/// figure in the Ctrl+B I info panel — same constant, same rounding.
fn format_reading_time(words: usize) -> String {
    if words == 0 {
        return "<1m".to_string();
    }
    let minutes = ((words as f64) / 250.0).ceil() as u64;
    if minutes < 60 {
        format!("~{minutes}m")
    } else {
        let h = minutes / 60;
        let m = minutes % 60;
        if m == 0 {
            format!("~{h}h")
        } else {
            format!("~{h}h {m}m")
        }
    }
}

/// Format a `Duration` as a coarse "N units ago" string using only the
/// largest two units (days+hours, hours+minutes, etc.). humantime's
/// default formatter prints every non-zero unit down to nanoseconds,
/// which is too noisy for a "how old is this PDF" read-out.
fn format_age_humantime(dur: std::time::Duration) -> String {
    let total_secs = dur.as_secs();
    if total_secs < 60 {
        return format!("{total_secs}s");
    }
    let days = total_secs / 86_400;
    let hours = (total_secs % 86_400) / 3600;
    let minutes = (total_secs % 3600) / 60;
    if days > 0 {
        if hours > 0 {
            format!("{days}d {hours}h")
        } else {
            format!("{days}d")
        }
    } else if hours > 0 {
        if minutes > 0 {
            format!("{hours}h {minutes}m")
        } else {
            format!("{hours}h")
        }
    } else {
        format!("{minutes}m")
    }
}

/// Rewrite `<block>.<key> = <value_lit>` in an existing HJSON config
/// file in place, preserving every other byte — comments, key
/// ordering, indentation, trailing comments on the rewritten line.
/// Returns the new file contents. The strategy is a targeted text
/// edit (no full re-serialisation) so the carefully-annotated default
/// HJSON template survives an update.
///
/// `value_lit` is the literal text to write (already quoted /
/// formatted by the caller — e.g. `"ollama"` for a string, `true` for
/// a bool). When the key isn't present we insert it right after the
/// opening `{` of the block.
///
/// Returns Err with a human-readable reason when the file shape
/// doesn't match our expectations (no block of that name, unterminated
/// braces). The brace counter doesn't understand HJSON strings — it
/// would miscount a `{` / `}` inside a quoted string. Fine for our
/// shipped template, which uses braces only for nested objects.
fn set_key_in_hjson_block(
    raw: &str,
    block: &str,
    key: &str,
    value_lit: &str,
) -> Result<String, String> {
    let lines: Vec<&str> = raw.split_inclusive('\n').collect();
    if lines.is_empty() {
        return Err("config file is empty".into());
    }

    let block_prefix = format!("{block}:");
    let block_open_idx = lines.iter().position(|l| {
        let trimmed = l.trim_start();
        !trimmed.starts_with("//") && trimmed.starts_with(&block_prefix)
    });
    let block_open_idx = block_open_idx
        .ok_or_else(|| format!("no `{block}:` block found in HJSON"))?;

    // Walk forward tracking brace depth (ignoring `//` line comments)
    // so we know where the block ends.
    let mut depth: i32 = 0;
    let mut block_started = false;
    let mut block_end: Option<usize> = None;
    for (i, line) in lines.iter().enumerate().skip(block_open_idx) {
        let code = line.split("//").next().unwrap_or("");
        for c in code.chars() {
            match c {
                '{' => {
                    depth += 1;
                    block_started = true;
                }
                '}' => depth -= 1,
                _ => {}
            }
        }
        if block_started && depth == 0 {
            block_end = Some(i);
            break;
        }
    }
    let block_end = block_end
        .ok_or_else(|| format!("unterminated `{block}: {{` block — check brace balance"))?;

    // Scan for the target key as a *direct* child of the block
    // (depth == 1 at the time the line starts being read).
    let key_unquoted = format!("{key}:");
    let key_quoted = format!("\"{key}\":");
    let mut depth: i32 = 0;
    let mut target_idx: Option<usize> = None;
    for (i, line) in lines.iter().enumerate().take(block_end + 1).skip(block_open_idx) {
        let depth_before = depth;
        let code = line.split("//").next().unwrap_or("");
        for c in code.chars() {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        if i == block_open_idx {
            continue;
        }
        if depth_before == 1 {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                continue;
            }
            if trimmed.starts_with(&key_unquoted) || trimmed.starts_with(&key_quoted) {
                target_idx = Some(i);
                break;
            }
        }
    }

    let mut out = String::with_capacity(raw.len() + value_lit.len());
    match target_idx {
        Some(idx) => {
            let mut rewrote = false;
            for (i, line) in lines.iter().enumerate() {
                if i == idx {
                    rewrote = true;
                    let (eol, core): (&str, &str) =
                        if let Some(stripped) = line.strip_suffix("\r\n") {
                            ("\r\n", stripped)
                        } else if let Some(stripped) = line.strip_suffix('\n') {
                            ("\n", stripped)
                        } else {
                            ("", *line)
                        };
                    let colon_pos = core.find(':').ok_or_else(|| {
                        format!("`{key}` line missing `:` separator — unexpected HJSON")
                    })?;
                    let head = &core[..=colon_pos]; // includes ":"
                    let tail = &core[colon_pos + 1..];
                    let comment_pos = tail.find("//");
                    let (_old_value, comment_suffix) = match comment_pos {
                        Some(p) => (&tail[..p], &tail[p..]),
                        None => (tail, ""),
                    };
                    if comment_suffix.is_empty() {
                        out.push_str(&format!("{head} {value_lit}{eol}"));
                    } else {
                        // Keep one space between the new value and the
                        // trailing comment so it doesn't slide left.
                        out.push_str(&format!("{head} {value_lit}  {comment_suffix}{eol}"));
                    }
                } else {
                    out.push_str(line);
                }
            }
            if !rewrote {
                return Err("internal error: target line not rewritten".into());
            }
            Ok(out)
        }
        None => {
            // Insert the missing key right after the block-opening
            // line, using two extra spaces of indentation relative to
            // it.
            let block_indent: String = lines[block_open_idx]
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .collect();
            let child_indent = format!("{block_indent}  ");
            for (i, line) in lines.iter().enumerate() {
                out.push_str(line);
                if i == block_open_idx {
                    let eol = if line.ends_with("\r\n") {
                        "\r\n"
                    } else if line.ends_with('\n') {
                        "\n"
                    } else {
                        "\n"
                    };
                    out.push_str(&format!("{child_indent}{key}: {value_lit}{eol}"));
                }
            }
            Ok(out)
        }
    }
}

/// Wrapper that quotes `new_default` if needed and delegates to
/// `set_key_in_hjson_block` for the `llm.default` slot.
fn set_llm_default_in_hjson(raw: &str, new_default: &str) -> Result<String, String> {
    let quote_needed = new_default.is_empty()
        || !new_default
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    let value_lit = if quote_needed {
        format!("\"{}\"", new_default.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        new_default.to_string()
    };
    set_key_in_hjson_block(raw, "llm", "default", &value_lit)
}

/// Set `sound.enabled = true|false` in inkhaven.hjson. Inserts the
/// key (and synthesises the block when missing) when the user has
/// stripped them from an older config.
fn set_sound_enabled_in_hjson(raw: &str, enabled: bool) -> Result<String, String> {
    let value_lit = if enabled { "true" } else { "false" };
    match set_key_in_hjson_block(raw, "sound", "enabled", value_lit) {
        Ok(s) => Ok(s),
        Err(reason) if reason.contains("no `sound:` block") => {
            insert_sound_block_before_root_close(raw, value_lit)
        }
        Err(other) => Err(other),
    }
}

/// Append a fresh `sound: { ... }` block just *inside* the root
/// object's closing `}`. Older configs predating the sound feature
/// don't have the block at all — the toggle synthesises one. The
/// previous version of this helper appended after the file end, which
/// landed the block *outside* the root and broke parsing on next
/// launch.
fn insert_sound_block_before_root_close(raw: &str, value_lit: &str) -> Result<String, String> {
    let lines: Vec<&str> = raw.split_inclusive('\n').collect();
    // Scan backward for the root object's closing brace — the last
    // line whose first non-whitespace character is `}` and whose code
    // (stripped of `//` comments) contains *only* whitespace + `}`.
    let root_close_idx = lines.iter().enumerate().rev().find_map(|(i, l)| {
        let code = l.split("//").next().unwrap_or("");
        let trimmed = code.trim();
        if trimmed == "}" {
            Some(i)
        } else {
            None
        }
    });
    let root_close_idx = root_close_idx.ok_or_else(|| {
        "no root closing `}` found — file shape unrecognised".to_string()
    })?;

    let block = format!(
        "\n  // Typewriter SFX (Ctrl+B E to toggle).\n  sound: {{\n    enabled: {value_lit}\n    volume: 0.6\n  }}\n"
    );

    let mut out = String::with_capacity(raw.len() + block.len());
    for (i, line) in lines.iter().enumerate() {
        if i == root_close_idx {
            out.push_str(&block);
        }
        out.push_str(line);
    }
    Ok(out)
}

/// Split a file body into editor lines, normalising CRLF (`\r\n`) and
/// bare CR (`\r`) line endings to LF first so trailing `\r` bytes never
/// survive into the textarea (where ratatui would render them as
/// control glyphs that look like vertical bars and visually offset the
/// following characters). Triggered by Windows / DOS / old-Mac text
/// dumps the user might import (e.g. the RFC corpus).
fn body_to_lines(body: &str) -> Vec<String> {
    if body.is_empty() {
        return vec![String::new()];
    }
    // CRLF first so we don't double-split, then any remaining bare CR
    // (pre-OS-X Mac files). After this every line break is one `\n`.
    let normalised = body.replace("\r\n", "\n").replace('\r', "\n");
    normalised.split('\n').map(String::from).collect()
}

fn extract_first_sentence(content: &str) -> Option<String> {
    let prose: String = content
        .lines()
        .filter_map(|l| {
            let t = l.trim();
            if t.is_empty() || t.starts_with("=") || t.starts_with("//") {
                None
            } else {
                Some(t.to_string())
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    if prose.is_empty() {
        return None;
    }

    let chars: Vec<char> = prose.chars().collect();
    let mut end = chars.len();
    for (i, c) in chars.iter().enumerate() {
        if matches!(*c, '.' | '!' | '?') {
            let next_is_space_or_end = i + 1 >= chars.len() || chars[i + 1].is_whitespace();
            if next_is_space_or_end {
                end = i + 1;
                break;
            }
        }
    }
    let sentence: String = chars.iter().take(end).collect();
    let sentence = sentence.trim();
    if sentence.is_empty() {
        return None;
    }

    let s_chars: Vec<char> = sentence.chars().collect();
    if s_chars.len() > TITLE_MAX_DISPLAY {
        let mut out: String = s_chars.iter().take(TITLE_MAX_DISPLAY - 1).collect();
        out.push('…');
        Some(out)
    } else {
        Some(sentence.to_string())
    }
}

/// Render one Quick reference entry as a single Line, sized to fit
/// `col_w` terminal cells. Headers get cyan-bold styling; regular entries
/// get a fixed 14-char key column followed by the description.
fn format_entry_line(e: &quickref::Entry, col_w: usize) -> Line<'static> {
    if e.is_header {
        let text = if e.key.is_empty() {
            String::new()
        } else {
            format!(" {}", e.key)
        };
        return Line::from(Span::styled(
            truncate_to_chars(&text, col_w),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    }
    let key_field = 14;
    // Pad/truncate the key to a fixed width so descriptions align.
    let key_padded = pad_or_trim(&e.key, key_field);
    let desc_max = col_w.saturating_sub(key_field + 2);
    let desc = truncate_to_chars(&e.desc, desc_max);
    let line = format!(" {} {}", key_padded, desc);
    Line::from(vec![
        Span::styled(
            format!(" {}", key_padded),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::raw(desc),
    ])
    // Note: `line` above is unused — kept as a clarity comment of the
    // intended width budget. Compiler will eliminate.
    .style(Style::default())
    .alignment(ratatui::layout::Alignment::Left)
    .clone()
    .ok_or_else_unused(line)
}

/// Stub to silence the unused `line` binding above without changing
/// semantics. Compiler should inline to no-op.
trait OkOrElseUnused {
    fn ok_or_else_unused(self, _unused: String) -> Self;
}
impl OkOrElseUnused for Line<'static> {
    fn ok_or_else_unused(self, _unused: String) -> Self {
        self
    }
}

fn pad_or_trim(s: &str, width: usize) -> String {
    let cs: Vec<char> = s.chars().collect();
    if cs.len() >= width {
        cs.iter().take(width).collect()
    } else {
        let mut out: String = cs.iter().collect();
        while out.chars().count() < width {
            out.push(' ');
        }
        out
    }
}

fn truncate_to_chars(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else if max == 0 {
        String::new()
    } else {
        let mut out: String = chars.iter().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// Locate the Places and Characters system books in the loaded hierarchy
/// and compile a fresh `Lexicon` from their nested paragraph titles. Called
/// at startup and after every successful save. Stemmer languages come from
/// `editor.stemming.languages` in the project config. Unknown language
/// names are skipped silently (with a tracing warning) so a typo doesn't
/// break the editor.
fn build_lexicon(hierarchy: &Hierarchy, cfg: &Config) -> super::lexicon::Lexicon {
    use super::lexicon::LexCategory;
    let mut places: Option<Uuid> = None;
    let mut characters: Option<Uuid> = None;
    let mut notes: Option<Uuid> = None;
    let mut artefacts: Option<Uuid> = None;
    for node in hierarchy.iter() {
        match node.system_tag.as_deref() {
            Some(crate::store::SYSTEM_TAG_PLACES) => places = Some(node.id),
            Some(crate::store::SYSTEM_TAG_CHARACTERS) => characters = Some(node.id),
            Some(crate::store::SYSTEM_TAG_NOTES) => notes = Some(node.id),
            Some(crate::store::SYSTEM_TAG_ARTEFACTS) => artefacts = Some(node.id),
            _ => {}
        }
    }
    // Precedence: top-level `language` (when non-empty) wins over the
    // legacy `editor.stemming.languages` list. The former is the one-knob
    // primary setting; the latter stays for power users who want to
    // stem across multiple languages simultaneously.
    let algos: Vec<rust_stemmers::Algorithm> = if !cfg.language.trim().is_empty() {
        match crate::config::parse_stemmer_language(&cfg.language) {
            Some(a) => vec![a],
            None => {
                tracing::warn!(
                    "language `{}` is not a known Snowball algorithm — \
                     stemmer disabled (falling back to exact-phrase matching)",
                    cfg.language
                );
                Vec::new()
            }
        }
    } else {
        cfg.editor
            .stemming
            .languages
            .iter()
            .filter_map(|name| match crate::config::parse_stemmer_language(name) {
                Some(a) => Some(a),
                None => {
                    tracing::warn!(
                        "editor.stemming.languages: unknown language `{name}` — skipped"
                    );
                    None
                }
            })
            .collect()
    };
    // Higher-priority first: Place > Character > Artefact > Note —
    // matches the renderer's overlap precedence so the build-time
    // dedupe and the per-column style picker agree.
    let mut books: Vec<(Uuid, LexCategory)> = Vec::new();
    if let Some(id) = places {
        books.push((id, LexCategory::Place));
    }
    if let Some(id) = characters {
        books.push((id, LexCategory::Character));
    }
    if let Some(id) = artefacts {
        books.push((id, LexCategory::Artefact));
    }
    if let Some(id) = notes {
        books.push((id, LexCategory::Note));
    }
    super::lexicon::Lexicon::build(hierarchy, &books, algos)
}

/// Standard text-input key dispatch: typing, navigation, deletion. Shared
/// helper so new modals don't have to re-implement the pattern that older
/// modals (Add, Rename, FindReplace) inline.
/// One-line status-bar summary of a `bund eval` result. Surfaces
/// captured stdout (from print / println) and the top-of-stack
/// value in a stable order. `context` is an optional preamble —
/// passed when the eval came from a named buffer ("ran `script-
/// name`").
fn format_eval_output(out: &crate::scripting::EvalOutput, context: Option<&str>) -> String {
    let stdout = out.stdout.trim_end().to_string();
    let top_str = out
        .top
        .as_ref()
        .map(crate::scripting::format_value);
    let preamble = match context {
        Some(name) => format!("bund `{name}`"),
        None => "bund".to_string(),
    };
    match (stdout.is_empty(), top_str) {
        (true, Some(v)) => format!("{preamble} → {v}"),
        (false, Some(v)) => format!("{preamble} → {v}  ·  stdout: {stdout}"),
        (false, None) => format!("{preamble} stdout: {stdout}"),
        (true, None) => format!("{preamble}: (no result)"),
    }
}

fn handle_text_input_key(input: &mut TextInput, key: KeyEvent) {
    use KeyCode::*;
    match key.code {
        Backspace => input.backspace(),
        Delete => input.delete(),
        Left => input.move_left(),
        Right => input.move_right(),
        Home => input.move_home(),
        End => input.move_end(),
        Char(c) => {
            let mut residual = key.modifiers;
            residual.remove(KeyModifiers::SHIFT);
            if residual.is_empty() {
                let final_c = if key.modifiers.contains(KeyModifiers::SHIFT)
                    && c.is_ascii_alphabetic()
                {
                    c.to_ascii_uppercase()
                } else {
                    c
                };
                input.insert_char(final_c);
            }
        }
        _ => {}
    }
}

/// Which system book a lexicon-RAG inference draws context from.
/// Picked by the editor-meta chords (`P` Places, `C` Characters,
/// `G` Notes, `Y` Artefacts).
#[derive(Debug, Clone, Copy)]
enum LexiconKind {
    Places,
    Characters,
    Notes,
    Artefacts,
}

impl LexiconKind {
    fn label(self) -> &'static str {
        match self {
            LexiconKind::Places => "Place",
            LexiconKind::Characters => "Character",
            LexiconKind::Notes => "Note",
            LexiconKind::Artefacts => "Artefact",
        }
    }
    fn system_tag(self) -> &'static str {
        match self {
            LexiconKind::Places => crate::store::SYSTEM_TAG_PLACES,
            LexiconKind::Characters => crate::store::SYSTEM_TAG_CHARACTERS,
            LexiconKind::Notes => crate::store::SYSTEM_TAG_NOTES,
            LexiconKind::Artefacts => crate::store::SYSTEM_TAG_ARTEFACTS,
        }
    }
}

/// System prompt used by regular chat when InferenceMode is Local. The
/// model is told to treat any "── … context ──" blocks the user prepends
/// (via the AI scope cycle) as the sole admissible source and to refuse
/// rather than fall back on general knowledge. Empty context just means
/// the conversation itself is the source.
const LOCAL_SYSTEM_PROMPT: &str = "\
You are Inkhaven's writing assistant. The user may prepend `── … context ──` \
blocks (a selection, a paragraph, or whole chapters from their book). When \
present, treat those blocks as the sole admissible source of information. \
Do NOT introduce facts, names, or claims that are absent from the context. \
If the context is insufficient, say so plainly rather than improvising. \
Without an explicit context block, rely only on prior conversation turns; \
do not draw on general knowledge.";

/// System prompt used by regular chat when InferenceMode is Full. Context
/// blocks (when present) are still ground truth, but the model is free to
/// augment with general knowledge — the typical chat / brainstorm mode.
const FULL_SYSTEM_PROMPT: &str = "\
You are Inkhaven's writing assistant. If the user prepends `── … context ──` \
blocks, treat that text as ground truth and prefer it to any conflicting \
general knowledge. Otherwise, answer freely using your general knowledge \
of writing craft, world-building, and the relevant subject matter. Be \
concise; favour short paragraphs and concrete suggestions.";

/// Grammar-check system prompt. Bounds the model to a copy-editor role,
/// keeps Typst markup intact, and emits the final corrected paragraph
/// inside machine-parseable markers (`<<<CORRECTED>>>` / `<<<END>>>`) so
/// the AI pane's `T` action can lift it out without dragging the
/// summary / issue list along.
const GRAMMAR_CHECK_SYSTEM_PROMPT: &str = "\
You are a meticulous copy-editor reviewing a paragraph from a Typst-formatted \
manuscript. Check for grammar, syntax, and punctuation issues only. \
Preserve every Typst markup token verbatim — `= Heading`, `== Subheading`, \
`*bold*`, `_italic_`, `#link(\"…\")[…]`, raw / code blocks, and any other \
Typst-specific syntax must round-trip unchanged. Do not rewrite for style, \
do not change voice, do not propose structural edits unless the original \
sentence is grammatically broken.

Output format (follow exactly):
1. Start with a short summary line (e.g. \"3 grammar issues, 1 punctuation \
issue, otherwise clean\").
2. Then list each issue with the exact original phrase and a suggested \
correction.
3. Finally, emit the fully corrected paragraph between the literal markers \
shown below — nothing else may appear inside the markers, and the markers \
themselves must appear on their own lines:

<<<CORRECTED>>>
(the corrected paragraph, with every Typst markup token preserved)
<<<END>>>

Do not place commentary inside the markers. The editor pipeline will lift \
the text between the markers and overwrite the paragraph buffer with it.";

/// Markers the grammar-check system prompt instructs the model to wrap
/// the corrected paragraph in. Kept as named constants so the parser and
/// the prompt stay in sync.
const CORRECTED_BEGIN: &str = "<<<CORRECTED>>>";
const CORRECTED_END: &str = "<<<END>>>";

/// Fallback prompt body for F7 grammar check when no user-defined
/// `Grammar check` prompt exists in the Prompts book or `prompts.hjson`.
/// The configured `language` from the HJSON drives the grammar rules.
fn grammar_check_default_prompt(language: &str) -> String {
    let lang = if language.trim().is_empty() {
        "English"
    } else {
        language.trim()
    };
    format!(
        "Run a copy-edit pass on the paragraph below. Treat it as {lang} \
prose. Check syntax, agreement, tense, and punctuation; flag anything \
that's grammatically incorrect according to standard {lang} grammar. \
Typst markup may be present — preserve it verbatim in any corrected \
output. After listing issues, give the fully corrected paragraph."
    )
}

/// System prompt for the F1 / "Help!" RAG flow. We force the model to
/// stick to the supplied excerpts so the help feature behaves like a
/// retrieval-grounded manual and not a general LLM chat — it should admit
/// ignorance rather than confabulate Inkhaven features.
const HELP_SYSTEM_PROMPT: &str = "\
You are the Inkhaven help-manual assistant. Your job is to answer the \
user's question about Inkhaven (a Rust TUI literary editor for Typst \
books) using ONLY the Help excerpts the user provides below.

Rules:
- Use only the supplied excerpts. Do not invent commands, keybindings, \
  features, or file paths that are not present in the excerpts.
- Do not fall back on general LLM knowledge or assumptions about other \
  editors. If the excerpts do not answer the question, say so plainly \
  and suggest which area of the Help book might cover it.
- Quote keybindings, command names, and option labels verbatim from the \
  excerpts where useful.
- Be concise. Prefer short paragraphs and bulleted lists. Skip pleasantries.
- If multiple excerpts cover the topic, synthesise them; do not list them \
  as separate answers.
- Plain text only — no markdown headings beyond `#`/`-` lists.";

/// Inclusive on the top-left, exclusive on the bottom-right — matches
/// ratatui's Rect semantics where width/height are spans (the column at
/// `x + width` is one past the rect's last column).
/// Components Inkhaven directly depends on. Each entry is
/// `(crate-name, license, one-line description)`. The list is curated by
/// hand so the credits panel stays readable — auto-pulling every
/// transitive dep from Cargo.lock would dump 200+ rows nobody would
/// scroll through. When you add a new direct dep in Cargo.toml, add it
/// here too.
const CREDITS_COMPONENTS: &[(&str, &str, &str)] = &[
    ("duckdb",                "MIT",             "embedded SQL engine — metadata + blob stores"),
    ("vecstore",              "MIT",             "HNSW vector index — semantic search"),
    ("fastembed",             "Apache-2.0",      "multilingual ONNX text embeddings"),
    ("ratatui",               "MIT",             "TUI rendering framework"),
    ("tui-textarea",          "MIT",             "multi-line text widget (state model)"),
    ("crossterm",             "MIT",             "cross-platform terminal control"),
    ("tree-sitter",           "MIT",             "incremental parser engine"),
    ("tree-sitter-highlight", "MIT",             "syntax-highlight tagging on top of tree-sitter"),
    ("tree-sitter-typst",     "MIT",             "Typst grammar for tree-sitter (uben0)"),
    ("genai",                 "MIT / Apache-2.0", "provider-neutral LLM client (Gemini, DeepSeek, Ollama, OpenAI, …)"),
    ("pulldown-cmark",        "MIT",             "CommonMark parser — markdown rendering in the AI pane"),
    ("rust-stemmers",         "MIT",             "Snowball stemmers — multilingual lexicon overlay"),
    ("unicode-segmentation",  "MIT / Apache-2.0", "Unicode word boundaries"),
    ("regex",                 "MIT / Apache-2.0", "in-buffer find / replace"),
    ("tokio",                 "MIT",             "async runtime"),
    ("tokio-stream",          "MIT",             "Stream adapters for tokio"),
    ("futures-util",          "MIT / Apache-2.0", "futures combinators"),
    ("clap",                  "MIT / Apache-2.0", "CLI parser"),
    ("serde",                 "MIT / Apache-2.0", "serialisation framework"),
    ("serde_json",            "MIT / Apache-2.0", "JSON support for serde"),
    ("serde-hjson",           "MIT",             "HJSON parser — friendly config file format"),
    ("humantime",             "MIT / Apache-2.0", "human-readable duration parsing — backup max_age"),
    ("humantime-serde",       "MIT / Apache-2.0", "serde glue for humantime durations"),
    ("rodio",                 "MIT / Apache-2.0", "audio playback — typewriter SFX (Ctrl+B E)"),
    ("ratatui-image",         "MIT",             "in-TUI image preview — Enter on an Image node"),
    ("image",                 "MIT / Apache-2.0", "image decoder for the preview pane"),
    ("chrono",                "MIT / Apache-2.0", "timestamps, RFC-3339 formatting"),
    ("uuid",                  "Apache-2.0",      "UUIDv7 paragraph IDs"),
    ("zip",                   "MIT",             "backup / restore archive format"),
    ("walkdir",               "MIT / Unlicense", "recursive directory walking"),
    ("arboard",               "MIT / Apache-2.0", "system clipboard access"),
    ("directories",           "MIT / Apache-2.0", "per-user cache path resolution"),
    ("slug",                  "MIT / Apache-2.0", "URL-safe slug generation"),
    ("tracing",               "MIT",             "structured logging"),
    ("tracing-subscriber",    "MIT",             "log filtering and writer config"),
    ("anyhow",                "MIT / Apache-2.0", "error wrapping in application boundaries"),
    ("thiserror",             "MIT / Apache-2.0", "derive macros for typed errors"),
];

/// Build the styled `Line`s the credits modal renders. Returns one Line
/// per row — section headers in cyan-bold, crate names in the configured
/// modal-border colour, descriptions in dim. Each crate row is wrapped
/// to fit a reasonable terminal width; very long descriptions naturally
/// truncate at the right edge of the modal.
fn build_credits_lines(
    theme: &super::theme::Theme,
    engine_summary: &str,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    let bold_accent = Style::default()
        .fg(theme.modal_border)
        .add_modifier(Modifier::BOLD);
    let dim = Style::default().add_modifier(Modifier::DIM);

    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        format!("  Inkhaven v{}", env!("CARGO_PKG_VERSION")),
        bold_accent,
    )]));
    lines.push(Line::from(Span::styled(
        format!("  {}", env!("CARGO_PKG_DESCRIPTION")),
        dim,
    )));
    lines.push(Line::from(""));

    // 1.2.5+: surface the active Typst engine so users can confirm
    // their HJSON setting took effect without going to the logs.
    lines.push(Line::from(vec![Span::styled(
        "  Typst engine".to_string(),
        bold_accent,
    )]));
    lines.push(Line::from(format!("    {engine_summary}")));
    lines.push(Line::from(""));

    lines.push(Line::from(vec![Span::styled(
        "  Author".to_string(),
        bold_accent,
    )]));
    for a in env!("CARGO_PKG_AUTHORS").split(':') {
        if !a.is_empty() {
            lines.push(Line::from(format!("    {a}")));
        }
    }
    lines.push(Line::from(""));

    lines.push(Line::from(vec![Span::styled(
        "  Project".to_string(),
        bold_accent,
    )]));
    lines.push(Line::from(format!(
        "    Repository: {}",
        env!("CARGO_PKG_REPOSITORY")
    )));
    lines.push(Line::from(format!(
        "    Licence:    {}",
        env!("CARGO_PKG_LICENSE")
    )));
    lines.push(Line::from(""));

    lines.push(Line::from(vec![Span::styled(
        "  Components used".to_string(),
        bold_accent,
    )]));
    lines.push(Line::from(Span::styled(
        "  Inkhaven stands on the shoulders of these open-source projects:".to_string(),
        dim,
    )));
    lines.push(Line::from(""));

    // Two-column rendering inside the credits body would be neat but
    // complicates wrapping; a single column with name + licence + tagline
    // reads cleanly even on narrow terminals.
    for (name, license, desc) in CREDITS_COMPONENTS {
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(
                format!("{:<24}", name),
                Style::default()
                    .fg(theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  [{}]", license),
                Style::default().add_modifier(Modifier::DIM),
            ),
        ]));
        lines.push(Line::from(Span::styled(
            format!("        {desc}"),
            dim,
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  And a long tail of transitive dependencies — every one is".to_string(),
        dim,
    )));
    lines.push(Line::from(Span::styled(
        "  listed in `Cargo.lock`. Thanks to every author.".to_string(),
        dim,
    )));
    lines.push(Line::from(""));

    lines
}

fn rect_contains(rect: Rect, col: u16, row: u16) -> bool {
    if rect.width == 0 || rect.height == 0 {
        return false;
    }
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

fn digit_count(n: usize) -> usize {
    let mut x = n.max(1);
    let mut d = 0;
    while x > 0 {
        d += 1;
        x /= 10;
    }
    d
}

/// Allowlist of keystrokes that are non-mutating in the editor pane. Used to
/// gate the editor when an open paragraph lives inside the read-only Help
/// subtree. Anything not listed here is rejected with a status message.
fn is_read_only_safe_key(key: &KeyEvent) -> bool {
    use KeyCode::*;
    // Pure navigation / scrolling / selection — always safe.
    if matches!(
        key.code,
        Left | Right | Up | Down | Home | End | PageUp | PageDown | Esc | Tab | BackTab,
    ) {
        return true;
    }
    // F-keys: F3/F4/F6 are viewers (file picker, split toggle, snapshot
    // picker) — picking a file or snapshot WOULD mutate, but we gate the
    // actual replace in those flows. F5 (new snapshot) and Ctrl+F4 (accept
    // snapshot) mutate, so they're blocked here.
    if matches!(key.code, F(1) | F(3) | F(4) | F(6)) {
        // F4 with Ctrl is "accept snapshot" → block.
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        return !(matches!(key.code, F(4)) && ctrl);
    }
    // Alt+arrows and Alt+C are block-selection / block-copy — safe.
    if key.modifiers.contains(KeyModifiers::ALT) {
        if matches!(key.code, Left | Right | Up | Down) {
            return true;
        }
        if matches!(key.code, Char('c') | Char('C')) {
            return true;
        }
        return false;
    }
    // Ctrl combos: an explicit allowlist of read-safe operations.
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return matches!(
            key.code,
            Char('a') | Char('A')                // select all
            | Char('c') | Char('C')              // copy
            | Char('f') | Char('F')              // find
            | Char('x') | Char('X')              // repeat (next match)
            | Char('s') | Char('S')              // save (no-op + status)
            | Char('h') | Char('H')              // split-scroll up (only effective in split)
            | Char('j') | Char('J')              // split-scroll down
            | Char('q') | Char('Q')              // quit
            | Char('b') | Char('B')              // meta prefix
            | Char('t') | Char('T')              // focus Tree alias
            | Char('1') | Char('2') | Char('3') | Char('4') | Char('5') // focus jumps
            | Char('@') | Char('/')              // Ctrl+2 alternates / search focus
        );
    }
    false
}

fn reverse_chip(fg: Color) -> Style {
    Style::default()
        .bg(fg)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD)
}

/// Extract the substring spanning `(start_row, start_col)..(end_row, end_col)`
/// from a slice of lines, where both ends are char-indexed. Used for pulling
/// the editor selection out for `{{selection}}` substitution.
fn slice_lines(lines: &[String], r1: usize, c1: usize, r2: usize, c2: usize) -> String {
    if r1 >= lines.len() {
        return String::new();
    }
    if r1 == r2 {
        let chars: Vec<char> = lines[r1].chars().collect();
        let s = c1.min(chars.len());
        let e = c2.min(chars.len());
        return chars[s..e].iter().collect();
    }
    let mut out = String::new();
    let first: String = lines[r1].chars().skip(c1).collect();
    out.push_str(&first);
    out.push('\n');
    for r in (r1 + 1)..r2 {
        if let Some(line) = lines.get(r) {
            out.push_str(line);
            out.push('\n');
        }
    }
    if let Some(last_line) = lines.get(r2) {
        let chars: Vec<char> = last_line.chars().collect();
        let e = c2.min(chars.len());
        out.extend(chars[..e].iter());
    }
    out
}
