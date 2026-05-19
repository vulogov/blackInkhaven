// Root Cause Analysis on drain3 log-template observations — G-Forest variant.
//
// Every time the drain3 miner stores or updates a template document inside a
// shard's `tplstorage`, the metadata carries the Unix timestamp of the
// triggering log entry and the template body (e.g. "user <*> logged in from
// <*>").  This module treats each such stored template as an event
//
//   (template_body, log_timestamp)
//
// and runs the same G-Forest co-occurrence pipeline used by `rca.rs`:
//
//   1. Fetch (body, ts) pairs from `ShardsManager::templates_by_timestamp`.
//   2. Partition into non-overlapping time buckets.
//   3. Build a pairwise Jaccard co-occurrence matrix across template bodies.
//   4. Cluster via union-find G-Forest; rank clusters by cohesion.
//   5. Optionally rank probable precursors for a named failure template.
//
// Because drain stores multiple documents per logical template cluster (one
// per New/Updated event), the same body may appear at several timestamps —
// providing the temporal spread required for meaningful co-occurrence analysis.
//
// # Usage
//
// ```rust,no_run
// use bdslib::analysis::rca_templates::{RcaTemplatesConfig, RcaTemplatesResult};
// use bdslib::ShardsManager;
//
// let result = RcaTemplatesResult::analyze(&mgr, "2h", &RcaTemplatesConfig::default())?;
// println!("{} template clusters found", result.clusters.len());
//
// let rca = RcaTemplatesResult::analyze_failure(
//     &mgr, "app crash: <*>", "2h", &RcaTemplatesConfig::default()
// )?;
// for c in &rca.probable_causes {
//     println!("{:?}  lead={:.1}s", c.body, c.avg_lead_secs);
// }
// ```

use crate::common::error::{err_msg, Result};
use crate::shardsmanager::ShardsManager;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// ── Configuration ─────────────────────────────────────────────────────────────

/// Tuning knobs for the template RCA pipeline.
///
/// All fields have sensible defaults via [`Default`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RcaTemplatesConfig {
    /// Width of the non-overlapping time bucket used for co-occurrence
    /// counting (seconds).  Template events within the same bucket are
    /// considered co-occurring.  Default: 300 (5 minutes).
    pub bucket_secs: u64,

    /// Minimum number of time buckets a template body must appear in to be
    /// eligible for clustering.  Bodies below this threshold are skipped
    /// before the matrix is built.  Default: 2.
    pub min_support: usize,

    /// Minimum Jaccard similarity for two template bodies to be placed in
    /// the same cluster.  Lower values produce larger, looser clusters.
    /// Default: 0.2.
    pub jaccard_threshold: f64,

    /// Upper bound on distinct template bodies to analyse.  Bodies are
    /// ranked by total event count (most frequent first) before the cap is
    /// applied, so the most-active patterns are always included.
    /// Default: 200.
    pub max_keys: usize,
}

impl Default for RcaTemplatesConfig {
    fn default() -> Self {
        Self {
            bucket_secs: 300,
            min_support: 2,
            jaccard_threshold: 0.2,
            max_keys: 200,
        }
    }
}

// ── Output types ──────────────────────────────────────────────────────────────

/// A group of template bodies that co-occur in time more strongly than the
/// configured Jaccard threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateCluster {
    /// Sequential cluster id, assigned after sorting by cohesion descending.
    pub id: usize,

    /// Template body strings that belong to this cluster (sorted
    /// alphabetically).  Each string is the literal drain3 template pattern
    /// such as `"user <*> logged in from <*>"`.
    pub members: Vec<String>,

    /// Minimum bucket-frequency among all members — how often the whole
    /// cluster is simultaneously visible in the data.
    pub support: usize,

    /// Average pairwise Jaccard similarity among members.
    /// 1.0 = every member always appears in exactly the same time buckets.
    pub cohesion: f64,
}

/// A template body that co-occurs with a named failure template and tends to
/// precede it in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateCausalCandidate {
    /// Template body of this candidate.
    pub body: String,

    /// Number of time buckets in which both this body and the failure body
    /// were observed.
    pub co_occurrence_count: usize,

    /// Jaccard similarity between this body and the failure body.
    pub jaccard: f64,

    /// Mean seconds by which this body's earliest event in each shared bucket
    /// precedes the earliest failure event in that bucket.
    /// Positive = this template fires *before* the failure (causal signal).
    /// Negative = this template fires *after* the failure (consequence).
    pub avg_lead_secs: f64,
}

