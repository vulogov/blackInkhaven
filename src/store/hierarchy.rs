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
}

impl Default for Hierarchy {
    fn default() -> Self {
        Self {
            by_id: HashMap::new(),
            order: Vec::new(),
        }
    }
}

impl Hierarchy {
    pub fn load(store: &Store) -> Result<Self> {
        let raw = store
            .raw()
            .list_metadata()
            .map_err(|e| Error::Store(format!("list_metadata: {e}")))?;

        let mut by_id = HashMap::with_capacity(raw.len());
        for (id, value) in raw {
            // Skip non-hierarchy documents (e.g. chunked bodies) — those won't
            // have our schema. Don't fail the whole load if one is malformed.
            if let Ok(node) = Node::from_json(id, &value) {
                by_id.insert(id, node);
            }
        }

        let mut order: Vec<Uuid> = by_id.keys().copied().collect();
        order.sort_by_key(|id| {
            let n = &by_id[id];
            (n.path.len(), n.order, n.slug.clone())
        });

        Ok(Self { by_id, order })
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

    pub fn children_of(&self, parent_id: Option<Uuid>) -> Vec<&Node> {
        let mut out: Vec<&Node> = self
            .iter()
            .filter(|n| n.parent_id == parent_id)
            .collect();
        out.sort_by_key(|n| n.order);
        out
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
    pub fn has_children(&self, node_id: Uuid) -> bool {
        !self.children_of(Some(node_id)).is_empty()
    }

    pub fn next_order(&self, parent_id: Option<Uuid>) -> u32 {
        self.children_of(parent_id)
            .iter()
            .map(|n| n.order)
            .max()
            .map(|m| m + 1)
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
        let allowed = match (parent.map(|p| p.kind), child_kind) {
            (None, NodeKind::Book) => true,
            (None, _) => false,
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
