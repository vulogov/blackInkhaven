//! RESRCH-WIKISOURCE (1.6.16+) — Wikisource as a multilingual public-domain
//! research source. Search `{lang}.wikisource.org` via the keyless MediaWiki API
//! (`list=search`), then fetch the chosen page's plain-text extract
//! (`prop=extracts&explaintext`) and ingest it as a `research_source`.
//!
//! One adapter serves every language: the subdomain is the book's language code
//! (Russian book → `ru.wikisource.org`), so a native-language author gets native
//! public-domain texts — classics, the Synodal Bible, philosophy translations.

use anyhow::{Result, anyhow};
use serde_json::Value as Json;

use crate::config::WikisourceConfig;

/// One Wikisource page hit.
#[derive(Debug, Clone)]
pub(super) struct WikisourcePage {
    pub title: String,
    pub lang: String,
    #[allow(dead_code)]
    pub pageid: i64,
}

impl WikisourcePage {
    /// The article URL (underscored title).
    pub(super) fn url(&self) -> String {
        format!(
            "https://{}.wikisource.org/wiki/{}",
            self.lang,
            self.title.replace(' ', "_")
        )
    }

    /// A SOURCES-1 `BibEntry` (auto-cite). Wikisource search gives no author, so
    /// `author` is left empty; the wiki + language are the note.
    pub(super) fn to_bibentry(&self) -> crate::sources::BibEntry {
        let slug: String = self
            .title
            .to_ascii_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(20)
            .collect();
        crate::sources::BibEntry {
            key: format!("ws{}{slug}", self.lang),
            entry_type: "misc".to_string(),
            title: self.title.clone(),
            url: Some(self.url()),
            note: Some(format!("Wikisource ({})", self.lang)),
            ..Default::default()
        }
    }
}

pub(super) fn available(cfg: &WikisourceConfig) -> bool {
    cfg.enabled
}

fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("inkhaven-research/1.0 (https://crates.io/crates/inkhaven)")
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| anyhow!("http client: {e}"))
}

/// Search `{lang}.wikisource.org` for `query`, fetch the top page's plain-text
/// extract (capped at `max_chars`), and return it with a few alternatives.
/// `lang` is the book's language code (falls back to `cfg.default_lang`).
pub(super) async fn fetch(
    cfg: WikisourceConfig,
    query: String,
    lang: String,
) -> Result<(WikisourcePage, String, Vec<WikisourcePage>)> {
    let lang = if lang.trim().is_empty() {
        cfg.default_lang.clone()
    } else {
        lang
    };
    let host = format!("https://{lang}.wikisource.org");
    let client = client()?;

    let mut pages = search(&client, &host, &lang, &query).await?;
    if pages.is_empty() {
        return Err(anyhow!("no Wikisource ({lang}) page for `{query}`"));
    }
    let chosen = pages.remove(0);
    pages.truncate(4);

    let text = fetch_extract(&client, &host, &chosen.title).await?;
    let capped: String = text.trim().chars().take(cfg.max_chars.max(1000)).collect();
    if capped.trim().is_empty() {
        return Err(anyhow!("`{}` has no extractable plain text", chosen.title));
    }
    Ok((chosen, capped, pages))
}

/// MediaWiki `list=search` over the main namespace.
async fn search(
    client: &reqwest::Client,
    host: &str,
    lang: &str,
    query: &str,
) -> Result<Vec<WikisourcePage>> {
    let params = [
        ("action", "query"),
        ("format", "json"),
        ("list", "search"),
        ("srsearch", query),
        ("srnamespace", "0"),
        ("srlimit", "5"),
    ];
    let json: Json = client
        .get(format!("{host}/w/api.php"))
        .query(&params)
        .send()
        .await
        .map_err(|e| anyhow!("wikisource search: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow!("wikisource decode: {e}"))?;
    Ok(json
        .get("query")
        .and_then(|q| q.get("search"))
        .and_then(|s| s.as_array())
        .map(|a| a.iter().filter_map(|d| parse_search(lang, d)).collect())
        .unwrap_or_default())
}

fn parse_search(lang: &str, d: &Json) -> Option<WikisourcePage> {
    let title = d.get("title")?.as_str()?.trim().to_string();
    if title.is_empty() {
        return None;
    }
    Some(WikisourcePage {
        pageid: d.get("pageid").and_then(|v| v.as_i64()).unwrap_or(0),
        lang: lang.to_string(),
        title,
    })
}

/// MediaWiki `prop=extracts&explaintext` — the page's full plain text. `pages` is
/// keyed by page id; we take the first non-empty `extract`.
async fn fetch_extract(client: &reqwest::Client, host: &str, title: &str) -> Result<String> {
    let params = [
        ("action", "query"),
        ("format", "json"),
        ("prop", "extracts"),
        ("explaintext", "1"),
        ("redirects", "1"),
        ("titles", title),
    ];
    let json: Json = client
        .get(format!("{host}/w/api.php"))
        .query(&params)
        .send()
        .await
        .map_err(|e| anyhow!("wikisource extract: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow!("wikisource decode: {e}"))?;
    let pages = json
        .get("query")
        .and_then(|q| q.get("pages"))
        .and_then(|p| p.as_object())
        .ok_or_else(|| anyhow!("wikisource: no pages for `{title}`"))?;
    for page in pages.values() {
        if let Some(ex) = page.get("extract").and_then(|e| e.as_str()) {
            if !ex.trim().is_empty() {
                return Ok(ex.to_string());
            }
        }
    }
    Err(anyhow!("wikisource: no plain-text extract for `{title}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_result() {
        let d = serde_json::json!({ "title": "Война и мир", "pageid": 42, "snippet": "…" });
        let p = parse_search("ru", &d).unwrap();
        assert_eq!(p.title, "Война и мир");
        assert_eq!(p.lang, "ru");
        assert_eq!(p.pageid, 42);
        assert!(p.url().contains("ru.wikisource.org/wiki/"));
        assert!(parse_search("ru", &serde_json::json!({ "pageid": 1 })).is_none());
    }

    #[test]
    fn page_to_bibentry() {
        let p = WikisourcePage { title: "Meditations".into(), lang: "en".into(), pageid: 7 };
        let e = p.to_bibentry();
        assert_eq!(e.key, "wsenmeditations");
        assert_eq!(e.title, "Meditations");
        assert_eq!(e.note.as_deref(), Some("Wikisource (en)"));
        assert!(e.url.as_deref().unwrap().contains("en.wikisource.org"));
        assert!(e.is_valid());
    }
}
