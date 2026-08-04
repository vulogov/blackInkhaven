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


/// 1.2.11+ — wrap a single diff row to `column_w`,
/// returning one `Line` per wrapped row.  The first
/// row keeps the supplied `prefix` ("- " / "+ " /
/// "  ") so the diff marker stays leftmost;
/// continuation rows are indented two columns to
/// match the prefix width, so a long sentence reads
/// as one visually continuous block.  Whitespace
/// between words is collapsed to a single space.
/// Hard-breaks words that are themselves wider than
/// the column (URLs, em-dash-heavy phrases, etc).
fn wrap_diff_row(
    text: &str,
    prefix: &str,
    column_w: usize,
    style: Style,
) -> Vec<Line<'static>> {
    let prefix_w = prefix.chars().count();
    let cont_indent = "  ";
    let cont_indent_w = cont_indent.chars().count();
    let body_w_first = column_w.saturating_sub(prefix_w).max(1);
    let body_w_cont = column_w.saturating_sub(cont_indent_w).max(1);
    let mut rows: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0usize;
    let mut first = true;
    let row_w = |first: bool| if first { body_w_first } else { body_w_cont };
    for word in text.split_whitespace() {
        let w = word.chars().count();
        if cur_w == 0 {
            if w > row_w(first) {
                let mut buf = String::new();
                let mut buf_w = 0;
                for ch in word.chars() {
                    if buf_w == row_w(first) {
                        rows.push(std::mem::take(&mut buf));
                        first = false;
                        buf_w = 0;
                    }
                    buf.push(ch);
                    buf_w += 1;
                }
                cur = buf;
                cur_w = buf_w;
            } else {
                cur.push_str(word);
                cur_w = w;
            }
        } else if cur_w + 1 + w > row_w(first) {
            rows.push(std::mem::take(&mut cur));
            first = false;
            if w > row_w(first) {
                let mut buf = String::new();
                let mut buf_w = 0;
                for ch in word.chars() {
                    if buf_w == row_w(first) {
                        rows.push(std::mem::take(&mut buf));
                        first = false;
                        buf_w = 0;
                    }
                    buf.push(ch);
                    buf_w += 1;
                }
                cur = buf;
                cur_w = buf_w;
            } else {
                cur.push_str(word);
                cur_w = w;
            }
        } else {
            cur.push(' ');
            cur.push_str(word);
            cur_w += 1 + w;
        }
    }
    if !cur.is_empty() {
        rows.push(cur);
    }
    if rows.is_empty() {
        rows.push(String::new());
    }
    rows.into_iter()
        .enumerate()
        .map(|(i, body)| {
            let display = if i == 0 {
                format!("{prefix}{body}")
            } else {
                format!("{cont_indent}{body}")
            };
            Line::from(Span::styled(display, style))
        })
        .collect()
}


