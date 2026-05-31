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
use super::super::modal::Modal;
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

fn humanise_age(when: chrono::DateTime<chrono::Utc>) -> String {
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
