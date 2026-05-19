use askama::Template;
use axum::{extract::{Query, State}, response::Html};
use serde::Deserialize;
use serde_json::json;

use crate::{client::{rpc, SESSION}, error::AppError, state::AppState};

#[derive(Deserialize, Default)]
pub struct Params {
    #[serde(default)]
    pub q: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}
fn default_limit() -> usize { 10 }

#[derive(Debug)]
pub struct DocCard {
    pub id:       String,
    pub score:    String,
    pub name:     String,
    pub category: String,
    pub preview:  String,
    pub metadata_json: String,
}

fn to_cards(results: &serde_json::Value) -> Vec<DocCard> {
    results.as_array()
           .map(|arr| arr.iter().map(hit_to_card).collect())
           .unwrap_or_default()
}

fn hit_to_card(v: &serde_json::Value) -> DocCard {
    let meta = v.get("metadata").cloned().unwrap_or_default();
    let name = meta.get("name").and_then(|x| x.as_str())
        .or_else(|| meta.get("document_name").and_then(|x| x.as_str()))
        .unwrap_or("Untitled")
        .to_owned();
    let category = meta.get("category").and_then(|x| x.as_str())
        .unwrap_or("—")
        .to_owned();
    let content = v.get("document").and_then(|x| x.as_str()).unwrap_or("");
    let score = v.get("score").and_then(|x| x.as_f64())
                 .map(|f| format!("{f:.3}"))
                 .unwrap_or_else(|| "—".to_owned());
    DocCard {
        id:            v.get("id").and_then(|x| x.as_str()).unwrap_or("—").to_owned(),
        score,
        name,
        category,
        preview:       truncate(content, 280),
        metadata_json: serde_json::to_string_pretty(&meta).unwrap_or_default(),
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n { s.to_owned() } else { format!("{}…", &s[..n]) }
}

// ── Full page ─────────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "docs.html")]
struct DocsPage { q: String, limit: usize }

pub async fn page(Query(p): Query<Params>) -> Result<Html<String>, AppError> {
    Ok(Html(DocsPage { q: p.q, limit: p.limit }.render()?))
}

// ── HTMX results fragment ─────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "partials/doc_cards.html")]
struct DocCards { cards: Vec<DocCard>, q: String }

pub async fn results(
    State(state): State<AppState>,
    Query(p): Query<Params>,
) -> Result<Html<String>, AppError> {
    if p.q.is_empty() {
        return Ok(Html(DocCards { cards: vec![], q: p.q }.render()?));
    }

    let resp = rpc(&state, "v2/doc.search", json!({
        "session": SESSION,
        "query":   p.q,
        "limit":   p.limit,
    })).await?;

    Ok(Html(DocCards { cards: to_cards(&resp["results"]), q: p.q }.render()?))
}
