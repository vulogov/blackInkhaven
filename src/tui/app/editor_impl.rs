//! Editor-buffer methods on `App` — clipboard, smart auto-pair
//! / smart-delete behaviour, line-killing chords, session +
//! chat-history persistence, paragraph open / save / load,
//! hierarchy reload after store mutations, the editor-pane
//! `Block` builder, and the F7 paragraph-target modal pair.
//! Extracted from `tui::app` in the 1.2.7 refactor, Phase 3
//! batch 8.


use anyhow::Result;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders};

use super::super::focus::Focus;
use tui_textarea::{CursorMove, TextArea};
use uuid::Uuid;


use crate::ai::stream::ChatTurn;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};

use super::super::input::TextInput;
use super::super::lexicon_build::build_lexicon;
use super::super::modal::Modal;
use super::super::session::{
    EditorSession, ParagraphCursor, SessionState, TimelineViewSnapshot, TreeSession,
};
use super::super::state::OpenedDoc;
use super::super::text_utils::{
    body_to_lines, extract_first_sentence, PARAGRAPH_PLACEHOLDER_TITLE,
};

impl super::App {

    /// Insert `open` + `close` at the cursor and step back one
    /// character so the cursor sits between them.
    pub(super) fn editor_auto_open_pair(&mut self, open: char, close: char) {
        let Some(doc) = self.opened.as_mut() else { return };
        doc.textarea.insert_char(open);
        doc.textarea.insert_char(close);
        doc.textarea.move_cursor(CursorMove::Back);
        doc.dirty = true;
    }

    /// When the next char on the line is the same close character
    /// the user just typed, step over it instead of inserting a
    /// duplicate. Returns `false` when the next char doesn't match
    /// (caller falls through to normal insertion).
    pub(super) fn editor_try_skip_close(&mut self, close: char) -> bool {
        let Some(doc) = self.opened.as_mut() else { return false };
        let (row, col) = doc.textarea.cursor();
        let line = doc.textarea.lines().get(row).cloned().unwrap_or_default();
        let next_char = line.chars().nth(col);
        if next_char == Some(close) {
            doc.textarea.move_cursor(CursorMove::Forward);
            return true;
        }
        false
    }

    /// Enter pressed with the cursor between matching brackets:
    /// expand to a 3-line indented block. Returns `false` when the
    /// cursor isn't between a pair (caller does the regular Enter).
    pub(super) fn editor_try_expand_pair_on_enter(&mut self) -> bool {
        let Some(doc) = self.opened.as_ref() else { return false };
        let (row, col) = doc.textarea.cursor();
        let line = doc.textarea.lines().get(row).cloned().unwrap_or_default();
        let chars: Vec<char> = line.chars().collect();
        let before = if col > 0 { chars.get(col - 1).copied() } else { None };
        let after = chars.get(col).copied();
        if !matches!(
            (before, after),
            (Some('('), Some(')')) | (Some('['), Some(']')) | (Some('{'), Some('}'))
        ) {
            return false;
        }
        let base_indent: String = chars
            .iter()
            .take_while(|c| **c == ' ' || **c == '\t')
            .collect();
        let extra = " ".repeat(self.cfg.editor.tab_width.max(1));
        let new_indent = format!("{base_indent}{extra}");
        let Some(doc) = self.opened.as_mut() else { return false };
        // 1. Newline → cursor lands at column 0 of a new line with
        //    the close-bracket as its first char.
        doc.textarea.insert_char('\n');
        // 2. Type the deeper indent, then ANOTHER newline so the
        //    close-bracket slides further down. Cursor lands at
        //    column 0 of the close-bracket line.
        doc.textarea.insert_str(&new_indent);
        doc.textarea.insert_char('\n');
        // 3. Indent the close-bracket line with the base indent.
        doc.textarea.insert_str(&base_indent);
        // 4. Move up to the middle line, end of indent.
        doc.textarea.move_cursor(CursorMove::Up);
        doc.textarea.move_cursor(CursorMove::End);
        doc.dirty = true;
        true
    }

    /// Backspace when cursor sits between a freshly typed pair like
    /// `(|)`: delete BOTH halves. Returns `false` otherwise.
    pub(super) fn editor_try_delete_pair(&mut self) -> bool {
        let Some(doc) = self.opened.as_ref() else { return false };
        let (row, col) = doc.textarea.cursor();
        let line = doc.textarea.lines().get(row).cloned().unwrap_or_default();
        let chars: Vec<char> = line.chars().collect();
        let before = if col > 0 { chars.get(col - 1).copied() } else { None };
        let after = chars.get(col).copied();
        let is_pair = matches!(
            (before, after),
            (Some('('), Some(')'))
                | (Some('['), Some(']'))
                | (Some('{'), Some('}'))
                | (Some('"'), Some('"'))
                | (Some('\''), Some('\''))
        );
        if !is_pair {
            return false;
        }
        let Some(doc) = self.opened.as_mut() else { return false };
        doc.textarea.delete_next_char();
        doc.textarea.delete_char();
        doc.dirty = true;
        true
    }

    pub(super) fn editor_copy(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        doc.textarea.copy();
        let text = doc.textarea.yank_text();
        if !text.is_empty() {
            if let Some(cb) = self.clipboard.as_mut() {
                let _ = cb.set_text(text);
            }
        }
    }

    pub(super) fn editor_cut(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        if doc.textarea.cut() {
            let text = doc.textarea.yank_text();
            if !text.is_empty() {
                if let Some(cb) = self.clipboard.as_mut() {
                    let _ = cb.set_text(text);
                }
            }
            doc.dirty = true;
        }
    }

