//! 1.2.14+ Phase A.2 — Threads picker + weave view
//! sub-module of `tui::app`.
//!
//! `Ctrl+V Shift+H` opens the picker; `w` from
//! inside the picker pushes the weave view as a
//! sub-modal (the picker stored in `return_to` so
//! `Esc` pops back).
//!
//! All HJSON parsing happens at picker-open time
//! and is cached on `ThreadsPickerEntry`; the
//! weave-view grid is also pre-computed at modal-
//! open time so navigation is pure cursor math.
//!
//! See `Documentation/PROPOSALS/1.2.14_PLAN.md`
//! §3.3 for the design.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Deserialize;
use uuid::Uuid;

use crate::store::node::NodeKind;
use crate::store::SYSTEM_TAG_THREADS;

use super::super::input::TextInput;
use super::super::modal::{Modal, ThreadsPickerEntry};
use super::App;

/// Subset of thread HJSON fields the picker / weave
/// view need.  Mirrors `cli::thread::ThreadSummary`
/// but lives here so the TUI module doesn't reach
/// into the CLI crate just for a parse target.
#[derive(Debug, Default, Clone, Deserialize)]
struct ThreadBody {
    #[serde(default)]
    title: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    weight: String,
    #[serde(default)]
    tension: i32,
    #[serde(default)]
    characters: Vec<String>,
    #[serde(default)]
    places: Vec<String>,
}

impl App {
    /// 1.2.14+ Phase A.2 — `Ctrl+V Shift+H` handler.
    /// Walk the Threads system book subtree,
    /// materialise one `ThreadsPickerEntry` per
    /// paragraph (HJSON parse + reverse-link
    /// count), open the picker modal.
    pub(super) fn open_threads_picker(&mut self) {
        let Some(threads_root_id) =
            self.system_book_id(SYSTEM_TAG_THREADS)
        else {
            self.status = "threads picker: Threads system book missing — \
                           re-open the project to seed it"
                .into();
            return;
        };
        let entries = self.collect_thread_picker_entries(threads_root_id);
        if entries.is_empty() {
            self.status = "threads picker: no threads defined — \
                           run `inkhaven thread add <name>`"
                .into();
            return;
        }
        let visible: Vec<usize> = (0..entries.len()).collect();
        self.modal = Modal::ThreadsPicker {
            entries,
            cursor: 0,
            filter: TextInput::new(),
            filter_active: false,
            visible,
        };
        self.status =
            "↑↓ Enter open · Shift+Enter pin to secondary · w weave · / filter · Esc".into();
    }

    fn collect_thread_picker_entries(
        &self,
        threads_root_id: Uuid,
    ) -> Vec<ThreadsPickerEntry> {
        let mut out: Vec<ThreadsPickerEntry> = Vec::new();
        // Build the reverse-link tally in one pass
        // over the hierarchy so we don't pay
        // O(threads * paragraphs) for the per-
        // thread `link_count` field.
        let mut link_tally: std::collections::HashMap<Uuid, usize> =
            std::collections::HashMap::new();
        for node in self.hierarchy.iter() {
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            for target in &node.linked_paragraphs {
                *link_tally.entry(*target).or_insert(0) += 1;
            }
        }
        for id in self.hierarchy.collect_subtree(threads_root_id) {
            if id == threads_root_id {
                continue;
            }
            let Some(node) = self.hierarchy.get(id) else { continue; };
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            let body = match self.store.get_content(id) {
                Ok(Some(bytes)) => bytes,
                _ => continue,
            };
            let body_str = std::str::from_utf8(&body).unwrap_or("");
            let parsed: ThreadBody =
                serde_hjson::from_str(body_str).unwrap_or_default();
            let title_field = if parsed.title.trim().is_empty() {
                node.title.clone()
            } else {
                parsed.title.clone()
            };
            out.push(ThreadsPickerEntry {
                id,
                name: node.title.clone(),
                title_field,
                status: parsed.status,
                weight: parsed.weight,
                tension: parsed.tension,
                character_count: parsed.characters.len(),
                place_count: parsed.places.len(),
                link_count: link_tally.get(&id).copied().unwrap_or(0),
            });
        }
        // Canonical order: by paragraph order index.
        // `collect_subtree` already returns pre-
        // order; that matches the tree pane.  Keep.
        out
    }

