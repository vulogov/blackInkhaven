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

/// Chat line role — drives the query/response colour distinction.
enum RowKind {
    Header,
    Prompt,
    Response,
    Plain,
}

/// Border style for a pane: the theme's focused colour (bold) when active, the
/// unfocused colour otherwise — a visible, theme-aware selection cue.
pub(super) fn border_style(app: &ResearchApp, pane: Focus) -> Style {
    if app.focus == pane {
        Style::new().fg(app.theme.border_focused).bold()
    } else {
        Style::new().fg(app.theme.border_unfocused)
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

    // The full quick-reference overlay (Ctrl+B h) sits over everything.
    if app.show_help {
        render_help(frame, app, area);
    }
}

/// Ctrl+B h — a full reference of panes' chords and the `/command` namespace.
fn render_help(frame: &mut Frame, app: &ResearchApp, area: Rect) {
    let lines: Vec<Line> = vec![
        Line::from(Span::styled("  Research Assistant — quick reference", Style::new().bold())),
        Line::from(""),
        Line::from(Span::styled("  Global", Style::new().fg(app.theme.ai_scope_fg).bold())),
        Line::from("    Tab / Shift+Tab   cycle panes (Facts tree · query · chat)"),
        Line::from("    F10               cycle RAG mode (Facts+Full · Facts · Full)"),
        Line::from("    Ctrl+B h          this reference     ?  toggle hints bar"),
        Line::from("    Ctrl+Q / Ctrl+C   quit               q  quit (outside text)"),
        Line::from(""),
        Line::from(Span::styled("  Facts tree", Style::new().fg(app.theme.ai_scope_fg).bold())),
        Line::from("    j/k g/G  nav      h/l/Enter fold      Ctrl+P pin (max 3)"),
        Line::from("    n new fact        R rename            c/s new chapter/subchapter"),
        Line::from("    - / D delete (¶/branch)               K/J move up/down"),
        Line::from("    y/x/p  copy / cut / paste across parents"),
        Line::from(""),
        Line::from(Span::styled("  Query prompt", Style::new().fg(app.theme.ai_scope_fg).bold())),
        Line::from("    Enter send        Alt+Enter newline   ↑↓ history (at edges)"),
        Line::from("    ←→ Home/End edit  Esc clear / defocus"),
        Line::from(""),
        Line::from(Span::styled("  Chat", Style::new().fg(app.theme.ai_scope_fg).bold())),
        Line::from("    j/k g/G scroll    Ctrl+F search (n/N matches)"),
        Line::from(""),
        Line::from(Span::styled("  /commands", Style::new().fg(app.theme.ai_scope_fg).bold())),
        Line::from("    /fact \"…\" [→ path]   extract last response → Facts (confirm)"),
        Line::from("    /note \"…\" [→ path]   → Notes (speculative)"),
        Line::from("    /goto facts/path     jump the tree to a node"),
        Line::from("    /diff                similar facts already in the corpus"),
        Line::from("    /verify              confidence-probe the last response"),
        Line::from("    /factcheck           audit the whole corpus (truth + consistency)"),
        Line::from("    /chain a → b → c     sequential research pipeline"),
        Line::from("    /rag /clear /save    switch RAG · clear chat · rename thread"),
        Line::from(""),
        Line::from(Span::styled("  Press any key to close.", Style::new().dim())),
    ];
    let h = (lines.len() as u16 + 2).min(area.height);
    let w = 72u16.min(area.width);
    let modal = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    frame.render_widget(Clear, modal);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help (Ctrl+B h) ")
        .border_style(Style::new().fg(app.theme.border_focused).bold());
    frame.render_widget(Paragraph::new(Text::from(lines)).block(block), modal);
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
    if let Some(ti) = &app.tree_input {
        let overlay = Rect { x: inner.x, y: inner.y, width: inner.width, height: 3.min(inner.height) };
        frame.render_widget(Clear, overlay);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} (Enter · Esc) ", ti.kind.label()));
        frame.render_widget(
            Paragraph::new(Line::from(format!(" {}_", ti.buf))).block(block),
            overlay,
        );
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

    // Build the chat lines as `(text, kind)` so query and response render in
    // distinct colours.
    let mut rows: Vec<(String, RowKind)> = Vec::new();
    for (i, turn) in app.chat_history.iter().enumerate() {
        if i > 0 {
            rows.push((String::new(), RowKind::Plain));
        }
        rows.push((format!("❯ query {}", i + 1), RowKind::Header));
        for l in turn.prompt.split('\n') {
            rows.push((l.to_string(), RowKind::Prompt));
        }
        rows.push((String::new(), RowKind::Plain));
        for l in turn.response.split('\n') {
            rows.push((l.to_string(), RowKind::Response));
        }
        if turn.streaming {
            rows.push(("▌".to_string(), RowKind::Plain));
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

    // Query header + prompt in the theme's accent colour (bold header); the
    // response in the default pane foreground — a clear query/response split.
    let accent = Style::new().fg(app.theme.ai_scope_fg);
    let lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .map(|(i, (text, kind))| {
            if let (Some(q), true) = (&query, match_lines.contains(&i)) {
                highlight_line(app, text, q, current_line == Some(i))
            } else {
                let style = match kind {
                    RowKind::Header => accent.bold(),
                    RowKind::Prompt => accent,
                    RowKind::Response | RowKind::Plain => Style::new().fg(app.theme.pane_fg),
                };
                Line::from(Span::styled(text.clone(), style))
            }
        })
        .collect();

    // Scroll in WRAPPED (visual) rows — the pane word-wraps, so a logical-line
    // count under-measures the height and bottom-anchoring would cut the end.
    let width = content.width as usize;
    let wrapped: Vec<usize> = rows.iter().map(|(t, _)| wrapped_rows(t, width)).collect();
    let total_visual: usize = wrapped.iter().sum();
    let height = content.height as usize;
    let max_scroll = total_visual.saturating_sub(height);
    let top = match current_line {
        Some(l) => {
            let above: usize = wrapped[..l.min(wrapped.len())].iter().sum();
            above.saturating_sub(height / 2).min(max_scroll) as u16
        }
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

/// How many wrapped rows a logical line occupies at `width` columns — a close
/// approximation of ratatui's word-wrapper (greedy word packing, hard-breaking
/// over-long words). Used so the chat scroll measures visual rows, not logical
/// lines. Cyrillic / Latin text is ~1 cell per char, which this assumes.
fn wrapped_rows(text: &str, width: usize) -> usize {
    if width == 0 {
        return 1;
    }
    let mut rows = 1usize;
    let mut col = 0usize;
    for word in text.split(' ') {
        let w = word.chars().count();
        if w == 0 {
            // A space (incl. runs of spaces): consumes one column.
            col += 1;
            if col > width {
                rows += 1;
                col = 0;
            }
            continue;
        }
        let need = if col == 0 { w } else { w + 1 };
        if col + need <= width {
            col += need;
        } else {
            // Wrap to a new row; hard-break a word longer than the width.
            rows += 1;
            if w <= width {
                col = w;
            } else {
                rows += (w - 1) / width;
                col = w % width;
                if col == 0 {
                    col = width;
                }
            }
        }
    }
    rows.max(1)
}

/// Split a line on `query` (case-insensitive), highlighting the matches. The
/// current match line is additionally bold. Match colours come from the theme.
fn highlight_line<'a>(app: &ResearchApp, text: &'a str, query: &str, is_current: bool) -> Line<'a> {
    let base = if is_current { Style::new().bold() } else { Style::new() };
    let bg = if is_current { app.theme.search_current_bg } else { app.theme.search_match_bg };
    let hit = Style::new().bg(bg).fg(app.theme.pane_bg);
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
        Focus::FactsTree => {
            " n:fact c/s:chap/sub R:rename -/D:del K/J:move y/x/p:copy/cut/paste Ctrl+P:pin Tab:chat"
        }
        Focus::QueryPrompt => {
            " ⏎ send · Alt+⏎ newline · ←→↑↓ edit · F10 RAG · Ctrl+B h help · /fact /diff /verify /factcheck …"
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
