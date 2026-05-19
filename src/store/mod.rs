pub mod hierarchy;
pub mod node;

use std::path::Path;
use std::sync::Arc;

use bdslib::DocumentStorage;
use bdslib::EmbeddingEngine;
use bdslib::embedding::Model;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::{BOOKS_DIR, ProjectLayout};
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind as NK};

pub use node::NodeKind;

/// Canonical ordering of the six project-managed books. The tag is the
/// `system_tag` we write into metadata; the second element is the human-
/// facing default title used when the book is freshly created. Display order
/// matches array order (Notes first, Help last).
pub const SYSTEM_BOOKS: &[(&str, &str)] = &[
    ("notes", "Notes"),
    ("research", "Research"),
    ("prompts", "Prompts"),
    ("places", "Places"),
    ("characters", "Characters"),
    ("typst", "Typst"),
    ("help", "Help"),
];

pub const SYSTEM_TAG_NOTES: &str = "notes";
pub const SYSTEM_TAG_PROMPTS: &str = "prompts";
pub const SYSTEM_TAG_PLACES: &str = "places";
pub const SYSTEM_TAG_CHARACTERS: &str = "characters";
pub const SYSTEM_TAG_TYPST: &str = "typst";
pub const SYSTEM_TAG_HELP: &str = "help";

/// Where a newly-created node lands among its parent's existing children.
#[derive(Debug, Clone, Copy)]
pub enum InsertPosition {
    /// Append after the last existing sibling (typical "add at end" behavior).
    End,
    /// Insert immediately after the given sibling; all siblings with order >
    /// the anchor's get bumped by +1 and have their fs entries renamed.
    After(Uuid),
    /// Insert immediately BEFORE the given sibling; the anchor and every
    /// sibling at or after its order get bumped by +1, and the new node
    /// takes the anchor's old order. Mirror of `After`.
    Before(Uuid),
}

/// A versioned snapshot of a paragraph's body at a point in time. Stored as a
/// separate bdslib document with `kind: "snapshot"` so it doesn't pollute the
/// hierarchy listing or vector search.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub id: Uuid,
    #[allow(dead_code)]
    pub parent_id: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub word_count: u64,
    pub preview: String,
}

/// Thin wrapper over bdslib's DocumentStorage with the project's chosen
/// embedding engine attached.
#[derive(Clone)]
pub struct Store {
    inner: DocumentStorage,
    layout: Arc<ProjectLayout>,
}

impl Store {
    pub fn open(layout: ProjectLayout, cfg: &Config) -> Result<Self> {
        let root = layout
            .store_root()
            .to_str()
            .ok_or_else(|| Error::Store("project root path is not valid UTF-8".into()))?;

        let engine = build_embedding_engine(&cfg.embeddings.model)?;
        let inner = DocumentStorage::with_embedding(root, engine).map_err(|e| {
            Error::Store(format!(
                "couldn't open the document store at {} — {}.\n\
                 Another inkhaven process may be using the project, or the database \
                 may be corrupt. If you have a backup, restore it; otherwise \
                 `inkhaven init` a fresh project and re-add your work.",
                layout.root.display(),
                e
            ))
        })?;

        let store = Self {
            inner,
            layout: Arc::new(layout),
        };
        store.ensure_system_books(cfg)?;
        store.ensure_artefacts_directory(cfg)?;
        Ok(store)
    }

    /// Create the per-project artefacts root (config field
    /// `artefacts_directory`) if it doesn't exist yet. Each user book
    /// gets its own subdirectory under here for PDFs / build
    /// intermediates / etc. Empty `artefacts_directory` disables the
    /// hook.
    fn ensure_artefacts_directory(&self, cfg: &Config) -> Result<()> {
        let dir = cfg.artefacts_directory.trim();
        if dir.is_empty() {
            return Ok(());
        }
        let abs = self.resolve_artefacts_dir(cfg);
        std::fs::create_dir_all(&abs).map_err(Error::Io)?;
        Ok(())
    }

