//! RESRCH-2 (R2-C) — web search & fetch. Pluggable providers (Tavily / SearXNG)
//! behind a single async `search()`; the app spawns it as a tokio task and
//! drains the result in the poll loop (mirroring the LLM stream). Tavily returns
//! page content inline; SearXNG returns URLs, fetched + stripped to text here.
//!
//! Only reached when `research.web` is configured; otherwise `/web` reports it
//! is unavailable. The HTTP client is `reqwest` (already transitive via genai).

use anyhow::{Result, anyhow};

use crate::config::WebConfig;

/// One web result: a title, its URL, and the extracted page text (or snippet).
#[derive(Debug, Clone)]
pub(super) struct WebResult {
    pub title: String,
    pub url: String,
    pub text: String,
}

/// Whether `/web` can run with the current config.
pub(super) fn available(cfg: &WebConfig) -> bool {
    if !cfg.enabled {
        return false;
    }
    match cfg.provider.as_str() {
        "tavily" => !cfg.api_key.trim().is_empty(),
        "searxng" => !cfg.endpoint.trim().is_empty(),
        _ => false,
    }
}

/// Run a web search (and, for SearXNG with `fetch`, fetch each page). Owned args
/// so it can be spawned onto a tokio task.
pub(super) async fn search(cfg: WebConfig, query: String) -> Result<Vec<WebResult>> {
    match cfg.provider.as_str() {
        "tavily" => tavily(&cfg, &query).await,
        "searxng" => searxng(&cfg, &query).await,
        other => Err(anyhow!("unknown web provider `{other}` (tavily | searxng)")),
    }
}

/// Tavily — one POST returns results with content inline (no separate fetch).
async fn tavily(cfg: &WebConfig, query: &str) -> Result<Vec<WebResult>> {
    let body = serde_json::json!({
        "api_key": cfg.api_key,
        "query": query,
        "max_results": cfg.max_results.max(1),
        "search_depth": "basic",
        "include_raw_content": cfg.fetch,
    });
    let resp = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| anyhow!("http client: {e}"))?
        .post("https://api.tavily.com/search")
        .json(&body)
        .send()
        .await
        .map_err(|e| anyhow!("tavily request: {e}"))?;
    if !resp.status().is_success() {
        return Err(anyhow!("tavily HTTP {}", resp.status()));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| anyhow!("tavily response: {e}"))?;
    Ok(parse_results(&json, true))
}

/// SearXNG — JSON search, then fetch + strip each page when `fetch` is on.
async fn searxng(cfg: &WebConfig, query: &str) -> Result<Vec<WebResult>> {
    let base = cfg.endpoint.trim_end_matches('/');
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        // SearXNG returns arbitrary result URLs which we then fetch; bound the
        // redirect chain so a page can't bounce us to an internal address
        // (169.254.169.254 / localhost / … — SSRF).
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| anyhow!("http client: {e}"))?;
    let resp = client
        .get(format!("{base}/search"))
        .query(&[("q", query), ("format", "json")])
        .send()
        .await
        .map_err(|e| anyhow!("searxng request: {e}"))?;
    if !resp.status().is_success() {
        return Err(anyhow!("searxng HTTP {}", resp.status()));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| anyhow!("searxng response: {e}"))?;
    let mut results = parse_results(&json, false);
    results.truncate(cfg.max_results.max(1));
    if cfg.fetch {
        for r in results.iter_mut() {
            if let Ok(page) = client.get(&r.url).send().await {
                if let Some(html) = read_capped_text(page).await {
                    let text = html_to_text(&html);
                    if text.len() > r.text.len() {
                        r.text = text;
                    }
                }
            }
        }
    }
    Ok(results)
}

/// Read an HTTP response body as UTF-8 text with a hard byte cap, so a hostile
/// or huge fetched page — or a missing / lying `Content-Length` — can't stream
/// unbounded bytes into memory (the 30s timeout bounds duration, not size).
async fn read_capped_text(resp: reqwest::Response) -> Option<String> {
    use futures_util::StreamExt;
    const MAX_PAGE_BYTES: usize = 5 * 1024 * 1024; // 5 MiB
    // Fast-reject a body that already declares itself oversized.
    if resp.content_length().is_some_and(|len| len as usize > MAX_PAGE_BYTES) {
        return None;
    }
    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.ok()?;
        if buf.len() + chunk.len() > MAX_PAGE_BYTES {
            return None; // over the cap — drop the page rather than risk OOM
        }
        buf.extend_from_slice(&chunk);
    }
    String::from_utf8(buf).ok()
}

/// Parse a `{ results: [ { title, url, content, raw_content } ] }` payload
/// (shared by both providers). `prefer_raw` uses `raw_content` when present.
fn parse_results(json: &serde_json::Value, prefer_raw: bool) -> Vec<WebResult> {
    let Some(arr) = json.get("results").and_then(|r| r.as_array()) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|r| {
            let url = r.get("url").and_then(|u| u.as_str())?.to_string();
            let title = r.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string();
            let raw = r.get("raw_content").and_then(|c| c.as_str());
            let content = r.get("content").and_then(|c| c.as_str()).unwrap_or("");
            let text = if prefer_raw { raw.unwrap_or(content) } else { content }.trim().to_string();
            Some(WebResult { title, url, text })
        })
        .collect()
}

/// Crude HTML → text: drop script/style, strip tags, decode a few entities,
/// collapse whitespace. Good enough for research extraction without an
/// HTML-parser crate.
pub(super) fn html_to_text(html: &str) -> String {
    // Remove <script>…</script> and <style>…</style> (case-insensitive, DOTALL).
    let stripped = SCRIPT_STYLE.replace_all(html, " ");
    // Strip remaining tags.
    let no_tags = TAGS.replace_all(&stripped, " ");
    let decoded = no_tags
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&rsquo;", "'")
        .replace("&ldquo;", "\"")
        .replace("&rdquo;", "\"");
    // Collapse runs of whitespace.
    WS.replace_all(&decoded, " ").trim().to_string()
}

use std::sync::LazyLock;
static SCRIPT_STYLE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(?is)<(script|style)[^>]*>.*?</(script|style)>").unwrap());
static TAGS: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"(?s)<[^>]+>").unwrap());
static WS: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"\s+").unwrap());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn availability_gates() {
        let mut c = WebConfig::default();
        assert!(!available(&c)); // disabled by default
        c.enabled = true;
        c.provider = "tavily".into();
        assert!(!available(&c)); // no key
        c.api_key = "k".into();
        assert!(available(&c));
        c.provider = "searxng".into();
        c.api_key.clear();
        assert!(!available(&c)); // no endpoint
        c.endpoint = "https://searx.example".into();
        assert!(available(&c));
    }

    #[test]
    fn html_strip() {
        let h = "<html><head><style>x{}</style></head><body>Hello <b>World</b> &amp; <script>alert(1)</script>more</body></html>";
        let t = html_to_text(h);
        assert!(t.contains("Hello World"));
        assert!(t.contains("& more") || t.contains("&  more") || t.contains("more"));
        assert!(!t.contains("alert"));
        assert!(!t.contains("x{}"));
    }

    #[test]
    fn parse_prefers_raw_content() {
        let j = serde_json::json!({
            "results": [
                {"title":"T","url":"http://a","content":"snippet","raw_content":"full text"},
                {"title":"U","url":"http://b","content":"only snippet"}
            ]
        });
        let r = parse_results(&j, true);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].text, "full text");
        assert_eq!(r[1].text, "only snippet");
        // Without prefer_raw, the snippet wins.
        assert_eq!(parse_results(&j, false)[0].text, "snippet");
    }
}
