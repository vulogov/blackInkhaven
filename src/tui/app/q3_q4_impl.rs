//! 1.2.14+ Phase Q.3 + Q.4 — handler wiring for
//! the continuation drafting / footnote insertion
//! / project-goal modal / style-transfer
//! workflows.
//!
//! Continuation drafting (`Ctrl+V d`) and style
//! transfer (`Ctrl+V y`) both share the
//! `Inference` streaming machinery the
//! translation + rewrite chords use; the helpers
//! are inline here rather than ai_impl.rs to
//! keep the per-phase grouping legible.

use chrono::NaiveDate;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Modifier, Style};
use tui_textarea::{Input, Key, TextArea};
use uuid::Uuid;

use super::super::inference::{Inference, InferenceStatus};
use super::super::modal::Modal;
use super::super::project_goal::{self, ProjectGoalData};
use super::App;

const CONTINUATION_BEGIN: &str = "<<<DRAFT>>>";
const REWRITE_BEGIN: &str = "<<<REWRITE>>>";
const COMMON_END: &str = "<<<END>>>";

impl App {
    // ─────────────────────────────────────────
    // Q.3a — AI continuation drafting (Ctrl+V d)
    // ─────────────────────────────────────────

    pub(super) fn start_continuation_draft(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "continuation: no paragraph open".into();
            return;
        };
        let body = doc.textarea.lines().join("\n");
        let title = doc.title.clone();
        let (cursor_row, cursor_col) = doc.textarea.cursor();
        // Build a marker-tagged source where the
        // model can see where to continue from.
        let marked_source = mark_cursor(&doc.textarea.lines(), cursor_row, cursor_col);
        let anchors = self.collect_continuation_anchors(doc.id);
        let envelope = compose_continuation_prompt(
            &title,
            &marked_source,
            &anchors,
            &self.cfg.language,
        );
        let _ = body;
        let (model, _env) = match self.ai.resolve_provider(&self.cfg.llm, None) {
            Ok(p) => p,
            Err(e) => {
                self.status = format!("continuation: {e}");
                return;
            }
        };
        let model = model.to_string();
        let provider = self.ai.default_provider.clone();
        let rx = super::super::super::ai::stream::spawn_chat_stream(
            self.ai.client.clone(),
            model.clone(),
            None,
            Vec::new(),
            envelope,
        );
        self.inference = Some(Inference {
            provider: provider.clone(),
            model,
            response: String::new(),
            status: InferenceStatus::Streaming,
            rx,
            started_at: std::time::Instant::now(),
        });
        self.pending_chat_user_msg = None;
        self.pending_continuation_draft = true;
        self.change_focus(super::super::focus::Focus::Ai);
        self.status =
            format!("continuation: streaming from {provider}…");
    }

    /// Walk back from the open paragraph through
    /// the hierarchy to collect the previous N
    /// paragraphs (configurable via
    /// `editor.continuation_anchor_count`,
    /// default 3) as voice anchors.  Returns
    /// in chronological (earliest-first) order so
    /// the prompt reads the voice progression.
    fn collect_continuation_anchors(
        &self,
        current_id: Uuid,
    ) -> Vec<(String, String)> {
        let n = self.cfg.editor.continuation_anchor_count.max(1);
        use crate::store::node::NodeKind;
        // Walk paragraphs in book hierarchy order;
        // collect every paragraph's id + title +
        // (body via store) into a Vec, find the
        // current position, take the N before.
        let mut paragraphs: Vec<(Uuid, String, String)> = Vec::new();
        for node in self.hierarchy.iter() {
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            // Skip system books — voice anchors
            // come only from manuscript prose.
            let mut cur = Some(node.id);
            let mut is_system = false;
            while let Some(id) = cur {
                if let Some(n) = self.hierarchy.get(id) {
                    if n.system_tag.is_some() {
                        is_system = true;
                        break;
                    }
                    cur = n.parent_id;
                } else {
                    break;
                }
            }
            if is_system {
                continue;
            }
            let body = match self.store.get_content(node.id) {
                Ok(Some(b)) => b,
                _ => continue,
            };
            let body_str =
                std::str::from_utf8(&body).unwrap_or("").to_string();
            paragraphs.push((node.id, node.title.clone(), body_str));
        }
        let Some(pos) = paragraphs.iter().position(|(id, _, _)| *id == current_id)
        else {
            return Vec::new();
        };
        let start = pos.saturating_sub(n);
        paragraphs[start..pos]
            .iter()
            .map(|(_, t, b)| (t.clone(), b.clone()))
            .collect()
    }

    // ─────────────────────────────────────────
    // Q.3b — Inline footnote (Ctrl+V f)
    // ─────────────────────────────────────────

    pub(super) fn start_insert_footnote(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "footnote: no paragraph open".into();
            return;
        };
        if doc.read_only {
            self.status = "footnote: paragraph is read-only".into();
            return;
        }
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(
            Style::default().add_modifier(Modifier::REVERSED),
        );
        self.modal = Modal::FootnoteEditor {
            textarea,
            paragraph_id: doc.id,
        };
        self.status = "footnote · Ctrl+S commit · Esc cancel".into();
    }

    pub(super) fn footnote_editor_handle_key(
        &mut self,
        key: KeyEvent,
    ) -> bool {
        let action = {
            let Modal::FootnoteEditor { textarea, .. } = &mut self.modal else {
                return false;
            };
            match (key.code, key.modifiers) {
                (KeyCode::Esc, _) => {
                    if textarea.lines().iter().all(|l| l.is_empty()) {
                        FootnoteAction::Cancel
                    } else {
                        FootnoteAction::None
                    }
                }
                (KeyCode::Char('s'), m) if m.contains(KeyModifiers::CONTROL) => {
                    FootnoteAction::Commit
                }
                (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => {
                    FootnoteAction::Cancel
                }
                _ => {
                    let input = Input::from(key);
                    if input.key != Key::Null {
                        textarea.input(input);
                    }
                    FootnoteAction::None
                }
            }
        };
        match action {
            FootnoteAction::Cancel => {
                self.modal = Modal::None;
                self.status = "footnote cancelled".into();
                true
            }
            FootnoteAction::Commit => {
                self.commit_footnote();
                true
            }
            FootnoteAction::None => true,
        }
    }

    fn commit_footnote(&mut self) {
        let (paragraph_id, body) = match std::mem::replace(
            &mut self.modal,
            Modal::None,
        ) {
            Modal::FootnoteEditor { textarea, paragraph_id } => {
                let body = textarea.lines().join("\n");
                (paragraph_id, body)
            }
            other => {
                self.modal = other;
                return;
            }
        };
        if body.trim().is_empty() {
            self.status = "footnote: empty body — nothing inserted".into();
            return;
        }
        let style = self.cfg.editor.footnote_style.to_lowercase();
        let style = style.trim();
        let Some(doc) = self.opened.as_mut() else {
            self.status = "footnote: paragraph closed before commit".into();
            return;
        };
        if doc.id != paragraph_id {
            self.status = "footnote: paragraph changed before commit".into();
            return;
        }
        match style {
            "markdown" => {
                // Inline `[^id]` marker + a trailing
                // reference line.  ID = first six
                // hex chars of a fresh UUID v7 so
                // multiple footnotes on the same
                // paragraph don't collide.
                let id: String =
                    Uuid::new_v4().to_string().chars().take(6).collect();
                let marker = format!("[^{id}]");
                doc.textarea.insert_str(&marker);
                use tui_textarea::CursorMove;
                doc.textarea.move_cursor(CursorMove::Bottom);
                doc.textarea.move_cursor(CursorMove::End);
                doc.textarea.insert_str(&format!("\n\n[^{id}]: {body}"));
            }
            _ => {
                // Default: Typst `#footnote[…]`
                // inline.
                let snippet = format!("#footnote[{body}]");
                doc.textarea.insert_str(&snippet);
            }
        }
        doc.dirty = true;
        self.status = format!("footnote inserted ({style})");
    }

    // ─────────────────────────────────────────
    // Q.4a — Project goal modal (Ctrl+V Shift+G)
    // ─────────────────────────────────────────

    pub(super) fn open_project_goal_modal(&mut self) {
        let data = self.compute_project_goal_data();
        self.modal = Modal::ProjectGoalModal { data };
        self.status = "project goal · Esc closes".into();
    }

    pub(super) fn project_goal_handle_key(&mut self, key: KeyEvent) -> bool {
        if matches!(key.code, KeyCode::Esc | KeyCode::Enter) {
            self.modal = Modal::None;
            return true;
        }
        // Swallow other input so the modal stays
        // up.
        true
    }

    fn compute_project_goal_data(&self) -> ProjectGoalData {
        use crate::store::node::NodeKind;
        let goal = self.cfg.project.word_count_goal;
        let target = parse_iso_date(&self.cfg.project.target_date);
        let counted = self
            .cfg
            .project
            .counted_books
            .iter()
            .map(|s| s.to_lowercase())
            .collect::<Vec<_>>();
        // Walk every user book; per-book count =
        // sum of paragraph word counts.
        let mut per_book: Vec<(String, u64)> = Vec::new();
        for book in self.hierarchy.children_of(None) {
            if book.kind != NodeKind::Book {
                continue;
            }
            if book.system_tag.is_some() {
                continue;
            }
            if !counted.is_empty()
                && !counted.iter().any(|n| n.eq_ignore_ascii_case(&book.title))
            {
                continue;
            }
            let mut total = 0u64;
            for id in self.hierarchy.collect_subtree(book.id) {
                let Some(n) = self.hierarchy.get(id) else { continue; };
                if n.kind == NodeKind::Paragraph {
                    total += n.word_count;
                }
            }
            per_book.push((book.title.clone(), total));
        }
        let total_words: u64 = per_book.iter().map(|(_, w)| *w).sum();
        let per_book_rows: Vec<(String, u64, u32)> = per_book
            .into_iter()
            .map(|(title, words)| {
                let pct = if total_words == 0 {
                    0
                } else {
                    ((words * 100) / total_words) as u32
                };
                (title, words, pct)
            })
            .collect();
        let pct = if goal == 0 {
            0
        } else {
            (((total_words.min(u64::MAX as u64) as u128) * 100)
                / (goal as u128)) as u32
        };
        let pct = pct.min(999);
        let remaining = goal.saturating_sub(total_words);
        let today = project_goal::today_local();
        let days_remaining =
            target.map(|t| (t - today).num_days());
        let required_per_day = match (days_remaining, remaining) {
            (Some(d), r) if d > 0 && r > 0 => Some(((r as i64 + d - 1) / d) as u64),
            _ => None,
        };
        let recent_avg = self.compute_recent_words_per_day();
        let projection_date = match (recent_avg, remaining) {
            (Some(avg), r) if avg > 0 && r > 0 => {
                let days_needed = (r + avg - 1) / avg;
                Some(today + chrono::Duration::days(days_needed as i64))
            }
            _ => None,
        };
        let verdict = project_goal::verdict_for(
            total_words,
            goal,
            target,
            projection_date,
        );
        ProjectGoalData {
            total_words,
            goal,
            pct,
            remaining,
            days_remaining,
            required_per_day,
            recent_avg,
            projection_date,
            per_book: per_book_rows,
            verdict,
        }
    }

    fn compute_recent_words_per_day(&self) -> Option<u64> {
        // 1.2.14+ Phase D.2 — read the existing
        // ProgressSnapshot sparkline (last 30 days,
        // oldest first, project-wide).  Average
        // the non-zero days so a stretch of vacation
        // doesn't drag the rate down to "you'll
        // never finish" status.  Returns `None`
        // when the cache hasn't built yet (no save
        // events recorded) or every day is zero.
        let snap = self.progress_cache.as_ref()?;
        if snap.sparkline.is_empty() {
            return None;
        }
        let mut sum: i64 = 0;
        let mut active_days = 0u64;
        for delta in &snap.sparkline {
            if *delta > 0 {
                sum += *delta;
                active_days += 1;
            }
        }
        if active_days == 0 {
            return None;
        }
        Some((sum as u64) / active_days)
    }

    // ─────────────────────────────────────────
    // Q.4b — Style transfer rewrite (Ctrl+V y)
    // ─────────────────────────────────────────

    pub(super) fn start_style_transfer_picker(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "style xfer: no paragraph open".into();
            return;
        };
        let target_id = doc.id;
        // Scope: every paragraph in the current
        // book (the one the open paragraph lives in).
        use crate::store::node::NodeKind;
        let mut book_id: Option<Uuid> = None;
        let mut cur = Some(target_id);
        while let Some(id) = cur {
            if let Some(node) = self.hierarchy.get(id) {
                if node.kind == NodeKind::Book {
                    book_id = Some(node.id);
                    break;
                }
                cur = node.parent_id;
            } else {
                break;
            }
        }
        let Some(book_id) = book_id else {
            self.status = "style xfer: no containing book".into();
            return;
        };
        let mut entries: Vec<(Uuid, String)> = Vec::new();
        for id in self.hierarchy.collect_subtree(book_id) {
            if id == target_id {
                continue;
            }
            let Some(node) = self.hierarchy.get(id) else { continue; };
            if node.kind == NodeKind::Paragraph {
                entries.push((node.id, node.title.clone()));
            }
        }
        if entries.is_empty() {
            self.status =
                "style xfer: no other paragraphs in this book".into();
            return;
        }
        let visible: Vec<usize> = (0..entries.len()).collect();
        self.modal = Modal::StyleTransferPicker {
            entries,
            cursor: 0,
            filter: super::super::input::TextInput::new(),
            filter_active: false,
            visible,
            target_paragraph_id: target_id,
        };
        self.status =
            "↑↓ Enter pick voice sample · / filter · Esc cancel".into();
    }

    pub(super) fn style_transfer_picker_handle_key(
        &mut self,
        key: KeyEvent,
    ) -> bool {
        let Modal::StyleTransferPicker {
            entries,
            cursor,
            filter,
            filter_active,
            visible,
            ..
        } = &mut self.modal
        else {
            return false;
        };
        if *filter_active {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => {
                    *filter_active = false;
                    return true;
                }
                KeyCode::Backspace => {
                    filter.backspace();
                    let f = filter.as_str().to_lowercase();
                    let f = f.trim().to_string();
                    let new_visible: Vec<usize> = entries
                        .iter()
                        .enumerate()
                        .filter(|(_, (_, t))| f.is_empty() || t.to_lowercase().contains(&f))
                        .map(|(i, _)| i)
                        .collect();
                    *visible = new_visible;
                    if *cursor >= visible.len() {
                        *cursor = visible.len().saturating_sub(1);
                    }
                    return true;
                }
                KeyCode::Char(c) => {
                    filter.insert_char(c);
                    let f = filter.as_str().to_lowercase();
                    let f = f.trim().to_string();
                    let new_visible: Vec<usize> = entries
                        .iter()
                        .enumerate()
                        .filter(|(_, (_, t))| f.is_empty() || t.to_lowercase().contains(&f))
                        .map(|(i, _)| i)
                        .collect();
                    *visible = new_visible;
                    if *cursor >= visible.len() {
                        *cursor = visible.len().saturating_sub(1);
                    }
                    return true;
                }
                _ => return true,
            }
        }
        let visible_len = visible.len();
        match key.code {
            KeyCode::Up => {
                if *cursor > 0 {
                    *cursor -= 1;
                }
                true
            }
            KeyCode::Down => {
                if *cursor + 1 < visible_len {
                    *cursor += 1;
                }
                true
            }
            KeyCode::Char('/') => {
                *filter_active = true;
                true
            }
            KeyCode::Esc => {
                self.modal = Modal::None;
                self.status = "style xfer cancelled".into();
                true
            }
            KeyCode::Enter => {
                self.commit_style_transfer_picker();
                true
            }
            _ => false,
        }
    }

    fn commit_style_transfer_picker(&mut self) {
        let (reference_id, target_id) =
            match std::mem::replace(&mut self.modal, Modal::None) {
                Modal::StyleTransferPicker {
                    entries,
                    cursor,
                    visible,
                    target_paragraph_id,
                    ..
                } => {
                    let Some(src_idx) = visible.get(cursor).copied() else { return; };
                    let Some((id, _)) = entries.get(src_idx).cloned() else { return; };
                    (id, target_paragraph_id)
                }
                other => {
                    self.modal = other;
                    return;
                }
            };
        // Load reference + target bodies.
        let Some(ref_node) = self.hierarchy.get(reference_id).cloned() else {
            self.status = "style xfer: reference paragraph vanished".into();
            return;
        };
        let Some(target_node) = self.hierarchy.get(target_id).cloned() else {
            self.status = "style xfer: target paragraph vanished".into();
            return;
        };
        let ref_body = self
            .store
            .get_content(ref_node.id)
            .ok()
            .flatten()
            .and_then(|b| String::from_utf8(b).ok())
            .unwrap_or_default();
        let target_body = self
            .store
            .get_content(target_node.id)
            .ok()
            .flatten()
            .and_then(|b| String::from_utf8(b).ok())
            .unwrap_or_default();
        let envelope = compose_style_transfer_prompt(
            &ref_node.title,
            &ref_body,
            &target_node.title,
            &target_body,
        );
        let (model, _env) = match self.ai.resolve_provider(&self.cfg.llm, None) {
            Ok(p) => p,
            Err(e) => {
                self.status = format!("style xfer: {e}");
                return;
            }
        };
        let model = model.to_string();
        let provider = self.ai.default_provider.clone();
        let rx = super::super::super::ai::stream::spawn_chat_stream(
            self.ai.client.clone(),
            model.clone(),
            None,
            Vec::new(),
            envelope,
        );
        self.inference = Some(Inference {
            provider: provider.clone(),
            model,
            response: String::new(),
            status: InferenceStatus::Streaming,
            rx,
            started_at: std::time::Instant::now(),
        });
        self.pending_chat_user_msg = None;
        self.pending_style_transfer = true;
        self.change_focus(super::super::focus::Focus::Ai);
        self.status =
            format!("style xfer: streaming from {provider}…");
    }
}

