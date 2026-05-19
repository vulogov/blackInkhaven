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
    #[serde(default = "default_n_concepts")]
    pub n_concepts: usize,
}
fn default_min_word_len() -> usize { 2 }
fn default_n_concepts()   -> usize { 3 }

// ── Full page (shell) ─────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "primary_lsa_query_summary.html")]
struct PrimaryLsaQuerySummaryPage {
    q:             String,
    max_sentences: usize,
    min_word_len:  usize,
    n_concepts:    usize,
}

pub async fn page(Query(p): Query<Params>) -> Result<Html<String>, AppError> {
    Ok(Html(PrimaryLsaQuerySummaryPage {
        q:             p.q,
        max_sentences: p.max_sentences,
        min_word_len:  p.min_word_len,
        n_concepts:    p.n_concepts,
    }.render()?))
}

// ── HTMX results fragment ─────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "partials/primary_lsa_query_summary_result.html")]
struct PrimaryLsaQuerySummaryResult {
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
        return Ok(Html(PrimaryLsaQuerySummaryResult {
            q: p.q, max_sentences: p.max_sentences,
            summary: String::new(), has_summary: false, no_query: true,
        }.render()?));
    }

    let resp = rpc(&state, "v2/summary_lsa_for_query", json!({
        "session":       SESSION,
        "query":         p.q,
        "max_sentences": p.max_sentences,
        "min_word_len":  p.min_word_len,
        "n_concepts":    p.n_concepts,
    })).await?;

    let summary = resp.get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();
    let has_summary = !summary.is_empty();

    Ok(Html(PrimaryLsaQuerySummaryResult {
        q:             p.q,
        max_sentences: p.max_sentences,
        summary,
        has_summary,
        no_query: false,
    }.render()?))
}
