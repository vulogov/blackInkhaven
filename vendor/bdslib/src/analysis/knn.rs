//! k-Nearest-Neighbour intelligence over a list of strings.
//!
//! `knn_summary` analyses a corpus of short text snippets — log lines, JSON
//! fingerprints, sentences — by:
//!
//!   1. **Tokenise** every input (lowercase alphanumeric, stop-words
//!      removed, very short tokens dropped).
//!   2. **TF-IDF** weights for every (term, input) pair (smoothed IDF).
//!   3. **Pairwise cosine similarity** across all inputs.
//!   4. **k-Nearest neighbours** per input — the `k` most similar peers,
//!      excluding self.
//!   5. **Density** per input — average similarity to its top-`k` neighbours.
//!      Higher density ⇒ the input is more "central" within its cluster.
//!   6. **Anomalies** — inputs whose top-1 similarity to any peer is below
//!      [`KnnConfig::anomaly_threshold`].  These are isolated outliers.
//!   7. **Clusters** — connected components of the **k-NN graph**: edge
//!      `i↔j` exists when `j ∈ top-k(i)` *or* `i ∈ top-k(j)`.  Anomalies
//!      are excluded from clustering, so nothing pulls an isolated outlier
//!      into a real cluster via a single asymmetric edge.
//!   8. **Representatives** — the highest-density member of each cluster
//!      (the most "central" input within the cluster).
//!
//! The result is returned as a `serde_json::Value` for ergonomic embedding
//! in JSON-RPC responses, dashboards, and pipelines.
//!
//! ## Output shape
//!
//! ```json
//! {
//!   "n_logs":             37,
//!   "k":                  5,
//!   "anomaly_threshold":  0.2,
//!   "n_clusters":         3,
//!   "n_anomalies":        4,
//!   "clusters": [
//!     {
//!       "id":   0,
//!       "size": 14,
//!       "representative": { "idx": 2, "text": "...", "density": 0.83 },
//!       "members": [
//!         { "idx": 2, "text": "...", "density": 0.83 },
//!         { "idx": 7, "text": "...", "density": 0.79 }
//!       ]
//!     }
//!   ],
//!   "anomalies": [
//!     { "idx": 31, "text": "...", "max_similarity": 0.04 }
//!   ],
//!   "representatives": [
//!     { "idx": 2, "text": "...", "density": 0.83, "cluster": 0 }
//!   ]
//! }
//! ```
//!
//! Member and anomaly lists are capped via [`KnnConfig::max_cluster_members`]
//! and [`KnnConfig::max_anomalies`] so the JSON stays bounded for very large
//! corpora; `size` and `n_anomalies` always reflect the true counts.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use std::collections::{HashMap, HashSet};

// ── public configuration ─────────────────────────────────────────────────────

/// Tuning knobs for [`knn_summary_with`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnnConfig {
    /// Number of nearest neighbours to consider per input.  Clamped to
    /// `min(k, n - 1)` at runtime so it never exceeds the corpus size.
    /// Default: `5`.
    pub k: usize,

    /// Tokens shorter than this many characters are discarded before scoring.
    /// Default: `2`.
    pub min_word_len: usize,

    /// Inputs whose top-1 cosine similarity to any peer is below this
    /// threshold are flagged as anomalies (no usable nearest neighbour).
    /// Range `[0.0, 1.0]`. Default: `0.2`.
    pub anomaly_threshold: f32,

    /// Maximum number of cluster members included in the JSON output (per
    /// cluster).  The full cluster `size` is always reported separately.
    /// Default: `10`.
    pub max_cluster_members: usize,

    /// Maximum number of anomalies included in the JSON output, sorted from
    /// most-isolated to least-isolated.  `n_anomalies` always reflects the
    /// true total. Default: `20`.
    pub max_anomalies: usize,
}

impl Default for KnnConfig {
    fn default() -> Self {
        Self {
            k: 5,
            min_word_len: 2,
            anomaly_threshold: 0.2,
            max_cluster_members: 10,
            max_anomalies: 20,
        }
    }
}

// ── public API ───────────────────────────────────────────────────────────────

/// Run k-NN intelligence over `logs` using the default [`KnnConfig`].
///
/// Returns a structured JSON document with clusters, anomalies, and
/// representatives — see the module-level docs for the output shape.
pub fn knn_summary(logs: &[String]) -> JsonValue {
    knn_summary_with(logs, &KnnConfig::default())
}

