use askama::Template;
use axum::{extract::{Query, State}, response::Html};
use serde::Deserialize;
use serde_json::json;

use crate::{client::{rpc, SESSION}, error::AppError, state::AppState};

// ── Query parameters ──────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct Params {
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub max_sentences: usize,
    #[serde(default = "default_min_word_len")]
    pub min_word_len: usize,
}
fn default_min_word_len() -> usize { 2 }

// ── Full page (shell) ─────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "primary_query_summary.html")]
struct PrimaryQuerySummaryPage {
    q:             String,
    max_sentences: usize,
    min_word_len:  usize,
}

pub async fn page(Query(p): Query<Params>) -> Result<Html<String>, AppError> {
    Ok(Html(PrimaryQuerySummaryPage {
        q:             p.q,
        max_sentences: p.max_sentences,
        min_word_len:  p.min_word_len,
    }.render()?))
}

// ── HTMX results fragment ─────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "partials/primary_query_summary_result.html")]
struct PrimaryQuerySummaryResult {
    q:             String,
    max_sentences: usize,
    summary:       String,
    has_summary:   bool,
    no_query:      bool,
}

pub async fn results(
    State(state): State<AppState>,
    Query(p): Query<Params>,
) -> Result<Html<String>, AppError> {
    if p.q.trim().is_empty() {
        return Ok(Html(PrimaryQuerySummaryResult {
            q: p.q, max_sentences: p.max_sentences,
            summary: String::new(), has_summary: false, no_query: true,
        }.render()?));
    }

    let resp = rpc(&state, "v2/summary_for_query", json!({
        "session":       SESSION,
        "query":         p.q,
        "max_sentences": p.max_sentences,
        "min_word_len":  p.min_word_len,
    })).await?;

    let summary = resp.get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();
    let has_summary = !summary.is_empty();

    Ok(Html(PrimaryQuerySummaryResult {
        q:             p.q,
        max_sentences: p.max_sentences,
        summary,
        has_summary,
        no_query: false,
    }.render()?))
}
