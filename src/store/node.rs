use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use uuid::Uuid;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Book,
    Chapter,
    Subchapter,
    Paragraph,
    /// Standalone graphic in the book tree — first-class hierarchy
    /// node alongside Paragraph. Image nodes have:
    /// * `file: Some("<NN-slug>.<ext>")` pointing at the bytes on disk
    ///   under `books/<...>/`.
    /// * `image_ext` carrying the file extension (`png`, `jpg`, …) so
    ///   `fs_name()` can reconstruct the filename.
    /// * Optional `image_caption` / `image_alt` for the wrap_image
    ///   functions emitted during Book assembly.
    Image,
    /// Bund script as a first-class hierarchy node. Lives anywhere
    /// in the tree (default home: the `Scripts` system book), is
    /// stored on disk as a `.bund` file under `books/<...>/`, and
    /// gets `bund.eval`'d into the Adam VM at project open. That's
    /// where user-authored hook lambdas (`hook.on_save`, etc.) come
    /// from in P5+ — the HJSON `scripting.bootstrap` field remains
    /// for tiny inline rules.
    Script,
}

impl NodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeKind::Book => "book",
            NodeKind::Chapter => "chapter",
            NodeKind::Subchapter => "subchapter",
            NodeKind::Paragraph => "paragraph",
            NodeKind::Image => "image",
            NodeKind::Script => "script",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "book" => Some(NodeKind::Book),
            "chapter" => Some(NodeKind::Chapter),
            "subchapter" => Some(NodeKind::Subchapter),
            "paragraph" => Some(NodeKind::Paragraph),
            "image" => Some(NodeKind::Image),
            "script" => Some(NodeKind::Script),
            _ => None,
        }
    }

    /// Image / Paragraph / Script are leaves; chapters /
    /// subchapters / books can have children. Used in tree-
    /// rendering and the placement validator.
    pub fn is_leaf(&self) -> bool {
        matches!(
            self,
            NodeKind::Paragraph | NodeKind::Image | NodeKind::Script
        )
    }
}

