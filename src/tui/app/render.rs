//! Rendering methods for `App` — every `draw_*` modal + pane
//! painter, plus the one `render_template` text-substitution
//! helper they all consult. Sub-module of `tui::app`, so it has
//! direct access to `App`'s private fields (parent-private items
//! are visible to children). Extracted from `tui::app` in the
//! 1.2.7 refactor, Phase 3 batch 1.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use super::{
    digit_count, filter_functions, filter_tag_results, find_cursor_visual,
    format_entry_line, format_progress_gauge, fuzzy_filter_entries,
    highlight_for_content, highlight_substring_in_line, reverse_chip,
};

use crate::store::InsertPosition;

use super::super::credits::build_credits_lines;
use super::super::diff_utils::{SnapshotDiffKind, SnapshotDiffRow};
use super::super::file_picker::{FilePicker, PickerContext};
use super::super::focus::Focus;
use super::super::highlight::{
    build_row_spans, build_visual_row_spans, diff_added, wrap_line, RowHit,
};
use super::super::inference::{AiMode, InferenceStatus};
use super::super::modal::{
    Modal,
    PromptSource, ScriptPickerScope, TagPickerTarget, visible_event_entries,
};
use super::super::quickref;
use super::super::search_replace::{row_matches, RowMatch};
use super::super::state::LinkPickDirection;
use super::super::status_helpers::status_style;
use super::super::text_utils::{
    format_active_duration,
    format_age_humantime, format_reading_time, truncate_label,
};
use super::super::timeline_state::TimelineEvent;

impl super::App {

    pub(super) fn render_template(&self, template: &str) -> String {
        let selection = self.current_selection_or_paragraph();
        let context = self.current_context_breadcrumb();
        template
            .replace("{{selection}}", &selection)
            .replace("{{context}}", &context)
            .trim()
            .to_string()
    }

