// Root Cause Analysis
//
// Fetch all non-telemetry events from the global DB over a time window,
// cluster the event keys that routinely co-occur in the same time buckets
// (Jaccard-similarity-based union-find), and — when a specific failure key
// is named — rank every co-occurring key by how consistently it precedes the
// failure, yielding a list of probable root-cause candidates.
//
// Telemetry discrimination: a record is discarded when its `data` field is a
// JSON number, or when `data["value"]` is a JSON number (the standard shape
// produced by the metric pipeline).  Everything else is treated as an event.

use crate::common::error::{err_msg, Result};
use crate::globals::get_db;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

// ── Configuration ─────────────────────────────────────────────────────────────

/// Tuning knobs for the RCA pipeline.
///
/// All fields have sensible defaults via [`Default`]; construct with
/// `RcaConfig { bucket_secs: 60, ..Default::default() }` to override one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RcaConfig {
    /// Width of the non-overlapping time bucket used for co-occurrence
    /// counting (seconds).  Events within the same bucket are considered
    /// to have co-occurred.  Default: 300 (5 minutes).
    pub bucket_secs: u64,

    /// Minimum number of time buckets a key must appear in to be eligible.
    /// Keys below this threshold are skipped before the matrix is built.
    /// Default: 2.
    pub min_support: usize,

    /// Minimum Jaccard similarity between two keys for them to be placed in
    /// the same cluster.  Range [0, 1].  Lower values produce larger, looser
    /// clusters.  Default: 0.2.
    pub jaccard_threshold: f64,

    /// Upper bound on distinct event keys to analyse.  Keys are ranked by
    /// total primary-record count (most frequent first) before the cap is
    /// applied, so the most-informative signals are always included.
    /// Default: 200.
    pub max_keys: usize,
}

impl Default for RcaConfig {
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

/// A group of event keys that co-occur in time more strongly than the
/// configured Jaccard threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventCluster {
    /// Sequential cluster id, assigned after sorting by cohesion descending.
    pub id: usize,

    /// Event keys that belong to this cluster (sorted alphabetically).
    pub members: Vec<String>,

    /// Minimum bucket-frequency among all members — a conservative measure
    /// of how often the whole cluster is visible in the data.
    pub support: usize,

    /// Average pairwise Jaccard similarity among members.
    /// 1.0 = every member always appears in exactly the same buckets;
    /// 0.0 = no two members ever appear together.
    pub cohesion: f64,
}

/// A single candidate root-cause event: a key that co-occurs with the named
/// failure and tends to appear before it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalCandidate {
    /// Event key of this candidate.
    pub key: String,

    /// Number of time buckets in which both this key and the failure key
    /// were observed.
    pub co_occurrence_count: usize,

    /// Jaccard similarity between this key and the failure key.
    pub jaccard: f64,

    /// Mean seconds by which this key's earliest event in a shared bucket
    /// precedes the earliest failure event in that same bucket.
    /// Positive = this key tends to arrive *before* the failure (causal signal).
    /// Negative = this key tends to arrive *after* the failure (consequence).
    pub avg_lead_secs: f64,
}

/// Complete result of a root-cause analysis.
///
/// Obtain via [`RcaResult::analyze`] or [`RcaResult::analyze_failure`].
/// Both require [`init_db`](crate::init_db) to have been called first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RcaResult {
    /// Failure key supplied to [`RcaResult::analyze_failure`], if any.
    pub failure_key: Option<String>,
    /// Unix seconds of the earliest event timestamp seen in the analysis window.
    pub start: u64,
    /// Unix seconds of the latest event timestamp seen in the analysis window.
    pub end: u64,
    /// Total non-telemetry primary records analysed.
    pub n_events: usize,
    /// Number of distinct event keys after telemetry filtering and support
    /// thresholding.
    pub n_keys: usize,
    /// Co-occurrence clusters, sorted by cohesion descending, then support
    /// descending.
    pub clusters: Vec<EventCluster>,
    /// Probable root-cause candidates ranked by average lead time descending
    /// (strongest precursors first).  Empty when no failure key was given, or
    /// when the failure key was not observed in the window.
    pub probable_causes: Vec<CausalCandidate>,
}

impl RcaResult {
    /// Cluster all non-telemetry events in the given `duration` window without
    /// targeting a specific failure.
    ///
    /// `duration` uses humantime notation (`"1h"`, `"30min"`, `"7days"`).
    pub fn analyze(duration: &str, config: &RcaConfig) -> Result<Self> {
        run(None, duration, config)
    }

    /// Cluster events in `duration` and rank probable causes for `failure_key`.
    ///
    /// `duration` uses humantime notation (`"1h"`, `"30min"`, `"7days"`).
    pub fn analyze_failure(
        failure_key: &str,
        duration: &str,
        config: &RcaConfig,
    ) -> Result<Self> {
        run(Some(failure_key), duration, config)
    }
}