/// Hierarchy node metadata as stored in bdslib's JsonStorage layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: Uuid,
    pub kind: NodeKind,
    pub title: String,
    pub slug: String,
    /// Slug path from the books root down to (but not including) this node.
    pub path: Vec<String>,
    pub parent_id: Option<Uuid>,
    pub order: u32,
    /// Path of the `.typ` file relative to project root. Set for paragraphs;
    /// branches leave it `None`.
    pub file: Option<String>,
    #[serde(default)]
    pub word_count: u64,
    pub modified_at: chrono::DateTime<chrono::Utc>,
    /// True for nodes the user is not allowed to delete or rename. Set by
    /// `Store::ensure_system_books` and persisted via the metadata JSON. Old
    /// projects (which don't have the field) round-trip as `false`.
    #[serde(default)]
    pub protected: bool,
    /// Stable identifier for system-created books (e.g. `"places"`,
    /// `"help"`). Lets callers find Places/Characters/Help by tag rather than
    /// by display title, so the lexicon highlighting and read-only behaviour
    /// survive a hypothetical future rename.
    #[serde(default)]
    pub system_tag: Option<String>,

    /// For Image nodes: the file extension (`png`, `jpg`, `webp`, …)
    /// without the leading dot. Used by `fs_name()` to reconstruct the
    /// on-disk filename, and by `wrap_image_*` calls in the assembled
    /// typst tree to pick the right relative path. None on every
    /// other kind.
    #[serde(default)]
    pub image_ext: Option<String>,

    /// For Image nodes: optional caption rendered by the matching
    /// `wrap_image_*` function in the assembled output. None → no
    /// caption is emitted.
    #[serde(default)]
    pub image_caption: Option<String>,

    /// For Image nodes: alt-text for accessibility; flows into typst
    /// `image(..., alt: ...)` when set.
    #[serde(default)]
    pub image_alt: Option<String>,

    /// For Paragraph nodes: the editor / highlighter language. None
    /// or `"typst"` (the default) treats the file as a Typst document
    /// and picks the tree-sitter-typst highlighter. `"hjson"` switches
    /// to inkhaven's hand-rolled HJSON highlighter and gives the file
    /// the `.hjson` extension on disk. Future values (`"json"`,
    /// `"yaml"`) can land without breaking persisted projects because
    /// the field is serde-optional with the typst default.
    #[serde(default)]
    pub content_type: Option<String>,

    /// Document-status workflow tag — Ctrl+B R in the editor cycles
    /// through Napkin → First → Second → Third → Final → Ready (and
    /// back to None) so the writer can mark progress without leaving
    /// the buffer. None / empty = no badge. Stored as a string so
    /// future projects can extend the workflow without a migration.
    #[serde(default)]
    pub status: Option<String>,

    /// Per-paragraph word-count goal (1.2.4+). When set, the tree
    /// pane shows a 4-char Unicode gauge + percent beside the
    /// paragraph; on save, the auto-promote machinery checks
    /// whether `word_count >= target_words` and bumps `status` one
    /// ladder step if `goals.auto_promote_on_target` is true. None
    /// = no goal. Stored as `i32` so the field round-trips cleanly
    /// through JSON / DuckDB without unsigned-conversion surprises;
    /// negative values are clamped to None at load time.
    #[serde(default)]
    pub target_words: Option<i32>,

    /// Bookkeeping for "promote once per `(paragraph, status)`"
    /// (1.2.4+). Holds the status the paragraph held immediately
    /// after the most recent auto-promotion (i.e. the new status,
    /// not the pre-promotion one). Subsequent saves that stay at
    /// or above `target_words` won't re-promote while this matches
    /// the current status. A manual `Ctrl+B R` cycle clears /
    /// changes the status field; the next save will re-fire
    /// auto-promote if the goal is still met.
    #[serde(default)]
    pub target_hit_at_status: Option<String>,

    /// Outgoing wiki-links to other paragraphs (1.2.4+). Stored as
    /// metadata only — the link does NOT appear in the typst
    /// source, so it travels safely through export pipelines. The
    /// status-bar widget surfaces the count; the AI inference
    /// path inlines each linked paragraph's body into the prompt
    /// when scope=Paragraph; the Ctrl+V L modal lets the user
    /// inspect / delete them. Circular references are rejected at
    /// `add_link` time.
    #[serde(default)]
    pub linked_paragraphs: Vec<Uuid>,

    /// User-toggled bookmark flag (1.2.4+). `Ctrl+V B` flips it;
    /// `Ctrl+V M` opens a picker over every bookmarked
    /// paragraph in the project.
    #[serde(default)]
    pub bookmark: bool,

    /// User-defined tags (1.2.5+). Free-form strings; case is
    /// preserved on save but the picker dedups case-sensitively.
    /// `Ctrl+B ]` (editor) or `g` (tree) add/remove tags;
    /// `Ctrl+B }` searches by tag. Project-wide tag deletion
    /// removes the tag from every node that carries it.
    #[serde(default)]
    pub tags: Vec<String>,

    /// 1.2.6+ — per-paragraph AI memory: bounded rolling
    /// buffer of recent `(user, assistant)` turns scoped to
    /// this paragraph. Enabled by HJSON
    /// `ai.per_paragraph_memory = true`; capped at
    /// `ai.per_paragraph_memory_max_turns` (oldest evicted
    /// first). Prepended to the chat-history payload when the
    /// next AI prompt fires with scope=Paragraph and this
    /// paragraph is open, giving the model continuity across
    /// sessions without polluting the project-wide chat
    /// history. Stored as alternating `user` / `assistant`
    /// entries in the JSON metadata blob (see `AiMemoryTurn`).
    #[serde(default)]
    pub ai_memory: Vec<AiMemoryTurn>,
}

/// 1.2.6+ — one turn in `Node.ai_memory`. `role` is either
/// `"user"` or `"assistant"`; `text` is the prompt or response
/// verbatim.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct AiMemoryTurn {
    pub role: String,
    pub text: String,
}

impl Node {
    pub fn to_json(&self) -> JsonValue {
        json!({
            "kind":          self.kind.as_str(),
            "title":         self.title,
            "slug":          self.slug,
            "path":          self.path,
            "parent_id":     self.parent_id.map(|u| u.to_string()),
            "order":         self.order,
            "file":          self.file,
            "word_count":    self.word_count,
            "modified_at":   self.modified_at.to_rfc3339(),
            "protected":     self.protected,
            "system_tag":    self.system_tag,
            "image_ext":     self.image_ext,
            "image_caption": self.image_caption,
            "image_alt":     self.image_alt,
            "content_type":  self.content_type,
            "status":        self.status,
            "target_words":         self.target_words,
            "target_hit_at_status": self.target_hit_at_status,
            "linked_paragraphs":    self.linked_paragraphs
                .iter()
                .map(|u| u.to_string())
                .collect::<Vec<_>>(),
            "bookmark":             self.bookmark,
            "tags":                 self.tags,
            "ai_memory":            self.ai_memory,
        })
    }