/// 1.2.22 R.3 — KWIC spans for a replace hit: the matched span
/// highlighted in its line, ~30 chars of context each side, ellipsed.
fn match_spans(hit: &crate::replace::Hit) -> Vec<Span<'static>> {
    let chars: Vec<char> = hit.line_text.chars().collect();
    let start = (hit.col.saturating_sub(1)).min(chars.len());
    let end = (start + hit.matched.chars().count()).min(chars.len());
    let lead = start.saturating_sub(30);
    let trail = (end + 30).min(chars.len());
    let before: String = chars[lead..start].iter().collect();
    let matched: String = chars[start..end].iter().collect();
    let after: String = chars[end..trail].iter().collect();
    let mut out: Vec<Span<'static>> = Vec::new();
    if lead > 0 {
        out.push(Span::raw("…"));
    }
    out.push(Span::raw(before));
    out.push(Span::styled(
        matched,
        Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    ));
    out.push(Span::raw(after));
    if trail < chars.len() {
        out.push(Span::raw("…"));
    }
    out
}

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

    /// 1.3.34+ — the AI cost dashboard panel (Ctrl+B $). Computes today's tallies on
    /// render via the shared `cli::cost` aggregator.
    pub(in crate::tui::app) fn draw_cost_dashboard_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
        scroll: usize,
    ) {
        let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let report = crate::cli::cost::gather(
            self.store.project_root(),
            &day,
            self.cfg.cost.world_daily_call_cap,
            self.cfg.cost.inner_socrates_daily_call_cap,
            self.cfg.inner_editor.llm.editor_engagement.max_calls_per_day,
        );
        let lines: Vec<Line<'_>> = crate::cli::cost::render_lines(&report)
            .into_iter()
            .map(Line::from)
            .collect();
        let total = lines.len();

        let width = area.width.saturating_sub(8).max(56);
        let height = area.height.saturating_sub(4).max(10);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" AI cost · Ctrl+B $ ")
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1) as usize;
        let body_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: inner.height.saturating_sub(1) };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };

        let max_scroll = total.saturating_sub(body_h);
        let scroll = scroll.min(max_scroll);
        let end = (scroll + body_h).min(total);
        f.render_widget(Paragraph::new(lines[scroll..end].to_vec()), body_rect);

        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " ↑↓ scroll · Esc close · CLI: inkhaven cost ",
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// STRUCT-2 — the structural paragraph type picker (`i` in the Tree pane).
    /// A simple vertical list over `STRUCTURAL_TYPES`; layout mirrors the LLM
    /// picker.
    pub(in crate::tui::app) fn draw_structural_type_picker_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::StructuralTypePicker { cursor } = &self.modal else {
            return;
        };
        let types = super::super::para_type_table();
        let verse_start = super::super::STRUCTURAL_TYPES.len();
        let header_lines = 1;
        let footer_lines = 2;
        // +1 for the "Verse" section separator.
        let height = (header_lines + types.len() + 1 + footer_lines + 2) as u16;
        let height = height.clamp(8, area.height.saturating_sub(2));
        let width = 48u16.clamp(40, area.width.saturating_sub(6));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Add structural paragraph · i ")
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
        for (i, (_tag, glyph, label, _seed)) in types.iter().enumerate() {
            // POEM-1 — a "Verse" separator opens the verse family.
            if i == verse_start {
                lines.push(Line::from(Span::styled(
                    "  ── Verse ──────────────────────".to_string(),
                    Style::default().add_modifier(Modifier::DIM),
                )));
            }
            let marker = if i == *cursor { "›" } else { " " };
            let row = format!("  {marker} {glyph} {label}");
            let style = if i == *cursor {
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(row, style)));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  ↑↓ select · Enter create · Esc cancel".to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )));

        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }

    /// POEM-TUI (PO-P12) — the form picker (`Ctrl+B J → P → D`). Lists the
    /// built-in forms; Enter attaches the chosen form's `poem:` block to the
    /// open verse paragraph.
    pub(in crate::tui::app) fn draw_poem_form_picker_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::PoemFormPicker { cursor, .. } = &self.modal else {
            return;
        };
        let lib = crate::poetry::form::FormsLibrary::builtin();
        let forms = lib.all();

        let width = 60u16.clamp(44, area.width.saturating_sub(6));
        let max_rows = area.height.saturating_sub(8).max(6) as usize;
        let visible = forms.len().min(max_rows);
        let height = (visible + 6) as u16;
        let height = height.clamp(10, area.height.saturating_sub(2));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" ♪ Declare a form · D ")
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

        // Scroll window so the cursor stays visible with >max_rows forms.
        let start = if *cursor >= visible {
            (*cursor + 1).saturating_sub(visible)
        } else {
            0
        };
        let end = (start + visible).min(forms.len());

        let name_w = forms.iter().map(|f| f.form.len()).max().unwrap_or(16).min(20);
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(""));
        for (i, pf) in forms.iter().enumerate().take(end).skip(start) {
            let marker = if i == *cursor { "›" } else { " " };
            let desc_room = (width as usize).saturating_sub(name_w + 8);
            let mut desc = pf.desc.clone();
            if desc.chars().count() > desc_room {
                desc = desc.chars().take(desc_room.saturating_sub(1)).collect::<String>() + "…";
            }
            let row = format!("  {marker} {:<name_w$}  {desc}", pf.form, name_w = name_w);
            let style = if i == *cursor {
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(row, style)));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  writes a poem: block beside the stanza — no verse is generated".to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )));
        lines.push(Line::from(Span::styled(
            "  ↑↓ select · Enter attach · Esc cancel".to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )));

        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }

    /// POEM-TUI (PO-P15) — the two-column translation view. Splits a
    /// `para:verse-translation` paragraph into source ∥ translation, line-aligned,
    /// with the Form/Sound trilemma beneath (Meaning stays the Inner Poet's axis).
    /// Read-only review; never rewrites the poem.
    pub(in crate::tui::app) fn draw_verse_translation_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::VerseTranslationView { verse_id } = &self.modal else {
            return;
        };
        let verse_id = *verse_id;

        // Live buffer if this paragraph is open, else the stored body.
        let body = if self.opened.as_ref().map(|d| d.id) == Some(verse_id) {
            self.opened
                .as_ref()
                .map(|d| d.textarea.lines().join("\n"))
                .unwrap_or_default()
        } else {
            self.store
                .get_content(verse_id)
                .ok()
                .flatten()
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .unwrap_or_default()
        };

        let width = area.width.saturating_sub(6).clamp(48, area.width);
        let height = area.height.saturating_sub(4).clamp(10, area.height);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" ⇄ Translation ")
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let split = crate::poetry::translation::split_source_translation(&body);
        let Some((source, translation)) = split else {
            let hint = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  This paragraph isn't a paired translation yet.".to_string(),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  Put the source above a `---` (or `⇄`) line and the translation below it:".to_string(),
                    Style::default().add_modifier(Modifier::DIM),
                )),
                Line::from(""),
                Line::from(Span::styled("      Мой дядя самых честных правил".to_string(), Style::default())),
                Line::from(Span::styled("      ---".to_string(), Style::default().add_modifier(Modifier::DIM))),
                Line::from(Span::styled("      My uncle, a man of the purest honour".to_string(), Style::default())),
                Line::from(""),
                Line::from(Span::styled("  Esc closes.".to_string(), Style::default().add_modifier(Modifier::DIM))),
            ];
            f.render_widget(Paragraph::new(hint).wrap(Wrap { trim: false }), inner);
            return;
        };

        // Languages: translation is the project language; source is detected
        // (whatlang), falling back to the project language.
        let trans_iso = crate::ai::prompts::iso_from_long(&self.cfg.language);
        let trans_lang = crate::prose::ProseLanguage::from_label(trans_iso);
        let src_iso = whatlang::detect(&source)
            .filter(|i| i.is_reliable())
            .and_then(|i| crate::ai::prompts::iso_from_alpha3(i.lang().code()))
            .unwrap_or(trans_iso);
        let src_lang = crate::prose::ProseLanguage::from_label(src_iso);
        let form = self
            .poem_form_for(verse_id)
            .unwrap_or_else(crate::poetry::form::PoemForm::default);
        let tri = crate::poetry::translation::trilemma(
            &source, &src_lang, &translation, &trans_lang, &form,
        );

        // Layout: a two-column body over a 5-line trilemma footer.
        let footer_h = 5u16.min(inner.height.saturating_sub(2));
        let body_h = inner.height.saturating_sub(footer_h);
        let col_w = (inner.width.saturating_sub(3) / 2) as usize;

        let src_lines: Vec<&str> = source.lines().collect();
        let trans_lines: Vec<&str> = translation.lines().collect();
        let rows = src_lines.len().max(trans_lines.len());
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(vec![
            Span::styled(
                format!(" source ({src_iso}) "),
                Style::default().fg(self.theme.tree_chapter_fg).add_modifier(Modifier::BOLD),
            ),
        ]));
        for i in 0..rows.min(body_h.saturating_sub(1) as usize) {
            let s = src_lines.get(i).copied().unwrap_or("");
            let t = trans_lines.get(i).copied().unwrap_or("");
            let s = truncate_to(s, col_w.max(1));
            let t = truncate_to(t, col_w.max(1));
            lines.push(Line::from(vec![
                Span::styled(format!("{s:<col_w$}"), Style::default()),
                Span::styled(" │ ", Style::default().fg(self.theme.modal_border)),
                Span::styled(format!("→ {t}"), Style::default().fg(self.theme.tree_subchapter_fg)),
            ]));
        }
        let body_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: body_h };
        f.render_widget(Paragraph::new(lines), body_rect);

        // Trilemma footer.
        let bar = |score: f64| -> String {
            let n = (score * 10.0).round().clamp(0.0, 10.0) as usize;
            format!("{}{}", "█".repeat(n), "░".repeat(10 - n))
        };
        let mut foot: Vec<Line<'static>> = Vec::new();
        foot.push(Line::from(Span::styled(
            format!(" ── trilemma ({src_iso} → {trans_iso}) ────────"),
            Style::default().fg(self.theme.modal_border).add_modifier(Modifier::DIM),
        )));
        foot.push(Line::from(vec![
            Span::styled(" Form    ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(bar(tri.form_score), Style::default().fg(ratatui::style::Color::Green)),
            Span::raw(format!("  {:>3.0}%  ", tri.form_score * 100.0)),
            Span::styled(
                truncate_to(&format!("{} · {}", tri.metre_note, tri.rhyme_note), col_w),
                Style::default().add_modifier(Modifier::DIM),
            ),
        ]));
        foot.push(Line::from(vec![
            Span::styled(" Meaning ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled("░░░░░░░░░░", Style::default().add_modifier(Modifier::DIM)),
            Span::raw("       "),
            Span::styled(
                "the AI axis — engage the Inner Poet (E)".to_string(),
                Style::default().add_modifier(Modifier::DIM),
            ),
        ]));
        foot.push(Line::from(vec![
            Span::styled(" Sound   ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(bar(tri.sound_score), Style::default().fg(ratatui::style::Color::Green)),
            Span::raw(format!("  {:>3.0}%  ", tri.sound_score * 100.0)),
            Span::styled(
                truncate_to(&tri.sound_note, col_w),
                Style::default().add_modifier(Modifier::DIM),
            ),
        ]));
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + body_h,
            width: inner.width,
            height: footer_h,
        };
        f.render_widget(Paragraph::new(foot), footer_rect);
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

    /// 1.2.13+ Phase C.2 — `Ctrl+B Q` / `Ctrl+B Shift+Q`
    /// language picker.  Pops only when 2+ Language sub-
    /// books exist (single-language projects skip the
    /// modal entirely).  Layout mirrors the LlmPicker
    /// modal — small centred list with first-letter
    /// shortcut hint in the footer.
    pub(in crate::tui::app) fn draw_translation_picker_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::TranslationLanguagePicker {
            entries,
            cursor,
            direction,
            ..
        } = &self.modal
        else {
            return;
        };
        let header_lines = 2;
        let footer_lines = 2;
        let body_lines = entries.len();
        let height = (header_lines + body_lines + footer_lines + 2) as u16;
        let height = height.clamp(8, area.height.saturating_sub(2));

        let max_name = entries
            .iter()
            .map(|(_, n)| n.chars().count())
            .max()
            .unwrap_or(8);
        let width = (max_name + 14) as u16;
        let width = width.clamp(40, area.width.saturating_sub(6));

        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let arrow = match direction {
            super::super::super::modal::TranslationDirection::ToInvented =>
                " Translate INTO · Ctrl+B Q ",
            super::super::super::modal::TranslationDirection::FromInvented =>
                " Translate FROM · Ctrl+B Shift+Q ",
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(arrow)
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
        for (i, (_, name)) in entries.iter().enumerate() {
            let marker = if i == *cursor { "›" } else { " " };
            let first_letter = name
                .chars()
                .next()
                .map(|c| c.to_ascii_uppercase().to_string())
                .unwrap_or_else(|| "?".into());
            // Highlight the first letter so the
            // "press the letter to jump+commit" hint
            // in the footer is obvious from the rows
            // themselves.
            let style = if i == *cursor {
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let row = format!("  {marker} [{first_letter}] {name}");
            lines.push(Line::from(Span::styled(row, style)));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  ↑↓ Enter · type first letter to jump-and-commit · Esc to cancel"
                .to_string(),
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
    /// Long lines are wrapped at column width (1.2.11+) —
    /// each side wraps independently then the shorter side
    /// is padded with empty rows so paired diff entries stay
    /// vertically aligned.  Continuation rows are indented
    /// two columns (matching the diff prefix width) so the
    /// visual flow of a wrapped sentence is unambiguous.
    pub(in crate::tui::app) fn draw_ai_diff_review_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let (before_text, after_text, scroll_in) = {
            let Modal::AiDiffReview {
                before_lines,
                after_lines,
                scroll,
                ..
            } = &self.modal
            else {
                return;
            };
            (
                before_lines.join("\n"),
                after_lines.join("\n"),
                *scroll,
            )
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
        // Leave one trailing cell as a visual gutter so the
        // wrapped tail doesn't kiss the column boundary.
        let left_w = (before_rect.width as usize).saturating_sub(1).max(1);
        let right_w = (after_rect.width as usize).saturating_sub(1).max(1);

        let diff = similar::TextDiff::from_lines(&before_text, &after_text);
        let mut left: Vec<Line> = Vec::new();
        let mut right: Vec<Line> = Vec::new();
        for change in diff.iter_all_changes() {
            let raw = change.value().trim_end_matches('\n').to_string();
            let (left_rows, right_rows) = match change.tag() {
                similar::ChangeTag::Equal => (
                    wrap_diff_row(&raw, "  ", left_w, Style::default()),
                    wrap_diff_row(&raw, "  ", right_w, Style::default()),
                ),
                similar::ChangeTag::Delete => (
                    wrap_diff_row(
                        &raw,
                        "- ",
                        left_w,
                        Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    ),
                    vec![Line::from("")],
                ),
                similar::ChangeTag::Insert => (
                    vec![Line::from("")],
                    wrap_diff_row(
                        &raw,
                        "+ ",
                        right_w,
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                ),
            };
            let n = left_rows.len().max(right_rows.len()).max(1);
            for i in 0..n {
                left.push(
                    left_rows.get(i).cloned().unwrap_or_else(|| Line::from("")),
                );
                right.push(
                    right_rows.get(i).cloned().unwrap_or_else(|| Line::from("")),
                );
            }
        }
        let total = left.len();
        // Write the wrapped total back into the modal so the
        // key handler can clamp scroll against the
        // post-wrap row count instead of the source-line
        // count.
        if let Modal::AiDiffReview { wrapped_total, .. } = &mut self.modal {
            *wrapped_total = total;
        }
        let start = scroll_in.min(total.saturating_sub(1));
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
        // 1.2.15+ Phase S.2 — `.get(*idx)` instead
        // of `entries[*idx]` so a stale match index
        // (e.g. after a tree mutation invalidated
        // `entries` but the picker still holds the
        // old `matches` ring) skips that row instead
        // of panicking the renderer.
        let lines: Vec<Line<'_>> = matches
            .iter()
            .enumerate()
            .skip(*scroll)
            .take(body_h)
            .filter_map(|(i, idx)| {
                let e = entries.get(*idx)?;
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
                Some(line)
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        // 1.2.12+ Phase B — Shift+Enter pins to the
        // split-view secondary pane.
        let hint = " ↑↓ select · Enter opens · Shift+Enter pins to split · Esc closes ";
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// 1.4.5+ SOURCES-1 — the Ctrl+V @ cite picker. Each row is
    /// `@key   year · author — title`; the input box fuzzy-filters;
    /// Enter inserts `@key` at the editor cursor.
    pub(in crate::tui::app) fn draw_cite_picker_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::CitePicker { input, entries, cursor, scroll } = &self.modal
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

        let header = format!(" Cite ({}/{}) ", matches.len(), entries.len());
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

        let input_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: 1 };
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
            Paragraph::new(Line::from(format!(" › {}", input.render_with_cursor('│')))),
            input_rect,
        );

        let body_h = body_rect.height as usize;
        let lines: Vec<Line<'_>> = matches
            .iter()
            .enumerate()
            .skip(*scroll)
            .take(body_h)
            .filter_map(|(i, idx)| {
                let e = entries.get(*idx)?;
                let head = format!(" @{}", e.title);
                let desc = format!("    {}", e.slug_path);
                let spans: Vec<Span> = vec![
                    Span::raw(head),
                    Span::styled(desc, Style::default().add_modifier(Modifier::DIM)),
                ];
                let mut line = Line::from(spans);
                if i == *cursor {
                    line = line.style(Style::default().add_modifier(Modifier::REVERSED));
                }
                Some(line)
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        let hint = " ↑↓ select · Enter inserts @key · Esc closes ";
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// 1.6.15+ TYPST-UNIVERSE — the Ctrl+V # package import picker. Each row is
    /// `@preview/<name>:<version>  ★stars · description`; the input box
    /// fuzzy-filters; Enter inserts a `#import` line.
    pub(in crate::tui::app) fn draw_universe_picker_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::UniversePicker { input, entries, cursor, scroll } = &self.modal
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

        let header = format!(" Typst Universe ({}/{}) ", matches.len(), entries.len());
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

        let input_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: 1 };
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
            Paragraph::new(Line::from(format!(" › {}", input.render_with_cursor('│')))),
            input_rect,
        );

        let body_h = body_rect.height as usize;
        let lines: Vec<Line<'_>> = matches
            .iter()
            .enumerate()
            .skip(*scroll)
            .take(body_h)
            .filter_map(|(i, idx)| {
                let e = entries.get(*idx)?;
                let head = format!(" {}", e.title);
                let desc = format!("    {}", e.slug_path);
                let spans: Vec<Span> = vec![
                    Span::raw(head),
                    Span::styled(desc, Style::default().add_modifier(Modifier::DIM)),
                ];
                let mut line = Line::from(spans);
                if i == *cursor {
                    line = line.style(Style::default().add_modifier(Modifier::REVERSED));
                }
                Some(line)
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        let hint = " ↑↓ select · Enter inserts #import · Ctrl+R refresh · Esc closes ";
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// 1.6.15+ XREF-2 — the Ctrl+V & cross-reference picker. Each row is
    /// `@label  category · where`; the input box fuzzy-filters; Enter inserts a
    /// `@label` reference.
    pub(in crate::tui::app) fn draw_xref_picker_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::XrefPicker { input, entries, cursor, scroll } = &self.modal
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

        let header = format!(" Cross-reference ({}/{}) ", matches.len(), entries.len());
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

        let input_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: 1 };
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
            Paragraph::new(Line::from(format!(" › {}", input.render_with_cursor('│')))),
            input_rect,
        );

        let body_h = body_rect.height as usize;
        let lines: Vec<Line<'_>> = matches
            .iter()
            .enumerate()
            .skip(*scroll)
            .take(body_h)
            .filter_map(|(i, idx)| {
                let e = entries.get(*idx)?;
                let head = format!(" @{}", e.title);
                let desc = format!("    {}", e.slug_path);
                let spans: Vec<Span> = vec![
                    Span::raw(head),
                    Span::styled(desc, Style::default().add_modifier(Modifier::DIM)),
                ];
                let mut line = Line::from(spans);
                if i == *cursor {
                    line = line.style(Style::default().add_modifier(Modifier::REVERSED));
                }
                Some(line)
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        let hint = " ↑↓ select · Enter inserts @label · Esc closes ";
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// 1.4.9+ REUSE-1 — the Ctrl+V x snippet include picker. Each row is
    /// `slug  preview`; the input box fuzzy-filters; Enter inserts/replaces a
    /// `#include`.
    pub(in crate::tui::app) fn draw_snippet_include_picker_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::SnippetIncludePicker { input, entries, cursor, scroll, mode } = &self.modal
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

        let verb = match mode {
            crate::tui::state::SnippetPickerMode::Replace { .. } => "Replace",
            crate::tui::state::SnippetPickerMode::Insert => "Insert",
        };
        let header = format!(" Snippet · {verb} ({}/{}) ", matches.len(), entries.len());
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

        let input_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: 1 };
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
            Paragraph::new(Line::from(format!(" › {}", input.render_with_cursor('│')))),
            input_rect,
        );

        let body_h = body_rect.height as usize;
        let lines: Vec<Line<'_>> = matches
            .iter()
            .enumerate()
            .skip(*scroll)
            .take(body_h)
            .filter_map(|(i, idx)| {
                let e = entries.get(*idx)?;
                let head = format!(" {}", e.title);
                let desc = format!("    {}", e.slug_path);
                let spans: Vec<Span> = vec![
                    Span::raw(head),
                    Span::styled(desc, Style::default().add_modifier(Modifier::DIM)),
                ];
                let mut line = Line::from(spans);
                if i == *cursor {
                    line = line.style(Style::default().add_modifier(Modifier::REVERSED));
                }
                Some(line)
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        let hint = match mode {
            crate::tui::state::SnippetPickerMode::Replace { .. } => {
                " ↑↓ select · Enter replaces the include path · Esc closes "
            }
            crate::tui::state::SnippetPickerMode::Insert => {
                " ↑↓ select · Enter inserts #include · Esc closes "
            }
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// 1.4.9+ REUSE-1 — the Ctrl+V Shift+X snippets overview: `slug  (N refs)
    /// preview`, Enter jumps to the source.
    pub(in crate::tui::app) fn draw_snippets_overview_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::SnippetsOverview { rows, cursor, scroll } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(14);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Snippets ({}) ", rows.len()))
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

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
        let body_h = body_rect.height as usize;
        let lines: Vec<Line<'_>> = rows
            .iter()
            .enumerate()
            .skip(*scroll)
            .take(body_h)
            .map(|(i, r)| {
                let refs = if r.reference_count == 0 {
                    "  (unused)".to_string()
                } else {
                    format!("  ({} ref)", r.reference_count)
                };
                let spans: Vec<Span> = vec![
                    Span::raw(format!(" {}", r.slug)),
                    Span::styled(refs, Style::default().add_modifier(Modifier::DIM)),
                    Span::styled(
                        format!("   {}", r.preview),
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

        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " ↑↓ select · Enter jumps to source · Esc closes ",
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// 1.3.33+ — the Ctrl+Shift+P command palette. Each row is
    /// `label  chord  description`; the input box fuzzy-filters.
    pub(in crate::tui::app) fn draw_command_palette_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::CommandPalette { input, entries, cursor, scroll } = &self.modal
        else {
            return;
        };
        let matches = crate::tui::palette::fuzzy_filter(entries, input.as_str());

        let width = area.width.saturating_sub(8).max(72);
        let height = area.height.saturating_sub(4).max(16);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = format!(" Command palette ({}/{}) ", matches.len(), entries.len());
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

        let input_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: 1 };
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
            Paragraph::new(Line::from(format!(" › {}", input.render_with_cursor('│')))),
            input_rect,
        );

        let body_h = body_rect.height as usize;
        let lines: Vec<Line<'_>> = matches
            .iter()
            .enumerate()
            .skip(*scroll)
            .take(body_h)
            .filter_map(|(i, idx)| {
                let e = entries.get(*idx)?;
                let spans: Vec<Span> = vec![
                    Span::raw(format!(" {:<26}", e.label)),
                    Span::styled(
                        format!(" {:<16}", e.chord),
                        Style::default().fg(ratatui::style::Color::Cyan),
                    ),
                    Span::styled(
                        format!(" {}", e.description),
                        Style::default().add_modifier(Modifier::DIM),
                    ),
                ];
                let mut line = Line::from(spans);
                if i == *cursor {
                    line = line.style(Style::default().add_modifier(Modifier::REVERSED));
                }
                Some(line)
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        let hint = " type to filter · ↑↓ select · Enter run · Esc close ";
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
                " ↑↓ select · Enter opens · Shift+Enter pins to split · D removes bookmark · Esc closes    ({}/{}) ",
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
            // 1.2.8+ — colored prompt:
            //   "[ " white   <cwd> blue   " > " red   <input>
            // The cwd reflects `$env.PWD` so `cd` mutations
            // surface immediately.  Long paths under $HOME
            // are abbreviated to `~/...` for legibility; the
            // path is otherwise rendered verbatim and the
            // terminal will let it run off-screen if absurdly
            // long (acceptable — the user can resize or
            // `cd` to a shorter location).
            let cwd_display: String = self
                .shell_engine
                .as_ref()
                .map(|e| {
                    let p = e.cwd();
                    let raw = p.to_string_lossy().into_owned();
                    if let Some(home) = std::env::var_os("HOME") {
                        let home = home.to_string_lossy().into_owned();
                        if raw == home {
                            "~".to_string()
                        } else if raw.starts_with(&format!("{home}/")) {
                            format!("~{}", &raw[home.len()..])
                        } else {
                            raw
                        }
                    } else {
                        raw
                    }
                })
                .unwrap_or_else(|| ".".to_string());
            let mut spans: Vec<Span<'_>> = vec![
                Span::styled(
                    "[ ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    cwd_display.clone(),
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " > ",
                    Style::default()
                        .fg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                ),
            ];
            // Width of the prompt prefix, in display columns
            // — used to position the cursor after the typed
            // text.  Assumes 1 col / char, which is correct
            // for ASCII paths; non-ASCII cwd chars would
            // slightly off-set the cursor but that's a niche
            // issue we'll fix when it appears.
            let prefix_cols = "[ ".chars().count()
                + cwd_display.chars().count()
                + " > ".chars().count();

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
            let cursor_col = prefix_cols + input.cursor();
            let max_col = prompt_line_rect.width.saturating_sub(1) as usize;
            let x = prompt_line_rect.x
                + cursor_col.min(max_col) as u16;
            f.set_cursor_position((x, prompt_line_rect.y));
        }
        let hint = if *selection_mode {
            " ↑↓ turn · PgUp/PgDn scroll · c copy · i insert · Ctrl+Z h exit · Esc exit "
        } else {
            " Enter run · Tab complete · Ctrl+B H help · ↑↓ cmd history · Esc close "
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

        // 1.2.8+ — help overlay.  Renders ON TOP of the
        // pane, centered, with chord + command basics.  Any
        // key dismisses it (handled in shell_pane_handle_key
        // before falling into the normal key dispatcher).
        let show_help = matches!(
            self.modal,
            Modal::ShellPane { show_help: true, .. }
        );
        if show_help {
            draw_shell_help_overlay(f, rect);
        }
    }

    /// 1.2.8+ — full-screen HJSON editor for the project's
    /// `inkhaven.hjson`.  Renders the textarea's lines
    /// manually so per-line `hjson_highlight` styling
    /// (keys / strings / comments / numbers / keywords) can
    /// 1.2.9+ — GitHub-style writing-streak heatmap.
    /// 13×7 grid (91 days), each cell colored by daily
    /// word-count bucket (0 → dim, 1-249 → faint, 250-
    /// 499 → medium, 500-999 → bright, 1000+ → max).
    /// Week columns left-to-right oldest→today; day
    /// rows Mon-Sun.  Footer shows current streak,
    /// longest streak in window, total words, and
    /// active-day average.  Modal closes on any key.
    pub(in crate::tui::app) fn draw_writing_streak_heatmap(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let (daily_words, streak_days, longest_streak, today_ymd) =
            match &self.modal {
                Modal::WritingStreakHeatmap {
                    daily_words,
                    streak_days,
                    longest_streak,
                    today_ymd,
                } => (daily_words.clone(), *streak_days, *longest_streak, *today_ymd),
                _ => return,
            };

        // Modal rect: centered, ~70% wide, ~18 rows tall
        // (enough for the grid + header + footer +
        // borders).
        let w = area.width.saturating_sub(6).min(80);
        let h = area.height.saturating_sub(4).min(20);
        let x = area.x + (area.width - w) / 2;
        let y = area.y + (area.height - h) / 2;
        let rect = Rect { x, y, width: w, height: h };

        f.render_widget(ratatui::widgets::Clear, rect);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Writing streak — last 91 days ")
            .border_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        // Today's weekday so the bottom-right of the grid
        // is today.  91 days = 13 weeks × 7.
        let n = daily_words.len();
        let today = match chrono::NaiveDate::from_ymd_opt(
            today_ymd.0,
            today_ymd.1,
            today_ymd.2,
        ) {
            Some(d) => d,
            None => chrono::Utc::now().date_naive(),
        };
        use chrono::Datelike;
        let today_wd = today.weekday().num_days_from_monday();
        // Today sits at column 12 (rightmost), row =
        // today_wd.  Each cell at (col, row) maps to a
        // day index in daily_words.
        let today_cell: i64 = (today_wd as i64) + 12 * 7;

        // Layout sub-rects.
        let header_h: u16 = 2;
        let footer_h: u16 = 5;
        let grid_h: u16 = inner.height.saturating_sub(header_h + footer_h);
        let header_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: header_h,
        };
        let grid_rect = Rect {
            x: inner.x,
            y: inner.y + header_h,
            width: inner.width,
            height: grid_h,
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + header_h + grid_h,
            width: inner.width,
            height: footer_h,
        };

        // Header — month labels above each week column.
        let mut header_text = String::from("    "); // skip day-label column
        let mut last_month: Option<u32> = None;
        for col in 0..13_i64 {
            let cell = col * 7;
            let day_offset_from_today = today_cell - cell;
            let date = today
                .checked_sub_signed(chrono::Duration::days(day_offset_from_today))
                .unwrap_or(today);
            let month = date.month();
            let label = if Some(month) != last_month {
                last_month = Some(month);
                match month {
                    1 => "Jn",
                    2 => "Fb",
                    3 => "Mr",
                    4 => "Ap",
                    5 => "My",
                    6 => "Jn",
                    7 => "Jl",
                    8 => "Au",
                    9 => "Sp",
                    10 => "Oc",
                    11 => "Nv",
                    12 => "Dc",
                    _ => "??",
                }
            } else {
                "  "
            };
            header_text.push_str(label);
        }
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                header_text,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            header_rect,
        );

        // Grid.  Day-label column on the left, then
        // 13 columns × 7 rows of colored cells.
        let day_names = [" Mon", " Tue", " Wed", " Thu", " Fri", " Sat", " Sun"];
        for row in 0..7_usize {
            let mut spans: Vec<Span<'_>> = Vec::with_capacity(14);
            spans.push(Span::styled(
                day_names[row],
                Style::default().add_modifier(Modifier::DIM),
            ));
            for col in 0..13_i64 {
                let cell = col * 7 + row as i64;
                let day_idx = (n as i64 - 1) - (today_cell - cell);
                let (glyph, color) = if day_idx < 0 || (day_idx as usize) >= n {
                    ("·", Color::DarkGray)
                } else {
                    let words = daily_words[day_idx as usize];
                    heat_glyph_and_color(words)
                };
                let is_today =
                    day_idx >= 0 && (day_idx as usize) == n.saturating_sub(1);
                let style = if is_today {
                    Style::default().fg(color).bg(Color::Rgb(0x44, 0x44, 0x44))
                } else {
                    Style::default().fg(color)
                };
                spans.push(Span::raw(" "));
                spans.push(Span::styled(glyph.to_string(), style));
            }
            let row_rect = Rect {
                x: grid_rect.x,
                y: grid_rect.y + row as u16,
                width: grid_rect.width,
                height: 1,
            };
            f.render_widget(Paragraph::new(Line::from(spans)), row_rect);
        }

        // Footer.
        let total_words: i64 = daily_words.iter().sum();
        let active_days = daily_words.iter().filter(|w| **w > 0).count();
        let avg_per_active = if active_days > 0 {
            total_words / active_days as i64
        } else {
            0
        };
        let footer_lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    format!(" {streak_days}-day current streak"),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  ·  "),
                Span::raw(format!("{longest_streak}-day longest in window")),
                Span::raw("  ·  "),
                Span::raw(format!(
                    "{active_days}/91 days active · avg {avg_per_active} w/day"
                )),
            ]),
            Line::from(Span::raw(format!(
                " {total_words} total words in window"
            ))),
            Line::from(vec![
                Span::raw(" Legend: "),
                Span::styled("·", Style::default().fg(Color::DarkGray)),
                Span::raw(" 0  "),
                Span::styled("░", Style::default().fg(Color::Rgb(0x40, 0xa0, 0x40))),
                Span::raw(" 1-249  "),
                Span::styled("▒", Style::default().fg(Color::Rgb(0x60, 0xc0, 0x60))),
                Span::raw(" 250-499  "),
                Span::styled("▓", Style::default().fg(Color::Rgb(0x40, 0xe0, 0x40))),
                Span::raw(" 500-999  "),
                Span::styled("█", Style::default().fg(Color::Rgb(0x80, 0xff, 0x80))),
                Span::raw(" 1000+"),
            ]),
            Line::from(Span::styled(
                " e edit goals · any other key closes",
                Style::default().add_modifier(Modifier::DIM),
            )),
        ];
        f.render_widget(
            Paragraph::new(footer_lines).wrap(Wrap { trim: false }),
            footer_rect,
        );
    }

    /// be applied — tui-textarea's built-in widget supports
    /// only line-level + cursor-level styling, not per-token.
    /// Pops a centered "config changed, restart inkhaven"
    /// overlay when `restart_required = true`.  Status hint
    /// at the bottom row.
    pub(in crate::tui::app) fn draw_hjson_editor_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let (lines, cursor_pos, restart_required, path_display, scroll_row, scroll_col) =
            match &self.modal {
                Modal::HjsonEditor {
                    textarea,
                    restart_required,
                    path,
                    scroll_row,
                    scroll_col,
                    ..
                } => (
                    textarea.lines().to_vec(),
                    textarea.cursor(),
                    *restart_required,
                    path.to_string_lossy().into_owned(),
                    *scroll_row,
                    *scroll_col,
                ),
                _ => return,
            };

        // Fullscreen-floating with a 1-cell margin so the
        // editor pane borders stay visible underneath.
        let rect = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };
        f.render_widget(ratatui::widgets::Clear, rect);
        let dirty = matches!(
            &self.modal,
            Modal::HjsonEditor { textarea, original_content, .. }
                if textarea.lines().join("\n") != *original_content
        );
        let title = if dirty {
            format!(" {} • [modified] ", path_display)
        } else {
            format!(" {} ", path_display)
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

        // Reserve last row for the status hint.
        let body_h = inner.height.saturating_sub(1);
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: body_h,
        };
        let hint_rect = Rect {
            x: inner.x,
            y: inner.y + body_h,
            width: inner.width,
            height: 1,
        };

        // Recompute scroll to keep the cursor visible.  We
        // can't mutate scroll on `&self.modal` (we only have
        // a borrow here), so capture the new values and
        // write back at the end.
        let body_h_us = body_h as usize;
        let body_w_us = body_rect.width as usize;
        let (cur_row, cur_col) = cursor_pos;
        let mut new_scroll_row = scroll_row;
        let mut new_scroll_col = scroll_col;
        if body_h_us > 0 {
            if cur_row < new_scroll_row {
                new_scroll_row = cur_row;
            } else if cur_row >= new_scroll_row + body_h_us {
                new_scroll_row = cur_row + 1 - body_h_us;
            }
        }
        // Reserve 4 cells for the line-number gutter when
        // computing visible width.
        let gutter_w: usize = 5;
        let editable_w = body_w_us.saturating_sub(gutter_w);
        if editable_w > 0 {
            if cur_col < new_scroll_col {
                new_scroll_col = cur_col;
            } else if cur_col >= new_scroll_col + editable_w {
                new_scroll_col = cur_col + 1 - editable_w;
            }
        }

        // Highlight the entire source (all lines) so cross-
        // line `/* … */` / `''' … '''` constructs colour
        // correctly even when the user scrolls into the
        // middle of one.
        let source: String = lines.join("\n");
        let highlighted =
            super::super::super::hjson_highlight::highlight_hjson_lines(
                &source,
                &self.theme,
            );

        // 1.2.15+ Phase S.2 — bound `row_end` against
        // the actual `highlighted.len()`, not the
        // `.max(1)`-clamped `total_lines`.  The clamp
        // was needed for "show 1 empty line when the
        // buffer is empty" semantics, but with it
        // active, an empty buffer would index
        // `highlighted[0]` below and crash.  Skip the
        // loop instead when the buffer is genuinely
        // empty.
        let total_lines = highlighted.len();
        let row_end = (new_scroll_row + body_h_us).min(total_lines);
        let mut painted: Vec<Line<'_>> = Vec::with_capacity(body_h_us);
        for row in new_scroll_row..row_end {
            let lineno_text = format!("{:>4} ", row + 1);
            let mut spans: Vec<Span<'_>> = vec![Span::styled(
                lineno_text,
                Style::default().fg(self.theme.line_number_fg),
            )];
            // Concat the highlighted runs into a single
            // string + parallel style list so we can slice
            // by column for horizontal scroll.
            //
            // 1.2.15+ Phase S.2 — defensive read.  The
            // loop bound guarantees `row < highlighted.
            // len()`, but a future refactor could break
            // that; use `.get(row)` + early-continue so
            // a stray miscalculation skips a row
            // instead of panicking the renderer.
            let Some(runs) = highlighted.get(row) else {
                continue;
            };
            let mut cells: Vec<(char, Style)> = Vec::new();
            for run in runs {
                for ch in run.text.chars() {
                    cells.push((ch, run.style));
                }
            }
            // Slice by horizontal scroll.
            let start = new_scroll_col.min(cells.len());
            let end = (new_scroll_col + editable_w).min(cells.len());
            // Pack consecutive same-style runs back into Spans.
            let mut i = start;
            while i < end {
                let style = cells[i].1;
                let run_start = i;
                while i < end && cells[i].1 == style {
                    i += 1;
                }
                let text: String = cells[run_start..i].iter().map(|(c, _)| *c).collect();
                spans.push(Span::styled(text, style));
            }
            painted.push(Line::from(spans));
        }
        f.render_widget(
            Paragraph::new(painted),
            body_rect,
        );

        // Place the terminal cursor for visual feedback —
        // gutter (5 cells) + column relative to scroll.
        let cursor_screen_col = gutter_w + cur_col.saturating_sub(new_scroll_col);
        let cursor_screen_row = cur_row.saturating_sub(new_scroll_row);
        if cursor_screen_row < body_h_us && cursor_screen_col < body_w_us {
            f.set_cursor_position((
                body_rect.x + cursor_screen_col as u16,
                body_rect.y + cursor_screen_row as u16,
            ));
        }

        // Hint line.  1.2.12+ — Ctrl+R fires the
        // reviewer-LLM critique of the buffer; the
        // response streams into App.inference and is
        // visible in the AI pane after closing.
        let hint = if dirty {
            " Ctrl+S save · Ctrl+R review · Esc close · arrows / Page navigate · [unsaved] "
        } else {
            " Ctrl+S save · Ctrl+R review · Esc close · arrows / Page navigate "
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            hint_rect,
        );

        // Write scroll changes back into the modal state
        // for the next render frame.
        if let Modal::HjsonEditor {
            scroll_row,
            scroll_col,
            ..
        } = &mut self.modal
        {
            *scroll_row = new_scroll_row;
            *scroll_col = new_scroll_col;
        }

        // Restart-required overlay (drawn last so it's on top).
        if restart_required {
            draw_hjson_restart_overlay(f, rect);
        }
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

        let header = format!(
            " Kill-ring ({}/{}) ",
            len, self.cfg.editor.deleted_paragraph_history
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

    /// 1.8.5 — the WordNet thesaurus modal: the word under the cursor and a
    /// pick-list of candidate replacements (synonym / antonym / hypernym /
    /// hyponym), each tagged by its relation. Enter replaces the word.
    pub(in crate::tui::app) fn draw_thesaurus_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::Thesaurus { panel, scroll, .. } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(8).clamp(40, 64);
        let height = area.height.saturating_sub(4).max(10);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = format!(" Thesaurus · {} ({}) ", panel.word, panel.lang);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1) as usize;
        let body_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: inner.height.saturating_sub(1) };
        let footer_rect = Rect { x: inner.x, y: inner.y + inner.height.saturating_sub(1), width: inner.width, height: 1 };

        let lines: Vec<Line<'_>> = panel
            .suggestions
            .iter()
            .enumerate()
            .skip(*scroll)
            .take(body_h)
            .map(|(i, s)| {
                let row = format!(" {:<9} {}", s.kind, s.word);
                let mut line = Line::from(vec![
                    Span::styled(
                        format!(" {:<9}", s.kind),
                        Style::default().add_modifier(Modifier::DIM),
                    ),
                    Span::raw(format!(" {}", s.word)),
                ]);
                let _ = row;
                if i == panel.selected {
                    line = line.style(Style::default().add_modifier(Modifier::REVERSED));
                }
                line
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        let hint = format!(
            " ↑↓ select · Enter replace · Esc cancel    ({}/{}) ",
            panel.selected + 1,
            panel.suggestions.len()
        );
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(hint, Style::default().add_modifier(Modifier::DIM)))),
            footer_rect,
        );
    }

    /// 1.3.0 PDF-1 — `Ctrl+B Q` imposition preview: the plan (signatures
    /// / sheets / creep) + the first sheet's schematic, with an
    /// impose/cancel footer.
    pub(in crate::tui::app) fn draw_imposition_preview_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::ImpositionPreview { lines, .. } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(8).clamp(50, 80);
        let height = (lines.len() as u16 + 4).min(area.height.saturating_sub(2));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect {
            x,
            y,
            width,
            height,
        };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Imposition preview ")
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
        let body = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: body_h,
        };
        let footer = Rect {
            x: inner.x,
            y: inner.y + body_h,
            width: inner.width,
            height: 1,
        };
        let text: Vec<Line> = lines.iter().map(|l| Line::from(l.clone())).collect();
        f.render_widget(Paragraph::new(text), body);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " Enter: impose · Esc: cancel ",
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer,
        );
    }

    /// 1.3.1 SUBMISSION-1 — the submission tracker: one row per record with
    /// a colour-coded status, the cursor row reversed.  Space/`s` cycles
    /// status, `d` removes (both persist), Esc closes.
    pub(in crate::tui::app) fn draw_submissions_tracker_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        use ratatui::style::Color;
        let Modal::SubmissionsTracker { records, cursor } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(6).clamp(50, 92);
        // The selected record expands to show its note trail, so reserve
        // those extra lines too.
        let extra = records.get(*cursor).map(|r| r.log.len()).unwrap_or(0) as u16;
        let rows = (records.len() as u16 + extra).max(1);
        let height = (rows + 4).min(area.height.saturating_sub(2));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Submissions ({}) ", records.len()))
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
        let body = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: body_h,
        };
        let footer = Rect {
            x: inner.x,
            y: inner.y + body_h,
            width: inner.width,
            height: 1,
        };

        let status_color = |s: crate::submissions::SubmissionStatus| -> Color {
            use crate::submissions::SubmissionStatus as S;
            match s {
                S::Drafting => Color::Gray,
                S::Sent => Color::Cyan,
                S::Rejected => Color::Red,
                S::Offer => Color::Green,
                S::Withdrawn => Color::DarkGray,
            }
        };

        let mut lines: Vec<Line> = Vec::new();
        if records.is_empty() {
            lines.push(Line::from(Span::styled(
                "  No submissions yet — add with: inkhaven submissions add --market \"…\"",
                Style::default().add_modifier(Modifier::DIM),
            )));
        } else {
            for (i, r) in records.iter().enumerate() {
                let sel = i == *cursor;
                let mut spans = vec![
                    Span::raw(if sel { "▶ " } else { "  " }),
                    Span::raw(format!("{:<4} ", r.id)),
                    Span::styled(
                        format!("{:<9} ", r.status.label()),
                        Style::default()
                            .fg(status_color(r.status))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(r.market.clone()),
                ];
                if let Some(a) = &r.agent {
                    spans.push(Span::raw(format!(" · {a}")));
                }
                if let Some(d) = &r.date_sent {
                    spans.push(Span::raw(format!(" · sent {d}")));
                }
                if let Some(d) = &r.response_date {
                    spans.push(Span::raw(format!(" · heard {d}")));
                }
                if let Some(d) = &r.next_action_date {
                    spans.push(Span::styled(
                        format!(" · next {d}"),
                        Style::default().fg(Color::Yellow),
                    ));
                }
                if !r.log.is_empty() {
                    spans.push(Span::styled(
                        format!(" · 📝{}", r.log.len()),
                        Style::default().add_modifier(Modifier::DIM),
                    ));
                }
                let line = Line::from(spans);
                lines.push(if sel {
                    line.style(Style::default().add_modifier(Modifier::REVERSED))
                } else {
                    line
                });
                // Expand the selected record's timestamped note trail.
                if sel {
                    for entry in &r.log {
                        lines.push(Line::from(Span::styled(
                            format!("      [{}] {}", entry.date, entry.text),
                            Style::default().add_modifier(Modifier::DIM),
                        )));
                    }
                }
            }
        }
        f.render_widget(Paragraph::new(lines), body);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " ↑↓ move · Space/s status · d remove · Esc close ",
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer,
        );
    }

    /// 1.3.1 SUBMISSION-1 P3.3 — the generator picker: a short menu of the
    /// package pieces; Enter streams the selected one into the AI pane.
    pub(in crate::tui::app) fn draw_submission_gen_picker_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::SubmissionGenPicker { cursor, .. } = &self.modal else {
            return;
        };
        let kinds = crate::submission_gen::SubmissionKind::ALL;
        let width = area.width.saturating_sub(6).clamp(40, 64);
        let height = (kinds.len() as u16 + 4).min(area.height.saturating_sub(2));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Generate submission piece ")
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let body_h = inner.height.saturating_sub(1);
        let body = Rect { x: inner.x, y: inner.y, width: inner.width, height: body_h };
        let footer = Rect {
            x: inner.x,
            y: inner.y + body_h,
            width: inner.width,
            height: 1,
        };

        let lines: Vec<Line> = kinds
            .iter()
            .enumerate()
            .map(|(i, k)| {
                let sel = i == *cursor;
                let text = format!("{} {}", if sel { "▶" } else { " " }, k.title());
                let line = Line::from(text);
                if sel {
                    line.style(Style::default().add_modifier(Modifier::REVERSED))
                } else {
                    line
                }
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " ↑↓ pick · Enter → AI pane · Esc cancel ",
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer,
        );
    }

    /// 1.3.2 PLANNING-1 P2 — the structure outline: the `plan check` report
    /// as a per-beat position bar (target `|` vs actual `●`) + act pacing.
    pub(in crate::tui::app) fn draw_plan_outline_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        use ratatui::style::Color;
        let Modal::PlanOutline {
            book_title,
            framework,
            report,
            cursor,
            picking,
            thread_pick,
            scenes,
            scene_view,
            scene_cursor,
            ..
        } = &self.modal
        else {
            return;
        };
        let drift = 0.10_f32;
        let bar_w = 20usize;
        // beats + pacing + the tension overlay (header + 2 sparklines +
        // numerals) + chrome.
        let rows = report.beats.len() + report.acts.len() + 10;
        let width = area.width.saturating_sub(6).clamp(54, 88);
        let height = (rows as u16 + 4).min(area.height.saturating_sub(2));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let fw_label = crate::planning::Framework::parse(framework)
            .map(|f| f.label().to_string())
            .unwrap_or_else(|| framework.clone());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Structure · {book_title} · {fw_label} "))
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);
        let body_h = inner.height.saturating_sub(1);
        let body = Rect { x: inner.x, y: inner.y, width: inner.width, height: body_h };
        let footer = Rect {
            x: inner.x,
            y: inner.y + body_h,
            width: inner.width,
            height: 1,
        };

        // Scene board sub-mode (`v`): the Planning book's scene + sequel
        // cards grouped by chapter, the selected one's spine expanded.
        if *scene_view {
            let mut lines: Vec<Line> = vec![Line::from(Span::styled(
                format!("SCENES / SEQUELS ({} card{})", scenes.len(), if scenes.len() == 1 { "" } else { "s" }),
                Style::default().add_modifier(Modifier::DIM),
            ))];
            let mut last_ch = String::new();
            let mk = |b: bool| if b { '●' } else { '○' };
            for (i, s) in scenes.iter().enumerate() {
                if s.chapter != last_ch {
                    last_ch = s.chapter.clone();
                    lines.push(Line::from(Span::styled(
                        if s.chapter.is_empty() { "(no chapter)".to_string() } else { s.chapter.clone() },
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                let slots = s.slots();
                let filled = [
                    !slots[0].1.trim().is_empty(),
                    !slots[1].1.trim().is_empty(),
                    !slots[2].1.trim().is_empty(),
                ];
                // scene weak = goal but no disaster; sequel weak = dilemma but no decision.
                let weak = if s.is_sequel() {
                    filled[1] && !filled[2]
                } else {
                    filled[0] && !filled[2]
                };
                let sel = i == *scene_cursor;
                let head = format!(
                    "{} [{:<6}] {:<24} {}{}{}{}",
                    if weak { '⚠' } else { '·' },
                    if s.is_sequel() { "sequel" } else { "scene" },
                    truncate_to(&s.title, 24),
                    mk(filled[0]),
                    mk(filled[1]),
                    mk(filled[2]),
                    if weak {
                        if s.is_sequel() { "  no decision" } else { "  no turn" }
                    } else {
                        ""
                    },
                );
                let color = if weak { Color::Yellow } else { Color::Green };
                let line = Line::from(Span::styled(head, Style::default().fg(color)));
                lines.push(if sel {
                    line.style(Style::default().fg(color).add_modifier(Modifier::REVERSED))
                } else {
                    line
                });
                if sel {
                    for (label, text) in slots {
                        if !text.trim().is_empty() {
                            lines.push(Line::from(Span::styled(
                                format!("     {label}: {}", text.trim()),
                                Style::default().add_modifier(Modifier::ITALIC),
                            )));
                        }
                    }
                }
            }
            f.render_widget(Paragraph::new(lines), body);
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    " ↑↓ · g regenerate · v/Esc back to beats ",
                    Style::default().add_modifier(Modifier::DIM),
                ))),
                footer,
            );
            return;
        }

        // Chapter-picker sub-mode (after `m`): list the chapters to map the
        // selected beat to.
        if let Some(pc) = picking {
            let beat = report.beats.get(*cursor).map(|b| b.beat.clone()).unwrap_or_default();
            let mut lines: Vec<Line> = vec![
                Line::from(Span::styled(
                    format!("Map “{beat}” to a chapter:"),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
            ];
            for (i, c) in report.chapters.iter().enumerate() {
                let row = format!(
                    "{} {:<30} {:>3.0}%",
                    if i == *pc { "▶" } else { " " },
                    c.slug,
                    c.position * 100.0
                );
                let line = Line::from(row);
                lines.push(if i == *pc {
                    line.style(Style::default().add_modifier(Modifier::REVERSED))
                } else {
                    line
                });
            }
            f.render_widget(Paragraph::new(lines), body);
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    " ↑↓ pick · Enter map · Esc cancel ",
                    Style::default().add_modifier(Modifier::DIM),
                ))),
                footer,
            );
            return;
        }

        // Thread-link sub-mode (after `t`): toggle the cursor beat's threads.
        if let Some(tc) = thread_pick {
            let beat = report.beats.get(*cursor);
            let current: &[String] = beat.map(|b| b.threads.as_slice()).unwrap_or(&[]);
            let name = beat.map(|b| b.beat.clone()).unwrap_or_default();
            let mut lines: Vec<Line> = vec![
                Line::from(Span::styled(
                    format!("Link threads to “{name}”:"),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
            ];
            for (i, t) in report.available_threads.iter().enumerate() {
                let on = current.iter().any(|c| c == t);
                let row = format!(
                    "{} [{}] {}",
                    if i == *tc { "▶" } else { " " },
                    if on { "x" } else { " " },
                    t
                );
                let line = Line::from(row);
                lines.push(if i == *tc {
                    line.style(Style::default().add_modifier(Modifier::REVERSED))
                } else if on {
                    line.style(Style::default().fg(Color::Green))
                } else {
                    line
                });
            }
            f.render_widget(Paragraph::new(lines), body);
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    " ↑↓ pick · Space toggle · Enter/Esc done ",
                    Style::default().add_modifier(Modifier::DIM),
                ))),
                footer,
            );
            return;
        }

        // Position bar: baseline ·, target |, actual ● (# when they overlap).
        let pos_bar = |target: f32, actual: Option<f32>| -> String {
            let mut cells = vec!['·'; bar_w];
            let ti = ((target * bar_w as f32) as usize).min(bar_w - 1);
            cells[ti] = '|';
            if let Some(a) = actual {
                let ai = ((a * bar_w as f32) as usize).min(bar_w - 1);
                cells[ai] = if ai == ti { '#' } else { '●' };
            }
            cells.into_iter().collect()
        };

        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(Span::styled("BEATS", Style::default().add_modifier(Modifier::DIM))));
        for (i, b) in report.beats.iter().enumerate() {
            let sel = i == *cursor;
            let (icon, color, info) = match (b.actual_position, b.drift) {
                (Some(a), Some(d)) => {
                    let warn = d.abs() > drift;
                    (
                        if warn { '⚠' } else { '✓' },
                        if warn { Color::Yellow } else { Color::Green },
                        format!("a{:>3.0}% {:+.0}%", a * 100.0, d * 100.0),
                    )
                }
                _ => ('✗', Color::Red, "gap".to_string()),
            };
            let threads = if b.threads.is_empty() {
                String::new()
            } else if b.unknown_threads.is_empty() {
                format!(" ↪{}", b.threads.len())
            } else {
                format!(" ↪{}?", b.threads.len())
            };
            let row = format!(
                "{icon} {:<22} {} t{:>3.0}% {}{threads}",
                truncate_to(&b.beat, 22),
                pos_bar(b.target_position, b.actual_position),
                b.target_position * 100.0,
                info,
            );
            let line = Line::from(Span::styled(row, Style::default().fg(color)));
            lines.push(if sel {
                line.style(Style::default().fg(color).add_modifier(Modifier::REVERSED))
            } else {
                line
            });
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "PACING (act word-share)",
            Style::default().add_modifier(Modifier::DIM),
        )));
        for p in &report.acts {
            let (actual, flag, color) = match p.actual {
                Some(a) => {
                    let dev = a - p.expected;
                    if dev.abs() > drift {
                        (
                            format!("{:.0}%", a * 100.0),
                            if dev > 0.0 { " ⚠ long" } else { " ⚠ short" },
                            Color::Yellow,
                        )
                    } else {
                        (format!("{:.0}%", a * 100.0), "", Color::Green)
                    }
                }
                None => ("?".to_string(), " (map the act boundary)", Color::DarkGray),
            };
            lines.push(Line::from(Span::styled(
                format!(
                    "  Act {}  expected {:>3.0}%  actual {:>4}{flag}",
                    p.act,
                    p.expected * 100.0,
                    actual
                ),
                Style::default().fg(color),
            )));
        }

        // TENSION overlay (1.3.4 P1): the framework's expected intensity vs
        // the actual open-obligation density, as block-ramp sparklines
        // aligned under the position bars above (so a beat's `●` sits over
        // its actual-tension cell).
        if let Some(t) = &report.tension {
            // Expected control points: (target position, expected) per beat
            // — the framework's intended shape, independent of mapping.
            let mut exp_pts: Vec<(f32, f32)> = report
                .beats
                .iter()
                .zip(&t.points)
                .map(|(b, p)| (b.target_position, p.expected))
                .collect();
            exp_pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "TENSION (intensity · aligned to the bars above)",
                Style::default().add_modifier(Modifier::DIM),
            )));
            lines.push(Line::from(Span::styled(
                format!("{:<25}{}", "  expected", crate::planning::intensity_sparkline(&exp_pts, bar_w)),
                Style::default().fg(Color::Cyan),
            )));
            if t.has_actual {
                lines.push(Line::from(Span::styled(
                    format!("{:<25}{}", "  actual", crate::planning::intensity_sparkline(&t.series, bar_w)),
                    Style::default().fg(Color::Green),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    "  actual: run `inkhaven tension scan` (or link threads) to chart it".to_string(),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            // The AI second opinion (1.3.5 P3), once `plan tension rate` ran.
            if t.has_ai {
                lines.push(Line::from(Span::styled(
                    format!("{:<25}{}", "  ai", crate::planning::intensity_sparkline(&t.ai_series, bar_w)),
                    Style::default().fg(Color::Magenta),
                )));
            }
            // The selected beat's tension numerals + flat flag.
            if let Some(p) = t.points.get(*cursor) {
                if let Some(a) = p.actual {
                    let flat = p.gap.map(|g| p.expected >= 0.5 && g > 0.25).unwrap_or(false);
                    let ai = p.ai.map(|v| format!(" · ai {:.0}%", v * 100.0)).unwrap_or_default();
                    lines.push(Line::from(Span::styled(
                        format!(
                            "  ~ {}: actual {:.0}% vs expected {:.0}%{ai}{}",
                            truncate_to(&p.beat, 20),
                            a * 100.0,
                            p.expected * 100.0,
                            if flat { "  ⚠ flat" } else { "" }
                        ),
                        Style::default().fg(if flat { Color::Yellow } else { Color::Green }),
                    )));
                }
            }
        }

        // The selected beat's intention (filled by `plan scaffold`).
        if let Some(b) = report.beats.get(*cursor) {
            if !b.notes.trim().is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    format!("↳ {}", b.notes.trim()),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::ITALIC),
                )));
            }
        }

        f.render_widget(Paragraph::new(lines), body);
        let keys = "↑↓ · m map · t threads · s status · v scenes · a analyze · ⏎ open · Esc";
        let summary = if report.warnings.is_empty() {
            format!(" ✓ no findings · {keys} ")
        } else {
            format!(" {} finding(s) · {keys} ", report.warnings.len())
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                summary,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer,
        );
    }

    /// 1.3.6 EDITORIAL-1 P1 — the Editorial Pass cockpit: the ranked
    /// revision worklist (`inkhaven edit`), errors first, with a category
    /// filter and jump-to-location.
    pub(in crate::tui::app) fn draw_editorial_pass_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        use crate::editorial::Severity;
        use ratatui::style::Color;
        let Modal::EditorialPass { findings, cursor, filter, .. } = &self.modal else {
            return;
        };
        let keep =
            |fnd: &&crate::editorial::EditorialFinding| filter.as_deref().is_none_or(|c| fnd.category == c);
        let shown: Vec<&crate::editorial::EditorialFinding> = findings.iter().filter(keep).collect();
        let (mut ne, mut nw, mut ni) = (0usize, 0usize, 0usize);
        for fnd in &shown {
            match fnd.severity {
                Severity::Error => ne += 1,
                Severity::Warn => nw += 1,
                Severity::Info => ni += 1,
            }
        }

        let width = area.width.saturating_sub(6).clamp(60, 112);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let filt = filter.as_deref().map(|c| format!(" · {c}")).unwrap_or_default();
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(
                " Editorial Pass · {} finding(s){filt} · {ne}✗ {nw}⚠ {ni}· ",
                shown.len()
            ))
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let list_h = inner.height.saturating_sub(2).max(1) as usize; // hint + footer
        let hint_rect = Rect { x: inner.x, y: inner.y + inner.height - 2, width: inner.width, height: 1 };
        let footer_rect = Rect { x: inner.x, y: inner.y + inner.height - 1, width: inner.width, height: 1 };
        let body_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: list_h as u16 };

        let cur = (*cursor).min(shown.len().saturating_sub(1));
        // Bottom-anchored window that keeps the cursor visible.
        let start = if cur >= list_h { cur + 1 - list_h } else { 0 };
        let msg_w = inner.width.saturating_sub(30) as usize;
        let mut lines: Vec<Line> = Vec::new();
        if shown.is_empty() {
            lines.push(Line::from(Span::styled(
                "  ✓ no findings in this filter",
                Style::default().fg(Color::Green),
            )));
        }
        for (i, fnd) in shown.iter().enumerate().skip(start).take(list_h) {
            let color = match fnd.severity {
                Severity::Error => Color::Red,
                Severity::Warn => Color::Yellow,
                Severity::Info => Color::DarkGray,
            };
            // ✎ = AI-rewritable (press f); → = jumpable only.
            let mark = if fnd.rewritable() {
                '✎'
            } else if fnd.location.paragraph.is_some() {
                '→'
            } else {
                ' '
            };
            let row = format!(
                "{} {} {:<10} {:<12} {}",
                fnd.severity.icon(),
                mark,
                truncate_to(&fnd.category, 10),
                truncate_to(&fnd.location.label(), 12),
                truncate_to(&fnd.message, msg_w),
            );
            let line = Line::from(Span::styled(row, Style::default().fg(color)));
            lines.push(if i == cur {
                line.style(Style::default().fg(color).add_modifier(Modifier::REVERSED))
            } else {
                line
            });
        }
        f.render_widget(Paragraph::new(lines), body_rect);

        // The selected finding's full message + hint.
        let hint = shown.get(cur).map(|fnd| {
            let mut s = fnd.message.clone();
            if let Some(h) = &fnd.hint {
                s.push_str(" — ");
                s.push_str(h);
            }
            s
        });
        if let Some(h) = hint {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!("↳ {}", truncate_to(&h, inner.width.saturating_sub(2) as usize)),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::ITALIC),
                ))),
                hint_rect,
            );
        }
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " ↑↓ · [ ] filter · ⏎ jump · ✎ f fix · F fix-all · s skip · d defer · Esc ",
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// 1.3.8 WORLD-1 P2 — the story-bible view: the world consolidated
    /// (characters + continuity attributes, places, artefacts, facts).
    pub(in crate::tui::app) fn draw_story_bible_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        use crate::tui::modal::BibleRowKind;
        use ratatui::style::Color;
        let Modal::StoryBible { rows, cursor } = &self.modal else {
            return;
        };

        let width = area.width.saturating_sub(6).clamp(48, 92);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Story bible ")
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let list_h = inner.height.saturating_sub(1).max(1) as usize;
        let body_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: list_h as u16 };
        let footer_rect = Rect { x: inner.x, y: inner.y + inner.height - 1, width: inner.width, height: 1 };

        let cur = (*cursor).min(rows.len().saturating_sub(1));
        let start = if cur >= list_h { cur + 1 - list_h } else { 0 };
        let mut lines: Vec<Line> = Vec::new();
        for (i, r) in rows.iter().enumerate().skip(start).take(list_h) {
            let line = match r.kind {
                BibleRowKind::Header => Line::from(Span::styled(
                    r.text.clone(),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                )),
                BibleRowKind::Entry => Line::from(Span::styled(
                    format!("  {} {}", if r.jump.is_some() { '→' } else { ' ' }, truncate_to(&r.text, 70)),
                    Style::default(),
                )),
                BibleRowKind::Attr => Line::from(Span::styled(
                    format!("      {}", truncate_to(&r.text, 70)),
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                )),
                BibleRowKind::Drift => Line::from(Span::styled(
                    format!("    {} {}", if r.jump.is_some() { '→' } else { ' ' }, truncate_to(&r.text, 72)),
                    Style::default().fg(Color::Rgb(0xeb, 0xa6, 0x72)),
                )),
            };
            lines.push(if i == cur {
                line.style(Style::default().add_modifier(Modifier::REVERSED))
            } else {
                line
            });
        }
        f.render_widget(Paragraph::new(lines), body_rect);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " ↑↓ · ⏎ jump to source · Esc ",
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// LANG-1 P2.7b — the ConLang hub overview (`Ctrl+B X`): a read-only,
    /// scrollable summary of every language (phonology / lexicon / prosody /
    /// speakers). Header rows are language names; the rest are stats.
    pub(in crate::tui::app) fn draw_conlang_hub_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        use ratatui::style::Color;
        let Modal::ConlangHub { rows, cursor } = &self.modal else {
            return;
        };

        let width = area.width.saturating_sub(6).clamp(48, 86);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" ConLang hub ")
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let list_h = inner.height.saturating_sub(1).max(1) as usize;
        let body_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: list_h as u16 };
        let footer_rect =
            Rect { x: inner.x, y: inner.y + inner.height - 1, width: inner.width, height: 1 };

        let cur = (*cursor).min(rows.len().saturating_sub(1));
        let start = if cur >= list_h { cur + 1 - list_h } else { 0 };
        let mut lines: Vec<Line> = Vec::new();
        for r in rows.iter().skip(start).take(list_h) {
            let line = if r.header {
                Line::from(Span::styled(
                    r.text.clone(),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(Span::styled(truncate_to(&r.text, 80), Style::default()))
            };
            lines.push(line);
        }
        f.render_widget(Paragraph::new(lines), body_rect);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " ↑↓ scroll · Esc ",
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// DIALOG-1 — the per-character dialogue fingerprint view (`Ctrl+V Shift+Q`):
    /// a read-only scrollable panel of pre-rendered metric-bar rows. The first
    /// non-blank line (the header) renders bold-cyan.
    pub(in crate::tui::app) fn draw_dialogue_fingerprint_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        use ratatui::style::Color;
        let Modal::DialogueFingerprint { rows, cursor } = &self.modal else {
            return;
        };

        let width = area.width.saturating_sub(6).clamp(52, 92);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Dialogue fingerprint ")
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let list_h = inner.height.saturating_sub(1).max(1) as usize;
        let body_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: list_h as u16 };
        let footer_rect =
            Rect { x: inner.x, y: inner.y + inner.height - 1, width: inner.width, height: 1 };

        let cur = (*cursor).min(rows.len().saturating_sub(1));
        let start = if cur >= list_h { cur + 1 - list_h } else { 0 };
        let mut lines: Vec<Line> = Vec::new();
        for (i, r) in rows.iter().enumerate().skip(start).take(list_h) {
            let is_header = i == 0 || (!r.is_empty() && r.contains("Dialogue fingerprint"));
            let line = if is_header {
                Line::from(Span::styled(
                    r.clone(),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(Span::styled(truncate_to(r, 90), Style::default()))
            };
            lines.push(line);
        }
        f.render_widget(Paragraph::new(lines), body_rect);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " ↑↓ scroll · Esc ",
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// CHAR-1 — the per-character arc view (`Ctrl+V Shift+N`): a read-only
    /// scrollable panel of the cached arc (declaration · state chain · agency ·
    /// checks · planning gaps). Section headers (column-0, non-blank) render
    /// bold-cyan; indented lines are detail.
    pub(in crate::tui::app) fn draw_character_arc_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        use ratatui::style::Color;
        let Modal::CharacterArc { rows, cursor } = &self.modal else {
            return;
        };

        let width = area.width.saturating_sub(6).clamp(52, 92);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Character arc ")
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let list_h = inner.height.saturating_sub(1).max(1) as usize;
        let body_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: list_h as u16 };
        let footer_rect =
            Rect { x: inner.x, y: inner.y + inner.height - 1, width: inner.width, height: 1 };

        let cur = (*cursor).min(rows.len().saturating_sub(1));
        let start = if cur >= list_h { cur + 1 - list_h } else { 0 };
        let mut lines: Vec<Line> = Vec::new();
        for (i, r) in rows.iter().enumerate().skip(start).take(list_h) {
            // Header = the title line, or a non-blank line starting at column 0.
            let is_header = i == 0 || (!r.is_empty() && !r.starts_with(' '));
            let line = if is_header {
                Line::from(Span::styled(
                    r.clone(),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(Span::styled(truncate_to(r, 90), Style::default()))
            };
            lines.push(line);
        }
        f.render_widget(Paragraph::new(lines), body_rect);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " ↑↓ scroll · Esc ",
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// SEMNET — the knowledge-graph neighbourhood view (`Ctrl+V g`): the open
    /// paragraph's one-hop edges as a read-only scrollable tree. The focus line
    /// (`◆`) and per-kind group headers (`├─`) are bold-cyan; the `│`-prefixed
    /// detail rows are plain.
    pub(in crate::tui::app) fn draw_graph_neighbourhood_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        use ratatui::style::Color;
        let Modal::GraphNeighbourhood { rows, cursor } = &self.modal else {
            return;
        };

        let width = area.width.saturating_sub(6).clamp(52, 92);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Graph neighbourhood ")
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let list_h = inner.height.saturating_sub(1).max(1) as usize;
        let body_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: list_h as u16 };
        let footer_rect =
            Rect { x: inner.x, y: inner.y + inner.height - 1, width: inner.width, height: 1 };

        let cur = (*cursor).min(rows.len().saturating_sub(1));
        let start = if cur >= list_h { cur + 1 - list_h } else { 0 };
        let mut lines: Vec<Line> = Vec::new();
        for r in rows.iter().skip(start).take(list_h) {
            // The focus (◆) and group headers (├─) are headers; │-rows are detail.
            let is_header = r.starts_with('◆') || r.starts_with('├');
            let line = if is_header {
                Line::from(Span::styled(
                    truncate_to(r, 90),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(Span::styled(truncate_to(r, 90), Style::default()))
            };
            lines.push(line);
        }
        f.render_widget(Paragraph::new(lines), body_rect);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " ↑↓ scroll · Esc ",
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// GRAPHMIND — the graph hub (`Ctrl+B z`): a tiny menu onto the graph.
    pub(in crate::tui::app) fn draw_graph_hub_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        use ratatui::style::Color;
        if !matches!(self.modal, Modal::GraphHub) {
            return;
        }
        let width = area.width.saturating_sub(6).clamp(40, 60);
        let height = 6u16.min(area.height);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Graph hub ")
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);
        let key = |k: &str| Span::styled(k.to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        let lines = vec![
            Line::from(vec![key(" n "), Span::raw(" neighbourhood — the open paragraph's edges")]),
            Line::from(vec![key(" i "), Span::raw(" inbox — advisory edges awaiting triage")]),
            Line::from(Span::styled("  Esc to close", Style::default().add_modifier(Modifier::DIM))),
        ];
        f.render_widget(Paragraph::new(lines), inner);
    }

    /// GRAPHMIND — the edge inbox (graph hub → `i`): the advisory `Judged` edges,
    /// the cursor row highlighted, `P` promote / `d` reject.
    pub(in crate::tui::app) fn draw_graph_inbox_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::GraphEdgeInbox { rows, cursor } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(6).clamp(52, 100);
        let height = area.height.saturating_sub(4).max(10);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Edge inbox ({}) ", rows.len()))
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let list_h = inner.height.saturating_sub(1).max(1) as usize;
        let body_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: list_h as u16 };
        let footer_rect =
            Rect { x: inner.x, y: inner.y + inner.height - 1, width: inner.width, height: 1 };

        let cur = (*cursor).min(rows.len().saturating_sub(1));
        let start = if cur >= list_h { cur + 1 - list_h } else { 0 };
        let mut lines: Vec<Line> = Vec::new();
        for (i, (_, text)) in rows.iter().enumerate().skip(start).take(list_h) {
            let style = if i == cur {
                Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(truncate_to(text, 98), style)));
        }
        f.render_widget(Paragraph::new(lines), body_rect);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " ↑↓ · P keep · d reject · Esc ",
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// WORLD-4 — the World overview (`Ctrl+B W`): a read-only scrollable
    /// summary of the world definition, compiled astronomy, and materialization
    /// status. Lines that begin at column 0 (and aren't blank) are section
    /// headers, rendered bold-cyan; indented lines are detail.
    pub(in crate::tui::app) fn draw_world_overview_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        use ratatui::style::Color;
        let Modal::WorldOverview { rows, cursor } = &self.modal else {
            return;
        };

        let width = area.width.saturating_sub(6).clamp(48, 86);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" World overview ")
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let list_h = inner.height.saturating_sub(1).max(1) as usize;
        let body_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: list_h as u16 };
        let footer_rect =
            Rect { x: inner.x, y: inner.y + inner.height - 1, width: inner.width, height: 1 };

        let cur = (*cursor).min(rows.len().saturating_sub(1));
        let start = if cur >= list_h { cur + 1 - list_h } else { 0 };
        let mut lines: Vec<Line> = Vec::new();
        for r in rows.iter().skip(start).take(list_h) {
            let is_header = !r.is_empty() && !r.starts_with(' ');
            let line = if is_header {
                Line::from(Span::styled(
                    r.clone(),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(Span::styled(truncate_to(r, 80), Style::default()))
            };
            lines.push(line);
        }
        f.render_widget(Paragraph::new(lines), body_rect);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " ↑↓ · C compile · P proposals · F fact-check ¶ · Esc ",
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// OUTLINE-1 — the full-screen manuscript Outline pane (`Ctrl+2` /
    /// `Ctrl+B Shift+O`). The whole book as a foldable tree over the live
    /// hierarchy: depth indent, a fold marker (`▾`/`▸`) on branches, a kind
    /// glyph on leaves, the title, and the paragraph status letter. The cursor
    /// row is reversed. Unlike the centered overview modals this fills the
    /// screen — it's a primary navigation surface, not a small overlay.
    pub(in crate::tui::app) fn draw_outline_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        use crate::store::node::NodeKind;
        use super::super::super::status_helpers::{display_status, status_letter};
        let Some(state) = self.outline_state.as_ref() else {
            return;
        };

        f.render_widget(ratatui::widgets::Clear, area);
        // Title reflects an active `/` filter.
        let title = if state.filter_str.trim().is_empty() {
            " Outline ".to_string()
        } else {
            format!(" Outline · /{} ", state.filter_str)
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(area);
        f.render_widget(block, area);

        let list_h = inner.height.saturating_sub(1).max(1) as usize;
        // Carve a right-hand detail panel when the pane is wide enough.
        let detail_w: u16 = if inner.width >= 80 {
            (inner.width / 3).clamp(28, 44)
        } else {
            0
        };
        let list_w = inner.width - detail_w;
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: list_w,
            height: list_h as u16,
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height - 1,
            width: inner.width,
            height: 1,
        };

        let rows = state.visible_rows(&self.hierarchy);
        if rows.is_empty() {
            let msg = if state.filter_str.trim().is_empty() {
                "  manuscript is empty — add a Book in the Tree pane first"
            } else {
                "  no matches — Esc clears the filter"
            };
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    msg,
                    Style::default().add_modifier(Modifier::DIM),
                ))),
                body_rect,
            );
            return;
        }
        let cur = state.cursor_index(&rows);
        let start = if cur >= list_h { cur + 1 - list_h } else { 0 };
        let width = list_w as usize;

        // Right-hand detail panel for the cursor node (O-P5).
        if detail_w > 0 {
            let detail_rect = Rect {
                x: inner.x + list_w,
                y: inner.y,
                width: detail_w,
                height: list_h as u16,
            };
            let detail = self.outline_detail_lines(rows[cur].id, detail_w as usize);
            f.render_widget(
                Paragraph::new(detail).block(
                    Block::default()
                        .borders(Borders::LEFT)
                        .border_style(Style::default().fg(self.theme.modal_border)),
                ),
                detail_rect,
            );
        }

        let mut lines: Vec<Line> = Vec::new();
        for (i, r) in rows.iter().enumerate().skip(start).take(list_h) {
            let Some(node) = self.hierarchy.get(r.id) else {
                continue;
            };
            let has_children = self.hierarchy.has_children(r.id);
            let marker = if has_children {
                if state.is_expanded(&r.id) { "▾ " } else { "▸ " }
            } else {
                match node.kind {
                    NodeKind::Paragraph => {
                        if node.event.is_some() {
                            "◆ "
                        } else {
                            match node.content_type.as_deref() {
                                Some("hjson") => "❴ ",
                                Some("jinja") => "⟡ ",
                                // POEM-TUI (PO-P14) — verse subtypes carry their
                                // own glyph (♩ ‖ ⁛ …), so a poem's shape shows in
                                // the outline; falls through to prose ¶ otherwise.
                                _ => super::super::structural_glyph(node)
                                    .or_else(|| crate::poetry::verse_glyph(node))
                                    .unwrap_or("¶ "),
                            }
                        }
                    }
                    NodeKind::Image => "▣ ",
                    NodeKind::Script => "λ ",
                    _ => "  ",
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
            if i == cur {
                row_style = row_style.add_modifier(Modifier::REVERSED);
            }

            let indent = "  ".repeat(r.depth);
            // Status letter for paragraphs (matches the Tree pane's badge).
            let status = if matches!(node.kind, NodeKind::Paragraph) {
                let label = display_status(node.status.as_deref());
                if label == "None" {
                    String::new()
                } else {
                    format!("[{}] ", status_letter(label))
                }
            } else {
                String::new()
            };
            // POEM-TUI (PO-P14) — a completion chip on verse rows: `♩8/14`, or a
            // ✓ when a bounded form is complete. Cheap enough here (the Outline is
            // an on-demand pane, only visible rows, redrawn on events) though not
            // in the per-frame Tree. Reserve its width from the title budget so it
            // never gets clipped.
            let chip = if matches!(node.kind, NodeKind::Paragraph) {
                self.poem_completion_chip(r.id)
            } else {
                None
            };
            let chip_str = chip
                .as_ref()
                .map(|(t, done)| format!("  {t}{}", if *done { " ✓" } else { "" }))
                .unwrap_or_default();
            let prefix = format!("{indent}{marker}{status}");
            let title_budget = width
                .saturating_sub(prefix.chars().count())
                .saturating_sub(chip_str.chars().count())
                .max(1);
            let title = truncate_to(&node.title, title_budget);
            let mut spans = vec![Span::styled(format!("{prefix}{title}"), row_style)];
            if let Some((t, done)) = chip {
                let chip_style = if done {
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                } else {
                    row_style.add_modifier(Modifier::DIM)
                };
                spans.push(Span::styled(
                    format!("  {t}{}", if done { " ✓" } else { "" }),
                    chip_style,
                ));
            }
            lines.push(Line::from(spans));
        }
        f.render_widget(Paragraph::new(lines), body_rect);

        let total = rows.len();
        let held = match self.para_clipboard.as_ref() {
            Some(c) => {
                let verb = match c.mode {
                    super::super::super::outline::ClipMode::Copy => "copy",
                    super::super::super::outline::ClipMode::Move => "move",
                };
                let slug = self
                    .hierarchy
                    .get(c.id)
                    .map(|n| n.slug.as_str())
                    .unwrap_or("?");
                format!(" · [{verb}: {slug}]")
            }
            None => String::new(),
        };
        let footer_line = if self.outline_editing_filter {
            Line::from(vec![
                Span::styled(
                    " filter: ",
                    Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("{}\u{2588}", state.filter_str), Style::default()),
                Span::styled(
                    "  · Enter apply · Esc exit ",
                    Style::default().add_modifier(Modifier::DIM),
                ),
            ])
        } else {
            Line::from(Span::styled(
                format!(
                    " {}/{} · jk move · l/h fold · JK reorder · </> promote · ymf copy/move/affix · / filter · Esc{held} ",
                    cur + 1,
                    total
                ),
                Style::default().add_modifier(Modifier::DIM),
            ))
        };
        f.render_widget(Paragraph::new(footer_line), footer_rect);
    }

    /// OUTLINE-1 (O-P5) — the right-hand detail panel lines for the cursor
    /// node: title, kind + breadcrumb, status, word count (vs target), tags,
    /// and last-modified date. Read-only; mirrors the data the Tree pane pips
    /// expose, gathered in one place.
    fn outline_detail_lines(&self, id: uuid::Uuid, width: usize) -> Vec<Line<'_>> {
        use crate::store::node::NodeKind;
        use super::super::super::status_helpers::display_status;
        let inner_w = width.saturating_sub(2).max(8); // minus the LEFT border + pad
        let Some(node) = self.hierarchy.get(id) else {
            return vec![Line::from("")];
        };
        let label = |s: &str| Span::styled(
            format!(" {s}: "),
            Style::default().fg(self.theme.tree_subchapter_fg).add_modifier(Modifier::DIM),
        );
        let mut out: Vec<Line> = Vec::new();
        out.push(Line::from(Span::styled(
            format!(" {}", truncate_to(&node.title, inner_w)),
            Style::default().add_modifier(Modifier::BOLD),
        )));
        out.push(Line::from(Span::styled(
            format!(" {}", node.kind.as_str()),
            Style::default().fg(self.theme.tree_chapter_fg),
        )));
        // Breadcrumb of ancestor slugs.
        let crumb: Vec<String> = self
            .hierarchy
            .ancestors(node)
            .into_iter()
            .map(|a| a.slug.clone())
            .collect();
        if !crumb.is_empty() {
            out.push(Line::from(vec![
                label("in"),
                Span::raw(truncate_to(&crumb.join(" › "), inner_w.saturating_sub(5))),
            ]));
        }
        if matches!(node.kind, NodeKind::Paragraph) {
            out.push(Line::from(vec![
                label("status"),
                Span::raw(display_status(node.status.as_deref()).to_string()),
            ]));
            let words = match node.target_words.filter(|t| *t > 0) {
                Some(t) => format!("{} / {}", node.word_count, t),
                None => node.word_count.to_string(),
            };
            out.push(Line::from(vec![label("words"), Span::raw(words)]));
            // POEM-TUI (PO-P14) — for a verse paragraph with a declared form,
            // show its completion (line ratio + state) and any structural issues,
            // the same reckoning `poetry status` prints on the CLI.
            if crate::poetry::is_verse_paragraph(node) {
                if let Some(form) = self.poem_form_for(id) {
                    // 1.8.23 — compute the status ONCE from the already-resolved
                    // form + one content read (was: poem_form_for 3× + get_content
                    // 2× per frame for the cursor node).
                    let st = self
                        .store
                        .get_content(id)
                        .ok()
                        .flatten()
                        .map(|b| crate::poetry::form_check::check_form(&String::from_utf8_lossy(&b), &form));
                    let ratio = st
                        .as_ref()
                        .map(|st| match st.expected_lines {
                            Some(exp) => {
                                let state = if st.complete { "complete" } else { "drafting" };
                                format!("{}/{} · {state}", st.lines_written, exp)
                            }
                            None => format!("{} lines (open form)", st.lines_written),
                        })
                        .unwrap_or_else(|| "—".into());
                    out.push(Line::from(vec![
                        label("poem"),
                        Span::raw(truncate_to(&form.form, inner_w.saturating_sub(7))),
                    ]));
                    out.push(Line::from(vec![label("lines"), Span::raw(ratio)]));
                    if let Some(st) = &st {
                        for issue in st.issues.iter().take(3) {
                            out.push(Line::from(Span::styled(
                                format!("  ⚠ {}", truncate_to(issue, inner_w.saturating_sub(4))),
                                Style::default().fg(self.theme.tree_chapter_fg).add_modifier(Modifier::DIM),
                            )));
                        }
                    }
                } else {
                    out.push(Line::from(vec![
                        label("poem"),
                        Span::styled(
                            "no form — Ctrl+B J → P → D".to_string(),
                            Style::default().add_modifier(Modifier::DIM),
                        ),
                    ]));
                }
            }
        } else {
            let kids = self.hierarchy.children_of(Some(node.id)).len();
            out.push(Line::from(vec![
                label("children"),
                Span::raw(kids.to_string()),
            ]));
        }
        if !node.tags.is_empty() {
            let tags: String = node
                .tags
                .iter()
                .map(|t| format!("#{t}"))
                .collect::<Vec<_>>()
                .join(" ");
            out.push(Line::from(vec![
                label("tags"),
                Span::raw(truncate_to(&tags, inner_w.saturating_sub(7))),
            ]));
        }
        out.push(Line::from(vec![
            label("modified"),
            Span::raw(node.modified_at.format("%Y-%m-%d").to_string()),
        ]));
        out
    }

    /// INNER_SOCRATES-1 — the `Ctrl+B J` overview (active persona, recent
    /// questions, the intent ledger). Same scrollable shape as the World overview.
    /// POEM-3 — the Inner Poet overview: a small hint box (F fast · E engage).
    pub(in crate::tui::app) fn draw_inner_poet_overview_modal(&self, f: &mut ratatui::Frame, area: Rect) {
        if !matches!(self.modal, Modal::InnerPoetOverview) {
            return;
        }
        let width = area.width.saturating_sub(8).clamp(40, 58);
        let height = 9u16.min(area.height.saturating_sub(2));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" ♪ Inner Poet ")
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let dim = Style::default().add_modifier(Modifier::DIM);
        let lines = vec![
            Line::from(""),
            Line::from("  F   fast-scan this stanza — metre + rhyme → Output"),
            Line::from(""),
            Line::from("  E   engage the AI — enjambment, sound, caesura,"),
            Line::from("      the volta → Thoughts pane"),
            Line::from(""),
            Line::from(Span::styled("  the Inner Poet observes; it never rewrites.  Esc closes.", dim)),
        ];
        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }

    /// CHORUS CH-P7b — the Inner Stylist overview menu (`Ctrl+B J → Y`).
    pub(in crate::tui::app) fn draw_inner_stylist_overview_modal(&self, f: &mut ratatui::Frame, area: Rect) {
        if !matches!(self.modal, Modal::InnerStylistOverview) {
            return;
        }
        let width = area.width.saturating_sub(8).clamp(44, 62);
        let height = 11u16.min(area.height.saturating_sub(2));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" ❝ Inner Stylist ")
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let dim = Style::default().add_modifier(Modifier::DIM);
        let lines = vec![
            Line::from(""),
            Line::from("  F   synthesise the book's voice → Output"),
            Line::from("      (distinctiveness · drift · POV · tense · register)"),
            Line::from(""),
            Line::from("  E   engage the AI coach → Thoughts pane"),
            Line::from(""),
            Line::from("  R   the voice report dashboard"),
            Line::from(""),
            Line::from(Span::styled("  the Inner Stylist observes; it never rewrites.  Esc closes.", dim)),
        ];
        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }

    /// CHORUS CH-P8 — the scrollable book-scale voice report dashboard.
    pub(in crate::tui::app) fn draw_style_report_modal(&self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::StyleReport { rows, cursor } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(6).clamp(52, 92);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Voice report ")
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let list_h = inner.height.saturating_sub(1).max(1) as usize;
        let cur = (*cursor).min(rows.len().saturating_sub(1));
        let start = if cur >= list_h { cur + 1 - list_h } else { 0 };
        let dim = Style::default().add_modifier(Modifier::DIM);
        let head = Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD);
        let mut lines: Vec<Line> = Vec::new();
        for r in rows.iter().skip(start).take(list_h) {
            // Section headers (no leading space) are bold; indented rows are body.
            let styled = if !r.starts_with(' ') && !r.is_empty() {
                Line::from(Span::styled(truncate_to(r, 90), head))
            } else {
                Line::from(Span::styled(truncate_to(r, 90), dim))
            };
            lines.push(styled);
        }
        let body_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: list_h as u16 };
        f.render_widget(Paragraph::new(lines), body_rect);

        let footer = Rect { x: inner.x, y: inner.y + inner.height - 1, width: inner.width, height: 1 };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(" ↑↓ scroll · Esc close", dim))),
            footer,
        );
    }

    /// SENTINEL-1 (CT-P6) — the scrollable continuity ledger dashboard. Like the
    /// voice report, but the cursor row is highlighted (Enter jumps to its
    /// paragraph when it has one).
    pub(in crate::tui::app) fn draw_continuity_ledger_modal(&self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::ContinuityLedger { rows, anchors, cursor } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(6).clamp(52, 92);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Continuity ledger ")
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let list_h = inner.height.saturating_sub(1).max(1) as usize;
        let cur = (*cursor).min(rows.len().saturating_sub(1));
        let start = if cur >= list_h { cur + 1 - list_h } else { 0 };
        let dim = Style::default().add_modifier(Modifier::DIM);
        let head = Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD);
        let sel = Style::default().bg(self.theme.modal_border).fg(self.theme.modal_bg).add_modifier(Modifier::BOLD);
        let mut lines: Vec<Line> = Vec::new();
        for (i, r) in rows.iter().enumerate().skip(start).take(list_h) {
            let is_cursor = i == cur && anchors.get(i).copied().flatten().is_some();
            let style = if is_cursor {
                sel
            } else if !r.starts_with(' ') && !r.is_empty() {
                head
            } else {
                dim
            };
            lines.push(Line::from(Span::styled(truncate_to(r, 90), style)));
        }
        let body_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: list_h as u16 };
        f.render_widget(Paragraph::new(lines), body_rect);

        let footer = Rect { x: inner.x, y: inner.y + inner.height - 1, width: inner.width, height: 1 };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " ↑↓ scroll · Enter jump · k coherence pass (LLM) · Esc close",
                dim,
            ))),
            footer,
        );
    }

    /// REDLINE-1 (RD-P3) — the guided-decision prompt: the finding + a text field
    /// where the author states the resolution the AI will apply.
    pub(in crate::tui::app) fn draw_revision_decision_modal(&self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::RevisionDecision { finding, input, .. } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(6).clamp(52, 88);
        let height = 12u16.min(area.height.saturating_sub(2));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Decision · {} ", finding.category))
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let dim = Style::default().add_modifier(Modifier::DIM);
        let head = Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD);
        let lines: Vec<Line> = vec![
            Line::from(Span::styled(truncate_to(&finding.message, 82), dim)),
            Line::from(""),
            Line::from(Span::styled("What's true / how should it be resolved?", head)),
            Line::from(""),
            Line::from(vec![
                Span::styled("› ", head),
                Span::raw(truncate_to(input, 78)),
                Span::styled("▏", head),
            ]),
        ];
        let body = Rect { x: inner.x, y: inner.y, width: inner.width, height: inner.height.saturating_sub(1) };
        f.render_widget(Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false }), body);

        let footer = Rect { x: inner.x, y: inner.y + inner.height - 1, width: inner.width, height: 1 };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " the AI applies YOUR decision · Enter reconcile · Esc cancel",
                dim,
            ))),
            footer,
        );
    }

    /// LECTOR-1 (LR-P5b) — the scrollable read-through dashboard.
    pub(in crate::tui::app) fn draw_read_through_modal(&self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::ReadThrough { rows, anchors, cursor } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(6).clamp(52, 92);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Read-through ")
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let list_h = inner.height.saturating_sub(1).max(1) as usize;
        let cur = (*cursor).min(rows.len().saturating_sub(1));
        let start = if cur >= list_h { cur + 1 - list_h } else { 0 };
        let dim = Style::default().add_modifier(Modifier::DIM);
        let head = Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD);
        let sel = Style::default().bg(self.theme.modal_border).fg(self.theme.modal_bg).add_modifier(Modifier::BOLD);
        let mut lines: Vec<Line> = Vec::new();
        for (i, r) in rows.iter().enumerate().skip(start).take(list_h) {
            let is_cursor = i == cur && anchors.get(i).copied().flatten().is_some();
            let style = if is_cursor {
                sel
            } else if !r.starts_with(' ') && !r.is_empty() {
                head
            } else {
                dim
            };
            lines.push(Line::from(Span::styled(truncate_to(r, 90), style)));
        }
        let body_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: list_h as u16 };
        f.render_widget(Paragraph::new(lines), body_rect);

        let footer = Rect { x: inner.x, y: inner.y + inner.height - 1, width: inner.width, height: 1 };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " ↑↓ scroll · Enter jump · k deep read (LLM) · Esc close",
                dim,
            ))),
            footer,
        );
    }

    pub(in crate::tui::app) fn draw_inner_socrates_overview_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        use ratatui::style::Color;
        let Modal::InnerSocratesOverview { rows, cursor } = &self.modal else {
            return;
        };

        let width = area.width.saturating_sub(6).clamp(48, 86);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Inner Socrates ")
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        // Reserve two rows for the footer so the (long) sub-key legend — now
        // including T theologian — wraps instead of clipping at the right edge.
        let footer_h: u16 = if inner.height > 3 { 2 } else { 1 };
        let list_h = inner.height.saturating_sub(footer_h).max(1) as usize;
        let body_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: list_h as u16 };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height - footer_h,
            width: inner.width,
            height: footer_h,
        };

        let cur = (*cursor).min(rows.len().saturating_sub(1));
        let start = if cur >= list_h { cur + 1 - list_h } else { 0 };
        let mut lines: Vec<Line> = Vec::new();
        for r in rows.iter().skip(start).take(list_h) {
            let is_header = !r.is_empty() && !r.starts_with(' ');
            let line = if is_header {
                Line::from(Span::styled(
                    r.clone(),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(Span::styled(truncate_to(r, 80), Style::default()))
            };
            lines.push(line);
        }
        f.render_widget(Paragraph::new(lines), body_rect);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " ↑↓ · F check · E engage (slow) · T theologian · C converse · N persona · S cycle · L ledger · A auto · Esc ",
                Style::default().add_modifier(Modifier::DIM),
            )))
            .wrap(Wrap { trim: true }),
            footer_rect,
        );
    }

    pub(in crate::tui::app) fn draw_inner_editor_overview_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        use ratatui::style::Color;
        let Modal::InnerEditorOverview { rows, cursor } = &self.modal else {
            return;
        };

        let width = area.width.saturating_sub(6).clamp(48, 90);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Inner Editor ✎ ")
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let list_h = inner.height.saturating_sub(1).max(1) as usize;
        let body_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: list_h as u16 };
        let footer_rect =
            Rect { x: inner.x, y: inner.y + inner.height - 1, width: inner.width, height: 1 };

        let cur = (*cursor).min(rows.len().saturating_sub(1));
        let start = if cur >= list_h { cur + 1 - list_h } else { 0 };
        let mut lines: Vec<Line> = Vec::new();
        for r in rows.iter().skip(start).take(list_h) {
            let is_header = !r.is_empty() && !r.starts_with(' ');
            let line = if is_header {
                // Warm-earth header to match the Editor's Output palette.
                Line::from(Span::styled(
                    r.clone(),
                    Style::default().fg(Color::Rgb(188, 110, 78)).add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(Span::styled(truncate_to(r, 84), Style::default()))
            };
            lines.push(line);
        }
        f.render_widget(Paragraph::new(lines), body_rect);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " ↑↓ · E engage ¶ · C converse · A ambient auto · F findings · Esc ",
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// WORLD-4 — the proposal queue (`Ctrl+B W` → `P`): the compiler's pending
    /// Place proposals. Two lines each (name + class, then rationale); the
    /// selected one is marked. `Enter` accepts, `r` rejects.
    pub(in crate::tui::app) fn draw_world_proposals_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        use ratatui::style::Color;
        let Modal::WorldProposals { proposals, cursor } = &self.modal else {
            return;
        };

        let width = area.width.saturating_sub(6).clamp(48, 86);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" World proposals · {} ", proposals.len()))
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let list_h = inner.height.saturating_sub(1).max(1) as usize;
        let body_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: list_h as u16 };
        let footer_rect =
            Rect { x: inner.x, y: inner.y + inner.height - 1, width: inner.width, height: 1 };

        let cur = (*cursor).min(proposals.len().saturating_sub(1));
        // Two display lines per proposal; scroll so the selected one stays in view.
        let start_idx = if cur >= list_h / 2 { cur + 1 - (list_h / 2).max(1) } else { 0 };
        let mut lines: Vec<Line> = Vec::new();
        for (i, p) in proposals.iter().enumerate().skip(start_idx) {
            if lines.len() >= list_h {
                break;
            }
            let sel = i == cur;
            let marker = if sel { "▌" } else { " " };
            let head_style = if sel {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };
            lines.push(Line::from(Span::styled(format!("{marker}{}", p.name), head_style)));
            if lines.len() < list_h {
                lines.push(Line::from(Span::styled(
                    truncate_to(&format!("   {}", p.rationale), 80),
                    Style::default().add_modifier(Modifier::DIM),
                )));
            }
        }
        f.render_widget(Paragraph::new(lines), body_rect);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " ↑↓ · ⏎ accept · r reject · Esc back ",
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// LANG-1 P2.7c — the `:lang:` inline-insertion picker: a filterable list
    /// of a language's dictionary words; `Enter` inserts the chosen word in
    /// place of the `:lang:` trigger.
    pub(in crate::tui::app) fn draw_lang_insert_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::LangInsert { language, entries, query, cursor, .. } = &self.modal else {
            return;
        };
        let q = query.to_lowercase();
        let filtered: Vec<usize> = if query.is_empty() {
            (0..entries.len()).collect()
        } else {
            entries
                .iter()
                .enumerate()
                .filter(|(_, (w, g))| w.to_lowercase().contains(&q) || g.to_lowercase().contains(&q))
                .map(|(i, _)| i)
                .collect()
        };

        let width = area.width.saturating_sub(6).clamp(40, 70);
        let height = area.height.saturating_sub(4).clamp(8, 20);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" :{language}: insert "))
            .border_style(Style::default().fg(self.theme.modal_border).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let query_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: 1 };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("/{query}"),
                Style::default().add_modifier(Modifier::BOLD),
            ))),
            query_rect,
        );

        let list_h = inner.height.saturating_sub(2).max(1) as usize;
        let body_rect = Rect { x: inner.x, y: inner.y + 1, width: inner.width, height: list_h as u16 };
        let footer_rect =
            Rect { x: inner.x, y: inner.y + inner.height - 1, width: inner.width, height: 1 };

        let cur = (*cursor).min(filtered.len().saturating_sub(1));
        let start = if cur >= list_h { cur + 1 - list_h } else { 0 };
        let cap = (inner.width as usize).saturating_sub(2);
        let mut lines: Vec<Line> = Vec::new();
        if filtered.is_empty() {
            lines.push(Line::from(Span::styled(
                "(no matches)",
                Style::default().add_modifier(Modifier::DIM),
            )));
        }
        for (vis, &ei) in filtered.iter().enumerate().skip(start).take(list_h) {
            let (w, g) = &entries[ei];
            let text = if g.is_empty() { w.clone() } else { format!("{w:<16} {g}") };
            let mut line = Line::from(Span::raw(truncate_to(&text, cap)));
            if vis == cur {
                line = line.style(Style::default().add_modifier(Modifier::REVERSED));
            }
            lines.push(line);
        }
        f.render_widget(Paragraph::new(lines), body_rect);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " type filter · ↑↓ · ⏎ insert · Esc ",
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// 1.2.22 R.3 — the project-replace review: matches grouped by
    /// paragraph, each with a `[x]`/`[ ]` keep/skip box and the matched
    /// span highlighted in its line.  Enter applies the kept ones.
    pub(in crate::tui::app) fn draw_replace_review_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::ReplaceReview {
            pattern,
            replacement,
            opts,
            matches,
            flat,
            cursor,
            skipped,
        } = &self.modal
        else {
            return;
        };
        let width = area.width.saturating_sub(6).max(64);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let kept = flat.len().saturating_sub(skipped.len());
        let header = format!(
            " Replace: {pattern} → {replacement}  [{}]  ({kept}/{} kept) ",
            crate::replace::opts_label(*opts),
            flat.len(),
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
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

        // Build every line (paragraph headers + hit rows), noting the
        // rendered index of the cursor row so we keep it in view.
        let mut lines: Vec<Line<'_>> = Vec::new();
        let mut cursor_render = 0usize;
        let mut flat_idx = 0usize;
        for (pi, pm) in matches.iter().enumerate() {
            lines.push(Line::from(Span::styled(
                pm.slug_path.clone(),
                Style::default().add_modifier(Modifier::DIM | Modifier::BOLD),
            )));
            for (hi, hit) in pm.hits.iter().enumerate() {
                let on_cursor = flat_idx == *cursor;
                if on_cursor {
                    cursor_render = lines.len();
                }
                let kept = !skipped.contains(&(pi, hi));
                let mut spans: Vec<Span> = vec![Span::raw(format!(
                    "  {} {:>3}:{:<3} ",
                    if kept { "[x]" } else { "[ ]" },
                    hit.line,
                    hit.col,
                ))];
                spans.extend(match_spans(hit));
                let mut line = Line::from(spans);
                if on_cursor {
                    line = line.style(Style::default().add_modifier(Modifier::REVERSED));
                } else if !kept {
                    line = line.style(Style::default().add_modifier(Modifier::DIM));
                }
                lines.push(line);
                flat_idx += 1;
            }
        }
        let view_scroll = if body_h > 0 && cursor_render >= body_h {
            cursor_render + 1 - body_h
        } else {
            0
        };
        let view: Vec<Line<'_>> = lines.into_iter().skip(view_scroll).take(body_h).collect();
        f.render_widget(Paragraph::new(view), body_rect);

        let hint =
            " ↑↓ move · Space skip · a/n keep/skip all · w whole-word · i case · x regex · Enter apply · Esc cancel ";
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_rect,
        );
    }

    /// 1.2.21+ FF.1 — the Facts semantic-search modal: a query box on
    /// top, the ranked matches below (with `[x]`/`[ ]` mark boxes once
    /// browsing), and a mode-aware footer.
    pub(in crate::tui::app) fn draw_facts_search_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let Modal::FactsSearch { input, entries, cursor, marked, browsing } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(8).max(60);
        let height = area.height.saturating_sub(4).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let header = format!(
            " Facts search{} ",
            if entries.is_empty() {
                String::new()
            } else {
                format!(" ({} match{})", entries.len(), if entries.len() == 1 { "" } else { "es" })
            },
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .title(header)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        // Row 0: the query box (cursor shown only while editing).
        let query_text = if *browsing {
            format!(" query: {}", input.as_str())
        } else {
            format!(" query: {}", input.render_with_cursor('▌'))
        };
        let query_rect = Rect { x: inner.x, y: inner.y, width: inner.width, height: 1 };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                query_text,
                Style::default().add_modifier(if *browsing {
                    Modifier::DIM
                } else {
                    Modifier::BOLD
                }),
            ))),
            query_rect,
        );

        // Body: the ranked matches; footer: hints.
        let body_h = inner.height.saturating_sub(2) as usize;
        let body_rect = Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: inner.height.saturating_sub(2),
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        // Keep the cursor in view without persisting scroll state.
        let view_scroll = if body_h > 0 && *cursor >= body_h {
            *cursor + 1 - body_h
        } else {
            0
        };

        let lines: Vec<Line<'_>> = entries
            .iter()
            .enumerate()
            .skip(view_scroll)
            .take(body_h)
            .map(|(i, e)| {
                let mark = if marked.contains(&e.id) { "[x]" } else { "[ ]" };
                let score_pct = (e.score * 100.0).round() as i64;
                let head = format!(" {mark} {:>3}%  {}", score_pct, e.title);
                let path_dim = format!("   {}", e.slug_path);
                let mut spans: Vec<Span> = vec![
                    Span::raw(head),
                    Span::raw("  "),
                    Span::styled(path_dim, Style::default().add_modifier(Modifier::DIM)),
                ];
                if !e.snippet.is_empty() {
                    spans.push(Span::raw("  · "));
                    spans.push(Span::styled(
                        e.snippet.clone(),
                        Style::default().add_modifier(Modifier::DIM),
                    ));
                }
                let mut line = Line::from(spans);
                if i == *cursor && *browsing {
                    line = line.style(Style::default().add_modifier(Modifier::REVERSED));
                }
                line
            })
            .collect();
        f.render_widget(Paragraph::new(lines), body_rect);

        let hint = if !*browsing {
            " type a query · Enter search · Esc close ".to_string()
        } else {
            format!(
                " ↑↓ move · Space mark · Enter send{} · type to refine · Esc close    ({}/{}) ",
                if marked.is_empty() {
                    String::new()
                } else {
                    format!(" ({} marked)", marked.len())
                },
                cursor + 1,
                entries.len().max(1),
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
            "   streak: {}d{} (grace {}/{} per week)",
            snap.streak.days,
            if snap.streak.best > snap.streak.days {
                format!(" · best {}d", snap.streak.best)
            } else {
                String::new()
            },
            snap.streak.grace_used,
            snap.streak.grace_per_week
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
        let hint = " ↑↓ / PgUp/PgDn scroll · r refresh · e edit goals · Esc close ";
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(hint, dim))),
            footer_rect,
        );
    }

    /// 1.3.35 — the in-app goals editor (opened with `e` from the
    /// progress modal). A compact field list; the selected field
    /// shows a caret and is highlighted. Commit writes the changed
    /// `goals.*` keys back to `inkhaven.hjson`.
    pub(in crate::tui::app) fn draw_goals_editor_modal(&mut self, f: &mut ratatui::Frame, area: Rect) {
        let (values, cursor) = match &self.modal {
            Modal::GoalsEditor { values, cursor, .. } => (values.clone(), *cursor),
            _ => return,
        };

        let w = area.width.saturating_sub(6).min(60);
        let h: u16 = 10;
        let x = area.x + (area.width.saturating_sub(w)) / 2;
        let y = area.y + (area.height.saturating_sub(h)) / 2;
        let rect = Rect { x, y, width: w, height: h };

        f.render_widget(ratatui::widgets::Clear, rect);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Writing goals — edit ")
            .border_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let dim = Style::default().add_modifier(Modifier::DIM);
        let sel = Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD);

        let mut lines: Vec<Line> = vec![Line::from("")];
        for (i, (_, label)) in Self::GOALS_EDITOR_FIELDS.iter().enumerate() {
            let value = if values[i].is_empty() { "0" } else { values[i].as_str() };
            let caret = if i == cursor { "▸ " } else { "  " };
            let style = if i == cursor { sel } else { Style::default() };
            lines.push(Line::from(vec![
                Span::raw(caret),
                Span::styled(format!(" {label:<26} ", ), style),
                Span::raw("  "),
                Span::styled(
                    format!("{value}{}", if i == cursor { "_" } else { "" }),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  0 = disabled. Writes to inkhaven.hjson (backup kept).",
            dim,
        )));
        lines.push(Line::from(Span::styled(
            "  ↑↓ field · digits edit · ⌫ delete · Enter save · Esc cancel",
            dim,
        )));

        f.render_widget(Paragraph::new(lines), inner);
    }

    /// 1.3.36 — the project-wide snapshot browser (Ctrl+F6). A roomy
    /// scrollable list of every snapshot across all paragraphs, one
    /// row each (timestamp · words · paragraph · annotation/preview),
    /// with a `/` filter and a cursor-following window.
    pub(in crate::tui::app) fn draw_snapshot_browser_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let (entries, cursor, filter, filter_focused) = match &self.modal {
            Modal::SnapshotBrowser {
                entries,
                cursor,
                filter,
                filter_focused,
                ..
            } => (entries.clone(), *cursor, filter.clone(), *filter_focused),
            _ => return,
        };

        let w = area.width.saturating_sub(6).min(110);
        let h = area.height.saturating_sub(4).min(34);
        let x = area.x + (area.width.saturating_sub(w)) / 2;
        let y = area.y + (area.height.saturating_sub(h)) / 2;
        let rect = Rect { x, y, width: w, height: h };

        f.render_widget(ratatui::widgets::Clear, rect);
        let visible = super::super::snapshot_impl::browser_visible_indices(&entries, &filter);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Snapshots — project ({}) ", entries.len()))
            .border_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let dim = Style::default().add_modifier(Modifier::DIM);

        // Header: filter row (+ match count) + a blank separator.
        let filter_text = if filter.is_empty() {
            "/ (filter by paragraph or annotation)".to_string()
        } else {
            format!("/ {filter}   ({} of {} match)", visible.len(), entries.len())
        };
        let filter_style = if filter_focused {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else if filter.is_empty() {
            dim
        } else {
            Style::default().fg(Color::Yellow)
        };
        let footer = if filter_focused {
            " filter mode: type to narrow · ⌫ edits · Enter/Esc exit filter "
        } else {
            " ↑↓ move · / filter · V diff vs current · Enter open paragraph · Esc close "
        };

        // Body window: one row per visible entry, cursor-following.
        let header_rows = 2u16;
        let footer_rows = 2u16;
        let body_h = inner.height.saturating_sub(header_rows + footer_rows).max(1) as usize;
        let start = if cursor >= body_h { cursor - body_h + 1 } else { 0 };

        let mut lines: Vec<Line> = Vec::with_capacity(body_h + 4);
        lines.push(Line::from(Span::styled(filter_text, filter_style)));
        lines.push(Line::from(""));
        if visible.is_empty() {
            lines.push(Line::from(Span::styled("  (nothing matches the filter)", dim)));
        }
        for (vis_i, &abs_i) in visible.iter().enumerate().skip(start).take(body_h) {
            let (title, snap) = &entries[abs_i];
            let selected = vis_i == cursor;
            let ts = snap
                .created_at
                .with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M");
            let note = if !snap.annotation.trim().is_empty() {
                format!("✎ {}", snap.annotation)
            } else if !snap.preview.is_empty() {
                snap.preview.clone()
            } else {
                "(no body)".to_string()
            };
            let row = format!(
                " {ts}  {:>6}w  [{}]  {}",
                snap.word_count,
                truncate_to(title, 24),
                truncate_to(&note, 40),
            );
            let style = if selected {
                Style::default()
                    .add_modifier(Modifier::REVERSED | Modifier::BOLD)
                    .fg(Color::Cyan)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(row, style)));
        }
        // Pad so the footer sits at the bottom edge.
        while lines.len() < (header_rows as usize + body_h) {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(footer, dim)));

        f.render_widget(Paragraph::new(lines), inner);
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

    /// 1.2.9+ — project-wide concordance modal painter
    /// (Ctrl+B Shift+L).  Three-region layout: header
    /// (stats + filter input + sort label), main list
    /// (rank · headword · count · variants), footer
    /// (KWIC samples for the selected row + key hints).
    /// Cursor + scroll clamped here against the visible
    /// height so resizing the terminal mid-modal can't
    /// strand the selection off-screen.
    pub(in crate::tui::app) fn draw_concordance_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        // Modal sizing: centred, generous since the
        // content (counts + KWIC samples) needs width.
        let w = area.width.saturating_sub(4).min(120).max(60);
        let h = area.height.saturating_sub(2).min(40).max(18);
        let x = area.x + (area.width.saturating_sub(w)) / 2;
        let y = area.y + (area.height.saturating_sub(h)) / 2;
        let rect = Rect { x, y, width: w, height: h };
        f.render_widget(ratatui::widgets::Clear, rect);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Concordance — project-wide ")
            .border_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        // Header: 3 rows (stats line, filter line, column header)
        let header_h: u16 = 3;
        let footer_h: u16 = 6; // 3 sample rows + hint + divider + headroom
        let list_h: u16 = inner.height.saturating_sub(header_h + footer_h);
        let header_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: header_h,
        };
        let list_rect = Rect {
            x: inner.x,
            y: inner.y + header_h,
            width: inner.width,
            height: list_h,
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + header_h + list_h,
            width: inner.width,
            height: footer_h,
        };

        // Pull modal state out by reference.  We need to
        // mutate `scroll` to clamp against `list_h`, so
        // a single mut borrow throughout.
        let dim_style = Style::default().add_modifier(Modifier::DIM);
        let bold_style = Style::default().add_modifier(Modifier::BOLD);
        let sel_style = Style::default()
            .bg(self.theme.current_line_bg)
            .add_modifier(Modifier::BOLD);
        let accent = Color::Cyan;

        let Modal::Concordance {
            data,
            filter,
            cursor,
            scroll,
            sort,
            visible,
        } = &mut self.modal
        else {
            return;
        };

        let stats_text = format!(
            " {} distinct · {} tokens · {} paragraphs scanned",
            data.distinct_words,
            data.total_tokens,
            data.paragraphs_scanned,
        );
        let filter_text = format!(
            " filter: {}   sort: {}   ({} shown)",
            filter.render_with_cursor('│'),
            sort.label(),
            visible.len(),
        );
        let col_header = " #     word                       count   variants";

        let header_lines: Vec<Line<'_>> = vec![
            Line::from(Span::styled(stats_text, Style::default().fg(accent).add_modifier(Modifier::BOLD))),
            Line::from(filter_text),
            Line::from(Span::styled(col_header, dim_style)),
        ];
        f.render_widget(Paragraph::new(header_lines), header_rect);

        // Clamp scroll so cursor stays inside the
        // visible region.  `list_h` is the number of
        // rows we can paint.
        let viewport = list_h as usize;
        if viewport > 0 {
            if *cursor < *scroll {
                *scroll = *cursor;
            } else if *cursor >= *scroll + viewport {
                *scroll = cursor.saturating_sub(viewport - 1);
            }
        }

        // Paint the list rows.
        let mut row_lines: Vec<Line<'_>> = Vec::with_capacity(viewport);
        let row_count = visible.len();
        for vis_off in 0..viewport {
            let vis_idx = *scroll + vis_off;
            if vis_idx >= row_count {
                break;
            }
            let entry_idx = visible[vis_idx];
            let entry = &data.entries[entry_idx];
            let rank = vis_idx + 1;
            // Build the variants trailer.  Skip the
            // headword itself if it appears as the
            // first variant (it usually does).
            let variants: Vec<String> = entry
                .variants
                .iter()
                .filter(|v| *v != &entry.headword)
                .take(3)
                .cloned()
                .collect();
            let variants_label = if variants.is_empty() {
                String::new()
            } else {
                format!("({})", variants.join(", "))
            };
            let row_text = format!(
                " {:>4}  {:<24}  {:>6}   {}",
                rank,
                truncate_label(&entry.headword, 24),
                entry.count,
                variants_label,
            );
            let style = if vis_idx == *cursor { sel_style } else { Style::default() };
            row_lines.push(Line::from(Span::styled(row_text, style)));
        }
        if row_lines.is_empty() {
            row_lines.push(Line::from(Span::styled(
                "  (no entries match the current filter)",
                dim_style,
            )));
        }
        f.render_widget(Paragraph::new(row_lines), list_rect);

        // Footer: KWIC samples for the currently
        // selected entry + key hints on the bottom row.
        let selected_entry: Option<&crate::tui::concordance::ConcordanceEntry> =
            visible.get(*cursor).and_then(|i| data.entries.get(*i));
        let mut footer_lines: Vec<Line<'_>> = Vec::new();
        if let Some(entry) = selected_entry {
            footer_lines.push(Line::from(vec![
                Span::styled(" samples for ", dim_style),
                Span::styled(format!("\"{}\"", entry.headword), bold_style),
                Span::styled(
                    format!("  ({}× total)", entry.count),
                    dim_style,
                ),
            ]));
            for sample in entry.samples.iter().take(3) {
                let prefix = format!(
                    "  {}:l{}  ",
                    truncate_label(&sample.slug_path, 32),
                    sample.line_no,
                );
                let kwic = truncate_label(
                    &sample.kwic,
                    (inner.width as usize).saturating_sub(prefix.len() + 2),
                );
                footer_lines.push(Line::from(vec![
                    Span::styled(prefix, dim_style),
                    Span::raw(kwic),
                ]));
            }
            // Pad the samples block out to a stable
            // height so the hint line stays at the
            // bottom even when an entry has fewer than
            // 3 samples.
            while footer_lines.len() < 4 {
                footer_lines.push(Line::from(""));
            }
        } else {
            footer_lines.push(Line::from(Span::styled(
                " (no selection)",
                dim_style,
            )));
            while footer_lines.len() < 4 {
                footer_lines.push(Line::from(""));
            }
        }
        footer_lines.push(Line::from(Span::styled(
            " ↑↓ navigate · type to filter · Ctrl+S sort · Esc close ",
            dim_style,
        )));
        f.render_widget(Paragraph::new(footer_lines), footer_rect);
    }

    /// 1.2.9+ — sentence-rhythm gauge modal painter
    /// (Ctrl+B Shift+H).  Three regions: header
    /// (verdict + numeric stats), main list (per-
    /// sentence bar chart), footer (outliers + key
    /// hints).
    pub(in crate::tui::app) fn draw_sentence_rhythm_modal(
        &mut self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let w = area.width.saturating_sub(4).min(110).max(60);
        let h = area.height.saturating_sub(2).min(36).max(18);
        let x = area.x + (area.width.saturating_sub(w)) / 2;
        let y = area.y + (area.height.saturating_sub(h)) / 2;
        let rect = Rect { x, y, width: w, height: h };
        f.render_widget(ratatui::widgets::Clear, rect);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Sentence rhythm — open paragraph ")
            .border_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .style(
                Style::default()
                    .bg(self.theme.modal_bg)
                    .fg(self.theme.modal_fg),
            );
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let (stats, scroll) = match &self.modal {
            Modal::SentenceRhythm { stats, scroll } => (stats.clone(), *scroll),
            _ => return,
        };

        let header_h: u16 = 4;
        let footer_h: u16 = 8;
        let list_h: u16 = inner.height.saturating_sub(header_h + footer_h);
        let header_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: header_h,
        };
        let list_rect = Rect {
            x: inner.x,
            y: inner.y + header_h,
            width: inner.width,
            height: list_h,
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + header_h + list_h,
            width: inner.width,
            height: footer_h,
        };

        let dim = Style::default().add_modifier(Modifier::DIM);
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let verdict_color = match stats.verdict {
            crate::tui::sentence_rhythm::RhythmVerdict::TooShort => Color::DarkGray,
            crate::tui::sentence_rhythm::RhythmVerdict::Monotone => Color::Red,
            crate::tui::sentence_rhythm::RhythmVerdict::Steady => Color::Yellow,
            crate::tui::sentence_rhythm::RhythmVerdict::Varied => Color::Green,
            crate::tui::sentence_rhythm::RhythmVerdict::Choppy => Color::Cyan,
        };

        let header_lines = vec![
            Line::from(vec![
                Span::styled(" verdict: ", dim),
                Span::styled(
                    stats.verdict.label(),
                    Style::default()
                        .fg(verdict_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("   ({})", stats.verdict.note()),
                    dim,
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    format!(
                        " {} sentences · mean {:.1} · stdev {:.1} · CV {:.2} · min {} · max {}",
                        stats.lengths.len(),
                        stats.mean,
                        stats.stdev,
                        stats.cv,
                        stats.min,
                        stats.max,
                    ),
                    bold,
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  #     bar (each block = 1 word, capped at 40)            words   preview",
                dim,
            )),
        ];
        f.render_widget(Paragraph::new(header_lines), header_rect);

        // Per-sentence list.  Each row: index, bar
        // proportional to word count, count, short
        // preview.  Cap the bar width so very long
        // sentences don't blow up the layout —
        // anything ≥ 40 words renders as `█████…`
        // (cap glyph trails ellipsis).
        let mut rows: Vec<Line<'_>> = Vec::new();
        let viewport = list_h as usize;
        let max_bar_chars: usize = 40;
        for off in 0..viewport {
            let idx = scroll + off;
            if idx >= stats.samples.len() {
                break;
            }
            let sample = &stats.samples[idx];
            let bar_chars = sample.word_count.min(max_bar_chars);
            let cap = if sample.word_count > max_bar_chars { "…" } else { "" };
            let bar: String = "█".repeat(bar_chars);
            let preview = truncate_label(
                &sample.preview,
                (inner.width as usize).saturating_sub(60),
            );
            let style = if idx == scroll {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            rows.push(Line::from(vec![
                Span::styled(
                    format!(" {:>3}  l{:<3} ", idx + 1, sample.line_no),
                    dim,
                ),
                Span::styled(
                    format!("{}{}", bar, cap),
                    Style::default().fg(verdict_color),
                ),
                Span::styled(
                    format!(
                        "{:padding$}{:>3}   ",
                        "",
                        sample.word_count,
                        padding = max_bar_chars + 2 - bar_chars - cap.chars().count(),
                    ),
                    style,
                ),
                Span::raw(preview),
            ]));
        }
        if rows.is_empty() {
            rows.push(Line::from(Span::styled(
                "  (no sentences in this paragraph)",
                dim,
            )));
        }
        f.render_widget(Paragraph::new(rows), list_rect);

        // Footer: outlier callouts (shortest +
        // longest) + key hints.
        let mut footer_lines: Vec<Line<'_>> = Vec::new();
        footer_lines.push(Line::from(Span::styled(" shortest:", dim)));
        for sample in stats.shortest.iter().take(3) {
            let preview = truncate_label(
                &sample.preview,
                (inner.width as usize).saturating_sub(20),
            );
            footer_lines.push(Line::from(vec![
                Span::styled(
                    format!("   l{:<3} {:>3}w  ", sample.line_no, sample.word_count),
                    dim,
                ),
                Span::raw(preview),
            ]));
        }
        footer_lines.push(Line::from(Span::styled(" longest:", dim)));
        for sample in stats.longest.iter().take(3) {
            let preview = truncate_label(
                &sample.preview,
                (inner.width as usize).saturating_sub(20),
            );
            footer_lines.push(Line::from(vec![
                Span::styled(
                    format!("   l{:<3} {:>3}w  ", sample.line_no, sample.word_count),
                    dim,
                ),
                Span::raw(preview),
            ]));
        }
        // Pad to stable height so the hint sits at
        // the bottom.
        while footer_lines.len() + 1 < footer_h as usize {
            footer_lines.push(Line::from(""));
        }
        footer_lines.push(Line::from(Span::styled(
            " ↑↓ / PgUp/PgDn / Home / End scroll · any other key closes ",
            dim,
        )));
        f.render_widget(Paragraph::new(footer_lines), footer_rect);
    }

    /// 1.2.14+ Phase A.2 — `Ctrl+V Shift+H` Threads
    /// picker.  Centred modal listing every plot
    /// thread with status / weight / tension /
    /// counts.  Filter input visible at the top
    /// when `filter_active`.
    pub(in crate::tui::app) fn draw_threads_picker_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::ThreadsPicker {
            entries,
            cursor,
            filter,
            filter_active,
            visible,
        } = &self.modal
        else {
            return;
        };

        let max_name = visible
            .iter()
            .filter_map(|i| entries.get(*i))
            .map(|e| e.title_field.chars().count())
            .max()
            .unwrap_or(8)
            .max(8);
        let name_w = max_name.min(40);

        // Layout columns: name | status | weight | tension | ch | pl | ←
        // header / body / footer row counts.
        let header_lines = 3usize;
        let body_lines = visible.len().min(20).max(1);
        let footer_lines = 3usize;
        let height = ((header_lines + body_lines + footer_lines + 2) as u16)
            .clamp(10, area.height.saturating_sub(2));
        let width = (name_w as u16 + 60).min(area.width.saturating_sub(4)).max(70);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let title = format!(
            " Threads · {} of {} · Ctrl+V Shift+H ",
            visible.len(),
            entries.len()
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

        let mut lines: Vec<Line<'static>> = Vec::new();
        // Header row: filter input (if active) or
        // column legend.
        if *filter_active {
            let prompt = format!(" / {}_ ", filter.as_str());
            lines.push(Line::from(Span::styled(
                prompt,
                Style::default()
                    .fg(self.theme.editor_position_fg)
                    .add_modifier(Modifier::BOLD),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                format!(
                    "   {:<width$}  {:>8}  {:>8}  {:>5}  {:>3}  {:>3}  {:>4}",
                    "name", "status", "weight", "ten.", "ch", "pl", "link",
                    width = name_w,
                ),
                Style::default().add_modifier(Modifier::DIM),
            )));
        }
        lines.push(Line::from(""));

        // Body rows.
        for (display_idx, src_idx) in visible.iter().enumerate() {
            let Some(e) = entries.get(*src_idx) else { continue; };
            let marker = if display_idx == *cursor { "›" } else { " " };
            let row = format!(
                "  {marker} {name:<width$}  {status:>8}  {weight:>8}  {ten:>5}  {ch:>3}  {pl:>3}  {link:>4}",
                marker = marker,
                name = truncate_to(&e.title_field, name_w),
                status = truncate_to(&e.status, 8),
                weight = truncate_to(&e.weight, 8),
                ten = e.tension,
                ch = e.character_count,
                pl = e.place_count,
                link = e.link_count,
                width = name_w,
            );
            let style = if display_idx == *cursor {
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(row, style)));
        }

        lines.push(Line::from(""));
        let hint = if *filter_active {
            " Enter / Esc exit filter · Backspace edits "
        } else {
            " ↑↓ Enter open · Shift+Enter pin · w weave · / filter · Esc close "
        };
        lines.push(Line::from(Span::styled(
            hint.to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )));

        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }

    /// 1.2.14+ Phase D.4 — TUI thread doctor
    /// modal (`Ctrl+V Shift+D`).  Read-only;
    /// renders the snapshot computed at modal
    /// open.
    pub(in crate::tui::app) fn draw_thread_doctor_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::ThreadDoctor { data } = &self.modal else {
            return;
        };
        let width = (area.width.saturating_sub(4)).min(70).max(50);
        let extra_rows = data.status_distribution.len()
            + data.weight_distribution.len()
            + data.zero_links.len()
            + data.payoff_unfired.len()
            + data.dormant.len();
        let height = ((10 + extra_rows) as u16)
            .clamp(14, area.height.saturating_sub(2));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);
        let title = format!(
            " Thread doctor · {} threads · Ctrl+V Shift+D ",
            data.thread_count
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

        let dim = Style::default().add_modifier(Modifier::DIM);
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::raw("  avg tension: "),
            Span::styled(format!("{:.1}", data.avg_tension), bold),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("  status:".to_string(), dim)));
        for (k, v) in &data.status_distribution {
            lines.push(Line::from(format!("    {:<10} {}", k, v)));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("  weight:".to_string(), dim)));
        for (k, v) in &data.weight_distribution {
            lines.push(Line::from(format!("    {:<10} {}", k, v)));
        }
        lines.push(Line::from(""));
        let no_blind = data.zero_links.is_empty()
            && data.payoff_unfired.is_empty()
            && data.dormant.is_empty();
        if no_blind {
            lines.push(Line::from(Span::styled(
                "  Blind spots: (none detected)".to_string(),
                dim,
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "  Blind spots:".to_string(),
                bold,
            )));
            if !data.zero_links.is_empty() {
                lines.push(Line::from(Span::styled(
                    "    ZERO LINKS — status past `setup`, no project links:".to_string(),
                    dim,
                )));
                for t in &data.zero_links {
                    lines.push(Line::from(format!(
                        "      · {}",
                        truncate_to(t, 60)
                    )));
                }
            }
            if !data.payoff_unfired.is_empty() {
                lines.push(Line::from(Span::styled(
                    "    PAYOFF UNFIRED — status `payoff`, no project links:".to_string(),
                    dim,
                )));
                for t in &data.payoff_unfired {
                    lines.push(Line::from(format!(
                        "      · {}",
                        truncate_to(t, 60)
                    )));
                }
            }
            if !data.dormant.is_empty() {
                lines.push(Line::from(Span::styled(
                    "    DORMANT — status `develop`, ≤1 link:".to_string(),
                    dim,
                )));
                for t in &data.dormant {
                    lines.push(Line::from(format!(
                        "      · {}",
                        truncate_to(t, 60)
                    )));
                }
            }
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Esc to close".to_string(),
            dim,
        )));
        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }

    /// 1.2.14+ Phase Q.3b — `Ctrl+V f` footnote
    /// editor.  Multi-line input box, much like
    /// the comment editor.
    pub(in crate::tui::app) fn draw_footnote_editor_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::FootnoteEditor { textarea, .. } = &self.modal else {
            return;
        };
        let width = (area.width.saturating_sub(4)).min(90).max(40);
        let height = (area.height.saturating_sub(4)).min(12).max(8);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);
        let style_label = match self.cfg.editor.footnote_style.to_lowercase().as_str() {
            "markdown" => " (markdown style)",
            _ => " (Typst style)",
        };
        let title =
            format!(" Footnote{style_label} · Ctrl+V f ");
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
        let editor_rect = Rect {
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
        let mut ta = textarea.clone();
        ta.set_block(
            Block::default()
                .borders(Borders::ALL)
                .style(
                    Style::default()
                        .bg(self.theme.modal_bg)
                        .fg(self.theme.modal_fg),
                )
                .border_style(
                    Style::default().fg(self.theme.modal_border),
                ),
        );
        f.render_widget(&ta, editor_rect);
        let hint_line = Line::from(Span::styled(
            " Ctrl+S commit · Esc cancel (when empty) · Ctrl+C cancel anytime "
                .to_string(),
            Style::default().add_modifier(Modifier::DIM),
        ));
        f.render_widget(Paragraph::new(hint_line), footer_rect);
    }

    /// 1.2.14+ Phase Q.4a — project goal +
    /// projection modal (`Ctrl+V Shift+G`).
    pub(in crate::tui::app) fn draw_project_goal_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::ProjectGoalModal { data } = &self.modal else {
            return;
        };
        let width = (area.width.saturating_sub(4)).min(70).max(50);
        // Lines: header + 6 stat lines + spacer +
        // per-book rows + footer = 11..20.
        let body_lines = 10 + data.per_book.len();
        let height = (body_lines as u16 + 4)
            .min(area.height.saturating_sub(2))
            .max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);
        let title = " Project goal · Ctrl+V Shift+G ";
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title.to_string())
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
        let dim = Style::default().add_modifier(Modifier::DIM);
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(""));
        if data.goal > 0 {
            lines.push(Line::from(vec![
                Span::raw("   Total: "),
                Span::styled(format!("{:>7}", data.total_words), bold),
                Span::raw(" / "),
                Span::styled(format!("{:>7}", data.goal), bold),
                Span::raw(format!("   ({:>3} %)", data.pct.min(100))),
            ]));
            lines.push(Line::from(progress_bar(data.pct, width.saturating_sub(8))));
        } else {
            lines.push(Line::from(vec![
                Span::raw("   Total: "),
                Span::styled(format!("{:>7}", data.total_words), bold),
                Span::raw("    (no goal set in HJSON)"),
            ]));
            lines.push(Line::from(""));
        }
        lines.push(Line::from(""));
        if let Some(days) = data.days_remaining {
            lines.push(Line::from(format!(
                "   Days remaining: {:>3}",
                days
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "   No target date set".to_string(),
                dim,
            )));
        }
        if let Some(req) = data.required_per_day {
            lines.push(Line::from(format!(
                "   Required:       {:>6} words / day from today",
                req
            )));
        }
        if let Some(avg) = data.recent_avg {
            lines.push(Line::from(format!(
                "   Recent avg:     {:>6} words / day",
                avg
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "   Recent avg:     (Q.4.1 — event-log wiring queued)"
                    .to_string(),
                dim,
            )));
        }
        if let Some(p) = data.projection_date {
            lines.push(Line::from(format!(
                "   Projection:     {}",
                p.format("%Y-%m-%d")
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(format!(
            "   Verdict:        {} {}",
            data.verdict.glyph(),
            data.verdict.label()
        )));
        if !data.per_book.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "   Per book:".to_string(),
                dim,
            )));
            for (title, words, pct) in &data.per_book {
                lines.push(Line::from(format!(
                    "    · {:<24} {:>6} ({:>3} %)",
                    truncate_to(title, 24),
                    words,
                    pct
                )));
            }
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "   Esc to close".to_string(),
            dim,
        )));
        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }

    /// 1.2.14+ Phase Q.4b — style transfer
    /// reference picker (`Ctrl+V y`).
    pub(in crate::tui::app) fn draw_style_transfer_picker_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::StyleTransferPicker {
            entries,
            cursor,
            filter,
            filter_active,
            visible,
            ..
        } = &self.modal
        else {
            return;
        };
        let width = (area.width.saturating_sub(4)).min(80).max(50);
        let body = visible.len().min(20).max(1);
        let height = ((body + 6) as u16)
            .clamp(10, area.height.saturating_sub(2));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);
        let title = format!(
            " Style transfer — pick a voice sample · {} of {} ",
            visible.len(),
            entries.len()
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
        let mut lines: Vec<Line<'static>> = Vec::new();
        if *filter_active {
            lines.push(Line::from(Span::styled(
                format!(" / {}_ ", filter.as_str()),
                Style::default()
                    .fg(self.theme.editor_position_fg)
                    .add_modifier(Modifier::BOLD),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "  pick a paragraph whose voice you want to mimic".to_string(),
                Style::default().add_modifier(Modifier::DIM),
            )));
        }
        lines.push(Line::from(""));
        for (display_idx, src_idx) in visible.iter().enumerate() {
            let Some((_, title)) = entries.get(*src_idx) else { continue; };
            let marker = if display_idx == *cursor { "›" } else { " " };
            let row = format!("  {marker} {title}");
            let style = if display_idx == *cursor {
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(row, style)));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " ↑↓ Enter pick · / filter · Esc cancel ".to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )));
        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }

    /// 1.2.14+ Phase C.2 — project-wide comments
    /// panel (`Ctrl+V Shift+C`).
    pub(in crate::tui::app) fn draw_comments_panel_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::CommentsPanel {
            entries,
            cursor,
            filter,
            filter_active,
            hide_resolved,
            visible,
        } = &self.modal
        else {
            return;
        };
        let width = (area.width.saturating_sub(4)).min(110).max(70);
        let height = (area.height.saturating_sub(4)).min(28).max(12);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let title = format!(
            " Comments · {} of {} · Ctrl+V Shift+C ",
            visible.len(),
            entries.len()
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

        let mut lines: Vec<Line<'static>> = Vec::new();
        // Filter input or status row.
        if *filter_active {
            let prompt = format!(" / {}_ ", filter.as_str());
            lines.push(Line::from(Span::styled(
                prompt,
                Style::default()
                    .fg(self.theme.editor_position_fg)
                    .add_modifier(Modifier::BOLD),
            )));
        } else {
            let hide = if *hide_resolved {
                " (resolved hidden — R to show)"
            } else {
                " (showing all — R to hide resolved)"
            };
            lines.push(Line::from(Span::styled(
                hide.to_string(),
                Style::default().add_modifier(Modifier::DIM),
            )));
        }
        lines.push(Line::from(""));

        // Body rows.
        let max_breadcrumb = 36usize;
        let max_author = 12usize;
        for (display_idx, src_idx) in visible.iter().enumerate() {
            let Some(e) = entries.get(*src_idx) else { continue; };
            let marker = if display_idx == *cursor { "›" } else { " " };
            let age = super::super::comments_impl::humanise_age(e.created_at);
            let breadcrumb =
                truncate_to(&e.paragraph_breadcrumb, max_breadcrumb);
            let author = truncate_to(&e.author, max_author);
            let snippet: String = e.text.chars().take(60).collect();
            let snippet = if e.text.chars().count() > 60 {
                format!("{snippet}…")
            } else {
                snippet
            };
            let resolved_label = if e.resolved { " [r]" } else { "" };
            // Dense indicator: only shown when the
            // paragraph has more than one comment.
            // Single-comment paragraphs would just
            // see (1/1 in ¶) which is noise.
            let dense_label = if e.paragraph_total_comments > 1 {
                format!(
                    " ({}/{} in ¶)",
                    e.paragraph_position, e.paragraph_total_comments
                )
            } else {
                String::new()
            };
            let header = format!(
                "  {marker} {breadcrumb:<bc_w$}  {author:<au_w$}  {age:>10}{resolved_label}{dense_label}",
                bc_w = max_breadcrumb,
                au_w = max_author,
            );
            let style = if display_idx == *cursor {
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .add_modifier(Modifier::BOLD)
            } else if e.resolved {
                Style::default().add_modifier(Modifier::DIM)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(header, style)));
            // Secondary indent line for the snippet.
            let snippet_line = format!("       {snippet}");
            let snippet_style = if e.resolved {
                Style::default().add_modifier(Modifier::DIM).add_modifier(Modifier::ITALIC)
            } else if display_idx == *cursor {
                Style::default().add_modifier(Modifier::ITALIC).add_modifier(Modifier::BOLD)
            } else {
                Style::default().add_modifier(Modifier::ITALIC)
            };
            lines.push(Line::from(Span::styled(snippet_line, snippet_style)));
        }

        lines.push(Line::from(""));
        let hint = if *filter_active {
            " Enter / Esc exit filter · Backspace edits "
        } else {
            " ↑↓ Enter open · r resolve · u reopen · R toggle resolved · d delete · a AI digest · / filter · Esc "
        };
        lines.push(Line::from(Span::styled(
            hint.to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )));

        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }

    /// 1.2.14+ Phase C.1 — `Ctrl+V c` comment editor.
    /// Header shows the anchor span snippet so the
    /// author sees what they're commenting on.
    /// Body is a multi-line TextArea (same widget
    /// the HJSON editor uses).  Footer hints at
    /// commit / cancel.
    pub(in crate::tui::app) fn draw_comment_editor_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::CommentEditor {
            textarea,
            anchor_preview,
            anchor_start,
            anchor_end,
            ..
        } = &self.modal
        else {
            return;
        };
        let span_chars = anchor_end.saturating_sub(*anchor_start);
        let width = (area.width.saturating_sub(4)).min(96).max(40);
        let height = (area.height.saturating_sub(4)).min(14).max(10);
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let title = format!(" Add comment · {span_chars} chars · Ctrl+V c ");
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

        // Carve the inner area into:
        //  · 1 line  preview header
        //  · 1 line  spacer
        //  · N lines TextArea (the rest)
        //  · 1 line  footer hint
        let preview_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        };
        let editor_rect = Rect {
            x: inner.x,
            y: inner.y + 2,
            width: inner.width,
            height: inner.height.saturating_sub(3),
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };

        // Preview line.
        let preview_line = Line::from(vec![
            Span::styled(
                "  on: ".to_string(),
                Style::default().add_modifier(Modifier::DIM),
            ),
            Span::styled(
                format!("\"{anchor_preview}\""),
                Style::default().add_modifier(Modifier::ITALIC),
            ),
        ]);
        f.render_widget(Paragraph::new(preview_line), preview_rect);

        // Editor body — re-style the textarea with
        // the modal palette before render.  We do
        // this via a fresh widget so the textarea's
        // internal styling doesn't fight the modal
        // bg.
        let mut ta = textarea.clone();
        ta.set_block(
            Block::default()
                .borders(Borders::ALL)
                .style(
                    Style::default()
                        .bg(self.theme.modal_bg)
                        .fg(self.theme.modal_fg),
                )
                .border_style(
                    Style::default().fg(self.theme.modal_border),
                ),
        );
        f.render_widget(&ta, editor_rect);

        // Footer hint.
        let hint_line = Line::from(Span::styled(
            " Ctrl+S commit · Esc cancel (when empty) · Ctrl+C cancel anytime "
                .to_string(),
            Style::default().add_modifier(Modifier::DIM),
        ));
        f.render_widget(Paragraph::new(hint_line), footer_rect);
    }

    /// 1.2.14+ Phase A.2 — swim-lane weave view.
    /// Rendered as a table with one row per thread
    /// (down the side) and one column per chapter
    /// (across the top).  Each cell shows a count
    /// of paragraphs in that chapter that link to
    /// that thread; the cursor cell is reversed.
    /// Books are visually separated by a thin gap
    /// in the column headers.
    pub(in crate::tui::app) fn draw_thread_weave_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::ThreadWeaveView {
            threads,
            chapters,
            grid,
            cursor_row,
            cursor_col,
            scroll_row,
            scroll_col,
            ..
        } = &self.modal
        else {
            return;
        };

        let max_thread_name = threads
            .iter()
            .map(|t| t.title_field.chars().count())
            .max()
            .unwrap_or(12)
            .max(12)
            .min(24);
        let cell_width = 4usize;
        let label_w = max_thread_name + 2;

        let width = area.width.saturating_sub(2);
        let height = area.height.saturating_sub(2);
        let rect = Rect { x: area.x + 1, y: area.y + 1, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let title = format!(
            " Thread weave · {} threads × {} chapters · Esc back ",
            threads.len(),
            chapters.len()
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

        // How many chapter columns fit?  Use that
        // to drive horizontal scroll.
        let cols_visible = (inner.width as usize).saturating_sub(label_w)
            / cell_width;
        let cols_visible = cols_visible.max(1);
        let start_col = (*scroll_col).min(chapters.len().saturating_sub(cols_visible));
        let end_col = (start_col + cols_visible).min(chapters.len());

        // Vertical scroll.
        let rows_visible = (inner.height as usize).saturating_sub(4); // header + book row + hint + spacer
        let rows_visible = rows_visible.max(1);
        let start_row = (*scroll_row).min(threads.len().saturating_sub(rows_visible));
        let end_row = (start_row + rows_visible).min(threads.len());

        let mut lines: Vec<Line<'static>> = Vec::new();

        // Header row 1: book grouping (one cell per
        // chapter, showing book initial when the
        // chapter changes book).
        let mut book_row = String::with_capacity(inner.width as usize);
        book_row.push_str(&" ".repeat(label_w));
        let mut last_book: Option<&str> = None;
        for (idx, (_, book, _)) in chapters[start_col..end_col].iter().enumerate() {
            let cell = if last_book.map(|b| b == book.as_str()).unwrap_or(false) {
                "    ".to_string()
            } else {
                let _ = idx;
                let abbr: String = book.chars().take(3).collect();
                format!(" {:<3.3}", abbr)
            };
            book_row.push_str(&cell);
            last_book = Some(book.as_str());
        }
        lines.push(Line::from(Span::styled(
            book_row,
            Style::default().add_modifier(Modifier::DIM),
        )));

        // Header row 2: chapter index across the
        // top.  Two-digit indices.
        let mut chapter_row = String::with_capacity(inner.width as usize);
        chapter_row.push_str(&" ".repeat(label_w));
        for (idx, _) in chapters[start_col..end_col].iter().enumerate() {
            let global_idx = start_col + idx;
            chapter_row.push_str(&format!(" {:>2} ", global_idx + 1));
        }
        lines.push(Line::from(Span::styled(
            chapter_row,
            Style::default().add_modifier(Modifier::DIM),
        )));

        // Body rows.
        for r in start_row..end_row {
            let t = &threads[r];
            let mut row_cells: Vec<Span<'static>> = Vec::new();
            // Thread label.
            let label = format!(
                "{:<width$}  ",
                truncate_to(&t.title_field, max_thread_name),
                width = max_thread_name,
            );
            row_cells.push(Span::styled(
                label,
                if r == *cursor_row {
                    Style::default()
                        .fg(self.theme.modal_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ));
            for c in start_col..end_col {
                let cell_count = grid
                    .get(r)
                    .and_then(|row| row.get(c))
                    .map(|cell| cell.len())
                    .unwrap_or(0);
                let text = if cell_count == 0 {
                    "  ·".to_string()
                } else if cell_count == 1 {
                    "  ●".to_string()
                } else {
                    format!("  {cell_count}")
                };
                let style = if r == *cursor_row && c == *cursor_col {
                    Style::default()
                        .add_modifier(Modifier::REVERSED)
                        .add_modifier(Modifier::BOLD)
                } else if cell_count > 0 {
                    Style::default().fg(self.theme.places_fg)
                } else {
                    Style::default().add_modifier(Modifier::DIM)
                };
                row_cells.push(Span::styled(format!("{text:>4}"), style));
            }
            lines.push(Line::from(row_cells));
        }

        lines.push(Line::from(""));
        let dim = Style::default().add_modifier(Modifier::DIM);
        // Highlight the cell coordinate context.
        if let Some(t) = threads.get(*cursor_row) {
            if let Some((_, book, chapter)) = chapters.get(*cursor_col) {
                let cell_count = grid
                    .get(*cursor_row)
                    .and_then(|row| row.get(*cursor_col))
                    .map(|cell| cell.len())
                    .unwrap_or(0);
                let footer = format!(
                    " {} · {}/{} · {} linking paragraph{}",
                    truncate_to(&t.title_field, 30),
                    truncate_to(book, 20),
                    truncate_to(chapter, 24),
                    cell_count,
                    if cell_count == 1 { "" } else { "s" },
                );
                lines.push(Line::from(Span::styled(footer, dim)));
            }
        }
        lines.push(Line::from(Span::styled(
            " ↑↓ thread · ←→ chapter · Enter jump · Esc back to picker ".to_string(),
            dim,
        )));

        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }

    /// 1.2.15+ Phase D.3 — TUI project doctor
    /// panel.  Renders the findings table with the
    /// cursor highlighted, footer hints, and the
    /// last-action status line below.  Layout:
    ///
    ///   ┌ Doctor — N findings — Ctrl+B Shift+0 ┐
    ///   │   [N] sev · class · path             │
    ///   │       detail                          │
    ///   │   …                                   │
    ///   │ <last_status>                         │
    ///   │ ↑↓ navigate · r repair · R repair all │
    ///   └───────────────────────────────────────┘
    pub(in crate::tui::app) fn draw_doctor_panel_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::DoctorPanel { findings, cursor, scroll, last_status } =
            &self.modal
        else {
            return;
        };
        let total = findings.len();
        let width = area.width.saturating_sub(4).clamp(60, 110);
        let inner_w = width.saturating_sub(4) as usize;
        let header_lines = 1;
        let footer_lines = 2;
        let max_rows = area.height.saturating_sub(6) as usize;
        let body_h = ((total * 2).max(2)).min(max_rows.max(4));
        let height = (header_lines + body_h + footer_lines + 2) as u16;
        let height = height.min(area.height.saturating_sub(2));
        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(height) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);
        let title = if total == 0 {
            " Doctor — project is clean — Ctrl+B Shift+0 ".to_string()
        } else {
            format!(" Doctor — {total} finding(s) — Ctrl+B Shift+0 ")
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let dim = Style::default().add_modifier(Modifier::DIM);
        let mut lines: Vec<Line<'_>> = Vec::new();

        if findings.is_empty() {
            lines.push(Line::from(Span::styled(
                "  (nothing to do — the project scan returned zero findings)",
                dim,
            )));
        } else {
            // Visible window — body_h / 2 rows because
            // each finding takes 2 lines (heading + detail).
            let rows_per_line = 2;
            let visible_rows = body_h / rows_per_line;
            let start = (*scroll).min(total.saturating_sub(visible_rows.max(1)));
            let end = (start + visible_rows.max(1)).min(total);
            for (idx, f) in findings.iter().enumerate().skip(start).take(end - start) {
                let marker = if idx == *cursor { "›" } else { " " };
                let sev = f.severity.slug();
                let class = f.class.slug();
                let path = f.path.as_deref().unwrap_or("-");
                let path_short = truncate_to(path, inner_w.saturating_sub(40));
                let heading = format!("  {marker} {sev:>8} · {class:<24} · {path_short}");
                let detail = truncate_to(&f.detail, inner_w.saturating_sub(8));
                let detail_line = format!("        {detail}");
                let style = if idx == *cursor {
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(heading, style)));
                lines.push(Line::from(Span::styled(detail_line, dim)));
            }
        }

        // Optional status line.
        lines.push(Line::from(""));
        if let Some(s) = last_status.as_deref() {
            lines.push(Line::from(Span::styled(
                format!("  {s}"),
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::ITALIC),
            )));
        }
        let footer = if findings.is_empty() {
            " Esc closes ".to_string()
        } else {
            " ↑↓ navigate · r repair · R repair all · Esc closes ".to_string()
        };
        lines.push(Line::from(Span::styled(footer, dim)));

        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }

    /// 1.2.16+ Phase P.6 — snippet picker modal.
    /// Mid-snippet-expansion overlay: the editor
    /// has already pasted the snippet's head; the
    /// user picks one entry from the relevant
    /// system book (Characters / Places /
    /// Artefacts), and Enter inserts its title +
    /// the stashed tail back into the editor.
    pub(in crate::tui::app) fn draw_snippet_picker_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::SnippetPicker {
            kind,
            input,
            candidates,
            matches,
            cursor,
            tail: _,
        } = &self.modal
        else {
            return;
        };
        let width = area.width.saturating_sub(6).clamp(50, 90);
        let body_max = area.height.saturating_sub(6).clamp(8, 20);
        let visible = matches.len().min(body_max as usize).max(1);
        let height = (5 + visible as u16).min(area.height.saturating_sub(2));
        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(height) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let title = format!(" Pick a {} — snippet expansion ", kind.label());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let dim = Style::default().add_modifier(Modifier::DIM);
        let mut lines: Vec<Line<'_>> = Vec::new();

        lines.push(Line::from(Span::styled(
            format!(" › {}", input.render_with_cursor('│')),
            Style::default().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        if matches.is_empty() {
            lines.push(Line::from(Span::styled(
                "  (no entries match — refine the filter or Esc to cancel)",
                dim,
            )));
        } else {
            for (i, idx) in matches.iter().enumerate().take(body_max as usize) {
                let Some(name) = candidates.get(*idx) else { continue };
                let marker = if i == *cursor { "›" } else { " " };
                let style = if i == *cursor {
                    Style::default()
                        .add_modifier(Modifier::REVERSED)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(
                    format!("  {marker} {name}"),
                    style,
                )));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " type filter · ↑↓ select · Enter commit · Esc restore placeholder ",
            dim,
        )));

        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    }

    /// 1.2.16+ Phase A.2 — manuscript intelligence
    /// dashboard render.  Sectioned synthesis of
    /// every metric inkhaven has been collecting
    /// since 1.2.5.
    pub(in crate::tui::app) fn draw_journal_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::Journal { snapshot, scroll, last_status } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(4).clamp(58, 100);
        let height = area.height.saturating_sub(2).clamp(20, 44);
        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(height) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);
        let title = format!(
            " Journal — manuscript intelligence ({}) ",
            snapshot.generated_at.format("%Y-%m-%d %H:%M UTC")
        );
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::BOLD),
            )
            .style(Style::default().bg(self.theme.modal_bg).fg(self.theme.modal_fg));
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let dim = Style::default().add_modifier(Modifier::DIM);
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let mut all: Vec<Line<'_>> = Vec::new();

        // Word count section.
        all.push(Line::from(Span::styled(" Word count", bold)));
        all.push(Line::from(format!(
            "   today: {} · total: {} · streak: {}d",
            snapshot.word_count.today,
            snapshot.word_count.total,
            snapshot.word_count.streak_days,
        )));
        if snapshot.word_count.goal > 0 {
            let remaining =
                (snapshot.word_count.goal - snapshot.word_count.total).max(0);
            let target = if snapshot.word_count.target_date.is_empty() {
                "(no target date)".to_string()
            } else {
                snapshot.word_count.target_date.clone()
            };
            all.push(Line::from(format!(
                "   goal: {} · remaining: {} · target: {}",
                snapshot.word_count.goal, remaining, target,
            )));
        }
        all.push(Line::from(format!(
            "   active: {}m today · {}m this week",
            snapshot.word_count.active_seconds_today / 60,
            snapshot.word_count.active_seconds_week / 60,
        )));
        all.push(Line::from(""));

        // Structure.
        all.push(Line::from(Span::styled(" Structure", bold)));
        all.push(Line::from(format!(
            "   books: {} · chapters: {} · paragraphs: {}",
            snapshot.structure.user_books,
            snapshot.structure.chapters,
            snapshot.structure.paragraphs,
        )));
        if !snapshot.structure.chapter_word_counts.is_empty() {
            all.push(Line::from(format!(
                "   mean chapter: {:.0} words ± {:.0} (CV {:.0}%)",
                snapshot.structure.avg_chapter_words,
                snapshot.structure.stdev_chapter_words,
                snapshot.structure.cv * 100.0,
            )));
            all.push(Line::from(format!(
                "   pacing: {}",
                snapshot.structure.pacing_verdict
            )));
        }
        all.push(Line::from(""));

        // Threads.
        all.push(Line::from(Span::styled(" Threads", bold)));
        all.push(Line::from(format!(
            "   total: {} · active: {} · dormant (>{}d): {}",
            snapshot.threads.total,
            snapshot.threads.active,
            crate::tui::journal::DORMANT_DAYS,
            snapshot.threads.dormant,
        )));
        all.push(Line::from(""));

        // Comments.
        all.push(Line::from(Span::styled(" Comments", bold)));
        all.push(Line::from(format!(
            "   open: {} · resolved this week: {} · resolved total: {}",
            snapshot.comments.open,
            snapshot.comments.resolved_this_week,
            snapshot.comments.resolved_total,
        )));
        all.push(Line::from(""));

        // Status / footer.
        if let Some(s) = last_status.as_deref() {
            all.push(Line::from(Span::styled(
                format!(" {s}"),
                Style::default()
                    .fg(self.theme.modal_border)
                    .add_modifier(Modifier::ITALIC),
            )));
            all.push(Line::from(""));
        }

        let body_h = inner.height.saturating_sub(1) as usize;
        let total = all.len();
        let max_scroll = total.saturating_sub(body_h);
        let scroll = (*scroll).min(max_scroll);
        let end = (scroll + body_h).min(total);
        let visible: Vec<Line<'_>> = all[scroll..end].to_vec();

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

        f.render_widget(Paragraph::new(visible).wrap(Wrap { trim: false }), body_rect);
        let more = if end < total { " · ↓ more below" } else { "" };
        let footer = format!(
            " ↑↓ scroll · e export to journal-<ts>.md · Esc closes{more} ",
        );
        f.render_widget(
            Paragraph::new(Span::styled(footer, dim)),
            footer_rect,
        );
    }

    /// 1.2.17+ T.6 — voice picker render.  Header
    /// shows catalog state (fresh / stale / dir-only),
    /// filter on the left + entry count on the right,
    /// table-like rows (key · lang · quality · status
    /// chip · size), footer with keybinds.
    pub(in crate::tui::app) fn draw_tts_voice_picker_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::TtsVoicePicker { state } = &self.modal else {
            return;
        };
        let width = area.width.saturating_sub(6).clamp(60, 100);
        let body_max = area.height.saturating_sub(8).clamp(8, 24);
        let filtered = state.filtered_indices();
        let visible = filtered.len().min(body_max as usize).max(1);
        let height = (7 + visible as u16).min(area.height.saturating_sub(2));
        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(height) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let title = " Piper voices ".to_string();
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

        let dim = Style::default().add_modifier(Modifier::DIM);
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let amber = Style::default().fg(ratatui::style::Color::Yellow);
        let green = Style::default().fg(ratatui::style::Color::Green);
        let mut lines: Vec<Line<'_>> = Vec::new();

        // Header row: catalog status + count.
        let header = if let Some(reason) = state.catalog_failed.as_ref() {
            format!(
                " catalog: offline ({}) · {} voice(s) on disk",
                truncate_to(reason, 40),
                state.entries.len(),
            )
        } else if state.catalog_stale {
            format!(
                " catalog: stale (using cached) · {} voice(s)",
                state.entries.len(),
            )
        } else {
            format!(
                " catalog: fresh · {} voice(s)",
                state.entries.len(),
            )
        };
        let header_style = if state.catalog_failed.is_some()
            || state.catalog_stale
        {
            amber
        } else {
            green
        };
        lines.push(Line::from(Span::styled(header, header_style)));

        // Filter input.
        let filter_label = if state.filter.is_empty() {
            "  filter: (type to filter by lang or name)".to_string()
        } else {
            format!("  filter: /{}│  ({} match)", state.filter, filtered.len())
        };
        lines.push(Line::from(Span::styled(filter_label, dim)));
        lines.push(Line::from(""));

        if filtered.is_empty() {
            lines.push(Line::from(Span::styled(
                "  (no entries — clear the filter or Esc to close)",
                dim,
            )));
        } else {
            let cursor = state.cursor;
            for (row, idx) in filtered.iter().enumerate().take(body_max as usize) {
                let Some(entry) = state.entries.get(*idx) else {
                    continue;
                };
                let chip = if entry.downloaded {
                    "✓"
                } else {
                    "⬇"
                };
                let size = if entry.size_bytes > 0 {
                    format!(" {:>4} MB", entry.size_bytes / 1_048_576)
                } else {
                    "".to_string()
                };
                let lang = if entry.language_english.is_empty() {
                    entry.language_code.clone()
                } else {
                    format!(
                        "{} ({})",
                        entry.language_english, entry.language_code,
                    )
                };
                let line = format!(
                    "  {marker} {chip} {key:<28} {lang:<22} {q:<7}{size}",
                    marker = if row == cursor { "›" } else { " " },
                    key = truncate_to(&entry.key, 28),
                    lang = truncate_to(&lang, 22),
                    q = truncate_to(&entry.quality, 7),
                );
                let style = if row == cursor {
                    bold.add_modifier(Modifier::REVERSED)
                } else if !entry.downloaded {
                    Style::default()
                } else {
                    bold
                };
                lines.push(Line::from(Span::styled(line, style)));
            }
        }

        // Picker-local status line (per-action message).
        lines.push(Line::from(""));
        if !state.status.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("  {}", state.status),
                amber,
            )));
        }

        // Footer keybinds.
        lines.push(Line::from(Span::styled(
            " ↑↓ select · / filter · Enter download/use · d remove · Esc close ",
            dim,
        )));

        f.render_widget(
            Paragraph::new(lines).wrap(Wrap { trim: false }),
            inner,
        );
    }

    /// 1.2.18+ R.4 — reader-pace teleprompter render.
    /// Shows the paragraph prose with already-read words
    /// dim, the current word reverse-highlighted, and
    /// upcoming words normal.  A footer reports the live
    /// elapsed / remaining time + the keys.
    pub(in crate::tui::app) fn draw_reader_pace_modal(
        &self,
        f: &mut ratatui::Frame,
        area: Rect,
    ) {
        let Modal::ReaderPace { words, paused, wpm, .. } = &self.modal else {
            return;
        };
        let (idx, total) = self.reader_pace_index().unwrap_or((0, words.len()));

        let width = area.width.saturating_sub(8).clamp(40, 84);
        let height = area.height.saturating_sub(6).clamp(10, 24);
        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(height) / 2;
        let rect = Rect { x, y, width, height };
        f.render_widget(ratatui::widgets::Clear, rect);

        let finished = idx >= total;
        let title = if finished {
            " Reader pace — done ".to_string()
        } else if *paused {
            " Reader pace — paused ".to_string()
        } else {
            " Reader pace ".to_string()
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

        // Build a single wrapped line of spans: dim for
        // already-read, reversed for the current word,
        // normal for upcoming.
        let dim = Style::default().add_modifier(Modifier::DIM);
        let current = Style::default()
            .add_modifier(Modifier::REVERSED)
            .add_modifier(Modifier::BOLD);
        let mut spans: Vec<Span<'_>> = Vec::with_capacity(words.len() * 2);
        for (i, w) in words.iter().enumerate() {
            let style = if i < idx {
                dim
            } else if i == idx {
                current
            } else {
                Style::default()
            };
            spans.push(Span::styled(w.clone(), style));
            spans.push(Span::raw(" "));
        }

        // Reserve the bottom row for the footer.
        let body_rect = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(2),
        };
        let footer_rect = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };

        f.render_widget(
            Paragraph::new(Line::from(spans)).wrap(Wrap { trim: true }),
            body_rect,
        );

        let remaining = crate::tui::reading_time::fmt_compact(
            crate::tui::reader_pace::remaining_secs(idx, total, *wpm),
        );
        let footer = if finished {
            format!(
                " done · {total} words @ {wpm} wpm · r restart · Esc close "
            )
        } else {
            format!(
                " {}/{} · {} left @ {} wpm · Space {} · ←→ step · r restart · Esc close ",
                idx.min(total),
                total,
                remaining,
                wpm,
                if *paused { "play" } else { "pause" },
            )
        };
        f.render_widget(
            Paragraph::new(Span::styled(
                footer,
                Style::default().add_modifier(Modifier::DIM),
            )),
            footer_rect,
        );
    }
}

