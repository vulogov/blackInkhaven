//! Pane-painter methods owned by `App` — every `draw_*` method
//! that paints one of the four (tree / editor / AI / status)
//! main panes, the editor split-snapshot lower half, the AI
//! pane's chat-history and prompt-picker overlays, and the
//! status-bar / footer / search-bar chrome. Sub-module of
//! `tui::app::render`. Extracted from `tui::app::render` in the
//! 1.2.7 refactor, Phase 4 batch 2.
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use super::super::{
    digit_count, find_cursor_visual, format_progress_gauge,
    highlight_for_content, highlight_substring_in_line, reverse_chip,
};


use super::super::super::focus::Focus;
use super::super::super::highlight::{
    build_row_spans, build_visual_row_spans, diff_added, wrap_line, RowHit,
};
use super::super::super::inference::{AiMode, InferenceStatus};
use super::super::super::modal::PromptSource;
use super::super::super::search_replace::{row_matches, RowMatch};
use super::super::super::state::LinkPickDirection;
use super::super::super::status_helpers::status_style;
use super::super::super::text_utils::{
    format_age_humantime, format_reading_time,
};


impl super::super::App {

    /// Render the secondary editor pane (right side, replaces AI
    /// when in similar-paragraph mode). Simpler than draw_editor —
    /// no syntax highlighting, no find/replace overlay, no split
    /// view — but supports a moving cursor so the user can edit.
    /// Focus highlight comes from `self.secondary_focused`, which
    /// is independent of `self.focus` (keystrokes get routed to
    /// secondary by the swap-on-dispatch wrapper in
    /// `handle_editor_key`).
    pub(in crate::tui::app) fn draw_secondary_editor(&mut self, f: &mut ratatui::Frame, area: Rect) {
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
    pub(in crate::tui::app) fn draw_primary_pane_footer(&self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(in crate::tui::app) fn draw_search_bar(&self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(in crate::tui::app) fn draw_ai_prompt(&self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(in crate::tui::app) fn draw_tree(&self, f: &mut ratatui::Frame, area: Rect) {
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
        let width = inner.width as usize;
        let mut scroll = self.tree_scroll;
        if self.tree_cursor < scroll {
            scroll = self.tree_cursor;
        }
        // 1.2.6+: titles wrap rather than truncate, so a single
        // logical row can occupy multiple visual lines. Find the
        // smallest `scroll` such that the rows [scroll..=cursor]
        // fit inside the pane's `height` visual lines. Greedy:
        // walk forward from `scroll`, summing visual heights;
        // advance `scroll` whenever the cumulative total
        // overshoots.
        if height > 0 && width > 0 {
            let mut cumulative = 0usize;
            let mut head = scroll;
            for row_idx in scroll..=self.tree_cursor {
                cumulative += self.tree_row_visual_height(row_idx, width);
                while cumulative > height && head < self.tree_cursor {
                    cumulative = cumulative.saturating_sub(
                        self.tree_row_visual_height(head, width),
                    );
                    head += 1;
                }
                let _ = row_idx;
            }
            scroll = head;
        }
        // `take(...)` was a logical-row cap when the tree didn't
        // wrap. With wrap on, render every row from `scroll`
        // onward and let ratatui clip at the pane bottom — that
        // way a partially-visible wrapped row still shows its
        // first lines instead of being dropped entirely.

        // Build the visible Lines by delegating each row to
        // `tree_row_lines`, which does the wrap + hanging-indent
        // layout. ratatui clips at the pane bottom, so emitting
        // every row from `scroll` onward is fine — a wrapped row
        // straddling the bottom still shows its first lines.
        let mut lines: Vec<Line> = Vec::new();
        for row_idx in scroll..self.rows.len() {
            for line in self.tree_row_lines(row_idx, width) {
                lines.push(line);
            }
            // Cheap upper-bound check so we don't build Lines
            // for rows that are clearly off-screen.
            if lines.len() >= height + 4 {
                break;
            }
        }

        // Pre-wrapped manually so ratatui doesn't re-wrap and
        // double-indent. No `.wrap(...)` here.
        let p = Paragraph::new(lines);
        f.render_widget(p, inner);
    }

    pub(in crate::tui::app) fn draw_editor(&mut self, f: &mut ratatui::Frame, area: Rect) {
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
                // 1.2.6+: event paragraphs show their calendar
                // timing (start [→ end] · precision · track) and
                // an [ORPHAN] tag when unlinked, so the timing
                // metadata is visible while editing the body.
                // Use Ctrl+V Shift+T to open the swim-lane view;
                // edit start / end / precision / track via the
                // `inkhaven event ...` CLI for now.
                let event_summary: Option<String> = status_node.and_then(|n| {
                    n.event.as_ref().map(|ev| {
                        let cal = crate::timeline::Calendar::from_config(
                            self.cfg.timeline.calendar.clone(),
                        );
                        let start = cal.format(
                            crate::timeline::TimelinePoint::from_ticks(ev.start_ticks),
                            ev.precision,
                        );
                        let mut s = start;
                        if let Some(end_ticks) = ev.end_ticks {
                            let end = cal.format(
                                crate::timeline::TimelinePoint::from_ticks(end_ticks),
                                ev.precision,
                            );
                            s.push_str(" → ");
                            s.push_str(&end);
                        }
                        let prec = match ev.precision {
                            crate::timeline::Precision::Year => "year",
                            crate::timeline::Precision::Season => "season",
                            crate::timeline::Precision::Month => "month",
                            crate::timeline::Precision::Week => "week",
                            crate::timeline::Precision::Day => "day",
                            crate::timeline::Precision::Hour => "hour",
                            crate::timeline::Precision::Tick => "tick",
                        };
                        s.push_str(&format!(" · {prec}"));
                        if let Some(track) = ev.track.as_ref() {
                            s.push_str(&format!(" · {track}"));
                        }
                        s
                    })
                });
                let is_orphan_event = status_node
                    .map(|n| {
                        n.event.is_some()
                            && n.tags
                                .iter()
                                .any(|t| t.eq_ignore_ascii_case("orphan"))
                    })
                    .unwrap_or(false);
                // 1.2.6+ — when the open paragraph is a regular
                // manuscript paragraph (not itself an event),
                // count how many timeline events link to it. The
                // data model has supported many-to-one for a
                // while; this surface makes the relationship
                // visible from the editor. Linear scan over the
                // hierarchy; cheap at literary scale.
                let incoming_events: usize = status_node
                    .filter(|n| n.event.is_none())
                    .map(|n| {
                        let me = n.id;
                        self.hierarchy
                            .iter()
                            .filter(|other| {
                                other.event.is_some()
                                    && other.linked_paragraphs.contains(&me)
                            })
                            .count()
                    })
                    .unwrap_or(0);

                let mut spans: Vec<Span<'_>> = Vec::new();
                spans.push(Span::raw(format!(
                    " Editor — {}{}{}{} · ",
                    d.title, lang_tag, ro, dirty
                )));
                if let Some(summary) = event_summary {
                    spans.push(Span::styled(
                        format!("◆ {summary}"),
                        Style::default()
                            .fg(self.theme.tree_open_marker)
                            .add_modifier(Modifier::BOLD),
                    ));
                    spans.push(Span::raw(" · "));
                    if is_orphan_event {
                        spans.push(Span::styled(
                            "[ORPHAN]",
                            Style::default()
                                .fg(Color::Red)
                                .add_modifier(Modifier::BOLD),
                        ));
                        spans.push(Span::raw(" · "));
                    }
                } else if incoming_events > 0 {
                    let plural = if incoming_events == 1 { "" } else { "s" };
                    spans.push(Span::styled(
                        format!("◆ linked from {incoming_events} event{plural}"),
                        Style::default()
                            .fg(self.theme.tree_open_marker)
                            .add_modifier(Modifier::DIM),
                    ));
                    spans.push(Span::raw(" · "));
                }
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

    /// Render the lower (read-only) pane of split-edit mode. No cursor,
    /// no diff/bold, no current-line highlight — it's a frozen view of the
    /// buffer at the moment F4 was pressed.
    pub(in crate::tui::app) fn draw_split_snapshot(&self, f: &mut ratatui::Frame, area: Rect) {
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

    /// 1.2.8+ — Help-book paragraph render path.  The
    /// Help book is read-only documentation, so instead of
    /// showing colored source we run the buffer through
    /// `tui::markdown::render` (the same pulldown-cmark
    /// pipeline the AI pane uses) and paint the resulting
    /// styled `Line`s anchored to the doc's scroll
    /// position.  No gutter, no cursor — purely a viewer.
    /// `Wrap { trim: false }` so long lines wrap inside the
    /// pane.  Scrolling is driven by `opened.scroll_row`
    /// (set by the same arrow / PgUp / PgDn handlers the
    /// source view uses); horizontal scroll is unused
    /// because the renderer hard-wraps long lines.
    pub(in crate::tui::app) fn draw_help_paragraph_rendered(
        &mut self,
        f: &mut ratatui::Frame,
        inner: Rect,
    ) {
        // `inner` is already the inside-border rect — the
        // editor border is painted up the stack in
        // `draw_editor`.  We just paint the rendered
        // markdown lines here.
        let opened = self.opened.as_mut().expect("opened checked above");
        let source: String = opened.textarea.lines().join("\n");
        let rendered: Vec<ratatui::text::Line<'static>> =
            super::super::super::markdown::render(&source);

        let total = rendered.len();
        let height = inner.height as usize;
        // Clamp scroll: don't allow scrolling past the
        // bottom — bottom = total - height when total >
        // height, else 0.
        let max_scroll = total.saturating_sub(height);
        if opened.scroll_row > max_scroll {
            opened.scroll_row = max_scroll;
        }
        // Take a generous window so wrapping doesn't truncate
        // mid-render.  Paragraph then handles its own clipping.
        let end = total.min(opened.scroll_row + height + 32);
        let visible_slice: Vec<ratatui::text::Line<'static>> =
            rendered[opened.scroll_row..end].to_vec();

        f.render_widget(
            Paragraph::new(visible_slice).wrap(Wrap { trim: false }),
            inner,
        );
    }

    pub(in crate::tui::app) fn draw_editor_unwrapped(&mut self, f: &mut ratatui::Frame, inner: Rect) {
        // 1.2.8+ — Help-book paragraphs render as fully-
        // rendered markdown (headings, lists, emphasis,
        // code fences, blockquotes…) instead of the
        // colored source.  Detection: the paragraph carries
        // both `read_only = true` (set at open time when
        // the Help-tag is in the ancestor chain) AND
        // `content_type = "markdown"`.  Both conditions
        // together identify the Help book without false
        // positives — other read-only views (snapshots,
        // diffs) keep the existing source view.
        let opened_ref = self.opened.as_ref().expect("opened checked above");
        let is_help_rendered = opened_ref.read_only
            && opened_ref.content_type.as_deref() == Some("markdown");
        if is_help_rendered {
            self.draw_help_paragraph_rendered(f, inner);
            return;
        }

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
        let lex_per_row: Vec<Vec<super::super::super::lexicon::LexHit>> = current_lines
            .iter()
            .map(|line| {
                if lexicon.is_empty() {
                    Vec::new()
                } else {
                    lexicon.row_hits(line)
                }
            })
            .collect();

        // 1.2.9+ — style-warning overlays.  Effective
        // enable flag is the session toggle if set, else
        // the HJSON setting.  Filter-word detector +
        // repeated-phrase detector both built once per
        // render frame.  Per-row hits union both
        // detectors' outputs (sorted by col_start).
        let style_enabled = self
            .style_warnings_toggle
            .unwrap_or(self.cfg.editor.style_warnings.enabled);
        let style_lang = self.cfg.language.as_str();
        let style_cfg = &self.cfg.editor.style_warnings;
        let filter_detector =
            if style_enabled && style_cfg.filter_words.enabled {
                Some(
                    super::super::super::style_warnings::FilterWordsDetector::new(
                        &style_cfg.filter_words,
                        style_lang,
                    ),
                )
            } else {
                None
            };
        let phrase_detector =
            if style_enabled && style_cfg.repeated_phrases.enabled {
                Some(
                    super::super::super::style_warnings::RepeatedPhraseDetector::new(
                        &style_cfg.repeated_phrases,
                        style_lang,
                        &current_lines,
                    ),
                )
            } else {
                None
            };
        let sdt_detector =
            if style_enabled && style_cfg.show_dont_tell.enabled {
                Some(
                    super::super::super::style_warnings::ShowDontTellDetector::new(
                        &style_cfg.show_dont_tell,
                        style_lang,
                    ),
                )
            } else {
                None
            };
        let style_per_row: Vec<Vec<super::super::super::style_warnings::StyleHit>> =
            current_lines
                .iter()
                .enumerate()
                .map(|(row, line)| {
                    let mut hits = Vec::new();
                    if let Some(d) = &filter_detector {
                        if !d.is_empty() {
                            hits.extend(d.detect(line));
                        }
                    }
                    if let Some(d) = &phrase_detector {
                        if !d.is_empty() {
                            hits.extend(d.hits_for_row(row).iter().copied());
                        }
                    }
                    if let Some(d) = &sdt_detector {
                        if !d.is_empty() {
                            hits.extend(d.detect(line));
                        }
                    }
                    hits.sort_by_key(|h| h.col_start);
                    hits
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

        // 1.2.6+ — set of editor lines (1-based) that carry a
        // typst diagnostic. Used to paint a red `●` in the
        // trailing-space slot of the line-number gutter.
        let diag_lines: std::collections::HashSet<usize> = opened
            .typst_diagnostics
            .iter()
            .map(|d| d.line)
            .collect();

        let mut visible_lines: Vec<Line> = Vec::with_capacity(h);
        let row_end = (opened.scroll_row + h).min(highlighted.len());
        for row in opened.scroll_row..row_end {
            let is_current = row == cur_row;
            // Split the gutter into digits + 1-char marker slot
            // (which is normally a space). When this row has a
            // diagnostic, the slot turns into a bold red `●`.
            let lineno_text = format!("{:>chars$}", row + 1, chars = lineno_chars);
            let has_diag = diag_lines.contains(&(row + 1));
            let mut lineno_span_style = lineno_style;
            if is_current {
                lineno_span_style = lineno_span_style
                    .bg(current_bg)
                    .add_modifier(Modifier::BOLD);
            }
            let marker_text = if has_diag { "●" } else { " " };
            let mut marker_style = Style::default();
            if has_diag {
                marker_style = marker_style
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD);
            }
            if is_current {
                marker_style = marker_style.bg(current_bg);
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
            let style_hits = style_per_row
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
                style_hits,
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
            let mut spans = vec![
                Span::styled(lineno_text, lineno_span_style),
                Span::styled(marker_text.to_string(), marker_style),
            ];
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

    pub(in crate::tui::app) fn draw_editor_wrapped(&mut self, f: &mut ratatui::Frame, inner: Rect) {
        // Same Help-paragraph rendered-markdown short-
        // circuit as `draw_editor_unwrapped` — keep both
        // entry points consistent.
        let opened_ref = self.opened.as_ref().expect("opened checked above");
        let is_help_rendered = opened_ref.read_only
            && opened_ref.content_type.as_deref() == Some("markdown");
        if is_help_rendered {
            self.draw_help_paragraph_rendered(f, inner);
            return;
        }

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

        let lex_per_row: Vec<Vec<super::super::super::lexicon::LexHit>> = current_lines
            .iter()
            .map(|line| {
                if lexicon.is_empty() {
                    Vec::new()
                } else {
                    lexicon.row_hits(line)
                }
            })
            .collect();

        // 1.2.9+ — style-warning overlays.  Effective
        // enable flag is the session toggle if set, else
        // the HJSON setting.  Filter-word detector +
        // repeated-phrase detector both built once per
        // render frame.  Per-row hits union both
        // detectors' outputs (sorted by col_start).
        let style_enabled = self
            .style_warnings_toggle
            .unwrap_or(self.cfg.editor.style_warnings.enabled);
        let style_lang = self.cfg.language.as_str();
        let style_cfg = &self.cfg.editor.style_warnings;
        let filter_detector =
            if style_enabled && style_cfg.filter_words.enabled {
                Some(
                    super::super::super::style_warnings::FilterWordsDetector::new(
                        &style_cfg.filter_words,
                        style_lang,
                    ),
                )
            } else {
                None
            };
        let phrase_detector =
            if style_enabled && style_cfg.repeated_phrases.enabled {
                Some(
                    super::super::super::style_warnings::RepeatedPhraseDetector::new(
                        &style_cfg.repeated_phrases,
                        style_lang,
                        &current_lines,
                    ),
                )
            } else {
                None
            };
        let sdt_detector =
            if style_enabled && style_cfg.show_dont_tell.enabled {
                Some(
                    super::super::super::style_warnings::ShowDontTellDetector::new(
                        &style_cfg.show_dont_tell,
                        style_lang,
                    ),
                )
            } else {
                None
            };
        let style_per_row: Vec<Vec<super::super::super::style_warnings::StyleHit>> =
            current_lines
                .iter()
                .enumerate()
                .map(|(row, line)| {
                    let mut hits = Vec::new();
                    if let Some(d) = &filter_detector {
                        if !d.is_empty() {
                            hits.extend(d.detect(line));
                        }
                    }
                    if let Some(d) = &phrase_detector {
                        if !d.is_empty() {
                            hits.extend(d.hits_for_row(row).iter().copied());
                        }
                    }
                    if let Some(d) = &sdt_detector {
                        if !d.is_empty() {
                            hits.extend(d.detect(line));
                        }
                    }
                    hits.sort_by_key(|h| h.col_start);
                    hits
                })
                .collect();

        let (cur_row, cur_col) = opened.textarea.cursor();
        let selection = opened.textarea.selection_range();

        let total_lines = highlighted.len().max(1);
        let lineno_chars = digit_count(total_lines);
        let gutter_width = (lineno_chars + 1) as u16;

        let h = inner.height as usize;
        let w = inner.width.saturating_sub(gutter_width) as usize;

        let mut visual: Vec<super::super::super::highlight::VisualRow> = Vec::new();
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

        // 1.2.6+ — diagnostic marker set, same shape as the
        // unwrapped renderer.
        let diag_lines: std::collections::HashSet<usize> = opened
            .typst_diagnostics
            .iter()
            .map(|d| d.line)
            .collect();

        let mut lines: Vec<Line> = Vec::with_capacity(h);
        let row_end = (opened.scroll_row + h).min(visual.len());
        for (i, v) in visual[opened.scroll_row..row_end].iter().enumerate() {
            let visual_row_idx = opened.scroll_row + i;
            let is_current = visual_row_idx == cursor_visual.0;

            // Line number only on the first visual row of each source row.
            let lineno_text = if v.src_col_start == 0 {
                format!("{:>chars$}", v.src_row + 1, chars = lineno_chars)
            } else {
                format!("{:>chars$}", "", chars = lineno_chars)
            };
            let mut lineno_span_style = lineno_style;
            if is_current {
                lineno_span_style = lineno_span_style
                    .bg(current_bg)
                    .add_modifier(Modifier::BOLD);
            }
            // 1.2.6+ — diagnostic marker slot. Mirrors the
            // unwrapped renderer above. Only paint the marker
            // on the first visual row of the source line (so a
            // wrapped line shows the dot once, not on every
            // visual continuation).
            let has_diag =
                v.src_col_start == 0 && diag_lines.contains(&(v.src_row + 1));
            let marker_text = if has_diag { "●" } else { " " };
            let mut marker_style = Style::default();
            if has_diag {
                marker_style = marker_style
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD);
            }
            if is_current {
                marker_style = marker_style.bg(current_bg);
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
            let style_hits = style_per_row
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
                style_hits,
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
            let mut spans = vec![
                Span::styled(lineno_text, lineno_span_style),
                Span::styled(marker_text.to_string(), marker_style),
            ];
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

    pub(in crate::tui::app) fn draw_ai(&self, f: &mut ratatui::Frame, area: Rect) {
        // Title carries the inference state plus mode chips so the user
        // can see at a glance:
        //   - bound LLM default (Ctrl+B L picker target) — always shown
        //     so swap-effect from Ctrl+B L is visible without opening
        //     Ctrl+B I
        //   - in-flight provider + streaming/done/error status
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
        // 1.2.8+ — bound LLM chip. Always visible; in-flight provider
        // appears below as a separate fragment when inference != None.
        spans.push(Span::raw(" · llm="));
        spans.push(Span::styled(
            self.cfg.llm.default.clone(),
            Style::default()
                .fg(self.theme.ai_infer_fg)
                .add_modifier(Modifier::BOLD),
        ));
        if let Some(inf) = &self.inference {
            // Suppress the redundant provider tag when the in-flight
            // run is on the bound default — the chip already shows it.
            // When the user fired the request and THEN swapped default
            // (Ctrl+B L) the two diverge — show both.
            let status_text = if inf.provider == self.cfg.llm.default {
                match &inf.status {
                    InferenceStatus::Streaming => " · streaming…".to_string(),
                    InferenceStatus::Done => " · done".to_string(),
                    InferenceStatus::Error(_) => " · error".to_string(),
                }
            } else {
                match &inf.status {
                    InferenceStatus::Streaming => format!(" — {} · streaming…", inf.provider),
                    InferenceStatus::Done => format!(" — {} · done", inf.provider),
                    InferenceStatus::Error(_) => format!(" — {} · error", inf.provider),
                }
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
                        let lines = super::super::super::markdown::render(&inf.response);
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

    /// Render the accumulated chat history (User / Assistant turns).
    /// Used by the `Ctrl+B K` AI-fullscreen layout. The newest turn is
    /// pinned to the bottom of the pane — old history scrolls up off-
    /// screen, matching the natural chat-window UX. `Paragraph::scroll`
    /// handles the offset so we don't have to track per-pane state.
    pub(in crate::tui::app) fn draw_chat_history(&self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(in crate::tui::app) fn draw_prompt_picker(&self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(in crate::tui::app) fn draw_status(&self, f: &mut ratatui::Frame, area: Rect) {
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
        spans.extend(self.pov_chip_spans());
        spans.push(Span::raw("  "));
        spans.push(Span::raw(self.status.clone()));

        // Right-aligned progress widget — drawn on its own
        // Paragraph with right alignment so it can't be pushed
        // off-screen by a long status message; the left part
        // truncates if the terminal is narrow.
        let progress_spans = self.progress_widget_spans();
        if !progress_spans.is_empty() {
            let right = Paragraph::new(Line::from(progress_spans))
                .alignment(ratatui::layout::Alignment::Right);
            f.render_widget(right, area);
        }
        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    pub(in crate::tui::app) fn draw_search_overlay(&self, f: &mut ratatui::Frame, area: Rect) {
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

}