    /// 1.2.14+ Phase A.2 — recompute `visible`
    /// indices after a filter edit.  Case-
    /// insensitive substring against name + title
    /// + status + weight.
    fn threads_picker_refilter(&mut self) {
        let Modal::ThreadsPicker {
            entries,
            cursor,
            filter,
            visible,
            ..
        } = &mut self.modal
        else {
            return;
        };
        let f = filter.as_str().to_lowercase();
        let f = f.trim();
        if f.is_empty() {
            *visible = (0..entries.len()).collect();
        } else {
            *visible = entries
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    e.name.to_lowercase().contains(f)
                        || e.title_field.to_lowercase().contains(f)
                        || e.status.to_lowercase().contains(f)
                        || e.weight.to_lowercase().contains(f)
                })
                .map(|(i, _)| i)
                .collect();
        }
        if *cursor >= visible.len() {
            *cursor = visible.len().saturating_sub(1);
        }
    }

    /// 1.2.14+ Phase A.2 — picker key handler.
    pub(super) fn threads_picker_handle_key(
        &mut self,
        key: KeyEvent,
    ) -> bool {
        let Modal::ThreadsPicker {
            entries,
            cursor,
            filter,
            filter_active,
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
                    self.threads_picker_refilter();
                    return true;
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    filter.insert_char(c);
                    self.threads_picker_refilter();
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
                let target_id = visible
                    .get(*cursor)
                    .and_then(|i| entries.get(*i))
                    .map(|e| e.id);
                let pin_to_secondary =
                    key.modifiers.contains(KeyModifiers::SHIFT);
                self.modal = Modal::None;
                if let Some(id) = target_id {
                    if pin_to_secondary {
                        let _ = self.pin_secondary_by_uuid(id);
                    } else if let Some(node) = self.hierarchy.get(id).cloned() {
                        let _ = self.load_paragraph(&node);
                    }
                }
                true
            }
            KeyCode::Char('w') => {
                self.open_thread_weave_view();
                true
            }
            _ => false,
        }
    }

    /// 1.2.14+ Phase A.2 — `w` from picker.
    /// Snapshot the current picker state into the
    /// new sub-modal's `return_to`; compute the
    /// chapter list + per-cell paragraph grid.
    fn open_thread_weave_view(&mut self) {
        let threads: Vec<ThreadsPickerEntry> = match &self.modal {
            Modal::ThreadsPicker { entries, .. } => entries.clone(),
            _ => return,
        };
        if threads.is_empty() {
            return;
        }
        // Every Chapter under every user book in
        // canonical order.  System books (Notes /
        // Threads / Language / etc.) excluded so
        // the weave shows only manuscript chapters.
        let mut chapters: Vec<(Uuid, String, String)> = Vec::new();
        for book in self.hierarchy.children_of(None) {
            if book.kind != NodeKind::Book {
                continue;
            }
            if book.system_tag.is_some() {
                continue;
            }
            for chapter in self.hierarchy.children_of(Some(book.id)) {
                if chapter.kind != NodeKind::Chapter {
                    continue;
                }
                chapters.push((
                    chapter.id,
                    book.title.clone(),
                    chapter.title.clone(),
                ));
            }
        }

        // Pre-compute grid.  For each chapter,
        // collect its subtree's paragraphs; for
        // each thread, count which of those
        // paragraphs links to it.
        let mut chapter_paragraphs: Vec<Vec<Uuid>> =
            Vec::with_capacity(chapters.len());
        for (chapter_id, _, _) in &chapters {
            let mut ids: Vec<Uuid> = Vec::new();
            for id in self.hierarchy.collect_subtree(*chapter_id) {
                let Some(node) = self.hierarchy.get(id) else { continue; };
                if node.kind == NodeKind::Paragraph {
                    ids.push(id);
                }
            }
            chapter_paragraphs.push(ids);
        }
        let mut grid: Vec<Vec<Vec<Uuid>>> =
            Vec::with_capacity(threads.len());
        for thread in &threads {
            let mut row: Vec<Vec<Uuid>> = Vec::with_capacity(chapters.len());
            for ids in &chapter_paragraphs {
                let mut cell: Vec<Uuid> = Vec::new();
                for pid in ids {
                    if let Some(n) = self.hierarchy.get(*pid) {
                        if n.linked_paragraphs.contains(&thread.id) {
                            cell.push(*pid);
                        }
                    }
                }
                row.push(cell);
            }
            grid.push(row);
        }

        let return_to = Box::new(std::mem::replace(&mut self.modal, Modal::None));
        self.modal = Modal::ThreadWeaveView {
            threads,
            chapters,
            grid,
            cursor_row: 0,
            cursor_col: 0,
            scroll_row: 0,
            scroll_col: 0,
            return_to,
        };
        self.status =
            "weave: ↑↓ thread · ←→ chapter · Enter jump to ¶ · Esc back to picker".into();
    }

    /// 1.2.14+ Phase A.2 — weave view key handler.
    pub(super) fn thread_weave_handle_key(&mut self, key: KeyEvent) -> bool {
        let Modal::ThreadWeaveView {
            threads,
            chapters,
            grid,
            cursor_row,
            cursor_col,
            return_to,
            ..
        } = &mut self.modal
        else {
            return false;
        };
        let n_rows = threads.len();
        let n_cols = chapters.len();
        match key.code {
            KeyCode::Up => {
                if *cursor_row > 0 {
                    *cursor_row -= 1;
                }
                true
            }
            KeyCode::Down => {
                if *cursor_row + 1 < n_rows {
                    *cursor_row += 1;
                }
                true
            }
            KeyCode::Left => {
                if *cursor_col > 0 {
                    *cursor_col -= 1;
                }
                true
            }
            KeyCode::Right => {
                if *cursor_col + 1 < n_cols {
                    *cursor_col += 1;
                }
                true
            }
            KeyCode::Home => {
                *cursor_col = 0;
                true
            }
            KeyCode::End => {
                *cursor_col = n_cols.saturating_sub(1);
                true
            }
            KeyCode::Enter => {
                let target = grid
                    .get(*cursor_row)
                    .and_then(|row| row.get(*cursor_col))
                    .and_then(|cell| cell.first())
                    .copied();
                if let Some(id) = target {
                    if let Some(node) = self.hierarchy.get(id).cloned() {
                        self.modal = Modal::None;
                        let _ = self.load_paragraph(&node);
                    } else {
                        self.status = "weave: target paragraph vanished".into();
                    }
                } else {
                    self.status = "weave: this cell has no linking paragraph".into();
                }
                true
            }
            KeyCode::Esc => {
                let restored = std::mem::replace(return_to.as_mut(), Modal::None);
                self.modal = restored;
                true
            }
            _ => false,
        }
    }
}