// ── Core pipeline ─────────────────────────────────────────────────────────────

fn run(failure_key: Option<&str>, duration: &str, config: &RcaConfig) -> Result<RcaResult> {
    let (window_start, window_end) = parse_window(duration)?;

    let events = fetch_events(duration, config)?;

    let start = events.iter().map(|(_, ts)| *ts).min().unwrap_or(window_start);
    let end   = events.iter().map(|(_, ts)| *ts).max().unwrap_or(window_end);

    let (cooccurrence, frequencies) = build_cooccurrence(&events, config);

    let clusters = cluster_by_jaccard(&cooccurrence, &frequencies, config);

    let probable_causes = match failure_key {
        Some(fk) => rank_causes(fk, &events, &cooccurrence, &frequencies, config),
        None      => vec![],
    };

    Ok(RcaResult {
        failure_key: failure_key.map(str::to_owned),
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
        .map_err(|e| err_msg(format!("rca: invalid duration '{duration}': {e}")))?;
    let now   = SystemTime::now();
    let start = now.checked_sub(dur).unwrap_or(UNIX_EPOCH);
    let start_secs = start.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let end_secs   = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    Ok((start_secs, end_secs))
}

// ── Event fetching ────────────────────────────────────────────────────────────

/// Collect `(key, unix_timestamp)` pairs for every non-telemetry primary event
/// in the given duration window.
///
/// The DB is queried key-by-key via the ShardsManager public API:
///   1. `primaries_explore` enumerates all keys with >1 primary in the window,
///      together with their total primary-record count.
///   2. Keys are sorted most-frequent-first and capped at `config.max_keys`.
///   3. For each eligible key, `primaries_get` fetches `(uuid, ts, data)`.
///   4. Records whose `data` is numeric (telemetry) are discarded.  If a key
///      has no non-telemetry records at all it is silently skipped and does not
///      count against `max_keys`.
fn fetch_events(duration: &str, config: &RcaConfig) -> Result<Vec<(String, u64)>> {
    let db = get_db()?;

    let mut key_infos = db
        .primaries_explore(duration)
        .map_err(|e| err_msg(format!("rca: primaries_explore: {e}")))?;

    // Most-frequent keys first so the cap retains the strongest signals.
    key_infos.sort_by(|a, b| b.1.cmp(&a.1));

    let mut events: Vec<(String, u64)> = Vec::new();
    let mut keys_accepted = 0usize;

    for (key, count, _ids) in key_infos {
        if keys_accepted >= config.max_keys {
            break;
        }
        // primaries_explore already enforces count > 1; min_support may be higher.
        if count < config.min_support {
            continue;
        }

        let records = db
            .primaries_get(duration, &key)
            .map_err(|e| err_msg(format!("rca: primaries_get({key}): {e}")))?;

        let non_telemetry: Vec<(String, u64)> = records
            .into_iter()
            .filter(|(_, _, data)| !is_telemetry(data))
            .map(|(_, ts, _)| (key.clone(), ts))
            .collect();

        if non_telemetry.is_empty() {
            // All records for this key are numeric metrics — skip without
            // counting against max_keys so the cap applies to event keys only.
            continue;
        }

        keys_accepted += 1;
        events.extend(non_telemetry);
    }

    Ok(events)
}

/// Returns `true` when the `data` payload is a numeric measurement (telemetry)
/// rather than a discrete event log.
///
/// Matches both bare-number data (`"data": 72.4`) and structured metric shape
/// (`"data": {"value": 72.4, "unit": "percent", ...}`).
#[inline]
fn is_telemetry(data: &JsonValue) -> bool {
    data.is_number() || data.get("value").map_or(false, JsonValue::is_number)
}

// ── Co-occurrence matrix ──────────────────────────────────────────────────────

/// Partition events into non-overlapping time buckets of `config.bucket_secs`
/// and count:
/// - how many distinct buckets each key appears in (`frequencies`), and
/// - how many distinct buckets each unordered pair of keys share (`cooccurrence`).
fn build_cooccurrence(
    events: &[(String, u64)],
    config: &RcaConfig,
) -> (HashMap<(String, String), usize>, HashMap<String, usize>) {
    // One de-duplicated key-set per bucket (a key is counted once per bucket
    // even if multiple events with that key fall in the same bucket).
    let mut buckets: HashMap<u64, std::collections::HashSet<String>> = HashMap::new();
    for (key, ts) in events {
        buckets
            .entry(ts / config.bucket_secs)
            .or_default()
            .insert(key.clone());
    }

    let mut cooccurrence: HashMap<(String, String), usize> = HashMap::new();
    let mut frequencies:  HashMap<String, usize>           = HashMap::new();

    for keys in buckets.values() {
        for k in keys {
            *frequencies.entry(k.clone()).or_default() += 1;
        }
        // Every unordered pair that shares this bucket gets one co-occurrence vote.
        let kv: Vec<&String> = keys.iter().collect();
        for i in 0..kv.len() {
            for j in (i + 1)..kv.len() {
                *cooccurrence
                    .entry(ordered_pair(kv[i].clone(), kv[j].clone()))
                    .or_default() += 1;
            }
        }
    }

    (cooccurrence, frequencies)
}

/// Canonical unordered-pair key: lexicographically smaller string first.
#[inline]
fn ordered_pair(a: String, b: String) -> (String, String) {
    if a <= b { (a, b) } else { (b, a) }
}

/// Jaccard similarity of two keys based on shared bucket appearances.
///
/// J(A, B) = |A ∩ B| / |A ∪ B|  where the sets are the bucket-ids each
/// key appears in.
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
    let union = fa + fb - co; // |A ∪ B| = |A| + |B| - |A ∩ B|
    if union <= 0.0 { 0.0 } else { co / union }
}

// ── Clustering ────────────────────────────────────────────────────────────────

/// Path-compressed, union-by-rank union-find.
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
            self.parent[x] = root; // path compression
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

/// Build clusters via union-find: add an edge between every pair of keys whose
/// Jaccard similarity meets or exceeds `config.jaccard_threshold`.
///
/// Result is sorted by cohesion descending, then support descending.
fn cluster_by_jaccard(
    cooccurrence: &HashMap<(String, String), usize>,
    frequencies:  &HashMap<String, usize>,
    config: &RcaConfig,
) -> Vec<EventCluster> {
    let mut keys: Vec<String> = frequencies.keys().cloned().collect();
    keys.sort(); // deterministic ordering before index assignment
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

    // Group key indices by their union-find root.
    let mut components: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        let root = uf.find(i);
        components.entry(root).or_default().push(i);
    }

    let mut clusters: Vec<EventCluster> = components
        .values()
        .enumerate()
        .map(|(id, indices)| {
            let mut members: Vec<String> =
                indices.iter().map(|&i| keys[i].clone()).collect();
            members.sort();

            // Support = minimum bucket-frequency among all members (most
            // conservative: the whole cluster is visible only when every
            // member fires, so the bottleneck member is the constraint).
            let support = members
                .iter()
                .filter_map(|k| frequencies.get(k))
                .copied()
                .min()
                .unwrap_or(0);

            let cohesion =
                pairwise_mean_jaccard(&members, cooccurrence, frequencies);

            EventCluster { id, members, support, cohesion }
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

/// For each key that shares at least one time bucket with `failure_key`,
/// compute:
///
/// 1. **co_occurrence_count** — how many shared buckets (raw event count, not
///    bucket count, because one key may fire multiple times in a bucket).
/// 2. **jaccard** — overall Jaccard similarity between the candidate and the
///    failure key.
/// 3. **avg_lead_secs** — average signed delta between the earliest failure
///    timestamp in each shared bucket and the candidate's own timestamp.
///    Positive values indicate the candidate fires *before* the failure,
///    making it a candidate precursor / root cause.
///
/// Candidates are sorted by avg_lead_secs descending (strongest precursors
/// first); ties are broken by Jaccard descending.
fn rank_causes(
    failure_key:  &str,
    events:       &[(String, u64)],
    cooccurrence: &HashMap<(String, String), usize>,
    frequencies:  &HashMap<String, usize>,
    config:       &RcaConfig,
) -> Vec<CausalCandidate> {
    if frequencies.get(failure_key).copied().unwrap_or(0) == 0 {
        return vec![];
    }

    // Earliest failure timestamp per bucket — used as the reference point
    // for lead-time computation.
    let mut failure_buckets: HashMap<u64, u64> = HashMap::new();
    for (key, ts) in events {
        if key == failure_key {
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

    // For every non-failure event that falls in a failure-containing bucket,
    // accumulate (count, sum of lead seconds).
    let mut acc: HashMap<String, (usize, f64)> = HashMap::new();
    for (key, ts) in events {
        if key == failure_key { continue; }
        let bucket = ts / config.bucket_secs;
        if let Some(&fail_ts) = failure_buckets.get(&bucket) {
            // Positive when this event precedes the earliest failure in its bucket.
            let lead = fail_ts as f64 - *ts as f64;
            let e = acc.entry(key.clone()).or_default();
            e.0 += 1;
            e.1 += lead;
        }
    }

    let mut candidates: Vec<CausalCandidate> = acc
        .into_iter()
        .map(|(key, (count, sum_lead))| CausalCandidate {
            jaccard: jaccard_sim(&key, failure_key, cooccurrence, frequencies),
            co_occurrence_count: count,
            avg_lead_secs: if count > 0 { sum_lead / count as f64 } else { 0.0 },
            key,
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