/// Complete result of a template-level root-cause analysis.
///
/// Obtain via [`RcaTemplatesResult::analyze`] or
/// [`RcaTemplatesResult::analyze_failure`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RcaTemplatesResult {
    /// Failure template body supplied to
    /// [`analyze_failure`](RcaTemplatesResult::analyze_failure), if any.
    pub failure_body: Option<String>,
    /// Unix seconds of the earliest template event seen in the analysis window.
    pub start: u64,
    /// Unix seconds of the latest template event seen in the analysis window.
    pub end: u64,
    /// Total template events analysed (one per `tplstorage` storage call in
    /// the window; same body may appear multiple times at different timestamps).
    pub n_events: usize,
    /// Number of distinct template bodies after support thresholding.
    pub n_keys: usize,
    /// Co-occurrence clusters, sorted by cohesion descending.
    pub clusters: Vec<TemplateCluster>,
    /// Probable precursor templates ranked by average lead time descending
    /// (strongest precursors first).  Empty when no failure body was given or
    /// the failure body was not observed in the window.
    pub probable_causes: Vec<TemplateCausalCandidate>,
}

impl RcaTemplatesResult {
    /// Cluster all templates stored in `manager`'s shards during `duration`.
    ///
    /// `duration` uses humantime notation: `"1h"`, `"30min"`, `"7days"`.
    pub fn analyze(
        manager: &ShardsManager,
        duration: &str,
        config: &RcaTemplatesConfig,
    ) -> Result<Self> {
        run(manager, None, duration, config)
    }

    /// Cluster templates and rank probable causes for `failure_body`.
    ///
    /// `failure_body` must be the exact template body string (e.g.
    /// `"error <*> in module <*>: <*>"`).  Returns an empty `probable_causes`
    /// list when the failure body was not observed in the analysis window.
    ///
    /// `duration` uses humantime notation: `"1h"`, `"30min"`, `"7days"`.
    pub fn analyze_failure(
        manager: &ShardsManager,
        failure_body: &str,
        duration: &str,
        config: &RcaTemplatesConfig,
    ) -> Result<Self> {
        run(manager, Some(failure_body), duration, config)
    }
}

// ── Core pipeline ─────────────────────────────────────────────────────────────

fn run(
    manager: &ShardsManager,
    failure_body: Option<&str>,
    duration: &str,
    config: &RcaTemplatesConfig,
) -> Result<RcaTemplatesResult> {
    let (window_start, window_end) = parse_window(duration)?;

    let events = fetch_template_events(manager, window_start, window_end, config)?;

    let start = events.iter().map(|(_, ts)| *ts).min().unwrap_or(window_start);
    let end   = events.iter().map(|(_, ts)| *ts).max().unwrap_or(window_end);

    let (cooccurrence, frequencies) = build_cooccurrence(&events, config);

    let clusters = cluster_by_jaccard(&cooccurrence, &frequencies, config);

    let probable_causes = match failure_body {
        Some(fb) => rank_causes(fb, &events, &cooccurrence, &frequencies, config),
        None     => vec![],
    };

    Ok(RcaTemplatesResult {
        failure_body: failure_body.map(str::to_owned),
        start,
        end,
        n_events: events.len(),
        n_keys:   frequencies.len(),
        clusters,
        probable_causes,
    })
}

fn parse_window(duration: &str) -> Result<(u64, u64)> {
    let dur = humantime::parse_duration(duration)
        .map_err(|e| err_msg(format!("rca_templates: invalid duration '{duration}': {e}")))?;
    let now   = SystemTime::now();
    let start = now.checked_sub(dur).unwrap_or(UNIX_EPOCH);
    let s = start.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let e = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    Ok((s, e))
}

// ── Event fetching ────────────────────────────────────────────────────────────

