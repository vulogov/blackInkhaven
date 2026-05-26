//! Methods on `App` that drive the 1.2.5+ project-wide tag
//! picker (`Modal::TagPicker`) and its add / rename / delete /
//! search variants — everything `tag_*` or `open_tag_*` in the
//! original app.rs. Tags themselves live on `Node.tags` in the
//! store; this module is the picker-state choreography on top
//! of those. Extracted from `tui::app` in the 1.2.7 refactor,
//! Phase 3 batch 3.

use crossterm::event::{KeyCode, KeyEvent};
use uuid::Uuid;

use super::{filter_tag_results, handle_text_input_key};

use crate::store::node::NodeKind;

use super::super::input::TextInput;
use super::super::modal::{Modal, TagPickerTarget};

impl super::App {

    /// Ctrl+B ] (editor) — open the tag picker for the currently
    /// open paragraph.
    pub(super) fn open_tag_picker_for_editor(&mut self) {
        let Some(doc) = self.opened.as_ref() else {
            self.status =
                "tag ¶: no paragraph open (Ctrl+B ] needs an editor buffer)".into();
            return;
        };
        let target = TagPickerTarget::EditorParagraph {
            id: doc.id,
            title: doc.title.clone(),
        };
        self.open_tag_picker_modal(target);
    }

    /// `g` (tree pane) — open the tag picker over the tree's
    /// marked set, falling back to the cursor row when no marks
    /// exist. Only paragraph-kind nodes go in; non-paragraphs
    /// are skipped with a status hint when nothing applies.
    pub(super) fn open_tag_picker_for_tree_selection(&mut self) {
        let marked: Vec<Uuid> = self.tree_marked.iter().copied().collect();
        let candidates: Vec<Uuid> = if !marked.is_empty() {
            marked
        } else if let Some(&(id, _)) = self.rows.get(self.tree_cursor) {
            vec![id]
        } else {
            Vec::new()
        };
        let paragraphs: Vec<Uuid> = candidates
            .into_iter()
            .filter(|id| {
                self.hierarchy
                    .get(*id)
                    .map(|n| n.kind == NodeKind::Paragraph)
                    .unwrap_or(false)
            })
            .collect();
        if paragraphs.is_empty() {
            self.status =
                "tag g: select at least one paragraph (Space marks rows in the tree pane)".into();
            return;
        }
        let target = TagPickerTarget::TreeSelection(paragraphs);
        self.open_tag_picker_modal(target);
    }

    /// Ctrl+B } — open the tag picker in search mode.
    pub(super) fn open_tag_search_picker(&mut self) {
        self.open_tag_picker_modal(TagPickerTarget::Search);
    }

    /// Shared open-the-picker plumbing.
    pub(super) fn open_tag_picker_modal(&mut self, target: TagPickerTarget) {
        let all_tags = self.collect_all_tags();
        // 1.2.6+: pre-populate `selected` with the target's current
        // tags so the `[x]/[ ]` markers reflect reality on open.
        // Single-paragraph targets get a set-replace commit (an
        // unchecked tag is removed); multi-paragraph stays additive
        // and so opens empty.
        let preselected: std::collections::BTreeSet<String> = match &target {
            TagPickerTarget::EditorParagraph { id, .. } => self
                .hierarchy
                .get(*id)
                .map(|n| n.tags.iter().cloned().collect())
                .unwrap_or_default(),
            TagPickerTarget::TreeSelection(ids) if ids.len() == 1 => self
                .hierarchy
                .get(ids[0])
                .map(|n| n.tags.iter().cloned().collect())
                .unwrap_or_default(),
            _ => std::collections::BTreeSet::new(),
        };
        // Don't block — an empty tag namespace is the normal
        // starting state; the user adds via `A`.
        let status = match (&target, all_tags.is_empty()) {
            (TagPickerTarget::EditorParagraph { title, .. }, true) => format!(
                "tag ¶ `{title}`: no tags yet — press A to add the first one"
            ),
            (TagPickerTarget::EditorParagraph { title, .. }, false) => format!(
                "tag ¶ `{title}`: Space selects · T applies · A adds · D deletes · Esc closes"
            ),
            (TagPickerTarget::TreeSelection(ids), true) => format!(
                "tag g ({} paragraph(s)): no tags yet — press A to add the first one",
                ids.len()
            ),
            (TagPickerTarget::TreeSelection(ids), false) => format!(
                "tag g ({} paragraph(s)): Space selects · T applies · A adds · D deletes · Esc closes",
                ids.len()
            ),
            (TagPickerTarget::Search, true) => {
                "tag search: no tags yet · A adds · Esc closes".into()
            }
            (TagPickerTarget::Search, false) => {
                "tag search: ↑↓ select · Enter opens results · A adds · D deletes · Esc closes".into()
            }
        };
        self.status = status;
        self.modal = Modal::TagPicker {
            target,
            all_tags,
            cursor: 0,
            selected: preselected,
        };
    }