    pub(super) fn editor_paste(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        if let Some(cb) = self.clipboard.as_mut() {
            if let Ok(text) = cb.get_text() {
                doc.textarea.set_yank_text(text);
            }
        }
        if doc.textarea.paste() {
            doc.dirty = true;
        }
    }

    /// Delete the current line entirely (content + trailing newline). Cursor
    /// lands on the line that took its place. Preserves the yank buffer.
    pub(super) fn editor_delete_line(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        let saved_yank = doc.textarea.yank_text();
        // Move to start of line, clear to end, then remove the newline so
        // the next line collapses up.
        doc.textarea.move_cursor(CursorMove::Head);
        doc.textarea.delete_line_by_end();
        // delete_next_char removes the newline; on the last line it's a
        // no-op which leaves an empty line where the deleted one was. That's
        // an acceptable quirk — the user can hit Ctrl+D again to remove it,
        // or move up and delete the previous newline.
        doc.textarea.delete_next_char();
        doc.textarea.set_yank_text(saved_yank);
        doc.dirty = true;
    }

    /// Delete from the cursor to the end of the current line. Used by both
    /// Ctrl+E and Ctrl+Z (the user requested both bindings for the same op).
    pub(super) fn editor_delete_to_eol(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        let saved_yank = doc.textarea.yank_text();
        if doc.textarea.delete_line_by_end() {
            doc.dirty = true;
        }
        doc.textarea.set_yank_text(saved_yank);
    }

    /// Delete from the cursor back to the beginning of the current line.
    pub(super) fn editor_delete_to_bol(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        let saved_yank = doc.textarea.yank_text();
        if doc.textarea.delete_line_by_head() {
            doc.dirty = true;
        }
        doc.textarea.set_yank_text(saved_yank);
    }

    pub(super) fn editor_select_all(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        doc.textarea.move_cursor(CursorMove::Top);
        doc.textarea.start_selection();
        doc.textarea.move_cursor(CursorMove::Bottom);
        // CursorMove::Bottom lands at (last_row, 0). Without this End move
        // the selection would exclude the last line's content entirely.
        doc.textarea.move_cursor(CursorMove::End);
    }

    pub(super) fn save_session(&mut self) -> std::io::Result<()> {
        // Snapshot the live paragraph's cursor into the persistent map before
        // we serialise — otherwise an exit (or focus-loss session save) right
        // after a cursor move would lose the latest position.
        self.snapshot_open_paragraph_cursor();

        let cursor_id = self
            .rows
            .get(self.tree_cursor)
            .map(|(id, _)| id.to_string());
        let collapsed: Vec<String> = self
            .collapsed_nodes
            .iter()
            .map(|u| u.to_string())
            .collect();
        let editor_session = self.opened.as_ref().map(|d| {
            let (row, col) = d.textarea.cursor();
            EditorSession {
                opened_id: d.id.to_string(),
                cursor_row: row,
                cursor_col: col,
            }
        });
        let paragraph_cursors: std::collections::HashMap<String, ParagraphCursor> = self
            .paragraph_cursors
            .iter()
            .map(|(id, pc)| (id.to_string(), *pc))
            .collect();
        let visited_history: Vec<String> = self
            .visited_history
            .iter()
            .map(|u| u.to_string())
            .collect();
        // 1.2.7+ — also serialise any open timeline view's
        // state into the cache so the snapshot we persist
        // includes the user's CURRENT layout, not just the
        // last one they closed.
        if matches!(self.modal, Modal::TimelineView { .. }) {
            self.timeline_capture_view_state();
        }
        let timeline_views: std::collections::HashMap<String, TimelineViewSnapshot> =
            self.timeline_views
                .iter()
                .map(|(id, snap)| (id.to_string(), snap.clone()))
                .collect();
        let state = SessionState {
            tree: TreeSession {
                cursor_id,
                collapsed_nodes: collapsed,
            },
            editor: editor_session,
            focus: format!("{:?}", self.focus),
            paragraph_cursors,
            visited_history,
            visited_cursor: self.visited_cursor,
            timeline_views,
        };
        state.save(&self.layout.root)
    }

    /// Save an arbitrary OpenedDoc to disk. Used by the
    /// similar-paragraph mode toggle to flush the secondary doc
    /// (which lives in `self.secondary`, outside the normal
    /// save_current path). Mirrors save_current's body so the
    /// two stay in sync; refactoring both onto one impl is
    /// future work.
    pub(super) fn save_doc(
        &mut self,
        doc: &mut OpenedDoc,
    ) -> std::result::Result<(), String> {
        let abs = self.layout.root.join(&doc.rel_path);
        let body = doc.textarea.lines().join("\n");
        let prev_words =
            crate::progress::count_words(&doc.saved_lines.join("\n"));
        std::fs::write(&abs, body.as_bytes())
            .map_err(|e| format!("write {}: {e}", abs.display()))?;
        // Refresh the store so subsequent searches see the new
        // text. We deliberately skip the snapshot machinery —
        // secondary saves are routine + cheap; explicit snapshots
        // go through the F5 / Ctrl+B N flow on the primary doc.
        let mut node = self
            .hierarchy
            .get(doc.id)
            .cloned()
            .ok_or_else(|| format!("paragraph {} not in hierarchy", doc.id))?;
        self.store
            .update_paragraph_content(&mut node, body.as_bytes())
            .map_err(|e| format!("store update: {e}"))?;
        doc.dirty = false;
        doc.saved_lines = doc.textarea.lines().to_vec();
        // 1.2.7+ — restamp loaded_mtime so the external-
        // change watcher doesn't see our OWN save as a
        // "file changed under us" event.
        let abs = self.layout.root.join(&doc.rel_path);
        doc.loaded_mtime = std::fs::metadata(&abs)
            .and_then(|m| m.modified())
            .ok();
        let new_words = crate::progress::count_words(&body);
        let book_id = self.book_of_node(doc.id);
        crate::progress::record_save(doc.id, book_id, prev_words, new_words);
        self.refresh_progress_cache();
        Ok(())
    }

