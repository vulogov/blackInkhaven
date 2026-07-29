//! WBLD-1 (WB-P0) — the worldbuilder frame.
//!
//! Outer layout: main area (left column ∥ right pane) · hints bar · full-width
//! Query prompt · status bar. The left column splits into the Facts pane over the
//! World pane. WB-P0 draws the empty, focus-aware chrome; panes gain content in
//! WB-P1+.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use super::app::WorldbuilderApp;
use super::focus::Focus;

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

    let ls = app.left_split.clamp(2, 8) as u32;
    let left = Layout::vertical([
        Constraint::Ratio(ls, 10),
        Constraint::Ratio(10 - ls, 10),
    ])
    .split(main[0]);

    render_pane(frame, app, left[0], Focus::FactsPane, " Facts ");
    render_pane(frame, app, left[1], Focus::WorldPane, " World ");
    render_right_pane(frame, app, main[1]);
    if app.show_hints {
        render_hints(frame, app, outer[1]);
    }
    render_query(frame, app, outer[2]);
    render_status(frame, app, outer[3]);
}

/// A left pane (Facts / World) — WB-P0 draws the bordered box; WB-P1 fills it.
fn render_pane(frame: &mut Frame, app: &WorldbuilderApp, area: Rect, pane: Focus, title: &str) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border(app, pane));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let hint = match pane {
        Focus::FactsPane => "Facts book — world facts (◎) land here",
        _ => "World book — realworld-compiled layers",
    };
    frame.render_widget(
        Paragraph::new(Span::styled(hint, Style::new().dim())),
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
    frame.render_widget(
        Paragraph::new(Span::styled(
            "(worldbuilding conversation appears here — WB-P2)",
            Style::new().dim(),
        )),
        inner,
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

fn render_hints(frame: &mut Frame, _app: &WorldbuilderApp, area: Rect) {
    let hint =
        "  Tab·cycle  { }·resize rows  [ ]·resize cols  Ctrl+R·right pane  ?·hints  Ctrl+Q·quit";
    frame.render_widget(Paragraph::new(Span::styled(hint, Style::new().dim())), area);
}

fn render_status(frame: &mut Frame, app: &WorldbuilderApp, area: Rect) {
    let left = format!(" worldbuilder · {} · s:{} ", app.world_name(), app.session.slug);
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
