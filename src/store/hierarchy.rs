use std::collections::HashMap;
use std::path::PathBuf;

use uuid::Uuid;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::{BOOKS_DIR, ProjectLayout};
use crate::store::Store;
use crate::store::node::{Node, NodeKind};

/// In-memory snapshot of every node in the project, loaded from bdslib via
/// `list_metadata()`. Cheap at literary scale (hundreds of nodes).
pub struct Hierarchy {
    by_id: HashMap<Uuid, Node>,
    /// Sorted by (depth, order) so iteration and printing stay stable.
    order: Vec<Uuid>,
    /// 1.2.18+ I.1.5 — parent → ordered child-id list,
    /// built once at construction.  Turns `children_of`
    /// from an O(n) full scan into an O(k) map lookup,
    /// which collapses `flatten`'s O(n²) into O(n).
    ///
    /// The buckets are pre-sorted: `order` is globally
    /// sorted by `(depth, order, slug)`, and a parent's
    /// children all share the same depth, so iterating
    /// `order` to fill the buckets yields each child list
    /// already in `(order, slug)` sequence — exactly what
    /// the old `children_of` produced with its
    /// `sort_by_key(|n| n.order)`.
    ///
    /// `Hierarchy` is immutable after construction
    /// (mutations reload via `Hierarchy::load`), so the
    /// index never needs invalidating.
    children: HashMap<Option<Uuid>, Vec<Uuid>>,
}

impl Default for Hierarchy {
    fn default() -> Self {
        Self {
            by_id: HashMap::new(),
            order: Vec::new(),
            children: HashMap::new(),
        }
    }
}

impl Hierarchy {
    pub fn load(store: &Store) -> Result<Self> {
        // 1.2.18+ I.1.3 — env-gated phase timing
        // (INKHAVEN_PERF_TRACE).  list_metadata is a
        // DuckDB scan; the parse loop is per-row JSON
        // deserialization; the sort is O(n log n) over
        // every node.  Splitting them tells us which to
        // attack in I.1.4+.
        let perf = crate::store::perf_trace_enabled();

        let t0 = std::time::Instant::now();
        let raw = store
            .raw()
            .list_metadata()
            .map_err(|e| Error::Store(format!("list_metadata: {e}")))?;
        crate::store::perf_mark(perf, "hierarchy.load.list_metadata", t0.elapsed());

        let t1 = std::time::Instant::now();
        let mut by_id = HashMap::with_capacity(raw.len());
        for (id, value) in raw {
            // Skip non-hierarchy documents (e.g. chunked bodies) — those won't
            // have our schema. Don't fail the whole load if one is malformed.
            if let Ok(node) = Node::from_json(id, &value) {
                by_id.insert(id, node);
            }
        }
        crate::store::perf_mark(perf, "hierarchy.load.parse_nodes", t1.elapsed());

        let t2 = std::time::Instant::now();
        let mut order: Vec<Uuid> = by_id.keys().copied().collect();
        order.sort_by_key(|id| {
            let n = &by_id[id];
            (n.path.len(), n.order, n.slug.clone())
        });
        crate::store::perf_mark(perf, "hierarchy.load.sort", t2.elapsed());

        // 1.2.18+ I.1.5 — build the parent→children index
        // in one O(n) pass over the already-sorted `order`
        // vec.  Each bucket comes out in display order
        // (see the field docs), so no per-bucket sort.
        let t3 = std::time::Instant::now();
        let children = build_children_index(&order, &by_id);
        crate::store::perf_mark(perf, "hierarchy.load.build_index", t3.elapsed());

        Ok(Self { by_id, order, children })
    }