    /// Resolve `cfg.artefacts_directory` against the project root.
    /// Absolute paths are used verbatim; relative paths join the layout
    /// root.
    pub fn resolve_artefacts_dir(&self, cfg: &Config) -> std::path::PathBuf {
        let raw = std::path::PathBuf::from(&cfg.artefacts_directory);
        if raw.is_absolute() {
            raw
        } else {
            self.layout.root.join(raw)
        }
    }

    /// Provision the side-effects of creating a user book at the root
    /// level: an artefacts subdirectory matching the book's slug, and a
    /// Typst-book chapter (same display name as the new book) carrying
    /// three starter paragraphs — `index.typ`, `settings.typ`,
    /// `globals.typ`. `index.typ` is seeded with `import` statements
    /// pulling in the other two so a fresh book's Typst skeleton is
    /// ready to render.
    ///
    /// Idempotent: re-calling with the same book is a no-op for any
    /// piece that's already in place.
    ///
    /// Called from both the TUI's `commit_add` (when creating a Book at
    /// root via the Tree pane) and the CLI's `add` subcommand.
    pub fn provision_user_book(
        &self,
        cfg: &Config,
        book_node: &Node,
    ) -> Result<()> {
        // Only root-level Books trigger provisioning.
        if book_node.kind != NK::Book || book_node.parent_id.is_some() {
            return Ok(());
        }
        // Skip system books — they shouldn't get artefact subdirs or a
        // self-referential Typst entry.
        if book_node.system_tag.is_some() {
            return Ok(());
        }

        // (a) Artefacts subdirectory: <artefacts_directory>/<book-slug>/
        if !cfg.artefacts_directory.trim().is_empty() {
            let sub = self.resolve_artefacts_dir(cfg).join(&book_node.slug);
            std::fs::create_dir_all(&sub).map_err(Error::Io)?;
        }

        // (b) Typst-book chapter + three starter paragraphs.
        self.ensure_typst_skeleton(cfg, &book_node.title)?;
        Ok(())
    }

    /// Ensure a chapter named after `book_title` exists inside the
    /// Typst system book, and that it contains paragraphs `index.typ`,
    /// `settings.typ`, `globals.typ`. Each part is created only if
    /// missing — safe to call repeatedly.
    fn ensure_typst_skeleton(&self, cfg: &Config, book_title: &str) -> Result<()> {
        let hierarchy = Hierarchy::load(self)?;
        let Some(typst_book) = hierarchy
            .iter()
            .find(|n| n.kind == NK::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_TYPST))
            .cloned()
        else {
            // Typst system book is missing — shouldn't happen because
            // ensure_system_books seeds it on every open, but bail
            // cleanly rather than panic if the hierarchy is unusual.
            return Ok(());
        };

        // Find or create the chapter matching the book's title.
        let chapter = match hierarchy
            .iter()
            .find(|n| n.kind == NK::Chapter
                && n.parent_id == Some(typst_book.id)
                && n.title == book_title)
            .cloned()
        {
            Some(n) => n,
            None => self.create_node(
                cfg,
                &hierarchy,
                NK::Chapter,
                book_title,
                Some(&typst_book),
                None,
                InsertPosition::End,
            )?,
        };

