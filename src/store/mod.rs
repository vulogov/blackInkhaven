pub mod hierarchy;
pub mod node;

use std::path::Path;
use std::sync::Arc;

use crate::storage::{DocumentStorage, EmbeddingEngine, Model};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::{BOOKS_DIR, ProjectLayout};
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind as NK};

pub use node::NodeKind;

/// Canonical ordering of the project-managed books. The tag is the
/// `system_tag` we write into metadata; the second element is the
/// human-facing default title used when the book is freshly created.
/// Display order matches array order (Notes first, Help last).
///
/// The `Scripts` system book is the default home for Bund script
/// nodes (`NodeKind::Script`) that aren't logically tied to a
/// particular user Book — global hooks, lint rules, AI templates.
/// Scripts can also live inside any user Book if they belong to
/// that book's workflow; nothing forces them under `Scripts`.
pub const SYSTEM_BOOKS: &[(&str, &str)] = &[
    ("notes", "Notes"),
    ("research", "Research"),
    ("prompts", "Prompts"),
    ("places", "Places"),
    ("characters", "Characters"),
    ("artefacts", "Artefacts"),
    ("threads", "Threads"),
    ("language", "Language"),
    ("typst", "Typst"),
    ("scripts", "Scripts"),
    ("help", "Help"),
];

