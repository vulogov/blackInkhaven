//! 1.2.14+ Phase C.1 — inline comments handlers
//! for the `tui::app` module.
//!
//! `Ctrl+V c` resolves an anchor span from the
//! textarea's current selection (or word at
//! cursor), pops a multi-line comment editor
//! modal.  On commit, the new `Comment` is
//! appended to the open paragraph's sidecar JSON
//! file + the in-memory `OpenedDoc.comments`
//! cache.
//!
//! See `Documentation/PROPOSALS/1.2.14_PLAN.md`
//! §4 for the full design.

use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Modifier, Style};
use tui_textarea::{Input, Key, TextArea};
use uuid::Uuid;

use super::super::comments::{
    self, comment_at_cursor, Comment,
};
use super::super::modal::{CommentsPanelEntry, Modal};
use super::super::input::TextInput;
use super::App;

impl App {
    /// 1.2.14+ Phase C.1 — `Ctrl+V c` handler.
    /// Resolves the anchor span from the textarea
    /// state, builds a snippet preview, opens the
    /// `Modal::CommentEditor`.
    pub(super) fn start_add_comment(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "comment: no paragraph open".into();
            return;
        };
        if doc.read_only {
            self.status = "comment: paragraph is read-only".into();
            return;
        }
        let lines = doc.textarea.lines().to_vec();
        let cursor = doc.textarea.cursor();
        let selection = doc.textarea.selection_range();
        let Some((char_start, char_end)) = comments::derive_anchor_span(
            &lines,
            cursor,
            selection,
        ) else {
            self.status = "comment: place cursor on a word OR select a span".into();
            return;
        };
        let anchor_preview =
            anchor_text_preview(&lines, char_start, char_end);
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(
            Style::default().add_modifier(Modifier::REVERSED),
        );
        self.modal = Modal::CommentEditor {
            textarea,
            anchor_start: char_start,
            anchor_end: char_end,
            anchor_preview,
            paragraph_id: doc.id,
        };
        self.status = "comment · Ctrl+S commit · Esc cancel".into();
    }

    /// 1.2.14+ Phase C.1 — modal key handler.
    pub(super) fn comment_editor_handle_key(
        &mut self,
        key: KeyEvent,
    ) -> bool {
        // Borrow split: handle commit / cancel up
        // here so the borrow on `self.modal` is
        // released before we mutate `self.opened`.
        let action = {
            let Modal::CommentEditor { textarea, .. } = &mut self.modal else {
                return false;
            };
            match (key.code, key.modifiers) {
                (KeyCode::Esc, _) => {
                    if textarea.lines().iter().all(|l| l.is_empty()) {
                        CommentEditorAction::Cancel
                    } else {
                        CommentEditorAction::None
                    }
                }
                (KeyCode::Char('s'), m) if m.contains(KeyModifiers::CONTROL) => {
                    CommentEditorAction::Commit
                }
                (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => {
                    // Ctrl+C cancels unconditionally
                    // (mirrors the common terminal
                    // convention for abandoning a
                    // prompt).
                    CommentEditorAction::Cancel
                }
                _ => {
                    // Forward every other keystroke
                    // to the textarea so the user can
                    // type normally.
                    let input = Input::from(key);
                    if input.key != Key::Null {
                        textarea.input(input);
                    }
                    CommentEditorAction::None
                }
            }
        };
        match action {
            CommentEditorAction::Cancel => {
                self.modal = Modal::None;
                self.status = "comment cancelled".into();
                true
            }
            CommentEditorAction::Commit => {
                self.commit_comment_editor();
                true
            }
            CommentEditorAction::None => true,
        }
    }

    fn commit_comment_editor(&mut self) {
        let (paragraph_id, anchor_start, anchor_end, body) =
            match std::mem::replace(&mut self.modal, Modal::None) {
                Modal::CommentEditor {
                    textarea,
                    anchor_start,
                    anchor_end,
                    paragraph_id,
                    ..
                } => {
                    let body = textarea.lines().join("\n");
                    (paragraph_id, anchor_start, anchor_end, body)
                }
                other => {
                    self.modal = other;
                    return;
                }
            };
        if body.trim().is_empty() {
            self.status = "comment: empty body — nothing saved".into();
            return;
        }
        let Some(doc) = self.opened.as_mut() else {
            self.status = "comment: paragraph closed before commit".into();
            return;
        };
        if doc.id != paragraph_id {
            self.status = "comment: paragraph changed before commit".into();
            return;
        }
        // Resolve author from config (if the
        // editor.comment_author field exists in
        // HJSON) or env-fallback.
        let configured = self.cfg.editor.comment_author.clone();
        let author = comments::resolve_author(configured.as_deref());
        let new_comment = Comment {
            id: Uuid::new_v4(),
            char_start: anchor_start,
            char_end: anchor_end,
            author,
            created_at: Utc::now(),
            resolved: false,
            resolved_at: None,
            text: body,
            replies: Vec::new(),
        };
        doc.comments.comments.push(new_comment);
        // Persist the sidecar alongside the .typ
        // file.  Errors are surfaced to the status
        // bar but don't roll back the in-memory
        // append — the user can re-trigger the save
        // by adding another comment and the whole
        // file rewrites.
        let abs = self.layout.root.join(&doc.rel_path);
        if let Err(e) = comments::save_to_sidecar(&abs, &doc.comments) {
            self.status = format!("comment saved in memory — sidecar write failed: {e}");
        } else {
            self.status = format!(
                "comment saved ({} total on this ¶)",
                doc.comments.comments.len()
            );
        }
    }

    /// 1.2.14+ Phase C.1 — render-time helper.  Returns the
    /// footer chip text for the open paragraph when the
    /// cursor lands inside a commented span; otherwise
    /// `None`.  Drives the same one-row footer slot the
    /// Language hit chip + goal gauge share.
    pub(super) fn comment_at_cursor_chip(&self) -> Option<String> {
        let doc = self.opened.as_ref()?;
        if doc.comments.comments.is_empty() {
            return None;
        }
        let lines = doc.textarea.lines().to_vec();
        let cursor = doc.textarea.cursor();
        let idx = comment_at_cursor(&lines, &doc.comments.comments, cursor)?;
        let c = &doc.comments.comments[idx];
        let snippet: String = c.text.chars().take(60).collect();
        let snippet = if c.text.chars().count() > 60 {
            format!("{snippet}…")
        } else {
            snippet
        };
        let age = humanise_age(c.created_at);
        let resolved = if c.resolved { " [resolved]" } else { "" };
        Some(format!(
            "[{} · {} · {age}{resolved}] {snippet}",
            c.author.chars().take(20).collect::<String>(),
            comment_span_label(c.char_end - c.char_start),
        ))
    }
}

