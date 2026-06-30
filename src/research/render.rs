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
    let min_width = app.cfg.research.min_width.max(40);
    if area.width < min_width {
        frame.render_widget(resize_message(area.width, min_width), area);
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

/// Shown when the terminal is narrower than the configured minimum.
fn resize_message(width: u16, min_width: u16) -> Paragraph<'static> {
    let text = Text::from(vec![
        Line::from(""),
        Line::from(format!(
            "  Terminal too narrow ({width} cols). Research needs ≥{min_width}."
        )),
        Line::from("  Resize the window, or press q / Ctrl+C to quit."),
    ]);
    Paragraph::new(text).wrap(Wrap { trim: false })
}

fn render_facts_tree(frame: &mut Frame, app: &ResearchApp, area: Rect) {
    let pin_count = app.pinned_nodes.len();
    let title = if pin_count > 0 {
        format!(" Facts  [⬡ {pin_count}/{}] ", app.cfg.research.max_pinned_nodes)
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

    // G8 — when search is open, reserve a top bar for it.
    let (search_bar, content) = match &app.chat_search {
        Some(_) => {
            let parts = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).split(inner);
            (Some(parts[0]), parts[1])
        }
        None => (None, inner),
    };

    // Build the chat lines as `(text, is_header)`.
    let mut rows: Vec<(String, bool)> = Vec::new();
    for (i, turn) in app.chat_history.iter().enumerate() {
        if i > 0 {
            rows.push((String::new(), false));
        }
        rows.push((format!("[query {}]", i + 1), true));
        rows.push((turn.prompt.clone(), false));
        rows.push((String::new(), false));
        for l in turn.response.split('\n') {
            rows.push((l.to_string(), false));
        }
        if turn.streaming {
            rows.push(("▌".to_string(), false));
        }
    }

    // G8 — match lines (case-insensitive) and the current ordinal.
    let query = app.chat_search.as_ref().map(|s| s.query.to_lowercase()).filter(|q| !q.is_empty());
    let match_lines: Vec<usize> = match &query {
        Some(q) => rows.iter().enumerate().filter(|(_, (t, _))| t.to_lowercase().contains(q)).map(|(i, _)| i).collect(),
        None => Vec::new(),
    };
    let current_line: Option<usize> = if match_lines.is_empty() {
        None
    } else {
        app.chat_search.as_ref().map(|s| match_lines[s.current % match_lines.len()])
    };

    let lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .map(|(i, (text, is_header))| {
            if let (Some(q), true) = (&query, match_lines.contains(&i)) {
                highlight_line(text, q, current_line == Some(i))
            } else if *is_header {
                Line::from(Span::styled(text.clone(), Style::new().bold()))
            } else {
                Line::from(text.clone())
            }
        })
        .collect();

    // Scroll: search jumps to the current match; otherwise bottom-anchored.
    let total = lines.len();
    let height = content.height as usize;
    let max_scroll = total.saturating_sub(height);
    let top = match current_line {
        Some(l) => l.saturating_sub(height / 2).min(max_scroll) as u16,
        None => {
            let from_bottom = (app.chat_scroll as usize).min(max_scroll);
            max_scroll.saturating_sub(from_bottom) as u16
        }
    };
    frame.render_widget(
        Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }).scroll((top, 0)),
        content,
    );

    if let (Some(bar), Some(search)) = (search_bar, &app.chat_search) {
        let count = match_lines.len();
        let pos = if count == 0 { 0 } else { (search.current % count) + 1 };
        let label = format!(" /{}  ({pos}/{count})  n·N next/prev  Esc close", search.query);
        frame.render_widget(Paragraph::new(label).style(Style::new().dim()), bar);
    }

    // The editable insertion confirmation overlay (G1/G2), in the lower area.
    if app.confirmation.is_some() {
        render_confirmation(frame, app, inner);
    }
}

/// Split a line on `query` (case-insensitive), highlighting the matches. The
/// current match line is additionally bold.
fn highlight_line<'a>(text: &'a str, query: &str, is_current: bool) -> Line<'a> {
    let base = if is_current { Style::new().bold() } else { Style::new() };
    let hit = Style::new().bg(ratatui::style::Color::Yellow).fg(ratatui::style::Color::Black);
    let mut spans: Vec<Span> = Vec::new();
    let lower = text.to_lowercase();
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find(query) {
        let start = from + rel;
        let end = start + query.len();
        if start > from {
            spans.push(Span::styled(text[from..start].to_string(), base));
        }
        spans.push(Span::styled(text[start..end].to_string(), hit));
        from = end;
    }
    if from < text.len() {
        spans.push(Span::styled(text[from..].to_string(), base));
    }
    Line::from(spans)
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
    // G11 — context-sensitive per focus (RFC §20).
    let hint = match app.focus {
        Focus::FactsTree => " Tab:chat  n:new fact  Ctrl+P:pin  j/k:nav  Enter:expand  ?:help  q:quit",
        Focus::QueryPrompt => {
            " Tab:tree  Enter:send  ↑↓:history  F10:RAG  /fact /note /goto /diff /verify /chain"
        }
        Focus::AiChat => " Tab:query  Ctrl+F:search  j/k:scroll  g/G:top/bottom  ?:help",
        Focus::ConfirmationOverlay => " Tab:field  Ctrl+S / Ctrl+Enter:confirm  Esc:discard",
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
    // A transient message overrides the middle segments (RFC §21).
    if let Some(msg) = &app.status_message {
        frame.render_widget(Paragraph::new(format!("  {msg}")).style(Style::new().dim()), area);
        return;
    }
    let mut text = format!("  [RAG: {}]  [~${:.3}]", app.thread.rag_mode.label(), app.session_cost);
    // Pinned-node segment (G4) — abbreviated titles.
    for id in app.pinned_nodes.iter() {
        if let Some(node) = app.hierarchy.get(*id) {
            let t: String = node.title.chars().take(15).collect();
            text.push_str(&format!("  [⬡ {t}]"));
        }
    }
    text.push_str("  [?:help  q:quit]");
    frame.render_widget(Paragraph::new(text).style(Style::new().dim()), area);
}