/// Collect `(template_body, unix_timestamp)` pairs for every template event
/// stored in `manager` within `[start, end]`.
///
/// Each shard's `tplstorage` FrequencyTracking is queried for template UUIDs
/// whose observation timestamp falls in the window.  For each UUID the
/// template body and its `metadata["timestamp"]` are extracted.  The body
/// string is used as the event key so that co-occurrence analysis reflects
/// which template patterns were active in the same time buckets.
///
/// Keys are ranked by frequency and capped at `config.max_keys` to keep the
/// O(n²) matrix tractable.
fn fetch_template_events(
    manager: &ShardsManager,
    start: u64,
    end: u64,
    config: &RcaTemplatesConfig,
) -> Result<Vec<(String, u64)>> {
    // Raw (body, ts) pairs before the max_keys cap.
    let mut raw: Vec<(String, u64)> = Vec::new();

    for info in manager.cache().info().list_all()? {
        let shard = manager.cache().shard(info.start_time)?;
        for id_str in shard.tplstorage.frequencytracking_time_range(start, end)? {
            let uuid = match uuid::Uuid::parse_str(&id_str) {
                Ok(u)  => u,
                Err(_) => continue,
            };
            let Some(metadata) = shard.tpl_get_metadata(uuid)? else { continue };
            let ts = metadata.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
            if ts == 0 { continue; }
            let body = shard
                .tpl_get_body(uuid)?
                .unwrap_or_default();
            let body_str = String::from_utf8_lossy(&body).into_owned();
            if body_str.is_empty() { continue; }
            raw.push((body_str, ts));
        }
    }

    // Count events per body; apply max_keys cap (most-frequent bodies first).
    let mut body_counts: HashMap<String, usize> = HashMap::new();
    for (body, _) in &raw {
        *body_counts.entry(body.clone()).or_default() += 1;
    }

    if body_counts.len() <= config.max_keys {
        return Ok(raw);
    }

    // Collect and sort by frequency descending, then cap.
    let mut freq_list: Vec<(String, usize)> = body_counts.into_iter().collect();
    freq_list.sort_by(|a, b| b.1.cmp(&a.1));
    let allowed: std::collections::HashSet<String> =
        freq_list.into_iter().take(config.max_keys).map(|(k, _)| k).collect();

    Ok(raw.into_iter().filter(|(b, _)| allowed.contains(b)).collect())
}

// ── Co-occurrence matrix ──────────────────────────────────────────────────────

fn build_cooccurrence(
    events: &[(String, u64)],
    config: &RcaTemplatesConfig,
) -> (HashMap<(String, String), usize>, HashMap<String, usize>) {
    let mut buckets: HashMap<u64, std::collections::HashSet<String>> = HashMap::new();
    for (body, ts) in events {
        buckets
            .entry(ts / config.bucket_secs)
            .or_default()
            .insert(body.clone());
    }

    let mut cooccurrence: HashMap<(String, String), usize> = HashMap::new();
    let mut frequencies:  HashMap<String, usize>           = HashMap::new();

    for bodies in buckets.values() {
        for b in bodies {
            *frequencies.entry(b.clone()).or_default() += 1;
        }
        let bv: Vec<&String> = bodies.iter().collect();
        for i in 0..bv.len() {
            for j in (i + 1)..bv.len() {
                *cooccurrence
                    .entry(ordered_pair(bv[i].clone(), bv[j].clone()))
                    .or_default() += 1;
            }
        }
    }

    // Apply min_support filter.
    frequencies.retain(|_, &mut v| v >= config.min_support);
    cooccurrence.retain(|(a, b), _| {
        frequencies.contains_key(a.as_str()) && frequencies.contains_key(b.as_str())
    });

    (cooccurrence, frequencies)
}

#[inline]
fn ordered_pair(a: String, b: String) -> (String, String) {
    if a <= b { (a, b) } else { (b, a) }
}

fn jaccard_sim(
    a: &str,
    b: &str,
    cooccurrence: &HashMap<(String, String), usize>,
    frequencies:  &HashMap<String, usize>,
) -> f64 {
    let co = *cooccurrence
        .get(&ordered_pair(a.to_owned(), b.to_owned()))
        .unwrap_or(&0) as f64;
    if co == 0.0 {
        return 0.0;
    }
    let fa    = *frequencies.get(a).unwrap_or(&0) as f64;
    let fb    = *frequencies.get(b).unwrap_or(&0) as f64;
    let union = fa + fb - co;
    if union <= 0.0 { 0.0 } else { co / union }
}

// ── G-Forest clustering ───────────────────────────────────────────────────────
//
// Union-Find forest where each tree represents one co-occurrence cluster.
// Edges are added between every pair of template bodies whose Jaccard
// similarity meets or exceeds `config.jaccard_threshold`.

struct UnionFind {
    parent: Vec<usize>,
    rank:   Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self { parent: (0..n).collect(), rank: vec![0; n] }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            let px   = self.parent[x];
            let root = self.find(px);
            self.parent[x] = root;
            root
        } else {
            x
        }
    }

    fn union(&mut self, x: usize, y: usize) {
        let px = self.find(x);
        let py = self.find(y);
        if px == py { return; }
        match self.rank[px].cmp(&self.rank[py]) {
            std::cmp::Ordering::Less    => self.parent[px] = py,
            std::cmp::Ordering::Greater => self.parent[py] = px,
            std::cmp::Ordering::Equal   => {
                self.parent[py] = px;
                self.rank[px] += 1;
            }
        }
    }
}