    pub fn from_json(id: Uuid, value: &JsonValue) -> Result<Self> {
        let obj = value
            .as_object()
            .ok_or_else(|| Error::Store(format!("node {id}: metadata is not an object")))?;

        let kind_str = obj
            .get("kind")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Store(format!("node {id}: missing `kind`")))?;
        let kind = NodeKind::from_str(kind_str)
            .ok_or_else(|| Error::Store(format!("node {id}: unknown kind `{kind_str}`")))?;

        let title = obj
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Store(format!("node {id}: missing `title`")))?
            .to_string();

        let slug = obj
            .get("slug")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::Store(format!("node {id}: missing `slug`")))?
            .to_string();

        let path = obj
            .get("path")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();

        let parent_id = match obj.get("parent_id") {
            None | Some(JsonValue::Null) => None,
            Some(JsonValue::String(s)) => Some(
                Uuid::parse_str(s)
                    .map_err(|e| Error::Store(format!("node {id}: bad parent_id: {e}")))?,
            ),
            Some(_) => return Err(Error::Store(format!("node {id}: parent_id not a string"))),
        };

        let order = obj
            .get("order")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let file = obj
            .get("file")
            .and_then(|v| v.as_str())
            .map(str::to_owned);

        let word_count = obj
            .get("word_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let modified_at = obj
            .get("modified_at")
            .and_then(|v| v.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(chrono::Utc::now);

        let protected = obj
            .get("protected")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let system_tag = obj
            .get("system_tag")
            .and_then(|v| v.as_str())
            .map(str::to_owned);

        let image_ext = obj
            .get("image_ext")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let image_caption = obj
            .get("image_caption")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let image_alt = obj
            .get("image_alt")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let content_type = obj
            .get("content_type")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let status = obj
            .get("status")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        // Per-paragraph goal — accept any integer JSON shape.
        // Negative values are nonsense; clamp to None.
        let target_words = obj
            .get("target_words")
            .and_then(|v| v.as_i64())
            .filter(|n| *n > 0)
            .map(|n| n.clamp(0, i32::MAX as i64) as i32);
        let target_hit_at_status = obj
            .get("target_hit_at_status")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        // Outgoing wiki-links — array of UUID strings. Silently
        // drops malformed entries (a renamed/deleted target whose
        // UUID went away survives a round-trip as missing here).
        let linked_paragraphs = obj
            .get("linked_paragraphs")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .filter_map(|s| Uuid::parse_str(s).ok())
                    .collect::<Vec<Uuid>>()
            })
            .unwrap_or_default();

        Ok(Self {
            id,
            kind,
            title,
            slug,
            path,
            parent_id,
            order,
            file,
            word_count,
            modified_at,
            protected,
            system_tag,
            image_ext,
            image_caption,
            image_alt,
            content_type,
            status,
            target_words,
            target_hit_at_status,
            linked_paragraphs,
            bookmark: obj
                .get("bookmark")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            tags: obj
                .get("tags")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .filter(|s| !s.trim().is_empty())
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default(),
            ai_memory: obj
                .get("ai_memory")
                .and_then(|v| serde_json::from_value::<Vec<AiMemoryTurn>>(v.clone()).ok())
                .unwrap_or_default(),
        })
    }

    /// Filesystem segment name for this node. Books use bare slugs; everything
    /// else gets a zero-padded numeric prefix so directory listings sort
    /// correctly (`01-preface.typ`, `02-chapter-one/`, …).
    pub fn fs_name(&self) -> String {
        match self.kind {
            NodeKind::Book => self.slug.clone(),
            NodeKind::Paragraph => {
                // content_type drives the extension. Default / None /
                // `"typst"` → `.typ`; `"hjson"` → `.hjson`. Future
                // values gain their own arms.
                let ext = match self.content_type.as_deref() {
                    Some("hjson") => "hjson",
                    _ => "typ",
                };
                format!("{:02}-{}.{}", self.order, self.slug, ext)
            }
            NodeKind::Image => {
                // Default to .png when an Image was somehow constructed
                // without an extension (test data, older project that
                // pre-dates the feature). Image bytes on disk would be
                // wrong, but the filename still sorts correctly.
                let ext = self.image_ext.as_deref().unwrap_or("png");
                format!("{:02}-{}.{}", self.order, self.slug, ext)
            }
            NodeKind::Script => format!("{:02}-{}.bund", self.order, self.slug),
            _ => format!("{:02}-{}", self.order, self.slug),
        }
    }
}
