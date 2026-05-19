//! Tree-style filesystem picker for the F3 dialogs.
//!
//! Renders as a flat list whose entries carry a depth + expanded flag, so
//! expanding a directory inlines its children just below it. The picker
//! exposes navigation primitives only; rendering and key dispatch live in
//! `app.rs`.

use std::path::{Path, PathBuf};

/// What the picker should do when the user presses Enter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerContext {
    /// Replace the open paragraph's editor buffer with the picked file's
    /// content. Picking a directory is rejected.
    EditorLoad,
    /// In the Tree pane: a file becomes a new paragraph inserted after the
    /// current cursor; a directory triggers a recursive import (dirs →
    /// subchapters, files → paragraphs).
    TreeInsertOrImport,
}

#[derive(Debug, Clone)]
pub struct FsEntry {
    pub path: PathBuf,
    pub depth: usize,
    pub is_dir: bool,
    pub expanded: bool,
}

pub struct FilePicker {
    pub root: PathBuf,
    pub entries: Vec<FsEntry>,
    pub cursor: usize,
    pub context: PickerContext,
}

impl FilePicker {
    pub fn new(root: PathBuf, context: PickerContext) -> Self {
        let mut picker = Self {
            root,
            entries: Vec::new(),
            cursor: 0,
            context,
        };
        picker.populate_root();
        picker
    }

    fn populate_root(&mut self) {
        self.entries = list_dir(&self.root, 0);
    }

    pub fn current(&self) -> Option<&FsEntry> {
        self.entries.get(self.cursor)
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor + 1 < self.entries.len() {
            self.cursor += 1;
        }
    }

    pub fn page_up(&mut self, n: usize) {
        self.cursor = self.cursor.saturating_sub(n);
    }

    pub fn page_down(&mut self, n: usize) {
        if !self.entries.is_empty() {
            self.cursor = (self.cursor + n).min(self.entries.len() - 1);
        }
    }

    pub fn jump_first(&mut self) {
        self.cursor = 0;
    }

    pub fn jump_last(&mut self) {
        if !self.entries.is_empty() {
            self.cursor = self.entries.len() - 1;
        }
    }

    /// Right arrow: expand the cursor's directory (read its children and
    /// inline them after the cursor). No-op for files or already-expanded
    /// directories.
    pub fn expand(&mut self) {
        let Some(entry) = self.entries.get_mut(self.cursor) else {
            return;
        };
        if !entry.is_dir || entry.expanded {
            return;
        }
        entry.expanded = true;
        let path = entry.path.clone();
        let depth = entry.depth;
        let insert_at = self.cursor + 1;
        let children = list_dir(&path, depth + 1);
        for (i, child) in children.into_iter().enumerate() {
            self.entries.insert(insert_at + i, child);
        }
    }

    /// Left arrow: if the cursor is on an expanded directory, collapse it.
    /// Otherwise move the cursor to its parent entry. If already at the
    /// outermost level, no-op.
    pub fn collapse_or_step_out(&mut self) {
        let Some(entry) = self.entries.get(self.cursor) else {
            return;
        };

        if entry.is_dir && entry.expanded {
            // Remove every subsequent entry at greater depth (i.e. the whole
            // expanded subtree under this dir).
            let cur_depth = entry.depth;
            let from = self.cursor + 1;
            let mut to = from;
            while to < self.entries.len() && self.entries[to].depth > cur_depth {
                to += 1;
            }
            self.entries.drain(from..to);
            self.entries[self.cursor].expanded = false;
            return;
        }

        // Walk back to the nearest entry at depth - 1.
        let cur_depth = entry.depth;
        if cur_depth == 0 {
            return;
        }
        for i in (0..self.cursor).rev() {
            if self.entries[i].depth == cur_depth - 1 {
                self.cursor = i;
                return;
            }
        }
    }
}

fn list_dir(path: &Path, depth: usize) -> Vec<FsEntry> {
    let Ok(rd) = std::fs::read_dir(path) else {
        return Vec::new();
    };
    let mut children: Vec<_> = rd
        .filter_map(Result::ok)
        .filter(|entry| {
            // Skip hidden files (those starting with '.') — they clutter the
            // picker and are usually OS or VCS metadata.
            entry
                .file_name()
                .to_str()
                .map(|s| !s.starts_with('.'))
                .unwrap_or(true)
        })
        .collect();
    children.sort_by(|a, b| {
        let a_dir = a.path().is_dir();
        let b_dir = b.path().is_dir();
        // Dirs first, then alphabetical within each group.
        match (a_dir, b_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });
    children
        .into_iter()
        .map(|entry| {
            let p = entry.path();
            let is_dir = p.is_dir();
            FsEntry {
                path: p,
                depth,
                is_dir,
                expanded: false,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_tree(root: &Path, layout: &[(&str, bool)]) {
        for (rel, is_dir) in layout {
            let p = root.join(rel);
            if *is_dir {
                fs::create_dir_all(&p).unwrap();
            } else {
                if let Some(parent) = p.parent() {
                    fs::create_dir_all(parent).unwrap();
                }
                fs::write(&p, b"").unwrap();
            }
        }
    }

    #[test]
    fn dirs_sort_first_within_level() {
        let tmp = tempfile::tempdir().unwrap();
        write_tree(
            tmp.path(),
            &[
                ("alpha", true),
                ("beta", false),
                ("gamma", true),
                ("delta.txt", false),
            ],
        );
        let p = FilePicker::new(tmp.path().to_path_buf(), PickerContext::EditorLoad);
        let names: Vec<&str> = p
            .entries
            .iter()
            .map(|e| e.path.file_name().unwrap().to_str().unwrap())
            .collect();
        // alpha, gamma (dirs), then beta, delta.txt (files), each
        // alphabetically.
        assert_eq!(names, vec!["alpha", "gamma", "beta", "delta.txt"]);
    }

    #[test]
    fn expand_inlines_children_and_collapse_removes_them() {
        let tmp = tempfile::tempdir().unwrap();
        write_tree(
            tmp.path(),
            &[
                ("dir/a.txt", false),
                ("dir/b.txt", false),
                ("dir/sub/c.txt", false),
                ("z.txt", false),
            ],
        );
        let mut p = FilePicker::new(tmp.path().to_path_buf(), PickerContext::EditorLoad);
        // cursor starts on 'dir'.
        p.expand();
        // sub (dir), a.txt, b.txt inlined.
        assert_eq!(p.entries.len(), 5);
        let names: Vec<&str> = p
            .entries
            .iter()
            .map(|e| e.path.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, vec!["dir", "sub", "a.txt", "b.txt", "z.txt"]);
        // Collapse.
        p.collapse_or_step_out();
        let names: Vec<&str> = p
            .entries
            .iter()
            .map(|e| e.path.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, vec!["dir", "z.txt"]);
    }

    #[test]
    fn left_arrow_on_child_moves_to_parent() {
        let tmp = tempfile::tempdir().unwrap();
        write_tree(tmp.path(), &[("dir/a.txt", false)]);
        let mut p = FilePicker::new(tmp.path().to_path_buf(), PickerContext::EditorLoad);
        p.expand(); // expand dir; cursor stays on 'dir' (idx 0)
        p.move_down(); // move to a.txt (idx 1)
        assert_eq!(p.cursor, 1);
        p.collapse_or_step_out(); // a.txt isn't expanded → move to parent
        assert_eq!(p.cursor, 0);
    }
}
