//! RESRCH-1 — rendering (RFC §5). The minimum-width guard, the outer four-region
//! vertical split (main / hints / query / status), and the main horizontal split
//! (Facts tree 40% | AI chat 60%). Active-pane borders are bright (bold),
//! inactive dim — the only focus distinction (consistent with the writing TUI).
//!
//! R-P1 — placeholder pane bodies; later phases render real content.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
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
    /// UX-P5 — a dim rule between turns.
    Separator,
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

/// UX-P2 — the trust-tier badge for a chat turn (label + colour), derived from
/// its provenance-bearing fields. `None` for a command's own output.
fn turn_badge(turn: &super::chat::ChatTurn) -> Option<(String, Style)> {
    let tag = |txt: String, c: Color| Some((format!("[{txt}]"), Style::new().fg(c)));
    if turn.computed {
        return tag("computed".into(), Color::Cyan);
    }
    if turn.simulation {
        return tag("simulation".into(), Color::Green);
    }
    if let Some(qid) = &turn.wikidata {
        return tag(format!("◆ {qid}"), Color::Cyan);
    }
    if let Some(p) = &turn.paper {
        return tag(format!("§ {}", p.source), Color::Magenta);
    }
    if turn.web_grounded {
        return tag("⚠ web".into(), Color::Yellow);
    }
    if !turn.sources.is_empty() {
        return tag("document".into(), Color::Blue);
    }
    // A model answer to a plain question — not a command's own output.
    if !turn.response.is_empty() && !turn.prompt.starts_with('/') {
        return Some(("[? model]".to_string(), Style::new().dim()));
    }
    None
}

/// UX-P5 — render one chat-response line with light markdown styling, reusing the
/// editor's `highlight_markdown_lines`. Markers are preserved (so the visible
/// width is unchanged, keeping the wrapped-scroll math valid); plain runs fall
/// back to the pane foreground so the base colour matches the rest of the chat.
fn md_line<'a>(app: &ResearchApp, text: &str) -> Line<'a> {
    let runs = crate::tui::markdown_highlight::highlight_markdown_lines(text, &app.theme);
    let first = runs.into_iter().next().unwrap_or_default();
    if first.is_empty() {
        return Line::from(String::new());
    }
    let spans: Vec<Span> = first
        .into_iter()
        .map(|r| {
            let style = if r.style.fg.is_none() { r.style.fg(app.theme.pane_fg) } else { r.style };
            Span::styled(r.text, style)
        })
        .collect();
    Line::from(spans)
}

/// UX-P4 — colour an evidence line by its verdict keyword (per-source vote /
/// fact-check verdict) in the confirmation overlay.
fn evidence_line_style(line: &str) -> Style {
    let u = line.to_ascii_uppercase();
    // The agreement summary line ("Agreement: n/m support") is checked first —
    // it contains the word "support" but shouldn't read as a per-source vote.
    if u.trim_start().starts_with("AGREEMENT") {
        Style::new().bold()
    } else if u.contains("CONTRADICT") || u.contains("INACCURATE") {
        Style::new().fg(Color::Red)
    } else if u.contains("DUBIOUS") {
        Style::new().fg(Color::Yellow)
    } else if u.contains("SUPPORT") || u.contains("ACCURATE") {
        Style::new().fg(Color::Green)
    } else {
        Style::new().dim()
    }
}

/// UX-P2 — the Facts-tree source-tier glyph + colour for a provenance origin.
/// `None` for origins with no meaningful tier (e.g. `manual`).
fn provenance_tier_glyph(origin: &str) -> Option<(&'static str, Color)> {
    match origin {
        "computed" | "simulation" => Some(("≡", Color::Green)), // deterministic
        "wikidata" => Some(("◆", Color::Cyan)),                 // structured
        "openalex" | "arxiv" => Some(("§", Color::Magenta)),    // scholarly
        "web" => Some(("◇", Color::Yellow)),                    // web prose
        "document" | "library" => Some(("▪", Color::Blue)),     // imported
        "promoted" => Some(("↑", Color::Blue)),
        "model" => Some(("·", Color::DarkGray)),                // closed-world
        _ => None,
    }
}

