use askama::Template;
use axum::{extract::{Query, State}, response::Html};
use serde::Deserialize;
use serde_json::json;

use crate::{client::{fmt_ts, rpc, SESSION}, error::AppError, state::AppState};

// ── Query parameters ──────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct Params {
    #[serde(default = "default_duration")]
    pub duration: String,
    #[serde(default)]
    pub failure_key: String,
    #[serde(default = "default_bucket_secs")]
    pub bucket_secs: u64,
    #[serde(default = "default_min_support")]
    pub min_support: usize,
    #[serde(default = "default_jaccard")]
    pub jaccard_threshold: f64,
}
fn default_duration()    -> String { "1h".to_owned() }
fn default_bucket_secs() -> u64   { 300 }
fn default_min_support() -> usize  { 2 }
fn default_jaccard()     -> f64   { 0.2 }

// ── Template data types ───────────────────────────────────────────────────────

pub struct RcaSummary {
    pub failure_key:   String,
    pub has_failure:   bool,
    pub window_start:  String,
    pub window_end:    String,
    pub n_events:      usize,
    pub n_keys:        usize,
    pub cluster_count: usize,
    pub cause_count:   usize,
}

pub struct CausalRow {
    pub rank:         usize,
    pub key:          String,
    pub co_count:     usize,
    pub jaccard:      String,
    pub lead_label:   String,
    pub lead_bar_pct: u8,
    pub is_precursor: bool,
    pub lead_cls:     String,
    pub bar_cls:      String,
}

pub struct ClusterCard {
    pub id:                usize,
    pub members:           Vec<String>,
    pub support:           usize,
    pub cohesion:          String,
    pub cohesion_pct:      u8,
    pub cohesion_bar_cls:  String,
    pub cohesion_badge_cls: String,
}

// ── Data extraction ───────────────────────────────────────────────────────────

fn fmt_lead_label(secs: f64) -> String {
    let abs = secs.abs();
    if abs < 1.0 { return "simultaneous".to_owned(); }
    let mins = (abs / 60.0) as u64;
    let s    = (abs % 60.0) as u64;
    let t = if mins > 0 { format!("{mins}m {s:02}s") } else { format!("{s}s") };
    if secs > 0.0 { format!("{t} before") } else { format!("{t} after") }
}

pub(super) fn extract_rca(v: &serde_json::Value) -> (RcaSummary, Vec<CausalRow>, Vec<ClusterCard>) {
    let failure_key = v.get("failure_key")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_owned();
    let has_failure = !failure_key.is_empty();

    let start    = v.get("start").and_then(|x| x.as_u64()).unwrap_or(0);
    let end      = v.get("end").and_then(|x| x.as_u64()).unwrap_or(0);
    let n_events = v.get("n_events").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
    let n_keys   = v.get("n_keys").and_then(|x| x.as_u64()).unwrap_or(0) as usize;

    // ── Clusters ──────────────────────────────────────────────────────────────
    let clusters: Vec<ClusterCard> = v
        .get("clusters")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter().map(|c| {
                let id         = c.get("id").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                let support    = c.get("support").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                let cohesion_f = c.get("cohesion").and_then(|x| x.as_f64()).unwrap_or(0.0);
                let cohesion_pct = (cohesion_f * 100.0).clamp(0.0, 100.0) as u8;
                let members: Vec<String> = c
                    .get("members")
                    .and_then(|x| x.as_array())
                    .map(|a| a.iter().filter_map(|m| m.as_str().map(str::to_owned)).collect())
                    .unwrap_or_default();
                let (cohesion_bar_cls, cohesion_badge_cls) = if cohesion_pct >= 70 {
                    ("bg-green-600", "bg-green-900 text-green-300")
                } else if cohesion_pct >= 40 {
                    ("bg-blue-600", "bg-blue-900 text-blue-300")
                } else {
                    ("bg-slate-600", "bg-slate-800 text-slate-400")
                };
                ClusterCard {
                    id,
                    members,
                    support,
                    cohesion: format!("{cohesion_f:.2}"),
                    cohesion_pct,
                    cohesion_bar_cls:  cohesion_bar_cls.to_owned(),
                    cohesion_badge_cls: cohesion_badge_cls.to_owned(),
                }
            }).collect()
        })
        .unwrap_or_default();

    // ── Probable causes ───────────────────────────────────────────────────────
    let causes_raw: Vec<serde_json::Value> = v
        .get("probable_causes")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();

    let max_abs_lead = causes_raw
        .iter()
        .filter_map(|c| c.get("avg_lead_secs").and_then(|x| x.as_f64()))
        .map(|f| f.abs())
        .fold(0.0f64, f64::max);

    let causes: Vec<CausalRow> = causes_raw.iter().enumerate().map(|(i, c)| {
        let key      = c.get("key").and_then(|x| x.as_str()).unwrap_or("—").to_owned();
        let co_count = c.get("co_occurrence_count").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
        let jaccard  = c.get("jaccard").and_then(|x| x.as_f64()).unwrap_or(0.0);
        let lead     = c.get("avg_lead_secs").and_then(|x| x.as_f64()).unwrap_or(0.0);
        let is_precursor = lead >= 0.0;
        let lead_bar_pct = if max_abs_lead > 0.0 {
            ((lead.abs() / max_abs_lead) * 100.0).clamp(0.0, 100.0) as u8
        } else { 0 };
        CausalRow {
            rank: i + 1,
            key,
            co_count,
            jaccard: format!("{jaccard:.2}"),
            lead_label: fmt_lead_label(lead),
            lead_bar_pct,
            is_precursor,
            lead_cls: if is_precursor { "text-green-400" } else { "text-amber-400" }.to_owned(),
            bar_cls:  if is_precursor { "bg-green-600"  } else { "bg-amber-600"  }.to_owned(),
        }
    }).collect();

    let summary = RcaSummary {
        failure_key,
        has_failure,
        window_start:  fmt_ts(start),
        window_end:    fmt_ts(end),
        n_events,
        n_keys,
        cluster_count: clusters.len(),
        cause_count:   causes.len(),
    };

    (summary, causes, clusters)
}

// ── Full page ─────────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "rca.html")]
struct RcaPage {
    duration:          String,
    failure_key:       String,
    bucket_secs:       u64,
    min_support:       usize,
    jaccard_threshold: f64,
}

pub async fn page(Query(p): Query<Params>) -> Result<Html<String>, AppError> {
    Ok(Html(RcaPage {
        duration:          p.duration,
        failure_key:       p.failure_key,
        bucket_secs:       p.bucket_secs,
        min_support:       p.min_support,
        jaccard_threshold: p.jaccard_threshold,
    }.render()?))
}

// ── HTMX results fragment ─────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "partials/rca_results.html")]
struct RcaResults {
    duration: String,
    summary:  RcaSummary,
    causes:   Vec<CausalRow>,
    clusters: Vec<ClusterCard>,
}

pub async fn results(
    State(state): State<AppState>,
    Query(p): Query<Params>,
) -> Result<Html<String>, AppError> {
    let fk: Option<String> = if p.failure_key.is_empty() { None } else { Some(p.failure_key.clone()) };

    let resp = rpc(&state, "v2/rca", json!({
        "session":           SESSION,
        "duration":          p.duration,
        "failure_key":       fk,
        "bucket_secs":       p.bucket_secs,
        "min_support":       p.min_support,
        "jaccard_threshold": p.jaccard_threshold,
    })).await?;

    let (summary, causes, clusters) = extract_rca(&resp);

    Ok(Html(RcaResults { duration: p.duration, summary, causes, clusters }.render()?))
}
