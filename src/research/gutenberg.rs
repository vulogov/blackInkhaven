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

impl GutenbergBook {
    /// PG-P3 — a SOURCES-1 `BibEntry` for the book (auto-cite). Gutendex carries
    /// no publication year, so `year` is left empty; the PG id is the note.
    pub(super) fn to_bibentry(&self) -> crate::sources::BibEntry {
        let surname: String = self
            .authors
            .first()
            .map(|a| a.split(',').next().unwrap_or(a).trim().to_ascii_lowercase())
            .unwrap_or_default()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect();
        let key = if surname.is_empty() { format!("pg{}", self.id) } else { format!("{surname}pg{}", self.id) };
        crate::sources::BibEntry {
            key,
            entry_type: "book".to_string(),
            author: self.authors.join(" and "),
            title: self.title.clone(),
            url: Some(format!("https://www.gutenberg.org/ebooks/{}", self.id)),
            note: Some(format!("Project Gutenberg #{}", self.id)),
            ..Default::default()
        }
    }
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

/// Search Gutendex (or, when `query` is a bare PG id, fetch that book directly),
/// download + strip the chosen book's text (capped at `max_chars`), and return it
/// with a few **alternative** matches (PG-P4 picker). Owned args → spawnable.
pub(super) async fn fetch(
    cfg: GutenbergConfig,
    query: String,
    language: String,
) -> Result<(GutenbergBook, String, Vec<GutenbergBook>)> {
    let base = cfg.endpoint.trim_end_matches('/').to_string();
    let client = client()?;

    // A bare number is a Project Gutenberg id → fetch that exact book.
    let (chosen, alternatives) = if let Ok(id) = query.trim().parse::<i64>() {
        let json: Json = client
            .get(format!("{base}/books/{id}"))
            .send()
            .await
            .map_err(|e| anyhow!("gutenberg lookup: {e}"))?
            .json()
            .await
            .map_err(|e| anyhow!("gutenberg decode: {e}"))?;
        let book = parse_book(&json)
            .ok_or_else(|| anyhow!("Project Gutenberg #{id} has no plain-text edition"))?;
        (book, Vec::new())
    } else {
        let mut books = search_books(&client, &base, &query, &language).await?;
        if books.is_empty() {
            return Err(anyhow!("no Project Gutenberg book for `{query}`"));
        }
        let chosen = books.remove(0);
        books.truncate(4);
        (chosen, books)
    };

    let raw = client
        .get(&chosen.text_url)
        .send()
        .await
        .map_err(|e| anyhow!("gutenberg text: {e}"))?
        .text()
        .await
        .map_err(|e| anyhow!("gutenberg text decode: {e}"))?;
    let stripped = strip_pg_boilerplate(&raw);
    let capped: String = stripped.chars().take(cfg.max_chars.max(1000)).collect();
    Ok((chosen, capped, alternatives))
}

/// Query Gutendex `/books?search=` → the results carrying a plain-text download.
async fn search_books(
    client: &reqwest::Client,
    base: &str,
    query: &str,
    language: &str,
) -> Result<Vec<GutenbergBook>> {
    let mut q: Vec<(&str, String)> = vec![("search", query.to_string())];
    if !language.is_empty() {
        q.push(("languages", language.to_string()));
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
    Ok(json
        .get("results")
        .and_then(|r| r.as_array())
        .map(|a| a.iter().filter_map(parse_book).collect())
        .unwrap_or_default())
}

/// One Gutendex book object (with a plain-text download) → a `GutenbergBook`.
fn parse_book(r: &Json) -> Option<GutenbergBook> {
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

/// PG-P4 — split a book's stripped text into chapters on heading lines
/// (`CHAPTER …`, `PART …`, `BOOK …`, or a standalone roman/arabic numeral). A
/// best-effort heuristic; returns the whole text as a single chapter when no
/// heading is found. Chapters are 1-indexed by the caller.
pub(super) fn split_chapters(text: &str) -> Vec<String> {
    let mut starts: Vec<usize> = Vec::new();
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        if is_chapter_heading(line.trim()) {
            starts.push(offset);
        }
        offset += line.len();
    }
    if starts.is_empty() {
        return vec![text.to_string()];
    }
    // Preamble before the first heading (title page etc.) is dropped.
    let mut chapters = Vec::new();
    for (i, &s) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(text.len());
        let seg = text[s..end].trim();
        if !seg.is_empty() {
            chapters.push(seg.to_string());
        }
    }
    chapters
}

/// Whether a trimmed line looks like a chapter heading.
fn is_chapter_heading(line: &str) -> bool {
    if line.is_empty() || line.len() > 60 {
        return false;
    }
    let upper = line.to_ascii_uppercase();
    if upper.starts_with("CHAPTER ") || upper.starts_with("PART ") || upper.starts_with("BOOK ") {
        return true;
    }
    // A standalone roman numeral (`I`, `IV`, `X021`… ) or arabic number line.
    let head = line.split_whitespace().next().unwrap_or("").trim_end_matches('.');
    !head.is_empty()
        && (head.chars().all(|c| "IVXLCDM".contains(c.to_ascii_uppercase()))
            || head.chars().all(|c| c.is_ascii_digit()))
        && line.split_whitespace().count() <= 3
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
        let j = serde_json::json!({
            "id": 1342, "title": "Pride and Prejudice",
            "authors": [{"name": "Austen, Jane"}],
            "subjects": ["England -- Fiction", "Love stories"],
            "formats": {
                "application/epub+zip": "https://x/1342.epub",
                "text/plain; charset=utf-8": "https://www.gutenberg.org/ebooks/1342.txt.utf-8"
            }
        });
        let b = parse_book(&j).unwrap();
        assert_eq!(b.id, 1342);
        assert_eq!(b.title, "Pride and Prejudice");
        assert_eq!(b.authors, vec!["Austen, Jane"]);
        assert!(b.text_url.ends_with("1342.txt.utf-8"));
    }

    #[test]
    fn skips_result_without_plaintext() {
        let audio = serde_json::json!({"id": 1, "title": "audio only", "formats": {"audio/mpeg": "https://x/a.mp3"}});
        let text = serde_json::json!({"id": 2, "title": "has text", "formats": {"text/plain": "https://x/2.txt"}});
        assert!(parse_book(&audio).is_none());
        assert_eq!(parse_book(&text).unwrap().id, 2);
    }

    #[test]
    fn book_to_bibentry() {
        let b = GutenbergBook {
            id: 1342,
            title: "Pride and Prejudice".into(),
            authors: vec!["Austen, Jane".into()],
            subjects: vec![],
            text_url: "https://x".into(),
        };
        let e = b.to_bibentry();
        assert_eq!(e.key, "austenpg1342");
        assert_eq!(e.entry_type, "book");
        assert_eq!(e.author, "Austen, Jane");
        assert_eq!(e.note.as_deref(), Some("Project Gutenberg #1342"));
        assert!(e.is_valid());
    }

    #[test]
    fn splits_chapters_on_headings() {
        let text = "Preamble matter.\nCHAPTER I\nThe first bit.\nCHAPTER II\nThe second bit.";
        let ch = split_chapters(text);
        assert_eq!(ch.len(), 2);
        assert!(ch[0].starts_with("CHAPTER I"));
        assert!(ch[1].contains("second bit"));
        // No headings → one chapter (the whole text).
        assert_eq!(split_chapters("just prose, no headings").len(), 1);
    }
}
