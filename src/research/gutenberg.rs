//! RESRCH-GUTENBERG (PG-P1) — Project Gutenberg as a research source. Search the
//! **Gutendex** catalogue (keyless JSON API over the ~75k public-domain PG books,
//! metadata search) for the best match, then fetch + strip its plain text. The
//! text flows through the existing `/import` ingestion path (chunk → embed as a
//! `research_source`), so the corpus RAG surfaces the relevant **snippets**.
//!
//! `reqwest` (already present); plain text, so no HTML parser — just strip the PG
//! header/footer boilerplate.

use anyhow::{Result, anyhow};
use serde_json::Value as Json;

use crate::config::GutenbergConfig;

/// One catalogue hit with a plain-text download.
#[derive(Debug, Clone)]
pub(super) struct GutenbergBook {
    pub id: i64,
    pub title: String,
    pub authors: Vec<String>,
    pub subjects: Vec<String>,
    pub text_url: String,
}

pub(super) fn available(cfg: &GutenbergConfig) -> bool {
    cfg.enabled
}

fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("inkhaven-research/1.0 (https://crates.io/crates/inkhaven)")
        .build()
        .map_err(|e| anyhow!("http client: {e}"))
}

/// Search Gutendex for the top matching book (with a plain-text download), then
/// fetch + strip its text (capped at `max_chars`). Owned args → spawnable.
pub(super) async fn fetch(
    cfg: GutenbergConfig,
    query: String,
    language: String,
) -> Result<(GutenbergBook, String)> {
    let base = cfg.endpoint.trim_end_matches('/').to_string();
    let client = client()?;
    let mut q: Vec<(&str, String)> = vec![("search", query.clone())];
    if !language.is_empty() {
        q.push(("languages", language));
    }
    let json: Json = client
        .get(format!("{base}/books"))
        .query(&q)
        .send()
        .await
        .map_err(|e| anyhow!("gutenberg search: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow!("gutenberg decode: {e}"))?;
    let book = parse_first(&json).ok_or_else(|| anyhow!("no Project Gutenberg book for `{query}`"))?;
    let raw = client
        .get(&book.text_url)
        .send()
        .await
        .map_err(|e| anyhow!("gutenberg text: {e}"))?
        .text()
        .await
        .map_err(|e| anyhow!("gutenberg text decode: {e}"))?;
    let stripped = strip_pg_boilerplate(&raw);
    let capped: String = stripped.chars().take(cfg.max_chars.max(1000)).collect();
    Ok((book, capped))
}

/// The first result carrying a plain-text download → a `GutenbergBook`.
fn parse_first(json: &Json) -> Option<GutenbergBook> {
    let r = json.get("results")?.as_array()?.iter().find(|b| text_url_of(b).is_some())?;
    Some(GutenbergBook {
        id: r.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
        title: r.get("title").and_then(|v| v.as_str()).unwrap_or("Untitled").to_string(),
        authors: r
            .get("authors")
            .and_then(|a| a.as_array())
            .map(|a| a.iter().filter_map(|x| x.get("name")?.as_str().map(str::to_string)).collect())
            .unwrap_or_default(),
        subjects: r
            .get("subjects")
            .and_then(|s| s.as_array())
            .map(|s| s.iter().filter_map(|x| x.as_str().map(str::to_string)).take(4).collect())
            .unwrap_or_default(),
        text_url: text_url_of(r)?,
    })
}

/// The `text/plain` download URL from a Gutendex `formats` map (skips `.zip`).
fn text_url_of(book: &Json) -> Option<String> {
    let formats = book.get("formats")?.as_object()?;
    formats
        .iter()
        .filter(|(k, _)| k.starts_with("text/plain"))
        .filter_map(|(_, v)| v.as_str())
        .find(|u| !u.ends_with(".zip"))
        .map(str::to_string)
}

/// Strip the PG header/footer boilerplate around the book body (leaving the text
/// itself). Tolerates both `THE`/`THIS` variants; returns the whole text if the
/// markers are absent.
pub(super) fn strip_pg_boilerplate(text: &str) -> String {
    let start = text
        .find("*** START OF THE PROJECT GUTENBERG")
        .or_else(|| text.find("*** START OF THIS PROJECT GUTENBERG"))
        .and_then(|i| text[i..].find('\n').map(|j| i + j + 1))
        .unwrap_or(0);
    let end = text[start..]
        .find("*** END OF THE PROJECT GUTENBERG")
        .or_else(|| text[start..].find("*** END OF THIS PROJECT GUTENBERG"))
        .map(|e| start + e)
        .unwrap_or(text.len());
    text[start..end].trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_pg_boilerplate() {
        let doc = "The Project Gutenberg eBook of X\nlicense blah\n\
                   *** START OF THE PROJECT GUTENBERG EBOOK X ***\n\
                   Chapter I. It was a bright cold day.\n\
                   *** END OF THE PROJECT GUTENBERG EBOOK X ***\n\
                   footer blah";
        let body = strip_pg_boilerplate(doc);
        assert_eq!(body, "Chapter I. It was a bright cold day.");
        // No markers → unchanged (trimmed).
        assert_eq!(strip_pg_boilerplate("  plain body  "), "plain body");
    }

    #[test]
    fn parses_gutendex_result() {
        let j = serde_json::json!({"results":[{
            "id": 1342, "title": "Pride and Prejudice",
            "authors": [{"name": "Austen, Jane"}],
            "subjects": ["England -- Fiction", "Love stories"],
            "formats": {
                "application/epub+zip": "https://x/1342.epub",
                "text/plain; charset=utf-8": "https://www.gutenberg.org/ebooks/1342.txt.utf-8"
            }
        }]});
        let b = parse_first(&j).unwrap();
        assert_eq!(b.id, 1342);
        assert_eq!(b.title, "Pride and Prejudice");
        assert_eq!(b.authors, vec!["Austen, Jane"]);
        assert!(b.text_url.ends_with("1342.txt.utf-8"));
    }

    #[test]
    fn skips_result_without_plaintext() {
        let j = serde_json::json!({"results":[
            {"id": 1, "title": "audio only", "formats": {"audio/mpeg": "https://x/a.mp3"}},
            {"id": 2, "title": "has text", "formats": {"text/plain": "https://x/2.txt"}}
        ]});
        assert_eq!(parse_first(&j).unwrap().id, 2);
    }
}