    /// Test-only constructor: build a Hierarchy from an
    /// in-memory node set without a `Store`.  Mirrors
    /// `load`'s sort + index build.  Used by unit tests
    /// across modules (`reading_time`, the index tests).
    #[cfg(test)]
    pub(crate) fn from_nodes_for_test(nodes: Vec<Node>) -> Self {
        let mut by_id = HashMap::new();
        for n in nodes {
            by_id.insert(n.id, n);
        }
        let mut order: Vec<Uuid> = by_id.keys().copied().collect();
        order.sort_by_key(|id| {
            let n = &by_id[id];
            (n.path.len(), n.order, n.slug.clone())
        });
        let children = build_children_index(&order, &by_id);
        Self { by_id, order, children }
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Node> {
        self.order.iter().map(move |id| &self.by_id[id])
    }

    pub fn get(&self, id: Uuid) -> Option<&Node> {
        self.by_id.get(&id)
    }

    /// Children of `parent_id`, in display order.
    ///
    /// 1.2.18+ I.1.5 — O(k) (k = child count) via the
    /// pre-sorted `children` index, down from the old
    /// O(n) full scan.  This is the change that makes
    /// `flatten` linear.
    pub fn children_of(&self, parent_id: Option<Uuid>) -> Vec<&Node> {
        match self.children.get(&parent_id) {
            Some(ids) => ids.iter().map(|id| &self.by_id[id]).collect(),
            None => Vec::new(),
        }
    }

    /// Depth-first flatten in display order. Each entry is `(node, depth)`
    /// where root books have depth 0.
    pub fn flatten(&self) -> Vec<(&Node, usize)> {
        let mut out: Vec<(&Node, usize)> = Vec::new();
        for root in self.children_of(None) {
            self.walk_into(root, 0, &mut out);
        }
        out
    }

    /// Same as `flatten`, but the children of any node whose id is in
    /// `collapsed` are hidden. The collapsed nodes themselves are still
    /// present in the output — they just don't expand into their subtree.
    pub fn flatten_with_collapsed(
        &self,
        collapsed: &std::collections::HashSet<Uuid>,
    ) -> Vec<(&Node, usize)> {
        let mut out: Vec<(&Node, usize)> = Vec::new();
        for root in self.children_of(None) {
            self.walk_into_collapsed(root, 0, collapsed, &mut out);
        }
        out
    }

    fn walk_into<'a>(&'a self, node: &'a Node, depth: usize, out: &mut Vec<(&'a Node, usize)>) {
        out.push((node, depth));
        for child in self.children_of(Some(node.id)) {
            self.walk_into(child, depth + 1, out);
        }
    }

    fn walk_into_collapsed<'a>(
        &'a self,
        node: &'a Node,
        depth: usize,
        collapsed: &std::collections::HashSet<Uuid>,
        out: &mut Vec<(&'a Node, usize)>,
    ) {
        out.push((node, depth));
        if collapsed.contains(&node.id) {
            return;
        }
        for child in self.children_of(Some(node.id)) {
            self.walk_into_collapsed(child, depth + 1, collapsed, out);
        }
    }

    /// True when `node_id` has at least one child in the hierarchy.
    /// 1.2.18+ I.1.5 — O(1) index lookup; no node materialisation.
    pub fn has_children(&self, node_id: Uuid) -> bool {
        self.children
            .get(&Some(node_id))
            .map(|ids| !ids.is_empty())
            .unwrap_or(false)
    }

    pub fn next_order(&self, parent_id: Option<Uuid>) -> u32 {
        // The index bucket is sorted by order, so the
        // max is the last entry — but stay robust to any
        // future non-monotonic ordering by taking the
        // real max over the (small) child list.
        self.children
            .get(&parent_id)
            .map(|ids| {
                ids.iter()
                    .map(|id| self.by_id[id].order)
                    .max()
                    .map(|m| m + 1)
                    .unwrap_or(1)
            })
            .unwrap_or(1)
    }

    /// Walk up from `start` and return the nearest node (including `start`
    /// itself) whose kind permits `child_kind` as a direct child under `cfg`.
    /// Returns `Ok(None)` when `child_kind` is `Book` (no parent needed).
    pub fn pick_parent_for(
        &self,
        cfg: &Config,
        start: Option<Uuid>,
        child_kind: NodeKind,
    ) -> Result<Option<&Node>> {
        if child_kind == NodeKind::Book {
            return Ok(None);
        }
        let mut current = start;
        while let Some(id) = current {
            let node = self
                .get(id)
                .ok_or_else(|| Error::Store(format!("hierarchy missing node {id}")))?;
            if self
                .validate_placement(cfg, Some(node), child_kind)
                .is_ok()
            {
                return Ok(Some(node));
            }
            current = node.parent_id;
        }
        Err(Error::Store(format!(
            "no ancestor accepts a {} as a child",
            child_kind.as_str()
        )))
    }

    /// IDs of `root` and all its descendants, in pre-order. Use for deletion.
    pub fn collect_subtree(&self, root: Uuid) -> Vec<Uuid> {
        let mut out = Vec::new();
        self.walk_ids(root, &mut out);
        out
    }

    fn walk_ids(&self, node_id: Uuid, out: &mut Vec<Uuid>) {
        if !self.by_id.contains_key(&node_id) {
            return;
        }
        out.push(node_id);
        for child in self.children_of(Some(node_id)) {
            self.walk_ids(child.id, out);
        }
    }

    /// Walk a slash-separated slug path (relative to `books/`) and return the
    /// node it identifies. Paragraphs cannot appear as intermediate segments.
    pub fn find_by_path(&self, path: &str) -> Option<&Node> {
        let segments: Vec<&str> = path
            .split('/')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if segments.is_empty() {
            return None;
        }

        let mut current_parent: Option<Uuid> = None;
        let mut current: Option<&Node> = None;
        for seg in segments {
            let next = self
                .children_of(current_parent)
                .into_iter()
                .find(|n| n.slug == seg)?;
            current_parent = Some(next.id);
            current = Some(next);
        }
        current
    }

    /// Filesystem path for a node, walking its ancestor chain to reconstruct
    /// the correct `NN-slug` prefixes.
    pub fn fs_path(&self, node: &Node, _layout: &ProjectLayout) -> PathBuf {
        let mut p = PathBuf::from(BOOKS_DIR);
        for ancestor in self.ancestors(node) {
            p.push(ancestor.fs_name());
        }
        p.push(node.fs_name());
        p
    }

    /// Ancestors from the root book down to (but not including) `node`.
    pub fn ancestors(&self, node: &Node) -> Vec<&Node> {
        let mut chain: Vec<&Node> = Vec::new();
        let mut cur = node.parent_id;
        while let Some(id) = cur {
            if let Some(parent) = self.by_id.get(&id) {
                chain.push(parent);
                cur = parent.parent_id;
            } else {
                break;
            }
        }
        chain.reverse();
        chain
    }

    /// Slash-separated slug path used in CLI args (e.g. `my-book/01-chapter`).
    pub fn slug_path(&self, node: &Node) -> String {
        let mut parts: Vec<&str> = self
            .ancestors(node)
            .into_iter()
            .map(|n| n.slug.as_str())
            .collect();
        parts.push(&node.slug);
        parts.join("/")
    }

    /// Validate that `child_kind` may be placed under `parent`.
    ///
    /// Default config (unbounded_subchapters = false):
    ///   books → chapter, paragraph, image
    ///   chapter → subchapter, paragraph, image
    ///   subchapter → paragraph, image
    ///
    /// Images sit wherever paragraphs sit — first-class leaves
    /// alongside prose. The wrap_image_* function picked by the
    /// assembler depends on the Image's parent kind (book art /
    /// chapter art / subchapter art).
    ///
    /// With unbounded_subchapters = true, subchapter → subchapter is also OK.
    pub fn validate_placement(
        &self,
        cfg: &Config,
        parent: Option<&Node>,
        child_kind: NodeKind,
    ) -> Result<()> {
        // 1.2.13+ Phase D.1 / hotfix — per-language sub-
        // books are nested Books under the `Language`
        // system book.  The general "Book under any
        // parent is disallowed" rule pre-dates the
        // Language system book and silently broke every
        // scaffold attempt (CLI `inkhaven language init`
        // included) — this special case allows
        // Book → Book ONLY when the parent carries
        // `system_tag == "language"`.
        let parent_is_language_root = parent
            .and_then(|p| p.system_tag.as_deref())
            == Some(crate::store::SYSTEM_TAG_LANGUAGES);
        let allowed = match (parent.map(|p| p.kind), child_kind) {
            (None, NodeKind::Book) => true,
            (None, _) => false,
            (Some(NodeKind::Book), NodeKind::Book) => parent_is_language_root,
            (Some(_), NodeKind::Book) => false,
            (Some(NodeKind::Book), NodeKind::Chapter) => true,
            (Some(NodeKind::Book), NodeKind::Paragraph) => true,
            (Some(NodeKind::Book), NodeKind::Image) => true,
            (Some(NodeKind::Book), NodeKind::Script) => true,
            (Some(NodeKind::Chapter), NodeKind::Subchapter) => true,
            (Some(NodeKind::Chapter), NodeKind::Paragraph) => true,
            (Some(NodeKind::Chapter), NodeKind::Image) => true,
            (Some(NodeKind::Chapter), NodeKind::Script) => true,
            (Some(NodeKind::Subchapter), NodeKind::Paragraph) => true,
            (Some(NodeKind::Subchapter), NodeKind::Image) => true,
            (Some(NodeKind::Subchapter), NodeKind::Script) => true,
            (Some(NodeKind::Subchapter), NodeKind::Subchapter) => {
                cfg.hierarchy.unbounded_subchapters
            }
            _ => false,
        };

        if allowed {
            Ok(())
        } else {
            let parent_desc = parent
                .map(|p| format!("a {}", p.kind.as_str()))
                .unwrap_or_else(|| "the root".into());
            Err(Error::Store(format!(
                "{} cannot be placed under {}",
                child_kind.as_str(),
                parent_desc
            )))
        }
    }
}