enum FootnoteAction {
    Cancel,
    Commit,
    None,
}

/// 1.2.14+ Phase Q.3 — marker the model uses to
/// understand where the cursor sat in the source.
const CURSOR_MARKER: &str = "[[CURSOR_HERE]]";

fn mark_cursor(lines: &[String], row: usize, col: usize) -> String {
    let mut out = String::new();
    for (r, line) in lines.iter().enumerate() {
        if r > 0 {
            out.push('\n');
        }
        if r == row {
            let chars: Vec<char> = line.chars().collect();
            let col = col.min(chars.len());
            let head: String = chars[..col].iter().collect();
            let tail: String = chars[col..].iter().collect();
            out.push_str(&head);
            out.push_str(CURSOR_MARKER);
            out.push_str(&tail);
        } else {
            out.push_str(line);
        }
    }
    out
}

fn compose_continuation_prompt(
    title: &str,
    source: &str,
    anchors: &[(String, String)],
    language: &str,
) -> String {
    let lang_hint = if language.trim().is_empty() {
        "the project's working language".to_string()
    } else {
        format!("`{language}`")
    };
    let anchors_text = if anchors.is_empty() {
        "(no prior paragraphs in this book — write fresh)".to_string()
    } else {
        anchors
            .iter()
            .map(|(t, b)| format!("── Anchor: {t} ──\n{b}"))
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    format!(
        "You are continuing a manuscript in the author's voice.  Write 2-4 \
         sentences continuing the open paragraph from the cursor position.\n\
         Use the prior paragraphs (below) as voice anchors — match sentence\n\
         length distribution, vocabulary register, narrative distance,\n\
         rhythm.  Do NOT introduce new plot facts the author hasn't already\n\
         set up.  Do NOT summarise; CONTINUE.\n\
         \n\
         Language: {lang_hint}.\n\
         \n\
         Wrap your draft between `<<<DRAFT>>>` and `<<<END>>>` markers on\n\
         lines by themselves.  Inside the block put ONLY the continuation\n\
         text — no commentary, no explanation, no quotation marks.\n\
         \n\
         ── Voice anchors (chronological, earliest first) ──\n\
         {anchors_text}\n\
         \n\
         ── Source paragraph: {title} ──\n\
         {source}\n\
         ── end source ──",
    )
}

fn compose_style_transfer_prompt(
    ref_title: &str,
    ref_body: &str,
    target_title: &str,
    target_body: &str,
) -> String {
    format!(
        "You are rewriting one paragraph in the style of a reference\n\
         paragraph.\n\
         \n\
         MATCH:\n\
           · sentence-length distribution\n\
           · vocabulary register\n\
           · narrative distance + voice\n\
           · rhythm + mood\n\
         \n\
         PRESERVE:\n\
           · literal meaning\n\
           · every named entity (characters, places, artefacts)\n\
           · every plot fact\n\
         \n\
         Wrap the rewritten paragraph between `<<<REWRITE>>>` and\n\
         `<<<END>>>` markers on lines by themselves.  Inside the block\n\
         put ONLY the rewritten paragraph — no commentary, no explanation.\n\
         \n\
         ── Reference paragraph (style source): {ref_title} ──\n\
         {ref_body}\n\
         \n\
         ── Target paragraph (to rewrite): {target_title} ──\n\
         {target_body}\n\
         ── end target ──",
    )
}

/// 1.2.14+ Phase Q.4 — parse `project.target_date`
/// (ISO 8601 `YYYY-MM-DD`).
fn parse_iso_date(raw: &str) -> Option<NaiveDate> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").ok()
}

