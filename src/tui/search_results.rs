use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::store::node::NodeKind;

/// Typed view of a single bdslib search result. Built by `SearchHit::parse`
/// from the JSON shape `{ id, metadata, document, score }` returned by
/// `DocumentStorage::search_document_text`.
#[derive(Debug, Clone)]
pub struct SearchHit {
    pub id: Uuid,
    pub score: f64,
    pub kind: NodeKind,
    pub title: String,
    /// Slug-derived filesystem path; kept around for diagnostics and for
    /// the parse test, but the UI renders a title-based breadcrumb instead
    /// (see `App::title_breadcrumb`). The CLI search subcommand still
    /// displays it.
    #[allow(dead_code)]
    pub slug_path: String,
    pub snippet: String,
}

impl SearchHit {
    pub fn parse(value: &JsonValue) -> Option<Self> {
        let id = value.get("id").and_then(|v| v.as_str())?;
        let id = Uuid::parse_str(id).ok()?;
        let score = value.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);

        let meta = value.get("metadata")?;
        let kind = meta
            .get("kind")
            .and_then(|v| v.as_str())
            .and_then(NodeKind::from_str)?;
        let title = meta
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("(untitled)")
            .to_string();
        let slug = meta.get("slug").and_then(|v| v.as_str()).unwrap_or("");
        let path: Vec<&str> = meta
            .get("path")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();
        let mut slug_path = path.join("/");
        if !slug_path.is_empty() && !slug.is_empty() {
            slug_path.push('/');
        }
        slug_path.push_str(slug);

        let snippet = extract_snippet(
            value
                .get("document")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
        );

        Some(Self {
            id,
            score,
            kind,
            title,
            slug_path,
            snippet,
        })
    }
}

/// Pick a useful one-line preview from a document body:
/// skip blank lines and Typst heading lines (`= …`), then truncate to 80
/// characters. Returns an empty string when only headings exist (typical for
/// freshly-`add`ed paragraphs).
fn extract_snippet(s: &str) -> String {
    for line in s.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let stripped = trimmed.trim_start_matches('=').trim_start();
        if stripped.len() != trimmed.len() && stripped != trimmed {
            // Heading line — skip.
            continue;
        }
        let chars: Vec<char> = trimmed.chars().collect();
        if chars.len() > 80 {
            let truncated: String = chars.iter().take(80).collect();
            return format!("{truncated}…");
        }
        return trimmed.to_string();
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_full() {
        let v = json!({
            "id": "019e3cf8-7023-7432-8a11-02b05cc8f1d0",
            "score": 0.91,
            "metadata": {
                "kind": "paragraph",
                "title": "Opening",
                "slug": "opening",
                "path": ["the-lighthouse", "storm", "morning-light"],
            },
            "document": "= Opening\n\nThe thunderstruck mariner stood at the rail.",
        });
        let hit = SearchHit::parse(&v).unwrap();
        assert_eq!(hit.kind, NodeKind::Paragraph);
        assert_eq!(hit.title, "Opening");
        assert_eq!(hit.slug_path, "the-lighthouse/storm/morning-light/opening");
        assert!(hit.snippet.starts_with("The thunderstruck"));
        assert!((hit.score - 0.91).abs() < 1e-6);
    }

    #[test]
    fn snippet_skips_heading() {
        assert_eq!(extract_snippet("= Title\n\nbody text"), "body text");
        assert_eq!(extract_snippet("= Title\n"), "");
    }

    #[test]
    fn snippet_truncates() {
        let long = "a".repeat(200);
        let s = extract_snippet(&long);
        assert!(s.ends_with("…"));
        assert!(s.chars().count() <= 81);
    }
}
