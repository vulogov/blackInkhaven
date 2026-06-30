//! RESRCH-1 — rendering (RFC §5). The minimum-width guard, the outer four-region
//! vertical split (main / hints / query / status), and the main horizontal split
//! (Facts tree 40% | AI chat 60%). Active-pane borders are bright (bold),
//! inactive dim — the only focus distinction (consistent with the writing TUI).
//!
//! R-P1 — placeholder pane bodies; later phases render real content.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use super::MIN_WIDTH;
use super::app::ResearchApp;
use super::focus::Focus;

/// Border style for a pane: bright/bold when focused, dim otherwise (RFC §5.3).
pub(super) fn border_style(app: &ResearchApp, pane: Focus) -> Style {
    if app.focus == pane {
        Style::new().bold()
    } else {
        Style::new().dim()
    }
}

pub(super) fn render(frame: &mut Frame, app: &ResearchApp) {
    let area = frame.area();
    if area.width < MIN_WIDTH {
        frame.render_widget(resize_message(area.width), area);
        return;
    }

    let hints_height = if app.show_hints { 1 } else { 0 };
    let outer = Layout::vertical([
        Constraint::Fill(1),              // main area
        Constraint::Length(hints_height), // context-sensitive hints (G11)
        Constraint::Length(4),            // query prompt (2 lines + border)
        Constraint::Length(1),            // status bar
    ])
    .split(area);

    let split = app.split_ratio.clamp(1, 9);
    let main = Layout::horizontal([
        Constraint::Ratio(split, 10),
        Constraint::Ratio(10 - split, 10),
    ])
    .split(outer[0]);

    render_facts_tree(frame, app, main[0]);
    render_ai_chat(frame, app, main[1]);
    if app.show_hints {
        render_hints(frame, app, outer[1]);
    }
    render_query_prompt(frame, app, outer[2]);
    render_status_bar(frame, app, outer[3]);
}

/// Shown when the terminal is narrower than `MIN_WIDTH`.
fn resize_message(width: u16) -> Paragraph<'static> {
    let text = Text::from(vec![
        Line::from(""),
        Line::from(format!(
            "  Terminal too narrow ({width} cols). Research needs ≥{MIN_WIDTH}."
        )),
        Line::from("  Resize the window, or press q / Ctrl+C to quit."),
    ]);
    Paragraph::new(text).wrap(Wrap { trim: false })
}

fn render_facts_tree(frame: &mut Frame, app: &ResearchApp, area: Rect) {
    let pin_count = app.pinned_nodes.len();
    let title = if pin_count > 0 {
        format!(" Facts  [⬡ {pin_count}/{}] ", super::app::DEFAULT_MAX_PINNED)
    } else {
        " Facts ".to_string()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border_style(app, Focus::FactsTree));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.facts_tree.is_empty() {
        let msg = Text::from(vec![
            Line::from(""),
            Line::from("  (empty Facts book)"),
            Line::from("  Press n to add a fact."),
        ]);
        frame.render_widget(Paragraph::new(msg).style(Style::new().dim()), inner);
    } else {
        let rows = app.facts_tree.rows();
        // Keep the cursor visible (simple top-anchored window).
        let height = inner.height as usize;
        let cursor = app.facts_tree.cursor;
        let start = cursor.saturating_sub(height.saturating_sub(1));
        let mut lines: Vec<Line> = Vec::new();
        for (i, row) in rows.iter().enumerate().skip(start).take(height) {
            let node = app.hierarchy.get(row.id);
            let title = node.map(|n| n.title.as_str()).unwrap_or("?");
            let fold = if row.has_children {
                if row.expanded { "▾ " } else { "▸ " }
            } else {
                "• "
            };
            let pin = if app.pinned_nodes.contains(&row.id) { "⬡ " } else { "" };
            let indent = "  ".repeat(row.depth);
            let label = format!("{indent}{fold}{pin}{title}");
            if i == cursor {
                lines.push(Line::from(Span::styled(label, Style::new().bold().reversed())));
            } else {
                lines.push(Line::from(label));
            }
        }
        frame.render_widget(Paragraph::new(Text::from(lines)), inner);
    }

    if let Some(m) = &app.manual {
        render_manual_overlay(frame, m, inner);
    }
}

fn render_manual_overlay(frame: &mut Frame, m: &super::app::ManualEntry, area: Rect) {
    use super::app::ManualStage;
    let h = if m.stage == ManualStage::Title { 3 } else { 8 };
    let overlay = Rect { x: area.x, y: area.y, width: area.width, height: h.min(area.height) };
    frame.render_widget(Clear, overlay);
    let (title, body): (&str, Vec<Line>) = match m.stage {
        ManualStage::Title => (
            " New fact — title ",
            vec![Line::from(format!(" {}_", m.title))],
        ),
        ManualStage::Body => {
            let mut lines: Vec<Line> = vec![Line::from(format!(" {}", m.title.trim())), Line::from("")];
            for l in m.body.split('\n') {
                lines.push(Line::from(format!(" {l}")));
            }
            (" New fact — body (Ctrl+S save · Esc cancel) ", lines)
        }
    };
    let block = Block::default().borders(Borders::ALL).title(title);
    frame.render_widget(Paragraph::new(Text::from(body)).block(block).wrap(Wrap { trim: false }), overlay);
}

