//! Rendering core for `App`. Houses the `draw_modal` dispatcher
//! (the big match that routes each `Modal` variant to its painter)
//! and the `render_template` text-substitution helper. The actual
//! painters live in two child sub-modules — see `modals` for
//! every `draw_*_modal` (overlays, pickers, prompts) and `panes`
//! for the four pane painters + status / footer / search chrome.
//! Originally extracted from `tui::app` in 1.2.7 Phase 3; further
//! split in Phase 4.

mod modals;
mod panes;

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};


use crate::store::InsertPosition;

use super::super::modal::Modal;

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
        if matches!(self.modal, Modal::KillRingPicker { .. }) {
            self.draw_kill_ring_picker_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::ShellPane { .. }) {
            self.draw_shell_pane_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::HjsonEditor { .. }) {
            self.draw_hjson_editor_modal(f, area);
            return;
        }
        if matches!(self.modal, Modal::WritingStreakHeatmap { .. }) {
            self.draw_writing_streak_heatmap(f, area);
            return;
        }
        if matches!(self.modal, Modal::Concordance { .. }) {
            self.draw_concordance_modal(f, area);
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
            Modal::KillRingPicker { .. } =>
                unreachable!("kill-ring picker handled above"),
            Modal::ShellPane { .. } =>
                unreachable!("shell pane handled above"),
            Modal::HjsonEditor { .. } =>
                unreachable!("hjson editor handled above"),
            Modal::WritingStreakHeatmap { .. } =>
                unreachable!("writing-streak heatmap handled above"),
            Modal::Concordance { .. } =>
                unreachable!("concordance handled above"),
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
            Modal::TtsSaveAsAudio { input, voice_label, .. } => {
                let prompt = format!(
                    " Path: {}",
                    input.render_with_cursor('│'),
                );
                let body = vec![
                    Line::from(""),
                    Line::from(Span::raw(prompt)),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!(" voice: {voice_label}"),
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                    Line::from(Span::styled(
                        " Enter writes; Esc cancels. Parent dir is created if missing.",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (" Save paragraph as audio ".into(), Color::Cyan, body)
            }
            Modal::ConfirmQuit => {
                let body = vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        " Quit inkhaven?",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        " Open paragraph will autosave first.",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                    Line::from(Span::styled(
                        " y / Enter to confirm · n / Esc to cancel ",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (" Confirm quit ".into(), Color::Yellow, body)
            }
            Modal::TtsUnavailable { title, reason } => {
                let mut body: Vec<Line<'_>> = vec![Line::from("")];
                for line in reason.lines() {
                    body.push(Line::from(Span::raw(format!(" {line}"))));
                }
                body.push(Line::from(""));
                body.push(Line::from(Span::styled(
                    " Press any key to dismiss ",
                    Style::default().add_modifier(Modifier::DIM),
                )));
                (title.clone(), Color::Yellow, body)
            }
            Modal::TtsPlayback { started_at, preview, voice_label } => {
                let elapsed = started_at.elapsed();
                let elapsed_str = format!(
                    "{:02}:{:02}",
                    elapsed.as_secs() / 60,
                    elapsed.as_secs() % 60,
                );
                // Spinner driven by tenths-of-seconds so it
                // moves on every render frame.  Real progress
                // is unknowable (the TTS engine doesn't
                // expose per-word callbacks), so honesty
                // beats a fake percentage.
                let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                let idx = (elapsed.as_millis() / 100) as usize
                    % spinner_frames.len();
                let spinner = spinner_frames[idx];
                let mut preview_show = preview.clone();
                if preview_show.chars().count() >= 80 {
                    preview_show.push('…');
                }
                let body = vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::raw(" "),
                        Span::styled(
                            spinner,
                            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                        ),
                        Span::raw("  "),
                        Span::styled(
                            "reading aloud",
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(format!("  ·  voice: {voice_label}  ·  elapsed: {elapsed_str}")),
                    ]),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!(" \"{preview_show}\""),
                        Style::default().add_modifier(Modifier::DIM | Modifier::ITALIC),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        " Press any key (Esc / Space) to stop ",
                        Style::default().add_modifier(Modifier::DIM),
                    )),
                ];
                (" Read aloud ".into(), Color::Cyan, body)
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
                // 1.2.8+ — filter snapshots by annotation
                // substring.  Index `cursor` walks visible
                // entries only; the actions resolve via
                // `visible_snapshot_indices` (snapshot_impl).
                let visible = self.visible_snapshot_indices(snapshots);
                let header = format!(" Snapshots — {} ", paragraph_title);
                let mut body: Vec<Line> = Vec::with_capacity(visible.len() + 4);
                // Filter input row.  Always shown so the affordance
                // is discoverable.  Highlighted when focused.
                let filter_text = if self.snapshot_filter.is_empty() {
                    "/ (filter annotations)".to_string()
                } else {
                    format!("/ {}", self.snapshot_filter)
                };
                let filter_style = if self.snapshot_filter_focused {
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::Yellow)
                } else if self.snapshot_filter.is_empty() {
                    Style::default().add_modifier(Modifier::DIM)
                } else {
                    Style::default().fg(Color::Yellow)
                };
                body.push(Line::from(Span::styled(filter_text, filter_style)));
                if !self.snapshot_filter.is_empty() {
                    body.push(Line::from(Span::styled(
                        format!(
                            " ({} of {} snapshots match)",
                            visible.len(),
                            snapshots.len()
                        ),
                        Style::default().add_modifier(Modifier::DIM),
                    )));
                }
                body.push(Line::from(""));
                if visible.is_empty() {
                    body.push(Line::from(Span::styled(
                        "  (no snapshots match — clear with Esc)",
                        Style::default().add_modifier(Modifier::DIM),
                    )));
                }
                for (visible_i, abs_i) in visible.iter().enumerate() {
                    let snap = &snapshots[*abs_i];
                    let selected = visible_i == *cursor;
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
                let hint = if self.snapshot_filter_focused {
                    " filter mode: type to narrow · Backspace edits · Esc exits filter "
                } else {
                    " ↑↓ navigate · Enter loads · V diff vs current · D / Del delete · / filter · Esc cancel "
                };
                body.push(Line::from(Span::styled(
                    hint,
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

}