fn cluster_by_jaccard(
    cooccurrence: &HashMap<(String, String), usize>,
    frequencies:  &HashMap<String, usize>,
    config: &RcaTemplatesConfig,
) -> Vec<TemplateCluster> {
    let mut keys: Vec<String> = frequencies.keys().cloned().collect();
    keys.sort();
    let n = keys.len();
    if n == 0 { return vec![]; }

    let mut uf = UnionFind::new(n);
    for i in 0..n {
        for j in (i + 1)..n {
            if jaccard_sim(&keys[i], &keys[j], cooccurrence, frequencies)
                >= config.jaccard_threshold
            {
                uf.union(i, j);
            }
        }
    }

    let mut components: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        components.entry(uf.find(i)).or_default().push(i);
    }

    let mut clusters: Vec<TemplateCluster> = components
        .values()
        .enumerate()
        .map(|(id, indices)| {
            let mut members: Vec<String> =
                indices.iter().map(|&i| keys[i].clone()).collect();
            members.sort();

            let support = members
                .iter()
                .filter_map(|k| frequencies.get(k))
                .copied()
                .min()
                .unwrap_or(0);

            let cohesion = pairwise_mean_jaccard(&members, cooccurrence, frequencies);

            TemplateCluster { id, members, support, cohesion }
        })
        .collect();

    clusters.sort_by(|a, b| {
        b.cohesion
            .partial_cmp(&a.cohesion)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.support.cmp(&a.support))
    });
    for (i, c) in clusters.iter_mut().enumerate() {
        c.id = i;
    }

    clusters
}

fn pairwise_mean_jaccard(
    members: &[String],
    cooccurrence: &HashMap<(String, String), usize>,
    frequencies:  &HashMap<String, usize>,
) -> f64 {
    let n = members.len();
    if n == 0 { return 0.0; }
    if n == 1 { return 1.0; }
    let mut sum = 0.0;
    let mut cnt = 0usize;
    for i in 0..n {
        for j in (i + 1)..n {
            sum += jaccard_sim(&members[i], &members[j], cooccurrence, frequencies);
            cnt += 1;
        }
    }
    if cnt > 0 { sum / cnt as f64 } else { 0.0 }
}

// ── Causal ranking ────────────────────────────────────────────────────────────

/// For each template body that shares at least one bucket with `failure_body`,
/// compute the co-occurrence count, Jaccard similarity, and average lead time
/// (positive = precedes the failure).  Results are sorted by lead time
/// descending (strongest precursors first).
fn rank_causes(
    failure_body: &str,
    events:       &[(String, u64)],
    cooccurrence: &HashMap<(String, String), usize>,
    frequencies:  &HashMap<String, usize>,
    config:       &RcaTemplatesConfig,
) -> Vec<TemplateCausalCandidate> {
    if frequencies.get(failure_body).copied().unwrap_or(0) == 0 {
        return vec![];
    }

    // Earliest failure timestamp per bucket.
    let mut failure_buckets: HashMap<u64, u64> = HashMap::new();
    for (body, ts) in events {
        if body == failure_body {
            let bucket = ts / config.bucket_secs;
            failure_buckets
                .entry(bucket)
                .and_modify(|e| { if *ts < *e { *e = *ts } })
                .or_insert(*ts);
        }
    }

    if failure_buckets.is_empty() {
        return vec![];
    }

    let mut acc: HashMap<String, (usize, f64)> = HashMap::new();
    for (body, ts) in events {
        if body == failure_body { continue; }
        let bucket = ts / config.bucket_secs;
        if let Some(&fail_ts) = failure_buckets.get(&bucket) {
            let lead = fail_ts as f64 - *ts as f64;
            let e = acc.entry(body.clone()).or_default();
            e.0 += 1;
            e.1 += lead;
        }
    }

    let mut candidates: Vec<TemplateCausalCandidate> = acc
        .into_iter()
        .map(|(body, (count, sum_lead))| TemplateCausalCandidate {
            jaccard: jaccard_sim(&body, failure_body, cooccurrence, frequencies),
            co_occurrence_count: count,
            avg_lead_secs: if count > 0 { sum_lead / count as f64 } else { 0.0 },
            body,
        })
        .collect();

    candidates.sort_by(|a, b| {
        b.avg_lead_secs
            .partial_cmp(&a.avg_lead_secs)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                b.jaccard
                    .partial_cmp(&a.jaccard)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });

    candidates
}
