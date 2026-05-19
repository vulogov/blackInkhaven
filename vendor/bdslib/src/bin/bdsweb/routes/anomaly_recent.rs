use askama::Template;
use axum::{extract::{Query, State}, response::Html};
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};

use crate::{client::{rpc, SESSION}, error::AppError, state::AppState};

// ── Query parameters ──────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct Params {
    #[serde(default = "default_duration")]
    pub duration: String,
    #[serde(default = "default_n")]
    pub n: usize,
    #[serde(default = "default_min_word_len")]
    pub min_word_len: usize,
    #[serde(default = "default_anomaly_threshold")]
    pub anomaly_threshold: f32,
    #[serde(default = "default_max_anomalies")]
    pub max_anomalies: usize,
}
fn default_duration()          -> String { "1h".to_owned() }
fn default_n()                 -> usize { 2 }
fn default_min_word_len()      -> usize { 2 }
fn default_anomaly_threshold() -> f32   { 0.7 }
fn default_max_anomalies()     -> usize { 20 }

// ── Page shell ────────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "anomaly_recent.html")]
struct AnomalyPage {
    duration:          String,
    n:                 usize,
    min_word_len:      usize,
    anomaly_threshold: f32,
    max_anomalies:     usize,
}

pub async fn page(Query(p): Query<Params>) -> Result<Html<String>, AppError> {
    Ok(Html(AnomalyPage {
        duration:          p.duration,
        n:                 p.n,
        min_word_len:      p.min_word_len,
        anomaly_threshold: p.anomaly_threshold,
        max_anomalies:     p.max_anomalies,
    }.render()?))
}

// ── HTMX results fragment ─────────────────────────────────────────────────────

#[derive(Debug)]
pub struct AnomalyRow {
    pub idx:          u64,
    pub rarity:       f64,
    pub text:         String,
    pub novel_ngrams: Vec<String>,
}

#[derive(Template)]
#[template(path = "partials/anomaly_recent_result.html")]
struct AnomalyResult {
    duration:        String,
    n_logs:          u64,
    n:               u64,
    n_unique_ngrams: u64,
    anomaly_threshold: f64,
    n_anomalies:     u64,
    mean_rarity:     f64,
    has_anomalies:   bool,
    anomalies:       Vec<AnomalyRow>,
}

pub async fn results(
    State(state): State<AppState>,
    Query(p): Query<Params>,
) -> Result<Html<String>, AppError> {
    let resp = rpc(&state, "v2/anomaly.recent", json!({
        "session":           SESSION,
        "duration":          p.duration.clone(),
        "n":                 p.n,
        "min_word_len":      p.min_word_len,
        "anomaly_threshold": p.anomaly_threshold,
        "max_anomalies":     p.max_anomalies,
    })).await?;

    let n_logs           = resp.get("n_logs").and_then(JsonValue::as_u64).unwrap_or(0);
    let n_eff            = resp.get("n").and_then(JsonValue::as_u64).unwrap_or(p.n as u64);
    let n_unique_ngrams  = resp.get("n_unique_ngrams").and_then(JsonValue::as_u64).unwrap_or(0);
    let n_anomalies      = resp.get("n_anomalies").and_then(JsonValue::as_u64).unwrap_or(0);
    let mean_rarity      = resp.get("mean_rarity").and_then(JsonValue::as_f64).unwrap_or(0.0);
    let anomaly_threshold = resp.get("anomaly_threshold").and_then(JsonValue::as_f64)
        .unwrap_or(p.anomaly_threshold as f64);

    let anomalies: Vec<AnomalyRow> = resp.get("anomalies")
        .and_then(JsonValue::as_array)
        .map(|arr| arr.iter().map(|a| {
            let novel: Vec<String> = a.get("novel_ngrams")
                .and_then(JsonValue::as_array)
                .map(|ngs| ngs.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect())
                .unwrap_or_default();
            AnomalyRow {
                idx:          a.get("idx").and_then(JsonValue::as_u64).unwrap_or(0),
                rarity:       a.get("rarity").and_then(JsonValue::as_f64).unwrap_or(0.0),
                text:         a.get("text").and_then(|v| v.as_str()).unwrap_or("").to_owned(),
                novel_ngrams: novel,
            }
        }).collect())
        .unwrap_or_default();

    let has_anomalies = !anomalies.is_empty();

    Ok(Html(AnomalyResult {
        duration:          p.duration,
        n_logs,
        n:                 n_eff,
        n_unique_ngrams,
        anomaly_threshold,
        n_anomalies,
        mean_rarity,
        has_anomalies,
        anomalies,
    }.render()?))
}