/// 1.2.18+ I.1.5 — build the parent→children index.
///
/// One O(n) pass over `order` (which is already sorted
/// by `(depth, order, slug)`).  Because a parent's
/// children all share the same depth, appending each id
/// to its parent's bucket in `order` sequence yields
/// buckets that are already in `(order, slug)` display
/// order — matching the old `children_of`'s
/// `sort_by_key(|n| n.order)` (stable) exactly, with no
/// per-bucket sort.
fn build_children_index(
    order: &[Uuid],
    by_id: &HashMap<Uuid, Node>,
) -> HashMap<Option<Uuid>, Vec<Uuid>> {
    let mut children: HashMap<Option<Uuid>, Vec<Uuid>> = HashMap::new();
    for id in order {
        let parent_id = by_id[id].parent_id;
        children.entry(parent_id).or_default().push(*id);
    }
    children
}

#[cfg(test)]
mod index_tests {
    use super::*;
    use uuid::Uuid;

    /// Build a Node via serde round-trip (same trick the
    /// placement_tests use) so we only specify the fields
    /// the index + ordering read.
    fn node(
        id: Uuid,
        kind: &str,
        slug: &str,
        path: &[&str],
        parent: Option<Uuid>,
        order: u32,
    ) -> Node {
        let raw = serde_json::json!({
            "id": id,
            "kind": kind,
            "title": slug,
            "slug": slug,
            "path": path,
            "parent_id": parent,
            "order": order,
            "file": null,
            "modified_at": "2026-01-01T00:00:00Z",
        });
        serde_json::from_value(raw).expect("test node deserialises")
    }

