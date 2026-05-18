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
}

impl NodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeKind::Book => "book",
            NodeKind::Chapter => "chapter",
            NodeKind::Subchapter => "subchapter",
            NodeKind::Paragraph => "paragraph",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "book" => Some(NodeKind::Book),
            "chapter" => Some(NodeKind::Chapter),
            "subchapter" => Some(NodeKind::Subchapter),
            "paragraph" => Some(NodeKind::Paragraph),
            _ => None,
        }
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
}

impl Node {
    pub fn to_json(&self) -> JsonValue {
        json!({
            "kind":        self.kind.as_str(),
            "title":       self.title,
            "slug":        self.slug,
            "path":        self.path,
            "parent_id":   self.parent_id.map(|u| u.to_string()),
            "order":       self.order,
            "file":        self.file,
            "word_count":  self.word_count,
            "modified_at": self.modified_at.to_rfc3339(),
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
        })
    }

    /// Filesystem segment name for this node. Books use bare slugs; everything
    /// else gets a zero-padded numeric prefix so directory listings sort
    /// correctly (`01-preface.typ`, `02-chapter-one/`, …).
    pub fn fs_name(&self) -> String {
        match self.kind {
            NodeKind::Book => self.slug.clone(),
            NodeKind::Paragraph => format!("{:02}-{}.typ", self.order, self.slug),
            _ => format!("{:02}-{}", self.order, self.slug),
        }
    }
}