/// Like [`knn_summary`] but accepts a caller-provided [`KnnConfig`].
pub fn knn_summary_with(logs: &[String], cfg: &KnnConfig) -> JsonValue {
    let n = logs.len();
    // Effective k: clamped to `[1, n - 1]` for the corpus we actually have.
    // Reported in the JSON so callers can tell when their requested `k`
    // was reduced (e.g. when the corpus is smaller than `cfg.k`).
    let k_eff = if n <= 1 { 0 } else { cfg.k.min(n - 1).max(1) };

    if n == 0 {
        return json!({
            "n_logs":            0,
            "k":                 k_eff,
            "anomaly_threshold": cfg.anomaly_threshold,
            "n_clusters":        0,
            "n_anomalies":       0,
            "clusters":          [],
            "anomalies":         [],
            "representatives":   [],
        });
    }

    if n == 1 {
        return json!({
            "n_logs":            1,
            "k":                 k_eff,
            "anomaly_threshold": cfg.anomaly_threshold,
            "n_clusters":        1,
            "n_anomalies":       0,
            "clusters": [{
                "id":   0,
                "size": 1,
                "representative": { "idx": 0, "text": logs[0], "density": 1.0 },
                "members": [{ "idx": 0, "text": logs[0], "density": 1.0 }],
            }],
            "anomalies":         [],
            "representatives":   [{ "idx": 0, "text": logs[0], "density": 1.0, "cluster": 0 }],
        });
    }

    // Tokenise → TF-IDF.  An empty global vocabulary degenerates to "every
    // input is an anomaly" since there's no signal to compare on.
    let bags: Vec<Vec<String>> = logs.iter().map(|s| tokenize(s, cfg.min_word_len)).collect();
    let tfidf = match build_tfidf(&bags) {
        Some(v) => v,
        None    => return all_anomalies_response(logs, cfg, k_eff),
    };

    // L2 norms for cosine similarity. Sorted-key summation keeps the
    // result reproducible across runs (HashMap iteration order would
    // otherwise leak last-bit nondeterminism into every downstream stat).
    let norms: Vec<f32> = tfidf
        .iter()
        .map(|v| {
            let mut keys: Vec<&String> = v.keys().collect();
            keys.sort_unstable();
            let mut sum = 0.0f32;
            for k in keys {
                if let Some(x) = v.get(k) {
                    sum += x * x;
                }
            }
            sum.sqrt()
        })
        .collect();

    // Pairwise cosine similarity matrix.  We only fill the upper triangle
    // and mirror, halving the work.
    //
    // Determinism note: floating-point addition is not associative, so the
    // summation order of `dot` must be deterministic across runs.  The
    // HashMap iteration order in `tfidf[i]` is randomised by `RandomState`,
    // which would otherwise leak last-bit nondeterminism into similarities,
    // densities, and representative selection.  We sort the keys of the
    // smaller bag before iterating to make the order reproducible.
    let mut sim = vec![vec![0.0f32; n]; n];
    for i in 0..n {
        for j in (i + 1)..n {
            // Iterate over the smaller bag to minimise hashmap probes.
            let (a, b) = if tfidf[i].len() <= tfidf[j].len() {
                (&tfidf[i], &tfidf[j])
            } else {
                (&tfidf[j], &tfidf[i])
            };
            let mut keys: Vec<&String> = a.keys().collect();
            keys.sort_unstable();
            let mut dot = 0.0f32;
            for term in keys {
                if let (Some(wa), Some(wb)) = (a.get(term), b.get(term)) {
                    dot += wa * wb;
                }
            }
            let s = dot / (norms[i].max(1e-12) * norms[j].max(1e-12));
            sim[i][j] = s;
            sim[j][i] = s;
        }
    }

    // For every row, find the top-k neighbour indices (excluding self) and
    // record the top-1 similarity (used for anomaly detection) and the
    // average top-k similarity (the input's "density" / centrality).
    let mut neighbours: Vec<Vec<usize>> = Vec::with_capacity(n);
    let mut top1_sim = vec![0.0f32; n];
    let mut density  = vec![0.0f32; n];

    for i in 0..n {
        let mut pairs: Vec<(usize, f32)> = (0..n)
            .filter(|&j| j != i)
            .map(|j| (j, sim[i][j]))
            .collect();
        pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        top1_sim[i] = pairs.first().map(|p| p.1).unwrap_or(0.0);

        let topk: Vec<usize> = pairs.iter().take(k_eff).map(|(j, _)| *j).collect();
        let topk_avg: f32 =
            pairs.iter().take(k_eff).map(|(_, s)| *s).sum::<f32>() / k_eff as f32;
        density[i] = topk_avg;
        neighbours.push(topk);
    }

    // Anomaly set: inputs whose closest neighbour is below the threshold.
    let anomaly_set: HashSet<usize> = (0..n)
        .filter(|&i| top1_sim[i] < cfg.anomaly_threshold)
        .collect();

    // k-NN graph clustering via union-find: union(i, j) when j is in
    // top-k(i), regardless of whether i is also in top-k(j).  This treats
    // the directed k-NN graph as undirected, so dense corpora (where every
    // pair has near-tied similarities and top-k membership is partly
    // arbitrary) still merge into a single connected component instead of
    // fragmenting along arbitrary tie-break boundaries.  Anomalies are
    // excluded so a single asymmetric edge cannot pull a true outlier into
    // a real cluster.
    let mut uf = UnionFind::new(n);
    for i in 0..n {
        if anomaly_set.contains(&i) {
            continue;
        }
        for &j in &neighbours[i] {
            if anomaly_set.contains(&j) {
                continue;
            }
            uf.union(i, j);
        }
    }

    // Bucket non-anomalies by their union-find root.
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        if anomaly_set.contains(&i) {
            continue;
        }
        let root = uf.find(i);
        groups.entry(root).or_default().push(i);
    }

    // Sort clusters: largest first; ties broken by representative density.
    let mut clusters: Vec<(usize, Vec<usize>)> = groups.into_iter().collect();
    clusters.sort_by(|a, b| {
        let by_size = b.1.len().cmp(&a.1.len());
        if by_size != std::cmp::Ordering::Equal {
            return by_size;
        }
        let da = a.1.iter().copied().map(|i| density[i])
            .fold(f32::MIN, f32::max);
        let db = b.1.iter().copied().map(|i| density[i])
            .fold(f32::MIN, f32::max);
        db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut clusters_json: Vec<JsonValue> = Vec::with_capacity(clusters.len());
    let mut representatives_json: Vec<JsonValue> = Vec::with_capacity(clusters.len());

    for (cid, (_root, members)) in clusters.iter().enumerate() {
        // Pick the densest member as the cluster representative.
        let rep = *members
            .iter()
            .max_by(|&&a, &&b| {
                density[a].partial_cmp(&density[b]).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(&members[0]);

        // Sort visible members by density descending and cap.
        let mut sorted_members = members.clone();
        sorted_members.sort_by(|&a, &b| {
            density[b].partial_cmp(&density[a]).unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted_members.truncate(cfg.max_cluster_members);

        let members_json: Vec<JsonValue> = sorted_members
            .iter()
            .map(|&i| {
                json!({
                    "idx":     i,
                    "text":    logs[i],
                    "density": density[i],
                })
            })
            .collect();

        clusters_json.push(json!({
            "id":   cid,
            "size": members.len(),
            "representative": {
                "idx":     rep,
                "text":    logs[rep],
                "density": density[rep],
            },
            "members": members_json,
        }));

        representatives_json.push(json!({
            "idx":     rep,
            "text":    logs[rep],
            "density": density[rep],
            "cluster": cid,
        }));
    }

    // Anomalies sorted from most-isolated (lowest top-1 similarity) to least.
    let mut anomalies_sorted: Vec<usize> = anomaly_set.iter().copied().collect();
    anomalies_sorted.sort_by(|&a, &b| {
        top1_sim[a].partial_cmp(&top1_sim[b]).unwrap_or(std::cmp::Ordering::Equal)
    });
    let n_anomalies = anomalies_sorted.len();
    anomalies_sorted.truncate(cfg.max_anomalies);

    let anomalies_json: Vec<JsonValue> = anomalies_sorted
        .iter()
        .map(|&i| {
            json!({
                "idx":            i,
                "text":           logs[i],
                "max_similarity": top1_sim[i],
            })
        })
        .collect();

    json!({
        "n_logs":            n,
        "k":                 k_eff,
        "anomaly_threshold": cfg.anomaly_threshold,
        "n_clusters":        clusters_json.len(),
        "n_anomalies":       n_anomalies,
        "clusters":          clusters_json,
        "anomalies":         anomalies_json,
        "representatives":   representatives_json,
    })
}

// ── degenerate case: empty global vocabulary ─────────────────────────────────

fn all_anomalies_response(logs: &[String], cfg: &KnnConfig, k_eff: usize) -> JsonValue {
    let n = logs.len();
    let take = cfg.max_anomalies.min(n);
    let anomalies: Vec<JsonValue> = (0..take)
        .map(|i| {
            json!({
                "idx":            i,
                "text":           logs[i],
                "max_similarity": 0.0,
            })
        })
        .collect();
    json!({
        "n_logs":            n,
        "k":                 k_eff,
        "anomaly_threshold": cfg.anomaly_threshold,
        "n_clusters":        0,
        "n_anomalies":       n,
        "clusters":          [],
        "anomalies":         anomalies,
        "representatives":   [],
    })
}

// ── union-find ───────────────────────────────────────────────────────────────

struct UnionFind {
    parent: Vec<usize>,
    rank:   Vec<u8>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self { parent: (0..n).collect(), rank: vec![0; n] }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }

    fn union(&mut self, a: usize, b: usize) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra == rb {
            return;
        }
        if self.rank[ra] < self.rank[rb] {
            self.parent[ra] = rb;
        } else if self.rank[ra] > self.rank[rb] {
            self.parent[rb] = ra;
        } else {
            self.parent[rb] = ra;
            self.rank[ra] += 1;
        }
    }
}

// ── TF-IDF helpers (kept self-contained — same shape as lsa.rs / textrank.rs) ─

/// Build smoothed TF-IDF vectors for every input.
///
/// Returns `None` when the global vocabulary is empty (every input was
/// composed entirely of stop-words or below-`min_word_len` tokens).
fn build_tfidf(bags: &[Vec<String>]) -> Option<Vec<HashMap<String, f32>>> {
    let n = bags.len();

    // Document frequency: how many inputs contain each term.
    let mut df: HashMap<&str, usize> = HashMap::new();
    for bag in bags {
        let mut seen: HashSet<&str> = HashSet::new();
        for w in bag {
            if seen.insert(w.as_str()) {
                *df.entry(w.as_str()).or_insert(0) += 1;
            }
        }
    }

    if df.is_empty() {
        return None;
    }

    let mut vecs: Vec<HashMap<String, f32>> = Vec::with_capacity(n);
    for bag in bags {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for w in bag {
            *counts.entry(w.as_str()).or_insert(0) += 1;
        }
        let total = bag.len().max(1) as f32;
        let mut v: HashMap<String, f32> = HashMap::new();
        for (term, cnt) in &counts {
            let tf = *cnt as f32 / total;
            let doc_freq = *df.get(term).unwrap_or(&1);
            // Smoothed IDF: log((N + 1) / (df + 1)) + 1
            let idf = ((n + 1) as f32 / (doc_freq + 1) as f32).ln() + 1.0;
            v.insert(term.to_string(), tf * idf);
        }
        vecs.push(v);
    }

    Some(vecs)
}

fn tokenize(s: &str, min_word_len: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        if !cur.is_empty() {
            if cur.len() >= min_word_len && !STOPWORDS.contains(&cur.as_str()) {
                out.push(cur.clone());
            }
            cur.clear();
        }
    };
    for c in s.chars() {
        if c.is_alphanumeric() {
            for lc in c.to_lowercase() {
                cur.push(lc);
            }
        } else {
            flush(&mut cur, &mut out);
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// Compact English stop-word list — same set used by the `lsa` and
/// `textrank` modules so all three rank vocabulary identically.
const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "has", "have",
    "he", "in", "is", "it", "its", "of", "on", "or", "that", "the", "this", "to",
    "was", "were", "will", "with", "but", "not", "no", "we", "you", "they", "them",
    "their", "there", "what", "which", "when", "where", "who", "why", "how", "all",
    "any", "been", "being", "do", "does", "did", "can", "could", "should", "would",
    "may", "might", "must", "shall", "if", "then", "than", "so", "such", "into",
    "out", "up", "down", "over", "under", "about", "after", "before", "between",
    "during", "while", "i", "me", "my", "your", "yours", "his", "her", "hers",
    "us", "our", "ours",
];
