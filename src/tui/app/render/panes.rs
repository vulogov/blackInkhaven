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
use super::super::super::state::{HighlightCache, LexCache, LinkPickDirection};
use super::super::super::status_helpers::status_style;
use super::super::super::text_utils::{
    format_age_humantime, format_reading_time,
};

/// 1.8.32+ hardening — a stable hash of the editor buffer's lines, used as the
/// key for [`HighlightCache`] / [`LexCache`]. Same line-by-line hashing as
/// `content_fingerprint` so the two agree on what "unchanged" means. Shared with
/// the status bar's POV chip so it can validate the editor's cached hits.
pub(in crate::tui::app) fn buffer_content_hash(lines: &[String]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for l in lines {
        l.hash(&mut h);
    }
    h.finish()
}


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
        // 1.2.12+ Phase C — title is mode-aware.  In
        // split-view the secondary is a peer of the
        // primary, so the badge reads "split"; in
        // similar-mode (Ctrl+V S) it stays as
        // "similar".  Cursor L/C surfaces so the user
        // can see where the secondary's cursor is
        // without Tabbing into it — handy in
        // translation work where you scroll the
        // secondary to keep pace with the primary.
        let (row, col) = doc.textarea.cursor();
        let mode_badge = if self.split_view { "split" } else { "similar" };
        let title = format!(
            " {}  ·  ({mode_badge})  ·  L{} C{} ",
            doc.title,
            row + 1,
            col + 1,
        );
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

    /// 1.2.12+ Phase D follow-up — placeholder for the
    /// right pane when fullscreen split-view is on
    /// (`App.split_view = true`) but `App.secondary`
    /// is None.  Without this, pressing Shift+F4 on a
    /// fresh session looked like a no-op because the
    /// renderer silently fell back to the standard
    /// layout.  Now: the layout flips visibly; the
    /// right pane shows a help-text panel with the
    /// chord-by-chord cookbook for filling the
    /// secondary slot.
    pub(in crate::tui::app) fn draw_split_placeholder(&self, f: &mut ratatui::Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" (no paragraph pinned — pick one) ")
            .border_style(
                Style::default()
                    .fg(self.theme.border_unfocused)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.pane_bg)
                    .fg(self.theme.pane_fg),
            );
        let inner = block.inner(area);
        f.render_widget(block, area);
        let dim = Style::default().add_modifier(Modifier::DIM);
        let key = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let body = Style::default();
        let lines: Vec<Line<'_>> = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Split-view is ON, right pane is empty.",
                body,
            )),
            Line::from(Span::styled(
                "  Pin a paragraph here via any of:",
                body,
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("    Tree pane", key),
                Span::styled(
                    " (left): navigate, then Shift+Enter pins",
                    body,
                ),
            ]),
            Line::from(vec![
                Span::styled("    Ctrl+V P", key),
                Span::styled(
                    "       fuzzy picker — Shift+Enter pins",
                    body,
                ),
            ]),
            Line::from(vec![
                Span::styled("    Ctrl+V Shift+P", key),
                Span::styled(
                    " recent paragraphs — Shift+Enter pins",
                    body,
                ),
            ]),
            Line::from(vec![
                Span::styled("    Ctrl+V M", key),
                Span::styled(
                    "       bookmarks — Shift+Enter pins",
                    body,
                ),
            ]),
            Line::from(vec![
                Span::styled("    Ctrl+V Shift+B", key),
                Span::styled(
                    " sibling-book (same slug, other book)",
                    body,
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  Tab swaps focus between editor panes.",
                dim,
            )),
            Line::from(Span::styled(
                "  Shift+F4 toggles the layout off again.",
                dim,
            )),
        ];
        f.render_widget(Paragraph::new(lines), inner);
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
                    Some("jinja") => " [jinja]",
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

        // Per-paragraph goal footer (1.2.4+).  Plus the
        // 1.2.13 Phase B.2 Language chip
        // `[word · POS · translation]` when the cursor
        // sits on a Language lexicon hit — the chip wins
        // the footer slot because it's transient,
        // cursor-position-relative info that the user
        // explicitly asked for by navigating to that
        // word.  Goal gauge comes back the moment the
        // cursor moves off.
        let goal_footer = self.editor_goal_footer_text();
        let language_chip = self.language_hit_chip();
        // 1.4.8+ TERMS-1 — banned-synonym-at-cursor chip.
        let terms_chip = self.terms_hit_chip();
        // 1.2.14+ Phase C.1 — comment-at-cursor
        // chip.  Takes priority over the Language
        // chip and the goal gauge — comments are
        // explicit reviewer attention the author
        // should see first.
        let comment_chip = self.comment_at_cursor_chip();
        // WORLD-10 — the ambient scene chip (lowest priority: shows when no
        // cursor-specific chip and no goal gauge occupies the footer).
        let scene_chip = self.scene_chip();
        let need_footer = goal_footer.is_some()
            || language_chip.is_some()
            || comment_chip.is_some()
            || terms_chip.is_some()
            || scene_chip.is_some();
        let (editor_rect, footer_rect) = if need_footer {
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
        } else {
            (inner, None)
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

        // Render the footer last so it sits on top of the
        // textarea's bottom row (the carve-out above shrunk
        // the textarea, leaving exactly one free row).
        // Priority: Language chip > goal gauge.  The chip
        // is italic + Language colour to mirror the
        // editor overlay style — keeps the visual link
        // between the highlighted word and the chip
        // describing it.
        if let Some(rect) = footer_rect {
            if let Some(chip) = comment_chip {
                let style = Style::default()
                    .add_modifier(self.theme.comment_span_modifier);
                let line = Line::from(vec![
                    Span::raw(" "),
                    Span::styled(chip, style),
                ]);
                f.render_widget(Paragraph::new(line), rect);
            } else if let Some(chip) = language_chip {
                let style = Style::default()
                    .fg(self.theme.language_word_fg)
                    .add_modifier(Modifier::ITALIC);
                let line = Line::from(vec![
                    Span::raw(" "),
                    Span::styled(chip, style),
                ]);
                f.render_widget(Paragraph::new(line), rect);
            } else if let Some(chip) = terms_chip {
                // TERMS-1 — red, matching the banned-synonym overlay hue.
                let style = Style::default().fg(self.theme.style_warning_banned_synonym_fg);
                let line = Line::from(vec![Span::raw(" "), Span::styled(chip, style)]);
                f.render_widget(Paragraph::new(line), rect);
            } else if let Some((gauge, words, target)) = goal_footer {
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
            } else if let Some(chip) = scene_chip {
                let style = Style::default().add_modifier(Modifier::DIM | Modifier::ITALIC);
                let line = Line::from(vec![
                    Span::styled(" scene · ", style),
                    Span::styled(chip, style),
                ]);
                f.render_widget(Paragraph::new(line), rect);
            }
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
        //
        // 1.2.15+ Phase S.1 — graceful no-op when
        // `opened` is None.  The invariant ("caller
        // checked .is_some()") still holds in every
        // call site we know about, but a missed
        // refactor shouldn't take down the TUI; an
        // empty frame is the right fallback.
        let Some(opened) = self.opened.as_mut() else {
            tracing::warn!(
                target: "inkhaven::tui::render",
                "draw_help_paragraph_rendered called with no opened paragraph",
            );
            return;
        };
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
        // 1.2.15+ Phase S.1 — see draw_help_paragraph_rendered
        // for rationale; graceful no-op on None.
        let Some(opened_ref) = self.opened.as_ref() else {
            tracing::warn!(
                target: "inkhaven::tui::render",
                "editor draw called with no opened paragraph",
            );
            return;
        };
        let is_help_rendered = opened_ref.read_only
            && opened_ref.content_type.as_deref() == Some("markdown");
        if is_help_rendered {
            self.draw_help_paragraph_rendered(f, inner);
            return;
        }

        let block = self.current_block();
        let lexicon = &self.lexicon;
        let lex_gen = self.lexicon_generation;
        let theme = &self.theme;
        let Some(opened) = self.opened.as_mut() else {
            return;
        };
        let highlighter = &mut self.highlighter;
        let current_lines: Vec<String> = opened.textarea.lines().to_vec();
        let source = current_lines.join("\n");
        // 1.8.32+ hardening — reuse the memoized highlight when the buffer and
        // content type are unchanged; the editor repaints every frame, so this
        // skips the whole-buffer tree-sitter / lexer pass on idle frames.
        let content_hash = buffer_content_hash(&current_lines);
        let cache_hit = opened
            .highlight_cache
            .as_ref()
            .is_some_and(|c| c.content_hash == content_hash && c.content_type == opened.content_type);
        if !cache_hit {
            let computed =
                highlight_for_content(highlighter, &source, theme, opened.content_type.as_deref());
            opened.highlight_cache = Some(HighlightCache {
                content_hash,
                content_type: opened.content_type.clone(),
                lines: computed,
            });
        }
        let highlighted = opened.highlight_cache.as_ref().unwrap().lines.clone();

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
        // 1.8.33+ hardening — reuse the memoized per-row lexicon hits when the
        // buffer and the lexicon are both unchanged, so the whole-buffer Porter
        // stem pass doesn't re-run on idle frames.
        let lex_per_row: Vec<Vec<super::super::super::lexicon::LexHit>> = {
            let lex_hit = opened
                .lex_cache
                .as_ref()
                .is_some_and(|c| c.content_hash == content_hash && c.lexicon_generation == lex_gen);
            if !lex_hit {
                let computed: Vec<Vec<super::super::super::lexicon::LexHit>> = current_lines
                    .iter()
                    .map(|line| {
                        if lexicon.is_empty() {
                            Vec::new()
                        } else {
                            lexicon.row_hits(line)
                        }
                    })
                    .collect();
                opened.lex_cache = Some(LexCache {
                    content_hash,
                    lexicon_generation: lex_gen,
                    rows: computed,
                });
            }
            opened.lex_cache.as_ref().unwrap().rows.clone()
        };

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
        // 1.3.9+ — anachronism overlay.  Self-gating: the detector is empty
        // (and thus silent) until `anachronism.year` is set, so it needs no
        // enable flag of its own beyond the master style toggle.
        let anach_detector = if style_enabled {
            Some(
                super::super::super::style_warnings::AnachronismDetector::new(
                    &style_cfg.anachronism,
                ),
            )
        } else {
            None
        };
        // 1.2.20+ C.1.b — echo overlay.  Independent of the
        // Shift+F style toggle; driven by its own Shift+K
        // toggle + the `echo_overlay_stems` cache refreshed
        // each main-loop iteration.  Cheap per-line detector
        // built from the cached stem set.
        // Field accesses (not a method call) so the borrow
        // stays disjoint from the `self.opened` mutable
        // borrow above.
        let echo_active = self
            .echo_overlay_toggle
            .unwrap_or(self.cfg.editor.echo_overlay);
        let echo_detector = if echo_active
            && !self.echo_overlay_stems.is_empty()
        {
            Some(crate::tui::echo_overlay::EchoHighlighter::new(
                &self.echo_overlay_stems,
                style_lang,
            ))
        } else {
            None
        };
        // 1.4.8+ TERMS-1 — banned-synonym overlay from the Glossary book. Gated
        // on the master style toggle; defaults on within it (`Ctrl+V z` flips
        // `terms_overlay_toggle`). Self-gating: an empty Glossary → empty
        // detector → short-circuited line scan. Store/hierarchy field accesses
        // keep the borrow disjoint from the `self.opened` borrow above. The live
        // overlay applies the whole Glossary; `terms check --book` scopes per book.
        // 1.8.34 hardening — reuse the cached banned-synonym detector, rebuilding
        // it (a blocking fs read per Glossary paragraph) only after it was
        // invalidated in reload_hierarchy, instead of on every repaint. Writing
        // self.glossary_detector_cache + reading self.store/self.hierarchy are
        // disjoint from the self.opened borrow held above.
        let glossary_detector = if style_enabled
            && self.terms_overlay_toggle.unwrap_or(true)
        {
            if self.glossary_detector_cache.is_none() {
                self.glossary_detector_cache = Some(
                    super::super::super::style_warnings::BannedSynonymDetector::from_store(
                        &self.store,
                        &self.hierarchy,
                        None,
                    ),
                );
            }
            self.glossary_detector_cache.as_ref()
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
                    if let Some(d) = &anach_detector {
                        if !d.is_empty() {
                            hits.extend(d.detect(line));
                        }
                    }
                    if let Some(d) = &echo_detector {
                        if !d.is_empty() {
                            hits.extend(d.detect(line));
                        }
                    }
                    if let Some(d) = &glossary_detector {
                        if !d.is_empty() {
                            hits.extend(d.detect(line));
                        }
                    }
                    hits.sort_by_key(|h| h.col_start);
                    hits
                })
                .collect();

        // 1.2.14+ Phase C.1.1 — comment-span hits
        // per editor row.  Empty fast-path when the
        // open paragraph has no comments (the
        // common case — most paragraphs carry
        // none).
        let comment_per_row: Vec<Vec<super::super::super::comments::RowHit>> =
            if opened.comments.comments.is_empty() {
                vec![Vec::new(); current_lines.len()]
            } else {
                super::super::super::comments::per_row_hits(
                    &current_lines,
                    &opened.comments.comments,
                )
            };

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
            let comment_hits = comment_per_row
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
                comment_hits,
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
        // 1.2.15+ Phase S.1 — see draw_help_paragraph_rendered
        // for rationale; graceful no-op on None.
        let Some(opened_ref) = self.opened.as_ref() else {
            tracing::warn!(
                target: "inkhaven::tui::render",
                "editor draw called with no opened paragraph",
            );
            return;
        };
        let is_help_rendered = opened_ref.read_only
            && opened_ref.content_type.as_deref() == Some("markdown");
        if is_help_rendered {
            self.draw_help_paragraph_rendered(f, inner);
            return;
        }

        let block = self.current_block();
        let lexicon = &self.lexicon;
        let lex_gen = self.lexicon_generation;
        let theme = &self.theme;
        let Some(opened) = self.opened.as_mut() else {
            return;
        };
        let highlighter = &mut self.highlighter;
        let current_lines: Vec<String> = opened.textarea.lines().to_vec();
        let source = current_lines.join("\n");
        // 1.8.32+ hardening — reuse the memoized highlight when the buffer and
        // content type are unchanged; the editor repaints every frame, so this
        // skips the whole-buffer tree-sitter / lexer pass on idle frames.
        let content_hash = buffer_content_hash(&current_lines);
        let cache_hit = opened
            .highlight_cache
            .as_ref()
            .is_some_and(|c| c.content_hash == content_hash && c.content_type == opened.content_type);
        if !cache_hit {
            let computed =
                highlight_for_content(highlighter, &source, theme, opened.content_type.as_deref());
            opened.highlight_cache = Some(HighlightCache {
                content_hash,
                content_type: opened.content_type.clone(),
                lines: computed,
            });
        }
        let highlighted = opened.highlight_cache.as_ref().unwrap().lines.clone();

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

        // 1.8.33+ hardening — reuse the memoized per-row lexicon hits when the
        // buffer and the lexicon are both unchanged, so the whole-buffer Porter
        // stem pass doesn't re-run on idle frames.
        let lex_per_row: Vec<Vec<super::super::super::lexicon::LexHit>> = {
            let lex_hit = opened
                .lex_cache
                .as_ref()
                .is_some_and(|c| c.content_hash == content_hash && c.lexicon_generation == lex_gen);
            if !lex_hit {
                let computed: Vec<Vec<super::super::super::lexicon::LexHit>> = current_lines
                    .iter()
                    .map(|line| {
                        if lexicon.is_empty() {
                            Vec::new()
                        } else {
                            lexicon.row_hits(line)
                        }
                    })
                    .collect();
                opened.lex_cache = Some(LexCache {
                    content_hash,
                    lexicon_generation: lex_gen,
                    rows: computed,
                });
            }
            opened.lex_cache.as_ref().unwrap().rows.clone()
        };

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
        // 1.3.9+ — anachronism overlay.  Self-gating: the detector is empty
        // (and thus silent) until `anachronism.year` is set, so it needs no
        // enable flag of its own beyond the master style toggle.
        let anach_detector = if style_enabled {
            Some(
                super::super::super::style_warnings::AnachronismDetector::new(
                    &style_cfg.anachronism,
                ),
            )
        } else {
            None
        };
        // 1.2.20+ C.1.b — echo overlay.  Independent of the
        // Shift+F style toggle; driven by its own Shift+K
        // toggle + the `echo_overlay_stems` cache refreshed
        // each main-loop iteration.  Cheap per-line detector
        // built from the cached stem set.
        // Field accesses (not a method call) so the borrow
        // stays disjoint from the `self.opened` mutable
        // borrow above.
        let echo_active = self
            .echo_overlay_toggle
            .unwrap_or(self.cfg.editor.echo_overlay);
        let echo_detector = if echo_active
            && !self.echo_overlay_stems.is_empty()
        {
            Some(crate::tui::echo_overlay::EchoHighlighter::new(
                &self.echo_overlay_stems,
                style_lang,
            ))
        } else {
            None
        };
        // 1.4.8+ TERMS-1 — banned-synonym overlay from the Glossary book. Gated
        // on the master style toggle; defaults on within it (`Ctrl+V z` flips
        // `terms_overlay_toggle`). Self-gating: an empty Glossary → empty
        // detector → short-circuited line scan. Store/hierarchy field accesses
        // keep the borrow disjoint from the `self.opened` borrow above. The live
        // overlay applies the whole Glossary; `terms check --book` scopes per book.
        // 1.8.34 hardening — reuse the cached banned-synonym detector, rebuilding
        // it (a blocking fs read per Glossary paragraph) only after it was
        // invalidated in reload_hierarchy, instead of on every repaint. Writing
        // self.glossary_detector_cache + reading self.store/self.hierarchy are
        // disjoint from the self.opened borrow held above.
        let glossary_detector = if style_enabled
            && self.terms_overlay_toggle.unwrap_or(true)
        {
            if self.glossary_detector_cache.is_none() {
                self.glossary_detector_cache = Some(
                    super::super::super::style_warnings::BannedSynonymDetector::from_store(
                        &self.store,
                        &self.hierarchy,
                        None,
                    ),
                );
            }
            self.glossary_detector_cache.as_ref()
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
                    if let Some(d) = &anach_detector {
                        if !d.is_empty() {
                            hits.extend(d.detect(line));
                        }
                    }
                    if let Some(d) = &echo_detector {
                        if !d.is_empty() {
                            hits.extend(d.detect(line));
                        }
                    }
                    if let Some(d) = &glossary_detector {
                        if !d.is_empty() {
                            hits.extend(d.detect(line));
                        }
                    }
                    hits.sort_by_key(|h| h.col_start);
                    hits
                })
                .collect();

        // 1.2.14+ Phase C.1.1 — comment-span hits
        // per editor row.  Empty fast-path when the
        // open paragraph has no comments (the
        // common case — most paragraphs carry
        // none).
        let comment_per_row: Vec<Vec<super::super::super::comments::RowHit>> =
            if opened.comments.comments.is_empty() {
                vec![Vec::new(); current_lines.len()]
            } else {
                super::super::super::comments::per_row_hits(
                    &current_lines,
                    &opened.comments.comments,
                )
            };

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
        // M6 — clamp the scroll origin into range before slicing. A
        // stale `scroll_row` (e.g. a zero-height split pane whose scroll
        // wasn't reset) could exceed `visual.len()`, making `start >
        // row_end` and panicking the slice.
        let start = opened.scroll_row.min(visual.len());
        let row_end = (start + h).min(visual.len());
        for (i, v) in visual[start..row_end].iter().enumerate() {
            let visual_row_idx = start + i;
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
            let comment_hits = comment_per_row
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
                comment_hits,
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

    /// PANE-1 — the Output pane: structured notifications from every subsystem.
    /// Each message is a two-line entry (severity icon + kind, then its text),
    /// the selected row marked and bold when the region is focused.
    /// THOUGHTS-1 — the Thoughts pane: a read-only, scrollable view of reflective
    /// blocks (newest first), e.g. an Inner Theologian session. `thoughts_scroll`
    /// counts lines from the top; ratatui clamps over-scroll.
    pub(in crate::tui::app) fn draw_thoughts(&self, f: &mut ratatui::Frame, area: Rect) {
        let focused =
            self.focus == Focus::Ai && self.right_pane == crate::tui::app::RightPane::Thoughts;
        let border_style = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(format!(" Thoughts · {} ", self.thoughts.len()));
        let inner = block.inner(area);
        f.render_widget(block, area);

        if self.thoughts.is_empty() {
            let hint = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled("  No thoughts yet.", Style::default().fg(Color::DarkGray))),
                Line::from(Span::styled(
                    "  Ctrl+B J→T asks the Inner Theologian.",
                    Style::default().fg(Color::DarkGray),
                )),
            ]);
            f.render_widget(hint, inner);
            return;
        }

        // Render each thought block as Markdown (bold / italic / headings /
        // lists / quotes / rules), reusing the AI pane's CommonMark lexer.
        let mut lines: Vec<Line> = Vec::new();
        for (i, t) in self.thoughts.iter().rev().enumerate() {
            if i > 0 {
                lines.push(Line::from(Span::styled(
                    "─".repeat(inner.width.max(1) as usize),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            for row in crate::tui::markdown_highlight::highlight_markdown_lines(t, &self.theme) {
                lines.push(Line::from(
                    row.into_iter()
                        .map(|r| Span::styled(r.text, r.style))
                        .collect::<Vec<_>>(),
                ));
            }
        }

        // Reserve the bottom row for the footer hint.
        let footer_h: u16 = if inner.height > 1 { 1 } else { 0 };
        let body_rect = Rect { height: inner.height - footer_h, ..inner };

        // Clamp the scroll to the WRAPPED line count for the current width — the
        // offset counts wrapped lines, which shrink when the pane widens (e.g.
        // split → fullscreen), so a raw offset can land past the content and show
        // a blank pane. Estimate wrapped rows by char width per line (ratatui's
        // word-wrap breaks earlier, so this under-counts → the clamp is safe and
        // never blanks).
        let w = body_rect.width.max(1) as usize;
        let wrapped_total: usize = lines
            .iter()
            .map(|l| {
                let len: usize = l.spans.iter().map(|s| s.content.chars().count()).sum();
                len.div_ceil(w).max(1)
            })
            .sum();
        let max_scroll = wrapped_total.saturating_sub(body_rect.height as usize);
        let scroll = self.thoughts_scroll.min(max_scroll);
        let para = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll.min(u16::MAX as usize) as u16, 0));
        f.render_widget(para, body_rect);

        if footer_h == 1 {
            let footer = Rect { x: inner.x, y: inner.y + inner.height - 1, width: inner.width, height: 1 };
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    " ↑↓ scroll · g/G top/bottom · c clear · Ctrl+Z f fullscreen · Ctrl+B Tab panes ",
                    Style::default().add_modifier(Modifier::DIM),
                ))),
                footer,
            );
        }
    }

    pub(in crate::tui::app) fn draw_output(&self, f: &mut ratatui::Frame, area: Rect) {
        use crate::pane::output::Severity;

        let focused = self.focus == Focus::Ai;
        let border_style = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        // PANE-1 filtering — the same filtered view the key handler acts on.
        let msgs = self.filtered_output_messages();
        let title = if self.output_filter.is_active() {
            let total = crate::pane::output::active()
                .and_then(|s| s.count_active(None).ok())
                .unwrap_or(msgs.len());
            format!(" Output · {}/{} · {} ", msgs.len(), total, self.output_filter.summary())
        } else {
            format!(" Output · {} ", msgs.len())
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title);
        let inner = block.inner(area);
        f.render_widget(block, area);

        if msgs.is_empty() {
            let hint = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  No notifications.",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(Span::styled(
                    "  Ctrl+B Tab → AI",
                    Style::default().fg(Color::DarkGray),
                )),
            ]);
            f.render_widget(hint, inner);
            return;
        }

        let mut lines: Vec<Line> = Vec::new();
        let mut sel_line_idx = 0usize;
        for (i, m) in msgs.iter().enumerate() {
            let sel = focused && i == self.output_selected;
            if i == self.output_selected {
                sel_line_idx = lines.len();
            }
            let (icon, mut color) = match m.severity {
                Severity::Info => ('●', Color::Gray),
                Severity::Warning => ('⚠', Color::Yellow),
                Severity::Contradiction => ('⊗', Color::Red),
                Severity::Progress => ('↻', Color::Cyan),
            };
            // INNER_EDITOR-1 — warm-earth palette by severity (Praise muted gold,
            // Note terracotta, Concern deep ochre), distinct from the contemplative
            // purple/grey of the other companions.
            if m.kind == crate::pane::output::kinds::INNER_EDITOR_OBSERVATION {
                color = match m.severity {
                    Severity::Info => Color::Rgb(198, 156, 70),       // muted gold
                    Severity::Warning => Color::Rgb(188, 110, 78),    // terracotta
                    Severity::Contradiction => Color::Rgb(160, 96, 40), // deep ochre
                    Severity::Progress => color,
                };
            }
            let text =
                m.metadata.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let pin = if m.pinned { " 📌" } else { "" };
            // WORLD-5 — a 📅 marker on timeline-derived fact-check findings.
            let timeline = if m.metadata.get("timeline").and_then(|v| v.as_bool()).unwrap_or(false) {
                " 📅"
            } else {
                ""
            };
            let marker = if sel { "▌" } else { " " };
            // TIMELINE-2-INTEGRATION — a kind glyph distinguishes the two
            // timeline-critique findings (orphan ⊘ / fuzzy overlap ⧉) without
            // colliding with the severity icons.
            let kind_glyph = match m.kind.as_str() {
                crate::pane::output::kinds::TIMELINE_ORPHAN_WARNING => "⊘ ",
                crate::pane::output::kinds::TIMELINE_FUZZY_OVERLAP_WARNING => "⧉ ",
                crate::pane::output::kinds::INNER_EDITOR_OBSERVATION => "✎ ",
                crate::pane::output::kinds::HAIKU => "✦ ",
                crate::pane::output::kinds::THEOLOGIAN => "⚖ ",
                crate::pane::output::kinds::MYTH => "⊛ ",
                crate::pane::output::kinds::DOC_VERIFY => "⌨ ",
                crate::pane::output::kinds::SOURCING => "❝ ",
                crate::pane::output::kinds::ARGUMENT => "⇉ ",
                crate::pane::output::kinds::XREF => "⌘ ",
                crate::pane::output::kinds::CONFRONT => "⚔ ",
                crate::pane::output::kinds::LOCUS => "⚑ ",
                crate::pane::output::kinds::RIGOR => "⊬ ",
                crate::pane::output::kinds::ORACLE => "⌥ ",
                crate::pane::output::kinds::POEM => "♪ ",
                _ => "",
            };
            lines.push(Line::from(vec![
                Span::styled(format!("{marker}{icon} "), Style::default().fg(color)),
                Span::styled(format!("{kind_glyph}{}{timeline}{pin}", m.kind), Style::default().fg(Color::DarkGray)),
            ]));
            let text_style = if sel {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            // HAIKU-1 — render the three lines as a poem (the pane's row model is
            // one text line, so embedded newlines won't lay out; we push a line
            // per haiku line). Falls back to the inline `text` if the array is
            // absent.
            if m.kind == crate::pane::output::kinds::HAIKU {
                match m.metadata.get("haiku_lines").and_then(|v| v.as_array()) {
                    Some(arr) => {
                        for hl in arr {
                            if let Some(s) = hl.as_str() {
                                lines.push(Line::from(vec![
                                    Span::raw("   "),
                                    Span::styled(s.to_string(), text_style),
                                ]));
                            }
                        }
                    }
                    None => lines.push(Line::from(vec![Span::raw("   "), Span::styled(text, text_style)])),
                }
            } else {
                lines.push(Line::from(vec![Span::raw("   "), Span::styled(text, text_style)]));
            }

            // Expanded detail (`o`/Space): per-word trace + alternatives, or the
            // remaining metadata fields for kinds without a trace.
            if self.output_expanded.contains(&m.id) {
                let dim = Style::default().fg(Color::DarkGray);
                let trace = m.metadata.get("trace").and_then(|v| v.as_array());
                let alts = m.metadata.get("alternatives").and_then(|v| v.as_array());
                // PANE-1 P3 — lexicon proposals: list every candidate word.
                let proposals = m.metadata.get("proposals").and_then(|v| v.as_array());
                // PANE-1 P3 — variety renderings: list each base→variety pair.
                let renderings = m.metadata.get("renderings").and_then(|v| v.as_array());
                if let Some(proposals) = proposals {
                    for p in proposals {
                        let form = p.get("form").and_then(|v| v.as_str()).unwrap_or("");
                        let gloss = p.get("gloss").and_then(|v| v.as_str()).unwrap_or("");
                        let pos = p.get("pos").and_then(|v| v.as_str()).unwrap_or("");
                        lines.push(Line::from(Span::styled(
                            format!("      {form:<16} {gloss} ({pos})"),
                            dim,
                        )));
                    }
                }
                if let Some(renderings) = renderings {
                    for r in renderings {
                        let base = r.get("base").and_then(|v| v.as_str()).unwrap_or("");
                        let rendered = r.get("rendered").and_then(|v| v.as_str()).unwrap_or("");
                        let arrow = if base == rendered { " =" } else { "→" };
                        lines.push(Line::from(Span::styled(
                            format!("      {base}  {arrow}  {rendered}"),
                            dim,
                        )));
                    }
                }
                if let Some(trace) = trace {
                    for e in trace {
                        let src = e.get("source").and_then(|v| v.as_str()).unwrap_or("");
                        let tgt = e.get("target").and_then(|v| v.as_str()).unwrap_or("");
                        let conf = e.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let dec = e
                            .get("decision")
                            .and_then(|d| d.get("kind"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        lines.push(Line::from(Span::styled(
                            format!("      {src} → {tgt}  ({dec}, {conf:.2})"),
                            dim,
                        )));
                    }
                }
                if let Some(alts) = alts {
                    for a in alts {
                        let at = a.get("text").and_then(|v| v.as_str()).unwrap_or("");
                        let rat = a.get("rationale").and_then(|v| v.as_str()).unwrap_or("");
                        lines.push(Line::from(Span::styled(
                            format!("      alt: {at}  ({rat})"),
                            dim,
                        )));
                    }
                }
                if trace.is_none()
                    && alts.is_none()
                    && proposals.is_none()
                    && renderings.is_none()
                    && m.kind != crate::pane::output::kinds::HAIKU
                {
                    if let Some(obj) = m.metadata.as_object() {
                        for (k, v) in obj.iter().filter(|(k, _)| k.as_str() != "text") {
                            lines.push(Line::from(Span::styled(format!("      {k}: {v}"), dim)));
                        }
                    }
                }
            }
        }

        // Reserve up to two bottom rows for the action-key hint (it wraps when
        // the pane is narrow, so a single row truncated the longer hints).
        let footer_h: u16 = if inner.height > 4 {
            2
        } else if inner.height > 2 {
            1
        } else {
            0
        };
        let list_area = Rect { height: inner.height - footer_h, ..inner };

        // Scroll so the selected entry stays visible (entries vary in height when
        // expanded, so use the selected entry's actual first line).
        let rows = list_area.height as usize;
        let offset = sel_line_idx.saturating_sub(rows.saturating_sub(2)) as u16;
        let para = Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((offset, 0));
        f.render_widget(para, list_area);

        if footer_h >= 1 {
            let footer = Rect {
                x: inner.x,
                y: inner.y + inner.height - footer_h,
                width: inner.width,
                height: footer_h,
            };
            // The action row is context-aware: a lexicon proposal advertises
            // its Enter→accept; a translation result, r→remember.
            use crate::pane::output::kinds as k;
            let sel_kind = msgs.get(self.output_selected).map(|m| m.kind.as_str());
            let hint_text = match sel_kind {
                Some(s) if s == k::LEXICON_PROPOSAL => {
                    " ↑↓ · ⏎ accept · o expand · a ask AI · d dismiss · p pin · ^B Tab"
                }
                Some(s) if s == k::TRANSLATION_RESULT => {
                    " ↑↓ · ⏎ insert · e edit+remember · r remember · a ask AI · d dismiss · ^B Tab"
                }
                Some(s) if s == k::AI_TASK_COMPLETE => {
                    " ↑↓ · ⏎ open target · o expand · d dismiss · p pin · ^B Tab"
                }
                Some(s) if s == k::SOCRATIC_INQUIRY => {
                    " ↑↓ · i intent · m note · x addressed · a ask AI · d dismiss · ^B Tab"
                }
                Some(s) if s == k::INNER_EDITOR_OBSERVATION => {
                    " ↑↓ · i intent · o expand · a ask AI · d dismiss · ^B Tab"
                }
                Some(s)
                    if s == k::TIMELINE_ORPHAN_WARNING
                        || s == k::TIMELINE_FUZZY_OVERLAP_WARNING =>
                {
                    " ↑↓ · ⏎ jump to event · o expand · a ask AI · d dismiss · s snooze · ^B Tab"
                }
                _ => " ↑↓ · o expand · r remember · a ask AI · d dismiss · p pin · ^B Tab",
            };
            // Compact filter cue: `f` cycles source, `S` severity, `t` this-¶,
            // `/` free-text search, `c` clears the filter, `C` clears the pane.
            let filter_cue = if self.output_filter.is_active() {
                " · C clear · filter:f/S/t · /:search · c:clr"
            } else {
                " · C clear · f:filter · /:search"
            };
            // PANE-2 — while the query line is focused, the footer becomes a live
            // search input; otherwise it shows the navigation + filter cues.
            let hint = if self.output_query_focused {
                let q = self.output_filter.text_query.as_deref().unwrap_or("");
                Paragraph::new(Line::from(vec![
                    Span::styled("  search /", Style::default().fg(Color::Cyan)),
                    Span::styled(format!("{q}▏"), Style::default().fg(Color::White)),
                    Span::styled("   ⏎ keep · Esc clear", Style::default().fg(Color::DarkGray)),
                ]))
            } else {
                // Wrap so the longer hints don't truncate in a narrow pane.
                Paragraph::new(Line::from(Span::styled(
                    format!("{hint_text}{filter_cue}"),
                    Style::default().fg(Color::DarkGray),
                )))
                .wrap(ratatui::widgets::Wrap { trim: false })
            };
            f.render_widget(hint, footer);
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
        // 1.2.21+ FF.4b — the Facts scope is always grounded in the
        // supplied facts (it runs the fact-analysis system prompt, not
        // F10's Local/Full prompt), so the chip reads `Local` there
        // regardless of the F10 toggle — honest about the clamp.
        let infer_label = if self.ai_mode == crate::tui::inference::AiMode::Facts {
            "Local"
        } else {
            self.inference_mode.label()
        };
        spans.push(Span::styled(
            infer_label.to_string(),
            Style::default()
                .fg(self.theme.ai_infer_fg)
                .add_modifier(Modifier::BOLD),
        ));
        // 1.2.12+ Phase C — prompt-language chip.  Shows
        // the ISO code the resolver will target plus a
        // one-word mode hint (`book` / `paragraph`).
        // `Ctrl+B Shift+N` cycles the session override
        // and the chip flips immediately.
        spans.push(Span::raw(" · lang="));
        spans.push(Span::styled(
            self.ai_pane_language_label(),
            Style::default()
                .fg(self.theme.ai_infer_fg)
                .add_modifier(Modifier::BOLD),
        ));
        // 1.2.13+ Phase D.1 — translation chip.  Visible
        // only while a Ctrl+B Q / Ctrl+B Shift+Q stream
        // is in flight (`pending_translation` flag set
        // at spawn, cleared after the I-apply
        // extraction).  Tells the author at a glance
        // that the I key will lift only the
        // <<<TRANSLATION>>> block, not the whole
        // response.  Italic + Language colour to mirror
        // the editor overlay's Language style.
        if self.pending_translation {
            spans.push(Span::raw(" · translate"));
            spans.push(Span::styled(
                "[on]".to_string(),
                Style::default()
                    .fg(self.theme.language_word_fg)
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::ITALIC),
            ));
        }
        if chat_turns > 0 {
            spans.push(Span::raw(format!(" · {chat_turns} turn(s)")));
        }
        spans.push(Span::raw(" "));
        let title_line = Line::from(spans);
        let block = self.pane_block_line(title_line, Focus::Ai);
        let inner = block.inner(area);
        f.render_widget(block, area);

        // BOOK_RAG-1 — Book scope is a conversation: render the running
        // transcript (retrieved-passages panel + prior turns + the streaming
        // turn) so the author sees the whole chat, not just the latest reply.
        // Other scopes keep the single-response + action-hints view.
        if self.ai_mode == AiMode::Book
            && (!self.chat_history.is_empty() || self.inference.is_some())
        {
            self.draw_ai_book_conversation(f, inner);
            return;
        }

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
                    let mut hint_spans = vec![
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
                    ];
                    // Only offered when this response carries a system-book
                    // destination (a submission draft / structural analysis).
                    if self.lift_target_matches_current() {
                        hint_spans.push(Span::raw("  "));
                        hint_spans.push(Span::styled(" L ", reverse_chip(Color::Cyan)));
                        hint_spans.push(Span::raw("file"));
                    }
                    f.render_widget(Paragraph::new(Line::from(hint_spans)), hints_rect);
                }
            }
        }
    }

    /// BOOK_RAG-1 — render the Book-scope conversation inline in the AI pane:
    /// the collapsible retrieved-passages panel, every finalised User/Assistant
    /// turn, then the in-flight turn (the author's question + the streaming
    /// answer) which hasn't folded into `chat_history` yet. Newest content is
    /// pinned to the bottom; `chat_history_scroll` (PageUp) lifts the window.
    fn draw_ai_book_conversation(&self, f: &mut ratatui::Frame, inner: Rect) {
        let user_style = Style::default()
            .fg(self.theme.ai_scope_fg)
            .add_modifier(Modifier::BOLD);
        let assistant_style = Style::default()
            .fg(self.theme.ai_infer_fg)
            .add_modifier(Modifier::BOLD);
        let dim = Style::default().add_modifier(Modifier::DIM);

        let mut lines = self.book_rag_transparency_lines();
        let (history_lines, _) = self.build_chat_history_lines();
        lines.extend(history_lines);

        // The in-flight turn: present only while streaming (or on error) —
        // once done it folds into `chat_history` and `pending_chat_user_msg`
        // clears, so this never double-renders the latest answer.
        if let (Some(pending), Some(inf)) =
            (self.pending_chat_user_msg.as_ref(), self.inference.as_ref())
        {
            if !self.chat_history.is_empty() {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled("❯ User".to_string(), user_style)));
            for l in pending.lines() {
                lines.push(Line::from(format!("  {l}")));
            }
            lines.push(Line::from(Span::styled(
                "← Assistant".to_string(),
                assistant_style,
            )));
            match &inf.status {
                InferenceStatus::Error(e) => lines.push(Line::from(Span::styled(
                    format!("  {e}"),
                    Style::default().fg(Color::Red),
                ))),
                _ => {
                    let rendered = super::super::super::markdown::render(&inf.response);
                    if rendered.is_empty() {
                        lines.push(Line::from(Span::styled("  ▌streaming…".to_string(), dim)));
                    } else {
                        lines.extend(rendered);
                    }
                }
            }
        }

        // Bottom-pinned scroll: newest turn sits at the bottom; PageUp
        // (`chat_history_scroll`) lifts the window toward older turns.
        let body_h = inner.height as usize;
        let total = lines.len();
        let auto_scroll = total.saturating_sub(body_h);
        let scroll_offset = auto_scroll.saturating_sub(self.chat_history_scroll);
        let p = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll_offset as u16, 0));
        f.render_widget(p, inner);
    }

    /// Render the accumulated chat history (User / Assistant turns).
    /// Used by the `Ctrl+B K` AI-fullscreen layout. The newest turn is
    /// pinned to the bottom of the pane — old history scrolls up off-
    /// screen, matching the natural chat-window UX. `Paragraph::scroll`
    /// handles the offset so we don't have to track per-pane state.
    /// BOOK_RAG-1 P3 — the collapsible "Retrieved passages" transparency
    /// section, prepended to the chat history so the author can always see
    /// the evidence behind a Book-scope answer. Collapsed by default;
    /// toggled with `p` in the AI pane. Empty when no retrieval is held.
    fn book_rag_transparency_lines(&self) -> Vec<Line<'static>> {
        let Some(passages) = self.book_rag_last_retrieval.as_ref() else {
            return Vec::new();
        };
        if passages.is_empty() {
            return Vec::new();
        }
        let dim = Style::default().add_modifier(Modifier::DIM);
        let n = passages.len();
        let mut out: Vec<Line<'static>> = Vec::new();
        if !self.book_rag_passages_expanded {
            out.push(Line::from(Span::styled(
                format!("▶ Retrieved passages ({n}) · p to expand"),
                dim,
            )));
        } else {
            out.push(Line::from(Span::styled(
                format!("▼ Retrieved passages ({n}) · p to collapse"),
                dim,
            )));
            for p in passages {
                let star = if p.is_hit { "★" } else { " " };
                out.push(Line::from(vec![
                    Span::styled(
                        format!("  {:.2} {} ", p.score, star),
                        Style::default().fg(self.theme.ai_scope_fg),
                    ),
                    // The location path is the citation token the answer uses —
                    // show it, not the author-useless UUID.
                    Span::styled(format!("[{}]", p.breadcrumb), dim),
                ]));
                // First non-empty prose line, markup-stripped + truncated.
                let opening: String = p
                    .body
                    .lines()
                    .map(|l| l.trim_start_matches(['=', ' ', '#', '*', '_']).trim())
                    .find(|l| !l.is_empty())
                    .unwrap_or("")
                    .chars()
                    .take(72)
                    .collect();
                if !opening.is_empty() {
                    out.push(Line::from(Span::styled(format!("      {opening}"), dim)));
                }
            }
            // Once-per-conversation: surface how to refresh.
            out.push(Line::from(Span::styled(
                "  (retrieved once for this chat — clear history to retrieve again)",
                dim,
            )));
        }
        out.push(Line::from("")); // separator before the conversation
        out
    }

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

        let (mut lines, mut turn_ranges) = self.build_chat_history_lines();

        // BOOK_RAG-1 — prepend the "Retrieved passages" transparency section
        // and shift the turn ranges so chat-selection highlighting still
        // lands on the right lines.
        let section = self.book_rag_transparency_lines();
        if !section.is_empty() {
            let k = section.len();
            for r in turn_ranges.iter_mut() {
                r.start += k;
                r.end += k;
            }
            let mut combined = section;
            combined.append(&mut lines);
            lines = combined;
        }

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
            // 1.2.12+ Phase C — section the list by
            // language-priority bucket: active language
            // first, then untagged (back-compat), then
            // other languages.  Section headers appear
            // at each transition so the user can see
            // why "their" prompts are at the top.
            let active = self.active_prompt_language();
            let bucket_of = |lang: &Option<String>| -> u8 {
                match lang.as_deref() {
                    Some(l) if l.eq_ignore_ascii_case(&active) => 0,
                    None => 1,
                    Some(_) => 2,
                }
            };
            let header_for = |b: u8| -> String {
                match b {
                    0 => format!("── In active language ({active}) ──"),
                    1 => "── Untagged ──".to_string(),
                    _ => "── Other languages ──".to_string(),
                }
            };
            let mut last_bucket: Option<u8> = None;
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
                // Section header on bucket transition.
                let bucket = bucket_of(&p.language);
                if last_bucket != Some(bucket) {
                    if last_bucket.is_some() {
                        lines.push(Line::from(""));
                    }
                    lines.push(Line::from(Span::styled(
                        header_for(bucket),
                        Style::default()
                            .add_modifier(Modifier::DIM | Modifier::BOLD),
                    )));
                    last_bucket = Some(bucket);
                }
                // Inline language chip per row — `[ru]`
                // when tagged, `[—]` when untagged so
                // the visual width stays steady.
                let lang_chip = match p.language.as_deref() {
                    Some(l) => format!(" [{l}]"),
                    None => " [—]".to_string(),
                };
                let lang_chip_style = Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::DIM);
                lines.push(Line::from(vec![
                    Span::styled(chip_text.to_string(), chip_style),
                    Span::styled(lang_chip, lang_chip_style),
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
        // 1.2.15+ Phase H.1 — background health-
        // monitor chip.  Glyph + colour reflect the
        // most recent finding the TUI consumed from
        // the channel.  Hidden when no monitor is
        // running (config disabled OR receiver
        // disconnected).
        spans.extend(self.health_chip_spans());
        spans.extend(self.pov_chip_spans());
        // 1.2.16+ Phase A.5 — glossary chip
        // (worldbuilding density at-a-glance).
        spans.extend(self.glossary_chip_spans());
        // 1.2.21+ FF.6 — Facts chip (world-invariant count).
        spans.extend(self.facts_chip_spans());
        // POEM-TUI (PO-P13) — live verse readout (current line's syllables +
        // position), shown only while a verse paragraph is open.
        spans.extend(self.verse_chip_spans());
        // 1.2.18+ R.3 — reading-time chip (book length
        // + time remaining at editor.reading_wpm).
        spans.extend(self.reading_time_chip_spans());
        // 1.3.12 DEEP-1 — background-job spinner chip (persists regardless of
        // what else writes the status line).
        spans.extend(self.bg_job_chip_spans());
        // PANE-1 P4 — a thin separator delimits the transient status text from
        // the persistent state chips to its left, so the eye can tell
        // "what just happened" apart from "what's always true".
        if !self.status.is_empty() {
            spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
            spans.push(Span::raw(self.status.clone()));
        } else {
            spans.push(Span::raw("  "));
        }

        // PANE-1 P4 — split the bar into two non-overlapping regions instead of
        // painting the left spans and the right-aligned progress widget onto the
        // SAME rect (where a long status could overwrite the progress). The left
        // region (chips + status) truncates within itself; the progress widget
        // owns a reserved right column and is always visible.
        let progress_spans = self.progress_widget_spans();
        if progress_spans.is_empty() {
            f.render_widget(Paragraph::new(Line::from(spans)), area);
        } else {
            let progress_w: u16 = progress_spans
                .iter()
                .map(|s| s.content.chars().count() as u16)
                .sum::<u16>()
                .saturating_add(1); // one column of breathing room
            let chunks = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Horizontal)
                .constraints([
                    ratatui::layout::Constraint::Min(10),
                    ratatui::layout::Constraint::Length(progress_w),
                ])
                .split(area);
            f.render_widget(Paragraph::new(Line::from(spans)), chunks[0]);
            f.render_widget(
                Paragraph::new(Line::from(progress_spans))
                    .alignment(ratatui::layout::Alignment::Right),
                chunks[1],
            );
        }
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

#[cfg(test)]
mod highlight_cache_tests {
    use super::buffer_content_hash;

    #[test]
    fn hash_is_stable_for_identical_buffers() {
        let a = vec!["line one".to_string(), "line two".to_string()];
        let b = vec!["line one".to_string(), "line two".to_string()];
        assert_eq!(buffer_content_hash(&a), buffer_content_hash(&b));
    }

    #[test]
    fn hash_changes_when_content_changes() {
        let base = vec!["the cat".to_string(), "sat".to_string()];
        let edited = vec!["the cat".to_string(), "sat down".to_string()];
        assert_ne!(buffer_content_hash(&base), buffer_content_hash(&edited));
    }

    #[test]
    fn line_boundary_shifts_change_the_hash() {
        // Hashing per line (not the joined string) so a line split/merge that
        // keeps the same characters still invalidates the cache.
        let one = vec!["ab".to_string()];
        let two = vec!["a".to_string(), "b".to_string()];
        assert_ne!(buffer_content_hash(&one), buffer_content_hash(&two));
    }
}
