use askama::Template;
use axum::{extract::{Query, State}, response::Html};
use serde::Deserialize;
use serde_json::json;

use crate::{client::{rpc, SESSION}, error::AppError, state::AppState};

// ── Query parameters ──────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct Params {
    #[serde(default = "default_duration")]
    pub duration: String,
    #[serde(default)]
    pub max_sentences: usize,
    #[serde(default = "default_min_word_len")]
    pub min_word_len: usize,
    #[serde(default = "default_n_concepts")]
    pub n_concepts: usize,
}
fn default_duration()    -> String { "1h".to_owned() }
fn default_min_word_len() -> usize { 2 }
fn default_n_concepts()   -> usize { 3 }

// ── Full page (shell) ─────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "primary_lsa_summary.html")]
struct PrimaryLsaSummaryPage {
    duration:      String,
    max_sentences: usize,
    min_word_len:  usize,
    n_concepts:    usize,
}

pub async fn page(Query(p): Query<Params>) -> Result<Html<String>, AppError> {
    Ok(Html(PrimaryLsaSummaryPage {
        duration:      p.duration,
        max_sentences: p.max_sentences,
        min_word_len:  p.min_word_len,
        n_concepts:    p.n_concepts,
    }.render()?))
}

// ── HTMX results fragment ─────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "partials/primary_lsa_summary_result.html")]
struct PrimaryLsaSummaryResult {
    duration:      String,
    max_sentences: usize,
    summary:       String,
    has_summary:   bool,
}

pub async fn results(
    State(state): State<AppState>,
    Query(p): Query<Params>,
) -> Result<Html<String>, AppError> {
    let resp = rpc(&state, "v2/summary_lsa_for_recent", json!({
        "session":       SESSION,
        "duration":      p.duration,
        "max_sentences": p.max_sentences,
        "min_word_len":  p.min_word_len,
        "n_concepts":    p.n_concepts,
    })).await?;

    let summary = resp.get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();
    let has_summary = !summary.is_empty();

    Ok(Html(PrimaryLsaSummaryResult {
        duration:      p.duration,
        max_sentences: p.max_sentences,
        summary,
        has_summary,
    }.render()?))
}
