//! Snapshot picker (F6), snapshot annotation (F5), snapshot
//! diff (V from picker), and split-snapshot accept/restore —
//! every App method that mutates the snapshot store or its
//! picker modal. Extracted from `tui::app` in the 1.2.7
//! refactor, Phase 3 batch 4.

use ratatui::style::{Color, Modifier, Style};
use tui_textarea::TextArea;
use uuid::Uuid;

use super::super::diff_utils::compute_line_diff;
use super::super::focus::Focus;
use super::super::input::TextInput;
use super::super::modal::Modal;
use super::super::session::ParagraphCursor;
use super::super::text_utils::body_to_lines;

impl super::App {

    /// Copy the currently-open paragraph's cursor + scroll into the
    /// in-memory `paragraph_cursors` map. Called on focus loss, on
    /// paragraph switch, and right before `save_session` writes to disk.
    /// No-op when no paragraph is open.
    pub(super) fn snapshot_open_paragraph_cursor(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            return;
        };
        let (row, col) = doc.textarea.cursor();
        self.paragraph_cursors.insert(
            doc.id,
            ParagraphCursor {
                cursor_row: row,
                cursor_col: col,
                scroll_row: doc.scroll_row,
                scroll_col: doc.scroll_col,
            },
        );
    }

    /// Replace the live buffer with the split snapshot and exit split mode.
    /// Used to "roll back" to the captured version after experimenting.
    pub(super) fn accept_split_snapshot(&mut self) {
        let Some(doc) = self.opened.as_mut() else {
            return;
        };
        let Some(split) = doc.split.take() else {
            self.status = "split is not open".into();
            return;
        };
        let lines = if split.snapshot_lines.is_empty() {
            vec![String::new()]
        } else {
            split.snapshot_lines
        };
        let mut new_ta = TextArea::new(lines);
        new_ta.set_cursor_line_style(Style::default().add_modifier(Modifier::REVERSED));
        new_ta.set_line_number_style(Style::default().fg(Color::DarkGray));
        doc.textarea = new_ta;
        doc.dirty = true;
        doc.scroll_row = 0;
        doc.scroll_col = 0;
        doc.last_activity = std::time::Instant::now();
        self.status =
            "split snapshot accepted — buffer replaced; Ctrl+S to commit · bold shows the diff"
                .into();
    }

    pub(super) fn create_snapshot_of_current(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "no paragraph open".into();
            return;
        };
        let body = doc.textarea.lines().join("\n").into_bytes();
        let id = doc.id;
        let Some(node) = self.hierarchy.get(id).cloned() else {
            self.status = "node missing from hierarchy".into();
            return;
        };
        // 1.2.7+ — dedupe. If the latest existing snapshot for
        // this paragraph has identical content, skip the
        // annotation prompt entirely and stamp a "no changes"
        // status line. Stops F5 mashing from littering history
        // with duplicates AND keeps the annotation prompt
        // honest (no point annotating a no-op).
        if let Ok(snaps) = self.store.list_snapshots(id) {
            if let Some(latest) = snaps.first() {
                if let Ok(Some(prev)) =
                    self.store.snapshot_content(latest.id)
                {
                    if prev == body {
                        self.status = format!(
                            "snapshot: `{}` unchanged since the last snapshot — no new snapshot taken",
                            node.title
                        );
                        return;
                    }
                }
            }
        }
        // 1.2.6+ — pop an annotation prompt so the user can
        // jot a one-line note ("first complete draft", "before
        // the lighthouse rewrite"). Enter on empty input still
        // commits — keeps the F5 → Enter flow as fast as the
        // old one-keystroke path. Esc cancels.
        self.modal = Modal::SnapshotAnnotation {
            input: TextInput::new(),
            parent_id: id,
            parent_title: node.title,
            body,
        };
        self.status =
            "snapshot annotation: type a note (or Enter for no note) · Esc cancels".into();
    }

    /// Commit step for `Modal::SnapshotAnnotation` — invoked by
    /// the modal's Enter handler. Calls
    /// `Store::create_snapshot_annotated`, stamps the result on
    /// the status bar, and closes the modal.
    pub(super) fn commit_snapshot_annotation(
        &mut self,
        parent_id: Uuid,
        parent_title: &str,
        body: &[u8],
        annotation: &str,
    ) {
        let Some(node) = self.hierarchy.get(parent_id).cloned() else {
            self.status = "snapshot: paragraph vanished".into();
            return;
        };
        match self
            .store
            .create_snapshot_annotated(&node, body, annotation)
        {
            Ok(snap_id) => {
                let n_snaps = self
                    .store
                    .list_snapshots(parent_id)
                    .map(|v| v.len())
                    .unwrap_or(0);
                let note = if annotation.trim().is_empty() {
                    String::new()
                } else {
                    format!(" · `{annotation}`")
                };
                self.status = format!(
                    "snapshot {} of `{parent_title}` created ({} total){note} — F6 to view",
                    snap_id.simple(),
                    n_snaps,
                );
            }
            Err(e) => {
                self.status = format!("snapshot failed: {e}");
            }
        }
    }

    /// 1.2.11+ — snapshot the currently open
    /// paragraph with `annotation`, picking up the
    /// live editor body (not the on-disk file).
    /// Used by flows that need to label the
    /// pre-mutation state before they apply an
    /// AI rewrite — most notably the Ctrl+B Shift+M
    /// sentence-rhythm rewrite, which snapshots
    /// the unrewritten body annotated `Sentence
    /// rhythm rewrite` immediately before the
    /// rewrite lands.  No-op when no paragraph is
    /// open; failures land on the status bar.
    pub(super) fn snapshot_open_paragraph_with_annotation(
        &mut self,
        annotation: &str,
    ) {
        let Some(doc) = self.opened.as_ref() else {
            self.status =
                "snapshot: no paragraph open".into();
            return;
        };
        let parent_id = doc.id;
        let parent_title = doc.title.clone();
        let body = doc.textarea.lines().join("\n").into_bytes();
        self.commit_snapshot_annotation(
            parent_id,
            &parent_title,
            &body,
            annotation,
        );
    }

    pub(super) fn open_snapshot_picker(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status = "no paragraph open".into();
            return;
        };
        let id = doc.id;
        let title = doc.title.clone();
        match self.store.list_snapshots(id) {
            Ok(snapshots) => {
                if snapshots.is_empty() {
                    self.status =
                        format!("no snapshots yet for `{title}` — press F5 to create one");
                    return;
                }
                // 1.2.8+ — reset the annotation filter; previous
                // session's filter shouldn't haunt a fresh picker.
                self.snapshot_filter.clear();
                self.snapshot_filter_focused = false;
                self.modal = Modal::SnapshotPicker {
                    paragraph_id: id,
                    paragraph_title: title,
                    snapshots,
                    cursor: 0,
                };
            }
            Err(e) => {
                self.status = format!("snapshot list failed: {e}");
            }
        }
    }

    /// 1.2.8+ — return indices into `snaps` that match the
    /// current annotation filter (case-insensitive substring
    /// against annotation text). Empty filter returns every
    /// index in order. The picker uses these via
    /// `visible[cursor]` to look up the absolute snapshot the
    /// cursor refers to.
    pub(super) fn visible_snapshot_indices(
        &self,
        snaps: &[crate::store::Snapshot],
    ) -> Vec<usize> {
        if self.snapshot_filter.is_empty() {
            return (0..snaps.len()).collect();
        }
        let needle = self.snapshot_filter.to_lowercase();
        snaps
            .iter()
            .enumerate()
            .filter(|(_, s)| s.annotation.to_lowercase().contains(&needle))
            .map(|(i, _)| i)
            .collect()
    }

    pub(super) fn commit_snapshot_load(&mut self) {
        // 1.2.8+ — when the annotation filter is non-empty,
        // `cursor` indexes the FILTERED visible list, not the
        // absolute snapshots Vec.  Translate before reading.
        let (snap_id, when) = match &self.modal {
            Modal::SnapshotPicker {
                snapshots, cursor, ..
            } => {
                let visible = self.visible_snapshot_indices(snapshots);
                let Some(abs_idx) = visible.get(*cursor) else {
                    self.modal = Modal::None;
                    return;
                };
                let Some(snap) = snapshots.get(*abs_idx) else {
                    self.modal = Modal::None;
                    return;
                };
                (snap.id, snap.created_at)
            }
            _ => return,
        };
        let content = match self.store.snapshot_content(snap_id) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => {
                self.status = "snapshot has no body".into();
                self.modal = Modal::None;
                return;
            }
            Err(e) => {
                self.status = format!("snapshot load failed: {e}");
                self.modal = Modal::None;
                return;
            }
        };

        // Safety net (1.2.4+): before we replace the editor buffer,
        // snapshot whatever is currently in it. Without this, hitting
        // Enter on an old snapshot would silently discard any
        // unsaved typing — "oops! a day of work gone". Now the
        // recoverable history grows by one row instead.
        //
        // The pre-restore snapshot fires `hook.on_snapshot` like any
        // other snapshot. If snapshot creation itself fails, we
        // abort the load: the whole point is data safety, so doing
        // the replace without the safety net would defeat the
        // change. The user can fix the underlying error (disk full,
        // store offline) and retry.
        let pre_restore_id = if let Some(doc) = self.opened.as_ref() {
            let body_now = doc.textarea.lines().join("\n");
            let node = self.hierarchy.get(doc.id).cloned();
            match node {
                Some(n) => match self.store.create_snapshot(&n, body_now.as_bytes()) {
                    Ok(id) => Some(id),
                    Err(e) => {
                        self.status = format!(
                            "snapshot load aborted: safety snapshot failed ({e}) — retry once the store is healthy"
                        );
                        self.modal = Modal::None;
                        return;
                    }
                },
                None => None,
            }
        } else {
            None
        };

        let body = String::from_utf8_lossy(&content).into_owned();
        let Some(doc) = self.opened.as_mut() else {
            self.modal = Modal::None;
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
        // saved_lines stays at the previously-saved on-disk version, so the
        // snapshot text shows as "added" (bold) until the user accepts it
        // by hitting Ctrl+S.
        self.modal = Modal::None;
        self.change_focus(Focus::Editor);
        let safety_msg = match pre_restore_id {
            Some(id) => format!(" · safety snapshot {} created", id.simple()),
            None => String::new(),
        };
        self.status = format!(
            "loaded snapshot from {} — bold marks the change vs saved{}",
            when.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S %z"),
            safety_msg,
        );
    }

    /// Open the snapshot-diff modal against the cursor's snapshot.
    /// Stashes the current `SnapshotPicker` modal inside the new
    /// variant so `Esc` returns to the picker rather than closing
    /// both layers.
    pub(super) fn open_snapshot_diff(&mut self) {
        let (snap_id, when, paragraph_title) = match &self.modal {
            Modal::SnapshotPicker {
                snapshots,
                cursor,
                paragraph_title,
                ..
            } => {
                // 1.2.8+ — translate visible cursor → absolute index.
                let visible = self.visible_snapshot_indices(snapshots);
                let Some(abs_idx) = visible.get(*cursor) else {
                    return;
                };
                let Some(snap) = snapshots.get(*abs_idx) else {
                    return;
                };
                (snap.id, snap.created_at, paragraph_title.clone())
            }
            _ => return,
        };
        let snapshot_bytes = match self.store.snapshot_content(snap_id) {
            Ok(Some(b)) => b,
            Ok(None) => {
                self.status = "snapshot has no body".into();
                return;
            }
            Err(e) => {
                self.status = format!("snapshot load failed: {e}");
                return;
            }
        };
        let snapshot_text = String::from_utf8_lossy(&snapshot_bytes).into_owned();
        let current_text = self
            .opened
            .as_ref()
            .map(|d| d.textarea.lines().join("\n"))
            .unwrap_or_default();
        let rows = compute_line_diff(&snapshot_text, &current_text);
        let when_str = when
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M:%S %z")
            .to_string();
        let return_to = Box::new(std::mem::replace(&mut self.modal, Modal::None));
        self.modal = Modal::SnapshotDiff {
            paragraph_title,
            when: when_str,
            rows,
            scroll: 0,
            return_to,
        };
        self.status = "diff: snapshot ← left · current → right · ↑↓ scroll · Esc back".into();
    }

    pub(super) fn delete_current_snapshot(&mut self) {
        // 1.2.8+ — translate visible cursor → absolute index
        // via the annotation-filter helper, same as the load
        // path.
        let (snap_id, when, paragraph_id, paragraph_title) = match &self.modal {
            Modal::SnapshotPicker {
                snapshots,
                cursor,
                paragraph_id,
                paragraph_title,
            } => {
                let visible = self.visible_snapshot_indices(snapshots);
                let Some(abs_idx) = visible.get(*cursor) else {
                    return;
                };
                let Some(snap) = snapshots.get(*abs_idx).cloned() else {
                    return;
                };
                (snap.id, snap.created_at, *paragraph_id, paragraph_title.clone())
            }
            _ => return,
        };

        if let Err(e) = self.store.delete_snapshot(snap_id) {
            self.status = format!("delete snapshot failed: {e}");
            return;
        }

        let when_local = when
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M:%S %z");

        match self.store.list_snapshots(paragraph_id) {
            Ok(snapshots) => {
                if snapshots.is_empty() {
                    self.modal = Modal::None;
                    self.status = format!(
                        "deleted snapshot {when_local} — no snapshots left for `{paragraph_title}`"
                    );
                } else {
                    // Keep the cursor on the same row index, clamped
                    // to the new (shorter) list — feels like "the row
                    // below the deleted one slid up".
                    let new_cursor = match &self.modal {
                        Modal::SnapshotPicker { cursor, .. } => {
                            (*cursor).min(snapshots.len() - 1)
                        }
                        _ => 0,
                    };
                    self.modal = Modal::SnapshotPicker {
                        paragraph_id,
                        paragraph_title,
                        snapshots,
                        cursor: new_cursor,
                    };
                    self.status = format!("deleted snapshot {when_local}");
                }
            }
            Err(e) => {
                self.modal = Modal::None;
                self.status =
                    format!("deleted snapshot, but couldn't refresh list: {e}");
            }
        }
    }

}