fn render_ai_chat(frame: &mut Frame, app: &ResearchApp, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Research · thread: {} ", app.thread.display_name))
        .border_style(border_style(app, Focus::AiChat));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.chat_history.is_empty() {
        let body = Text::from(vec![
            Line::from(""),
            Line::from("  No queries yet — type below and press Enter."),
        ]);
        frame.render_widget(Paragraph::new(body).style(Style::new().dim()), inner);
        return;
    }

    let mut lines: Vec<Line> = Vec::new();
    for (i, turn) in app.chat_history.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(format!("[query {}]", i + 1), Style::new().bold())));
        lines.push(Line::from(turn.prompt.clone()));
        lines.push(Line::from(""));
        for l in turn.response.split('\n') {
            lines.push(Line::from(l.to_string()));
        }
        if turn.streaming {
            lines.push(Line::from(Span::styled("▌", Style::new().dim())));
        }
    }

    // Scroll: chat_scroll counts lines UP from the bottom; clamp so we never
    // scroll past the content (width-independent approximation).
    let total = lines.len();
    let height = inner.height as usize;
    let max_scroll = total.saturating_sub(height);
    let from_bottom = (app.chat_scroll as usize).min(max_scroll);
    let top = max_scroll.saturating_sub(from_bottom) as u16;
    frame.render_widget(
        Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }).scroll((top, 0)),
        inner,
    );

    // The editable insertion confirmation overlay (G1/G2), in the lower area.
    if app.confirmation.is_some() {
        render_confirmation(frame, app, inner);
    }
}

fn render_confirmation(frame: &mut Frame, app: &ResearchApp, area: Rect) {
    use super::app::ConfirmField;
    let Some(c) = &app.confirmation else { return };

    let h = (area.height as i16 / 2).clamp(8, 16) as u16;
    let overlay = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(h),
        width: area.width,
        height: h,
    };
    frame.render_widget(Clear, overlay);

    let path = match c.target.and_then(|id| app.hierarchy.get(id)) {
        Some(n) => app.hierarchy.slug_path(n),
        None => format!("{} (root)", c.book.label()),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Confirm insertion → {} ", c.book.label()));
    let inner = block.inner(overlay);
    frame.render_widget(block, overlay);

    let parts = Layout::vertical([
        Constraint::Length(1), // Title label
        Constraint::Length(1), // Title field
        Constraint::Length(1), // separator
        Constraint::Fill(1),   // Body field
        Constraint::Length(1), // path
        Constraint::Length(1), // action bar
    ])
    .split(inner);

    let title_focused = c.field == ConfirmField::Title;
    frame.render_widget(
        Paragraph::new(Span::styled(
            "Title:",
            if title_focused { Style::new().bold() } else { Style::new().dim() },
        )),
        parts[0],
    );
    frame.render_widget(&c.title, parts[1]);
    frame.render_widget(Paragraph::new(Span::styled("─".repeat(inner.width as usize), Style::new().dim())), parts[2]);
    frame.render_widget(&c.body, parts[3]);
    frame.render_widget(
        Paragraph::new(Span::styled(format!("→ {path}"), Style::new().dim())),
        parts[4],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            "[Tab: field]  [Ctrl+S / Ctrl+Enter: confirm]  [Esc: discard]",
            Style::new().dim(),
        )),
        parts[5],
    );
}

fn render_hints(frame: &mut Frame, app: &ResearchApp, area: Rect) {
    // Context-sensitive content arrives in R-P17; R-P1 shows a static line.
    let hint = match app.focus {
        Focus::FactsTree => " Tab:chat  n:new fact  Ctrl+P:pin  ?:help  q:quit",
        Focus::QueryPrompt => " Tab:tree  Enter:send  ↑↓:history  ?:help  q:quit",
        Focus::AiChat => " Tab:query  Ctrl+F:search  j/k:scroll  ?:help  q:quit",
        Focus::ConfirmationOverlay => " Tab:field  Ctrl+Enter:confirm  Esc:discard",
    };
    frame.render_widget(Paragraph::new(hint).style(Style::new().dim()), area);
}

fn render_query_prompt(frame: &mut Frame, app: &ResearchApp, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Query ")
        .border_style(border_style(app, Focus::QueryPrompt));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(&app.query, inner);
}

fn render_status_bar(frame: &mut Frame, app: &ResearchApp, area: Rect) {
    let text = match &app.status_message {
        Some(msg) => format!("  {msg}"),
        None => format!(
            "  [RAG: {}]  [~${:.3}]  [?:help  q:quit]",
            app.thread.rag_mode.label(),
            app.session_cost,
        ),
    };
    frame.render_widget(Paragraph::new(text).style(Style::new().dim()), area);
}
