//! OUTLINE-1 — full-screen manuscript Outline pane.
//!
//! This module owns the pane's persisted view state (`OutlineState`). Its
//! renderer + key handling and the structural edits — reorder (`Shift+J/K`),
//! promote/demote (`</>`), and cross-parent paragraph copy/move (`y/m/f`) —
//! shipped in 1.4.13 (O-P1..P4) and run in the TUI app, calling the store's
//! filesystem-aware `swap_siblings` / `move_node_to_parent` (the same primitives
//! the Tree pane and the `inkhaven outline` / `paragraph` CLI share).
//!
//! O-P0 — the state: expanded/collapsed flags, cursor, scroll, and the inline
//! filter string, persisted per-project to `.inkhaven/outline-state.json`
//! (ephemeral UI state; not backed up, not snapshot-versioned).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};

/// One visible row in the Outline pane: the node and its indent depth (root
/// books at depth 0). Built by [`OutlineState::visible_rows`] honoring the
/// expand map (and, from O-P5, the inline filter).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OutlineRow {
    pub id: Uuid,
    pub depth: usize,
}

/// OUTLINE-1 (O-P4) — whether a clipboarded paragraph is being copied
/// (duplicated) or moved (relocated) on the next affix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ClipMode {
    Copy,
    Move,
}

/// OUTLINE-1 (O-P4) — the cross-pane paragraph clipboard. Set by `y` (copy) or
/// `m` (move) in either the Outline or the Tree pane; consumed by `f` (affix),
/// which lands the paragraph as the last child of the target's effective
/// parent. A `Copy` clipboard survives the affix (paste again); a `Move`
/// clipboard is cleared once the relocation lands.
#[derive(Debug, Clone, Copy)]
pub(super) struct ParaClipboard {
    pub id: Uuid,
    pub mode: ClipMode,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(super) struct OutlineState {
    /// Per-node expand flag. Absent → collapsed; `apply_default_view` seeds a
    /// chapter-level default (books expanded) when the map is empty.
    #[serde(default)]
    pub expanded: HashMap<Uuid, bool>,
    #[serde(default)]
    pub cursor_uuid: Option<Uuid>,
    #[serde(default)]
    pub scroll_offset: usize,
    /// Inline `/` filter (O-P5). Empty = no filter.
    #[serde(default)]
    pub filter_str: String,
}

impl OutlineState {
    pub(super) fn sidecar_path(project_root: &Path) -> PathBuf {
        project_root.join(".inkhaven").join("outline-state.json")
    }

    /// Load the saved state. Absent **or malformed** → default (the pane is UI
    /// state — never fail the TUI over a corrupt sidecar).
    pub(super) fn load(project_root: &Path) -> OutlineState {
        match std::fs::read_to_string(Self::sidecar_path(project_root)) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => OutlineState::default(),
        }
    }

