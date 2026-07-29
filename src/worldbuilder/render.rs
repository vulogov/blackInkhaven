//! WBLD-1 (WB-P0) — the worldbuilder frame.
//!
//! Outer layout: main area (left column ∥ right pane) · hints bar · full-width
//! Query prompt · status bar. The left column splits into the Facts pane over the
//! World pane. WB-P0 draws the empty, focus-aware chrome; panes gain content in
//! WB-P1+.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::store::NodeKind;

use super::app::{FACT_WORLD_TAG, WorldbuilderApp};
use super::focus::{Focus, RightPane};

/// Border style for a pane: bright/bold when focused, dim otherwise.
fn border(app: &WorldbuilderApp, pane: Focus) -> Style {
    if app.focus == pane {
        Style::new().fg(app.theme.border_focused).bold()
    } else {
        Style::new().fg(app.theme.border_unfocused)
    }
}

pub(super) fn render(frame: &mut Frame, app: &WorldbuilderApp) {
    let area = frame.area();
    if area.width < 40 || area.height < 10 {
        frame.render_widget(
            Paragraph::new("Terminal too small — needs at least 40×10."),
            area,
        );
        return;
    }

    let hints_h = if app.show_hints { 1 } else { 0 };
    let outer = Layout::vertical([
        Constraint::Fill(1),       // main area
        Constraint::Length(hints_h), // hints bar
        Constraint::Length(4),     // query prompt (2 rows + border)
        Constraint::Length(1),     // status bar
    ])
    .split(area);

    let split = app.split_ratio.clamp(2, 8) as u32;
    let main = Layout::horizontal([
        Constraint::Ratio(split, 10),
        Constraint::Ratio(10 - split, 10),
    ])
    .split(outer[0]);

    // `z` zooms one left pane to fill the column; otherwise Facts over World.
    match app.zoom {
        Some(Focus::FactsPane) => render_left_tree(frame, app, main[0], Focus::FactsPane),
        Some(Focus::WorldPane) => render_left_tree(frame, app, main[0], Focus::WorldPane),
        _ => {
            let ls = app.left_split.clamp(2, 8) as u32;
            let left = Layout::vertical([
                Constraint::Ratio(ls, 10),
                Constraint::Ratio(10 - ls, 10),
            ])
            .split(main[0]);
            render_left_tree(frame, app, left[0], Focus::FactsPane);
            render_left_tree(frame, app, left[1], Focus::WorldPane);
        }
    }
    render_right_pane(frame, app, main[1]);
    if app.show_hints {
        render_hints(frame, app, outer[1]);
    }
    render_query(frame, app, outer[2]);
    render_status(frame, app, outer[3]);

    // WB-P4 — the shaping-delta confirmation sits above everything.
    if app.hjson_preview.is_some() {
        render_delta_preview(frame, app, area);
    }
}

