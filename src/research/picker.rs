//! RESRCH-1 (R-P3) — the thread picker (RFC §18.2). Shown at launch when no
//! `--thread` was given and the project has more than one thread. Resolves which
//! thread to open before the main research app is constructed, so `ResearchApp`
//! always starts with a thread in hand.
//!
//! Launch rules: 0 threads → `default` (created on open); 1 → open it; 2+ →
//! this picker. `Enter` opens the highlighted thread, `n` creates a new one
//! (name prompt), `d` deletes (with confirmation), `Esc` cancels and exits.

use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::project::ProjectLayout;

use super::thread::{self, ThreadSummary};

/// Resolve the thread to open. `Ok(Some(name))` → open/create that thread;
/// `Ok(None)` → the user cancelled (exit `inkhaven research`).
pub(super) fn resolve_thread<B: Backend>(
    terminal: &mut Terminal<B>,
    layout: &ProjectLayout,
    requested: Option<String>,
) -> Result<Option<String>> {
    if let Some(name) = requested {
        return Ok(Some(name));
    }
    let summaries = thread::list_threads(layout);
    match summaries.len() {
        0 => Ok(Some("default".to_string())),
        1 => Ok(Some(summaries[0].display_name.clone())),
        _ => run_picker(terminal, layout, summaries),
    }
}

enum Mode {
    Browse,
    NewName(String),
    ConfirmDelete,
}

struct Picker {
    summaries: Vec<ThreadSummary>,
    cursor: usize,
    mode: Mode,
}

fn run_picker<B: Backend>(
    terminal: &mut Terminal<B>,
    layout: &ProjectLayout,
    summaries: Vec<ThreadSummary>,
) -> Result<Option<String>> {
    let mut p = Picker { summaries, cursor: 0, mode: Mode::Browse };

    loop {
        terminal.draw(|f| draw(f, &p))?;
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let Event::Key(key) = event::read()? else { continue };
        if key.kind == KeyEventKind::Release {
            continue;
        }

        match &mut p.mode {
            Mode::Browse => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => return Ok(None),
                KeyCode::Up | KeyCode::Char('k') => {
                    p.cursor = p.cursor.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if p.cursor + 1 < p.summaries.len() {
                        p.cursor += 1;
                    }
                }
                KeyCode::Enter => {
                    if let Some(s) = p.summaries.get(p.cursor) {
                        return Ok(Some(s.display_name.clone()));
                    }
                }
                KeyCode::Char('n') => p.mode = Mode::NewName(String::new()),
                KeyCode::Char('d') => {
                    if !p.summaries.is_empty() {
                        p.mode = Mode::ConfirmDelete;
                    }
                }
                _ => {}
            },
            Mode::NewName(buf) => match key.code {
                KeyCode::Esc => p.mode = Mode::Browse,
                KeyCode::Enter => {
                    let name = buf.trim().to_string();
                    if !name.is_empty() {
                        return Ok(Some(name));
                    }
                    p.mode = Mode::Browse;
                }
                KeyCode::Backspace => {
                    buf.pop();
                }
                KeyCode::Char(c) => buf.push(c),
                _ => {}
            },
            Mode::ConfirmDelete => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    if let Some(s) = p.summaries.get(p.cursor) {
                        let slug = s.name.clone();
                        let _ = thread::delete_thread(layout, &slug);
                    }
                    p.summaries = thread::list_threads(layout);
                    p.cursor = p.cursor.min(p.summaries.len().saturating_sub(1));
                    p.mode = Mode::Browse;
                    // If the last thread was deleted, fall back to default.
                    if p.summaries.is_empty() {
                        return Ok(Some("default".to_string()));
                    }
                }
                _ => p.mode = Mode::Browse,
            },
        }
    }
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect { x, y, width: w.min(area.width), height: h.min(area.height) }
}

fn draw(frame: &mut ratatui::Frame, p: &Picker) {
    let area = frame.area();
    let h = (p.summaries.len() as u16 + 6).min(area.height);
    let modal = centered(area, 64.min(area.width), h);
    frame.render_widget(Clear, modal);

    let mut lines: Vec<Line> = vec![Line::from("")];
    for (i, s) in p.summaries.iter().enumerate() {
        let date = s.last_active.get(0..10).unwrap_or(&s.last_active);
        let label = format!(" {:<20} {:<11} {:>3} turns  ${:.2}", s.display_name, date, s.turns, s.cost);
        if i == p.cursor {
            lines.push(Line::from(Span::styled(label, Style::new().bold().reversed())));
        } else {
            lines.push(Line::from(label));
        }
    }
    lines.push(Line::from(""));
    let footer = match &p.mode {
        Mode::Browse => " Enter:open  n:new  d:delete  Esc:quit".to_string(),
        Mode::NewName(buf) => format!(" New thread name: {buf}_   (Enter:create  Esc:cancel)"),
        Mode::ConfirmDelete => {
            let name = p.summaries.get(p.cursor).map(|s| s.display_name.as_str()).unwrap_or("");
            format!(" Delete `{name}`? y / N")
        }
    };
    lines.push(Line::from(Span::styled(footer, Style::new().dim())));

    let block = Block::default().borders(Borders::ALL).title(" Research threads ");
    frame.render_widget(Paragraph::new(Text::from(lines)).block(block), modal);
}
