//! RESRCH-3 (R3-B) — scholarly sources: `/openalex` and `/arxiv`. Both keyless
//! (`reqwest`), returning a paper's title / authors / year / abstract + a stable
//! **DOI / arXiv-ID**. Scholarly tier of the trust ladder — the metadata is
//! authoritative (gate skipped), and a `/fact` from a paper can auto-create a
//! SOURCES-1 `BibEntry` so the fact and a real bibliography entry land together.

use std::sync::LazyLock;

use anyhow::{Result, anyhow};
use serde_json::Value as Json;

use crate::config::ScholarlyConfig;
use crate::sources::BibEntry;

/// One resolved paper (from either provider).
#[derive(Debug, Clone)]
pub(super) struct Paper {
    pub source: &'static str, // "openalex" | "arxiv"
    pub id: String,           // OpenAlex work id (W…) or arXiv id
    pub doi: String,          // bare DOI, or empty
    pub title: String,
    pub authors: Vec<String>,
    pub year: String,
    pub abstract_: String,
    pub url: String,
}

impl Paper {
    /// The provenance detail: the DOI when present, else the provider id.
    pub(super) fn cite_detail(&self) -> String {
        if !self.doi.is_empty() {
            format!("doi:{}", self.doi)
        } else {
            format!("{}:{}", self.source, self.id)
        }
    }

    /// A cite key: first author's surname + year (slug), else provider+id.
    fn cite_key(&self) -> String {
        let surname = self
            .authors
            .first()
            .and_then(|a| a.split_whitespace().last())
            .unwrap_or(self.source);
        let base: String = format!("{surname}{}", self.year)
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_lowercase();
        if base.is_empty() { format!("{}-{}", self.source, self.id) } else { base }
    }

    /// Build a SOURCES-1 `BibEntry` for the auto-citation (R3-B).
    pub(super) fn to_bibentry(&self) -> BibEntry {
        BibEntry {
            key: self.cite_key(),
            entry_type: "article".to_string(),
            author: self.authors.join(" and "),
            title: self.title.clone(),
            year: self.year.clone(),
            doi: (!self.doi.is_empty()).then(|| self.doi.clone()),
            url: (!self.url.is_empty()).then(|| self.url.clone()),
            note: Some(format!("{} {}", self.source, self.id)),
            abstract_: (!self.abstract_.is_empty()).then(|| truncate(&self.abstract_, 2000)),
            ..Default::default()
        }
    }
}

pub(super) fn available(cfg: &ScholarlyConfig) -> bool {
    cfg.enabled
}

// ── OpenAlex ─────────────────────────────────────────────────────────────────