/// The `/`-command delta preview: shows the pending edit(s) for y/n confirmation.
fn render_delta_preview(frame: &mut Frame, app: &WorldbuilderApp, area: Rect) {
    let Some((label, ops)) = &app.hjson_preview else { return };
    let w = (area.width as f32 * 0.7) as u16;
    let h = ((ops.len() as u16) + 6).min(area.height);
    let modal = Rect {
        x: area.x + area.width.saturating_sub(w.max(30)) / 2,
        y: area.y + area.height.saturating_sub(h) / 2,
        width: w.max(30),
        height: h.max(6),
    };
    frame.render_widget(Clear, modal);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Confirm delta → world.hjson ")
        .border_style(Style::new().fg(app.theme.border_focused).bold());
    let inner = block.inner(modal);
    frame.render_widget(block, modal);

    let mut lines: Vec<Line> = vec![Line::from(Span::styled(label.clone(), Style::new().bold()))];
    lines.push(Line::from(""));
    for op in ops {
        lines.push(Line::from(Span::styled(
            format!("  {}", op.preview()),
            Style::new().fg(app.theme.ai_scope_fg),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "y accept (into pending) · n/Esc discard · then /write to commit",
        Style::new().dim(),
    )));
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

/// A left tree pane (Facts or World). `◎` marks `fact:world` paragraphs; `·`
/// plain ones; `⊙` (dim) marks `realworld`-compiler-owned World chapters; `⬡`
/// prefixes pinned rows. The cursor row is reversed.
fn render_left_tree(frame: &mut Frame, app: &WorldbuilderApp, area: Rect, pane: Focus) {
    let is_facts = pane == Focus::FactsPane;
    let (tree, pins, title) = if is_facts {
        (&app.facts_tree, &app.facts_pins, " Facts ")
    } else {
        (&app.world_tree, &app.world_pins, " World ")
    };
    let title = if is_facts && app.facts_filter_world {
        " Facts · ◎ only "
    } else {
        title
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border(app, pane));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    for (i, row) in tree.rows().iter().enumerate() {
        let node = app.hierarchy.get(row.id);
        let name = node.map(|n| n.title.clone()).unwrap_or_else(|| "?".to_string());
        let fold = if row.has_children {
            if row.expanded { "▾" } else { "▸" }
        } else {
            " "
        };
        let mut style = Style::new();

        // Kind glyph + dimming.
        let kglyph = if is_facts {
            match node {
                Some(n) if n.kind == NodeKind::Paragraph => {
                    if n.tags.iter().any(|t| t == FACT_WORLD_TAG) {
                        "◎"
                    } else {
                        if app.facts_filter_world {
                            style = style.dim();
                        }
                        "·"
                    }
                }
                _ => " ",
            }
        } else if app.is_world_compiler_owned(row.id) {
            style = style.dim();
            "⊙"
        } else {
            " "
        };

        let pin = if pins.contains(&row.id) { "⬡" } else { " " };
        let indent = "  ".repeat(row.depth);
        let text = format!("{pin}{indent}{fold} {kglyph} {name}");
        if i == tree.cursor {
            style = style.add_modifier(Modifier::REVERSED);
        }
        lines.push(Line::from(Span::styled(text, style)));
    }
    if lines.is_empty() {
        let empty = if is_facts {
            "(no Facts book yet)"
        } else {
            "(no World book — run the interview or /compile)"
        };
        lines.push(Line::from(Span::styled(empty, Style::new().dim())));
    }
    frame.render_widget(
        Paragraph::new(lines).scroll((tree.scroll as u16, 0)),
        inner,
    );
}

fn render_right_pane(frame: &mut Frame, app: &WorldbuilderApp, area: Rect) {
    let title = format!(" {} ", app.right_pane.title());
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .title(Line::from(Span::styled(" Ctrl+R cycles ", Style::new().dim())).right_aligned())
        .border_style(border(app, Focus::RightPane));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    match app.right_pane {
        RightPane::Chat => render_chat(frame, app, inner),
        RightPane::Research => frame.render_widget(
            Paragraph::new(Span::styled(
                "(real-world research sub-mode — WB-P7)",
                Style::new().dim(),
            )),
            inner,
        ),
        RightPane::Map => frame.render_widget(
            Paragraph::new(Span::styled("(map render — WB-P6)", Style::new().dim())),
            inner,
        ),
        RightPane::Ledger => frame.render_widget(
            Paragraph::new(Span::styled("(magic ledger editor — WB-P9)", Style::new().dim())),
            inner,
        ),
    }
}

/// The streaming worldbuilding conversation.
fn render_chat(frame: &mut Frame, app: &WorldbuilderApp, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();
    for turn in &app.chat {
        lines.push(Line::from(Span::styled(
            "[You]",
            Style::new().fg(app.theme.ai_scope_fg).bold(),
        )));
        for l in turn.prompt.lines() {
            lines.push(Line::from(l.to_string()));
        }
        lines.push(Line::from(""));
        let hdr = if turn.streaming {
            "[World Builder — …]"
        } else {
            "[World Builder]"
        };
        lines.push(Line::from(Span::styled(hdr, Style::new().bold())));
        for l in turn.response.lines() {
            lines.push(Line::from(l.to_string()));
        }
        lines.push(Line::from(""));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "Ask the World Builder a question, or shape the world with / commands (WB-P4).",
            Style::new().dim(),
        )));
    }
    // `u16::MAX` scroll pins to the bottom (used while streaming).
    let total = lines.len() as u16;
    let scroll = if app.chat_scroll == u16::MAX {
        total.saturating_sub(area.height)
    } else {
        app.chat_scroll
    };
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((scroll, 0)),
        area,
    );
}

fn render_query(frame: &mut Frame, app: &WorldbuilderApp, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Query ")
        .border_style(border(app, Focus::QueryPrompt));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(&app.query, inner);
}

fn render_hints(frame: &mut Frame, app: &WorldbuilderApp, area: Rect) {
    let hint = match app.focus {
        Focus::FactsPane => {
            "  j/k·move  h/l·fold  Ctrl+P·pin  Ctrl+T·◎tag  Shift+F·filter  z·zoom  Tab·cycle"
        }
        Focus::WorldPane => {
            "  j/k·move  h/l·fold  Ctrl+P·pin  z·zoom  (⊙ chapters are compiler-owned)  Tab·cycle"
        }
        Focus::QueryPrompt => {
            "  type a question or /command  ·  Esc·clear  ·  Tab·cycle  ·  Ctrl+R·right pane"
        }
        _ => "  Tab·cycle  Ctrl+R·right pane  { }·rows  [ ]·cols  ?·hints  Ctrl+Q·quit",
    };
    frame.render_widget(Paragraph::new(Span::styled(hint, Style::new().dim())), area);
}

fn render_status(frame: &mut Frame, app: &WorldbuilderApp, area: Rect) {
    // WB-P3 — plausibility score chip (★ NN ▲/▼) when a world exists.
    let star = match app.plausibility_score {
        Some(s) => {
            let d = app.plausibility_delta_chip();
            if d.is_empty() {
                format!(" · ★ {s}")
            } else {
                format!(" · ★ {s} {d}")
            }
        }
        None => String::new(),
    };
    let left = format!(" worldbuilder · {}{star} · s:{} ", app.world_name(), app.session.slug);
    let right = format!("{} ", app.status);
    let cols = Layout::horizontal([Constraint::Length(left.len() as u16 + 1), Constraint::Fill(1)])
        .split(area);
    frame.render_widget(
        Paragraph::new(Span::styled(left, Style::new().fg(app.theme.ai_scope_fg))),
        cols[0],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(right, Style::new().dim())).right_aligned(),
        cols[1],
    );
}
