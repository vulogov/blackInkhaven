//! Centered "Please wait" overlays used during slow lifecycle
//! moments: project open, startup pulse, backup, import,
//! assembly, take-extras, typst compile. All members are pure
//! free functions (no `App` state) except the orchestrator
//! that owns `Store::open` — every per-frame redraw goes
//! through a `draw_*_splash` here. Extracted from `tui::app`
//! in the 1.2.7 refactor.
//!
//! Caller pattern: spawn the slow work on a worker thread,
//! loop on `terminal.draw(|f| draw_*_splash(...))`, poll
//! crossterm for an abort key, and `try_recv` the result.
//! See [`open_store_with_splash`] for the canonical shape.
//!
//! The lifecycle orchestrators that consult `App` state
//! (`run_startup_pulse_splash`, `run_manual_backup`,
//! `maybe_auto_backup`) live in `tui::app` / `tui::backup_ui`
//! and call into the draw helpers here.

use std::path::Path;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::Terminal;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::Store;

use super::text_utils::format_active_duration;

/// Spinner glyphs used by [`draw_typst_compile_splash`]. The
/// caller advances `spin_idx` each frame so the user can tell
/// the TUI is still alive while the child process churns.
pub(super) const TYPST_COMPILE_SPINNER: &[char] = &[
    '⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏',
];

/// Outcome of [`open_store_with_splash`]. `UserAborted` is
/// reserved for Ctrl+Q during the splash so the main `run`
/// loop can exit cleanly without a backtrace.
pub(super) enum StartupError {
    UserAborted,
    Store(anyhow::Error),
}

