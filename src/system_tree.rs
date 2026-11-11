//! A fold/expand tree over a system book (Facts, Languages, …), with vim
//! navigation, a cursor, and a reveal-by-id. Read-model only — it holds ids +
//! fold state and is rebuilt from the live `Hierarchy` after any mutation.
//! Shared by the Research companion (Facts) and the Linguistic companion
//! (Languages); root resolution is parameterised by system-tag.

use std::collections::HashSet;

use uuid::Uuid;

use crate::store::NodeKind;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

/// One visible row of the Facts tree (depth-first, fold-aware).
pub struct SystemBookRow {
    pub id: Uuid,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
}

pub struct SystemBookTree {
    /// The system book id resolved from the tag passed to [`SystemBookTree::new`]
    /// (the tree root; its children are the top rows).
    pub root: Option<Uuid>,
    expanded: HashSet<Uuid>,
    pub cursor: usize,
    pub scroll: usize,
    visible: Vec<SystemBookRow>,
}

impl SystemBookTree {
    /// Build a tree rooted at the system book carrying `system_tag`. The Research
    /// pane passes `SYSTEM_TAG_FACTS`; the root resolution is the only tag-specific
    /// step — everything below (fold state, navigation, reveal) is generic over the
    /// hierarchy, so a companion pane (e.g. the 1.7 Languages book) reuses it by
    /// passing its own tag. (Promotion to a shared `SystemBookTree` module lands
    /// with that second consumer.)
    pub fn new(h: &Hierarchy, system_tag: &str) -> SystemBookTree {
        let root = h
            .iter()
            .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(system_tag))
            .map(|n| n.id);
        let mut expanded = HashSet::new();
        if let Some(r) = root {
            expanded.insert(r); // the root's children are visible by default
        }
        let mut t = SystemBookTree { root, expanded, cursor: 0, scroll: 0, visible: Vec::new() };
        t.rebuild(h);
        t
    }

    /// Recompute the visible rows from the hierarchy + fold state.
    pub fn rebuild(&mut self, h: &Hierarchy) {
        self.rebuild_with(h, None);
    }

    /// [`rebuild`](Self::rebuild) showing only the paragraphs `keep` accepts;
    /// a chapter / subchapter stays visible while it still contains one kept
    /// paragraph somewhere beneath it (3.22 — the worldbuilder's `F` filter
    /// used to be a status-line toggle that changed nothing). Fold state and
    /// the cursor are kept; the cursor is clamped to the rows that remain.
    pub fn rebuild_filtered(&mut self, h: &Hierarchy, keep: &dyn Fn(&Node) -> bool) {
        self.rebuild_with(h, Some(keep));
    }

    fn rebuild_with(&mut self, h: &Hierarchy, keep: Option<&dyn Fn(&Node) -> bool>) {
        self.visible.clear();
        if let Some(root) = self.root {
            self.push_children(h, root, 0, keep);
        }
        if self.cursor >= self.visible.len() {
            self.cursor = self.visible.len().saturating_sub(1);
        }
    }

    /// Whether `node` (a paragraph) or any paragraph beneath it (a branch)
    /// passes `keep`.
    fn subtree_kept(h: &Hierarchy, node: &Node, keep: &dyn Fn(&Node) -> bool, depth: usize) -> bool {
        if depth > 32 {
            return false;
        }
        match node.kind {
            NodeKind::Paragraph => keep(node),
            NodeKind::Image | NodeKind::Script => false,
            _ => h
                .children_of(Some(node.id))
                .into_iter()
                .any(|c| Self::subtree_kept(h, c, keep, depth + 1)),
        }
    }

    fn push_children(&mut self, h: &Hierarchy, parent: Uuid, depth: usize, keep: Option<&dyn Fn(&Node) -> bool>) {
        // Defensive cap: a Facts book is only book → chapter → subchapter →
        // paragraph deep, so any recursion past this means a malformed (cyclic)
        // hierarchy — bail rather than overflow the stack.
        const MAX_DEPTH: usize = 32;
        if depth > MAX_DEPTH {
            return;
        }
        for child in h.children_of(Some(parent)) {
            // Skip non-content nodes (images/scripts) — facts are paragraphs and
            // their containing chapters/subchapters.
            if matches!(child.kind, NodeKind::Image | NodeKind::Script) {
                continue;
            }
            if let Some(keep) = keep {
                if !Self::subtree_kept(h, child, keep, depth) {
                    continue;
                }
            }
            let kids: Vec<Uuid> = h
                .children_of(Some(child.id))
                .iter()
                .filter(|n| !matches!(n.kind, NodeKind::Image | NodeKind::Script))
                .filter(|n| keep.is_none_or(|keep| Self::subtree_kept(h, n, keep, depth + 1)))
                .map(|n| n.id)
                .collect();
            let has_children = !kids.is_empty();
            let expanded = self.expanded.contains(&child.id);
            self.visible.push(SystemBookRow { id: child.id, depth, has_children, expanded });
            if has_children && expanded {
                self.push_children(h, child.id, depth + 1, keep);
            }
        }
    }

    pub fn rows(&self) -> &[SystemBookRow] {
        &self.visible
    }

    pub fn is_empty(&self) -> bool {
        self.visible.is_empty()
    }

    pub fn selected(&self) -> Option<Uuid> {
        self.visible.get(self.cursor).map(|r| r.id)
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        if self.cursor + 1 < self.visible.len() {
            self.cursor += 1;
        }
    }

    pub fn to_top(&mut self) {
        self.cursor = 0;
    }

    pub fn to_bottom(&mut self) {
        self.cursor = self.visible.len().saturating_sub(1);
    }

    /// `l` / `→` / `Enter` — expand a folded branch, or step into the first
    /// child of an already-expanded one.
    pub fn step_in(&mut self, h: &Hierarchy) {
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
    pub fn step_out(&mut self, h: &Hierarchy) {
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
    pub fn toggle(&mut self, h: &Hierarchy) {
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
    pub fn reveal(&mut self, h: &Hierarchy, id: Uuid) -> bool {
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
        let mut t = SystemBookTree::new(&h, crate::store::SYSTEM_TAG_FACTS);
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
        let mut t = SystemBookTree::new(&h, crate::store::SYSTEM_TAG_FACTS);
        assert_eq!(t.rows().len(), 1); // collapsed
        assert!(t.reveal(&h, p2));
        assert_eq!(t.selected(), Some(p2));
        // ch1 is now expanded.
        assert!(t.rows().iter().any(|r| r.id == ch1 && r.expanded));
    }
}
