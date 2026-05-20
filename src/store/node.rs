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
}

impl NodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeKind::Book => "book",
            NodeKind::Chapter => "chapter",
            NodeKind::Subchapter => "subchapter",
            NodeKind::Paragraph => "paragraph",
            NodeKind::Image => "image",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "book" => Some(NodeKind::Book),
            "chapter" => Some(NodeKind::Chapter),
            "subchapter" => Some(NodeKind::Subchapter),
            "paragraph" => Some(NodeKind::Paragraph),
            "image" => Some(NodeKind::Image),
            _ => None,
        }
    }

    /// Image / Paragraph are leaves; chapters / subchapters / books
    /// can have children. Used in tree-rendering and the placement
    /// validator.
    pub fn is_leaf(&self) -> bool {
        matches!(self, NodeKind::Paragraph | NodeKind::Image)
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
            _ => format!("{:02}-{}", self.order, self.slug),
        }
    }
}