        // Each paragraph needs a deterministic title and a starter body.
        const SEEDS: &[(&str, &str)] = &[
            (
                "index.typ",
                "= index.typ\n\n#import \"globals.typ\": *\n#import \"settings.typ\": *\n",
            ),
            (
                "settings.typ",
                "= settings.typ\n\n// Document-wide #set / #show rules go here.\n",
            ),
            (
                "globals.typ",
                "= globals.typ\n\n// Project-wide values and helpers go here.\n",
            ),
        ];
        // Reload hierarchy after each create so subsequent lookups see
        // freshly-added siblings.
        for (title, body) in SEEDS {
            let h = Hierarchy::load(self)?;
            let already = h.iter().any(|n| {
                n.kind == NK::Paragraph
                    && n.parent_id == Some(chapter.id)
                    && n.title == *title
            });
            if already {
                continue;
            }
            let mut created = self.create_node(
                cfg,
                &h,
                NK::Paragraph,
                title,
                Some(&chapter),
                None,
                InsertPosition::End,
            )?;
            // Overwrite the auto-generated `= Title\n\n` body with the
            // seeded content. The fs file already exists from
            // `create_node`'s paragraph branch.
            if let Some(rel) = &created.file {
                let abs = self.layout.root.join(rel);
                std::fs::write(&abs, body.as_bytes()).map_err(Error::Io)?;
                self.update_paragraph_content(&mut created, body.as_bytes())?;
            }
        }
        Ok(())
    }

    /// Six books that every project keeps at the root, in this order:
    /// Notes, Research, Prompts, Places, Characters, Help. They are created
    /// on first open (or on upgrade from a pre-feature project), tagged with
    /// `system_tag`, and marked `protected=true`. Help is additionally read-
    /// only — enforced by the editor layer, not the store, so the underlying
    /// content can still be authored by tooling or future bundled-help logic.
    fn ensure_system_books(&self, cfg: &Config) -> Result<()> {
        let hierarchy = Hierarchy::load(self)?;
        let mut existing_by_tag: std::collections::HashMap<String, Node> =
            std::collections::HashMap::new();
        for node in hierarchy.iter() {
            if node.kind == NK::Book {
                if let Some(tag) = node.system_tag.as_deref() {
                    existing_by_tag.insert(tag.to_string(), node.clone());
                }
            }
        }

        for (idx, (tag, title)) in SYSTEM_BOOKS.iter().enumerate() {
            let target_order = idx as u32;
            match existing_by_tag.get(*tag).cloned() {
                Some(mut node) => {
                    // System book already exists. Only re-stamp the flags
                    // that should never go missing (protected). Leave its
                    // `order` alone — user-added books may have shifted it
                    // away from the canonical position and we must not undo
                    // those shifts on every project open.
                    if !node.protected {
                        node.protected = true;
                        self.inner
                            .update_metadata(node.id, node.to_json())
                            .map_err(|e| {
                                Error::Store(format!("update_metadata: {e}"))
                            })?;
                    }
                }
                None => {
                    // First-time creation: the canonical order matches the
                    // SYSTEM_BOOKS array index. Reload before each create so
                    // subsequent slugs see prior creates and don't collide.
                    let h = Hierarchy::load(self)?;
                    let mut node = self.create_node(
                        cfg,
                        &h,
                        NK::Book,
                        title,
                        None,
                        None,
                        InsertPosition::End,
                    )?;
                    node.order = target_order;
                    node.protected = true;
                    node.system_tag = Some(tag.to_string());
                    self.inner
                        .update_metadata(node.id, node.to_json())
                        .map_err(|e| Error::Store(format!("update_metadata: {e}")))?;
                }
            }
        }
        self.sync()?;
        Ok(())
    }

    pub fn raw(&self) -> &DocumentStorage {
        &self.inner
    }

    /// Project root path (the directory the user passed via --project).
    /// Read-only accessor — callers should not mutate the layout.
    pub fn project_root(&self) -> &std::path::Path {
        &self.layout.root
    }

    pub fn sync(&self) -> Result<()> {
        self.inner.sync().map_err(|e| Error::Store(e.to_string()))
    }

    /// Add a hierarchy node to bdslib. The metadata is serialized; the content
    /// bytes are indexed for vector search. Returns the bdslib-assigned UUIDv7
    /// after we copy it back onto the Node.
    pub fn put_node(&self, node: &mut Node, content: &[u8]) -> Result<()> {
        let id = self
            .inner
            .add_document(node.to_json(), content)
            .map_err(|e| Error::Store(format!("add_document: {e}")))?;
        node.id = id;
        // Re-write metadata with the now-final id available to consumers.
        self.inner
            .update_metadata(id, node.to_json())
            .map_err(|e| Error::Store(format!("update_metadata: {e}")))?;
        Ok(())
    }

    pub fn search_text(&self, query: &str, limit: usize) -> Result<Vec<JsonValue>> {
        self.inner
            .search_document_text(query, limit)
            .map_err(|e| Error::Store(format!("search_document_text: {e}")))
    }

    pub fn get_content(&self, id: Uuid) -> Result<Option<Vec<u8>>> {
        self.inner
            .get_content(id)
            .map_err(|e| Error::Store(format!("get_content: {e}")))
    }

    /// Create a hierarchy node end-to-end: validate placement, derive a unique
    /// slug, compute the filesystem path, write a .typ template (paragraphs) or
    /// create a directory (branches), insert into bdslib, sync. Returns the
    /// fully-populated `Node`. Callers should reload `Hierarchy` afterward.
    ///
    /// `position` controls where among existing siblings the new node lands:
    ///   * `InsertPosition::End` — appended after the last sibling (default).
    ///   * `InsertPosition::After(uuid)` — placed immediately after the named
    ///     sibling; all later siblings get their `order` bumped by +1 and
    ///     their filesystem entries renamed.
    pub fn create_node(
        &self,
        cfg: &Config,
        hierarchy: &Hierarchy,
        kind: NodeKind,
        title: &str,
        parent: Option<&Node>,
        slug_override: Option<&str>,
        position: InsertPosition,
    ) -> Result<Node> {
        hierarchy.validate_placement(cfg, parent, kind)?;

        let slug_seed = slug_override.unwrap_or(title);
        let mut slug = slug::slugify(slug_seed);
        if slug.is_empty() {
            return Err(Error::Store(format!(
                "could not derive a slug from `{slug_seed}`; pass --slug"
            )));
        }

        let parent_id = parent.map(|p| p.id);
        let siblings = hierarchy.children_of(parent_id);
        if siblings.iter().any(|n| n.slug == slug) {
            let base = slug.clone();
            let mut n = 2;
            while siblings.iter().any(|s| s.slug == slug) {
                slug = format!("{base}-{n}");
                n += 1;
            }
        }

        // Decide the new node's `order`. For After(anchor), shift every
        // sibling strictly after the anchor by +1 (highest order first so
        // filesystem renames never collide).
        let order = match position {
            InsertPosition::End => hierarchy.next_order(parent_id),
            InsertPosition::After(anchor_id) => {
                let Some(anchor) = hierarchy.get(anchor_id) else {
                    return Err(Error::Store(format!("insert-after: missing anchor {anchor_id}")));
                };
                if anchor.parent_id != parent_id {
                    return Err(Error::Store(
                        "insert-after: anchor does not share the requested parent".into(),
                    ));
                }
                let anchor_order = anchor.order;
                let mut to_shift: Vec<(Uuid, u32)> = hierarchy
                    .children_of(parent_id)
                    .into_iter()
                    .filter(|n| n.order > anchor_order && n.id != anchor_id)
                    .map(|n| (n.id, n.order))
                    .collect();
                to_shift.sort_by_key(|(_, ord)| std::cmp::Reverse(*ord));
                for (id, old_order) in to_shift {
                    self.shift_sibling_order(hierarchy, id, old_order + 1)?;
                }
                anchor_order + 1
            }
            InsertPosition::Before(anchor_id) => {
                let Some(anchor) = hierarchy.get(anchor_id) else {
                    return Err(Error::Store(format!(
                        "insert-before: missing anchor {anchor_id}"
                    )));
                };
                if anchor.parent_id != parent_id {
                    return Err(Error::Store(
                        "insert-before: anchor does not share the requested parent".into(),
                    ));
                }
                let anchor_order = anchor.order;
                // Shift the anchor and everything after it up by 1, highest
                // first so renames never collide. The new node then takes
                // the anchor's old order.
                let mut to_shift: Vec<(Uuid, u32)> = hierarchy
                    .children_of(parent_id)
                    .into_iter()
                    .filter(|n| n.order >= anchor_order)
                    .map(|n| (n.id, n.order))
                    .collect();
                to_shift.sort_by_key(|(_, ord)| std::cmp::Reverse(*ord));
                for (id, old_order) in to_shift {
                    self.shift_sibling_order(hierarchy, id, old_order + 1)?;
                }
                anchor_order
            }
        };

        let path_chain: Vec<String> = match parent {
            None => Vec::new(),
            Some(p) => {
                let mut chain: Vec<String> = hierarchy
                    .ancestors(p)
                    .into_iter()
                    .map(|a| a.slug.clone())
                    .collect();
                chain.push(p.slug.clone());
                chain
            }
        };

        let mut node = Node {
            id: Uuid::nil(),
            kind,
            title: title.to_string(),
            slug,
            path: path_chain,
            parent_id,
            order,
            file: None,
            word_count: 0,
            modified_at: chrono::Utc::now(),
            protected: false,
            system_tag: None,
        };

        let rel_path = match parent {
            None => std::path::PathBuf::from(BOOKS_DIR).join(node.fs_name()),
            Some(p) => hierarchy.fs_path(p, &self.layout).join(node.fs_name()),
        };
        let abs_path = self.layout.root.join(&rel_path);

        let content: Vec<u8> = match kind {
            NK::Paragraph => {
                if let Some(parent_dir) = abs_path.parent() {
                    std::fs::create_dir_all(parent_dir)?;
                }
                let template = format!("= {}\n\n", node.title);
                std::fs::write(&abs_path, &template)?;
                node.file = Some(rel_path.to_string_lossy().into_owned());
                node.word_count = template.split_whitespace().count() as u64;
                template.into_bytes()
            }
            _ => {
                std::fs::create_dir_all(&abs_path)?;
                node.title.clone().into_bytes()
            }
        };

        self.put_node(&mut node, &content)?;
        self.sync()?;
        Ok(node)
    }

    /// Change a single node's `order` by renaming its filesystem entry and
    /// updating bdslib metadata (plus descendant `file` paths for branches).
    /// Errors if a sibling already occupies the target slot — callers are
    /// expected to process shifts highest-order-first to avoid this.
    fn shift_sibling_order(
        &self,
        hierarchy: &Hierarchy,
        node_id: Uuid,
        new_order: u32,
    ) -> Result<()> {
        let node = hierarchy
            .get(node_id)
            .cloned()
            .ok_or_else(|| Error::Store(format!("shift_sibling_order: missing {node_id}")))?;
        if node.order == new_order {
            return Ok(());
        }
        let old_rel = hierarchy.fs_path(&node, &self.layout);
        let old_abs = self.layout.root.join(&old_rel);

        let mut new_node = node.clone();
        new_node.order = new_order;
        let new_name = new_node.fs_name();
        let parent_dir = old_abs
            .parent()
            .ok_or_else(|| Error::Store("filesystem entry has no parent directory".into()))?;
        let new_abs = parent_dir.join(&new_name);

        // Books carry no `NN-` order prefix in their filesystem name, so a
        // reorder produces an identical path. Skip the rename in that case;
        // only metadata changes. (The `new_abs.exists()` guard would
        // otherwise reject the no-op as a collision.)
        let needs_rename = old_abs != new_abs;
        if needs_rename {
            if new_abs.exists() {
                return Err(Error::Store(format!(
                    "shift_sibling_order: target `{}` already exists",
                    new_abs.display()
                )));
            }
            std::fs::rename(&old_abs, &new_abs)?;
        }

        let new_rel = new_abs
            .strip_prefix(&self.layout.root)
            .unwrap_or(&new_abs)
            .to_string_lossy()
            .into_owned();
        if new_node.file.is_some() {
            new_node.file = Some(new_rel.clone());
        }
        self.inner
            .update_metadata(new_node.id, new_node.to_json())
            .map_err(|e| Error::Store(format!("update_metadata: {e}")))?;
        if node.kind != NK::Paragraph {
            self.rewrite_descendant_files(hierarchy, &node, &old_rel, &new_rel)?;
        }
        Ok(())
    }

    /// Change a node's displayed `title` without touching the filesystem.
    /// Slug, order, parent, file path all stay the same — only the
    /// `metadata.title` JSON field is updated in bdslib. Re-embeds so the
    /// new title participates in semantic search.
    pub fn rename_node(&self, hierarchy: &Hierarchy, node_id: Uuid, new_title: &str) -> Result<()> {
        let mut node = hierarchy
            .get(node_id)
            .cloned()
            .ok_or_else(|| Error::Store(format!("rename_node: missing {node_id}")))?;
        let trimmed = new_title.trim();
        if trimmed.is_empty() {
            return Err(Error::Store("rename: title cannot be empty".into()));
        }
        node.title = trimmed.to_string();
        node.modified_at = chrono::Utc::now();
        self.inner
            .update_metadata(node.id, node.to_json())
            .map_err(|e| Error::Store(format!("update_metadata: {e}")))?;
        self.inner
            .reembed_document(node.id)
            .map_err(|e| Error::Store(format!("reembed_document: {e}")))?;
        self.sync()?;
        Ok(())
    }

    /// Swap two sibling nodes in the hierarchy: exchange their `order` fields,
    /// rename the corresponding filesystem entries, and update every
    /// descendant paragraph's `file` path that points through a renamed
    /// branch directory.
    ///
    /// Caller passes both nodes by id; they must share the same `parent_id`.
    /// On error, partial filesystem renames may leave drift — `inkhaven
    /// reindex` will catch it.
    pub fn swap_siblings(
        &self,
        hierarchy: &Hierarchy,
        a_id: Uuid,
        b_id: Uuid,
    ) -> Result<()> {
        if a_id == b_id {
            return Err(Error::Store("cannot swap a node with itself".into()));
        }
        let a = hierarchy
            .get(a_id)
            .ok_or_else(|| Error::Store(format!("node {a_id} missing from hierarchy")))?
            .clone();
        let b = hierarchy
            .get(b_id)
            .ok_or_else(|| Error::Store(format!("node {b_id} missing from hierarchy")))?
            .clone();
        if a.parent_id != b.parent_id {
            return Err(Error::Store("can only swap siblings".into()));
        }

        let a_old_rel = hierarchy.fs_path(&a, &self.layout);
        let b_old_rel = hierarchy.fs_path(&b, &self.layout);
        let a_old_abs = self.layout.root.join(&a_old_rel);
        let b_old_abs = self.layout.root.join(&b_old_rel);

        // Build the new nodes with swapped orders so we can compute new
        // filesystem segment names via `fs_name()`.
        let mut a_new = a.clone();
        a_new.order = b.order;
        let mut b_new = b.clone();
        b_new.order = a.order;

        let a_new_name = a_new.fs_name();
        let b_new_name = b_new.fs_name();

        // Both new names exist in the same parent directory.
        let parent_dir = a_old_abs
            .parent()
            .ok_or_else(|| Error::Store("filesystem entry has no parent directory".into()))?;
        let a_new_abs = parent_dir.join(&a_new_name);
        let b_new_abs = parent_dir.join(&b_new_name);

        // Two-step rename to avoid any chance of clobbering when the slugs
        // happen to collide with the swapped order prefix. Use temp names
        // adjacent to the originals.
        let tmp_a = parent_dir.join(format!(".inkhaven-mv-a-{}", a.id.as_simple()));
        let tmp_b = parent_dir.join(format!(".inkhaven-mv-b-{}", b.id.as_simple()));

        std::fs::rename(&a_old_abs, &tmp_a)?;
        if let Err(e) = std::fs::rename(&b_old_abs, &tmp_b) {
            // Roll back the first rename so we don't leave the project broken.
            let _ = std::fs::rename(&tmp_a, &a_old_abs);
            return Err(Error::Io(e));
        }
        if let Err(e) = std::fs::rename(&tmp_a, &a_new_abs) {
            let _ = std::fs::rename(&tmp_b, &b_old_abs);
            return Err(Error::Io(e));
        }
        std::fs::rename(&tmp_b, &b_new_abs)?;

        // Update file paths on the moved nodes themselves (paragraphs only).
        let a_new_rel = a_new_abs
            .strip_prefix(&self.layout.root)
            .unwrap_or(&a_new_abs)
            .to_string_lossy()
            .into_owned();
        let b_new_rel = b_new_abs
            .strip_prefix(&self.layout.root)
            .unwrap_or(&b_new_abs)
            .to_string_lossy()
            .into_owned();
        if a_new.file.is_some() {
            a_new.file = Some(a_new_rel.clone());
        }
        if b_new.file.is_some() {
            b_new.file = Some(b_new_rel.clone());
        }

        // Persist both nodes' metadata.
        self.inner
            .update_metadata(a_new.id, a_new.to_json())
            .map_err(|e| Error::Store(format!("update_metadata: {e}")))?;
        self.inner
            .update_metadata(b_new.id, b_new.to_json())
            .map_err(|e| Error::Store(format!("update_metadata: {e}")))?;

        // For each renamed branch, rewrite descendants' `file` fields so they
        // point at the new directory.
        if a.kind != NK::Paragraph {
            self.rewrite_descendant_files(hierarchy, &a, &a_old_rel, &a_new_rel)?;
        }
        if b.kind != NK::Paragraph {
            self.rewrite_descendant_files(hierarchy, &b, &b_old_rel, &b_new_rel)?;
        }

        self.sync()?;
        Ok(())
    }

    /// Walk descendants of `moved` and rewrite each paragraph's `file` field
    /// so the prefix that used to be `old_rel` becomes `new_rel`.
    fn rewrite_descendant_files(
        &self,
        hierarchy: &Hierarchy,
        moved: &Node,
        old_rel: &std::path::Path,
        new_rel: &str,
    ) -> Result<()> {
        let old_prefix = old_rel.to_string_lossy().into_owned();
        for descendant_id in hierarchy.collect_subtree(moved.id) {
            if descendant_id == moved.id {
                continue;
            }
            let Some(descendant) = hierarchy.get(descendant_id) else {
                continue;
            };
            if descendant.kind != NK::Paragraph {
                continue;
            }
            let Some(old_file) = descendant.file.as_ref() else {
                continue;
            };
            if let Some(rest) = old_file.strip_prefix(&old_prefix) {
                let new_file = format!("{new_rel}{rest}");
                let mut updated = descendant.clone();
                updated.file = Some(new_file);
                self.inner
                    .update_metadata(updated.id, updated.to_json())
                    .map_err(|e| Error::Store(format!("update_metadata: {e}")))?;
            }
        }
        Ok(())
    }

    // -------- snapshots ---------------------------------------------------

    /// Create a versioned snapshot of `parent`'s current content. Stored as a
    /// bdslib document with `kind:"snapshot"` and a `parent_id` back-reference.
    /// Snapshots are added via `add_document_no_embed` so they don't appear in
    /// vector search results — only the live paragraph version does.
    pub fn create_snapshot(&self, parent: &Node, content: &[u8]) -> Result<Uuid> {
        let preview = first_prose_line(content);
        let word_count = std::str::from_utf8(content)
            .map(|s| s.split_whitespace().count() as u64)
            .unwrap_or(0);
        let meta = serde_json::json!({
            "kind": "snapshot",
            "parent_id": parent.id.to_string(),
            "parent_title": parent.title,
            "created_at": chrono::Utc::now().to_rfc3339(),
            "word_count": word_count,
            "preview": preview,
        });
        let id = self
            .inner
            .add_document_no_embed(meta, content)
            .map_err(|e| Error::Store(format!("create_snapshot: {e}")))?;
        self.sync()?;
        Ok(id)
    }

    /// Every snapshot whose `parent_id` matches the given paragraph,
    /// returned newest first.
    pub fn list_snapshots(&self, parent_id: Uuid) -> Result<Vec<Snapshot>> {
        let pid = parent_id.to_string();
        let raw = self
            .inner
            .list_metadata()
            .map_err(|e| Error::Store(format!("list_metadata: {e}")))?;
        let mut out: Vec<Snapshot> = raw
            .into_iter()
            .filter_map(|(id, meta)| {
                let kind = meta.get("kind").and_then(|v| v.as_str())?;
                if kind != "snapshot" {
                    return None;
                }
                let pid_in = meta.get("parent_id").and_then(|v| v.as_str())?;
                if pid_in != pid {
                    return None;
                }
                let created_at = meta
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(chrono::Utc::now);
                let word_count = meta.get("word_count").and_then(|v| v.as_u64()).unwrap_or(0);
                let preview = meta
                    .get("preview")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Some(Snapshot {
                    id,
                    parent_id,
                    created_at,
                    word_count,
                    preview,
                })
            })
            .collect();
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(out)
    }

    pub fn snapshot_content(&self, snapshot_id: Uuid) -> Result<Option<Vec<u8>>> {
        self.inner
            .get_content(snapshot_id)
            .map_err(|e| Error::Store(format!("snapshot_content: {e}")))
    }

    /// Delete the on-disk subtree at `fs_rel` (relative to project root) and
    /// remove every UUID in `ids` from bdslib. Errors from individual bdslib
    /// deletes are logged but don't abort the loop — orphans get caught by
    /// `inkhaven reindex`.
    pub fn delete_subtree(&self, fs_rel: &Path, ids: &[Uuid]) -> Result<()> {
        let abs = self.layout.root.join(fs_rel);
        if abs.is_dir() {
            std::fs::remove_dir_all(&abs)?;
        } else if abs.is_file() {
            std::fs::remove_file(&abs)?;
        }
        for id in ids {
            if let Err(e) = self.inner.delete_document(*id) {
                tracing::warn!(uuid = %id, "delete_document failed: {e}");
            }
        }
        self.sync()?;
        Ok(())
    }

    /// Update a paragraph's stored content + metadata and re-embed both
    /// vectors from the attached embedding engine. `node` is mutated to
    /// reflect the new word_count and modified_at.
    pub fn update_paragraph_content(&self, node: &mut Node, content: &[u8]) -> Result<()> {
        let id = node.id;
        node.word_count = std::str::from_utf8(content)
            .map(|s| s.split_whitespace().count() as u64)
            .unwrap_or(0);
        node.modified_at = chrono::Utc::now();

        self.inner
            .update_content(id, content)
            .map_err(|e| Error::Store(format!("update_content: {e}")))?;
        self.inner
            .update_metadata(id, node.to_json())
            .map_err(|e| Error::Store(format!("update_metadata: {e}")))?;
        self.inner
            .reembed_document(id)
            .map_err(|e| Error::Store(format!("reembed_document: {e}")))?;
        Ok(())
    }
}

