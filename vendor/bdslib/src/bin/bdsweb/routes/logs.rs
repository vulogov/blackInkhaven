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
pub struct LogRow {
    pub timestamp:         String,
    pub key:               String,
    pub message:           String,
    pub score:             String,
    pub secondaries_count: usize,
    pub secondaries_json:  String,
}

fn to_rows(results: &serde_json::Value) -> Vec<LogRow> {
    results.as_array()
           .map(|arr| arr.iter().map(hit_to_row).collect())
           .unwrap_or_default()
}

fn hit_to_row(v: &serde_json::Value) -> LogRow {
    let ts   = v.get("timestamp").and_then(|x| x.as_u64()).unwrap_or(0);
    let data = v.get("data");
    let message = data
        .and_then(|d| d.as_str()).map(str::to_owned)
        .or_else(|| data.and_then(|d| d.get("message")).and_then(|m| m.as_str()).map(str::to_owned))
        .or_else(|| data.map(|d| d.to_string()))
        .unwrap_or_default();
    let score = v.get("_score").and_then(|x| x.as_f64())
                 .map(|f| format!("{f:.3}"))
                 .unwrap_or_else(|| "—".to_owned());
    let secs = v.get("secondaries").and_then(|x| x.as_array());
    let secondaries_count = secs.map(|a| a.len()).unwrap_or(0);
    let secondaries_json = secs
        .map(|a| serde_json::to_string(a).unwrap_or_else(|_| "[]".to_owned()))
        .unwrap_or_else(|| "[]".to_owned());
    LogRow {
        timestamp: fmt_ts(ts),
        key:       v.get("key").and_then(|x| x.as_str()).unwrap_or("—").to_owned(),
        message:   truncate(&message, 160),
        score,
        secondaries_count,
        secondaries_json,
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n { s.to_owned() } else { format!("{}…", &s[..n]) }
}

// ── Full page ─────────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "logs.html")]
struct LogsPage { duration: String, q: String }

pub async fn page(Query(p): Query<Params>) -> Result<Html<String>, AppError> {
    Ok(Html(LogsPage { duration: p.duration, q: p.q }.render()?))
}

// ── HTMX: key cloud ──────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "partials/key_cloud.html")]
struct KeyCloud {
    keys:      Vec<String>,
    duration:  String,
    href_base: String,
}

pub async fn keys(
    State(state): State<AppState>,
    Query(p): Query<Params>,
) -> Result<Html<String>, AppError> {
    let resp = rpc(&state, "v2/keys.all", json!({
        "session":  SESSION,
        "duration": p.duration,
        "key":      "*",
    })).await.unwrap_or_default();

    let keys = resp.get("keys")
        .and_then(|x| x.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
        .unwrap_or_default();

    Ok(Html(KeyCloud {
        keys,
        duration:  p.duration,
        href_base: "/logs".to_owned(),
    }.render()?))
}

// ── HTMX: topics cloud ───────────────────────────────────────────────────────

pub struct TopicRow {
    pub key:      String,
    pub keywords: Vec<String>,
}

#[derive(Template)]
#[template(path = "partials/topics_cloud.html")]
struct TopicsCloud { topics: Vec<TopicRow>, duration: String }

pub async fn topics(
    State(state): State<AppState>,
    Query(p): Query<Params>,
) -> Result<Html<String>, AppError> {
    let resp = rpc(&state, "v2/topics.all", json!({
        "session":  SESSION,
        "duration": p.duration,
    })).await;

    let topics = match resp {
        Err(_) => vec![],
        Ok(v) => {
            v.get("topics")
             .and_then(|x| x.as_array())
             .map(|arr| arr.iter().map(|t| {
                 let key = t.get("key").and_then(|x| x.as_str()).unwrap_or("").to_owned();
                 let kw_str = t.get("keywords").and_then(|x| x.as_str()).unwrap_or("");
                 let keywords = kw_str.split(',')
                     .map(|s| s.trim().to_owned())
                     .filter(|s| !s.is_empty())
                     .collect();
                 TopicRow { key, keywords }
             }).collect())
             .unwrap_or_default()
        }
    };

    Ok(Html(TopicsCloud { topics, duration: p.duration }.render()?))
}

// ── HTMX: vector search results fragment ─────────────────────────────────────

#[derive(Template)]
#[template(path = "partials/log_rows.html")]
struct LogRows { rows: Vec<LogRow>, duration: String, q: String }

pub async fn results(
    State(state): State<AppState>,
    Query(p): Query<Params>,
) -> Result<Html<String>, AppError> {
    if p.q.is_empty() {
        return Ok(Html(LogRows { rows: vec![], duration: p.duration, q: p.q }.render()?));
    }

    let resp = rpc(&state, "v2/search.get", json!({
        "session":  SESSION,
        "query":    p.q,
        "duration": p.duration,
        "limit":    50,
    })).await?;

    Ok(Html(LogRows {
        rows:     to_rows(&resp["results"]),
        duration: p.duration,
        q:        p.q,
    }.render()?))
}