enum CommentEditorAction {
    Cancel,
    Commit,
    None,
}

/// Pull the underlying paragraph text under the
/// anchor span for the modal's "you're commenting
/// on" header.  Caps at 80 chars; collapses
/// internal newlines to ` ⏎ ` so the modal stays
/// single-line.
fn anchor_text_preview(
    lines: &[String],
    char_start: usize,
    char_end: usize,
) -> String {
    let body = lines.join("\n");
    let chars: Vec<char> = body.chars().collect();
    let end = char_end.min(chars.len());
    if char_start >= end {
        return String::new();
    }
    let raw: String = chars[char_start..end].iter().collect();
    let collapsed = raw.replace('\n', " ⏎ ");
    if collapsed.chars().count() > 80 {
        let head: String = collapsed.chars().take(79).collect();
        format!("{head}…")
    } else {
        collapsed
    }
}

fn comment_span_label(char_count: usize) -> String {
    if char_count == 1 {
        "1 char".into()
    } else {
        format!("{char_count} chars")
    }
}

pub(crate) fn humanise_age(when: chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let delta = now.signed_duration_since(when);
    let secs = delta.num_seconds();
    if secs < 60 {
        "just now".into()
    } else if secs < 3600 {
        format!("{}m ago", delta.num_minutes())
    } else if secs < 86_400 {
        format!("{}h ago", delta.num_hours())
    } else if secs < 86_400 * 30 {
        format!("{}d ago", delta.num_days())
    } else {
        when.format("%Y-%m-%d").to_string()
    }
}