/// Spawn `Store::open` on a worker thread and animate a "Please wait" splash
/// while it runs. Returns the opened store, or `UserAborted` if Ctrl+Q is
/// pressed during the splash.
pub(super) fn open_store_with_splash<B: ratatui::backend::Backend>(
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

pub(super) fn draw_splash(
    f: &mut ratatui::Frame,
    project_display: &str,
    spinner: char,
    elapsed_s: u64,
) {
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

/// Render the startup project-pulse splash. The orchestrator
/// (`App::run_startup_pulse_splash`-style) walks the hierarchy
/// to assemble `total_paragraphs` / `by_status` and passes them
/// in so this fn stays pure.
pub(super) fn draw_pulse_splash(
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

/// Centered backup splash. `done_dest` selects the variant:
/// * `None` — backup in flight; render the progress bar.
/// * `Some(Some(path))` — backup done; show the artefact path
///   and the "Press any key…" footer.
/// * `Some(None)` — backup failed; show failure header and the
///   "Press any key…" footer (the status bar carries the error).
pub(super) fn draw_backup_splash(
    f: &mut ratatui::Frame,
    project_display: &str,
    done: usize,
    total: usize,
    done_dest: Option<Option<&Path>>,
) {
    let area = f.area();
    let width = area.width.saturating_sub(8).clamp(50, 90);
    let height: u16 = if done_dest.is_some() { 11 } else { 9 };
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let rect = Rect { x, y, width, height };
    f.render_widget(ratatui::widgets::Clear, rect);
    let (title, border_fg) = match done_dest {
        None => (" Inkhaven · backup ", Color::Cyan),
        Some(Some(_)) => (" Inkhaven · backup · done ", Color::Green),
        Some(None) => (" Inkhaven · backup · failed ", Color::Red),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(
            Style::default()
                .fg(border_fg)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    let (header_text, header_fg) = match done_dest {
        None => (
            "  Performing database backup…".to_string(),
            Color::Yellow,
        ),
        Some(Some(_)) => ("  ✓  Backup complete.".to_string(), Color::Green),
        Some(None) => ("  ✗  Backup failed.".to_string(), Color::Red),
    };

    let mut body: Vec<Line<'static>> = vec![
        Line::from(""),
        Line::from(Span::styled(
            header_text,
            Style::default()
                .fg(header_fg)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("  Project: {project_display}"),
            Style::default().add_modifier(Modifier::DIM),
        )),
    ];
    match done_dest {
        None => {
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
            body.push(Line::from(Span::styled(
                bar,
                Style::default().add_modifier(Modifier::BOLD),
            )));
        }
        Some(Some(p)) => {
            body.push(Line::from(Span::styled(
                format!("  Wrote:   {}", p.display()),
                Style::default().add_modifier(Modifier::DIM),
            )));
            body.push(Line::from(""));
            body.push(Line::from(Span::styled(
                "  Press any key to continue…",
                Style::default().fg(Color::Gray),
            )));
        }
        Some(None) => {
            body.push(Line::from(Span::styled(
                "  See status bar for the error.",
                Style::default().add_modifier(Modifier::DIM),
            )));
            body.push(Line::from(""));
            body.push(Line::from(Span::styled(
                "  Press any key to continue…",
                Style::default().fg(Color::Gray),
            )));
        }
    }
    f.render_widget(Paragraph::new(body).wrap(Wrap { trim: false }), inner);
}

/// Block on a single key event so the user can read the
/// completed backup splash before the TUI redraws. Drain non-key
/// events (mouse, resize) so a stray scroll doesn't dismiss; on
/// resize, redraw and keep waiting.
pub(super) fn wait_for_any_key_on_backup_splash<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    project_display: &str,
    done: usize,
    total: usize,
    done_dest: Option<&Path>,
) {
    let done_flag = Some(done_dest);
    let _ = terminal.draw(|f| {
        draw_backup_splash(f, project_display, done, total, done_flag)
    });
    loop {
        match crossterm::event::read() {
            Ok(crossterm::event::Event::Key(_)) => break,
            Ok(crossterm::event::Event::Resize(_, _)) => {
                let _ = terminal.draw(|f| {
                    draw_backup_splash(f, project_display, done, total, done_flag)
                });
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
}

/// Render the centered "Importing directory" splash for the tree-pane
/// directory import. Mirrors `draw_backup_splash` but adds a third line
/// showing the file currently being imported so the user can see the
/// walk advance through the tree.
pub(super) fn draw_import_splash(
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
pub(super) fn draw_assembly_splash(
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

/// 1.2.6+ — splash that ticks through the configured
/// `output.extra_formats` during a Ctrl+B O take. Each
/// format gets ✓ (done), ▶ (in flight), or · (pending);
/// the body lists every format so the user can see what's
/// coming.
pub(super) fn draw_take_extras_splash(
    f: &mut ratatui::Frame,
    book_display: &str,
    current_idx: usize,
    formats: &[String],
    statuses: &[char],
) {
    let area = f.area();
    let height: u16 = (formats.len() as u16).saturating_add(7).min(20);
    let width = area.width.saturating_sub(8).clamp(50, 100);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let rect = Rect { x, y, width, height };
    f.render_widget(ratatui::widgets::Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Inkhaven · Take · extra formats ")
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    let mut body: Vec<Line<'_>> = Vec::with_capacity(formats.len() + 4);
    body.push(Line::from(""));
    body.push(Line::from(Span::styled(
        "  Writing extra formats alongside the PDF…".to_string(),
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )));
    body.push(Line::from(""));
    body.push(Line::from(Span::styled(
        format!("  Book: {book_display}"),
        Style::default().add_modifier(Modifier::DIM),
    )));
    body.push(Line::from(""));
    for (i, fmt) in formats.iter().enumerate() {
        let marker = statuses.get(i).copied().unwrap_or('·');
        let style = match marker {
            '✓' => Style::default().fg(Color::Green),
            '▶' => Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            '✗' => Style::default().fg(Color::Red),
            _ => Style::default().add_modifier(Modifier::DIM),
        };
        let highlight = if i == current_idx {
            "  ▶ "
        } else {
            "    "
        };
        body.push(Line::from(Span::styled(
            format!("{highlight}{marker}  {fmt}"),
            style,
        )));
    }
    f.render_widget(Paragraph::new(body).wrap(Wrap { trim: false }), inner);
}

/// Splash for the `typst compile` step (Ctrl+B B / Ctrl+B O). Static
/// "Please wait" body plus an animated spinner the caller advances
/// each frame so the user can tell the TUI is still alive while the
/// child process churns.
///
/// `done` selects the variant:
/// * `None` — compile is still running; header reads
///   "Please wait while PDF is generated…" and footer "Press Esc to cancel."
/// * `Some(true)` — compile succeeded; header reads "Build complete." and
///   footer "Press any key to continue…" (toggle:
///   `typst_compile.wait_for_key_after_compile`).
/// * `Some(false)` — compile failed; header reads "Build failed." and the
///   footer prompts the same way before the AI error-analysis chat opens.
pub(super) fn draw_typst_compile_splash(
    f: &mut ratatui::Frame,
    book_display: &str,
    engine_label: &str,
    elapsed_secs: u64,
    spinner: char,
    done: Option<bool>,
) {
    let area = f.area();
    let width = area.width.saturating_sub(8).clamp(50, 100);
    let height: u16 = 11;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let rect = Rect { x, y, width, height };
    f.render_widget(ratatui::widgets::Clear, rect);
    let (title, border_fg) = match done {
        None => (" Inkhaven · typst compile ", Color::Cyan),
        Some(true) => (" Inkhaven · typst compile · done ", Color::Green),
        Some(false) => (" Inkhaven · typst compile · failed ", Color::Red),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(
            Style::default()
                .fg(border_fg)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    let (header_text, header_fg) = match done {
        None => (
            format!("  {spinner}  Please wait while PDF is generated…"),
            Color::Yellow,
        ),
        Some(true) => ("  ✓  Build complete.".to_owned(), Color::Green),
        Some(false) => ("  ✗  Build failed.".to_owned(), Color::Red),
    };
    let footer_text = match done {
        None => "  Press Esc to cancel.",
        Some(_) => "  Press any key to continue…",
    };
    let body = vec![
        Line::from(""),
        Line::from(Span::styled(
            header_text,
            Style::default()
                .fg(header_fg)
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
            footer_text,
            Style::default().fg(Color::Gray),
        )),
    ];
    f.render_widget(Paragraph::new(body).wrap(Wrap { trim: false }), inner);
}
