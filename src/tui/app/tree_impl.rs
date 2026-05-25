//! Tree-pane navigation methods on `App` — explicit expand /
//! collapse / row-layout helpers used by the binding-table
//! arms that route to "tree pane only" actions. Other
//! tree-related dispatch (`move_cursor`, mark toggles, etc.)
//! stays in `tui::app` because those methods also drive non-
//! tree state. Extracted from `tui::app` in the 1.2.7 refactor,
//! Phase 3 batch 6.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use uuid::Uuid;

use crate::store::node::NodeKind;

use super::super::status_helpers::{display_status, status_letter, status_style};
use super::super::text_utils::wrap_words_or_chars;

impl super::App {

    pub(super) fn tree_expand_at_cursor(&mut self) {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            return;
        };
        if node.kind == NodeKind::Paragraph {
            return;
        }
        if self.collapsed_nodes.remove(&id) {
            self.rebuild_rows_preserving_cursor();
        }
    }

    pub(super) fn tree_collapse_or_step_out(&mut self) {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            return;
        };

        let is_branch = node.kind != NodeKind::Paragraph;
        let has_children = is_branch && self.hierarchy.has_children(id);
        let is_currently_collapsed = self.collapsed_nodes.contains(&id);

        if is_branch && has_children && !is_currently_collapsed {
            self.collapsed_nodes.insert(id);
            self.rebuild_rows_preserving_cursor();
            return;
        }

        // Otherwise step out to parent.
        if let Some(parent_id) = node.parent_id {
            if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == parent_id) {
                self.tree_cursor = i;
            }
        }
    }

    /// Collapse the cursor's enclosing Subchapter. If the cursor is on a
    /// Subchapter itself, collapse it directly. Walks ancestors otherwise;
    /// no-op if no Subchapter is in scope (e.g. cursor on a chapter or
    /// directly under a book). After collapsing, the tree cursor moves to
    /// the now-folded subchapter row so the user sees what happened.
    pub(super) fn collapse_enclosing_subchapter(&mut self) {
        let Some(&(id, _)) = self.rows.get(self.tree_cursor) else {
            self.status = "nothing selected".into();
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            return;
        };
        // Pick the cursor's enclosing subchapter — itself if it IS one,
        // otherwise the nearest ancestor of kind Subchapter.
        let target = if node.kind == NodeKind::Subchapter {
            Some(node.id)
        } else {
            self.hierarchy
                .ancestors(node)
                .into_iter()
                .find(|a| a.kind == NodeKind::Subchapter)
                .map(|a| a.id)
        };
        let Some(target_id) = target else {
            self.status = "no enclosing subchapter to collapse".into();
            return;
        };
        if self.collapsed_nodes.insert(target_id) {
            self.rebuild_rows_preserving_cursor();
            // Land the cursor on the freshly-collapsed subchapter row so
            // the user can see what was folded.
            if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == target_id) {
                self.tree_cursor = i;
            }
            let title = self
                .hierarchy
                .get(target_id)
                .map(|n| n.title.as_str())
                .unwrap_or("?");
            self.status = format!("collapsed subchapter `{title}`");
        } else {
            self.status = "subchapter is already collapsed".into();
        }
    }

    /// Collapse every branch that has children. Paragraphs and empty
    /// branches are untouched (they wouldn't render differently anyway).
    /// The tree cursor stays on the same node if it survives the fold;
    /// otherwise `rebuild_rows_preserving_cursor` snaps it to the nearest
    /// remaining visible row.
    pub(super) fn collapse_all_branches(&mut self) {
        let mut added = 0usize;
        let candidates: Vec<Uuid> = self
            .hierarchy
            .iter()
            .filter(|n| n.kind != NodeKind::Paragraph && self.hierarchy.has_children(n.id))
            .map(|n| n.id)
            .collect();
        for id in candidates {
            if self.collapsed_nodes.insert(id) {
                added += 1;
            }
        }
        if added == 0 {
            self.status = "all branches already collapsed".into();
            return;
        }
        self.rebuild_rows_preserving_cursor();
        self.status = format!("collapsed {added} branch(es)");
    }

    /// Exact visual height (in terminal lines) of one tree row
    /// at the given pane `width`. Delegates to `tree_row_lines`
    /// so the scroll loop and the renderer always agree on row
    /// height — no chance of "cursor row almost visible" drift.
    pub(super) fn tree_row_visual_height(&self, row_idx: usize, width: usize) -> usize {
        self.tree_row_lines(row_idx, width).len().max(1)
    }

    /// Build the styled `Line`s for a single tree row. Returns
    /// one Line when the row fits on a single visual line;
    /// otherwise returns N+1 Lines where the title wraps with a
    /// hanging indent matching the row's prefix column (so
    /// continuation lines start under the title, not at column
    /// zero). Pips ride on the last title line when they fit,
    /// otherwise they get their own hanging-indent line.
    pub(super) fn tree_row_lines(&self, row_idx: usize, width: usize) -> Vec<Line<'_>> {
        let Some(&(id, depth)) = self.rows.get(row_idx) else {
            return vec![Line::from("")];
        };
        let Some(node) = self.hierarchy.get(id) else {
            return vec![Line::from("")];
        };
        let open_id: Option<Uuid> = self.opened.as_ref().map(|d| d.id);
        let is_open = open_id.is_some_and(|o| o == node.id);
        let is_collapsed = self.collapsed_nodes.contains(&node.id);
        let marker = if is_open {
            "►"
        } else {
            match node.kind {
                NodeKind::Paragraph => {
                    // 1.2.6+ events outrank hjson — an event
                    // paragraph that also stores hjson body
                    // still reads first as a timeline event.
                    if node.event.is_some() {
                        "◆ "
                    } else {
                        match node.content_type.as_deref() {
                            Some("hjson") => "❴ ",
                            _ => "¶ ",
                        }
                    }
                }
                NodeKind::Image => "▣ ",
                NodeKind::Script => "λ ",
                _ => {
                    if is_collapsed {
                        "▸ "
                    } else {
                        "▾ "
                    }
                }
            }
        };
        let kind_fg = match node.kind {
            NodeKind::Book => self.theme.tree_book_fg,
            NodeKind::Chapter => self.theme.tree_chapter_fg,
            NodeKind::Subchapter => self.theme.tree_subchapter_fg,
            NodeKind::Paragraph => self.theme.tree_paragraph_fg,
            NodeKind::Image => self.theme.tree_image_fg,
            NodeKind::Script => self.theme.tree_script_fg,
        };
        let mut row_style = Style::default().fg(kind_fg);
        if matches!(node.kind, NodeKind::Book | NodeKind::Chapter) {
            row_style = row_style.add_modifier(Modifier::BOLD);
        }
        if is_open {
            row_style = row_style
                .fg(self.theme.tree_open_marker)
                .add_modifier(Modifier::BOLD);
        }
        let is_cursor = row_idx == self.tree_cursor;
        if is_cursor {
            row_style = row_style.add_modifier(Modifier::REVERSED);
        }

        let indent_str = "  ".repeat(depth);
        let select_prefix = if self.tree_marked.contains(&node.id) {
            "✓ "
        } else if !self.tree_marked.is_empty()
            && matches!(node.kind, NodeKind::Paragraph)
        {
            "  "
        } else {
            ""
        };
        let prefix_str = format!("{indent_str}{select_prefix}{marker}");
        let status_label = if matches!(node.kind, NodeKind::Paragraph) {
            display_status(node.status.as_deref())
        } else {
            "None"
        };
        let status_letter = status_letter(status_label);
        let status_badge_style = status_style(status_label, &self.theme);
        let status_str = format!("{status_letter} ");

        // The hanging indent (continuation column) sits where
        // the title starts — after prefix + status badge.
        let prefix_width = prefix_str.chars().count() + status_str.chars().count();

        // Trailing pips (progress + tags + "+N") — built once,
        // appended to whichever Line carries the title's last
        // chunk.
        let mut pip_spans: Vec<Span<'_>> = Vec::new();
        if matches!(node.kind, NodeKind::Paragraph) {
            if let Some(target) = node.target_words.filter(|n| *n > 0) {
                let pct =
                    (node.word_count as i64 * 100 / target as i64).clamp(0, 999);
                let pip = if pct >= 100 {
                    "●"
                } else if pct >= 75 {
                    "◕"
                } else if pct >= 50 {
                    "◑"
                } else if pct >= 25 {
                    "◔"
                } else {
                    "○"
                };
                let style = if pct >= 100 {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else if pct >= 75 {
                    Style::default().fg(Color::LightGreen)
                } else if pct >= 50 {
                    Style::default().fg(Color::Yellow)
                } else if pct >= 25 {
                    Style::default().fg(Color::LightRed)
                } else {
                    Style::default().fg(Color::Red).add_modifier(Modifier::DIM)
                };
                pip_spans.push(Span::raw(" "));
                pip_spans.push(Span::styled(pip.to_string(), style));
            }
        }
        if matches!(node.kind, NodeKind::Paragraph) && !node.tags.is_empty() {
            let tag_style = Style::default()
                .fg(self.theme.tree_script_fg)
                .add_modifier(Modifier::DIM);
            for tag in node.tags.iter().take(2) {
                let short: String = if tag.chars().count() > 10 {
                    let truncated: String = tag.chars().take(9).collect();
                    format!("{truncated}…")
                } else {
                    tag.clone()
                };
                pip_spans.push(Span::raw(" "));
                pip_spans.push(Span::styled(format!("#{short}"), tag_style));
            }
            if node.tags.len() > 2 {
                pip_spans.push(Span::styled(
                    format!(" +{}", node.tags.len() - 2),
                    tag_style,
                ));
            }
        }
        let pip_width: usize = pip_spans
            .iter()
            .map(|s| s.content.chars().count())
            .sum();

        // Wrap the title. Title chunks fill the pane width
        // minus the prefix; pips ride on the LAST chunk's line
        // when they fit, else get their own hanging-indent line.
        let title_budget = width.saturating_sub(prefix_width).max(1);
        let chunks = wrap_words_or_chars(&node.title, title_budget);
        let last_idx = chunks.len().saturating_sub(1);
        let last_chunk_width = chunks.last().map(|s| s.chars().count()).unwrap_or(0);
        let pips_fit_on_last = pip_width == 0
            || last_chunk_width + pip_width <= title_budget;

        let mut out: Vec<Line<'_>> = Vec::with_capacity(chunks.len() + 1);
        for (i, chunk) in chunks.iter().enumerate() {
            let is_last = i == last_idx;
            let mut spans: Vec<Span<'_>> = Vec::new();
            if i == 0 {
                spans.push(Span::styled(prefix_str.clone(), row_style));
                spans.push(Span::styled(
                    status_str.clone(),
                    if status_label == "None" {
                        Style::default().add_modifier(Modifier::DIM)
                    } else {
                        status_badge_style
                    },
                ));
            } else {
                // Hanging indent — whitespace styled with
                // row_style so the cursor's REVERSED highlight
                // bar extends across the continuation column.
                spans.push(Span::styled(" ".repeat(prefix_width), row_style));
            }
            spans.push(Span::styled(chunk.clone(), row_style));
            if is_last && pips_fit_on_last {
                spans.extend(pip_spans.iter().cloned());
            }
            out.push(Line::from(spans));
        }
        if !pips_fit_on_last && !pip_spans.is_empty() {
            let mut spans: Vec<Span<'_>> = Vec::new();
            spans.push(Span::styled(" ".repeat(prefix_width), row_style));
            spans.extend(pip_spans.into_iter());
            out.push(Line::from(spans));
        }
        if out.is_empty() {
            out.push(Line::from(""));
        }
        out
    }

}
