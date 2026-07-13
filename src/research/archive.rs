//! RESRCH-ARCHIVE (1.6.16+) — the Internet Archive as a research source. Search
//! archive.org's keyless `advancedsearch.php` (scoped to `mediatype:texts`) for the
//! best public-domain match, fetch its OCR plain text (`<id>_djvu.txt`), and ingest
//! it through the existing `/import` path (chunk → embed as a `research_source`), so
//! the corpus RAG surfaces the relevant snippets.
//!
//! Mirrors `/gutenberg`: `reqwest` (already present), plain text (no HTML parser).
//! archive.org's OCR text has no Project Gutenberg boilerplate, so the body is used
//! as-is (trimmed + capped).

use anyhow::{Result, anyhow};
use serde_json::Value as Json;

use crate::config::ArchiveConfig;

/// One Internet Archive text item with a plain-text download.
#[derive(Debug, Clone)]
pub(super) struct ArchiveItem {
    pub identifier: String,
    pub title: String,
    pub authors: Vec<String>,
    pub year: String,
    #[allow(dead_code)]
    pub description: String,
    pub text_url: String,
}

impl ArchiveItem {
    /// A SOURCES-1 `BibEntry` for the item (auto-cite). Unlike Gutendex, archive.org
    /// usually carries a publication year.
    pub(super) fn to_bibentry(&self) -> crate::sources::BibEntry {
        let surname: String = self
            .authors
            .first()
            .map(|a| a.split(',').next().unwrap_or(a).trim().to_ascii_lowercase())
            .unwrap_or_default()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();
        let slug: String = self
            .identifier
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(16)
            .collect();
        let key = if surname.is_empty() {
            format!("ia{slug}")
        } else {
            format!("{surname}ia{slug}")
        };
        crate::sources::BibEntry {
            key,
            entry_type: "book".to_string(),
            author: self.authors.join(" and "),
            title: self.title.clone(),
            year: self.year.clone(),
            url: Some(format!("https://archive.org/details/{}", self.identifier)),
            note: Some(format!("Internet Archive: {}", self.identifier)),
            ..Default::default()
        }
    }
}

pub(super) fn available(cfg: &ArchiveConfig) -> bool {
    cfg.enabled
}

fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("inkhaven-research/1.0 (https://crates.io/crates/inkhaven)")
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| anyhow!("http client: {e}"))
}

/// Search archive.org for public-domain text items matching `query`, download the
/// top hit's OCR plain text (capped at `max_chars`), and return it with a few
/// **alternative** matches (a re-run picker). Owned args → spawnable. When
/// `language` is non-empty it restricts the search to that language facet.
pub(super) async fn fetch(
    cfg: ArchiveConfig,
    query: String,
    language: String,
) -> Result<(ArchiveItem, String, Vec<ArchiveItem>)> {
    let base = cfg.endpoint.trim_end_matches('/').to_string();
    let client = client()?;

    let mut items = search_items(&client, &base, &query, &language).await?;
    if items.is_empty() {
        return Err(anyhow!("no Internet Archive text for `{query}`"));
    }
    let chosen = items.remove(0);
    items.truncate(4);

    let raw = client
        .get(&chosen.text_url)
        .send()
        .await
        .map_err(|e| anyhow!("archive text: {e}"))?
        .text()
        .await
        .map_err(|e| anyhow!("archive text decode: {e}"))?;
    let capped: String = raw.trim().chars().take(cfg.max_chars.max(1000)).collect();
    if capped.trim().is_empty() {
        return Err(anyhow!(
            "`{}` has no OCR plain text (try another match)",
            chosen.identifier
        ));
    }
    Ok((chosen, capped, items))
}