/// RE-P5 — colour for a `/factcheck` verdict glyph in the Facts tree.
fn verdict_style(level: super::verdicts::Level) -> Style {
    use super::verdicts::Level;
    let c = match level {
        Level::Accurate => Color::Green,
        Level::Dubious => Color::Yellow,
        Level::Inaccurate => Color::Red,
    };
    Style::new().fg(c)
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

    // UX-P5 — a zoomed pane takes the whole main area; else the normal split.
    match app.zoom {
        Some(Focus::FactsTree) => render_facts_tree(frame, app, outer[0]),
        Some(Focus::AiChat) => render_ai_chat(frame, app, outer[0]),
        _ => {
            let split = app.split_ratio.clamp(1, 9);
            let main = Layout::horizontal([
                Constraint::Ratio(split, 10),
                Constraint::Ratio(10 - split, 10),
            ])
            .split(outer[0]);
            render_facts_tree(frame, app, main[0]);
            render_ai_chat(frame, app, main[1]);
        }
    }
    if app.show_hints {
        render_hints(frame, app, outer[1]);
    }
    render_query_prompt(frame, app, outer[2]);
    render_status_bar(frame, app, outer[3]);

    // UX-P5 — the fact quick-view modal sits above the panes.
    if app.peek.is_some() {
        render_peek(frame, app, area);
    }
    // The full quick-reference overlay (Ctrl+B h) sits over everything.
    if app.show_help {
        render_help(frame, app, area);
    }
}

