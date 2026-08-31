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

    /// Sum `(word_count, target_words)` over every descendant of `id`.
    /// Paragraphs carry the counts; branches contribute their whole
    /// subtree. Used for the branch-level roll-up gauge — literary scale
    /// keeps the recursive walk negligible, and with the 3.8 dirty flag it
    /// only runs when the frame actually redraws.
    /// Jump the cursor to the previous/next major structural row — a Book or
    /// Chapter — in the flattened tree. Fast navigation past long paragraph
    /// runs. Stops at the first/last such row (no wrap); reports when there is
    /// none in that direction.
    pub(super) fn jump_structural(&mut self, forward: bool) {
        let is_major = |id: Uuid| {
            self.hierarchy
                .get(id)
                .is_some_and(|n| matches!(n.kind, NodeKind::Book | NodeKind::Chapter))
        };
        let start = self.tree_cursor;
        let target = if forward {
            (start + 1..self.rows.len()).find(|&i| is_major(self.rows[i].0))
        } else {
            (0..start).rev().find(|&i| is_major(self.rows[i].0))
        };
        match target {
            Some(i) => self.tree_cursor = i,
            None => {
                self.status = if forward {
                    "no chapter below".into()
                } else {
                    "no chapter above".into()
                };
            }
        }
    }

    pub(super) fn subtree_word_totals(&self, id: Uuid) -> (u64, i32) {
        let mut words: u64 = 0;
        let mut target: i32 = 0;
        for child in self.hierarchy.children_of(Some(id)) {
            words = words.saturating_add(child.word_count);
            if let Some(t) = child.target_words.filter(|t| *t > 0) {
                target = target.saturating_add(t);
            }
            let (w, t) = self.subtree_word_totals(child.id);
            words = words.saturating_add(w);
            target = target.saturating_add(t);
        }
        (words, target)
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
                            // STRUCT-1 — Jinja template paragraph.
                            Some("jinja") => "⟡ ",
                            // STRUCT-2 — `para:*` structural subtype glyph
                            // (code / admonition / math / procedure / table);
                            // WORLD-6 — `para:utopia-*` declaration glyph
                            // (⊢ ⚙ ⇒ ∅); MYTH-1 — `para:myth-*` declaration
                            // glyph (⊛ ∿ ⍟); else prose `¶`.
                            // POEM-1 — `para:verse-*` verse glyph (‖ ♩ ‗ ⁚ ⁛ ⇄).
                            _ => crate::myth::myth_glyph(node)
                                .or_else(|| crate::world::utopia::utopia_glyph(node))
                                .or_else(|| crate::poetry::verse_glyph(node))
                                .or_else(|| super::structural_glyph(node))
                                .unwrap_or("¶ "),
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
        // 3.8 — bookmark marker: paragraphs flagged via Ctrl+V B show a flag
        // glyph so bookmarked rows are visible while navigating (the jump
        // itself stays in the Ctrl+V M picker).
        if matches!(node.kind, NodeKind::Paragraph) && node.bookmark {
            pip_spans.push(Span::raw(" "));
            pip_spans.push(Span::styled(
                "⚑",
                Style::default().fg(Color::LightMagenta),
            ));
        }
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
        // 3.8 — branch word-count roll-up: aggregate descendant word count on
        // Book / Chapter / Subchapter rows (a chapter finally says how long it
        // is), plus a roll-up progress pip when the subtree carries any
        // target-word goals. Paragraph counts live in their own pip above.
        if matches!(
            node.kind,
            NodeKind::Book | NodeKind::Chapter | NodeKind::Subchapter
        ) {
            let (words, target) = self.subtree_word_totals(node.id);
            if words > 0 {
                pip_spans.push(Span::raw(" "));
                pip_spans.push(Span::styled(
                    fmt_wordcount(words),
                    Style::default().add_modifier(Modifier::DIM),
                ));
            }
            if target > 0 {
                let pct = (words as i64 * 100 / target as i64).clamp(0, 999);
                let (pip, style) = if pct >= 100 {
                    ("●", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
                } else if pct >= 75 {
                    ("◕", Style::default().fg(Color::LightGreen))
                } else if pct >= 50 {
                    ("◑", Style::default().fg(Color::Yellow))
                } else if pct >= 25 {
                    ("◔", Style::default().fg(Color::LightRed))
                } else {
                    ("○", Style::default().fg(Color::Red).add_modifier(Modifier::DIM))
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
        // 1.3.34+ — report-card badge: open Output findings under this node (a
        // count + worst-severity glyph), aggregated up from the source paragraphs.
        if let Some((count, sev)) = self.tree_badges.get(&node.id) {
            if *count > 0 {
                use crate::pane::output::Severity;
                let (glyph, color) = match sev {
                    Severity::Contradiction => ("⊗", Color::Red),
                    Severity::Warning => ("⚠", Color::Yellow),
                    Severity::Progress => ("↻", Color::Cyan),
                    _ => ("●", Color::Gray),
                };
                pip_spans.push(Span::raw(" "));
                pip_spans.push(Span::styled(
                    format!("{glyph}{count}"),
                    Style::default().fg(color),
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

/// Compact word-count label for the branch roll-up pip: exact under 1 000,
/// then `k` with one decimal (`12.3k`), dropping the decimal on round
/// thousands (`12k`). Keeps the pip narrow in a slim pane.
fn fmt_wordcount(n: u64) -> String {
    if n < 1000 {
        return n.to_string();
    }
    let thousands = n as f64 / 1000.0;
    if n % 1000 == 0 {
        format!("{}k", n / 1000)
    } else {
        format!("{thousands:.1}k")
    }
}

#[cfg(test)]
mod tests_rollup {
    use super::fmt_wordcount;

    #[test]
    fn fmt_wordcount_is_compact() {
        assert_eq!(fmt_wordcount(0), "0");
        assert_eq!(fmt_wordcount(999), "999");
        assert_eq!(fmt_wordcount(1000), "1k");
        assert_eq!(fmt_wordcount(12_000), "12k");
        assert_eq!(fmt_wordcount(12_345), "12.3k");
    }
}