    /// Construct a Hierarchy from an in-memory node set
    /// — delegates to the shared test constructor.
    fn build(nodes: Vec<Node>) -> Hierarchy {
        Hierarchy::from_nodes_for_test(nodes)
    }

    /// A two-book tree:
    ///   book-a (order 1)
    ///     ch-a1 (order 1)
    ///       p1 (order 2), p2 (order 1)   ← intentionally out of order
    ///   book-b (order 2)
    fn sample() -> (Hierarchy, Vec<Uuid>) {
        let ids: Vec<Uuid> = (0..5).map(|_| Uuid::now_v7()).collect();
        let (book_a, ch_a1, p1, p2, book_b) =
            (ids[0], ids[1], ids[2], ids[3], ids[4]);
        let h = build(vec![
            node(book_a, "book", "book-a", &[], None, 1),
            node(ch_a1, "chapter", "ch-a1", &["book-a"], Some(book_a), 1),
            // p1 has order 2, p2 has order 1 — children_of
            // must return them order-sorted (p2 then p1).
            node(p1, "paragraph", "p1", &["book-a", "ch-a1"], Some(ch_a1), 2),
            node(p2, "paragraph", "p2", &["book-a", "ch-a1"], Some(ch_a1), 1),
            node(book_b, "book", "book-b", &[], None, 2),
        ]);
        (h, ids)
    }

    #[test]
    fn children_of_root_is_order_sorted() {
        let (h, ids) = sample();
        let roots: Vec<Uuid> =
            h.children_of(None).iter().map(|n| n.id).collect();
        assert_eq!(roots, vec![ids[0], ids[4]], "book-a before book-b");
    }

    #[test]
    fn children_of_orders_by_order_field_not_insertion() {
        let (h, ids) = sample();
        // p2 (order 1) must come before p1 (order 2) even
        // though p1 was inserted first.
        let kids: Vec<Uuid> = h
            .children_of(Some(ids[1]))
            .iter()
            .map(|n| n.id)
            .collect();
        assert_eq!(kids, vec![ids[3], ids[2]], "p2 (order 1) before p1 (order 2)");
    }