/// Query `advancedsearch.php` (scoped to text media) → the item docs.
async fn search_items(
    client: &reqwest::Client,
    base: &str,
    query: &str,
    language: &str,
) -> Result<Vec<ArchiveItem>> {
    let mut q = format!("({query}) AND mediatype:texts");
    if !language.is_empty() {
        q.push_str(&format!(" AND language:{language}"));
    }
    let params: Vec<(&str, String)> = vec![
        ("q", q),
        ("fl[]", "identifier".to_string()),
        ("fl[]", "title".to_string()),
        ("fl[]", "creator".to_string()),
        ("fl[]", "year".to_string()),
        ("fl[]", "description".to_string()),
        ("sort[]", "downloads desc".to_string()),
        ("rows", "5".to_string()),
        ("page", "1".to_string()),
        ("output", "json".to_string()),
    ];
    let json: Json = client
        .get(format!("{base}/advancedsearch.php"))
        .query(&params)
        .send()
        .await
        .map_err(|e| anyhow!("archive search: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow!("archive decode: {e}"))?;
    Ok(json
        .get("response")
        .and_then(|r| r.get("docs"))
        .and_then(|d| d.as_array())
        .map(|a| a.iter().filter_map(|d| parse_item(base, d)).collect())
        .unwrap_or_default())
}

/// One `advancedsearch` doc → an `ArchiveItem`. archive.org fields may be a string
/// or an array of strings; both are tolerated.
fn parse_item(base: &str, d: &Json) -> Option<ArchiveItem> {
    let identifier = d.get("identifier")?.as_str()?.trim().to_string();
    if identifier.is_empty() {
        return None;
    }
    let title = {
        let t = d.get("title").map(json_first_string).unwrap_or_default();
        if t.is_empty() { "Untitled".to_string() } else { t }
    };
    Some(ArchiveItem {
        text_url: format!("{base}/download/{identifier}/{identifier}_djvu.txt"),
        authors: json_string_list(d.get("creator")),
        year: d.get("year").map(json_first_string).unwrap_or_default(),
        description: d.get("description").map(json_first_string).unwrap_or_default(),
        title,
        identifier,
    })
}

/// A field that is a string or an array-of-strings → the first string.
fn json_first_string(v: &Json) -> String {
    match v {
        Json::String(s) => s.trim().to_string(),
        Json::Array(a) => a
            .iter()
            .find_map(|x| x.as_str())
            .unwrap_or("")
            .trim()
            .to_string(),
        _ => String::new(),
    }
}

/// A field that is a string or an array-of-strings → a `Vec<String>`.
fn json_string_list(v: Option<&Json>) -> Vec<String> {
    match v {
        Some(Json::String(s)) => vec![s.trim().to_string()],
        Some(Json::Array(a)) => a
            .iter()
            .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_advancedsearch_doc_string_and_array_fields() {
        // `creator` as an array; `title`/`year` as strings.
        let d = serde_json::json!({
            "identifier": "gita-as-it-is",
            "title": "Bhagavad Gita",
            "creator": ["Vyasa", "Trans. Anon"],
            "year": "1901",
            "description": "A public-domain translation."
        });
        let item = parse_item("https://archive.org", &d).unwrap();
        assert_eq!(item.identifier, "gita-as-it-is");
        assert_eq!(item.title, "Bhagavad Gita");
        assert_eq!(item.authors, vec!["Vyasa", "Trans. Anon"]);
        assert_eq!(item.year, "1901");
        assert!(item.text_url.ends_with("gita-as-it-is/gita-as-it-is_djvu.txt"));

        // `creator` as a bare string; missing title → "Untitled".
        let d2 = serde_json::json!({ "identifier": "x", "creator": "Kant, Immanuel" });
        let item2 = parse_item("https://archive.org", &d2).unwrap();
        assert_eq!(item2.authors, vec!["Kant, Immanuel"]);
        assert_eq!(item2.title, "Untitled");

        // No identifier → dropped.
        assert!(parse_item("https://archive.org", &serde_json::json!({ "title": "no id" })).is_none());
    }

    #[test]
    fn item_to_bibentry() {
        let item = ArchiveItem {
            identifier: "critiqueofpurer00kant".into(),
            title: "Critique of Pure Reason".into(),
            authors: vec!["Kant, Immanuel".into()],
            year: "1901".into(),
            description: String::new(),
            text_url: "https://x".into(),
        };
        let e = item.to_bibentry();
        assert_eq!(e.key, "kantiacritiqueofpurer0");
        assert_eq!(e.entry_type, "book");
        assert_eq!(e.author, "Kant, Immanuel");
        assert_eq!(e.year, "1901");
        assert_eq!(e.note.as_deref(), Some("Internet Archive: critiqueofpurer00kant"));
        assert!(e.is_valid());
    }
}
