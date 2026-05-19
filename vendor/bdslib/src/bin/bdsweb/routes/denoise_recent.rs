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
    #[serde(default = "default_noise_threshold")]
    pub noise_threshold: f32,
    #[serde(default = "default_max_kept")]
    pub max_kept: usize,
    #[serde(default = "default_max_removed")]
    pub max_removed: usize,
}
fn default_duration()        -> String { "1h".to_owned() }
fn default_n()               -> usize { 2 }
fn default_min_word_len()    -> usize { 2 }
fn default_noise_threshold() -> f32   { 0.85 }
fn default_max_kept()        -> usize { 100 }
fn default_max_removed()     -> usize { 100 }

// ── Page shell ────────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "denoise_recent.html")]
struct DenoisePage {
    duration:        String,
    n:               usize,
    min_word_len:    usize,
    noise_threshold: f32,
    max_kept:        usize,
    max_removed:     usize,
}

pub async fn page(Query(p): Query<Params>) -> Result<Html<String>, AppError> {
    Ok(Html(DenoisePage {
        duration:        p.duration,
        n:               p.n,
        min_word_len:    p.min_word_len,
        noise_threshold: p.noise_threshold,
        max_kept:        p.max_kept,
        max_removed:     p.max_removed,
    }.render()?))
}

// ── HTMX results fragment ─────────────────────────────────────────────────────

#[derive(Debug)]
pub struct DenoiseRow {
    pub idx:        u64,
    pub commonness: f64,
    pub text:       String,
}

#[derive(Template)]
#[template(path = "partials/denoise_recent_result.html")]
struct DenoiseResult {
    duration:        String,
    n_logs:          u64,
    n:               u64,
    n_unique_ngrams: u64,
    noise_threshold: f64,
    n_kept:          u64,
    n_removed:       u64,
    has_kept:        bool,
    has_removed:     bool,
    kept:            Vec<DenoiseRow>,
    removed:         Vec<DenoiseRow>,
}

fn rows(arr: Option<&Vec<JsonValue>>) -> Vec<DenoiseRow> {
    arr.map(|items| items.iter().map(|v| DenoiseRow {
        idx:        v.get("idx").and_then(JsonValue::as_u64).unwrap_or(0),
        commonness: v.get("commonness").and_then(JsonValue::as_f64).unwrap_or(0.0),
        text:       v.get("text").and_then(|x| x.as_str()).unwrap_or("").to_owned(),
    }).collect()).unwrap_or_default()
}

pub async fn results(
    State(state): State<AppState>,
    Query(p): Query<Params>,
) -> Result<Html<String>, AppError> {
    let resp = rpc(&state, "v2/denoise.recent", json!({
        "session":         SESSION,
        "duration":        p.duration.clone(),
        "n":               p.n,
        "min_word_len":    p.min_word_len,
        "noise_threshold": p.noise_threshold,
        "max_kept":        p.max_kept,
        "max_removed":     p.max_removed,
    })).await?;

    let n_logs          = resp.get("n_logs").and_then(JsonValue::as_u64).unwrap_or(0);
    let n_eff           = resp.get("n").and_then(JsonValue::as_u64).unwrap_or(p.n as u64);
    let n_unique_ngrams = resp.get("n_unique_ngrams").and_then(JsonValue::as_u64).unwrap_or(0);
    let n_kept          = resp.get("n_kept").and_then(JsonValue::as_u64).unwrap_or(0);
    let n_removed       = resp.get("n_removed").and_then(JsonValue::as_u64).unwrap_or(0);
    let noise_threshold = resp.get("noise_threshold").and_then(JsonValue::as_f64)
        .unwrap_or(p.noise_threshold as f64);

    let kept    = rows(resp.get("kept").and_then(JsonValue::as_array));
    let removed = rows(resp.get("removed").and_then(JsonValue::as_array));

    let has_kept    = !kept.is_empty();
    let has_removed = !removed.is_empty();

    Ok(Html(DenoiseResult {
        duration:        p.duration,
        n_logs,
        n:               n_eff,
        n_unique_ngrams,
        noise_threshold,
        n_kept,
        n_removed,
        has_kept,
        has_removed,
        kept,
        removed,
    }.render()?))
}