impl App {
    /// 1.2.14+ Phase C.2 — `Ctrl+V Shift+C` handler.
    /// Walks every paragraph in the hierarchy +
    /// loads its sidecar; pops the panel with all
    /// comments listed (filtered to unresolved by
    /// default).
    pub(super) fn open_comments_panel(&mut self) {
        let entries = self.collect_all_comments_for_panel();
        if entries.is_empty() {
            self.status =
                "comments panel: no comments anywhere — Ctrl+V c to add one".into();
            return;
        }
        let total = entries.len();
        let visible: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.resolved)
            .map(|(i, _)| i)
            .collect();
        let unresolved = visible.len();
        self.modal = Modal::CommentsPanel {
            entries,
            cursor: 0,
            filter: TextInput::new(),
            filter_active: false,
            hide_resolved: true,
            visible,
        };
        self.status = format!(
            "comments · {unresolved} unresolved · {} hidden resolved · ↑↓ Enter open · r resolve · R show resolved · d delete · / filter · Esc",
            total.saturating_sub(unresolved)
        );
    }

    fn collect_all_comments_for_panel(&self) -> Vec<CommentsPanelEntry> {
        use crate::store::node::NodeKind;
        let mut out: Vec<CommentsPanelEntry> = Vec::new();
        for node in self.hierarchy.iter() {
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            let Some(rel) = &node.file else { continue; };
            let typ_abs = self.layout.root.join(rel);
            let file = match comments::load_from_sidecar(&typ_abs) {
                Ok(f) => f,
                Err(_) => continue,
            };
            if file.comments.is_empty() {
                continue;
            }
            let total = file.comments.len();
            let breadcrumb = self.hierarchy.slug_path(node);
            for (idx, c) in file.comments.iter().enumerate() {
                out.push(CommentsPanelEntry {
                    paragraph_id: node.id,
                    paragraph_breadcrumb: breadcrumb.clone(),
                    typ_abs_path: typ_abs.clone(),
                    comment_index: idx,
                    author: c.author.clone(),
                    created_at: c.created_at,
                    resolved: c.resolved,
                    text: c.text.clone(),
                    char_start: c.char_start,
                    char_end: c.char_end,
                    paragraph_total_comments: total,
                });
            }
        }
        // Newest first within each paragraph; older
        // groups bubble down.  Trivially sortable
        // here because the panel never re-sorts at
        // runtime.
        out.sort_by(|a, b| {
            a.paragraph_breadcrumb
                .cmp(&b.paragraph_breadcrumb)
                .then(b.created_at.cmp(&a.created_at))
        });
        out
    }

    /// Recompute `visible` after a filter / hide-
    /// resolved edit.
    fn comments_panel_refilter(&mut self) {
        let Modal::CommentsPanel {
            entries,
            cursor,
            filter,
            visible,
            hide_resolved,
            ..
        } = &mut self.modal
        else {
            return;
        };
        let f = filter.as_str().to_lowercase();
        let f = f.trim();
        *visible = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                if *hide_resolved && e.resolved {
                    return false;
                }
                if f.is_empty() {
                    return true;
                }
                e.text.to_lowercase().contains(f)
                    || e.author.to_lowercase().contains(f)
                    || e.paragraph_breadcrumb.to_lowercase().contains(f)
            })
            .map(|(i, _)| i)
            .collect();
        if *cursor >= visible.len() {
            *cursor = visible.len().saturating_sub(1);
        }
    }

    /// 1.2.14+ Phase C.2 — panel key handler.
    pub(super) fn comments_panel_handle_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> bool {
        use crossterm::event::KeyCode;
        let Modal::CommentsPanel {
            entries,
            cursor,
            filter,
            filter_active,
            hide_resolved,
            visible,
        } = &mut self.modal
        else {
            return false;
        };
        // ── filter-input mode ──────────────────
        if *filter_active {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => {
                    *filter_active = false;
                    return true;
                }
                KeyCode::Backspace => {
                    filter.backspace();
                    self.comments_panel_refilter();
                    return true;
                }
                KeyCode::Char(c) => {
                    filter.insert_char(c);
                    self.comments_panel_refilter();
                    return true;
                }
                _ => return true,
            }
        }
        // ── navigation mode ───────────────────
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
            KeyCode::Home => {
                *cursor = 0;
                true
            }
            KeyCode::End => {
                *cursor = visible_len.saturating_sub(1);
                true
            }
            KeyCode::Char('/') => {
                *filter_active = true;
                true
            }
            KeyCode::Esc => {
                self.modal = Modal::None;
                true
            }
            KeyCode::Enter => {
                let target = visible
                    .get(*cursor)
                    .and_then(|i| entries.get(*i))
                    .map(|e| (e.paragraph_id, e.char_start));
                self.modal = Modal::None;
                if let Some((id, _char_start)) = target {
                    if let Some(node) = self.hierarchy.get(id).cloned() {
                        let _ = self.load_paragraph(&node);
                        // Phase C.2.1 candidate:
                        // jump the cursor to
                        // char_start via
                        // textarea.move_cursor.
                    }
                }
                true
            }
            KeyCode::Char('r') => {
                self.comments_panel_resolve_current(true);
                true
            }
            KeyCode::Char('R') => {
                *hide_resolved = !*hide_resolved;
                self.comments_panel_refilter();
                true
            }
            KeyCode::Char('d') => {
                self.comments_panel_delete_current();
                true
            }
            KeyCode::Char('u') => {
                // Unresolve — flip resolved comment
                // back to unresolved.
                self.comments_panel_resolve_current(false);
                true
            }
            _ => false,
        }
    }

    fn comments_panel_resolve_current(&mut self, resolve: bool) {
        let entry = {
            let Modal::CommentsPanel {
                entries,
                cursor,
                visible,
                ..
            } = &self.modal
            else {
                return;
            };
            let Some(src_idx) = visible.get(*cursor) else {
                return;
            };
            entries.get(*src_idx).cloned()
        };
        let Some(entry) = entry else { return; };
        // Re-load the sidecar from disk so we
        // don't clobber an out-of-process edit.
        let mut file = match comments::load_from_sidecar(&entry.typ_abs_path) {
            Ok(f) => f,
            Err(e) => {
                self.status = format!("comments: load sidecar: {e}");
                return;
            }
        };
        let Some(c) = file.comments.get_mut(entry.comment_index) else {
            self.status =
                "comments: sidecar drifted — reload the panel".into();
            return;
        };
        c.resolved = resolve;
        c.resolved_at = if resolve {
            Some(chrono::Utc::now())
        } else {
            None
        };
        if let Err(e) = comments::save_to_sidecar(&entry.typ_abs_path, &file) {
            self.status = format!("comments: save sidecar: {e}");
            return;
        }
        // Update the in-memory entry + refilter.
        if let Modal::CommentsPanel { entries, .. } = &mut self.modal {
            for e in entries.iter_mut() {
                if e.typ_abs_path == entry.typ_abs_path
                    && e.comment_index == entry.comment_index
                {
                    e.resolved = resolve;
                }
            }
        }
        // If the panel's open paragraph is the one
        // we just touched, refresh its in-memory
        // comments cache so the editor overlay
        // matches.
        if let Some(doc) = self.opened.as_mut() {
            if doc.id == entry.paragraph_id {
                doc.comments = file;
            }
        }
        self.comments_panel_refilter();
        self.status = if resolve {
            "comment resolved".into()
        } else {
            "comment reopened".into()
        };
    }

    fn comments_panel_delete_current(&mut self) {
        let entry = {
            let Modal::CommentsPanel {
                entries,
                cursor,
                visible,
                ..
            } = &self.modal
            else {
                return;
            };
            let Some(src_idx) = visible.get(*cursor) else {
                return;
            };
            entries.get(*src_idx).cloned()
        };
        let Some(entry) = entry else { return; };
        let mut file = match comments::load_from_sidecar(&entry.typ_abs_path) {
            Ok(f) => f,
            Err(e) => {
                self.status = format!("comments: load sidecar: {e}");
                return;
            }
        };
        if entry.comment_index >= file.comments.len() {
            self.status =
                "comments: sidecar drifted — reload the panel".into();
            return;
        }
        file.comments.remove(entry.comment_index);
        if let Err(e) = comments::save_to_sidecar(&entry.typ_abs_path, &file) {
            self.status = format!("comments: save sidecar: {e}");
            return;
        }
        // Refresh open-doc cache.
        if let Some(doc) = self.opened.as_mut() {
            if doc.id == entry.paragraph_id {
                doc.comments = file;
            }
        }
        // Rebuild the panel entries (indices
        // shifted) — simpler than walking and
        // patching in place.
        let fresh = self.collect_all_comments_for_panel();
        if let Modal::CommentsPanel {
            entries,
            cursor,
            visible,
            hide_resolved,
            filter,
            ..
        } = &mut self.modal
        {
            *entries = fresh;
            *cursor = (*cursor).min(entries.len().saturating_sub(1));
            // Reapply filter.
            let f = filter.as_str().to_lowercase();
            let f = f.trim().to_string();
            *visible = entries
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    if *hide_resolved && e.resolved {
                        return false;
                    }
                    if f.is_empty() {
                        return true;
                    }
                    e.text.to_lowercase().contains(&f)
                        || e.author.to_lowercase().contains(&f)
                        || e.paragraph_breadcrumb.to_lowercase().contains(&f)
                })
                .map(|(i, _)| i)
                .collect();
            if *cursor >= visible.len() {
                *cursor = visible.len().saturating_sub(1);
            }
        }
        self.status = "comment deleted".into();
    }
}