fn first_prose_line(content: &[u8]) -> String {
    let s = std::str::from_utf8(content).unwrap_or("");
    for line in s.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('=') || t.starts_with("//") {
            continue;
        }
        let chars: Vec<char> = t.chars().collect();
        if chars.len() > 80 {
            let mut o: String = chars.iter().take(79).collect();
            o.push('…');
            return o;
        }
        return t.to_string();
    }
    String::new()
}

fn build_embedding_engine(model_name: &str) -> Result<EmbeddingEngine> {
    let model = match model_name {
        "MultilingualE5Small" => Model::MultilingualE5Small,
        "MultilingualE5Base" => Model::MultilingualE5Base,
        "MultilingualE5Large" => Model::MultilingualE5Large,
        "BGEM3" => Model::BGEM3,
        "BGESmallENV15" => Model::BGESmallENV15,
        "BGEBaseENV15" => Model::BGEBaseENV15,
        "BGELargeENV15" => Model::BGELargeENV15,
        other => {
            return Err(Error::Config(format!(
                "unknown embedding model `{other}`; see fastembed::EmbeddingModel for options"
            )));
        }
    };

    EmbeddingEngine::new(model, embedding_cache_dir())
        .map_err(|e| Error::Store(e.to_string()))
}

/// Per-user cache directory for fastembed model files. Returning None falls
/// back to fastembed's default (typically `.fastembed_cache/` in the current
/// working directory, which we'd rather not pollute).
fn embedding_cache_dir() -> Option<std::path::PathBuf> {
    let dirs = directories::ProjectDirs::from("dev", "inkhaven", "inkhaven")?;
    let path = dirs.cache_dir().join("embeddings");
    let _ = std::fs::create_dir_all(&path);
    Some(path)
}
