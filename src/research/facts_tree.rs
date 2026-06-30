//! RESRCH-1 (R-P4) — the Facts tree pane. A fold/expand tree scoped to the
//! Facts system book, with vim navigation, a cursor, pin markers, and the
//! reveal used by `/goto` (R-P12). Read-model only — it holds ids + fold state
//! and is rebuilt from the live `Hierarchy` after any mutation.

use std::collections::HashSet;

use uuid::Uuid;

use crate::store::NodeKind;
use crate::store::hierarchy::Hierarchy;

/// One visible row of the Facts tree (depth-first, fold-aware).
pub(super) struct FactsRow {
    pub id: Uuid,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
}

pub(super) struct FactsTree {
    /// The Facts system book id (the tree root; its children are the top rows).
    pub root: Option<Uuid>,
    expanded: HashSet<Uuid>,
    pub cursor: usize,
    pub scroll: usize,
    visible: Vec<FactsRow>,
}

impl FactsTree {
    pub(super) fn new(h: &Hierarchy) -> FactsTree {
        let root = h
            .iter()
            .find(|n| {
                n.kind == NodeKind::Book
                    && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_FACTS)
            })
            .map(|n| n.id);
        let mut expanded = HashSet::new();
        if let Some(r) = root {
            expanded.insert(r); // the root's children are visible by default
        }
        let mut t = FactsTree { root, expanded, cursor: 0, scroll: 0, visible: Vec::new() };
        t.rebuild(h);
        t
    }

    /// Recompute the visible rows from the hierarchy + fold state.
    pub(super) fn rebuild(&mut self, h: &Hierarchy) {
        self.visible.clear();
        if let Some(root) = self.root {
            self.push_children(h, root, 0);
        }
        if self.cursor >= self.visible.len() {
            self.cursor = self.visible.len().saturating_sub(1);
        }
    }

    fn push_children(&mut self, h: &Hierarchy, parent: Uuid, depth: usize) {
        for child in h.children_of(Some(parent)) {
            // Skip non-content nodes (images/scripts) — facts are paragraphs and
            // their containing chapters/subchapters.
            if matches!(child.kind, NodeKind::Image | NodeKind::Script) {
                continue;
            }
            let kids: Vec<Uuid> = h
                .children_of(Some(child.id))
                .iter()
                .filter(|n| !matches!(n.kind, NodeKind::Image | NodeKind::Script))
                .map(|n| n.id)
                .collect();
            let has_children = !kids.is_empty();
            let expanded = self.expanded.contains(&child.id);
            self.visible.push(FactsRow { id: child.id, depth, has_children, expanded });
            if has_children && expanded {
                self.push_children(h, child.id, depth + 1);
            }
        }
    }

    pub(super) fn rows(&self) -> &[FactsRow] {
        &self.visible
    }

    pub(super) fn is_empty(&self) -> bool {
        self.visible.is_empty()
    }

    pub(super) fn selected(&self) -> Option<Uuid> {
        self.visible.get(self.cursor).map(|r| r.id)
    }

    pub(super) fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub(super) fn move_down(&mut self) {
        if self.cursor + 1 < self.visible.len() {
            self.cursor += 1;
        }
    }

    pub(super) fn to_top(&mut self) {
        self.cursor = 0;
    }

    pub(super) fn to_bottom(&mut self) {
        self.cursor = self.visible.len().saturating_sub(1);
    }

    /// `l` / `→` / `Enter` — expand a folded branch, or step into the first
    /// child of an already-expanded one.
    pub(super) fn step_in(&mut self, h: &Hierarchy) {
        let Some(row) = self.visible.get(self.cursor) else { return };
        if row.has_children {
            if !row.expanded {
                self.expanded.insert(row.id);
                self.rebuild(h);
            } else {
                self.move_down();
            }
        }
    }

    /// `h` / `←` — collapse an expanded branch, or step out to the parent.
    pub(super) fn step_out(&mut self, h: &Hierarchy) {
        let Some(row) = self.visible.get(self.cursor) else { return };
        if row.has_children && row.expanded {
            self.expanded.remove(&row.id);
            self.rebuild(h);
            return;
        }
        // Move the cursor up to the parent row (lower depth, earlier).
        let depth = row.depth;
        if depth == 0 {
            return;
        }
        for i in (0..self.cursor).rev() {
            if self.visible[i].depth < depth {
                self.cursor = i;
                break;
            }
        }
    }

    /// `Enter` toggles fold (or is a no-op on a leaf — callers use it for
    /// navigation too).
    pub(super) fn toggle(&mut self, h: &Hierarchy) {
        let Some(row) = self.visible.get(self.cursor) else { return };
        if !row.has_children {
            return;
        }
        if row.expanded {
            self.expanded.remove(&row.id);
        } else {
            self.expanded.insert(row.id);
        }
        self.rebuild(h);
    }

    /// Expand every ancestor of `id` and place the cursor on it (`/goto`, R-P12).
    /// Returns whether the node was found under the Facts root.
    pub(super) fn reveal(&mut self, h: &Hierarchy, id: Uuid) -> bool {
        let Some(root) = self.root else { return false };
        // Walk ancestors up to the root, expanding each.
        let mut chain = Vec::new();
        let mut cur = Some(id);
        while let Some(c) = cur {
            if c == root {
                break;
            }
            chain.push(c);
            cur = h.get(c).and_then(|n| n.parent_id);
        }
        if cur != Some(root) {
            return false; // not under the Facts book
        }
        for ancestor in chain.iter().skip(1) {
            self.expanded.insert(*ancestor);
        }
        self.rebuild(h);
        if let Some(pos) = self.visible.iter().position(|r| r.id == id) {
            self.cursor = pos;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::node::Node;

    fn node(id: Uuid, kind: &str, parent: Option<Uuid>, order: u32, facts: bool) -> Node {
        let mut raw = serde_json::json!({
            "id": id,
            "kind": kind,
            "title": format!("{kind}-{order}"),
            "slug": format!("{kind}-{order}"),
            "path": [],
            "parent_id": parent,
            "order": order,
            "file": null,
            "modified_at": "2026-01-01T00:00:00Z",
        });
        if facts {
            raw["system_tag"] = serde_json::json!("facts");
        }
        serde_json::from_value(raw).expect("test node deserialises")
    }

    /// facts (book)
    ///   ch1
    ///     p1, p2
    fn sample() -> (Hierarchy, Uuid, Uuid, Uuid, Uuid) {
        let book = Uuid::now_v7();
        let ch1 = Uuid::now_v7();
        let p1 = Uuid::now_v7();
        let p2 = Uuid::now_v7();
        let nodes = vec![
            node(book, "book", None, 1, true),
            node(ch1, "chapter", Some(book), 1, false),
            node(p1, "paragraph", Some(ch1), 1, false),
            node(p2, "paragraph", Some(ch1), 2, false),
        ];
        (Hierarchy::from_nodes_for_test(nodes), book, ch1, p1, p2)
    }

    #[test]
    fn finds_root_and_folds() {
        let (h, book, ch1, _p1, _p2) = sample();
        let mut t = FactsTree::new(&h);
        assert_eq!(t.root, Some(book));
        // ch1 visible, collapsed by default → its paragraphs hidden.
        assert_eq!(t.rows().len(), 1);
        assert_eq!(t.selected(), Some(ch1));
        // Expand ch1 → 2 paragraphs appear.
        t.step_in(&h);
        assert_eq!(t.rows().len(), 3);
        // Collapse again.
        t.step_out(&h);
        assert_eq!(t.rows().len(), 1);
    }

    #[test]
    fn reveal_expands_ancestors() {
        let (h, _book, ch1, _p1, p2) = sample();
        let mut t = FactsTree::new(&h);
        assert_eq!(t.rows().len(), 1); // collapsed
        assert!(t.reveal(&h, p2));
        assert_eq!(t.selected(), Some(p2));
        // ch1 is now expanded.
        assert!(t.rows().iter().any(|r| r.id == ch1 && r.expanded));
    }
}