    /// Persist atomically (temp + rename via `io_atomic`).
    pub(super) fn save(&self, project_root: &Path) -> std::io::Result<()> {
        let path = Self::sidecar_path(project_root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_vec_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        crate::io_atomic::write(&path, &body)
    }

    /// Whether a branch node is currently expanded (default: collapsed).
    pub(super) fn is_expanded(&self, id: &Uuid) -> bool {
        self.expanded.get(id).copied().unwrap_or(false)
    }

    pub(super) fn set_expanded(&mut self, id: Uuid, expanded: bool) {
        self.expanded.insert(id, expanded);
    }

    pub(super) fn toggle_expanded(&mut self, id: Uuid) {
        let now = self.is_expanded(&id);
        self.expanded.insert(id, !now);
    }

    /// On first open (no persisted expand flags), seed a structural-overview
    /// default: Books and Chapters expanded, everything deeper (Subchapters,
    /// hence their paragraphs) collapsed. Drill-down is one keystroke from
    /// there. No-op once the user has toggled anything (the map is non-empty).
    pub(super) fn seed_default_expansion(&mut self, h: &Hierarchy) {
        if !self.expanded.is_empty() {
            return;
        }
        for n in h.iter() {
            if h.has_children(n.id) && matches!(n.kind, NodeKind::Book | NodeKind::Chapter) {
                self.expanded.insert(n.id, true);
            }
        }
    }

    /// The set of branch nodes (have children) that are *collapsed* — i.e. not
    /// expanded — for feeding [`Hierarchy::flatten_with_collapsed`]. Leaves and
    /// childless branches are never in the set (they don't fold).
    pub(super) fn collapsed_set(&self, h: &Hierarchy) -> HashSet<Uuid> {
        h.iter()
            .filter(|n| h.has_children(n.id) && !self.is_expanded(&n.id))
            .map(|n| n.id)
            .collect()
    }

    /// The currently-visible rows, top to bottom. With no filter this honors
    /// the expand map; with an active `/` filter it ignores folding and shows
    /// the pruned path-to-match tree — every node whose title or slug matches
    /// (case-insensitive, Unicode-aware) plus all of its ancestors.
    pub(super) fn visible_rows(&self, h: &Hierarchy) -> Vec<OutlineRow> {
        let needle = self.filter_str.trim().to_lowercase();
        if needle.is_empty() {
            let collapsed = self.collapsed_set(h);
            return h
                .flatten_with_collapsed(&collapsed)
                .into_iter()
                .map(|(n, depth)| OutlineRow { id: n.id, depth })
                .collect();
        }
        let mut out = Vec::new();
        for root in h.children_of(None) {
            Self::collect_filtered(h, root, 0, &needle, &mut out);
        }
        out
    }

    /// DFS helper for the filtered view. Pushes `node` (before its children)
    /// iff it matches `needle` or has any matching descendant; returns whether
    /// it was kept. Literary scale keeps the `insert`-at-front cost negligible.
    fn collect_filtered(
        h: &Hierarchy,
        node: &Node,
        depth: usize,
        needle: &str,
        out: &mut Vec<OutlineRow>,
    ) -> bool {
        let start = out.len();
        let mut any_child = false;
        for child in h.children_of(Some(node.id)) {
            if Self::collect_filtered(h, child, depth + 1, needle, out) {
                any_child = true;
            }
        }
        let self_match = node.title.to_lowercase().contains(needle)
            || node.slug.to_lowercase().contains(needle);
        if self_match || any_child {
            out.insert(start, OutlineRow { id: node.id, depth });
            true
        } else {
            out.truncate(start);
            false
        }
    }

    /// Index of the cursor within `rows`, or 0 when the cursor uuid is unset or
    /// no longer visible (e.g. its parent was collapsed). The caller clamps.
    pub(super) fn cursor_index(&self, rows: &[OutlineRow]) -> usize {
        self.cursor_uuid
            .and_then(|c| rows.iter().position(|r| r.id == c))
            .unwrap_or(0)
    }

    /// Move the cursor by `delta` rows (saturating at both ends) and update
    /// `cursor_uuid`. Returns the new index. No-op on an empty outline.
    pub(super) fn move_cursor(&mut self, rows: &[OutlineRow], delta: isize) -> usize {
        if rows.is_empty() {
            self.cursor_uuid = None;
            return 0;
        }
        let cur = self.cursor_index(rows) as isize;
        let next = (cur + delta).clamp(0, rows.len() as isize - 1) as usize;
        self.cursor_uuid = Some(rows[next].id);
        next
    }

    /// Snap the cursor onto a visible row after a structural change. If the
    /// current cursor uuid is still visible it is kept; otherwise it lands on
    /// the row at (clamped) `fallback_idx`. Clears it on an empty outline.
    pub(super) fn reanchor_cursor(&mut self, rows: &[OutlineRow], fallback_idx: usize) {
        if rows.is_empty() {
            self.cursor_uuid = None;
            return;
        }
        let still_visible = self
            .cursor_uuid
            .is_some_and(|c| rows.iter().any(|r| r.id == c));
        if !still_visible {
            let idx = fallback_idx.min(rows.len() - 1);
            self.cursor_uuid = Some(rows[idx].id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::node::Node;

    fn node(id: Uuid, kind: &str, slug: &str, parent: Option<Uuid>, order: u32) -> Node {
        serde_json::from_value(serde_json::json!({
            "id": id, "kind": kind, "title": slug, "slug": slug,
            "path": [], "parent_id": parent, "order": order, "file": null,
            "modified_at": "2026-01-01T00:00:00Z",
        }))
        .expect("test node")
    }

    /// book ─ chapter ─ subchapter ─ {p1, p2}; ids returned in that order.
    fn sample() -> (Hierarchy, [Uuid; 5]) {
        let book = Uuid::now_v7();
        let ch = Uuid::now_v7();
        let sub = Uuid::now_v7();
        let p1 = Uuid::now_v7();
        let p2 = Uuid::now_v7();
        let h = Hierarchy::from_nodes_for_test(vec![
            node(book, "book", "b", None, 1),
            node(ch, "chapter", "ch1", Some(book), 1),
            node(sub, "subchapter", "sub1", Some(ch), 1),
            node(p1, "paragraph", "p1", Some(sub), 1),
            node(p2, "paragraph", "p2", Some(sub), 2),
        ]);
        (h, [book, ch, sub, p1, p2])
    }

    #[test]
    fn default_view_expands_book_and_chapter_only() {
        let (h, [book, ch, sub, _p1, _p2]) = sample();
        let mut s = OutlineState::default();
        s.seed_default_expansion(&h);
        // Book + chapter expanded; the subchapter (a branch) is collapsed, so
        // its paragraphs don't show.
        let rows = s.visible_rows(&h);
        let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![book, ch, sub]);
        assert_eq!(rows[0].depth, 0);
        assert_eq!(rows[2].depth, 2);
    }

    #[test]
    fn expanding_subchapter_reveals_paragraphs() {
        let (h, [book, ch, sub, p1, p2]) = sample();
        let mut s = OutlineState::default();
        s.seed_default_expansion(&h);
        s.set_expanded(sub, true);
        let ids: Vec<Uuid> = s.visible_rows(&h).iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![book, ch, sub, p1, p2]);
    }

    #[test]
    fn filter_keeps_matches_and_their_ancestors() {
        let (h, [book, ch, sub, p1, p2]) = sample();
        let mut s = OutlineState::default();
        // A leaf match pulls in its whole ancestor chain, excludes the sibling.
        s.filter_str = "p1".into();
        let ids: Vec<Uuid> = s.visible_rows(&h).iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![book, ch, sub, p1]);
        assert!(!ids.contains(&p2));
        // A branch match shows the path to it; non-matching descendants drop.
        s.filter_str = "ch1".into();
        let ids: Vec<Uuid> = s.visible_rows(&h).iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![book, ch]);
        // Filtering ignores the expand map (folding is bypassed).
        s.set_expanded(ch, false);
        s.filter_str = "p2".into();
        let ids: Vec<Uuid> = s.visible_rows(&h).iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![book, ch, sub, p2]);
        // No match -> empty.
        s.filter_str = "zzz-nope".into();
        assert!(s.visible_rows(&h).is_empty());
    }

    #[test]
    fn seed_is_noop_once_user_has_toggled() {
        let (h, [book, ch, _sub, _p1, _p2]) = sample();
        let mut s = OutlineState::default();
        s.set_expanded(book, true); // prior persisted state ...
        s.set_expanded(ch, false); // ... user had collapsed the chapter
        s.seed_default_expansion(&h); // must not re-expand the chapter
        let ids: Vec<Uuid> = s.visible_rows(&h).iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![book, ch]); // chapter collapsed -> sub hidden
    }

    #[test]
    fn cursor_navigation_clamps_and_tracks_uuid() {
        let (h, [book, ch, sub, _p1, _p2]) = sample();
        let mut s = OutlineState::default();
        s.seed_default_expansion(&h);
        let rows = s.visible_rows(&h); // [book, ch, sub]
        assert_eq!(s.cursor_index(&rows), 0); // unset -> 0
        assert_eq!(s.move_cursor(&rows, 1), 1);
        assert_eq!(s.cursor_uuid, Some(ch));
        assert_eq!(s.move_cursor(&rows, 5), 2); // clamps at end
        assert_eq!(s.cursor_uuid, Some(sub));
        assert_eq!(s.move_cursor(&rows, -9), 0); // clamps at start
        assert_eq!(s.cursor_uuid, Some(book));
    }

    #[test]
    fn reanchor_keeps_visible_cursor_else_falls_back() {
        let (h, [book, ch, sub, _p1, _p2]) = sample();
        let mut s = OutlineState::default();
        s.seed_default_expansion(&h);
        let rows = s.visible_rows(&h);
        s.cursor_uuid = Some(ch);
        s.reanchor_cursor(&rows, 0);
        assert_eq!(s.cursor_uuid, Some(ch)); // still visible -> kept
        // Cursor on a now-hidden node -> falls back to clamped index.
        s.cursor_uuid = Some(Uuid::now_v7());
        s.reanchor_cursor(&rows, 99);
        assert_eq!(s.cursor_uuid, Some(sub)); // clamped to last row
        // Empty outline clears the cursor.
        s.reanchor_cursor(&[], 0);
        assert_eq!(s.cursor_uuid, None);
        let _ = book;
    }

    #[test]
    fn round_trips_through_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let mut s = OutlineState::default();
        s.set_expanded(a, true);
        s.set_expanded(b, false);
        s.cursor_uuid = Some(a);
        s.scroll_offset = 12;
        s.filter_str = "harbour".into();
        s.save(root).unwrap();

        let back = OutlineState::load(root);
        assert_eq!(back, s);
        assert!(back.is_expanded(&a));
        assert!(!back.is_expanded(&b));
        assert_eq!(back.cursor_uuid, Some(a));
        assert_eq!(back.scroll_offset, 12);
        assert_eq!(back.filter_str, "harbour");
    }

    #[test]
    fn absent_and_malformed_default() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // Absent → default.
        assert_eq!(OutlineState::load(root), OutlineState::default());
        // Malformed → default (never panics / errors).
        std::fs::create_dir_all(root.join(".inkhaven")).unwrap();
        std::fs::write(OutlineState::sidecar_path(root), b"{not json").unwrap();
        assert_eq!(OutlineState::load(root), OutlineState::default());
    }

    #[test]
    fn expand_helpers() {
        let mut s = OutlineState::default();
        let id = Uuid::new_v4();
        assert!(!s.is_expanded(&id)); // default collapsed
        s.toggle_expanded(id);
        assert!(s.is_expanded(&id));
        s.toggle_expanded(id);
        assert!(!s.is_expanded(&id));
    }
}