/// Query OpenAlex `works` for the top match. `mailto` (config) joins the polite
/// pool. Owned args so it can be spawned onto a tokio task.
pub(super) async fn openalex(cfg: ScholarlyConfig, query: String) -> Result<Paper> {
    let mut q: Vec<(&str, String)> =
        vec![("search", query.clone()), ("per_page", "1".to_string())];
    if !cfg.mailto.trim().is_empty() {
        q.push(("mailto", cfg.mailto.trim().to_string()));
    }
    let json: Json = client()?
        .get("https://api.openalex.org/works")
        .query(&q)
        .send()
        .await
        .map_err(|e| anyhow!("openalex request: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow!("openalex decode: {e}"))?;
    parse_openalex(&json).ok_or_else(|| anyhow!("no OpenAlex result for `{query}`"))
}

fn parse_openalex(json: &Json) -> Option<Paper> {
    let w = json.get("results")?.as_array()?.first()?;
    let id = strip_prefix_url(w.get("id")?.as_str()?, "https://openalex.org/");
    let doi = w
        .get("doi")
        .and_then(|d| d.as_str())
        .map(|d| strip_prefix_url(d, "https://doi.org/"))
        .unwrap_or_default();
    let title = w
        .get("display_name")
        .or_else(|| w.get("title"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    let year = w.get("publication_year").and_then(|y| y.as_i64()).map(|y| y.to_string()).unwrap_or_default();
    let authors: Vec<String> = w
        .get("authorships")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.get("author")?.get("display_name")?.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let abstract_ = w
        .get("abstract_inverted_index")
        .filter(|v| !v.is_null())
        .map(reconstruct_abstract)
        .unwrap_or_default();
    let url = w
        .get("primary_location")
        .and_then(|l| l.get("landing_page_url"))
        .and_then(|u| u.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            if !doi.is_empty() { format!("https://doi.org/{doi}") } else { format!("https://openalex.org/{id}") }
        });
    Some(Paper { source: "openalex", id, doi, title, authors, year, abstract_, url })
}

/// Rebuild an abstract from OpenAlex's `{word: [positions]}` inverted index.
fn reconstruct_abstract(inv: &Json) -> String {
    let Some(obj) = inv.as_object() else { return String::new() };
    let mut positioned: Vec<(u64, &str)> = Vec::new();
    for (word, positions) in obj {
        if let Some(arr) = positions.as_array() {
            for p in arr {
                if let Some(i) = p.as_u64() {
                    positioned.push((i, word.as_str()));
                }
            }
        }
    }
    positioned.sort_by_key(|(i, _)| *i);
    positioned.iter().map(|(_, w)| *w).collect::<Vec<_>>().join(" ")
}

// ── arXiv ────────────────────────────────────────────────────────────────────

/// Query the arXiv Atom API for the top match (crate-free XML extraction).
pub(super) async fn arxiv(query: String) -> Result<Paper> {
    let xml = client()?
        .get("https://export.arxiv.org/api/query")
        .query(&[("search_query", format!("all:{query}")), ("max_results", "1".to_string())])
        .send()
        .await
        .map_err(|e| anyhow!("arxiv request: {e}"))?
        .text()
        .await
        .map_err(|e| anyhow!("arxiv decode: {e}"))?;
    parse_arxiv_atom(&xml).ok_or_else(|| anyhow!("no arXiv result for `{query}`"))
}

static ENTRY: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?s)<entry>(.*?)</entry>").unwrap());
static TAG_TITLE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?s)<title>(.*?)</title>").unwrap());
static TAG_SUMMARY: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?s)<summary>(.*?)</summary>").unwrap());
static TAG_PUBLISHED: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?s)<published>(.*?)</published>").unwrap());
static TAG_ID: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?s)<id>(.*?)</id>").unwrap());
static TAG_NAME: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?s)<name>(.*?)</name>").unwrap());
static TAG_DOI: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?s)<arxiv:doi[^>]*>(.*?)</arxiv:doi>").unwrap());
static WS: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"\s+").unwrap());

fn parse_arxiv_atom(xml: &str) -> Option<Paper> {
    let entry = ENTRY.captures(xml)?.get(1)?.as_str();
    let title = cap(&TAG_TITLE, entry).unwrap_or_default();
    let abstract_ = cap(&TAG_SUMMARY, entry).unwrap_or_default();
    let published = cap(&TAG_PUBLISHED, entry).unwrap_or_default();
    let year = published.chars().take(4).collect::<String>();
    let id_url = cap(&TAG_ID, entry).unwrap_or_default();
    let id = id_url.rsplit("/abs/").next().unwrap_or(&id_url).to_string();
    let doi = cap(&TAG_DOI, entry).unwrap_or_default();
    let authors: Vec<String> =
        TAG_NAME.captures_iter(entry).filter_map(|c| c.get(1).map(|m| clean(m.as_str()))).collect();
    if title.is_empty() {
        return None;
    }
    Some(Paper { source: "arxiv", id, doi, title, authors, year, abstract_, url: id_url })
}

fn cap(re: &regex::Regex, s: &str) -> Option<String> {
    re.captures(s).and_then(|c| c.get(1)).map(|m| clean(m.as_str()))
}

/// Decode a few XML entities and collapse whitespace.
fn clean(s: &str) -> String {
    let d = s
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'");
    WS.replace_all(&d, " ").trim().to_string()
}