/// UX-P5 — the fact quick-view modal (Enter on a fact): a scrollable view of the
/// fact's body. `j/k`/arrows scroll · `g/G` top/bottom · `Y` copy · Esc closes.
fn render_peek(frame: &mut Frame, app: &ResearchApp, area: Rect) {
    let Some(p) = &app.peek else { return };
    let w = (area.width as f32 * 0.7) as u16;
    let h = (area.height as f32 * 0.7) as u16;
    let modal = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w.max(20),
        height: h.max(6),
    };
    frame.render_widget(Clear, modal);
    let title: String = p.title.chars().take(modal.width.saturating_sub(6) as usize).collect();
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {title} "))
        .border_style(Style::new().fg(app.theme.border_focused).bold());
    let inner = block.inner(modal);
    frame.render_widget(block, modal);

    // Clamp the scroll to the wrapped body height.
    let text_h = inner.height.saturating_sub(1) as usize;
    let wrapped: usize = p.body.lines().map(|l| wrapped_rows(l, inner.width as usize).max(1)).sum();
    let max_scroll = wrapped.saturating_sub(text_h) as u16;
    let scroll = p.scroll.min(max_scroll);
    let parts = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).split(inner);
    frame.render_widget(
        Paragraph::new(p.body.clone()).wrap(Wrap { trim: false }).scroll((scroll, 0)),
        parts[0],
    );
    let more = if scroll < max_scroll { "  ▼ more" } else { "" };
    frame.render_widget(
        Paragraph::new(Span::styled(format!(" j/k scroll · Y copy · Esc close{more}"), Style::new().dim())),
        parts[1],
    );
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
        Line::from("    < / > resize split (in tree/chat)     Ctrl+Z zoom pane"),
        Line::from("    Ctrl+Q / Ctrl+C   quit               q  quit (outside text)"),
        Line::from(""),
        Line::from(Span::styled("  Facts tree", Style::new().fg(app.theme.ai_scope_fg).bold())),
        Line::from("    j/k g/G  nav      Enter peek / fold   h/l step  Ctrl+P pin"),
        Line::from("    n new fact        R rename            c/s new chapter/subchapter"),
        Line::from("    - / D delete (¶/branch)               K/J move up/down"),
        Line::from("    y/x/p  copy / cut / paste across parents"),
        Line::from("    u  toggle ※ undisputed (authorial)   Y  copy fact body"),
        Line::from(""),
        Line::from(Span::styled("  Query prompt", Style::new().fg(app.theme.ai_scope_fg).bold())),
        Line::from("    Enter send        Alt+Enter newline   ↑↓ history (at edges)"),
        Line::from("    ←→ Home/End edit  Esc clear/defocus  Tab complete /command·path (hints below)"),
        Line::from(""),
        Line::from(Span::styled("  Chat", Style::new().fg(app.theme.ai_scope_fg).bold())),
        Line::from("    j/k g/G scroll    Ctrl+F search (n/N)   y copy last response"),
        Line::from(""),
        Line::from(Span::styled("  /commands", Style::new().fg(app.theme.ai_scope_fg).bold())),
        Line::from("    /fact \"…\" [→ path]   extract last response → Facts (confirm)"),
        Line::from("    /note \"…\" [→ path]   → Notes (speculative)"),
        Line::from("    /goto facts/path     jump the tree to a node"),
        Line::from("    /diff                similar facts already in the corpus"),
        Line::from("    /verify              confidence-probe the last response"),
        Line::from("    /factcheck           audit the corpus (truth + consistency) → tree ✓?✗"),
        Line::from("    /undisputed          common-sense check of ※ authorial facts (no rewrite)"),
        Line::from("    /whatswrong [path]   explain a fact flagged ✗/? (bare: selected fact)"),
        Line::from("    /sources             list each fact's recorded provenance"),
        Line::from("    /import [path]        ingest md/txt/pdf, a folder, or .bib/.json (bare: list)"),
        Line::from("    /forget <name>       remove an imported source"),
        Line::from("    /web [--ingest] q    web search & fetch (chat+factcheck, or ingest)"),
        Line::from("    /wikidata <query>    structured Wikidata triples (Q-ID cited, gate-skipped)"),
        Line::from("    /openalex /arxiv q   scholarly papers (DOI/ID; /fact auto-cites to Sources)"),
        Line::from("    /triangulate [claim] cross-check a claim across the structured sources"),
        Line::from("    /calc <expr>         deterministic calc/units + world.get (→ /fact)"),
        Line::from("    /world [layer]       your World simulation facts (origin=simulation)"),
        Line::from("    /synthesize <topic>  grounded, cited synthesis from your facts"),
        Line::from("    /outline <topic>     structured fact-citing outline (research→writing)"),
        Line::from("    /gaps <topic>        open questions the corpus doesn't answer"),
        Line::from("    /bibliography        Sources Research chapter → BibTeX (y to copy)"),
        Line::from("    /promote [note] [→ p] turn a Note into a verified Fact"),
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
            let mut spans: Vec<Span> = Vec::new();
            // RESRCH-UNDISPUTED — an authorial `※` glyph for a `fact:undisputed`
            // fact (outside the trust ladder; excluded from /factcheck). UD-P4 —
            // coloured by the last `/undisputed` common-sense verdict when present.
            if node.is_some_and(|n| n.tags.iter().any(|t| t == super::UNDISPUTED_TAG)) {
                let style = match app.undisputed_verdicts.level_for(row.id) {
                    Some(level) => verdict_style(level),
                    None => Style::new().fg(Color::Magenta),
                };
                spans.push(Span::styled("※", style));
            }
            // UX-P2 — a permanent provenance-tier glyph (source trust).
            if let Some(rec) = app.fact_provenance.for_node(&row.id.to_string()) {
                if let Some((g, c)) = provenance_tier_glyph(&rec.origin) {
                    spans.push(Span::styled(g.to_string(), Style::new().fg(c)));
                }
            }
            // RE-P5 — a `/factcheck` verdict glyph (✓ / ? / ✗), themed by severity.
            if let Some(v) = app.fact_verdicts.level_for(row.id) {
                spans.push(Span::styled(v.glyph().to_string(), verdict_style(v)));
            }
            if !spans.is_empty() {
                spans.push(Span::raw(" "));
            }
            if i == cursor {
                spans.push(Span::styled(label, Style::new().bold().reversed()));
            } else {
                spans.push(Span::raw(label));
            }
            lines.push(Line::from(spans));
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

    // Build the chat lines as `(text, kind, badge)` so query and response render
    // in distinct colours; the header carries an optional trust-tier badge (UX-P2).
    let mut rows: Vec<(String, RowKind, Option<(String, Style)>)> = Vec::new();
    for (i, turn) in app.chat_history.iter().enumerate() {
        if i > 0 {
            rows.push((String::new(), RowKind::Separator, None));
        }
        rows.push((format!("❯ query {}", i + 1), RowKind::Header, turn_badge(turn)));
        for l in turn.prompt.split('\n') {
            rows.push((l.to_string(), RowKind::Prompt, None));
        }
        rows.push((String::new(), RowKind::Plain, None));
        for l in turn.response.split('\n') {
            rows.push((l.to_string(), RowKind::Response, None));
        }
        if turn.streaming {
            rows.push(("▌".to_string(), RowKind::Plain, None));
        }
    }

    // G8 — match lines (case-insensitive) and the current ordinal.
    let query = app.chat_search.as_ref().map(|s| s.query.to_lowercase()).filter(|q| !q.is_empty());
    let match_lines: Vec<usize> = match &query {
        Some(q) => rows.iter().enumerate().filter(|(_, (t, _, _))| t.to_lowercase().contains(q)).map(|(i, _)| i).collect(),
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
    let width = content.width as usize;
    let lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .map(|(i, (text, kind, badge))| {
            if matches!(kind, RowKind::Separator) {
                return Line::from(Span::styled("─".repeat(width.max(1)), Style::new().dim()));
            }
            if let (Some(q), true) = (&query, match_lines.contains(&i)) {
                highlight_line(app, text, q, current_line == Some(i))
            } else if matches!(kind, RowKind::Response) {
                // UX-P5 — light markdown in responses (bold/italic/code/headings/
                // links/bullets); markers are kept, so wrap-width is unchanged.
                md_line(app, text)
            } else {
                let style = match kind {
                    RowKind::Header => accent.bold(),
                    RowKind::Prompt => accent,
                    RowKind::Response | RowKind::Plain | RowKind::Separator => {
                        Style::new().fg(app.theme.pane_fg)
                    }
                };
                match badge {
                    Some((label, bstyle)) => Line::from(vec![
                        Span::styled(text.clone(), style),
                        Span::raw("  "),
                        Span::styled(label.clone(), *bstyle),
                    ]),
                    None => Line::from(Span::styled(text.clone(), style)),
                }
            }
        })
        .collect();

    // Scroll in WRAPPED (visual) rows — the pane word-wraps, so a logical-line
    // count under-measures the height and bottom-anchoring would cut the end.
    let wrapped: Vec<usize> = rows.iter().map(|(t, _, _)| wrapped_rows(t, width)).collect();
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

    // UX-P5 — a "▼ more" affordance at the bottom-right when scrolled up.
    if (top as usize) < max_scroll && content.height > 0 {
        let label = " ▼ more ";
        let lw = (label.chars().count() as u16).min(content.width);
        let ind = Rect {
            x: content.x + content.width.saturating_sub(lw),
            y: content.y + content.height - 1,
            width: lw,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Span::styled(label, Style::new().fg(app.theme.ai_scope_fg).bold())),
            ind,
        );
    }

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

    // UX-P4 — give the overlay more room when an evidence panel is shown.
    let has_evidence = c.fc_detail.is_some() || c.dup_body.is_some();
    let max_h = if has_evidence { 22 } else { 16 };
    let h = (area.height as i16 * 6 / 10).clamp(8, max_h) as u16;
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
        Constraint::Fill(1),   // Body + (optional) evidence panel
        Constraint::Length(1), // path + provenance preview
        Constraint::Length(1), // action bar
    ])
    .split(inner);

    // UX-P4 — field labels carry a char count; the active field is bold.
    let title_focused = c.field == ConfirmField::Title;
    let title_chars = c.title.lines().join(" ").chars().count();
    let body_chars = c.body.lines().join("\n").chars().count();
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("Title ({title_chars})"),
                if title_focused { Style::new().bold() } else { Style::new().dim() },
            ),
            Span::styled(
                format!("   ·   Body ({body_chars})"),
                if title_focused { Style::new().dim() } else { Style::new().bold() },
            ),
        ])),
        parts[0],
    );
    frame.render_widget(&c.title, parts[1]);
    frame.render_widget(Paragraph::new(Span::styled("─".repeat(inner.width as usize), Style::new().dim())), parts[2]);

    // UX-P4 — when a verdict / near-duplicate is present, split the middle area
    // into the Body field (top) and an evidence panel (bottom) so the reasoning
    // is on-screen, not a status flash.
    let evidence = c.fc_detail.as_deref().filter(|s| !s.trim().is_empty());
    if evidence.is_some() || c.dup_body.is_some() {
        let mid = Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)]).split(parts[3]);
        frame.render_widget(&c.body, mid[0]);
        let mut lines: Vec<Line> = Vec::new();
        if let Some(detail) = evidence {
            lines.push(Line::from(Span::styled("── sources ──", Style::new().dim())));
            for l in detail.lines() {
                lines.push(Line::from(Span::styled(l.to_string(), evidence_line_style(l))));
            }
        } else if let Some(dup) = &c.dup_body {
            lines.push(Line::from(Span::styled("── similar fact already in the corpus ──", Style::new().dim())));
            lines.push(Line::from(Span::styled(dup.clone(), Style::new().fg(app.theme.pane_fg).dim())));
        }
        frame.render_widget(Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true }), mid[1]);
    } else {
        frame.render_widget(&c.body, parts[3]);
    }

    // UX-P4 — the exact provenance tier + citation that will be recorded.
    let prov = if c.book == super::extract::TargetBook::Facts && !c.prov.origin.is_empty() {
        let d = if c.prov.detail.is_empty() { String::new() } else { format!(" · {}", c.prov.detail) };
        format!("   will record: {}{}", c.prov.origin, d)
    } else {
        String::new()
    };
    frame.render_widget(
        Paragraph::new(Span::styled(format!("→ {path}{prov}"), Style::new().dim())),
        parts[4],
    );
    // The action bar — or a near-duplicate warning (T-P4) / fact-check verdict
    // (WC-P3) when present.
    let warn = Style::new().fg(app.theme.border_focused).bold();
    let action = if let Some(w) = &c.dup_warning {
        Span::styled(format!("⚠ {w}"), warn)
    } else if let Some(v) = &c.fc_verdict {
        Span::styled(format!("⚠ fact-check: {v} · Ctrl+S to insert anyway"), warn)
    } else {
        Span::styled(
            "[Tab: field]  [Ctrl+S / Ctrl+Enter: confirm]  [Esc: discard]",
            Style::new().dim(),
        )
    };
    frame.render_widget(Paragraph::new(action), parts[5]);
}