    pub(super) fn draw_book_info_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
        scroll: usize,
    ) {
        let lines = self.build_book_info_lines();
        let total = lines.len();

        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = " Book info · Ctrl+B I ".to_string();
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1) as usize;
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };

        let max_scroll = total.saturating_sub(body_h);
        let scroll = scroll.min(max_scroll);
        let end = (scroll + body_h).min(total);
        let visible: Vec<Line<'_>> = lines[scroll..end].to_vec();
        f.render_widget(Paragraph::new(visible), body_rect);

        let at_end = end >= total;
        let more_hint = if at_end { " " } else { " · more below" };
        let hint = format!(
            " ↑↓ / PgUp/PgDn / Home/End scroll · Esc close{more_hint}    (showing {}–{} of {total}) ",
            scroll + 1,
            end
        );
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    pub(super) fn draw_llm_picker_modal(&self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::LlmPicker {
            providers,
            cursor,
            initial_default,
        } = &self.modal
        else {
            return;
        };

        // Build the visible lines so we can size the modal to fit.
        let header_lines = 2; // title + blank
        let footer_lines = 2; // blank + hint
        let body_lines = providers.len();
        let height = (header_lines + body_lines + footer_lines + 2) as u16;
        let height = height.clamp(8, area.height.saturating_sub(2));

        // Widest provider name + model for column alignment.
        let max_name = providers.iter().map(|p| p.chars().count()).max().unwrap_or(8);
        let max_model = providers
            .iter()
            .filter_map(|p| self.cfg.llm.providers.get(p).map(|c| c.model.chars().count()))
            .max()
            .unwrap_or(8);
        let width = (max_name + max_model + 28) as u16;
        let width = width.clamp(50, area.width.saturating_sub(6));

        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Switch LLM provider · Ctrl+B L ")
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(""));
        for (i, name) in providers.iter().enumerate() {
            let prov = self.cfg.llm.providers.get(name);
            let model = prov.map(|p| p.model.as_str()).unwrap_or("?");
            let api_key_state = prov
                .and_then(|p| p.api_key_env.clone())
                .map(|env| {
                    if std::env::var(&env).is_ok() {
                        format!("· {env} set")
                    } else {
                        format!("· {env} MISSING")
                    }
                })
                .unwrap_or_else(|| "· local (no key)".to_string());
            let marker = if i == *cursor { "›" } else { " " };
            let current_tag = if name == initial_default {
                "  (current)"
            } else {
                ""
            };
            let name_padded = format!("{name:<width$}", width = max_name);
            let model_padded = format!("{model:<width$}", width = max_model);
            let row = format!(
                "  {marker} {name_padded}   {model_padded}   {api_key_state}{current_tag}"
            );
            let style = if i == *cursor {
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .add_modifier(Modifier::BOLD)
            } else if name == initial_default {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(row, style)));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  ↑↓ to select · Enter to switch · Esc to cancel".to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )));

        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }

    pub(super) fn draw_image_picker_modal(&self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::ImagePicker {
            entries, cursor, ..
        } = &self.modal
        else {
            return;
        };
        let header_lines = 2usize;
        let footer_lines = 2usize;
        let body_lines = entries.len().max(1);
        let height = ((header_lines + body_lines + footer_lines + 2) as u16)
            .clamp(8, area.height.saturating_sub(2));
        let max_name = entries
            .iter()
            .map(|e| e.fname.chars().count())
            .max()
            .unwrap_or(16);
        let max_title = entries
            .iter()
            .map(|e| e.title.chars().count())
            .max()
            .unwrap_or(16);
        let width = ((max_name + max_title + 24) as u16).clamp(50, area.width.saturating_sub(6));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Pick an image · Ctrl+B P ")
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(""));
        if entries.is_empty() {
            lines.push(Line::from(Span::styled(
                "  No Image siblings at this level. Use F3 to import one,"
                    .to_string(),
                Style::default().add_modifier(Modifier::DIM),
            )));
            lines.push(Line::from(Span::styled(
                "  then re-run Ctrl+B P inside the #image(\"…\") call."
                    .to_string(),
                Style::default().add_modifier(Modifier::DIM),
            )));
        } else {
            for (i, e) in entries.iter().enumerate() {
                let marker = if i == *cursor { "›" } else { " " };
                let name_padded =
                    format!("{n:<width$}", n = e.fname, width = max_name);
                let title_padded =
                    format!("{t:<width$}", t = e.title, width = max_title);
                let size_kib = e.size_bytes / 1024;
                let row = format!("  {marker} {name_padded}   {title_padded}   ({size_kib} KiB)");
                let style = if i == *cursor {
                    Style::default()
                        .add_modifier(Modifier::REVERSED)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(row, style)));
            }
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  ↑↓ select · Enter insert · Esc cancel".to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )));
        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }

    pub(super) fn draw_function_picker_modal(&self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::FunctionPicker { filter, cursor } = &self.modal else {
            return;
        };
        let matches = filter_functions(filter.as_str());
        let width = area.width.saturating_sub(6).max(60);
        let height = area.height.saturating_sub(4).max(14);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let title = " Typst function · Ctrl+B F ".to_string();
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        // 3 rows of chrome: filter, blank spacer, footer.
        let filter_h: u16 = 2;
        let footer_h: u16 = 2;
        let list_h = inner.height.saturating_sub(filter_h + footer_h);
        let filter_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: filter_h,
        };
        let list_rect = Rect {
            x: inner.x,
            y: inner.y + filter_h,
            width: inner.width,
            height: list_h,
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + filter_h + list_h,
            width: inner.width,
            height: footer_h,
        };

        let cursor_char = '│';
        let filter_lines = vec![
            Line::from(Span::styled(
                format!(" › Filter: {}", filter.render_with_cursor(cursor_char)),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                format!(
                    "   {} match{} of {}",
                    matches.len(),
                    if matches.len() == 1 { "" } else { "es" },
                    super::super::typst_funcs::all().len()
                ),
                Style::default().add_modifier(Modifier::DIM),
            )),
        ];
        f.render_widget(Paragraph::new(filter_lines), filter_rect);

        // List body — scroll so the cursor is always in view.
        let body_height = list_h as usize;
        let total = matches.len();
        let cursor = (*cursor).min(total.saturating_sub(1));
        let scroll = if cursor >= body_height {
            cursor - body_height + 1
        } else {
            0
        };
        let max_name = matches
            .iter()
            .map(|f| f.name.chars().count())
            .max()
            .unwrap_or(8);

        let mut rows: Vec<Line<'static>> = Vec::new();
        if matches.is_empty() {
            rows.push(Line::from(Span::styled(
                "  (no functions match the filter)".to_string(),
                Style::default().add_modifier(Modifier::DIM),
            )));
        }
        let body_end = (scroll + body_height).min(total);
        for i in scroll..body_end {
            let entry = matches[i];
            let marker = if i == cursor { "›" } else { " " };
            let name_padded =
                format!("{n:<width$}", n = entry.name, width = max_name);
            let line = format!("  {marker} {name_padded}   {desc}", desc = entry.description);
            let style = if i == cursor {
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            rows.push(Line::from(Span::styled(line, style)));
        }
        // Also include the signature underneath the selected entry as
        // a hint row. Kept narrow to avoid pushing the list off-screen.
        f.render_widget(Paragraph::new(rows), list_rect);

        let signature_hint = matches
            .get(cursor)
            .map(|f| format!(" sig: {}", f.signature))
            .unwrap_or_default();
        let hint = format!(
            "{signature_hint}\n ↑↓ select · Enter inserts #name(…) · Esc cancel"
        );
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            )))
            .wrap(Wrap { trim: false }),
            footer_rect,
        );
    }

    pub(super) fn draw_image_preview_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        // Pull the variant fields out by value (cloning the cheap
        // strings/numbers) and take a `&mut` borrow of `proto` only
        // for the render call — keeps the modal field accessible
        // for read elsewhere if needed.
        let Modal::ImagePreview {
            title,
            fs_rel,
            size_bytes,
            proto,
        } = &mut self.modal
        else {
            return;
        };

        let width = area.width.saturating_sub(4).max(40);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let title_line = format!(
            " 🖼 {title}  ·  {fs_rel}  ·  {size_bytes} bytes "
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title_line)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        // Reserve the last inner row for the hint line.
        let body_h = inner.height.saturating_sub(1);
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: body_h,
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + body_h,
            width: inner.width,
            height: 1,
        };

        let widget = ratatui_image::StatefulImage::new();
        f.render_stateful_widget(widget, body_rect, proto);

        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  Esc closes  ·  resize the terminal to re-fit ".to_string(),
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// Ctrl+V R floating preview. Same plumbing as the image-
    /// preview modal — ratatui-image's StatefulImage widget
    /// repaints on every frame so a terminal resize Just Works.
    /// Multi-page documents: ← / → cycle between page protos.
    pub(super) fn draw_rendered_preview_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::RenderedPreview {
            title,
            pages,
            current_page,
            ..
        } = &mut self.modal
        else {
            return;
        };
        let total = pages.len();
        let idx = (*current_page).min(total.saturating_sub(1));
        let page = match pages.get_mut(idx) {
            Some(p) => p,
            None => return,
        };
        let preview_width = page.width;
        let preview_height = page.height;

        let width = area.width.saturating_sub(4).max(40);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let pages_note = if total > 1 {
            format!(" · page {}/{}", idx + 1, total)
        } else {
            String::new()
        };
        let title_line = format!(
            " 🖨 {title}  ·  {preview_width}×{preview_height}{pages_note} "
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title_line)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1);
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: body_h,
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + body_h,
            width: inner.width,
            height: 1,
        };

        let widget = ratatui_image::StatefulImage::new();
        f.render_stateful_widget(widget, body_rect, &mut page.proto);

        let hint = if total > 1 {
            "  ← / → navigate  ·  S saves current  ·  A saves all  ·  Esc closes ".to_string()
        } else {
            "  Esc closes  ·  S saves full-DPI PNG  ·  A saves all (same here) ".to_string()
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// Save-as picker triggered by `S` in the rendered preview.
    /// Same dimensions / style as the markdown save-as picker so
    /// the UX is consistent.
    pub(super) fn draw_save_rendered_png_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::SaveRenderedPng { input, title, .. } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(8).clamp(40, 96);
        let height: u16 = 7;
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Save rendered PNG · {title} "))
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let cursor = '│';
        let body = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!(" Path: {}", input.render_with_cursor(cursor)),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Enter saves · Esc cancels · ~/ expands to home".to_string(),
                Style::default().add_modifier(Modifier::DIM),
            )),
        ];
        f.render_widget(Paragraph::new(body), inner);
    }

    /// Ctrl+V W floating preview. Same plumbing as the paragraph
    /// render preview, but single-page (no navigation) — DOT
    /// layout produces one canvas.
    pub(super) fn draw_story_view_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::StoryView {
            book_title,
            width,
            height,
            proto,
            ..
        } = &mut self.modal
        else {
            return;
        };

        let render_w = area.width.saturating_sub(4).max(40);
        let render_h = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(render_w)) / 2;
        let y = area.y + (area.height.saturating_sub(render_h)) / 2;
        let rect = Rect { x, y, width: render_w, height: render_h };
        f.render_widget(ratatui::widgets::Clear, rect);

        let title_line = format!(" 🕸 Story · {book_title}  ·  {width}×{height} ");
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title_line)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1);
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: body_h,
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + body_h,
            width: inner.width,
            height: 1,
        };

        let widget = ratatui_image::StatefulImage::new();
        f.render_stateful_widget(widget, body_rect, proto);

        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  Esc closes  ·  S saves PNG  ·  resize terminal to re-fit ".to_string(),
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// `S` inside the story-view modal — small save-as picker
    /// for the rendered PNG.
    pub(super) fn draw_save_story_png_modal(&self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::SaveStoryPng { input, book_title, .. } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(8).clamp(40, 96);
        let height: u16 = 7;
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Save story PNG · {book_title} "))
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!(" Path: {}", input.render_with_cursor('│')),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Enter saves · Esc cancels · ~/ expands to home".to_string(),
                Style::default().add_modifier(Modifier::DIM),
            )),
        ];
        f.render_widget(Paragraph::new(body), inner);
    }

    pub(super) fn draw_status_filter_modal(&self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::StatusFilter { status_label, scope, entries, cursor } = &self.modal else {
            return;
        };
        let header_lines = 3usize; // title row inside chrome stays 0; footer grows
        let footer_lines = 3usize;
        let body_lines = entries.len().max(1);
        let height = ((header_lines + body_lines + footer_lines + 2) as u16)
            .clamp(10, area.height.saturating_sub(2));
        let max_title = entries.iter().map(|e| e.title.chars().count()).max().unwrap_or(20);
        let max_crumb = entries.iter().map(|e| e.breadcrumb.chars().count()).max().unwrap_or(30);
        let width = ((max_title + max_crumb + 12) as u16).clamp(60, area.width.saturating_sub(6));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let title = format!(" Paragraphs with status [{status_label}] · scope: {scope} · Ctrl+B {} ",
            match *status_label {
                "Ready" => "1",
                "Final" => "2",
                "Third" => "3",
                "Second" => "4",
                "First" => "5",
                "Napkin" => "6",
                "None" => "7",
                _ => "?",
            });
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(""));
        if entries.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("  No paragraphs tagged [{status_label}]."),
                Style::default().add_modifier(Modifier::DIM),
            )));
        } else {
            let body_h = inner.height.saturating_sub((header_lines + footer_lines) as u16) as usize;
            let body_h = body_h.max(1);
            let cursor = (*cursor).min(entries.len() - 1);
            let scroll = if cursor >= body_h { cursor - body_h + 1 } else { 0 };
            let end = (scroll + body_h).min(entries.len());
            for (i_offset, entry) in entries[scroll..end].iter().enumerate() {
                let i = scroll + i_offset;
                let marker = if i == cursor { "›" } else { " " };
                let title_padded =
                    format!("{t:<width$}", t = entry.title, width = max_title);
                let row =
                    format!("  {marker} {title_padded}   {b}", b = entry.breadcrumb);
                let style = if i == cursor {
                    Style::default()
                        .add_modifier(Modifier::REVERSED)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(row, style)));
            }
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  ↑↓ select · Enter opens · r/R advances status · - / Backspace reverses · Esc cancel"
                .to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )));
        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }

    pub(super) fn draw_timeline_view_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::TimelineView { state } = &self.modal else {
            return;
        };
        let modal_w = area.width.saturating_sub(4).max(80);
        let modal_h = area.height.saturating_sub(2).max(14);
        let x = area.x + (area.width.saturating_sub(modal_w)) / 2;
        let y = area.y + (area.height.saturating_sub(modal_h)) / 2;
        let rect = Rect { x, y, width: modal_w, height: modal_h };
        f.render_widget(ratatui::widgets::Clear, rect);

        let crumb = self.timeline_scope_crumb(state);
        let title = format!(
            " Timeline · {crumb} · {n} events · zoom {z:.2}× ",
            n = state.events.len(),
            z = 1.0 / state.ticks_per_cell,
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        // Layout columns:
        //   [ label_w ][ swim_w ]
        // label_w = max track-name width + padding (min 8,
        // max 18); swim_w fills the rest.
        let default_track = &self.cfg.timeline.default_track;
        let raw_rows = crate::tui::timeline_render::layout_swim_lanes(
            &state.events,
            state.scroll_ticks,
            state.ticks_per_cell,
            inner.width.saturating_sub(10) as usize, // tentative
            default_track,
            self.cfg.timeline.display.show_orphans,
        );
        let label_w = raw_rows
            .iter()
            .map(|r| r.label.chars().count())
            .max()
            .unwrap_or(4)
            .clamp(4, 16) as u16
            // +3 = leading space + expand glyph (▾/▸) + space
            // after it, before the label text starts.
            + 3;
        let swim_w = inner.width.saturating_sub(label_w);
        // Recompute with the final swim_w (label widths might
        // have changed how much room the lanes get).
        let rows = crate::tui::timeline_render::layout_swim_lanes(
            &state.events,
            state.scroll_ticks,
            state.ticks_per_cell,
            swim_w as usize,
            default_track,
            self.cfg.timeline.display.show_orphans,
        );

        // Time axis (1 row).
        let calendar =
            crate::timeline::Calendar::from_config(self.cfg.timeline.calendar.clone());
        let axis_labels = crate::tui::timeline_render::time_axis_labels(
            state.scroll_ticks,
            state.ticks_per_cell,
            swim_w as usize,
        );
        // 1.2.7+ — grid stripes every N days, from HJSON.
        let grid_cols: std::collections::HashSet<usize> =
            crate::tui::timeline_render::grid_columns(
                state.scroll_ticks,
                state.ticks_per_cell,
                swim_w as usize,
                self.cfg.timeline.display.grid_every_days,
            )
            .into_iter()
            .collect();
        let mut axis_chars: Vec<char> = vec![' '; swim_w as usize];
        let mut label_strings: Vec<(usize, String)> = Vec::new();
        for (col, tick) in &axis_labels {
            if *col < swim_w as usize {
                axis_chars[*col] = '│';
                let label = calendar.format(
                    crate::timeline::TimelinePoint::from_ticks(*tick),
                    crate::timeline::Precision::Day,
                );
                label_strings.push((*col, label));
            }
        }
        // Cursor column marker.
        let cursor_col = (((state.cursor_ticks - state.scroll_ticks) as f64)
            / state.ticks_per_cell)
            .round() as isize;
        if cursor_col >= 0 && (cursor_col as usize) < swim_w as usize {
            // Draw a `▾` cursor on the axis tick row.
            // Replace whatever was there.
            axis_chars[cursor_col as usize] = '▾';
        }
        // Build axis line: a row of marker chars + a row
        // beneath with label text staggered every N columns.
        let axis_spans: Vec<Span<'_>> = vec![
            Span::raw(" ".repeat(label_w as usize)),
            Span::styled(
                axis_chars.iter().collect::<String>(),
                Style::default().fg(self.theme.tree_chapter_fg),
            ),
        ];
        let mut label_row: String = " ".repeat(label_w as usize);
        let mut label_chars: Vec<char> = vec![' '; swim_w as usize];
        for (col, label) in &label_strings {
            for (i, c) in label.chars().enumerate() {
                let pos = col + i;
                if pos < label_chars.len() {
                    label_chars[pos] = c;
                }
            }
        }
        label_row.push_str(&label_chars.iter().collect::<String>());

        // Footer hint.
        let footer = " Tab/Shift+Tab cycle · Enter expand/open · Backspace up · ←/→ scroll · ↑/↓ event · Space collapse · +/- zoom · F12 critique · Esc close ";

        // Compose lines.
        let mut all_lines: Vec<Line<'_>> = Vec::new();
        all_lines.push(Line::from(axis_spans));
        all_lines.push(Line::from(Span::styled(
            label_row,
            Style::default().add_modifier(Modifier::DIM),
        )));
        all_lines.push(Line::from("".to_string()));
        // Swim-lane rows.
        let track_label_style = Style::default()
            .fg(self.theme.tree_subchapter_fg)
            .add_modifier(Modifier::BOLD);
        let dim_style = Style::default().add_modifier(Modifier::DIM);
        for row in &rows {
            // 1.2.7+ — collapsed track: emit a one-line
            // header with ▸ glyph + event count, skip the
            // swim-lane cell loop. Orphan row is never
            // collapsible (it's already a one-liner).
            let is_collapsed = !row.is_orphan_row
                && state.collapsed_tracks.contains(&row.label);
            let is_highlighted = state
                .track_highlight
                .as_deref()
                == Some(row.label.as_str());
            // Tree-style expand glyph: ▾ expanded, ▸ collapsed.
            // Orphan row keeps a blank prefix.
            let expand_glyph = if row.is_orphan_row {
                ' '
            } else if is_collapsed {
                '▸'
            } else {
                '▾'
            };
            if is_collapsed {
                let n_events = state
                    .events
                    .iter()
                    .filter(|e| {
                        !e.is_orphan
                            && self.timeline_event_track_key(e) == row.label
                    })
                    .count();
                let mut style = dim_style;
                if is_highlighted {
                    style = style.add_modifier(Modifier::BOLD);
                }
                let line = format!(
                    " {expand_glyph} {label} · {n_events} event{plural} (collapsed — Space to expand)",
                    label = row.label,
                    plural = if n_events == 1 { "" } else { "s" },
                );
                all_lines.push(Line::from(Span::styled(line, style)));
                continue;
            }
            let mut spans: Vec<Span<'_>> = Vec::new();
            let truncated = truncate_label(
                &row.label,
                label_w as usize - 3,
            );
            let label_text = format!(
                "{expand_glyph} {:<width$}",
                truncated,
                width = label_w as usize - 3,
            );
            let label_style = if row.is_orphan_row {
                dim_style
            } else if is_highlighted {
                track_label_style.add_modifier(Modifier::UNDERLINED)
            } else {
                track_label_style
            };
            spans.push(Span::styled(format!("{label_text} "), label_style));
            // Each cell becomes one Span so we can give
            // bars / dots / cursor different colours
            // without flickering.
            let mut buf: String = String::new();
            let mut cur_style: Style =
                Style::default().fg(self.theme.tree_paragraph_fg);
            let flush =
                |buf: &mut String, style: Style, spans: &mut Vec<Span<'_>>| {
                    if !buf.is_empty() {
                        spans.push(Span::styled(std::mem::take(buf), style));
                    }
                };
            for (col, cell) in row.cells.iter().enumerate() {
                let is_cursor =
                    cursor_col >= 0 && col == cursor_col as usize;
                let (glyph, style) = match cell {
                    None => {
                        let is_grid = grid_cols.contains(&col);
                        let g = if is_cursor {
                            '│'
                        } else if is_grid {
                            '┊'
                        } else {
                            ' '
                        };
                        let s = if is_cursor {
                            Style::default()
                                .fg(self.theme.tree_chapter_fg)
                                .add_modifier(Modifier::DIM)
                        } else if is_grid {
                            // 1.2.7+ grid stripe — faint vertical
                            // dotted bar so the eye gets a
                            // 7-day (or whatever step) ruler
                            // beneath the events.
                            Style::default()
                                .fg(self.theme.tree_chapter_fg)
                                .add_modifier(Modifier::DIM)
                        } else {
                            Style::default()
                        };
                        (g, s)
                    }
                    Some(tc) => {
                        // 1.2.7+ — the cell belongs to the
                        // user-selected event (set by ↑/↓
                        // navigation)? If so, paint it BOLD
                        // + REVERSED so the whole event span
                        // — endpoints and interior cells —
                        // stands out from the rest of the
                        // swim lane.
                        let is_selected = state
                            .selected_event_id
                            .is_some_and(|id| id == tc.event_id);
                        let s = if is_selected {
                            Style::default()
                                .fg(self.theme.tree_chapter_fg)
                                .add_modifier(
                                    Modifier::BOLD | Modifier::REVERSED,
                                )
                        } else if tc.is_orphan {
                            dim_style.fg(Color::Yellow)
                        } else if tc.is_endpoint {
                            Style::default()
                                .fg(self.theme.tree_chapter_fg)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(self.theme.tree_paragraph_fg)
                        };
                        (tc.glyph, s)
                    }
                };
                if style != cur_style && !buf.is_empty() {
                    flush(&mut buf, cur_style, &mut spans);
                    cur_style = style;
                } else if buf.is_empty() {
                    cur_style = style;
                }
                buf.push(glyph);
            }
            flush(&mut buf, cur_style, &mut spans);
            all_lines.push(Line::from(spans));

            // 1.2.7+ — expanded track: emit each event of
            // this track as an indented text sub-row beneath
            // the swim lane. Mirrors the tree pane's
            // "branch expanded → children visible" model.
            // Highlights the currently-selected event row
            // when focus_level == Event.
            if !row.is_orphan_row
                && state.expanded_track.as_deref() == Some(row.label.as_str())
            {
                let mut track_events: Vec<&TimelineEvent> = state
                    .events
                    .iter()
                    .filter(|e| {
                        !e.is_orphan
                            && self.timeline_event_track_key(e) == row.label
                    })
                    .collect();
                track_events.sort_by_key(|e| e.start_ticks);
                for ev in track_events {
                    let is_focused = state
                        .selected_event_id
                        .is_some_and(|id| id == ev.id);
                    let start_str = calendar.format(
                        crate::timeline::TimelinePoint::from_ticks(ev.start_ticks),
                        ev.precision,
                    );
                    let timing = match ev.end_ticks {
                        Some(end_t) => {
                            let e = calendar.format(
                                crate::timeline::TimelinePoint::from_ticks(end_t),
                                ev.precision,
                            );
                            format!("{start_str} → {e}")
                        }
                        None => start_str,
                    };
                    let n_links = ev.linked_paragraphs.len();
                    let links_str = match n_links {
                        0 => "no links".to_string(),
                        1 => "1 link".to_string(),
                        n => format!("{n} links"),
                    };
                    let bullet = if is_focused { '►' } else { '◆' };
                    let line_text = format!(
                        "       {bullet} {title}  ·  {timing}  ·  {links_str}",
                        title = truncate_label(&ev.title, 40),
                    );
                    let style = if is_focused {
                        Style::default()
                            .fg(self.theme.tree_chapter_fg)
                            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                    } else {
                        Style::default()
                            .fg(self.theme.tree_paragraph_fg)
                    };
                    all_lines.push(Line::from(Span::styled(line_text, style)));
                }
            }
        }
        // Pad to fill the body height with empty lines.
        let body_h = inner.height.saturating_sub(1);
        while all_lines.len() < body_h as usize {
            all_lines.push(Line::from(""));
        }
        // Cursor-tick readout row (last visible row, dim).
        let cursor_tick_str = calendar.format(
            crate::timeline::TimelinePoint::from_ticks(state.cursor_ticks),
            crate::timeline::Precision::Day,
        );
        let stat_row = format!(
            " ▾ cursor: {cursor_tick_str}   scroll: tick {scroll}   pps: {pps:.3}",
            scroll = state.scroll_ticks,
            pps = state.ticks_per_cell,
        );
        if let Some(last) = all_lines.last_mut() {
            *last = Line::from(Span::styled(stat_row, dim_style));
        }

        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: body_h,
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + body_h,
            width: inner.width,
            height: 1,
        };
        f.render_widget(Paragraph::new(all_lines), body_rect);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                footer,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );

        // 1.2.6+ — descent picker overlay. Renders above
        // the swim lanes when active.
        if let Some(descent) = state.descent.as_ref() {
            let dw = (modal_w / 2).max(40).min(modal_w - 4);
            let dh = (descent.choices.len() as u16 + 4).min(modal_h - 4);
            let dx = rect.x + (modal_w - dw) / 2;
            let dy = rect.y + (modal_h - dh) / 2;
            let drect = Rect { x: dx, y: dy, width: dw, height: dh };
            f.render_widget(ratatui::widgets::Clear, drect);
            let dblock = Block::default()
                .borders(Borders::ALL)
                .title(" Descend into … ")
                .border_style(
                    Style::default()
                        .fg(self.theme.modal_border)
                        .add_modifier(Modifier::BOLD),
                )
                .style(
                    Style::default()
                        .bg(self.theme.modal_bg)
                        .fg(self.theme.modal_fg),
                );
            let dinner = dblock.inner(drect);
            f.render_widget(dblock, drect);
            let dim_style = Style::default().add_modifier(Modifier::DIM);
            let mut dlines: Vec<Line<'_>> = Vec::new();
            dlines.push(Line::from(""));
            for (i, choice) in descent.choices.iter().enumerate() {
                let glyph = if choice.event_count == 0 {
                    "◌"
                } else {
                    "●"
                };
                let main = format!(
                    "  {arrow} {glyph}  {title}",
                    arrow = if i == descent.cursor { "→" } else { " " },
                    glyph = glyph,
                    title = choice.title,
                );
                let trail = format!("   {} event(s)", choice.event_count);
                let style = if i == descent.cursor {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else if choice.event_count == 0 {
                    dim_style
                } else {
                    Style::default()
                };
                dlines.push(Line::from(vec![
                    Span::styled(main, style),
                    Span::styled(trail, dim_style),
                ]));
            }
            dlines.push(Line::from(""));
            dlines.push(Line::from(Span::styled(
                "  ↑↓ select · Enter descends · Esc returns to same scope",
                dim_style,
            )));
            f.render_widget(Paragraph::new(dlines), dinner);
        }
    }

    pub(super) fn draw_event_picker_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::EventPicker {
            entries,
            cursor,
            track_filter,
        } = &self.modal
        else {
            return;
        };
        let visible = visible_event_entries(entries, track_filter.as_deref());
        let total = visible.len();

        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(14);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);
        let title = match track_filter {
            Some(t) => format!(" Events ({total}) · track: {t} "),
            None => format!(" Events ({total}) · all tracks "),
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1) as usize;
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };

        let scroll = if *cursor >= body_h {
            cursor - body_h + 1
        } else {
            0
        };
        let lines: Vec<Line<'_>> = visible
            .iter()
            .enumerate()
            .skip(scroll)
            .take(body_h)
            .map(|(i, e)| {
                let track = e.track.as_deref().unwrap_or("—");
                let head = format!(
                    " {start:>14} {glyph}  ",
                    start = e.start_str,
                    glyph = e.glyph,
                );
                let title_style = if e.is_orphan {
                    Style::default().add_modifier(Modifier::DIM)
                } else {
                    Style::default()
                };
                let trail = format!("  ({track})");
                let line = Line::from(vec![
                    Span::styled(head, Style::default().fg(Color::Cyan)),
                    Span::styled(e.title.clone(), title_style),
                    Span::styled(trail, Style::default().add_modifier(Modifier::DIM)),
                ]);
                if i == *cursor {
                    line.style(Style::default().add_modifier(Modifier::REVERSED))
                } else {
                    line
                }
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " ↑↓ select · Enter opens · t cycles tracks · Esc closes ",
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    pub(super) fn draw_diagnostics_list_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::DiagnosticsList { cursor } = &self.modal else {
            return;
        };
        let diags: Vec<crate::typst_check::TypstDiagnostic> = self
            .opened
            .as_ref()
            .map(|d| d.typst_diagnostics.clone())
            .unwrap_or_default();
        let total = diags.len();

        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Typst diagnostics ({total}) "))
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1) as usize;
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };

        let scroll = if *cursor >= body_h {
            cursor - body_h + 1
        } else {
            0
        };
        let lines: Vec<Line<'_>> = diags
            .iter()
            .enumerate()
            .skip(scroll)
            .take(body_h)
            .map(|(i, d)| {
                let head = format!(" line {:>4}:{:<3} ", d.line, d.col);
                let line = Line::from(vec![
                    Span::styled(
                        head,
                        Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(d.message.clone()),
                ]);
                if i == *cursor {
                    line.style(Style::default().add_modifier(Modifier::REVERSED))
                } else {
                    line
                }
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " ↑↓ select · Enter jumps cursor · Esc closes ",
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// Side-by-side renderer for `Modal::AiDiffReview`. Uses
    /// `similar::TextDiff::from_lines` to mark inserted /
    /// removed lines; the two columns are aligned so paired
    /// changes land on the same screen row when possible.
    pub(super) fn draw_ai_diff_review_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::AiDiffReview {
            before_lines,
            after_lines,
            scroll,
            ..
        } = &self.modal
        else {
            return;
        };
        let width = area.width.saturating_sub(4).max(80);
        let height = area.height.saturating_sub(4).max(20);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" AI diff review — a accept · r reject ")
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let footer_h: u16 = 1;
        let body_h = inner.height.saturating_sub(footer_h) as usize;
        let half = inner.width / 2;
        let before_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: half,
            height: inner.height.saturating_sub(footer_h),
        };
        let after_rect = Rect {
            x: inner.x + half,
            y: inner.y,
            width: inner.width - half,
            height: inner.height.saturating_sub(footer_h),
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(footer_h),
            width: inner.width,
            height: footer_h,
        };

        let before_text = before_lines.join("\n");
        let after_text = after_lines.join("\n");
        let diff = similar::TextDiff::from_lines(&before_text, &after_text);
        let mut left: Vec<Line> = Vec::new();
        let mut right: Vec<Line> = Vec::new();
        for change in diff.iter_all_changes() {
            let raw = change.value().trim_end_matches('\n').to_string();
            match change.tag() {
                similar::ChangeTag::Equal => {
                    let line = Line::from(format!("  {raw}"));
                    left.push(line.clone());
                    right.push(line);
                }
                similar::ChangeTag::Delete => {
                    left.push(Line::from(Span::styled(
                        format!("- {raw}"),
                        Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    )));
                    right.push(Line::from(""));
                }
                similar::ChangeTag::Insert => {
                    left.push(Line::from(""));
                    right.push(Line::from(Span::styled(
                        format!("+ {raw}"),
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    )));
                }
            }
        }
        let total = left.len();
        let start = (*scroll).min(total.saturating_sub(1));
        let take = body_h;
        let left_view: Vec<Line> =
            left.into_iter().skip(start).take(take).collect();
        let right_view: Vec<Line> =
            right.into_iter().skip(start).take(take).collect();
        f.render_widget(Paragraph::new(left_view), before_rect);
        f.render_widget(Paragraph::new(right_view), after_rect);

        let footer = format!(
            "  before (left) ─ after (right) · scroll {start}/{total} · ↑↓ PgUp PgDn Home End ",
        );
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                footer,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// Render the Ctrl+B V credits panel. Version + author come from
    /// `CARGO_PKG_*` env vars set by cargo at compile time; the component
    /// list is a hand-curated static (kept here so it stays in sync with
    /// what Cargo.toml actually depends on — automating from Cargo.lock
    /// would dump 200+ transitive crates that no user wants to read).
    pub(super) fn draw_credits_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let engine_summary = crate::typst_compile::engine_summary(&self.cfg);
        let lines = build_credits_lines(&self.theme, &engine_summary);
        let total = lines.len();

        // Pull scroll + logo out of the modal up front. Logo is
        // taken via `&mut` so the StatefulImage widget can update
        // its protocol state during render.
        let Modal::Credits { scroll, logo } = &mut self.modal else {
            return;
        };
        let scroll_value = *scroll;
        let logo_present = logo.is_some();

        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = format!(
            " Inkhaven v{} · author / credits ",
            env!("CARGO_PKG_VERSION")
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        // Layout: optional logo banner (top), scrollable text body
        // (middle), one-row hint (bottom). When the logo is
        // present, give it the smaller of 1/3 of the inner height
        // or 12 rows — enough for the image to read without
        // crowding out the text.
        let footer_h: u16 = 1;
        let logo_h: u16 = if logo_present {
            (inner.height / 3).min(12).max(4).min(inner.height.saturating_sub(footer_h + 4))
        } else {
            0
        };
        let body_h_rows = inner.height.saturating_sub(logo_h + footer_h);

        let logo_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: logo_h,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y + logo_h,
            width: inner.width,
            height: body_h_rows,
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + logo_h + body_h_rows,
            width: inner.width,
            height: footer_h,
        };

        if let Some(proto) = logo.as_mut() {
            if logo_h > 0 {
                let widget = ratatui_image::StatefulImage::new();
                f.render_stateful_widget(widget, logo_rect, proto);
            }
        }

        let body_h = body_rect.height as usize;
        let max_scroll = total.saturating_sub(body_h);
        let scroll_value = scroll_value.min(max_scroll);
        let end = (scroll_value + body_h).min(total);
        let visible: Vec<Line<'_>> = lines[scroll_value..end].to_vec();
        f.render_widget(Paragraph::new(visible), body_rect);

        let at_end = end >= total;
        let more_hint = if at_end { " " } else { " · more below" };
        let hint = format!(
            " ↑↓ / PgUp/PgDn / Home/End scroll · Esc close{more_hint}    (showing {}–{} of {total}) ",
            scroll_value + 1,
            end
        );
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    pub(super) fn draw_quickref_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
        focus: Focus,
        scroll: usize,
    ) {
        let entries = quickref::entries_for(focus);
        let total = entries.len();

        // Roomy panel — most of the screen with a margin.
        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect {
            x,
            y,
            width,
            height,
        };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = format!(" Quick reference · {} pane ", focus.label());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let body_h = inner.height.saturating_sub(1) as usize;
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };

        // Two columns. Each column gets half the inner width (with a small
        // gap). Entries fill column 1 top-to-bottom, then column 2.
        let col_w = (inner.width / 2) as usize;
        let visible_per_col = body_h;
        let visible_count = (visible_per_col * 2).min(total.saturating_sub(scroll));

        let left_count = visible_count.min(visible_per_col);
        let right_count = visible_count.saturating_sub(left_count);

        let mut left_lines: Vec<Line> = Vec::with_capacity(left_count);
        let mut right_lines: Vec<Line> = Vec::with_capacity(right_count);

        for i in 0..left_count {
            let e = &entries[scroll + i];
            left_lines.push(format_entry_line(e, col_w));
        }
        for i in 0..right_count {
            let e = &entries[scroll + left_count + i];
            right_lines.push(format_entry_line(e, col_w));
        }

        let left_rect = Rect {
            x: body_rect.x,
            y: body_rect.y,
            width: (body_rect.width / 2),
            height: body_rect.height,
        };
        let right_rect = Rect {
            x: body_rect.x + (body_rect.width / 2),
            y: body_rect.y,
            width: body_rect.width - (body_rect.width / 2),
            height: body_rect.height,
        };
        f.render_widget(Paragraph::new(left_lines), left_rect);
        f.render_widget(Paragraph::new(right_lines), right_rect);

        let at_end = scroll + visible_count >= total;
        let more = if at_end { " " } else { " · more below" };
        let hint = format!(
            " ↑↓ / PgUp/PgDn / Home/End scroll · Esc close{more}    (showing {}–{} of {total}) ",
            scroll + 1,
            scroll + visible_count
        );
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    pub(super) fn draw_file_picker_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
        picker: &FilePicker,
    ) {
        // Roomy panel — most of the screen, leaving a margin on all sides.
        let width = area.width.saturating_sub(8).max(40);
        let height = area.height.saturating_sub(4).max(10);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect {
            x,
            y,
            width,
            height,
        };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = match picker.context {
            PickerContext::EditorLoad => format!(" Load file into editor — {} ", picker.root.display()),
            PickerContext::TreeInsertOrImport => {
                format!(" Import into tree — {} ", picker.root.display())
            }
        };

        // The block reserves 2 rows (borders); a footer hint takes 1 more.
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let list_height = inner.height.saturating_sub(1) as usize;
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let list_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };

        // Scroll: keep cursor in view.
        let mut scroll = 0;
        if picker.cursor >= list_height && list_height > 0 {
            scroll = picker.cursor + 1 - list_height;
        }

        let mut lines: Vec<Line> = Vec::with_capacity(list_height);
        for (i, entry) in picker
            .entries
            .iter()
            .enumerate()
            .skip(scroll)
            .take(list_height)
        {
            let indent = "  ".repeat(entry.depth);
            let glyph = if entry.is_dir {
                if entry.expanded { "▾ 📁 " } else { "▸ 📁 " }
            } else {
                "  📄 "
            };
            let name = entry
                .path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("?")
                .to_string();
            let mut style = if entry.is_dir {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };
            if i == picker.cursor {
                style = style.add_modifier(Modifier::REVERSED);
            }
            lines.push(Line::from(Span::styled(
                format!("{indent}{glyph}{name}"),
                style,
            )));
        }

        f.render_widget(Paragraph::new(lines), list_rect);

        let hint = Line::from(Span::styled(
            " ↑↓ navigate · → expand · ← collapse/parent · Enter pick · Esc cancel ",
            Style::default().add_modifier(Modifier::DIM),
        ));
        f.render_widget(Paragraph::new(hint), footer_rect);
    }

    pub(super) fn draw_bund_pane_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::BundPane { title, lines, scroll } = &self.modal else {
            return;
        };
        // Roomy panel — same shape as the quickref modal.
        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let total = lines.len();
        let header = format!(" Bund · {} ({} lines) ", title, total);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.tree_script_fg)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1) as usize;
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };

        let visible: Vec<Line<'_>> = lines
            .iter()
            .skip(*scroll)
            .take(body_h)
            .map(|l| Line::from(l.as_str()))
            .collect();
        f.render_widget(Paragraph::new(visible), body_rect);

        let at_end = scroll + body_h >= total;
        let more = if at_end { " " } else { " · more below" };
        let shown_start = scroll + 1;
        let shown_end = (scroll + body_h).min(total);
        let hint = format!(
            " ↑↓ / PgUp/PgDn / Home/End scroll · Ctrl+C clear · Esc close{more}    ({}–{} of {total}) ",
            shown_start.min(total.max(1)),
            shown_end
        );
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    pub(super) fn draw_script_picker_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::ScriptPicker {
            scope,
            entries,
            cursor,
            scroll,
        } = &self.modal
        else {
            return;
        };
        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let scope_label = match scope {
            ScriptPickerScope::Branch => "current branch",
            ScriptPickerScope::ScriptsBook => "Scripts book",
        };
        let header = format!(" Bund · pick a script ({}) ", scope_label);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.tree_script_fg)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1) as usize;
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };

        let lines: Vec<Line<'_>> = entries
            .iter()
            .enumerate()
            .skip(*scroll)
            .take(body_h)
            .map(|(i, e)| {
                let glyph = "λ ";
                let text = format!(" {glyph}{}    {}", e.title, e.slug_path);
                let mut style = Style::default();
                if i == *cursor {
                    style = style.add_modifier(Modifier::REVERSED);
                }
                Line::from(Span::styled(text, style))
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        let total = entries.len();
        let hint = if total == 0 {
            " (empty) · A toggle scope · Esc close ".to_string()
        } else {
            format!(
                " ↑↓ select · Enter run · A toggle scope · Esc close    ({}/{}) ",
                cursor + 1,
                total
            )
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    pub(super) fn draw_link_picker_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::LinkPicker { entries, cursor, scroll, .. } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = format!(" Linked paragraphs ({}) ", entries.len());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1) as usize;
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };

        let lines: Vec<Line<'_>> = entries
            .iter()
            .enumerate()
            .skip(*scroll)
            .take(body_h)
            .map(|(i, e)| {
                let head = format!(" → {}", e.title);
                let path_dim = format!("    {}", e.slug_path);
                let mut spans: Vec<Span> = vec![
                    Span::raw(head),
                    Span::styled(
                        path_dim,
                        Style::default().add_modifier(Modifier::DIM),
                    ),
                ];
                let mut line = Line::from(std::mem::take(&mut spans));
                if i == *cursor {
                    line = line.style(Style::default().add_modifier(Modifier::REVERSED));
                }
                line
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        let hint = if entries.is_empty() {
            " (empty) · Esc close ".to_string()
        } else {
            format!(
                " ↑↓ select · Enter opens · D removes · Esc closes    ({}/{}) ",
                cursor + 1,
                entries.len()
            )
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    pub(super) fn draw_fuzzy_paragraph_picker_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::FuzzyParagraphPicker { input, entries, cursor, scroll } = &self.modal
        else {
            return;
        };
        let matches = fuzzy_filter_entries(entries, input.as_str());

        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(14);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = format!(
            " Find paragraph ({}/{}) ",
            matches.len(),
            entries.len()
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        // Top input row, body list, footer hint.
        let input_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: inner.height.saturating_sub(2),
        };

        f.render_widget(
            Paragraph::new(Line::from(format!(
                " › {}",
                input.render_with_cursor('│')
            ))),
            input_rect,
        );

        let body_h = body_rect.height as usize;
        let lines: Vec<Line<'_>> = matches
            .iter()
            .enumerate()
            .skip(*scroll)
            .take(body_h)
            .map(|(i, idx)| {
                let e = &entries[*idx];
                let head = format!(" {}", e.title);
                let path = format!("    {}", e.slug_path);
                let spans: Vec<Span> = vec![
                    Span::raw(head),
                    Span::styled(
                        path,
                        Style::default().add_modifier(Modifier::DIM),
                    ),
                ];
                let mut line = Line::from(spans);
                if i == *cursor {
                    line = line.style(Style::default().add_modifier(Modifier::REVERSED));
                }
                line
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        let hint = " ↑↓ select · Enter opens · Esc closes ";
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// Ctrl+B ] / `g` / Ctrl+B } — floating tag-picker pane.
    /// Each row shows `[ ] tag-name` or `[x] tag-name` (Search
    /// mode hides the brackets — selection has no meaning).
    pub(super) fn draw_tag_picker_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::TagPicker {
            target,
            all_tags,
            cursor,
            selected,
        } = &self.modal
        else {
            return;
        };
        let in_search = matches!(target, TagPickerTarget::Search);
        let total = all_tags.len();

        let width = area.width.saturating_sub(8).max(50);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = match target {
            TagPickerTarget::EditorParagraph { title, .. } => {
                format!(" Tags · `{title}` · {total} project tag(s) ")
            }
            TagPickerTarget::TreeSelection(ids) => {
                format!(" Tags · {} paragraph(s) selected · {total} project tag(s) ", ids.len())
            }
            TagPickerTarget::Search => {
                format!(" Tags · search · {total} project tag(s) ")
            }
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1) as usize;
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };

        let visible_scroll = if *cursor >= body_h {
            cursor - body_h + 1
        } else {
            0
        };
        let lines: Vec<Line<'_>> = if all_tags.is_empty() {
            vec![Line::from(Span::styled(
                "  (no tags yet — press A to add the first one)".to_string(),
                Style::default().add_modifier(Modifier::DIM),
            ))]
        } else {
            all_tags
                .iter()
                .enumerate()
                .skip(visible_scroll)
                .take(body_h)
                .map(|(i, tag)| {
                    let marker = if in_search {
                        "  ".to_string()
                    } else if selected.contains(tag) {
                        " [x] ".to_string()
                    } else {
                        " [ ] ".to_string()
                    };
                    let line = Line::from(vec![
                        Span::raw(marker),
                        Span::raw(tag.clone()),
                    ]);
                    if i == *cursor {
                        line.style(Style::default().add_modifier(Modifier::REVERSED))
                    } else {
                        line
                    }
                })
                .collect()
        };
        f.render_widget(Paragraph::new(lines), body_rect);

        let hint = if in_search {
            " ↑↓ select · Enter opens results · A adds · D deletes · Esc closes "
        } else {
            " ↑↓ select · Space marks · T applies · A adds · R renames · D deletes · Esc closes "
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint.to_string(),
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// Enter from `TagPicker` in Search mode → list of paragraphs
    /// tagged with the chosen tag, with a typeable filter input.
    pub(super) fn draw_tag_search_results_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::TagSearchResults {
            tag,
            filter,
            all_results,
            cursor,
        } = &self.modal
        else {
            return;
        };
        let matches = filter_tag_results(all_results, filter.as_str());

        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(14);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = format!(
            " Tag `{tag}` · {} match{} of {} ",
            matches.len(),
            if matches.len() == 1 { "" } else { "es" },
            all_results.len()
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let input_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: inner.height.saturating_sub(2),
        };

        f.render_widget(
            Paragraph::new(Line::from(format!(
                " › Filter: {}",
                filter.render_with_cursor('│')
            ))),
            input_rect,
        );

        let body_h = body_rect.height as usize;
        let visible_scroll = if *cursor >= body_h {
            cursor - body_h + 1
        } else {
            0
        };
        let lines: Vec<Line<'_>> = matches
            .iter()
            .enumerate()
            .skip(visible_scroll)
            .take(body_h)
            .map(|(i, e)| {
                let spans = vec![
                    Span::raw(format!(" {}", e.title)),
                    Span::styled(
                        format!("    {}", e.slug_path),
                        Style::default().add_modifier(Modifier::DIM),
                    ),
                ];
                let line = Line::from(spans);
                if i == *cursor {
                    line.style(Style::default().add_modifier(Modifier::REVERSED))
                } else {
                    line
                }
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " ↑↓ select · Enter opens · type to filter · Esc closes ",
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    pub(super) fn draw_bookmark_picker_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::BookmarkPicker { entries, cursor, scroll } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = format!(" Bookmarks ({}) ", entries.len());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1) as usize;
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };

        let lines: Vec<Line<'_>> = entries
            .iter()
            .enumerate()
            .skip(*scroll)
            .take(body_h)
            .map(|(i, e)| {
                let head = format!(" ★ {}", e.title);
                let path_dim = format!("    {}", e.slug_path);
                let spans: Vec<Span> = vec![
                    Span::raw(head),
                    Span::styled(
                        path_dim,
                        Style::default().add_modifier(Modifier::DIM),
                    ),
                ];
                let mut line = Line::from(spans);
                if i == *cursor {
                    line = line.style(Style::default().add_modifier(Modifier::REVERSED));
                }
                line
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        let hint = if entries.is_empty() {
            " (empty) · Esc close ".to_string()
        } else {
            format!(
                " ↑↓ select · Enter opens · D removes bookmark · Esc closes    ({}/{}) ",
                cursor + 1,
                entries.len()
            )
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    pub(super) fn draw_backlink_picker_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::BacklinkPicker { entries, cursor, scroll, .. } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = format!(" Backlinks ({}) ", entries.len());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1) as usize;
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };

        let lines: Vec<Line<'_>> = entries
            .iter()
            .enumerate()
            .skip(*scroll)
            .take(body_h)
            .map(|(i, e)| {
                // "←" arrow signals incoming direction (vs the
                // "→" used by the outgoing-links picker).
                let head = format!(" ← {}", e.title);
                let path_dim = format!("    {}", e.slug_path);
                let spans: Vec<Span> = vec![
                    Span::raw(head),
                    Span::styled(
                        path_dim,
                        Style::default().add_modifier(Modifier::DIM),
                    ),
                ];
                let mut line = Line::from(spans);
                if i == *cursor {
                    line = line.style(Style::default().add_modifier(Modifier::REVERSED));
                }
                line
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        let hint = if entries.is_empty() {
            " (empty) · Esc close ".to_string()
        } else {
            format!(
                " ↑↓ select · Enter opens · D removes source link · Esc closes    ({}/{}) ",
                cursor + 1,
                entries.len()
            )
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    pub(super) fn draw_similar_picker_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::SimilarPicker { entries, cursor, scroll } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = format!(" Similar paragraphs ({} hits) ", entries.len());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1) as usize;
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };

        let lines: Vec<Line<'_>> = entries
            .iter()
            .enumerate()
            .skip(*scroll)
            .take(body_h)
            .map(|(i, e)| {
                let score_pct = (e.score * 100.0).round() as i64;
                let head = format!(" {:>3}%  {}", score_pct, e.title);
                let path_dim = format!("    {}", e.slug_path);
                let snippet_dim = if e.snippet.is_empty() {
                    String::new()
                } else {
                    format!("    {}", e.snippet)
                };
                let mut spans: Vec<Span> = vec![
                    Span::raw(head),
                    Span::raw("   "),
                    Span::styled(path_dim, Style::default().add_modifier(Modifier::DIM)),
                ];
                if !snippet_dim.is_empty() {
                    spans.push(Span::raw("  · "));
                    spans.push(Span::styled(
                        snippet_dim,
                        Style::default().add_modifier(Modifier::DIM),
                    ));
                }
                let mut line = Line::from(spans);
                if i == *cursor {
                    line = line.style(Style::default().add_modifier(Modifier::REVERSED));
                }
                line
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        let hint = if entries.is_empty() {
            " (empty) · Esc close ".to_string()
        } else {
            format!(
                " ↑↓ select · Enter open side-by-side · Esc cancel    ({}/{}) ",
                cursor + 1,
                entries.len()
            )
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    pub(super) fn draw_progress_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let scroll = match &self.modal {
            Modal::Progress { scroll } => *scroll,
            _ => return,
        };
        let snap = match self.progress_cache.as_ref() {
            Some(s) => s.clone(),
            None => {
                self.refresh_progress_cache();
                self.progress_cache.clone().unwrap_or_else(|| {
                    crate::progress::ProgressSnapshot::empty()
                })
            }
        };

        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(20);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = " Writing progress ".to_string();
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        // Two-column body: text on left (2/3), 30-day sparkline
        // + bar chart on right (1/3). Footer row.
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(60),
                Constraint::Percentage(40),
            ])
            .split(body_rect);
        let text_rect = split[0];
        let chart_rect = split[1];

        // ── Text panel ────────────────────────────────────────
        let mut lines: Vec<Line> = Vec::new();
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let dim = Style::default().add_modifier(Modifier::DIM);

        // Today + streak
        lines.push(Line::from(Span::styled(" Today", bold)));
        let today_line = match snap.project.daily_goal {
            Some(goal) => {
                let pct = if goal > 0 {
                    (snap.project.today_words.max(0) * 100 / goal).clamp(0, 999)
                } else {
                    0
                };
                format!(
                    "   words: {}/{} ({}%)",
                    snap.project.today_words, goal, pct
                )
            }
            None => format!("   words: {} (no daily goal set)", snap.project.today_words),
        };
        lines.push(Line::from(today_line));
        lines.push(Line::from(format!(
            "   streak: {}d (grace {}/{} per week)",
            snap.streak.days, snap.streak.grace_used, snap.streak.grace_per_week
        )));
        lines.push(Line::from(format!(
            "   active: {} today · {} this week",
            format_active_duration(snap.active_seconds_today),
            format_active_duration(snap.active_seconds_week),
        )));
        lines.push(Line::from(""));

        // Per-book breakdown
        lines.push(Line::from(Span::styled(" Books", bold)));
        if snap.books.is_empty() {
            lines.push(Line::from(Span::styled(
                "   (no user books)",
                dim,
            )));
        }
        for b in &snap.books {
            let header = match (b.target_words, b.required_pace, b.days_to_deadline) {
                (Some(t), Some(p), Some(dd)) => format!(
                    "   {}: {}w · target {}w · pace {}w/d · {} day(s)",
                    b.label, b.total_words, t, p, dd
                ),
                (Some(t), _, _) => {
                    format!("   {}: {}w · target {}w", b.label, b.total_words, t)
                }
                _ => format!("   {}: {}w", b.label, b.total_words),
            };
            lines.push(Line::from(header));
            lines.push(Line::from(Span::styled(
                format!("      today: {}w", b.today_words),
                dim,
            )));
        }
        lines.push(Line::from(""));

        // Status ladder
        lines.push(Line::from(Span::styled(
            " Status ladder · last 7 days",
            bold,
        )));
        if snap.status.recent.is_empty() && snap.status.goals.is_empty() {
            lines.push(Line::from(Span::styled(
                "   (no status promotions recorded yet)",
                dim,
            )));
        } else {
            // Display each goal alongside its recent count.
            let mut by_status: std::collections::HashMap<String, i64> =
                std::collections::HashMap::new();
            for (s, n) in &snap.status.recent {
                by_status.insert(s.clone(), *n);
            }
            let mut shown: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for (s, goal) in &snap.status.goals {
                let n = by_status.get(s).copied().unwrap_or(0);
                lines.push(Line::from(format!(
                    "   → {}: {}/{} this week",
                    s, n, goal
                )));
                shown.insert(s.clone());
            }
            for (s, n) in &snap.status.recent {
                if shown.contains(s) {
                    continue;
                }
                lines.push(Line::from(format!("   → {}: {}", s, n)));
            }
        }

        // Apply scroll. The renderer truncates after the visible
        // height; out-of-range scroll is clamped here so End +
        // PageDown saturate at "show the bottom".
        let total = lines.len();
        let body_h = text_rect.height as usize;
        let max_scroll = total.saturating_sub(body_h.max(1));
        let scroll = scroll.min(max_scroll);
        let visible: Vec<Line> = lines.into_iter().skip(scroll).take(body_h).collect();
        f.render_widget(Paragraph::new(visible), text_rect);

        // ── Chart column ───────────────────────────────────────
        // Top half: 30-day daily-words sparkline.
        // Bottom half: per-book progress bar chart (current %
        // of target, capped at 100 for the bar height; bars
        // can overshoot in the label).
        let chart_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(chart_rect);
        let sparkline_rect = chart_split[0];
        let bars_rect = chart_split[1];

        let data: Vec<u64> = snap
            .sparkline
            .iter()
            .map(|n| (*n).max(0) as u64)
            .collect();
        if !data.is_empty() && sparkline_rect.height > 4 {
            let sparkline = ratatui::widgets::Sparkline::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" 30d words/day "),
                )
                .data(&data)
                .style(Style::default().fg(self.theme.tree_script_fg));
            f.render_widget(sparkline, sparkline_rect);
        } else {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    " (not enough history)",
                    dim,
                )))
                .block(Block::default().borders(Borders::ALL).title(" 30d ")),
                sparkline_rect,
            );
        }

        // Per-book BarChart (1.2.4+). Each user book with a
        // target shows one bar = pct of target, capped at 100.
        // The labels are short slugs so multiple books fit in
        // the narrow chart column.
        let book_bars: Vec<(String, u64)> = snap
            .books
            .iter()
            .filter_map(|b| {
                let target = b.target_words?;
                if target <= 0 {
                    return None;
                }
                let pct = (b.total_words.max(0) * 100 / target).clamp(0, 100) as u64;
                // Slugify the label so a wide book title doesn't
                // truncate the bar.
                let label = slug::slugify(&b.label);
                Some((label, pct))
            })
            .collect();
        if !book_bars.is_empty() && bars_rect.height > 4 {
            let data: Vec<(&str, u64)> =
                book_bars.iter().map(|(s, n)| (s.as_str(), *n)).collect();
            let max_label_w = data
                .iter()
                .map(|(s, _)| s.len())
                .max()
                .unwrap_or(8)
                .max(6) as u16;
            let bar_chart = ratatui::widgets::BarChart::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" books: % of target "),
                )
                .data(&data)
                .max(100)
                .bar_width(max_label_w)
                .bar_gap(1)
                .bar_style(Style::default().fg(self.theme.tree_script_fg))
                .value_style(
                    Style::default()
                        .fg(self.theme.modal_fg)
                        .add_modifier(Modifier::BOLD),
                );
            f.render_widget(bar_chart, bars_rect);
        } else {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    " (no per-book targets set)",
                    dim,
                )))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" books "),
                ),
                bars_rect,
            );
        }

        // ── Footer ─────────────────────────────────────────────
        let hint = " ↑↓ / PgUp/PgDn scroll · r refresh · Esc close ";
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(hint, dim))),
            footer_rect,
        );
    }

    pub(super) fn draw_snapshot_diff_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let (paragraph_title, when, rows, scroll) = match &self.modal {
            Modal::SnapshotDiff {
                paragraph_title,
                when,
                rows,
                scroll,
                ..
            } => (
                paragraph_title.clone(),
                when.clone(),
                rows.clone(),
                *scroll,
            ),
            _ => return,
        };

        // Roomy modal — almost full screen so wide lines fit.
        let width = area.width.saturating_sub(4).max(80);
        let height = area.height.saturating_sub(2).max(20);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header =
            format!(" Diff · `{paragraph_title}` · snapshot {when} → current ");
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        // Footer.
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };
        // Split body into two columns: snapshot (left) | current (right).
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(body_rect);
        let left_rect = split[0];
        let right_rect = split[1];

        let body_h = left_rect.height as usize;
        let visible: Vec<&SnapshotDiffRow> =
            rows.iter().skip(scroll).take(body_h).collect();

        let mut left_lines: Vec<Line<'static>> = Vec::with_capacity(visible.len());
        let mut right_lines: Vec<Line<'static>> = Vec::with_capacity(visible.len());

        let removed_style = Style::default()
            .fg(Color::Red)
            .add_modifier(Modifier::BOLD);
        let added_style = Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD);
        let changed_style = Style::default().fg(Color::Yellow);
        let dim = Style::default().add_modifier(Modifier::DIM);

        for row in visible {
            let (l_marker, r_marker, l_style, r_style) = match row.kind {
                SnapshotDiffKind::Equal => (" ", " ", dim, dim),
                SnapshotDiffKind::Removed => ("-", " ", removed_style, dim),
                SnapshotDiffKind::Added => (" ", "+", dim, added_style),
                SnapshotDiffKind::Changed => ("~", "~", changed_style, changed_style),
            };
            let left_text = row.left.clone().unwrap_or_default();
            let right_text = row.right.clone().unwrap_or_default();
            left_lines.push(Line::from(Span::styled(
                format!("{l_marker} {left_text}"),
                l_style,
            )));
            right_lines.push(Line::from(Span::styled(
                format!("{r_marker} {right_text}"),
                r_style,
            )));
        }

        f.render_widget(Paragraph::new(left_lines), left_rect);
        f.render_widget(Paragraph::new(right_lines), right_rect);

        let hint = format!(
            " ↑↓ / PgUp/PgDn / Home/End scroll · Esc back ({}/{}) ",
            scroll + 1,
            rows.len().max(1)
        );
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    pub(super) fn draw_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        // The file picker needs a much larger panel than the fixed
        // 80-wide / 8-high box used for confirms — give it its own renderer.
        if let Modal::FilePicker(picker) = &self.modal {
            self.draw_file_picker_modal(f, area, picker);
            return;
        }
        if let Modal::QuickRef { focus, scroll } = &self.modal {
            self.draw_quickref_modal(f, area, *focus, *scroll);
            return;
        }
        if matches!(self.modal, Modal::Credits { .. }) {
            self.draw_credits_modal(f, area);
            return;
        }
        if let Modal::BookInfo { scroll } = &self.modal {
            self.draw_book_info_modal(f, area, *scroll);
            return;
        }
        if let Modal::LlmPicker { .. } = &self.modal {
            self.draw_llm_picker_modal(f, area);
            return;
        }
        if let Modal::ImagePicker { .. } = &self.modal {
            self.draw_image_picker_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::ImagePreview { .. }) {
            self.draw_image_preview_modal(f, area);
            return;
        }
        if let Modal::FunctionPicker { .. } = &self.modal {
            self.draw_function_picker_modal(f, area);
            return;
        }
        if let Modal::StatusFilter { .. } = &self.modal {
            self.draw_status_filter_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::BundPane { .. }) {
            self.draw_bund_pane_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::ScriptPicker { .. }) {
            self.draw_script_picker_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::SimilarPicker { .. }) {
            self.draw_similar_picker_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::Progress { .. }) {
            self.draw_progress_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::SnapshotDiff { .. }) {
            self.draw_snapshot_diff_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::LinkPicker { .. }) {
            self.draw_link_picker_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::BacklinkPicker { .. }) {
            self.draw_backlink_picker_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::BookmarkPicker { .. }) {
            self.draw_bookmark_picker_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::FuzzyParagraphPicker { .. }) {
            self.draw_fuzzy_paragraph_picker_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::RenderedPreview { .. }) {
            self.draw_rendered_preview_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::SaveRenderedPng { .. }) {
            self.draw_save_rendered_png_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::StoryView { .. }) {
            self.draw_story_view_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::DiagnosticsList { .. }) {
            self.draw_diagnostics_list_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::AiDiffReview { .. }) {
            self.draw_ai_diff_review_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::EventPicker { .. }) {
            self.draw_event_picker_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::TimelineView { .. }) {
            self.draw_timeline_view_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::SaveStoryPng { .. }) {
            self.draw_save_story_png_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::TagPicker { .. }) {
            self.draw_tag_picker_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::TagSearchResults { .. }) {
            self.draw_tag_search_results_modal(f, area);
            return;
        }

        let width = area.width.saturating_sub(8).clamp(30, 80);
        let height: u16 = 8;
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let (title, border_color, body): (String, Color, Vec<Line<'_>>) = match &self.modal {
            Modal::None => return,
            Modal::FilePicker(_) => unreachable!("file picker handled above"),
            Modal::QuickRef { .. } => unreachable!("quickref handled above"),
            Modal::Credits { .. } => unreachable!("credits handled above"),
            Modal::BookInfo { .. } => unreachable!("book info handled above"),
            Modal::LlmPicker { .. } => unreachable!("llm picker handled above"),
            Modal::ImagePicker { .. } => unreachable!("image picker handled above"),
            Modal::ImagePreview { .. } => unreachable!("image preview handled above"),
            Modal::FunctionPicker { .. } => unreachable!("function picker handled above"),
            Modal::StatusFilter { .. } => unreachable!("status filter handled above"),
            Modal::BundPane { .. } => unreachable!("bund pane handled above"),
            Modal::ScriptPicker { .. } => unreachable!("script picker handled above"),
            Modal::SimilarPicker { .. } => unreachable!("similar picker handled above"),
            Modal::Progress { .. } => unreachable!("progress modal handled above"),
            Modal::SnapshotDiff { .. } => unreachable!("snapshot diff handled above"),
            Modal::LinkPicker { .. } => unreachable!("link picker handled above"),
            Modal::BacklinkPicker { .. } => unreachable!("backlink picker handled above"),
            Modal::BookmarkPicker { .. } => unreachable!("bookmark picker handled above"),
            Modal::FuzzyParagraphPicker { .. } =>
                unreachable!("fuzzy paragraph picker handled above"),
            Modal::RenderedPreview { .. } =>
                unreachable!("rendered preview handled above"),
            Modal::SaveRenderedPng { .. } =>
                unreachable!("save rendered png handled above"),
            Modal::TagPicker { .. } =>
                unreachable!("tag picker handled above"),
            Modal::TagSearchResults { .. } =>
                unreachable!("tag search results handled above"),
            Modal::StoryView { .. } =>
                unreachable!("story view handled above"),
            Modal::DiagnosticsList { .. } =>
                unreachable!("diagnostics list handled above"),
            Modal::AiDiffReview { .. } =>
                unreachable!("ai diff review handled above"),
            Modal::EventPicker { .. } =>
                unreachable!("event picker handled above"),
            Modal::TimelineView { .. } =>
                unreachable!("timeline view handled above"),
            Modal::TimelineNewEventPrompt {
                input,
                cursor_ticks,
                track,
                ..
            } => {
                let calendar = crate::timeline::Calendar::from_config(
                    self.cfg.timeline.calendar.clone(),
                );
                let formatted = calendar.format(
                    crate::timeline::TimelinePoint::from_ticks(*cursor_ticks),
                    crate::timeline::Precision::Day,
                );
                let track_str = track.as_deref().unwrap_or("(default)");
                let body_lines = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!(" New event @ {formatted} · track: {track_str}"),
                        Style::default()
                            .fg(self.theme.tree_chapter_fg)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!(" › {}", input.render_with_cursor('│'))),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Enter commits · Esc cancels",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (
                    " New event — n ".to_string(),
                    self.theme.tree_chapter_fg,
                    body_lines,
                )
            }
            Modal::TimelineEditEventPrompt { input, .. } => {
                let body_lines = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        " Edit event — format: start | end | track",
                        Style::default()
                            .fg(self.theme.tree_chapter_fg)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!(" › {}", input.render_with_cursor('│'))),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Empty middle = no end · empty trailing = drop track · Enter commits · Esc cancels",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (
                    " Edit event — Ctrl+V Shift+I ".to_string(),
                    self.theme.tree_chapter_fg,
                    body_lines,
                )
            }
            Modal::SnapshotAnnotation { input, parent_title, .. } => {
                let body_lines = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!(" Snapshot `{parent_title}` — annotation:"),
                        Style::default()
                            .fg(self.theme.tree_script_fg)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!(" › {}", input.render_with_cursor('│'))),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Enter commits (empty = no note) · Esc cancels",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (
                    " Snapshot annotation — F5 ".to_string(),
                    self.theme.tree_script_fg,
                    body_lines,
                )
            }
            Modal::SaveStoryPng { .. } =>
                unreachable!("save story png handled above"),
            Modal::TagAddPrompt { input, .. } => {
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        " New tag name:",
                        Style::default()
                            .fg(self.theme.tree_script_fg)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!(" › {}", input.render_with_cursor('│'))),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Enter adds + auto-selects · Esc cancels",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (
                    " Add tag — Ctrl+B ] then A ".to_string(),
                    self.theme.tree_script_fg,
                    body,
                )
            }
            Modal::TagDeleteConfirm { tag, affected, .. } => {
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!(" Delete tag `{tag}` project-wide?"),
                        Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(Span::styled(
                        format!("   Will be removed from {affected} paragraph(s)."),
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  y / Enter confirm · n / Esc cancel",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (
                    " Delete tag — y / n ".to_string(),
                    Color::Red,
                    body,
                )
            }
            Modal::TagRenamePrompt {
                input,
                old_tag,
                affected,
                ..
            } => {
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!(" Rename tag `{old_tag}` ({affected} paragraph(s)):"),
                        Style::default()
                            .fg(self.theme.tree_script_fg)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!(" › {}", input.render_with_cursor('│'))),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Enter commits (merges if name exists) · Esc cancels",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (
                    " Rename tag — R ".to_string(),
                    self.theme.tree_script_fg,
                    body,
                )
            }
            Modal::ParagraphTarget { input } => {
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        " Paragraph word-count target:",
                        Style::default()
                            .fg(self.theme.tree_script_fg)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!(" › {}", input.render_with_cursor('│'))),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Enter sets · empty/0 clears · Esc cancels",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (
                    " Per-paragraph goal — Ctrl+V T ".to_string(),
                    self.theme.tree_script_fg,
                    body,
                )
            }
            Modal::SaveMarkdown { input, label, .. } => {
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!(" Save markdown of `{label}` to:"),
                        Style::default()
                            .fg(self.theme.tree_script_fg)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!(" › {}", input.render_with_cursor('│'))),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Enter writes (default path pre-filled) · Esc cancels",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (
                    " Save markdown — Ctrl+V ".to_string(),
                    self.theme.tree_script_fg,
                    body,
                )
            }
            Modal::HelpQuery { input } => {
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        " Ask the Help book:",
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!(" › {}", input.render_with_cursor('│'))),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Enter to ask · Esc to cancel · answer streams into the AI pane",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (" Help — F1 ".to_string(), Color::Cyan, body)
            }
            Modal::BundInput { prompt, input, hook } => {
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!(" {prompt}"),
                        Style::default()
                            .fg(self.theme.tree_script_fg)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!(" › {}", input.render_with_cursor('│'))),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!(
                            "  Enter fires hook `{hook}` with your input · Esc cancels"
                        ),
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (
                    " Bund — ink.input ".to_string(),
                    self.theme.tree_script_fg,
                    body,
                )
            }
            Modal::BundEval { input } => {
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        " Bund — evaluate one expression against Adam:",
                        Style::default()
                            .fg(self.theme.tree_script_fg)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!(" › {}", input.render_with_cursor('│'))),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Enter runs · Esc cancels · result lands on the status bar",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (
                    " Bund — Ctrl+Z E ".to_string(),
                    self.theme.tree_script_fg,
                    body,
                )
            }
            Modal::ChatSearchPrompt { input } => {
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        " Search chat history:",
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!(" › {}", input.render_with_cursor('│'))),
                    Line::from(""),
                    Line::from(Span::styled(
                        "  Enter starts from the newest match · Ctrl+X advances to older · Esc cancels",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (" Chat search — Ctrl+F ".to_string(), Color::Cyan, body)
            }
            Modal::FindReplace {
                search_input,
                replace_input,
                focus_replace,
            } => {
                let cursor_char = '│';
                let search_marker = if *focus_replace { " " } else { ">" };
                let replace_marker = if *focus_replace { ">" } else { " " };
                let mut body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!(
                            " {} Search:  {}",
                            search_marker,
                            search_input.render_with_cursor(cursor_char)
                        ),
                        if !*focus_replace {
                            Style::default()
                        } else {
                            Style::default().add_modifier(Modifier::DIM)
                        },
                    )),
                ];
                if let Some(r) = replace_input {
                    body.push(Line::from(Span::styled(
                        format!(
                            " {} Replace: {}",
                            replace_marker,
                            r.render_with_cursor(cursor_char)
                        ),
                        if *focus_replace {
                            Style::default()
                        } else {
                            Style::default().add_modifier(Modifier::DIM)
                        },
                    )));
                }
                body.push(Line::from(""));
                let hint = if replace_input.is_some() {
                    " Enter run · Tab switch field · Esc cancel "
                } else {
                    " Enter find · Esc cancel "
                };
                body.push(Line::from(Span::styled(
                    hint,
                    Style::default().add_modifier(Modifier::DIM),
                )));
                let header = if replace_input.is_some() {
                    " Find & Replace (regex) "
                } else {
                    " Find (regex) "
                };
                (header.into(), Color::Magenta, body)
            }
            Modal::Adding {
                kind,
                parent_label,
                input,
                position,
                ..
            } => {
                let header = match position {
                    InsertPosition::End => format!(" Add {} ", kind.as_str()),
                    InsertPosition::After(_) => {
                        format!(" Insert {} after current ", kind.as_str())
                    }
                    InsertPosition::Before(_) => {
                        format!(" Insert {} before anchor ", kind.as_str())
                    }
                };
                let parent = format!(" Parent: {}", parent_label);
                let where_line = match position {
                    InsertPosition::End => "    Where: append at end".to_string(),
                    InsertPosition::After(anchor_id) => {
                        let anchor_name = self
                            .hierarchy
                            .get(*anchor_id)
                            .map(|n| n.title.clone())
                            .unwrap_or_else(|| "<gone>".to_string());
                        format!("    Where: after `{anchor_name}`")
                    }
                    InsertPosition::Before(anchor_id) => {
                        let anchor_name = self
                            .hierarchy
                            .get(*anchor_id)
                            .map(|n| n.title.clone())
                            .unwrap_or_else(|| "<gone>".to_string());
                        format!("    Where: before `{anchor_name}`")
                    }
                };
                let title_line = format!(" Title : {}", input.render_with_cursor('│'));
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(parent, Style::default().add_modifier(Modifier::DIM))),
                    Line::from(Span::styled(
                        where_line,
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                    Line::from(title_line),
                    Line::from(Span::styled(
                        " Enter to confirm · Esc to cancel ",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (header, Color::Green, body)
            }
            Modal::Deleting {
                root_kind,
                title,
                descendant_count,
                ..
            } => {
                let prompt = if *descendant_count > 0 {
                    format!(
                        " Delete {} `{}` and {} descendant{}?",
                        root_kind.as_str(),
                        title,
                        descendant_count,
                        if *descendant_count == 1 { "" } else { "s" }
                    )
                } else {
                    format!(" Delete {} `{}`?", root_kind.as_str(), title)
                };
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        prompt,
                        Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        " Removes files from disk AND records from the store.",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                    Line::from(Span::styled(
                        " y / Enter to confirm · n / Esc to cancel ",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (" Confirm delete ".into(), Color::Red, body)
            }
            Modal::Renaming { kind, input, .. } => {
                let header = format!(" Rename {} ", kind.as_str());
                let title_line = format!(" Title : {}", input.render_with_cursor('│'));
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("    Renaming a {} — its slug and filesystem entry don't change.", kind.as_str()),
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                    Line::from(""),
                    Line::from(title_line),
                    Line::from(""),
                    Line::from(Span::styled(
                        " Enter to confirm · Esc to cancel ",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (header, Color::Blue, body)
            }
            Modal::SnapshotPicker {
                paragraph_title,
                snapshots,
                cursor,
                ..
            } => {
                let header = format!(" Snapshots — {} ", paragraph_title);
                let mut body: Vec<Line> = Vec::with_capacity(snapshots.len() + 2);
                body.push(Line::from(""));
                for (i, snap) in snapshots.iter().enumerate() {
                    let selected = i == *cursor;
                    let ts = snap
                        .created_at
                        .with_timezone(&chrono::Local)
                        .format("%Y-%m-%d %H:%M:%S %z");
                    let preview = if snap.preview.is_empty() {
                        "(no body yet)"
                    } else {
                        snap.preview.as_str()
                    };
                    let head = format!(
                        " {ts}   {}w   {}",
                        snap.word_count, preview,
                    );
                    let style = if selected {
                        Style::default()
                            .add_modifier(Modifier::REVERSED | Modifier::BOLD)
                            .fg(Color::Cyan)
                    } else {
                        Style::default()
                    };
                    body.push(Line::from(Span::styled(head, style)));
                    // 1.2.6+ — render the user's annotation on a
                    // second indented line when present. Italics +
                    // cyan keeps it visually distinct from the
                    // preview while staying readable.
                    if !snap.annotation.trim().is_empty() {
                        let annot_style = if selected {
                            Style::default()
                                .add_modifier(Modifier::REVERSED | Modifier::ITALIC)
                                .fg(Color::Cyan)
                        } else {
                            Style::default()
                                .add_modifier(Modifier::ITALIC)
                                .fg(Color::Cyan)
                        };
                        body.push(Line::from(Span::styled(
                            format!("       ✎ {}", snap.annotation),
                            annot_style,
                        )));
                    }
                }
                body.push(Line::from(""));
                body.push(Line::from(Span::styled(
                    " ↑↓ navigate · Enter loads · V diff vs current · D / Del delete · Esc cancel ",
                    Style::default().add_modifier(Modifier::DIM),
                )));
                (header, Color::Cyan, body)
            }
        };

        // Generic confirms (Adding / Deleting / FindReplace / Renaming /
        // SnapshotPicker) all share this final render. Theme drives the
        // modal background + foreground; per-modal accent colour is the
        // border (passed in via `border_color`).
        f.render_widget(
            Paragraph::new(body).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
                    .style(
                        Style::default()
                            .bg(self.theme.modal_bg)
                            .fg(self.theme.modal_fg),
                    ),
            ),
            rect,
        );
    }

    /// Render the secondary editor pane (right side, replaces AI
    /// when in similar-paragraph mode). Simpler than draw_editor —
    /// no syntax highlighting, no find/replace overlay, no split
    /// view — but supports a moving cursor so the user can edit.
    /// Focus highlight comes from `self.secondary_focused`, which
    /// is independent of `self.focus` (keystrokes get routed to
    /// secondary by the swap-on-dispatch wrapper in
    /// `handle_editor_key`).
    pub(super) fn draw_secondary_editor(&mut self, f: &mut ratatui::Frame, area: Rect) {
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
    pub(super) fn draw_primary_pane_footer(&self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(super) fn draw_search_bar(&self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(super) fn draw_ai_prompt(&self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(super) fn draw_tree(&self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(super) fn draw_editor(&mut self, f: &mut ratatui::Frame, area: Rect) {
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
    pub(super) fn draw_split_snapshot(&self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(super) fn draw_editor_unwrapped(&mut self, f: &mut ratatui::Frame, inner: Rect) {
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
        let lex_per_row: Vec<Vec<super::super::lexicon::LexHit>> = current_lines
            .iter()
            .map(|line| {
                if lexicon.is_empty() {
                    Vec::new()
                } else {
                    lexicon.row_hits(line)
                }
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

    pub(super) fn draw_editor_wrapped(&mut self, f: &mut ratatui::Frame, inner: Rect) {
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

        let lex_per_row: Vec<Vec<super::super::lexicon::LexHit>> = current_lines
            .iter()
            .map(|line| {
                if lexicon.is_empty() {
                    Vec::new()
                } else {
                    lexicon.row_hits(line)
                }
            })
            .collect();

        let (cur_row, cur_col) = opened.textarea.cursor();
        let selection = opened.textarea.selection_range();

        let total_lines = highlighted.len().max(1);
        let lineno_chars = digit_count(total_lines);
        let gutter_width = (lineno_chars + 1) as u16;

        let h = inner.height as usize;
        let w = inner.width.saturating_sub(gutter_width) as usize;

        let mut visual: Vec<super::super::highlight::VisualRow> = Vec::new();
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
            let mut text_spans = build_visual_row_spans(
                v,
                selection,
                block,
                added_flags,
                row_hits,
                lex_hits,
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

    pub(super) fn draw_ai(&self, f: &mut ratatui::Frame, area: Rect) {
        // Title carries the inference state plus mode chips so the user
        // can see at a glance:
        //   - provider + streaming/done/error status
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
        if let Some(inf) = &self.inference {
            let status_text = match &inf.status {
                InferenceStatus::Streaming => format!(" — {} · streaming…", inf.provider),
                InferenceStatus::Done => format!(" — {} · done", inf.provider),
                InferenceStatus::Error(_) => format!(" — {} · error", inf.provider),
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
                        let lines = super::super::markdown::render(&inf.response);
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
    pub(super) fn draw_chat_history(&self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(super) fn draw_prompt_picker(&self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(super) fn draw_status(&self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(super) fn draw_search_overlay(&self, f: &mut ratatui::Frame, area: Rect) {
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