pub const SYSTEM_TAG_NOTES: &str = "notes";
pub const SYSTEM_TAG_PROMPTS: &str = "prompts";
pub const SYSTEM_TAG_PLACES: &str = "places";
pub const SYSTEM_TAG_CHARACTERS: &str = "characters";
pub const SYSTEM_TAG_ARTEFACTS: &str = "artefacts";
/// 1.2.14+ — top-level container for narrative
/// plot threads.  Each thread is an HJSON-fronted
/// paragraph capturing one named arc (the
/// inheritance subplot, the redemption arc, the
/// secret-society reveal) with status / weight /
/// arc-shape / character-and-place links / tension
/// level.  Drives the thread weave view (`Ctrl+V
/// Shift+H` → picker → `w` weave), the AI thread
/// audit (`Ctrl+V Shift+A`), and the rebindable
/// CLI surface (`inkhaven thread add` / `list`).
/// See `Documentation/PROPOSALS/1.2.14_PLAN.md`.
pub const SYSTEM_TAG_THREADS: &str = "threads";
/// 1.2.13+ — top-level container for invented-
/// language books (Quenya / Drow / Klingon / …).
/// Per-language children are `NodeKind::Book` nodes
/// scaffolded with `Meta / Dictionary / Grammar /
/// Phonology / Sample texts` chapters by the
/// `inkhaven language init <name>` CLI.  Dictionary-
/// entry paragraphs live under alphabet
/// subchapters whose names come from the language's
/// `Meta/overview.alphabet` HJSON field — author-
/// defined so non-Latin orthographies (Hebrew,
/// Arabic, Asian scripts) can use logical
/// groupings instead of per-letter sections.
/// See `Documentation/PROPOSALS/LANGUAGE_BOOK.md`.
pub const SYSTEM_TAG_LANGUAGES: &str = "language";
pub const SYSTEM_TAG_TYPST: &str = "typst";
pub const SYSTEM_TAG_SCRIPTS: &str = "scripts";
pub const SYSTEM_TAG_HELP: &str = "help";
/// 1.2.6+ — system tag stamped onto the auto-created
/// Timeline chapter inside each user book that has events.
/// Lookups by tag find the chapter regardless of user
/// renames (rename keeps the tag).
pub const SYSTEM_TAG_BOOK_TIMELINE: &str = "book_timeline";

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
    /// 1.2.6+ — user-supplied annotation captured at snapshot
    /// time. Empty when the user pressed Enter on the prompt
    /// without typing anything (or skipped the prompt entirely
    /// via the legacy `create_snapshot` path). Surfaced by the
    /// F6 snapshot picker so the user sees what each version
    /// was *about*.
    pub annotation: String,
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
        // Arm the scripting layer for every path that opens a
        // project — TUI, `inkhaven bund`, `inkhaven add`,
        // `inkhaven reindex`, etc. Idempotent in practice
        // (single-project-per-process).
        crate::scripting::configure(cfg.scripting.clone(), store.clone(), cfg.clone());
        Ok(store)
    }

    /// Create the per-project artefacts root if it doesn't exist yet.
    /// Each user book gets its own subdirectory under here for PDFs /
    /// build intermediates / etc. With the empty-string default the
    /// directory lives in the OS per-user cache (see
    /// `resolve_artefacts_dir`).
    fn ensure_artefacts_directory(&self, cfg: &Config) -> Result<()> {
        let abs = self.resolve_artefacts_dir(cfg);
        std::fs::create_dir_all(&abs).map_err(Error::Io)?;
        Ok(())
    }

    /// Resolve `cfg.artefacts_directory` to an absolute path. Precedence:
    ///   1. **Empty string** — use the OS-appropriate per-user cache
    ///      directory: `<cache_dir>/inkhaven/artefacts/<project-basename>`.
    ///      This is the default behaviour because build artefacts are
    ///      ephemeral and don't belong inside the project tree.
    ///   2. **Absolute path** — used verbatim.
    ///   3. **Relative path** — joined to the project root (legacy
    ///      behaviour; useful if the user explicitly wants artefacts
    ///      tracked alongside the manuscript).
    pub fn resolve_artefacts_dir(&self, cfg: &Config) -> std::path::PathBuf {
        let raw = cfg.artefacts_directory.trim();
        if raw.is_empty() {
            return default_user_artefacts_dir(&self.layout.root);
        }
        let p = std::path::PathBuf::from(raw);
        if p.is_absolute() {
            p
        } else {
            self.layout.root.join(p)
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
    /// 1.2.6+ — find (or lazily create) the `Timeline`
    /// chapter inside `book_id`. Stamped with
    /// `system_tag: "book_timeline"` so subsequent lookups
    /// survive user renames. Returns the chapter's node id.
    /// Idempotent: a second call is a hierarchy lookup with
    /// no writes.
    pub fn ensure_timeline_chapter(
        &self,
        cfg: &Config,
        book_id: Uuid,
    ) -> Result<Uuid> {
        let hierarchy = crate::store::hierarchy::Hierarchy::load(self)?;
        let book = hierarchy
            .get(book_id)
            .cloned()
            .ok_or_else(|| Error::Store(format!(
                "ensure_timeline_chapter: book {book_id} missing"
            )))?;
        if book.kind != NK::Book {
            return Err(Error::Store(format!(
                "ensure_timeline_chapter: `{}` is not a Book", book.title
            )));
        }
        if let Some(existing) = hierarchy.iter().find(|n| {
            n.parent_id == Some(book_id)
                && n.system_tag.as_deref() == Some(SYSTEM_TAG_BOOK_TIMELINE)
        }) {
            return Ok(existing.id);
        }
        let mut created = self.create_node(
            cfg,
            &hierarchy,
            NK::Chapter,
            "Timeline",
            Some(&book),
            None,
            InsertPosition::End,
        )?;
        created.system_tag = Some(SYSTEM_TAG_BOOK_TIMELINE.to_owned());
        created.modified_at = chrono::Utc::now();
        self.inner
            .update_metadata(created.id, created.to_json())
            .map_err(|e| Error::Store(format!(
                "ensure_timeline_chapter: stamp system_tag: {e}"
            )))?;
        self.sync()?;
        Ok(created.id)
    }

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

        // (a) Artefacts subdirectory under the resolved root
        // (per-user cache by default — see `resolve_artefacts_dir`).
        let sub = self.resolve_artefacts_dir(cfg).join(&book_node.slug);
        std::fs::create_dir_all(&sub).map_err(Error::Io)?;

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

        // Each paragraph needs a deterministic title and a starter
        // body. globals.typ ships the four wrap_* functions used by the
        // Book assembly procedure (Ctrl+B A) — pulled from
        // `cfg.typst_templates` so the user can pre-customise them in
        // HJSON.
        let globals_body = cfg.typst_templates.globals_typ_body();
        let seeds: [(&str, String); 3] = [
            (
                "index.typ",
                "= index.typ\n\n#import \"globals.typ\": *\n#import \"settings.typ\": *\n"
                    .into(),
            ),
            (
                "settings.typ",
                "= settings.typ\n\n// Document-wide #set / #show rules go here.\n".into(),
            ),
            ("globals.typ", globals_body),
        ];
        // Reload hierarchy after each create so subsequent lookups see
        // freshly-added siblings.
        for (title, body) in &seeds {
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

                    // Make room for the new system book at its
                    // canonical slot: bump every existing root book
                    // (system or user) with `order >= target_order` up
                    // by 1. Without this, inserting `Artefacts` at
                    // slot 5 into a project that already has `Typst`
                    // at order 5 would either collide on order (sort
                    // tie-broken by slug — usually wrong) or land out
                    // of the intended sequence. A single open-time
                    // pass is enough; subsequent opens are no-ops
                    // since the book then exists.
                    for n in h.children_of(None) {
                        if n.order >= target_order {
                            let mut bumped = n.clone();
                            bumped.order += 1;
                            self.inner
                                .update_metadata(bumped.id, bumped.to_json())
                                .map_err(|e| {
                                    Error::Store(format!("update_metadata (bump): {e}"))
                                })?;
                        }
                    }
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
        // Heal-pass: if two system books ended up sharing the same
        // `order` (older seeder versions used to set the new book's
        // canonical idx without bumping the existing books), the
        // lexicographic tie-breaker chooses the wrong visual order.
        // Re-stamp the colliding pair to their canonical positions
        // and bump anything else above them. This is conservative —
        // it only fires when there's a clear collision, so a user
        // who deliberately reordered system books isn't surprised.
        let healed = Hierarchy::load(self)?;
        let mut by_order: std::collections::HashMap<u32, Vec<Node>> =
            std::collections::HashMap::new();
        for n in healed.children_of(None) {
            if n.system_tag.is_some() {
                by_order.entry(n.order).or_default().push(n.clone());
            }
        }
        let any_collision = by_order.values().any(|v| v.len() > 1);
        if any_collision {
            for (idx, (tag, _title)) in SYSTEM_BOOKS.iter().enumerate() {
                let target = idx as u32;
                if let Some(node) = healed.iter().find(|n| {
                    n.kind == NK::Book && n.system_tag.as_deref() == Some(*tag)
                }) {
                    if node.order != target {
                        let mut updated = node.clone();
                        updated.order = target;
                        self.inner
                            .update_metadata(updated.id, updated.to_json())
                            .map_err(|e| {
                                Error::Store(format!("update_metadata (heal): {e}"))
                            })?;
                    }
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

    /// Drain DuckDB's WAL into the main `.db` files. Used by the
    /// background sync tick and the TUI shutdown path; per-save
    /// callers don't need this because every commit is already
    /// fsync-durable.
    pub fn checkpoint(&self) -> Result<()> {
        self.inner
            .checkpoint()
            .map_err(|e| Error::Store(e.to_string()))
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
        // Fire hook.on_create ( uuid kind -- ). Errors are logged
        // and swallowed inside `hooks::fire` — a misbehaving hook
        // never aborts the create.
        fire_hook(
            "hook.on_create",
            vec![
                bund_string(&id.to_string()),
                bund_string(node.kind.as_str()),
            ],
        );
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
            image_ext: None,
            image_caption: None,
            image_alt: None,
            content_type: None,
            status: None,
            target_words: None,
            target_hit_at_status: None,
            linked_paragraphs: Vec::new(),
            bookmark: false,
            tags: Vec::new(),
            ai_memory: Vec::new(),
            event: None,
        };

        let rel_path = match parent {
            None => std::path::PathBuf::from(BOOKS_DIR).join(node.fs_name()),
            Some(p) => hierarchy.fs_path(p, &self.layout).join(node.fs_name()),
        };
        let abs_path = self.layout.root.join(&rel_path);

        // 1.2.8+ — paragraphs under the Help book default to
        // markdown content_type.  Doc content is mostly MD,
        // so this turns on the markdown syntax highlighter
        // for new Help paragraphs and gives them a `# Title`
        // template instead of typst's `= Title`.  Detection
        // is by walking the parent chain looking for a node
        // tagged with the system `help` tag (the well-known
        // root for the Help book — survives renames because
        // the tag is sticky).  Limited to the Help book ON
        // PURPOSE: every other book stays on the typst
        // default.
        let in_help_book = parent
            .map(|p| {
                let mut cur: Option<&Node> = Some(p);
                while let Some(n) = cur {
                    if n.system_tag.as_deref() == Some(SYSTEM_TAG_HELP) {
                        return true;
                    }
                    cur = n.parent_id.and_then(|id| hierarchy.get(id));
                }
                false
            })
            .unwrap_or(false);

        let content: Vec<u8> = match kind {
            NK::Paragraph => {
                if let Some(parent_dir) = abs_path.parent() {
                    std::fs::create_dir_all(parent_dir)?;
                }
                let template = if in_help_book {
                    node.content_type = Some("markdown".to_string());
                    format!("# {}\n\n", node.title)
                } else {
                    format!("= {}\n\n", node.title)
                };
                std::fs::write(&abs_path, &template)?;
                node.file = Some(rel_path.to_string_lossy().into_owned());
                node.word_count = template.split_whitespace().count() as u64;
                template.into_bytes()
            }
            NK::Script => {
                if let Some(parent_dir) = abs_path.parent() {
                    std::fs::create_dir_all(parent_dir)?;
                }
                let template = format!(
                    "// {}\n// Bund script — evaluated into the Adam VM at\n\
                     // project open. Register hooks via:\n\
                     //   \"hook.on_save\" {{ drop \"saved\" println }} register\n\n",
                    node.title
                );
                std::fs::write(&abs_path, &template)?;
                node.file = Some(rel_path.to_string_lossy().into_owned());
                node.word_count = template.split_whitespace().count() as u64;
                // Drive the Bund syntax highlighter via the same
                // content_type channel the HJSON path uses.
                node.content_type = Some("bund".to_string());
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

        // 1.2.4+: re-derive the slug for paragraph nodes so the
        // on-disk filename tracks the title. The slug is stable
        // for branches (renaming a chapter directory would also
        // need to rewrite every descendant's `file` field — out
        // of scope for this change; branch rename keeps the
        // original folder name).
        if matches!(node.kind, NodeKind::Paragraph) {
            let new_slug_base = slug::slugify(trimmed);
            if !new_slug_base.is_empty() && new_slug_base != node.slug {
                let mut new_slug = new_slug_base.clone();
                let mut n = 2;
                let siblings = hierarchy.children_of(node.parent_id);
                while siblings
                    .iter()
                    .any(|s| s.id != node.id && s.slug == new_slug)
                {
                    new_slug = format!("{new_slug_base}-{n}");
                    n += 1;
                }
                if new_slug != node.slug {
                    // Compute paths from the *current* slug (old)
                    // and the post-rename slug (new). Both live in
                    // the same parent directory so we only rename
                    // the basename.
                    if let Some(rel_old) = node.file.as_ref().cloned() {
                        let old_abs = self.layout.root.join(&rel_old);
                        // Rebuild the path with the new slug.
                        node.slug = new_slug.clone();
                        let new_name = node.fs_name();
                        let parent_rel = std::path::Path::new(&rel_old)
                            .parent()
                            .map(|p| p.to_path_buf())
                            .unwrap_or_default();
                        let new_rel = parent_rel.join(&new_name);
                        let new_abs = self.layout.root.join(&new_rel);
                        if old_abs != new_abs {
                            if let Err(e) = std::fs::rename(&old_abs, &new_abs) {
                                // Don't surface a hard error here:
                                // the metadata update can still
                                // happen, and `inkhaven reindex`
                                // will pick up the drift on next
                                // launch. Roll back the slug.
                                tracing::warn!(
                                    target: "inkhaven::rename",
                                    "rename {} → {} failed: {e}",
                                    old_abs.display(),
                                    new_abs.display(),
                                );
                                node.slug = rel_old
                                    .rsplit('/')
                                    .next()
                                    .and_then(|n| {
                                        // Reverse-engineer old slug
                                        // from "NN-slug.typ".
                                        n.trim_end_matches(".typ")
                                            .trim_end_matches(".hjson")
                                            .splitn(2, '-')
                                            .nth(1)
                                            .map(|s| s.to_string())
                                    })
                                    .unwrap_or(node.slug);
                            } else {
                                node.file =
                                    Some(new_rel.to_string_lossy().into_owned());
                            }
                        }
                    } else {
                        // No file on disk (shouldn't happen for a
                        // paragraph but stay safe). Just stamp the
                        // new slug.
                        node.slug = new_slug;
                    }
                }
            }
        }

        self.inner
            .update_metadata(node.id, node.to_json())
            .map_err(|e| Error::Store(format!("update_metadata: {e}")))?;
        self.inner
            .reembed_document(node.id)
            .map_err(|e| Error::Store(format!("reembed_document: {e}")))?;
        self.sync()?;
        // Fire hook.on_rename ( uuid new_title -- ).
        fire_hook(
            "hook.on_rename",
            vec![bund_string(&node.id.to_string()), bund_string(trimmed)],
        );
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
        self.create_snapshot_annotated(parent, content, "")
    }

    /// 1.2.6+ — `create_snapshot` plus a user-supplied
    /// `annotation` ("first complete draft", "before the
    /// lighthouse rewrite") stamped into the snapshot's
    /// metadata. Empty annotation = same shape as the legacy
    /// `create_snapshot` path.
    pub fn create_snapshot_annotated(
        &self,
        parent: &Node,
        content: &[u8],
        annotation: &str,
    ) -> Result<Uuid> {
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
            "annotation": annotation,
        });
        let id = self
            .inner
            .add_document_no_embed(meta, content)
            .map_err(|e| Error::Store(format!("create_snapshot: {e}")))?;
        self.sync()?;
        // Fire hook.on_snapshot ( parent_uuid snapshot_uuid -- ).
        fire_hook(
            "hook.on_snapshot",
            vec![
                bund_string(&parent.id.to_string()),
                bund_string(&id.to_string()),
            ],
        );
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
                let annotation = meta
                    .get("annotation")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Some(Snapshot {
                    id,
                    parent_id,
                    created_at,
                    word_count,
                    preview,
                    annotation,
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

    /// Fetch the raw bytes of an Image node from bdslib. The on-disk
    /// copy under `books/<...>` is the working copy; bdslib is the
    /// source of truth (so a hand-edit on disk isn't re-ingested by
    /// `Book assembly` — re-importing the file via F3 is the way).
    pub fn image_bytes(&self, image_id: Uuid) -> Result<Option<Vec<u8>>> {
        self.inner
            .get_content(image_id)
            .map_err(|e| Error::Store(format!("image_bytes: {e}")))
    }

    /// Create an Image node in bdslib + on disk. `title` becomes the
    /// node's display title; the slug is derived from it. `ext` is
    /// the canonical file extension (`png`, `jpg`, …) without a dot.
    /// `bytes` is the image content — written to disk verbatim and
    /// also stored in bdslib via `add_document_no_embed` so backup /
    /// restore round-trip it.
    pub fn create_image_node(
        &self,
        cfg: &Config,
        hierarchy: &Hierarchy,
        title: &str,
        ext: &str,
        bytes: &[u8],
        parent: Option<&Node>,
        position: InsertPosition,
    ) -> Result<Node> {
        // Build the node skeleton via the existing branch path —
        // create_node handles slug uniqueness, ordering, and
        // metadata persistence. We then override the kind-specific
        // fields and update.
        let mut node = self.create_node(
            cfg,
            hierarchy,
            NK::Image,
            title,
            parent,
            None,
            position,
        )?;
        node.image_ext = Some(ext.to_lowercase());
        // The placeholder body create_node wrote (a paragraph-style
        // `= Title` markdown stub) isn't useful for an image. Replace
        // both bdslib content and on-disk file with the bytes.
        let abs = self.layout.root.join(
            node.file.as_deref().unwrap_or(""),
        );
        std::fs::write(&abs, bytes).map_err(Error::Io)?;
        // Rename the on-disk file from `NN-slug.typ` (what create_node
        // wrote) to `NN-slug.<ext>` so the on-disk extension matches.
        let abs_typ = abs.clone();
        let abs_image =
            self.layout.root.join(
                std::path::PathBuf::from(node.file.clone().unwrap_or_default())
                    .with_extension(&node.image_ext.clone().unwrap_or_default()),
            );
        if abs_typ != abs_image && abs_typ.exists() {
            let _ = std::fs::rename(&abs_typ, &abs_image);
        }
        // Update node.file to reflect the new extension.
        if let Some(rel) = node.file.as_ref() {
            let rel_image =
                std::path::PathBuf::from(rel).with_extension(&node.image_ext.clone().unwrap_or_default());
            node.file = Some(rel_image.to_string_lossy().into_owned());
        }
        self.inner
            .update_content(node.id, bytes)
            .map_err(|e| Error::Store(format!("update_content (image): {e}")))?;
        self.inner
            .update_metadata(node.id, node.to_json())
            .map_err(|e| Error::Store(format!("update_metadata (image): {e}")))?;
        self.sync()?;
        Ok(node)
    }

    /// Delete a single snapshot by id. Snapshots have no on-disk file
    /// (they live entirely in bdslib as `add_document_no_embed`), so
    /// this is just a bdslib delete + a sync to flush the change.
    pub fn delete_snapshot(&self, snapshot_id: Uuid) -> Result<()> {
        self.inner
            .delete_document(snapshot_id)
            .map_err(|e| Error::Store(format!("delete_snapshot {snapshot_id}: {e}")))?;
        self.sync()?;
        Ok(())
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
        // 1.2.6+ — scrub paragraph-link references to the deleted
        // nodes. Without this, every other paragraph's
        // `linked_paragraphs: Vec<Uuid>` keeps the dead UUIDs
        // and the Ctrl+V L picker silently filters them at view
        // time. Walk the post-delete hierarchy, prune, persist
        // any node that changed. Errors are logged but don't
        // abort the delete itself.
        let deleted: std::collections::HashSet<Uuid> = ids.iter().copied().collect();
        let scrubbed = self.scrub_linked_paragraphs(&deleted);
        if scrubbed > 0 {
            tracing::info!(
                target: "inkhaven::delete",
                "delete_subtree: scrubbed paragraph links from {scrubbed} other paragraph(s)",
            );
        }
        // Fire hook.on_delete ( uuid -- ) once per deleted id, in
        // the same order the store walks them. Best-effort: hook
        // failures are logged inside `hooks::fire`, never abort.
        for id in ids {
            fire_hook("hook.on_delete", vec![bund_string(&id.to_string())]);
        }
        Ok(())
    }

    /// Walk every remaining node and remove any UUID in
    /// `deleted` from its `linked_paragraphs` field. Returns the
    /// number of nodes touched. Used by `delete_subtree` (1.2.6+)
    /// to keep paragraph-link metadata in sync with reality.
    fn scrub_linked_paragraphs(
        &self,
        deleted: &std::collections::HashSet<Uuid>,
    ) -> usize {
        // Re-load hierarchy after the delete so we walk what's
        // actually still on disk + in the store.
        let hierarchy = match crate::store::hierarchy::Hierarchy::load(self) {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!(
                    target: "inkhaven::delete",
                    "scrub: hierarchy reload failed: {e}",
                );
                return 0;
            }
        };
        let mut touched = 0usize;
        for (n, _) in hierarchy.flatten() {
            // 1.2.6+ — also scrub event-side links (characters
            // / places). Either source of dirt counts as
            // "needs rewrite".
            let para_hit = n
                .linked_paragraphs
                .iter()
                .any(|id| deleted.contains(id));
            let event_hit = n.event.as_ref().is_some_and(|e| {
                e.characters.iter().any(|id| deleted.contains(id))
                    || e.places.iter().any(|id| deleted.contains(id))
            });
            if !para_hit && !event_hit {
                continue;
            }
            let mut updated = n.clone();
            if para_hit {
                updated.linked_paragraphs.retain(|id| !deleted.contains(id));
            }
            if event_hit {
                if let Some(ev) = updated.event.as_mut() {
                    ev.characters.retain(|id| !deleted.contains(id));
                    ev.places.retain(|id| !deleted.contains(id));
                }
            }
            updated.modified_at = chrono::Utc::now();
            // Re-evaluate orphan state when this event lost
            // its last cross-link.
            reconcile_event_orphan_tag(&mut updated);
            if let Err(e) = self.inner.update_metadata(updated.id, updated.to_json()) {
                tracing::warn!(
                    target: "inkhaven::delete",
                    uuid = %updated.id,
                    "scrub: update_metadata failed: {e}",
                );
                continue;
            }
            touched += 1;
        }
        touched
    }

    /// Convert a text-leaf node between its three flavours:
    ///
    ///   Paragraph(typst)  ←→  Paragraph(hjson)  ←→  Script(bund)
    ///
    /// The conversion renames the file on disk to match the new
    /// extension (`.typ` / `.hjson` / `.bund`), stamps the new
    /// `kind` and `content_type` into the metadata, and persists
    /// via `update_metadata`. Body contents are NOT translated —
    /// switching `.typ` → `.bund` just changes the kind label;
    /// the writer is responsible for the body making sense in
    /// the new flavour.
    ///
    /// `new_kind` must be either `Paragraph` or `Script`;
    /// `new_content_type` is `None` for plain typst, `Some("hjson")`
    /// for HJSON, `Some("bund")` for Bund scripts. Other
    /// combinations are rejected.
    pub fn convert_leaf(
        &self,
        hierarchy: &Hierarchy,
        node_id: Uuid,
        new_kind: NodeKind,
        new_content_type: Option<&str>,
    ) -> Result<Node> {
        let node = hierarchy
            .get(node_id)
            .cloned()
            .ok_or_else(|| Error::Store(format!("convert_leaf: missing {node_id}")))?;
        if !matches!(node.kind, NodeKind::Paragraph | NodeKind::Script) {
            return Err(Error::Store(format!(
                "convert_leaf: can't convert a {} (only paragraph / script)",
                node.kind.as_str()
            )));
        }
        if !matches!(new_kind, NodeKind::Paragraph | NodeKind::Script) {
            return Err(Error::Store(format!(
                "convert_leaf: new kind {} is not a text leaf",
                new_kind.as_str()
            )));
        }
        // Validate content_type vs new kind.
        match (new_kind, new_content_type) {
            (NodeKind::Paragraph, None | Some("typst") | Some("hjson")) => {}
            (NodeKind::Script, Some("bund")) => {}
            (k, ct) => {
                return Err(Error::Store(format!(
                    "convert_leaf: content_type {ct:?} not valid for {}",
                    k.as_str()
                )));
            }
        }

        let Some(old_rel) = node.file.clone() else {
            return Err(Error::Store(
                "convert_leaf: node has no file on disk".into(),
            ));
        };
        let old_abs = self.layout.root.join(&old_rel);

        let mut new_node = node.clone();
        new_node.kind = new_kind;
        new_node.content_type = new_content_type.map(str::to_string);
        // "typst" as an explicit content_type is redundant — None
        // is the canonical default. Keep persistence terse.
        if new_node.content_type.as_deref() == Some("typst") {
            new_node.content_type = None;
        }
        new_node.modified_at = chrono::Utc::now();

        let new_name = new_node.fs_name();
        let parent_dir = old_abs
            .parent()
            .ok_or_else(|| Error::Store("convert_leaf: no parent directory".into()))?;
        let new_abs = parent_dir.join(&new_name);

        if new_abs != old_abs {
            if new_abs.exists() {
                return Err(Error::Store(format!(
                    "convert_leaf: target `{}` already exists",
                    new_abs.display()
                )));
            }
            std::fs::rename(&old_abs, &new_abs)?;
            let new_rel = new_abs
                .strip_prefix(&self.layout.root)
                .unwrap_or(&new_abs)
                .to_string_lossy()
                .into_owned();
            new_node.file = Some(new_rel);
        }

        self.inner
            .update_metadata(new_node.id, new_node.to_json())
            .map_err(|e| Error::Store(format!("update_metadata: {e}")))?;
        self.sync()?;
        Ok(new_node)
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
        // Fire hook.on_save ( uuid -- ).
        fire_hook("hook.on_save", vec![bund_string(&id.to_string())]);
        Ok(())
    }
}

// ── Bund hook helpers ─────────────────────────────────────────────────

/// Forward a hook fire request to the scripting layer. Behind a
/// helper so the call sites in this module stay one-liners.
fn fire_hook(name: &str, args: Vec<rust_dynamic::value::Value>) {
    crate::scripting::hooks::fire(name, args);
}

/// Build a Bund STRING value. Used to push `Uuid`, `&str`, etc.
/// onto the workbench in hook fire calls.
fn bund_string(s: &str) -> rust_dynamic::value::Value {
    rust_dynamic::value::Value::from_string(s)
}

/// 1.2.6+ — keep the `orphan` tag in sync with the event's
/// actual link state. Called by every mutation that touches
/// an event (add, link change, scrub-on-delete). Idempotent;
/// no-op when the node isn't an event.
///
/// Fires `hook.on_event_orphaned ( uuid -- )` on the
/// transition `linked → orphan`. The opposite transition
/// (orphan → linked) doesn't fire — link-add paths fire
/// `hook.on_event_added` or a future
/// `hook.on_event_linked`.
pub(crate) fn reconcile_event_orphan_tag(node: &mut Node) {
    let Some(ev) = node.event.as_ref() else {
        return;
    };
    let is_orphan = ev.is_orphan(&node.linked_paragraphs);
    let pos = node.tags.iter().position(|t| t.eq_ignore_ascii_case("orphan"));
    match (is_orphan, pos) {
        (true, None) => {
            node.tags.push("orphan".to_owned());
            fire_hook(
                "hook.on_event_orphaned",
                vec![bund_string(&node.id.to_string())],
            );
        }
        (false, Some(i)) => {
            node.tags.remove(i);
        }
        _ => {}
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

/// Default backup directory for a given project. Lives **next to** the
/// project (sibling directory in the same parent), in a shared
/// `inkhaven-backups/` folder with a `<project-basename>` subfolder so
/// multiple projects in the same parent don't collide.
///
/// Why sibling-of-project rather than an OS cache directory: backups
/// are user-facing artefacts that need to be obvious when listing files.
/// Hiding them in `~/Library/Caches/...` makes them hard to find and
/// hard to copy to external storage.
///
/// Layout for a project at `~/Books/my-novel/`:
/// ```text
/// ~/Books/
/// ├── inkhaven-backups/
/// │   └── my-novel/         ← snapshots land here
/// └── my-novel/
/// ```
pub fn default_user_backup_dir(project_root: &std::path::Path) -> std::path::PathBuf {
    let project_id = project_basename(project_root);
    project_root
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("inkhaven-backups")
        .join(project_id)
}

/// Default artefacts directory — same sibling-of-project pattern as
/// `default_user_backup_dir`. PDFs, build intermediates, and other
/// per-book outputs land under `<parent>/inkhaven-artefacts/<project-basename>/<book-slug>/`.
pub fn default_user_artefacts_dir(project_root: &std::path::Path) -> std::path::PathBuf {
    let project_id = project_basename(project_root);
    project_root
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("inkhaven-artefacts")
        .join(project_id)
}

/// Project identifier for the default-path resolvers. The bare filename
/// of the project root works in practice; collisions between same-named
/// projects in different parents are surfaced quickly by the user and
/// can be resolved by setting an explicit `backup.out_dir` /
/// `artefacts_directory` in their HJSON.
fn project_basename(project_root: &std::path::Path) -> String {
    project_root
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "default".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::node::EventData;

    fn make_event_node() -> Node {
        Node {
            id: Uuid::nil(),
            kind: NK::Paragraph,
            title: "Storm".into(),
            slug: "storm".into(),
            path: Vec::new(),
            parent_id: None,
            order: 0,
            file: None,
            word_count: 0,
            modified_at: chrono::Utc::now(),
            protected: false,
            system_tag: None,
            image_ext: None,
            image_caption: None,
            image_alt: None,
            content_type: None,
            status: None,
            target_words: None,
            target_hit_at_status: None,
            linked_paragraphs: Vec::new(),
            bookmark: false,
            tags: Vec::new(),
            ai_memory: Vec::new(),
            event: Some(EventData {
                start_ticks: 0,
                end_ticks: None,
                precision: crate::timeline::Precision::Day,
                characters: Vec::new(),
                places: Vec::new(),
                track: None,
            }),
        }
    }

    #[test]
    fn orphan_tag_added_when_all_links_empty() {
        let mut n = make_event_node();
        reconcile_event_orphan_tag(&mut n);
        assert!(n.tags.iter().any(|t| t == "orphan"));
    }

    #[test]
    fn orphan_tag_removed_when_link_added() {
        let mut n = make_event_node();
        n.tags.push("orphan".into());
        n.linked_paragraphs.push(Uuid::new_v4());
        reconcile_event_orphan_tag(&mut n);
        assert!(!n.tags.iter().any(|t| t == "orphan"));
    }

    #[test]
    fn orphan_tag_noop_on_non_event_node() {
        let mut n = make_event_node();
        n.event = None;
        // Even when tags include "orphan", non-event nodes
        // are left alone.
        n.tags.push("orphan".into());
        reconcile_event_orphan_tag(&mut n);
        assert!(n.tags.iter().any(|t| t == "orphan"));
    }

    #[test]
    fn orphan_tag_kept_when_event_links_present_but_paragraphs_empty() {
        // Either link kind keeps the event un-orphaned.
        let mut n = make_event_node();
        if let Some(ev) = n.event.as_mut() {
            ev.characters.push(Uuid::new_v4());
        }
        reconcile_event_orphan_tag(&mut n);
        assert!(!n.tags.iter().any(|t| t == "orphan"));
    }
}