// ── shared ───────────────────────────────────────────────────────────────────

fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("inkhaven-research/1.0 (https://crates.io/crates/inkhaven)")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| anyhow!("http client: {e}"))
}

fn strip_prefix_url(s: &str, prefix: &str) -> String {
    s.strip_prefix(prefix).unwrap_or(s).to_string()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max { s.to_string() } else { s.chars().take(max).collect::<String>() + "…" }
}

/// Render a paper as the chat body: title, authors · year, identifiers, abstract.
pub(super) fn render(p: &Paper) -> String {
    let mut s = p.title.clone();
    s.push('\n');
    let who = if p.authors.is_empty() { "unknown authors".to_string() } else { p.authors.join(", ") };
    s.push_str(&format!("{who}{}\n", if p.year.is_empty() { String::new() } else { format!(" · {}", p.year) }));
    let ident = if !p.doi.is_empty() { format!("doi:{}", p.doi) } else { format!("{}:{}", p.source, p.id) };
    s.push_str(&format!("{ident}\n"));
    if !p.abstract_.is_empty() {
        s.push_str(&format!("\n{}\n", truncate(&p.abstract_, 1200)));
    }
    s.push_str(&format!("\nSource: {} · {}", p.source, p.url));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openalex_parse_and_abstract() {
        let j = serde_json::json!({"results":[{
            "id":"https://openalex.org/W123","doi":"https://doi.org/10.1/x",
            "display_name":"Roman Aqueducts","publication_year":2012,
            "authorships":[{"author":{"display_name":"Jane Roe"}},{"author":{"display_name":"John Doe"}}],
            "abstract_inverted_index":{"Water":[0],"flows":[1],"downhill":[2]},
            "primary_location":{"landing_page_url":"https://example.org/a"}
        }]});
        let p = parse_openalex(&j).unwrap();
        assert_eq!(p.id, "W123");
        assert_eq!(p.doi, "10.1/x");
        assert_eq!(p.year, "2012");
        assert_eq!(p.authors, vec!["Jane Roe", "John Doe"]);
        assert_eq!(p.abstract_, "Water flows downhill");
        assert_eq!(p.cite_detail(), "doi:10.1/x");
        let b = p.to_bibentry();
        assert_eq!(b.key, "roe2012");
        assert_eq!(b.author, "Jane Roe and John Doe");
        assert_eq!(b.doi.as_deref(), Some("10.1/x"));
        assert!(b.is_valid());
    }

    #[test]
    fn arxiv_atom_parse() {
        let xml = r#"<feed><title>ArXiv Query</title>
          <entry>
            <id>http://arxiv.org/abs/1706.03762v5</id>
            <published>2017-06-12T17:57:34Z</published>
            <title>Attention Is All You Need</title>
            <summary>The dominant sequence models use recurrence.</summary>
            <author><name>Ashish Vaswani</name></author>
            <author><name>Noam Shazeer</name></author>
          </entry></feed>"#;
        let p = parse_arxiv_atom(xml).unwrap();
        assert_eq!(p.source, "arxiv");
        assert_eq!(p.id, "1706.03762v5");
        assert_eq!(p.year, "2017");
        assert_eq!(p.title, "Attention Is All You Need");
        assert_eq!(p.authors, vec!["Ashish Vaswani", "Noam Shazeer"]);
        assert!(p.abstract_.contains("recurrence"));
        assert_eq!(p.cite_detail(), "arxiv:1706.03762v5");
        assert_eq!(p.to_bibentry().key, "vaswani2017");
    }

    #[test]
    fn render_has_identifier_and_source() {
        let p = Paper {
            source: "arxiv", id: "1.2".into(), doi: String::new(), title: "T".into(),
            authors: vec!["A B".into()], year: "2020".into(), abstract_: "x".into(),
            url: "http://a".into(),
        };
        let r = render(&p);
        assert!(r.contains("arxiv:1.2"));
        assert!(r.contains("Source: arxiv"));
    }
}