/// 1.2.14+ Phase Q.3 — extract the continuation
/// draft block from a LLM response.  Mirrors
/// `extract_translation_text`.
pub(super) fn extract_continuation_text(response: &str) -> Option<String> {
    extract_marker_block(response, CONTINUATION_BEGIN)
}

/// 1.2.14+ Phase Q.4 — extract the rewritten
/// paragraph from a LLM response.
pub(super) fn extract_rewrite_text(response: &str) -> Option<String> {
    extract_marker_block(response, REWRITE_BEGIN)
}

fn extract_marker_block(response: &str, begin: &str) -> Option<String> {
    let start = response.find(begin)?;
    let after = &response[start + begin.len()..];
    let end_offset = after.find(COMMON_END)?;
    let inner = &after[..end_offset];
    let cleaned =
        inner.trim_matches(|c: char| c == '\n' || c == '\r' || c == ' ');
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_continuation_block() {
        let r = "preamble\n<<<DRAFT>>>\nShe walked into the rain.\n<<<END>>>\nnotes";
        assert_eq!(
            extract_continuation_text(r),
            Some("She walked into the rain.".to_string())
        );
    }

    #[test]
    fn extract_rewrite_block() {
        let r = "<<<REWRITE>>>\nIn the marketplace, fire.\n<<<END>>>";
        assert_eq!(
            extract_rewrite_text(r),
            Some("In the marketplace, fire.".to_string())
        );
    }

    #[test]
    fn extract_returns_none_on_missing_markers() {
        assert!(extract_continuation_text("no markers here").is_none());
        assert!(extract_rewrite_text("").is_none());
    }

    #[test]
    fn mark_cursor_inserts_marker_in_correct_position() {
        let lines = vec!["hello world".to_string()];
        let marked = mark_cursor(&lines, 0, 6);
        assert_eq!(marked, "hello [[CURSOR_HERE]]world");
    }

    #[test]
    fn parse_iso_date_handles_valid_and_invalid() {
        assert!(parse_iso_date("2026-09-01").is_some());
        assert!(parse_iso_date(" 2026-09-01 ").is_some());
        assert!(parse_iso_date("").is_none());
        assert!(parse_iso_date("not-a-date").is_none());
    }
}
