use askama::Template;
use axum::{extract::{Query, State}, response::Html};
use serde::Deserialize;
use serde_json::json;

use crate::{client::{rpc, SESSION}, error::AppError, state::AppState};

#[derive(Deserialize, Default)]
pub struct Params {
    #[serde(default)]
    pub q: String,
    #[serde(default = "default_duration")]
    pub duration: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}
fn default_duration() -> String { "1h".to_owned() }
fn default_limit() -> usize { 20 }

#[derive(Debug)]
pub struct SignalCard {
    pub id:             String,
    pub name:           String,
    pub severity:       String,
    pub severity_class: String,
    pub timestamp:      String,
    pub score:          String,
    pub metadata_json:  String,
}

fn severity_css(s: &str) -> &'static str {
    match s.to_lowercase().as_str() {
        "critical" | "error" => "text-red-400",
        "warning"  | "warn"  => "text-yellow-400",
        "info"               => "text-blue-300",
        _                    => "text-slate-400",
    }
}

fn fmt_ts(ts: u64) -> String {
    if ts == 0 { return "—".to_owned(); }
    format!(
        "{}",
        chrono::DateTime::from_timestamp(ts as i64, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            .unwrap_or_else(|| ts.to_string())
    )
}

fn meta_to_card(id: &str, meta: &serde_json::Value, score: Option<f64>) -> SignalCard {
    let name     = meta.get("name").and_then(|v| v.as_str()).unwrap_or("—").to_owned();
    let severity = meta.get("severity").and_then(|v| v.as_str()).unwrap_or("info").to_owned();
    let ts       = meta.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
    SignalCard {
        id:             id.to_owned(),
        severity_class: severity_css(&severity).to_owned(),
        severity,
        name,
        timestamp:      fmt_ts(ts),
        score:          score.map(|f| format!("{f:.3}")).unwrap_or_else(|| "—".to_owned()),
        metadata_json:  serde_json::to_string_pretty(meta).unwrap_or_default(),
    }
}

fn to_recent_cards(resp: &serde_json::Value) -> Vec<SignalCard> {
    resp.get("signals")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(|item| {
            let id   = item.get("id").and_then(|v| v.as_str()).unwrap_or("—");
            let meta = item.get("metadata").cloned().unwrap_or_default();
            meta_to_card(id, &meta, None)
        }).collect())
        .unwrap_or_default()
}

fn to_query_cards(resp: &serde_json::Value) -> Vec<SignalCard> {
    resp.get("results")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(|item| {
            let id    = item.get("id").and_then(|v| v.as_str()).unwrap_or("—");
            let meta  = item.get("metadata").cloned().unwrap_or_default();
            let score = item.get("score").and_then(|v| v.as_f64());
            meta_to_card(id, &meta, score)
        }).collect())
        .unwrap_or_default()
}

// ── Full page ─────────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "signals.html")]
struct SignalsPage { q: String, duration: String, limit: usize }

pub async fn page(Query(p): Query<Params>) -> Result<Html<String>, AppError> {
    Ok(Html(SignalsPage { q: p.q, duration: p.duration, limit: p.limit }.render()?))
}

// ── HTMX results fragment ─────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "partials/signal_cards.html")]
struct SignalCards {
    cards:     Vec<SignalCard>,
    q:         String,
    duration:  String,
    is_recent: bool,
}

pub async fn results(
    State(state): State<AppState>,
    Query(p): Query<Params>,
) -> Result<Html<String>, AppError> {
    let (cards, is_recent) = if p.q.is_empty() {
        let resp = rpc(&state, "v2/signals", json!({
            "session":  SESSION,
            "duration": p.duration,
        })).await?;
        (to_recent_cards(&resp), true)
    } else {
        let resp = rpc(&state, "v2/signals_query", json!({
            "session": SESSION,
            "query":   p.q,
            "limit":   p.limit,
        })).await?;
        (to_query_cards(&resp), false)
    };

    Ok(Html(SignalCards {
        cards,
        q:         p.q,
        duration:  p.duration,
        is_recent,
    }.render()?))
}
