use askama::Template;
use axum::{extract::{Query, State}, response::Html};
use serde::Deserialize;
use serde_json::json;

use crate::{client::{rpc, SESSION}, error::AppError, state::AppState};
use super::rca::{extract_rca, CausalRow, ClusterCard, RcaSummary};

// ── Query parameters ──────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct Params {
    #[serde(default = "default_duration")]
    pub duration: String,
    #[serde(default)]
    pub failure_body: String,
    #[serde(default = "default_bucket_secs")]
    pub bucket_secs: u64,
    #[serde(default = "default_min_support")]
    pub min_support: usize,
    #[serde(default = "default_jaccard")]
    pub jaccard_threshold: f64,
    #[serde(default = "default_max_keys")]
    pub max_keys: usize,
}
fn default_duration()    -> String { "1h".to_owned() }
fn default_bucket_secs() -> u64    { 300 }
fn default_min_support() -> usize  { 2 }
fn default_jaccard()     -> f64    { 0.2 }
fn default_max_keys()    -> usize  { 200 }

// ── Full page ─────────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "rca_templates.html")]
struct RcaTemplatesPage {
    duration:          String,
    failure_body:      String,
    bucket_secs:       u64,
    min_support:       usize,
    jaccard_threshold: f64,
    max_keys:          usize,
}

pub async fn page(Query(p): Query<Params>) -> Result<Html<String>, AppError> {
    Ok(Html(RcaTemplatesPage {
        duration:          p.duration,
        failure_body:      p.failure_body,
        bucket_secs:       p.bucket_secs,
        min_support:       p.min_support,
        jaccard_threshold: p.jaccard_threshold,
        max_keys:          p.max_keys,
    }.render()?))
}

// ── HTMX results fragment ─────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "partials/rca_templates_results.html")]
struct RcaTemplatesResults {
    duration: String,
    summary:  RcaSummary,
    causes:   Vec<CausalRow>,
    clusters: Vec<ClusterCard>,
}

/// Rename the few field names that differ between v2/rca and v2/rca.templates
/// so that the shared `extract_rca` extractor can consume the response without
/// duplication (v2/rca uses `failure_key` and `probable_causes[].key`, whereas
/// v2/rca.templates uses `failure_body` and `probable_causes[].body`).
fn normalize_to_rca_shape(mut v: serde_json::Value) -> serde_json::Value {
    if let Some(obj) = v.as_object_mut() {
        if let Some(fb) = obj.remove("failure_body") {
            obj.insert("failure_key".to_owned(), fb);
        }
        if let Some(causes) = obj.get_mut("probable_causes").and_then(|x| x.as_array_mut()) {
            for c in causes {
                if let Some(c_obj) = c.as_object_mut() {
                    if let Some(body) = c_obj.remove("body") {
                        c_obj.insert("key".to_owned(), body);
                    }
                }
            }
        }
    }
    v
}

pub async fn results(
    State(state): State<AppState>,
    Query(p): Query<Params>,
) -> Result<Html<String>, AppError> {
    let fb: Option<String> = if p.failure_body.is_empty() { None } else { Some(p.failure_body.clone()) };

    let resp = rpc(&state, "v2/rca.templates", json!({
        "session":           SESSION,
        "duration":          p.duration,
        "failure_body":      fb,
        "bucket_secs":       p.bucket_secs,
        "min_support":       p.min_support,
        "jaccard_threshold": p.jaccard_threshold,
        "max_keys":          p.max_keys,
    })).await?;

    let (summary, causes, clusters) = extract_rca(&normalize_to_rca_shape(resp));

    Ok(Html(RcaTemplatesResults { duration: p.duration, summary, causes, clusters }.render()?))
}