/// Truncate `s` to at most `max` characters,
/// appending an ellipsis when truncation happens.
/// Unicode-safe.
fn truncate_to(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else if max <= 1 {
        s.chars().take(max).collect()
    } else {
        let mut out: String = s.chars().take(max - 1).collect();
        out.push('…');
        out
    }
}

/// 1.2.14+ Phase Q.4a — text progress bar for
/// the project goal modal.  `pct` clamped to
/// 0..=100.  Width is the on-screen cell count
/// (not character count of the resulting string;
/// the bar is ASCII-only so they're equal).
fn progress_bar(pct: u32, cells: u16) -> String {
    let cells = cells.max(8) as usize;
    let filled = ((pct as usize).min(100) * cells) / 100;
    let empty = cells.saturating_sub(filled);
    let mut s = String::with_capacity(cells + 4);
    s.push_str("   ");
    for _ in 0..filled {
        s.push('█');
    }
    for _ in 0..empty {
        s.push('░');
    }
    s
}

#[cfg(test)]
mod truncate_tests {
    use super::truncate_to;

    #[test]
    fn keeps_short_strings_intact() {
        assert_eq!(truncate_to("aiya", 10), "aiya");
        assert_eq!(truncate_to("", 5), "");
    }

    #[test]
    fn truncates_with_ellipsis() {
        assert_eq!(truncate_to("inheritance subplot", 10), "inheritan…");
    }

