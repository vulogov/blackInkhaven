use askama::Template;
use axum::{extract::{Query, State}, response::Html};
use serde::Deserialize;
use serde_json::json;

use crate::{client::{fmt_ts, rpc, SESSION}, error::AppError, state::AppState};

// ── Query parameters ──────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct Params {
    #[serde(default = "default_duration")]
    pub duration: String,
    #[serde(default)]
    pub q: String,
}
fn default_duration() -> String { "1h".to_owned() }

// ── Row data type ─────────────────────────────────────────────────────────────

pub struct TplRow {
    #[allow(dead_code)]
    pub id:        String,
    pub name:      String,
    pub body:      String,
    pub timestamp: String,
    pub score:     String,
}

fn row_from_recent(v: &serde_json::Value) -> TplRow {
    let id   = v.get("id").and_then(|x| x.as_str()).unwrap_or("—").to_owned();
    let meta = v.get("metadata").cloned().unwrap_or_default();
    let name = meta.get("name").and_then(|x| x.as_str()).unwrap_or("—").to_owned();
    let ts   = meta.get("timestamp").and_then(|x| x.as_u64()).unwrap_or(0);
    let body = v.get("body").and_then(|x| x.as_str()).unwrap_or("—").to_owned();
    TplRow { id, name, body, timestamp: fmt_ts(ts), score: String::new() }
}

fn row_from_search(v: &serde_json::Value) -> TplRow {
    let id   = v.get("id").and_then(|x| x.as_str()).unwrap_or("—").to_owned();
    let meta = v.get("metadata").cloned().unwrap_or_default();
    let name = meta.get("name").and_then(|x| x.as_str()).unwrap_or("—").to_owned();
    let ts   = meta.get("timestamp").and_then(|x| x.as_u64()).unwrap_or(0);
    let body = v.get("document").and_then(|x| x.as_str()).unwrap_or("—").to_owned();
    let score = v.get("score").and_then(|x| x.as_f64())
                  .map(|f| format!("{f:.3}"))
                  .unwrap_or_else(|| "—".to_owned());
    TplRow { id, name, body, timestamp: fmt_ts(ts), score }
}

// ── Full page ─────────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "templates.html")]
struct TemplatesPage {
    duration: String,
    q:        String,
}

pub async fn page(Query(p): Query<Params>) -> Result<Html<String>, AppError> {
    Ok(Html(TemplatesPage { duration: p.duration, q: p.q }.render()?))
}

// ── HTMX results fragment ─────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "partials/template_rows.html")]
struct TemplateRows {
    rows:      Vec<TplRow>,
    duration:  String,
    q:         String,
    searching: bool,
}

pub async fn results(
    State(state): State<AppState>,
    Query(p): Query<Params>,
) -> Result<Html<String>, AppError> {
    if p.q.is_empty() {
        // Browse mode: show recently observed templates via FrequencyTracking
        let resp = rpc(&state, "v2/tpl.templates_recent", json!({
            "session":  SESSION,
            "duration": p.duration,
        }))
        .await
        .unwrap_or_default();

        let rows = resp
            .get("templates")
            .and_then(|x| x.as_array())
            .map(|a| a.iter().map(row_from_recent).collect())
            .unwrap_or_default();

        Ok(Html(TemplateRows { rows, duration: p.duration, q: p.q, searching: false }.render()?))
    } else {
        // Search mode: semantic vector search across tpl store
        let resp = rpc(&state, "v2/tpl.search", json!({
            "session":  SESSION,
            "duration": p.duration,
            "query":    p.q,
            "limit":    50,
        }))
        .await
        .unwrap_or_default();

        let rows = resp
            .get("results")
            .and_then(|x| x.as_array())
            .map(|a| a.iter().map(row_from_search).collect())
            .unwrap_or_default();

        Ok(Html(TemplateRows { rows, duration: p.duration, q: p.q, searching: true }.render()?))
    }
}