    /// Write the in-memory `chat_history` to disk. Empty history
    /// removes the file so a stale list doesn't haunt the next
    /// session.
    pub(super) fn save_chat_history_to_disk(&self) -> std::io::Result<()> {
        let path = self.chat_history_path();
        if self.chat_history.is_empty() {
            // Nothing to save — clean up any prior file so the next
            // entry doesn't restore a phantom.
            if path.exists() {
                std::fs::remove_file(&path)?;
            }
            return Ok(());
        }
        let json = serde_json::to_string_pretty(&self.chat_history)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(&path, json)
    }

    /// Load the on-disk chat history into `chat_history`. Returns
    /// the number of turns loaded (0 when the file is absent or
    /// empty). Parse / IO errors propagate so the caller can log.
    pub(super) fn load_chat_history_from_disk(&mut self) -> std::io::Result<usize> {
        let path = self.chat_history_path();
        if !path.exists() {
            return Ok(0);
        }
        let bytes = std::fs::read(&path)?;
        if bytes.is_empty() {
            return Ok(0);
        }
        let history: Vec<ChatTurn> = serde_json::from_slice(&bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let n = history.len();
        self.chat_history = history;
        Ok(n)
    }

    pub(super) fn load_file_into_editor(&mut self, path: &std::path::Path) {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.status = format!("read {}: {e}", path.display());
                return;
            }
        };
        let body = String::from_utf8_lossy(&bytes).into_owned();
        let Some(doc) = self.opened.as_mut() else {
            self.status =
                "no paragraph open — open one first, then F3 to replace its body".into();
            return;
        };
        let mut new_textarea = TextArea::new(body_to_lines(&body));
        new_textarea.set_cursor_line_style(Style::default().add_modifier(Modifier::REVERSED));
        new_textarea.set_line_number_style(Style::default().fg(Color::DarkGray));
        doc.textarea = new_textarea;
        doc.dirty = true;
        doc.scroll_row = 0;
        doc.scroll_col = 0;
        doc.last_activity = std::time::Instant::now();
        self.change_focus(Focus::Editor);
        self.status = format!("loaded `{}` — bold marks the change vs saved", path.display());
    }

    pub(super) fn load_paragraph(&mut self, node: &Node) -> Result<()> {
        if let Some(prev) = &self.opened {
            if prev.id == node.id {
                self.change_focus(Focus::Editor);
                return Ok(());
            }
            // Auto-save any pending edits in the previous paragraph before
            // swapping to the new one. If the save fails (disk error etc.),
            // keep the old doc open so the user can see and fix it.
            if prev.dirty {
                let _ = self.save_current();
                if self.opened.as_ref().is_some_and(|d| d.dirty) {
                    self.status = format!(
                        "couldn't autosave `{}` — opening blocked. Fix the error or Ctrl+S manually.",
                        self.opened.as_ref().map(|d| d.title.as_str()).unwrap_or("")
                    );
                    return Ok(());
                }
            }
            // Memorise the outgoing paragraph's cursor so re-opening it
            // (now or next session) lands back where the user left it.
            self.snapshot_open_paragraph_cursor();
        }

        let Some(rel) = node.file.as_ref() else {
            self.status = format!("paragraph `{}` has no file on disk", node.title);
            return Ok(());
        };
        let abs = self.layout.root.join(rel);
        // 1.2.12+ Phase D follow-up — fall back to bdslib
        // (`store.get_content`) when the on-disk file is
        // missing.  Some paragraphs live only in the
        // document store (prompts-editor TUI writes
        // bdslib without disk; some import flows do
        // similar).  We'd rather show bdslib content
        // than refuse to open the paragraph.
        let body = match std::fs::read_to_string(&abs) {
            Ok(b) => b,
            Err(disk_err) => {
                match self.store.get_content(node.id) {
                    Ok(Some(bytes)) => match String::from_utf8(bytes) {
                        Ok(s) => s,
                        Err(e) => {
                            self.status = format!(
                                "open `{}`: bdslib content isn't UTF-8: {e}",
                                node.title,
                            );
                            return Ok(());
                        }
                    },
                    Ok(None) => {
                        self.status = format!(
                            "read {}: {disk_err} · no bdslib record either",
                            abs.display(),
                        );
                        return Ok(());
                    }
                    Err(bdslib_err) => {
                        self.status = format!(
                            "read {}: {disk_err} · bdslib lookup failed: {bdslib_err}",
                            abs.display(),
                        );
                        return Ok(());
                    }
                }
            }
        };

        let lines = body_to_lines(&body);
        let saved_lines = lines.clone();
        let mut textarea = TextArea::new(lines);
        textarea.set_cursor_line_style(Style::default().add_modifier(Modifier::REVERSED));
        textarea.set_line_number_style(Style::default().fg(Color::DarkGray));

        let read_only = self.hierarchy.ancestors(node).iter().any(|a| {
            a.protected && a.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_HELP)
        });

        // Restore the saved cursor + scroll for this paragraph if we've
        // seen it before. Coords are clamped against the loaded buffer so a
        // shorter post-edit body can't crash the cursor.
        let saved_cursor = self.paragraph_cursors.get(&node.id).copied();
        let (init_row, init_col, init_scroll_row, init_scroll_col) = match saved_cursor {
            Some(pc) => {
                let max_row = textarea.lines().len().saturating_sub(1);
                let row = pc.cursor_row.min(max_row);
                let line_len = textarea
                    .lines()
                    .get(row)
                    .map_or(0, |s| s.chars().count());
                let col = pc.cursor_col.min(line_len);
                (row, col, pc.scroll_row.min(max_row), pc.scroll_col)
            }
            None => (0, 0, 0, 0),
        };
        if init_row > 0 || init_col > 0 {
            textarea.move_cursor(CursorMove::Jump(init_row as u16, init_col as u16));
        }

        // 1.2.14+ Phase C.1 — load the sidecar
        // comments file alongside the .typ.  Errors
        // are silently degraded to an empty file
        // (malformed JSON shouldn't block opening
        // the paragraph for editing); the next
        // status-bar refresh shows a hint via the
        // status field.
        let comments = match super::super::comments::load_from_sidecar(&abs) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(
                    "comments sidecar load failed for {}: {e}",
                    abs.display()
                );
                super::super::comments::CommentsFile::new()
            }
        };
        self.opened = Some(OpenedDoc {
            id: node.id,
            title: node.title.clone(),
            rel_path: rel.clone(),
            textarea,
            dirty: false,
            scroll_row: init_scroll_row,
            scroll_col: init_scroll_col,
            block_anchor: None,
            last_activity: std::time::Instant::now(),
            saved_lines,
            comments,
            loaded_mtime: std::fs::metadata(&abs)
                .and_then(|m| m.modified())
                .ok(),
            split: None,
            search: None,
            read_only,
            correction_baseline: None,
            // Script nodes default to the "bund" content_type even
            // if the persisted metadata is missing it — covers
            // scripts created before content_type stamping landed.
            content_type: node
                .content_type
                .clone()
                .or_else(|| match node.kind {
                    NodeKind::Script => Some("bund".to_string()),
                    _ => None,
                }),
            typst_diagnostics: Vec::new(),
            typst_diagnostics_checked_at: std::time::Instant::now(),
            typst_diag_last_fired: None,
            detected_language: None,
            detected_language_length: 0,
        });
        self.refresh_typst_diagnostics_for_opened();
        // 1.2.12+ — run whatlang on the freshly loaded body so
        // the AI resolver has a cached language when the user
        // fires their first chord.  No-op when the paragraph is
        // too short or `editor.prompt_language_mode` is
        // `book_defined` (the function itself short-circuits in
        // book mode to avoid unnecessary work).
        self.detect_paragraph_language();
        self.change_focus(Focus::Editor);
        self.status = format!("opened {}", abs.display());
        // 1.2.7+ — push to the visited-paragraph history,
        // unless this load_paragraph was triggered by a
        // back/forward navigation (in which case the cursor
        // already moved + the caller set skip_next_push).
        if std::mem::take(&mut self.visited_skip_next_push) {
            // back/forward — nothing to push
        } else {
            // New visit. Truncate any forward stack (browser
            // semantics) and push.
            let cur = self.visited_cursor;
            let already_current = self
                .visited_history
                .get(cur)
                .copied()
                == Some(node.id);
            if !already_current {
                self.visited_history.truncate(cur + 1);
                self.visited_history.push(node.id);
                self.visited_cursor = self.visited_history.len() - 1;
            }
        }
        Ok(())
    }

    pub(super) fn save_current(&mut self) -> Result<()> {
        let Some(doc) = self.opened.as_mut() else {
            return Ok(());
        };
        if doc.read_only {
            // Quietly clear the dirty bit so the autosave loop doesn't keep
            // retrying. Nothing got mutated anyway — the editor's key handler
            // blocks every write — but a stray block_anchor or focus blur
            // could still flip dirty=true on a corner case.
            doc.dirty = false;
            self.status = "Help is read-only — nothing to save".into();
            return Ok(());
        }
        let body = doc.textarea.lines().join("\n");
        // Capture pre-save word count before we overwrite anything
        // — used by the progress event log to compute word_delta.
        let prev_words = crate::progress::count_words(&doc.saved_lines.join("\n"));
        let abs = self.layout.root.join(&doc.rel_path);

        // Filesystem write first; if that fails, abort before touching the store.
        if let Err(e) = std::fs::write(&abs, body.as_bytes()) {
            self.status = format!("write {}: {e}", abs.display());
            return Ok(());
        }

        let id = doc.id;
        let Some(mut node) = self.hierarchy.get(id).cloned() else {
            self.status = format!("node {id} missing from hierarchy — try reopening the TUI");
            return Ok(());
        };

        // If this paragraph still has the placeholder title, derive a real one
        // from the body's first sentence and stamp it onto the node — that
        // becomes the displayed name in the tree pane.
        //
        // 1.2.4+: when we auto-derive a title here, route through
        // `rename_node` so the on-disk filename + slug track the
        // new title. The body has already been written to the
        // OLD path above; `rename_node` will `fs::rename` it to
        // the new path, so the bytes follow the new name.
        let title_was_placeholder = node.title == PARAGRAPH_PLACEHOLDER_TITLE;
        if title_was_placeholder {
            if let Some(derived) = extract_first_sentence(&body) {
                if let Err(e) =
                    self.store.rename_node(&self.hierarchy, node.id, &derived)
                {
                    tracing::warn!(
                        target: "inkhaven::save",
                        "auto-rename to first sentence failed: {e:#}",
                    );
                } else {
                    // Reload so the local `node` + the open doc
                    // reflect the new slug + file path. If the
                    // hierarchy reload itself fails, leave the
                    // existing one in place — the rename is
                    // already on disk, just no in-memory refresh.
                    if let Ok(h) =
                        crate::store::hierarchy::Hierarchy::load(&self.store)
                    {
                        self.hierarchy = h;
                    }
                    if let Some(refreshed) = self.hierarchy.get(node.id).cloned() {
                        // Sync `doc` (the outstanding &mut borrow
                        // taken at the top of save_current_inner)
                        // so its title + rel_path match the new
                        // on-disk layout.
                        doc.title = refreshed.title.clone();
                        if let Some(rel) = refreshed.file.as_ref() {
                            doc.rel_path = rel.clone();
                        }
                        node = refreshed;
                    }
                }
            }
        }

        if let Err(e) = self
            .store
            .update_paragraph_content(&mut node, body.as_bytes())
        {
            self.status = format!("store update failed: {e}");
            return Ok(());
        }
        if let Err(e) = self.store.sync() {
            self.status = format!("store sync failed: {e}");
            return Ok(());
        }

        doc.dirty = false;
        // Refresh the saved-lines snapshot so the bold-new-additions overlay
        // resets, and stamp last_activity to "now" so idle autosave restarts.
        // Save is the explicit "I've reviewed and accepted the
        // corrections" signal — drop the highlight + resume normal
        // autosave cadence.
        doc.saved_lines = doc.textarea.lines().to_vec();
        doc.correction_baseline = None;
        doc.last_activity = std::time::Instant::now();
        // Restamp loaded_mtime so `tick_external_change_check` on the
        // next loop iteration doesn't mistake our OWN save for an
        // external file edit and trigger a clean-reload (which would
        // recreate the textarea and snap the cursor to (0,0)). The
        // sibling save_doc path stamps the same way; this was missing
        // from save_current and surfaced as a cursor-jump-on-Ctrl+S
        // regression.
        let abs_for_mtime = self.layout.root.join(&doc.rel_path);
        doc.loaded_mtime = std::fs::metadata(&abs_for_mtime)
            .and_then(|m| m.modified())
            .ok();
        let words = node.word_count;
        if title_was_placeholder && node.title != PARAGRAPH_PLACEHOLDER_TITLE {
            self.status = format!(
                "saved {} ({} words) · named `{}` from first sentence",
                abs.display(),
                words,
                node.title
            );
        } else {
            self.status = format!("saved {} ({} words, re-embedded)", abs.display(), words);
        }
        // Progress event log. The book this paragraph belongs
        // to feeds per-book aggregates; project-wide events drop
        // book_id = None for the same record.
        let new_words = crate::progress::count_words(&body);
        let book_id = self.book_of_node(node.id);
        crate::progress::record_save(node.id, book_id, prev_words, new_words);
        // Auto-promote on goal-hit. Idempotent per
        // (paragraph, status) — see Goals.auto_promote_on_target
        // semantics.
        self.maybe_auto_promote_on_target(node.id, new_words);
        self.reload_hierarchy();
        self.refresh_progress_cache();
        // 1.2.5+: refresh typst-syntax diagnostics on save. Pulls
        // the most-recently-saved body straight from the editor's
        // mutable doc so the next render reflects errors the user
        // just introduced (or fixed).
        self.refresh_typst_diagnostics_for_opened();
        // 1.2.12+ Phase D — re-detect paragraph language if the
        // body length has drifted enough from the last detection
        // to plausibly change the dominant-language signal.
        // No-op when the effective mode is `book_defined` (the
        // function itself short-circuits there).
        self.maybe_redetect_paragraph_language();
        Ok(())
    }

    /// F8 (1.2.6+) — open the typst-diagnostics list modal.
    /// Refreshes the diagnostic cache up-front so the modal
    /// reflects the live buffer, not the last save.
    /// 1.2.6+ — open the paragraph with `id` in the editor;
    /// also moves the tree cursor onto it so the visible
    /// state is consistent with the action that triggered.
    pub(super) fn open_paragraph_by_uuid(&mut self, id: Uuid) -> std::result::Result<(), String> {
        let node = self
            .hierarchy
            .get(id)
            .cloned()
            .ok_or_else(|| format!("node {id} missing from hierarchy"))?;
        if node.kind != NodeKind::Paragraph {
            return Err(format!("{} is not a Paragraph", node.title));
        }
        if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == id) {
            self.tree_cursor = i;
        }
        self.load_paragraph(&node).map_err(|e| e.to_string())?;
        self.change_focus(Focus::Editor);
        // 1.2.6+: surface a clear next-step hint when the user
        // opens an orphan event paragraph — otherwise the only
        // visible signal that the event needs a target is the
        // `[ORPHAN]` tag in the timeline view, with no nudge
        // about which chord assigns one.
        if node.event.is_some()
            && node.tags.iter().any(|t| t.eq_ignore_ascii_case("orphan"))
        {
            self.status =
                "orphan event — Ctrl+V A to link a manuscript paragraph (target). Saving the link drops [ORPHAN].".into();
        }
        Ok(())
    }

    /// 1.2.7+ — return the current editor selection as a
    /// `String`, or `None` when no selection is active.
    /// Preserves the textarea's yank buffer (we briefly
    /// hijack it to read the selection then restore).
    pub(super) fn editor_selection_text(&mut self) -> Option<String> {
        let doc = self.opened.as_mut()?;
        if doc.textarea.selection_range().is_none() {
            return None;
        }
        let saved = doc.textarea.yank_text();
        doc.textarea.copy();
        let text = doc.textarea.yank_text();
        doc.textarea.set_yank_text(saved);
        if text.trim().is_empty() {
            None
        } else {
            Some(text)
        }
    }

    /// Re-read the hierarchy from bdslib and rebuild the flattened tree-row
    /// list, preserving the cursor on the same UUID if it still exists.
    pub(super) fn reload_hierarchy(&mut self) {
        let prev_id = self.rows.get(self.tree_cursor).map(|(id, _)| *id);
        match Hierarchy::load(&self.store) {
            Ok(h) => {
                self.hierarchy = h;
                // Prune collapsed-state for nodes that no longer exist.
                self.collapsed_nodes
                    .retain(|id| self.hierarchy.get(*id).is_some());
                self.rows = self
                    .hierarchy
                    .flatten_with_collapsed(&self.collapsed_nodes)
                    .into_iter()
                    .map(|(n, d)| (n.id, d))
                    .collect();
                if let Some(id) = prev_id {
                    if let Some(i) = self.rows.iter().position(|(rid, _)| *rid == id) {
                        self.tree_cursor = i;
                    }
                }
                if !self.rows.is_empty() {
                    self.tree_cursor = self.tree_cursor.min(self.rows.len() - 1);
                }
                // Lexicon depends on Places/Characters paragraph titles, so
                // any hierarchy change is potentially a lexicon change. The
                // recompute is cheap at literary scale (a few hundred names
                // stemmed once per language).
                let (lex, lang_index) =
                    build_lexicon(&self.hierarchy, &self.cfg, &self.store);
                self.lexicon = lex;
                self.language_entries = lang_index;
            }
            Err(e) => {
                self.status = format!("hierarchy reload failed: {e}");
            }
        }
    }

    /// Materialise the picked paragraph as a `secondary` OpenedDoc
    /// rendered in the right pane (replacing AI while in similar
    /// mode). Mirrors `load_paragraph`'s body construction; cursor
    /// memory is honoured so re-opening lands where the user left
    /// it (consistent with primary-pane behaviour).
    pub(super) fn load_secondary_paragraph(
        &mut self,
        id: Uuid,
    ) -> std::result::Result<(), String> {
        let node = self
            .hierarchy
            .get(id)
            .cloned()
            .ok_or_else(|| format!("paragraph {id} not in hierarchy"))?;
        if node.kind != NodeKind::Paragraph {
            return Err(format!("`{}` is not a paragraph", node.title));
        }
        let rel = node
            .file
            .as_ref()
            .ok_or_else(|| format!("paragraph `{}` has no file on disk", node.title))?;
        let abs = self.layout.root.join(rel);
        // 1.2.12+ Phase D follow-up — fall back to bdslib
        // (`store.get_content`) when the on-disk file is
        // missing.  Some paragraphs live only in the
        // document store: the prompts-editor TUI writes
        // to bdslib without touching disk, and the
        // `inkhaven import-*` flows can do the same.  The
        // primary `load_paragraph` errors on missing
        // files; for the secondary slot we'd rather show
        // bdslib content than block the pin entirely.
        // Same fallback the concordance system-book
        // filter validated.
        let body = match std::fs::read_to_string(&abs) {
            Ok(b) => b,
            Err(_) => {
                let bytes = self
                    .store
                    .get_content(node.id)
                    .map_err(|e| {
                        format!(
                            "read {}: missing on disk, and bdslib lookup failed: {e}",
                            abs.display(),
                        )
                    })?;
                let bytes = bytes.ok_or_else(|| {
                    format!(
                        "read {}: missing on disk, no bdslib record either",
                        abs.display(),
                    )
                })?;
                String::from_utf8(bytes).map_err(|e| {
                    format!(
                        "bdslib content for `{}` is not valid UTF-8: {e}",
                        node.title,
                    )
                })?
            }
        };
        let lines = body_to_lines(&body);
        let saved_lines = lines.clone();
        let mut textarea = TextArea::new(lines);
        textarea.set_cursor_line_style(Style::default().add_modifier(Modifier::REVERSED));
        textarea.set_line_number_style(Style::default().fg(Color::DarkGray));
        let read_only = self.hierarchy.ancestors(&node).iter().any(|a| {
            a.protected && a.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_HELP)
        });
        let saved_cursor = self.paragraph_cursors.get(&node.id).copied();
        let (init_row, init_col, init_scroll_row, init_scroll_col) = match saved_cursor {
            Some(pc) => {
                let max_row = textarea.lines().len().saturating_sub(1);
                let row = pc.cursor_row.min(max_row);
                let line_len = textarea
                    .lines()
                    .get(row)
                    .map_or(0, |s| s.chars().count());
                let col = pc.cursor_col.min(line_len);
                (row, col, pc.scroll_row.min(max_row), pc.scroll_col)
            }
            None => (0, 0, 0, 0),
        };
        if init_row > 0 || init_col > 0 {
            textarea.move_cursor(CursorMove::Jump(init_row as u16, init_col as u16));
        }
        // 1.2.14+ Phase C.1 — secondary slot also
        // loads its own comments so the split-view
        // overlay paints comments on both panes.
        let comments = super::super::comments::load_from_sidecar(&abs)
            .unwrap_or_else(|_| super::super::comments::CommentsFile::new());
        self.secondary = Some(OpenedDoc {
            id: node.id,
            title: node.title.clone(),
            rel_path: rel.clone(),
            textarea,
            dirty: false,
            scroll_row: init_scroll_row,
            scroll_col: init_scroll_col,
            block_anchor: None,
            last_activity: std::time::Instant::now(),
            saved_lines,
            comments,
            loaded_mtime: std::fs::metadata(&abs)
                .and_then(|m| m.modified())
                .ok(),
            split: None,
            search: None,
            read_only,
            correction_baseline: None,
            content_type: node.content_type.clone(),
            typst_diagnostics: Vec::new(),
            typst_diagnostics_checked_at: std::time::Instant::now(),
            typst_diag_last_fired: None,
            // 1.2.12+ — secondary docs (split-view, similar-mode)
            // don't drive the AI resolver; we never call
            // `active_prompt_language` against them.  Leave the
            // detection cache empty.
            detected_language: None,
            detected_language_length: 0,
        });
        self.secondary_focused = false;
        self.status = format!(
            "similar: `{}` opened side-by-side (Tab swaps focus · Ctrl+V S exits)",
            node.title
        );
        Ok(())
    }

    /// Open the per-paragraph goal-setting modal. Pre-fills the
    /// input box with the current `target_words` (if any) so
    /// editing a goal is one keystroke; empty / `0` on Enter
    /// clears the goal.
    pub(super) fn open_paragraph_target_modal(&mut self) {
        // 1.2.4+: when the tree has multi-select active, the
        // modal opens for ALL marked paragraphs and the commit
        // applies the same target to each. Prefill is empty
        // (no single "current" to display across a set).
        if !self.tree_marked.is_empty() {
            self.modal = Modal::ParagraphTarget {
                input: TextInput::new(),
            };
            self.status = format!(
                "paragraph target × {}: type a number, Enter sets all, Esc cancels",
                self.tree_marked.len()
            );
            return;
        }
        let Some(doc) = self.opened.as_ref() else {
            self.status = "view T: no paragraph open".into();
            return;
        };
        let current = self
            .hierarchy
            .get(doc.id)
            .and_then(|n| n.target_words)
            .filter(|n| *n > 0);
        let mut input = TextInput::new();
        if let Some(n) = current {
            for c in n.to_string().chars() {
                input.insert_char(c);
            }
        }
        self.modal = Modal::ParagraphTarget { input };
        self.status =
            "paragraph target: type a number, Enter to set, empty/0 to clear, Esc to cancel"
                .into();
    }

    /// Commit `raw` as the open paragraph's `target_words`. Empty
    /// or `"0"` clears the goal (sets to None). Non-numeric input
    /// surfaces an error and leaves the existing value untouched.
    ///
    /// 1.2.4+: when `tree_marked` is non-empty, the same target
    /// is applied to every marked paragraph instead of the open
    /// one.
    pub(super) fn commit_paragraph_target(&mut self, raw: &str) {
        let new_target: Option<i32> = if raw.is_empty() || raw == "0" {
            None
        } else {
            match raw.parse::<i32>() {
                Ok(n) if n > 0 => Some(n),
                Ok(_) => {
                    self.status = "view T: target must be > 0".into();
                    return;
                }
                Err(_) => {
                    self.status = format!("view T: `{raw}` is not a number");
                    return;
                }
            }
        };
        // Multi-select path: apply the same target to every
        // marked paragraph.
        if !self.tree_marked.is_empty() {
            let ids: Vec<Uuid> = self.tree_marked.iter().copied().collect();
            let mut ok = 0usize;
            let mut fail = 0usize;
            for id in &ids {
                if self.set_paragraph_target_now(*id, new_target).is_ok() {
                    ok += 1;
                } else {
                    fail += 1;
                }
            }
            self.status = match new_target {
                Some(n) => format!(
                    "target {n} set on {ok} paragraph(s){}",
                    if fail > 0 { format!(" · {fail} failed") } else { String::new() }
                ),
                None => format!(
                    "target cleared on {ok} paragraph(s){}",
                    if fail > 0 { format!(" · {fail} failed") } else { String::new() }
                ),
            };
            return;
        }
        let Some(doc) = self.opened.as_ref() else {
            self.status = "view T: paragraph closed during input".into();
            return;
        };
        let id = doc.id;
        match self.set_paragraph_target_now(id, new_target) {
            Ok(()) => {
                self.status = match new_target {
                    Some(n) => format!("paragraph target: {} words", n),
                    None => "paragraph target: cleared".into(),
                };
            }
            Err(e) => self.status = format!("view T: {e}"),
        }
    }

    /// 1.2.8+ — Ctrl+V h. Scan the open paragraph's buffer
    /// for "hidden" characters (tabs, trailing whitespace
    /// lines, CRs) and stamp a status-bar summary. Useful
    /// for spotting import noise from Scrivener / web paste
    /// before it lands in the final manuscript. No buffer
    /// rewrite — visual editor overlay scheduled for 1.2.9.
    pub(super) fn report_hidden_chars(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "hidden chars: no paragraph open".into();
            return;
        };
        let lines = doc.textarea.lines();
        let mut tab_count = 0usize;
        let mut trailing_ws_lines = 0usize;
        let mut cr_count = 0usize;
        for line in lines {
            tab_count += line.chars().filter(|c| *c == '\t').count();
            cr_count += line.chars().filter(|c| *c == '\r').count();
            // Trailing whitespace = ends with space or tab, and is
            // not just an entirely-blank line (those are usually
            // intentional paragraph breaks in typst).
            let trimmed = line.trim_end_matches(|c: char| c == ' ' || c == '\t');
            if !line.is_empty() && trimmed.len() < line.len() && !trimmed.is_empty() {
                trailing_ws_lines += 1;
            }
        }
        if tab_count == 0 && trailing_ws_lines == 0 && cr_count == 0 {
            self.status =
                "hidden chars: clean — no tabs, trailing whitespace, or CRs".into();
        } else {
            self.status = format!(
                "hidden chars: {tab_count} tab(s), {trailing_ws_lines} line(s) with trailing whitespace, {cr_count} CR(s)",
            );
        }
    }

    /// 1.2.8+ — Ctrl+V Shift+S. Print the hierarchy path
    /// from project root to the cursor on the status bar
    /// ("Book ▸ Chapter ▸ Subchapter ▸ Paragraph").  Pane-
    /// aware: in tree pane walks from `rows[tree_cursor]`;
    /// in editor pane walks from `opened.id`; falls through
    /// to the tree row when no doc is open.
    pub(super) fn show_cursor_breadcrumb(&mut self) {
        let cursor_id = match self.focus {
            Focus::Editor => self.opened.as_ref().map(|d| d.id),
            _ => self.rows.get(self.tree_cursor).map(|(id, _)| *id),
        };
        let Some(id) = cursor_id else {
            self.status = "breadcrumb: nothing under cursor".into();
            return;
        };
        let Some(node) = self.hierarchy.get(id) else {
            self.status = "breadcrumb: cursor row missing from hierarchy".into();
            return;
        };
        let mut chain: Vec<&str> = self
            .hierarchy
            .ancestors(node)
            .into_iter()
            .map(|n| n.title.as_str())
            .collect();
        chain.push(node.title.as_str());
        self.status = chain.join(" ▸ ");
    }

    /// Editor pane block builder. Takes a pre-built styled `Line` for
    /// the title so the renderer can mix theme colours into the header
    /// (used for the `L… C…` cursor read-out chip).
    pub(super) fn editor_block_line<'a>(&self, title: Line<'a>) -> Block<'a> {
        let border_color = if self.focus == Focus::Editor {
            let dirty = self.opened.as_ref().is_some_and(|d| d.dirty);
            let ro = self.opened.as_ref().is_some_and(|d| d.read_only);
            if ro {
                self.theme.border_readonly
            } else if dirty {
                self.theme.border_dirty
            } else {
                self.theme.border_saved
            }
        } else {
            self.theme.border_unfocused
        };
        Block::default()
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
            )
    }

    /// 1.2.13+ Phase B.2 — `[word · POS · translation]`
    /// chip text for the editor footer when the cursor
    /// lands on a Language lexicon hit.  Returns `None`
    /// when:
    ///   * no paragraph is open;
    ///   * the cursor isn't on a lexicon hit;
    ///   * the hit isn't Language-category;
    ///   * the matched form isn't in the entry index
    ///     (pre-Phase-B body with no HJSON block — the
    ///     overlay still lights up but there's no parsed
    ///     data to chip with).
    pub(super) fn language_hit_chip(&self) -> Option<String> {
        if self.language_entries.is_empty() {
            return None;
        }
        let doc = self.opened.as_ref()?;
        let (row, col) = doc.textarea.cursor();
        let lines = doc.textarea.lines();
        let line = lines.get(row)?;
        let hits = self.lexicon.row_hits(line);
        let hit = hits.iter().find(|h| {
            // Lexicon hits are in character (not byte)
            // coordinates; the cursor's `col` from
            // tui-textarea is also character-based.
            col >= h.col_start && col < h.col_end
        })?;
        if hit.category != super::super::lexicon::LexCategory::Language {
            return None;
        }
        let matched: String = line
            .chars()
            .skip(hit.col_start)
            .take(hit.col_end - hit.col_start)
            .collect();
        let entry = self.language_entries.lookup(&matched)?;
        let lemma = entry.word.trim();
        let pos = entry.pos.trim();
        let translation = entry.translation.trim();
        // Chip is best-effort — fields the author hasn't
        // filled in get omitted rather than rendering
        // empty dots that look like a parse error.
        let mut parts: Vec<&str> = Vec::new();
        if !lemma.is_empty() {
            parts.push(lemma);
        }
        if !pos.is_empty() {
            parts.push(pos);
        }
        if !translation.is_empty() {
            parts.push(translation);
        }
        if parts.is_empty() {
            return None;
        }
        Some(format!("[{}]", parts.join(" · ")))
    }

    /// Compute the editor-pane goal footer text from the open
    /// doc + its node metadata. Returns `(breadcrumb, words,
    /// target)` when a goal is set, otherwise `None`. The
    /// breadcrumb is the human-readable title chain
    /// ("My book › Chapter one › The morning") rather than the
    /// slug path — slugs are stale after a rename until we
    /// re-derive them, and users think in titles anyway.
    pub(super) fn editor_goal_footer_text(&self) -> Option<(String, i64, i64)> {
        let doc = self.opened.as_ref()?;
        let node = self.hierarchy.get(doc.id)?;
        let target = node.target_words.filter(|n| *n > 0)? as i64;
        // Count live in-memory text via the same algorithm the
        // save path uses so the footer matches what the save
        // event will record.
        let body = doc.textarea.lines().join("\n");
        let words = crate::progress::count_words(&body);
        let breadcrumb = self.title_breadcrumb(node.id);
        Some((breadcrumb, words, target))
    }

}