    #[test]
    fn handles_max_one_or_zero() {
        assert_eq!(truncate_to("aiya", 1), "a");
        assert_eq!(truncate_to("aiya", 0), "");
    }

    #[test]
    fn unicode_safe_no_byte_split() {
        // Cyrillic — each char is 2 bytes in UTF-8.
        // Slice-by-byte would panic; we slice by
        // char-count.
        assert_eq!(truncate_to("Москва", 4), "Мос…");
    }
}

/// 1.2.8+ — restart-required overlay painted on top of
/// the HJSON editor modal after a Ctrl+S save whose
/// written bytes differ from the pre-open original.
/// Informational only; the user dismisses with any key
/// (handled at the App level) and continues editing.
/// Restart is on the next manual relaunch — the modal
/// can't restart the process itself.
/// 1.2.9+ — map a daily word count to a (glyph, color)
/// pair for the writing-streak heatmap.  Five buckets:
///   0:        `·` dim gray         (no activity)
///   1-249:    `░` faint green      (light)
///   250-499:  `▒` medium green     (steady)
///   500-999:  `▓` bright green     (productive)
///   1000+:    `█` max green        (heavy)
/// The buckets bracket common writing-session sizes
/// (one paragraph ~ 250 words, one scene ~ 500 words,
/// one chapter ~ 1500 words).
fn heat_glyph_and_color(words: i64) -> (&'static str, Color) {
    if words <= 0 {
        ("·", Color::DarkGray)
    } else if words < 250 {
        ("░", Color::Rgb(0x40, 0xa0, 0x40))
    } else if words < 500 {
        ("▒", Color::Rgb(0x60, 0xc0, 0x60))
    } else if words < 1000 {
        ("▓", Color::Rgb(0x40, 0xe0, 0x40))
    } else {
        ("█", Color::Rgb(0x80, 0xff, 0x80))
    }
}

