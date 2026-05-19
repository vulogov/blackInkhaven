use askama::Template;
use axum::{
    extract::{Query, State},
    response::Html,
};
use serde::Deserialize;
use serde_json::json;

use crate::{client::{fmt_ts, rpc, SESSION}, error::AppError, state::AppState};

#[derive(Deserialize, Default)]
pub struct Params {
    #[serde(default = "default_duration")]
    pub duration: String,
    #[serde(default)]
    pub q: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}
fn default_duration() -> String { "1h".to_owned() }
fn default_limit()    -> usize  { 50 }

/// Clamp a user-supplied limit to the supported [1, 1000] range.
fn clamp_limit(n: usize) -> usize { n.clamp(1, 1000) }

#[derive(Debug)]
pub struct HitRow {
    pub timestamp: String,
    pub key:       String,
    pub data:      String,
    pub score:     String,
}

fn to_rows(results: &serde_json::Value) -> Vec<HitRow> {
    results.as_array()
           .map(|arr| arr.iter().map(hit_to_row).collect())
           .unwrap_or_default()
}

fn hit_to_row(v: &serde_json::Value) -> HitRow {
    let ts   = v.get("timestamp").and_then(|x| x.as_u64()).unwrap_or(0);
    let data = v.get("data").map(|d| d.to_string()).unwrap_or_default();
    let score = v.get("_score").and_then(|x| x.as_f64())
                 .map(|f| format!("{f:.3}"))
                 .unwrap_or_else(|| "—".to_owned());
    HitRow {
        timestamp: fmt_ts(ts),
        key:       v.get("key").and_then(|x| x.as_str()).unwrap_or("—").to_owned(),
        data,
        score,
    }
}

// ── Full page ─────────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "telemetry.html")]
struct TelemetryPage { duration: String, q: String, limit: usize }

pub async fn page(Query(p): Query<Params>) -> Result<Html<String>, AppError> {
    Ok(Html(TelemetryPage {
        duration: p.duration,
        q:        p.q,
        limit:    clamp_limit(p.limit),
    }.render()?))
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
        href_base: "/telemetry".to_owned(),
    }.render()?))
}

// ── HTMX: vector search results fragment ─────────────────────────────────────

#[derive(Template)]
#[template(path = "partials/telemetry_rows.html")]
struct TelemetryRows { rows: Vec<HitRow>, duration: String, q: String }

pub async fn results(
    State(state): State<AppState>,
    Query(p): Query<Params>,
) -> Result<Html<String>, AppError> {
    let limit = clamp_limit(p.limit);

    if p.q.is_empty() {
        return Ok(Html(TelemetryRows { rows: vec![], duration: p.duration, q: p.q }.render()?));
    }

    let resp = rpc(&state, "v2/search.get", json!({
        "session":  SESSION,
        "query":    p.q,
        "duration": p.duration,
        "limit":    limit,
    })).await?;

    Ok(Html(TelemetryRows {
        rows:     to_rows(&resp["results"]),
        duration: p.duration,
        q:        p.q,
    }.render()?))
}