    pub(super) fn tag_picker_handle_key(&mut self, key: KeyEvent) {
        let total = match &self.modal {
            Modal::TagPicker { all_tags, .. } => all_tags.len(),
            _ => return,
        };
        match key.code {
            KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() => {
                if let Modal::TagPicker { cursor, .. } = &mut self.modal {
                    if *cursor > 0 {
                        *cursor -= 1;
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers.is_empty() => {
                if let Modal::TagPicker { cursor, .. } = &mut self.modal {
                    if total > 0 && *cursor + 1 < total {
                        *cursor += 1;
                    }
                }
            }
            KeyCode::Home => {
                if let Modal::TagPicker { cursor, .. } = &mut self.modal {
                    *cursor = 0;
                }
            }
            KeyCode::End => {
                if let Modal::TagPicker { cursor, .. } = &mut self.modal {
                    *cursor = total.saturating_sub(1);
                }
            }
            KeyCode::Char(' ') => {
                // Multi-select toggle — no-op in Search mode.
                if let Modal::TagPicker {
                    target,
                    all_tags,
                    cursor,
                    selected,
                    ..
                } = &mut self.modal
                {
                    if matches!(target, TagPickerTarget::Search) {
                        return;
                    }
                    if let Some(tag) = all_tags.get(*cursor).cloned() {
                        if selected.contains(&tag) {
                            selected.remove(&tag);
                        } else {
                            selected.insert(tag);
                        }
                    }
                }
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.open_tag_add_prompt();
            }
            KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete => {
                self.open_tag_delete_confirm();
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.open_tag_rename_prompt();
            }
            KeyCode::Char('t') | KeyCode::Char('T') => {
                self.commit_tags_to_target();
            }
            KeyCode::Enter => {
                // Different meaning by mode:
                //   Search       — open results for the cursor tag
                //   Editor/Tree  — quality-of-life: same as T
                let in_search = matches!(
                    self.modal,
                    Modal::TagPicker {
                        target: TagPickerTarget::Search,
                        ..
                    }
                );
                if in_search {
                    self.open_tag_search_results_for_cursor();
                } else {
                    self.commit_tags_to_target();
                }
            }
            _ => {}
        }
    }

    /// `A` — pop a small text-input modal for a new tag name.
    /// On Enter we *don't* immediately apply the tag — we just
    /// add it to the project-wide list (by tagging the current
    /// target with it if there is one) and return to the picker.
    pub(super) fn open_tag_add_prompt(&mut self) {
        // Stash the current picker as return_to.
        let taken = std::mem::replace(&mut self.modal, Modal::None);
        if !matches!(taken, Modal::TagPicker { .. }) {
            // Shouldn't happen but restore and bail safely.
            self.modal = taken;
            return;
        }
        self.modal = Modal::TagAddPrompt {
            input: TextInput::new(),
            return_to: Box::new(taken),
        };
        self.status =
            "new tag: type a name · Tab completes from existing tags · Enter adds · Esc cancels".into();
    }

    pub(super) fn tag_add_prompt_handle_key(&mut self, key: KeyEvent) {
        // Esc is handled at the top of handle_modal_key. Here we
        // act on Tab (autocomplete from existing tags), Enter
        // (commit), and forward everything else to the input box.
        // 1.2.8+ — Tab autocompletes to the first existing
        // project tag whose name starts with the typed prefix
        // (case-insensitive). No-op when no match or the input
        // already names an existing tag.
        if matches!(key.code, KeyCode::Tab) {
            if let Modal::TagAddPrompt { input, return_to } = &mut self.modal {
                let prefix = input.as_str().to_string();
                if !prefix.is_empty() {
                    let lower_prefix = prefix.to_lowercase();
                    if let Modal::TagPicker { all_tags, .. } = return_to.as_ref() {
                        if let Some(hit) = all_tags.iter().find(|t| {
                            t.as_str() != prefix.as_str()
                                && t.to_lowercase().starts_with(&lower_prefix)
                        }) {
                            let hit = hit.clone();
                            input.clear();
                            for c in hit.chars() {
                                input.insert_char(c);
                            }
                        }
                    }
                }
            }
            return;
        }
        if matches!(key.code, KeyCode::Enter) {
            let taken = std::mem::replace(&mut self.modal, Modal::None);
            if let Modal::TagAddPrompt { input, return_to } = taken {
                let name = input.as_str().trim().to_string();
                let mut picker = *return_to;
                if name.is_empty() {
                    self.status = "tag add: empty name — try again".into();
                    self.modal = picker;
                    return;
                }
                // Bring the picker back so we can mutate its state
                // before re-displaying.
                if let Modal::TagPicker {
                    target,
                    all_tags,
                    cursor,
                    selected,
                    ..
                } = &mut picker
                {
                    let already_known =
                        all_tags.iter().any(|t| t == &name);
                    if !already_known {
                        all_tags.push(name.clone());
                        all_tags.sort();
                    }
                    // Land the cursor on the newly added tag.
                    if let Some(idx) =
                        all_tags.iter().position(|t| t == &name)
                    {
                        *cursor = idx;
                    }
                    // Auto-select for convenience in apply-modes
                    // — the user almost certainly wants to T it
                    // onto the target. No-op in Search mode.
                    if !matches!(target, TagPickerTarget::Search) {
                        selected.insert(name.clone());
                    }
                }
                self.modal = picker;
                self.status = format!("tag added: `{name}` · selected");
            }
            return;
        }
        if let Modal::TagAddPrompt { input, .. } = &mut self.modal {
            handle_text_input_key(input, key);
        }
    }

    /// `R` (1.2.6+) — open a project-wide rename prompt for
    /// the cursor tag.
    pub(super) fn open_tag_rename_prompt(&mut self) {
        let (old_tag, affected) = match &self.modal {
            Modal::TagPicker { all_tags, cursor, .. } => {
                let Some(t) = all_tags.get(*cursor).cloned() else {
                    self.status = "tag rename: no tag at cursor".into();
                    return;
                };
                let n = self.count_nodes_with_tag(&t);
                (t, n)
            }
            _ => return,
        };
        let mut input = TextInput::new();
        for c in old_tag.chars() {
            input.insert_char(c);
        }
        let taken = std::mem::replace(&mut self.modal, Modal::None);
        self.modal = Modal::TagRenamePrompt {
            input,
            old_tag: old_tag.clone(),
            affected,
            return_to: Box::new(taken),
        };
        self.status = format!(
            "tag rename `{old_tag}` ({affected} paragraph{plur}): edit + Enter · Esc cancels",
            plur = if affected == 1 { "" } else { "s" },
        );
    }

    pub(super) fn tag_rename_prompt_handle_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Enter) {
            let taken = std::mem::replace(&mut self.modal, Modal::None);
            if let Modal::TagRenamePrompt {
                input,
                old_tag,
                affected: _,
                return_to,
            } = taken
            {
                let new_name = input.as_str().trim().to_string();
                let mut picker = *return_to;
                if new_name.is_empty() || new_name == old_tag {
                    self.modal = picker;
                    self.status = if new_name.is_empty() {
                        "tag rename: empty name — cancelled".into()
                    } else {
                        "tag rename: same name — no-op".into()
                    };
                    return;
                }
                let touched =
                    self.rename_tag_project_wide(&old_tag, &new_name);
                self.reload_hierarchy();
                let fresh = self.collect_all_tags();
                if let Modal::TagPicker {
                    all_tags,
                    cursor,
                    selected,
                    ..
                } = &mut picker
                {
                    *all_tags = fresh;
                    // Land the cursor on the renamed tag's new
                    // position if present.
                    if let Some(idx) =
                        all_tags.iter().position(|t| t == &new_name)
                    {
                        *cursor = idx;
                    } else if *cursor >= all_tags.len().max(1) {
                        *cursor = all_tags.len().saturating_sub(1);
                    }
                    // Update the selection set in case the old
                    // name was selected: swap to the new name.
                    if selected.remove(&old_tag) {
                        selected.insert(new_name.clone());
                    }
                }
                self.modal = picker;
                self.status = format!(
                    "tag renamed: `{old_tag}` → `{new_name}` · touched {touched} paragraph(s)"
                );
            }
            return;
        }
        if let Modal::TagRenamePrompt { input, .. } = &mut self.modal {
            handle_text_input_key(input, key);
        }
    }

    /// `D` — confirm + execute project-wide deletion of the tag
    /// under the cursor. Pops a tiny y/n confirm modal so the
    /// user sees the blast radius first.
    pub(super) fn open_tag_delete_confirm(&mut self) {
        let (tag, affected) = match &self.modal {
            Modal::TagPicker {
                all_tags, cursor, ..
            } => {
                let Some(t) = all_tags.get(*cursor).cloned() else {
                    self.status = "tag delete: no tag selected".into();
                    return;
                };
                let n = self.count_nodes_with_tag(&t);
                (t, n)
            }
            _ => return,
        };
        let taken = std::mem::replace(&mut self.modal, Modal::None);
        self.modal = Modal::TagDeleteConfirm {
            tag: tag.clone(),
            affected,
            return_to: Box::new(taken),
        };
        self.status = format!(
            "delete tag `{tag}`? affects {affected} paragraph(s) · y / n"
        );
    }

    pub(super) fn tag_delete_confirm_handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                let taken = std::mem::replace(&mut self.modal, Modal::None);
                if let Modal::TagDeleteConfirm {
                    tag, return_to, ..
                } = taken
                {
                    let removed = self.delete_tag_project_wide(&tag);
                    self.reload_hierarchy();
                    // Rebuild the picker's all_tags + drop the
                    // deleted entry from its selection.
                    let mut picker = *return_to;
                    let fresh_tags = self.collect_all_tags();
                    if let Modal::TagPicker {
                        all_tags,
                        cursor,
                        selected,
                        ..
                    } = &mut picker
                    {
                        *all_tags = fresh_tags;
                        if *cursor >= all_tags.len().max(1) {
                            *cursor = all_tags.len().saturating_sub(1);
                        }
                        selected.remove(&tag);
                    }
                    self.modal = picker;
                    self.status = format!(
                        "tag deleted: `{tag}` · removed from {removed} paragraph(s)"
                    );
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                let taken = std::mem::replace(&mut self.modal, Modal::None);
                if let Modal::TagDeleteConfirm { return_to, .. } = taken {
                    self.modal = *return_to;
                    self.status = "tag delete: cancelled".into();
                }
            }
            _ => {}
        }
    }

    /// Search mode — Enter on a tag row → open `TagSearchResults`.
    pub(super) fn open_tag_search_results_for_cursor(&mut self) {
        let tag = match &self.modal {
            Modal::TagPicker {
                all_tags, cursor, ..
            } => match all_tags.get(*cursor).cloned() {
                Some(t) => t,
                None => {
                    self.status = "tag search: no tag at cursor".into();
                    return;
                }
            },
            _ => return,
        };
        let results = self.collect_paragraphs_with_tag(&tag);
        if results.is_empty() {
            self.status = format!("tag search: no paragraphs tagged `{tag}`");
            return;
        }
        let count = results.len();
        self.modal = Modal::TagSearchResults {
            tag: tag.clone(),
            filter: TextInput::new(),
            all_results: results,
            cursor: 0,
        };
        self.status = format!(
            "tag `{tag}`: {count} paragraph(s) · type to filter · Enter opens · Esc closes"
        );
    }

    pub(super) fn tag_search_results_handle_key(&mut self, key: KeyEvent) {
        let filtered_len = match &self.modal {
            Modal::TagSearchResults {
                all_results, filter, ..
            } => filter_tag_results(all_results, filter.as_str()).len(),
            _ => return,
        };
        match key.code {
            KeyCode::Up => {
                if let Modal::TagSearchResults { cursor, .. } = &mut self.modal {
                    if *cursor > 0 {
                        *cursor -= 1;
                    }
                }
            }
            KeyCode::Down => {
                if let Modal::TagSearchResults { cursor, .. } = &mut self.modal {
                    if filtered_len > 0 && *cursor + 1 < filtered_len {
                        *cursor += 1;
                    }
                }
            }
            KeyCode::Home => {
                if let Modal::TagSearchResults { cursor, .. } = &mut self.modal {
                    *cursor = 0;
                }
            }
            KeyCode::End => {
                if let Modal::TagSearchResults { cursor, .. } = &mut self.modal {
                    *cursor = filtered_len.saturating_sub(1);
                }
            }
            KeyCode::Enter => {
                let chosen = match &self.modal {
                    Modal::TagSearchResults {
                        all_results,
                        filter,
                        cursor,
                        ..
                    } => filter_tag_results(all_results, filter.as_str())
                        .get(*cursor)
                        .cloned(),
                    _ => None,
                };
                if let Some(entry) = chosen {
                    self.modal = Modal::None;
                    let _ = self.open_search_result(entry.id);
                }
            }
            _ => {
                if let Modal::TagSearchResults { filter, cursor, .. } = &mut self.modal {
                    handle_text_input_key(filter, key);
                    // Reset cursor on filter change so we don't
                    // sit past the filtered list's end.
                    *cursor = 0;
                }
            }
        }
    }

}