fn draw_hjson_restart_overlay(f: &mut ratatui::Frame, host: Rect) {
    let lines: Vec<Line<'_>> = vec![
        Line::from(Span::styled(
            "Config changed",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::raw(
            "inkhaven.hjson has been written to disk.",
        )),
        Line::from(Span::raw(
            "The running editor is still using the OLD config —",
        )),
        Line::from(Span::raw(
            "restart inkhaven to apply your changes.",
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Press any key to dismiss",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )),
    ];

    let content_w = 56u16.min(host.width.saturating_sub(4));
    let content_h = (lines.len() as u16 + 2).min(host.height.saturating_sub(2));
    let x = host.x + host.width.saturating_sub(content_w) / 2;
    let y = host.y + host.height.saturating_sub(content_h) / 2;
    let overlay = Rect { x, y, width: content_w, height: content_h };
    f.render_widget(ratatui::widgets::Clear, overlay);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Restart required ")
        .border_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(overlay);
    f.render_widget(block, overlay);
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        inner,
    );
}

/// 1.2.8+ — `Ctrl+B H` help overlay painted on top of the
/// OS Shell pane.  Centered box, ~70% of the pane width,
/// listing chord shortcuts + a one-paragraph introduction
/// to what the embedded shell does.  Dismissed by any key
/// (handler-level), preserves the underlying pane state.
fn draw_shell_help_overlay(f: &mut ratatui::Frame, host: Rect) {
    let lines: Vec<Line<'_>> = vec![
        Line::from(Span::styled(
            "OS Shell — quick reference",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::raw(
            "Embedded nushell in-process.  Pipelines, env vars,",
        )),
        Line::from(Span::raw(
            "and `def` declarations persist while the pane is open.",
        )),
        Line::from(Span::raw(
            "Externals are spawned with stdin=null and a captured",
        )),
        Line::from(Span::raw(
            "stdout/stderr pipe — not a real TTY, so full-screen",
        )),
        Line::from(Span::raw(
            "apps (vim, less, top, tmux, …) are refused before spawn.",
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Line editing",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::raw(" Ctrl+A / Ctrl+E    home / end of line")),
        Line::from(Span::raw(" Ctrl+U / Ctrl+K    kill to start / end")),
        Line::from(Span::raw(" Ctrl+W             kill word backward")),
        Line::from(Span::raw(" Alt+B / Alt+F      word back / forward")),
        Line::from(Span::raw(" Ctrl+Left/Right    word back / forward")),
        Line::from(Span::raw(" Ctrl+L             clear scrollback")),
        Line::from(Span::raw(" Ctrl+D             clear input (or close if empty)")),
        Line::from(Span::raw(" Tab                autocomplete commands / paths")),
        Line::from(""),
        Line::from(Span::styled(
            "Pane chords",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::raw(" Enter              run the line")),
        Line::from(Span::raw(" ↑ / ↓              walk command history")),
        Line::from(Span::raw(" PgUp / PgDn        scroll turn buffer")),
        Line::from(Span::raw(" Shift+Home / End   jump scrollback top/bottom")),
        Line::from(Span::raw(" Ctrl+Z h           selection mode (copy/insert turns)")),
        Line::from(Span::raw(" Ctrl+Z o           close pane (state preserved)")),
        Line::from(Span::raw(" Ctrl+Z O           close + drop engine (fresh on reopen)")),
        Line::from(Span::raw(" Ctrl+B H           this help")),
        Line::from(Span::raw(" exit / quit / Esc  close pane")),
        Line::from(""),
        Line::from(Span::styled(
            "Sample nu commands",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::raw(" ls                       list files as a table")),
        Line::from(Span::raw(" ls | where size > 1MB    filter the table")),
        Line::from(Span::raw(" cd subdir                change cwd (env persists)")),
        Line::from(Span::raw(" let x = 42              bind a variable")),
        Line::from(Span::raw(" help commands           every built-in command")),
        Line::from(Span::raw(" ^/bin/echo hello        run an external explicitly")),
        Line::from(""),
        Line::from(Span::styled(
            "Press any key to close",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )),
    ];

    // Center the overlay inside the host rect.  Width fixed
    // at 64 (or 90% of host, whichever is smaller); height
    // matches the content line count plus borders.
    let content_w = 64u16.min(host.width.saturating_sub(4));
    let content_h = (lines.len() as u16 + 2).min(host.height.saturating_sub(2));
    let x = host.x + host.width.saturating_sub(content_w) / 2;
    let y = host.y + host.height.saturating_sub(content_h) / 2;
    let overlay = Rect { x, y, width: content_w, height: content_h };

    f.render_widget(ratatui::widgets::Clear, overlay);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" OS Shell help ")
        .border_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(overlay);
    f.render_widget(block, overlay);
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        inner,
    );
}