    #[test]
    fn children_of_leaf_is_empty() {
        let (h, ids) = sample();
        assert!(h.children_of(Some(ids[2])).is_empty());
        assert!(h.children_of(Some(ids[4])).is_empty());
    }

    #[test]
    fn flatten_is_preorder_depth_first() {
        let (h, ids) = sample();
        let flat: Vec<(Uuid, usize)> =
            h.flatten().iter().map(|(n, d)| (n.id, *d)).collect();
        assert_eq!(
            flat,
            vec![
                (ids[0], 0), // book-a
                (ids[1], 1), // ch-a1
                (ids[3], 2), // p2 (order 1)
                (ids[2], 2), // p1 (order 2)
                (ids[4], 0), // book-b
            ],
        );
    }

    #[test]
    fn has_children_matches_index() {
        let (h, ids) = sample();
        assert!(h.has_children(ids[0])); // book-a has ch-a1
        assert!(h.has_children(ids[1])); // ch-a1 has p1+p2
        assert!(!h.has_children(ids[2])); // p1 is a leaf
        assert!(!h.has_children(ids[4])); // book-b is a leaf
    }

    #[test]
    fn next_order_is_max_plus_one() {
        let (h, ids) = sample();
        // Root has book-a(1) + book-b(2) → next is 3.
        assert_eq!(h.next_order(None), 3);
        // ch-a1 has p1(2) + p2(1) → next is 3.
        assert_eq!(h.next_order(Some(ids[1])), 3);
        // A leaf has no children → next is 1.
        assert_eq!(h.next_order(Some(ids[2])), 1);
    }

    #[test]
    fn empty_hierarchy_is_well_formed() {
        let h = Hierarchy::default();
        assert!(h.children_of(None).is_empty());
        assert_eq!(h.next_order(None), 1);
        assert!(h.flatten().is_empty());
    }
}

#[cfg(test)]
mod placement_tests {
    use super::*;
    use uuid::Uuid;

    /// Minimal Book Node for placement tests.  Skips the
    /// every-field expansion `make_event_node` does — only
    /// the fields validate_placement reads (kind +
    /// system_tag) need to be set; the rest fall through to
    /// reasonable defaults via the Node struct's Serde
    /// defaults (we use serde_json round-trip to avoid
    /// listing every field manually).
    fn book(system_tag: Option<&str>) -> Node {
        let raw = serde_json::json!({
            "id": Uuid::nil(),
            "kind": "book",
            "title": "test",
            "slug": "test",
            "path": [],
            "parent_id": null,
            "order": 0,
            "file": null,
            "modified_at": "2026-01-01T00:00:00Z",
            "system_tag": system_tag,
        });
        serde_json::from_value(raw).expect("test node deserialises")
    }

    /// 1.2.13+ hotfix — Book-under-Book is allowed ONLY
    /// when the parent is the Language system book.  This
    /// is the special case that makes
    /// `inkhaven language init` work; the general rule
    /// against nested Books still applies for every other
    /// parent (Notes, Places, user books, etc.).
    #[test]
    fn book_under_language_system_book_is_allowed() {
        let cfg = Config::default();
        let h = Hierarchy::default();
        let parent = book(Some(crate::store::SYSTEM_TAG_LANGUAGES));
        assert!(
            h.validate_placement(&cfg, Some(&parent), NodeKind::Book).is_ok(),
            "Book child under Language system book should be allowed (language sub-book)"
        );
    }

    #[test]
    fn book_under_regular_book_is_rejected() {
        let cfg = Config::default();
        let h = Hierarchy::default();
        let parent = book(None);
        assert!(
            h.validate_placement(&cfg, Some(&parent), NodeKind::Book).is_err(),
            "Book child under a non-system Book parent should still be rejected"
        );
    }

    #[test]
    fn book_under_non_language_system_book_is_rejected() {
        let cfg = Config::default();
        let h = Hierarchy::default();
        // Notes is a system book, but it's not Language —
        // the special case must NOT generalise to every
        // system book.
        let parent = book(Some(crate::store::SYSTEM_TAG_NOTES));
        assert!(
            h.validate_placement(&cfg, Some(&parent), NodeKind::Book).is_err(),
            "Book child under Notes (a non-Language system book) should still be rejected"
        );
    }
}
