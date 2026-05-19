use askama::Template;
use axum::{extract::{Query, State}, response::Html};
use serde::Deserialize;
use serde_json::json;

use crate::{client::{fmt_ts, rpc, SESSION}, error::AppError, state::AppState};

#[derive(Deserialize, Default)]
pub struct Params {
    #[serde(default = "default_duration")]
    pub duration: String,
    #[serde(default)]
    pub q: String,
}
fn default_duration() -> String { "1h".to_owned() }

#[derive(Debug)]
pub struct ObsHit {
    pub timestamp: String,
    pub key:       String,
    pub data:      String,
    pub score:     String,
}

#[derive(Debug)]
pub struct DocHit {
    pub name:     String,
    pub category: String,
    pub score:    String,
    pub preview:  String,
}

fn to_obs(arr: &serde_json::Value) -> Vec<ObsHit> {
    arr.as_array().map(|a| a.iter().map(|v| {
        let ts = v.get("timestamp").and_then(|x| x.as_u64()).unwrap_or(0);
        let data = v.get("data").map(|d| d.to_string()).unwrap_or_default();
        ObsHit {
            timestamp: fmt_ts(ts),
            key:       v.get("key").and_then(|x| x.as_str()).unwrap_or("—").to_owned(),
            data:      truncate(&data, 100),
            score:     v.get("_score").and_then(|x| x.as_f64())
                        .map(|f| format!("{f:.3}")).unwrap_or_else(|| "—".to_owned()),
        }
    }).collect()).unwrap_or_default()
}

fn to_docs(arr: &serde_json::Value) -> Vec<DocHit> {
    arr.as_array().map(|a| a.iter().map(|v| {
        let meta = v.get("metadata").cloned().unwrap_or_default();
        let name = meta.get("name").or_else(|| meta.get("document_name"))
                       .and_then(|x| x.as_str()).unwrap_or("Untitled").to_owned();
        let content = v.get("document").and_then(|x| x.as_str()).unwrap_or("");
        DocHit {
            name,
            category: meta.get("category").and_then(|x| x.as_str()).unwrap_or("—").to_owned(),
            score:    v.get("score").and_then(|x| x.as_f64())
                       .map(|f| format!("{f:.3}")).unwrap_or_else(|| "—".to_owned()),
            preview:  truncate(content, 200),
        }
    }).collect()).unwrap_or_default()
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n { s.to_owned() } else { format!("{}…", &s[..n]) }
}

// ── Full page ─────────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "search.html")]
struct SearchPage { duration: String, q: String }

pub async fn page(Query(p): Query<Params>) -> Result<Html<String>, AppError> {
    Ok(Html(SearchPage { duration: p.duration, q: p.q }.render()?))
}

// ── HTMX results fragment ─────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "partials/search_panels.html")]
struct SearchPanels {
    obs:      Vec<ObsHit>,
    docs:     Vec<DocHit>,
    q:        String,
    duration: String,
}

pub async fn results(
    State(state): State<AppState>,
    Query(p): Query<Params>,
) -> Result<Html<String>, AppError> {
    if p.q.is_empty() {
        return Ok(Html(SearchPanels { obs: vec![], docs: vec![], q: p.q, duration: p.duration }.render()?));
    }

    let resp = rpc(&state, "v2/aggregationsearch", json!({
        "session":  SESSION,
        "query":    p.q,
        "duration": p.duration,
    })).await?;

    Ok(Html(SearchPanels {
        obs:      to_obs(&resp["observability"]),
        docs:     to_docs(&resp["documents"]),
        q:        p.q,
        duration: p.duration,
    }.render()?))
}
