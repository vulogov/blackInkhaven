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
    #[serde(default = "default_k")]
    pub k: usize,
    #[serde(default = "default_min_word_len")]
    pub min_word_len: usize,
    #[serde(default = "default_anomaly_threshold")]
    pub anomaly_threshold: f32,
    #[serde(default = "default_max_cluster_members")]
    pub max_cluster_members: usize,
    #[serde(default = "default_max_anomalies")]
    pub max_anomalies: usize,
}
fn default_duration()            -> String { "1h".to_owned() }
fn default_k()                   -> usize { 5 }
fn default_min_word_len()        -> usize { 2 }
fn default_anomaly_threshold()   -> f32   { 0.2 }
fn default_max_cluster_members() -> usize { 10 }
fn default_max_anomalies()       -> usize { 20 }

// ── Page shell ────────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "knn.html")]
struct KnnPage {
    duration:            String,
    k:                   usize,
    min_word_len:        usize,
    anomaly_threshold:   f32,
    max_cluster_members: usize,
    max_anomalies:       usize,
}

pub async fn page(Query(p): Query<Params>) -> Result<Html<String>, AppError> {
    Ok(Html(KnnPage {
        duration:            p.duration,
        k:                   p.k,
        min_word_len:        p.min_word_len,
        anomaly_threshold:   p.anomaly_threshold,
        max_cluster_members: p.max_cluster_members,
        max_anomalies:       p.max_anomalies,
    }.render()?))
}

// ── HTMX results fragment ─────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ClusterMember {
    pub idx:     u64,
    pub density: f64,
    pub text:    String,
}

#[derive(Debug)]
pub struct ClusterRow {
    pub id:               u64,
    pub size:             u64,
    pub rep_idx:          u64,
    pub rep_density:      f64,
    pub rep_text:         String,
    pub rep_short:        String,
    pub members:          Vec<ClusterMember>,
    pub members_shown:    usize,
}

#[derive(Debug)]
pub struct AnomalyRow {
    pub idx:            u64,
    pub max_similarity: f64,
    pub text:           String,
}

#[derive(Template)]
#[template(path = "partials/knn_result.html")]
struct KnnResult {
    duration:            String,
    n_logs:              u64,
    k:                   u64,
    anomaly_threshold:   f64,
    n_clusters:          u64,
    n_anomalies:         u64,
    has_clusters:        bool,
    has_anomalies:       bool,
    clusters:            Vec<ClusterRow>,
    anomalies:           Vec<AnomalyRow>,
}

/// First 80 characters of `s` with an ellipsis appended when truncated —
/// keeps the cluster header card readable when the representative
/// fingerprint is very long.
fn shorten(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…")
    }
}

pub async fn results(
    State(state): State<AppState>,
    Query(p): Query<Params>,
) -> Result<Html<String>, AppError> {
    let resp = rpc(&state, "v2/knn", json!({
        "session":             SESSION,
        "duration":            p.duration.clone(),
        "k":                   p.k,
        "min_word_len":        p.min_word_len,
        "anomaly_threshold":   p.anomaly_threshold,
        "max_cluster_members": p.max_cluster_members,
        "max_anomalies":       p.max_anomalies,
    })).await?;

    let n_logs            = resp.get("n_logs").and_then(JsonValue::as_u64).unwrap_or(0);
    let k_eff             = resp.get("k").and_then(JsonValue::as_u64).unwrap_or(p.k as u64);
    let anomaly_threshold = resp.get("anomaly_threshold").and_then(JsonValue::as_f64)
        .unwrap_or(p.anomaly_threshold as f64);
    let n_clusters        = resp.get("n_clusters").and_then(JsonValue::as_u64).unwrap_or(0);
    let n_anomalies       = resp.get("n_anomalies").and_then(JsonValue::as_u64).unwrap_or(0);

    // Clusters: for each, extract id/size/representative/members.
    let clusters: Vec<ClusterRow> = resp.get("clusters")
        .and_then(JsonValue::as_array)
        .map(|arr| arr.iter().map(|c| {
            let rep = c.get("representative").cloned().unwrap_or(JsonValue::Null);
            let rep_text = rep.get("text").and_then(|v| v.as_str()).unwrap_or("").to_owned();
            let members: Vec<ClusterMember> = c.get("members")
                .and_then(JsonValue::as_array)
                .map(|ms| ms.iter().map(|m| ClusterMember {
                    idx:     m.get("idx").and_then(JsonValue::as_u64).unwrap_or(0),
                    density: m.get("density").and_then(JsonValue::as_f64).unwrap_or(0.0),
                    text:    m.get("text").and_then(|v| v.as_str()).unwrap_or("").to_owned(),
                }).collect())
                .unwrap_or_default();
            let members_shown = members.len();
            ClusterRow {
                id:           c.get("id").and_then(JsonValue::as_u64).unwrap_or(0),
                size:         c.get("size").and_then(JsonValue::as_u64).unwrap_or(0),
                rep_idx:      rep.get("idx").and_then(JsonValue::as_u64).unwrap_or(0),
                rep_density:  rep.get("density").and_then(JsonValue::as_f64).unwrap_or(0.0),
                rep_short:    shorten(&rep_text, 100),
                rep_text,
                members,
                members_shown,
            }
        }).collect())
        .unwrap_or_default();

    let anomalies: Vec<AnomalyRow> = resp.get("anomalies")
        .and_then(JsonValue::as_array)
        .map(|arr| arr.iter().map(|a| AnomalyRow {
            idx:            a.get("idx").and_then(JsonValue::as_u64).unwrap_or(0),
            max_similarity: a.get("max_similarity").and_then(JsonValue::as_f64).unwrap_or(0.0),
            text:           a.get("text").and_then(|v| v.as_str()).unwrap_or("").to_owned(),
        }).collect())
        .unwrap_or_default();

    let has_clusters  = !clusters.is_empty();
    let has_anomalies = !anomalies.is_empty();

    Ok(Html(KnnResult {
        duration:          p.duration,
        n_logs,
        k:                 k_eff,
        anomaly_threshold,
        n_clusters,
        n_anomalies,
        has_clusters,
        has_anomalies,
        clusters,
        anomalies,
    }.render()?))
}