fn render_hints(frame: &mut Frame, app: &ResearchApp, area: Rect) {
    // UX-P1 — while typing a `/command`, the hints bar tracks the input: show
    // matching command names, or the recognised command's usage + summary.
    if matches!(app.focus, Focus::QueryPrompt) {
        let input = app.query_text();
        if input.trim_start().starts_with('/') {
            if let Some(hint) = super::command::hint_for(&input) {
                frame.render_widget(
                    Paragraph::new(format!(" {hint}")).style(Style::new().fg(app.theme.ai_scope_fg)),
                    area,
                );
                return;
            }
        }
    }
    // G11 — context-sensitive per focus (RFC §20).
    let hint = match app.focus {
        Focus::FactsTree => {
            " Enter:peek n:fact c/s:chap R:rename -/D:del K/J:move y/x/p:node u:undisputed(※) Y:copy Ctrl+P:pin"
        }
        Focus::QueryPrompt => {
            " ⏎ send · Alt+⏎ newline · Tab: complete /command·path · F10 RAG · Ctrl+B h help · type / for commands"
        }
        Focus::AiChat => " Tab:query  Ctrl+F:search  j/k:scroll  y:copy  Ctrl+Z:zoom  < >:resize",
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
    // UX-P3 — a braille spinner + elapsed seconds while an async op is in flight.
    let spin = match app.async_started {
        Some(t) => {
            const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let f = FRAMES[app.spin_tick % FRAMES.len()];
            format!("{f} ({}s) ", t.elapsed().as_secs())
        }
        None => String::new(),
    };
    // A transient message overrides the middle segments (RFC §21).
    if let Some(msg) = &app.status_message {
        frame.render_widget(Paragraph::new(format!("  {spin}{msg}")).style(Style::new().dim()), area);
        return;
    }
    // R2-E — `$` when every turn was priced from real provider token usage,
    // `~$` once any turn fell back to the char-length estimate.
    let cost_mark = if app.session_cost_exact { "$" } else { "~$" };
    let mut text = format!(
        "  {spin}[RAG: {}]  [{cost_mark}{:.3}]",
        app.thread.rag_mode.label(),
        app.session_cost
    );
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

#[cfg(test)]
mod ux_tests {
    use super::*;

    #[test]
    fn evidence_line_colours() {
        assert_eq!(evidence_line_style("Wikidata: CONTRADICTS — differs").fg, Some(Color::Red));
        assert_eq!(evidence_line_style("arXiv: SUPPORTS — consistent").fg, Some(Color::Green));
        assert_eq!(evidence_line_style("verdict: DUBIOUS").fg, Some(Color::Yellow));
        assert!(evidence_line_style("Agreement: 2/3 support").add_modifier.contains(ratatui::style::Modifier::BOLD));
    }

    #[test]
    fn tier_glyph_mapping() {
        assert_eq!(provenance_tier_glyph("wikidata").map(|(g, _)| g), Some("◆"));
        assert_eq!(provenance_tier_glyph("arxiv").map(|(g, _)| g), Some("§"));
        assert_eq!(provenance_tier_glyph("computed").map(|(g, _)| g), Some("≡"));
        assert!(provenance_tier_glyph("manual").is_none());
    }
}
