//! Modal-overlay painters owned by `App` — every `draw_*_modal`
//! method that paints a centered/floating overlay over the
//! editor. Sub-module of `tui::app::render`. Extracted from
//! `tui::app::render` in the 1.2.7 refactor, Phase 4 batch 1.
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use super::super::{
    filter_functions, filter_tag_results,
    format_entry_line, fuzzy_filter_entries,
};


use super::super::super::credits::build_credits_lines;
use super::super::super::diff_utils::{SnapshotDiffKind, SnapshotDiffRow};
use super::super::super::file_picker::{FilePicker, PickerContext};
use super::super::super::focus::Focus;
use super::super::super::modal::{
    Modal, ScriptPickerScope, TagPickerTarget, visible_event_entries,
};
use super::super::super::quickref;
use super::super::super::text_utils::{
    format_active_duration, truncate_label,
};
use super::super::super::timeline_state::TimelineEvent;


impl super::super::App {

    pub(in crate::tui::app) fn draw_book_info_modal(
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

    pub(in crate::tui::app) fn draw_llm_picker_modal(&self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(in crate::tui::app) fn draw_image_picker_modal(&self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(in crate::tui::app) fn draw_function_picker_modal(&self, f: &mut ratatui::Frame, area: Rect) {
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
                    super::super::super::typst_funcs::all().len()
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

    pub(in crate::tui::app) fn draw_image_preview_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
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
    pub(in crate::tui::app) fn draw_rendered_preview_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
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
    pub(in crate::tui::app) fn draw_save_rendered_png_modal(
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
    pub(in crate::tui::app) fn draw_story_view_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
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
    pub(in crate::tui::app) fn draw_save_story_png_modal(&self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(in crate::tui::app) fn draw_status_filter_modal(&self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(in crate::tui::app) fn draw_timeline_view_modal(
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

    pub(in crate::tui::app) fn draw_event_picker_modal(
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

    pub(in crate::tui::app) fn draw_diagnostics_list_modal(
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
    pub(in crate::tui::app) fn draw_ai_diff_review_modal(
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
    pub(in crate::tui::app) fn draw_credits_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(in crate::tui::app) fn draw_quickref_modal(
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

    pub(in crate::tui::app) fn draw_file_picker_modal(
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

    pub(in crate::tui::app) fn draw_bund_pane_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(in crate::tui::app) fn draw_script_picker_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(in crate::tui::app) fn draw_link_picker_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(in crate::tui::app) fn draw_fuzzy_paragraph_picker_modal(
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
    pub(in crate::tui::app) fn draw_tag_picker_modal(
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
    pub(in crate::tui::app) fn draw_tag_search_results_modal(
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

    pub(in crate::tui::app) fn draw_bookmark_picker_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
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

    /// 1.2.8+ — embedded shell pane.  Renders the turn
    /// buffer as alternating prompt+output blocks; input
    /// line pinned to the bottom.  In selection mode the
    /// cursor-highlighted turn gets reversed styling so the
    /// user knows which output `c` / `i` will act on.
    pub(in crate::tui::app) fn draw_shell_pane_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::ShellPane {
            input,
            selection_mode,
            selection_cursor,
            scroll,
            ..
        } = &self.modal
        else {
            return;
        };
        let scroll = *scroll;

        // Fullscreen-floating: leave a 1-cell margin so the
        // editor pane's borders are still visible.
        let rect = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header_base = if *selection_mode {
            " OS Shell · selection mode"
        } else {
            " OS Shell"
        };
        let header_owned;
        let header = if scroll > 0 {
            header_owned = format!("{header_base} · ↑ scrolled (End→bottom) ");
            header_owned.as_str()
        } else {
            header_owned = format!("{header_base} ");
            header_owned.as_str()
        };
        let border_color = if *selection_mode {
            Color::Yellow
        } else {
            self.theme.modal_border
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(border_color)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        // Reserve last 2 rows for the input prompt + a
        // status hint.  Body gets the rest.
        let prompt_h: u16 = 2;
        let body_h = inner.height.saturating_sub(prompt_h);
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: body_h,
        };
        let prompt_rect = Rect {
            x: inner.x,
            y: inner.y + body_h,
            width: inner.width,
            height: prompt_h,
        };

        // Build the body lines from the turn buffer.  Each
        // turn renders as:
        //   $ <command>
        //   <stdout>
        //   [error: <stderr>]   (only when failure)
        //   <blank>
        // The newest turn anchors to the BOTTOM of body_rect
        // so the most-recent output is visible.
        let mut lines: Vec<Line<'_>> = Vec::with_capacity(
            self.shell_history.len() * 4 + 2,
        );
        // Track the starting `lines` index of each turn so
        // we can isolate the LATEST turn from older
        // scrollback at render time (see start-clamping
        // logic below).
        let mut turn_starts: Vec<usize> = Vec::with_capacity(self.shell_history.len());
        if self.shell_history.is_empty() {
            lines.push(Line::from(Span::styled(
                "(no commands yet — type a nu command and press Enter)",
                Style::default().add_modifier(Modifier::DIM),
            )));
        }
        for (i, turn) in self.shell_history.iter().enumerate() {
            turn_starts.push(lines.len());
            let is_selected_turn = *selection_mode && i == *selection_cursor;
            let prompt_style = if is_selected_turn {
                Style::default()
                    .add_modifier(Modifier::REVERSED | Modifier::BOLD)
                    .fg(Color::Cyan)
            } else {
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Cyan)
            };
            lines.push(Line::from(Span::styled(
                format!("$ {}", turn.command),
                prompt_style,
            )));
            for ln in turn.stdout.lines() {
                let s = if is_selected_turn {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(ln.to_string(), s)));
            }
            if !turn.success && !turn.stderr.is_empty() {
                for ln in turn.stderr.lines() {
                    lines.push(Line::from(Span::styled(
                        ln.to_string(),
                        Style::default().fg(Color::Red),
                    )));
                }
            }
            lines.push(Line::from(""));
        }
        // Anchor to bottom: render the last body_h lines.
        // `scroll` shifts the visible window UP by N logical
        // lines so older content comes into view.  Render
        // clamps to the valid range — if the handler advanced
        // scroll past total_lines, we silently cap at the top
        // of the buffer.  The field itself isn't rewritten;
        // PgDown will gradually bring it back into range.
        let visible_n = body_h as usize;
        let total = lines.len();
        let max_scroll = total.saturating_sub(visible_n);
        let effective_scroll = scroll.min(max_scroll);
        let end = total.saturating_sub(effective_scroll);
        let naive_start = end.saturating_sub(visible_n);
        // "Latest-turn isolation": when the user is NOT
        // scrolled (effective_scroll == 0), clamp the
        // visible-window start to the beginning of the
        // most-recent turn.  Without this clamp, after a
        // huge `help commands` (truncated to 1000 lines)
        // followed by a short `ls` (9 lines), the tail of
        // the help output would sit above the new `ls`
        // turn — visually masking it as "help still
        // showing" (the user-reported bug).  With the
        // clamp, only `ls`'s 9 lines render at the bottom
        // and the empty space above is genuinely empty.
        // PgUp brings the older content back into view
        // (scroll > 0 disables the clamp).
        let start = if effective_scroll == 0 {
            naive_start.max(turn_starts.last().copied().unwrap_or(0))
        } else {
            naive_start
        };
        let visible: Vec<Line<'_>> = lines[start..end].to_vec();
        // 1.2.8+ — anchor short content to the BOTTOM of the
        // body rect, not the top.  Without this, a fresh
        // session (one `ls` turn = ~9 lines) renders flush
        // against the top of a 60-row pane and the prompt
        // sits at the bottom with a huge empty gap in
        // between.  Terminal users expect the most-recent
        // output to be near the prompt (where the eyes
        // already are after pressing Enter), so we render
        // the visible lines in a sub-rect anchored to the
        // bottom edge of body_rect.  When visible.len() >=
        // body_h (long output, normal scrolling case),
        // sub_rect == body_rect — no behavioural change.
        //
        // `Wrap { trim: false }` is critical here.  Without
        // it, lines wider than the pane width get arbitrarily
        // truncated AND nu's table output (which sometimes
        // runs ~120 cols) clips on narrow terminals.  Wrap
        // also implicitly guards against ANSI bytes that
        // slip past `shell::strip_ansi`.
        let used_h = (visible.len() as u16).min(body_h);
        let render_rect = Rect {
            x: body_rect.x,
            y: body_rect.y + body_h.saturating_sub(used_h),
            width: body_rect.width,
            height: used_h,
        };
        f.render_widget(
            Paragraph::new(visible).wrap(Wrap { trim: false }),
            render_rect,
        );

        // Prompt + hint.
        let prompt_line_rect = Rect {
            x: prompt_rect.x,
            y: prompt_rect.y,
            width: prompt_rect.width,
            height: 1,
        };
        if *selection_mode {
            let s = format!(
                " (selection · turn {}/{})",
                selection_cursor + 1,
                self.shell_history.len().max(1)
            );
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    s,
                    Style::default().fg(Color::Yellow),
                ))),
                prompt_line_rect,
            );
        } else {
            // 1.2.8+ — syntax-highlight the live input via
            // nu-parser tokens.  Engine is lazily-init'd by
            // open_shell_pane before the modal opens, so
            // shell_engine.as_ref() is Some here; the unwrap
            // is logically infallible but we guard for
            // future refactors.
            let mut spans: Vec<Span<'_>> = vec![Span::styled(
                "$ ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )];
            let line_text = input.as_str().to_string();
            if let Some(eng) = self.shell_engine.as_ref() {
                for (chunk, style) in eng.highlight(&line_text) {
                    spans.push(Span::styled(chunk, style));
                }
            } else {
                spans.push(Span::raw(line_text));
            }
            f.render_widget(
                Paragraph::new(Line::from(spans)),
                prompt_line_rect,
            );
            // Place the terminal cursor at the end of the
            // typed text.  TextInput::cursor() returns the
            // char index; `"$ "` prefix consumes 2 cells.
            let cursor_col = 2usize + input.cursor();
            let max_col = prompt_line_rect.width.saturating_sub(1) as usize;
            let x = prompt_line_rect.x
                + cursor_col.min(max_col) as u16;
            f.set_cursor_position((x, prompt_line_rect.y));
        }
        let hint = if *selection_mode {
            " ↑↓ turn · PgUp/PgDn scroll · c copy · i insert · Ctrl+Z h exit · Esc exit "
        } else {
            " Enter run · ↑↓ cmd history · PgUp/PgDn scroll · Ctrl+Z h selection · Esc close "
        };
        let hint_rect = Rect {
            x: prompt_rect.x,
            y: prompt_rect.y + 1,
            width: prompt_rect.width,
            height: 1,
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            hint_rect,
        );
    }

    /// 1.2.8+ — kill-ring picker. Renders each deleted-
    /// paragraph stash as title + original parent breadcrumb
    /// + first-non-empty-line preview.  Cursor selection
    /// reversed-highlight; D not supported (Enter is the
    /// only mutator).
    pub(in crate::tui::app) fn draw_kill_ring_picker_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::KillRingPicker { cursor } = &self.modal else {
            return;
        };
        let len = self.kill_ring.len();
        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = format!(" Kill-ring ({}/{}) ", len, super::super::KILL_RING_CAP);
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

        // Each entry consumes TWO lines: a title row + a dim
        // breadcrumb+preview row.  Cap visible entries to
        // body_h / 2 to keep the layout stable.
        let per_entry = 2usize;
        let visible = (body_h / per_entry).max(1);
        let lines: Vec<Line<'_>> = self
            .kill_ring
            .iter()
            .enumerate()
            .take(visible)
            .flat_map(|(i, stash)| {
                let parent_label = stash
                    .parent_id
                    .and_then(|pid| self.hierarchy.get(pid))
                    .map(|p| p.title.clone())
                    .unwrap_or_else(|| "(parent gone)".into());
                let body_text = std::str::from_utf8(&stash.content).unwrap_or("");
                let first_line = body_text
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("(empty)");
                let preview_budget = inner.width.saturating_sub(8) as usize;
                let preview = if first_line.chars().count() > preview_budget {
                    let mut s: String = first_line
                        .chars()
                        .take(preview_budget.saturating_sub(1))
                        .collect();
                    s.push('…');
                    s
                } else {
                    first_line.to_string()
                };
                let head_text = format!(" ⌫ {}", stash.title);
                let dim_text = format!("    in `{}`  ·  {}", parent_label, preview);
                let mut head_line = Line::from(Span::raw(head_text));
                let mut dim_line = Line::from(Span::styled(
                    dim_text,
                    Style::default().add_modifier(Modifier::DIM),
                ));
                if i == *cursor {
                    head_line = head_line.style(
                        Style::default().add_modifier(Modifier::REVERSED),
                    );
                    dim_line = dim_line.style(
                        Style::default().add_modifier(Modifier::REVERSED),
                    );
                }
                vec![head_line, dim_line]
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        let hint = if len == 0 {
            " (empty — Ctrl+B delete pushes onto this ring) · Esc close ".to_string()
        } else {
            format!(
                " ↑↓ select · Enter restore · Esc cancel    ({}/{}) ",
                cursor + 1,
                len
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

    pub(in crate::tui::app) fn draw_backlink_picker_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(in crate::tui::app) fn draw_similar_picker_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(in crate::tui::app) fn draw_progress_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
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

    pub(in crate::tui::app) fn draw_snapshot_diff_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
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

}
