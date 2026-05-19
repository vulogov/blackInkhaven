//! N-gram intelligence over a list of strings.
//!
//! Two complementary endpoints, both built on the same primitive — the
//! document-frequency distribution of every n-gram across the corpus:
//!
//! - [`ngram_anomaly`] — flags lines whose n-grams are statistically rare
//!   in the corpus.  High **mean rarity** ⇒ the line uses uncommon phrases
//!   ⇒ candidate anomaly.
//!
//! - [`ngram_remove_noise`] — splits a corpus into "kept" (signal) and
//!   "removed" (noise) lines based on **mean commonness**.  A line whose
//!   n-grams are heavily-repeated across the corpus is contributing
//!   nothing new and is classified as noise.
//!
//! The two are duals: `rarity = 1 − commonness` per n-gram.  Anomaly
//! detection looks for lines at the upper end of the rarity distribution;
//! noise removal looks for lines at the upper end of the commonness
//! distribution.  Lines in the middle are normal traffic — present in
//! both `kept` (denoised) and *not* in `anomalies`.
//!
//! ## Tokenisation
//!
//! Tokens are lowercased alphanumeric runs.  Tokens shorter than
//! [`NgramAnomalyConfig::min_word_len`] / [`NgramNoiseConfig::min_word_len`]
//! are dropped.  **Stop-words are NOT filtered** — n-grams derive their
//! signal from phrase structure, and stop-word phrases like `"the system"`
//! or `"is the"` carry meaningful template information.  This is a
//! deliberate departure from the other `bdslib::analysis::*` modules
//! (LSA, TextRank, k-NN), which all filter stop-words because they treat
//! tokens as independent signals.
//!
//! ## Output
//!
//! Both functions return `serde_json::Value` for direct embedding in
//! JSON-RPC responses.  Member arrays are bounded by config so the JSON
//! stays compact for very large corpora; `n_anomalies` / `n_kept` /
//! `n_removed` always report the true totals.
//!
//! See `Documentation/Algorithm/NGRAM_ANOMALY.md` and
//! `Documentation/Algorithm/NGRAM_NOISE.md` for the long-form derivations.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use std::collections::{HashMap, HashSet};

// ── public configuration ────────────────────────────────────────────────────

/// Tuning knobs for [`ngram_anomaly_with`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NgramAnomalyConfig {
    /// N-gram length.  `2` (bigrams) is a robust default for log lines and
    /// short structured text; `1` (unigrams) is essentially "rare-word
    /// detection"; `3` (trigrams) catches phrase-level uniqueness at the
    /// cost of much higher per-line miss rates on short inputs.
    /// Default: `2`.
    pub n: usize,

    /// Tokens shorter than this many characters are discarded before
    /// n-gram construction.  Default: `2`.
    pub min_word_len: usize,

    /// Mean-rarity cutoff for anomaly classification.  Range `[0, 1]`.
    /// Higher ⇒ stricter; only the most strikingly-novel lines are flagged.
    /// Lower ⇒ more permissive.  Default: `0.7`.
    pub anomaly_threshold: f32,

    /// Maximum number of anomalies included in the JSON output, sorted
    /// from most-rare to least-rare.  `n_anomalies` always reports the
    /// true total.  Default: `20`.
    pub max_anomalies: usize,

    /// Maximum number of "novel n-grams" included with each anomaly entry,
    /// sorted by ascending document frequency (rarest first).  Default: `5`.
    pub max_novel_ngrams: usize,
}

impl Default for NgramAnomalyConfig {
    fn default() -> Self {
        Self {
            n: 2,
            min_word_len: 2,
            anomaly_threshold: 0.7,
            max_anomalies: 20,
            max_novel_ngrams: 5,
        }
    }
}

/// Tuning knobs for [`ngram_remove_noise_with`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NgramNoiseConfig {
    /// N-gram length.  Default: `2`.
    pub n: usize,

    /// Tokens shorter than this many characters are discarded before
    /// n-gram construction.  Default: `2`.
    pub min_word_len: usize,

    /// Mean-commonness cutoff for noise classification.  Range `[0, 1]`.
    /// Higher ⇒ less aggressive; only lines made of *very* repetitive
    /// n-grams are removed.  Lower ⇒ more aggressive denoising.
    /// Default: `0.85`.
    pub noise_threshold: f32,

    /// Maximum number of "kept" lines included in the JSON output (sample
    /// view in original input order).  `n_kept` always reports the true
    /// total.  Default: `100`.
    pub max_kept: usize,

    /// Maximum number of "removed" (noise) lines included in the JSON
    /// output, sorted from most-noise-like to least.  `n_removed` always
    /// reports the true total.  Default: `100`.
    pub max_removed: usize,
}

impl Default for NgramNoiseConfig {
    fn default() -> Self {
        Self {
            n: 2,
            min_word_len: 2,
            noise_threshold: 0.85,
            max_kept: 100,
            max_removed: 100,
        }
    }
}

// ── public API: anomaly detection ───────────────────────────────────────────

/// Run n-gram anomaly detection over `logs` with the default config.
///
/// Returns a structured JSON document — see the module-level docs and
/// [`NGRAM_ANOMALY.md`](../../Documentation/Algorithm/NGRAM_ANOMALY.md)
/// for the output shape.
pub fn ngram_anomaly(logs: &[String]) -> JsonValue {
    ngram_anomaly_with(logs, &NgramAnomalyConfig::default())
}

/// Like [`ngram_anomaly`] but with a caller-provided [`NgramAnomalyConfig`].
pub fn ngram_anomaly_with(logs: &[String], cfg: &NgramAnomalyConfig) -> JsonValue {
    let n_logs = logs.len();
    let n = cfg.n.max(1);

    if n_logs == 0 {
        return json!({
            "n_logs":            0,
            "n":                 n,
            "n_unique_ngrams":   0,
            "anomaly_threshold": cfg.anomaly_threshold,
            "n_anomalies":       0,
            "mean_rarity":       0.0,
            "anomalies":         [],
        });
    }

    // Tokenise and build per-line n-gram bags.
    let bags: Vec<Vec<String>> = logs
        .iter()
        .map(|s| build_ngrams(&tokenize(s, cfg.min_word_len), n))
        .collect();

    // Document frequency: how many lines contain each n-gram.
    let df = build_doc_frequency(&bags);
    let total_lines = n_logs as f32;

    // Per-line rarity = mean(1 - df[g] / N) across the line's n-grams.
    // Lines with no n-grams (too short for n) score 0 — they carry no
    // signal so cannot be flagged as anomalies.
    let rarity = per_line_score(&bags, &df, total_lines, /*invert=*/ true);

    // Mean rarity across all non-empty lines (informational, not used in
    // the classification cut).  Sorted summation for determinism.
    let mean_rarity = mean_score(&rarity, &bags);

    // Anomalies: rarity at-or-above threshold, line has at least one
    // scorable n-gram.  Sort by rarity descending; ties broken by idx.
    let mut anomaly_idx: Vec<usize> = (0..n_logs)
        .filter(|&i| !bags[i].is_empty() && rarity[i] >= cfg.anomaly_threshold)
        .collect();
    anomaly_idx.sort_by(|&a, &b| {
        rarity[b]
            .partial_cmp(&rarity[a])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(&b))
    });
    let true_n_anomalies = anomaly_idx.len();
    anomaly_idx.truncate(cfg.max_anomalies);

    // Render each anomaly with its top-K rarest distinct n-grams.
    let anomalies_json: Vec<JsonValue> = anomaly_idx
        .iter()
        .map(|&i| {
            // Pair every n-gram in this line with its document frequency,
            // then sort: rarest first; ties broken alphabetically for
            // deterministic output.
            let mut grams: Vec<(&str, f32)> = bags[i]
                .iter()
                .map(|g| {
                    let f = *df.get(g.as_str()).unwrap_or(&1) as f32 / total_lines;
                    (g.as_str(), f)
                })
                .collect();
            grams.sort_by(|a, b| {
                a.1.partial_cmp(&b.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.0.cmp(b.0))
            });

            // Deduplicate (a line can contain the same n-gram twice — only
            // its first appearance counts toward the novel list).
            let mut seen: HashSet<&str> = HashSet::new();
            let novel: Vec<&str> = grams
                .iter()
                .filter_map(|(g, _)| if seen.insert(g) { Some(*g) } else { None })
                .take(cfg.max_novel_ngrams)
                .collect();

            json!({
                "idx":          i,
                "text":         logs[i],
                "rarity":       rarity[i],
                "novel_ngrams": novel,
            })
        })
        .collect();

    json!({
        "n_logs":            n_logs,
        "n":                 n,
        "n_unique_ngrams":   df.len(),
        "anomaly_threshold": cfg.anomaly_threshold,
        "n_anomalies":       true_n_anomalies,
        "mean_rarity":       mean_rarity,
        "anomalies":         anomalies_json,
    })
}

// ── public API: noise removal ───────────────────────────────────────────────

/// Run n-gram noise removal over `logs` with the default config.
///
/// Returns a structured JSON document with `kept` and `removed` arrays —
/// see the module-level docs and
/// [`NGRAM_NOISE.md`](../../Documentation/Algorithm/NGRAM_NOISE.md) for
/// the output shape.
pub fn ngram_remove_noise(logs: &[String]) -> JsonValue {
    ngram_remove_noise_with(logs, &NgramNoiseConfig::default())
}

/// Like [`ngram_remove_noise`] but with a caller-provided [`NgramNoiseConfig`].
pub fn ngram_remove_noise_with(logs: &[String], cfg: &NgramNoiseConfig) -> JsonValue {
    let n_logs = logs.len();
    let n = cfg.n.max(1);

    if n_logs == 0 {
        return json!({
            "n_logs":          0,
            "n":               n,
            "n_unique_ngrams": 0,
            "noise_threshold": cfg.noise_threshold,
            "n_kept":          0,
            "n_removed":       0,
            "kept":            [],
            "removed":         [],
        });
    }

    let bags: Vec<Vec<String>> = logs
        .iter()
        .map(|s| build_ngrams(&tokenize(s, cfg.min_word_len), n))
        .collect();
    let df = build_doc_frequency(&bags);
    let total_lines = n_logs as f32;

    // Per-line commonness = mean(df[g] / N) across the line's n-grams.
    let commonness = per_line_score(&bags, &df, total_lines, /*invert=*/ false);

    // Classify: noise iff (line has n-grams) AND (commonness >= threshold).
    // Lines too short to produce any n-gram cannot be noise — we have no
    // basis to remove them, so they go in `kept`.
    let mut kept_idx:    Vec<usize> = Vec::new();
    let mut removed_idx: Vec<usize> = Vec::new();
    for i in 0..n_logs {
        if !bags[i].is_empty() && commonness[i] >= cfg.noise_threshold {
            removed_idx.push(i);
        } else {
            kept_idx.push(i);
        }
    }

    let true_n_kept    = kept_idx.len();
    let true_n_removed = removed_idx.len();

    // Kept preserves input order (so the user can read the denoised
    // corpus in sequence); removed sorted from most-noise-like first.
    kept_idx.truncate(cfg.max_kept);
    removed_idx.sort_by(|&a, &b| {
        commonness[b]
            .partial_cmp(&commonness[a])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(&b))
    });
    removed_idx.truncate(cfg.max_removed);

    let kept_json: Vec<JsonValue> = kept_idx
        .iter()
        .map(|&i| {
            json!({
                "idx":        i,
                "text":       logs[i],
                "commonness": commonness[i],
            })
        })
        .collect();
    let removed_json: Vec<JsonValue> = removed_idx
        .iter()
        .map(|&i| {
            json!({
                "idx":        i,
                "text":       logs[i],
                "commonness": commonness[i],
            })
        })
        .collect();

    json!({
        "n_logs":          n_logs,
        "n":               n,
        "n_unique_ngrams": df.len(),
        "noise_threshold": cfg.noise_threshold,
        "n_kept":          true_n_kept,
        "n_removed":       true_n_removed,
        "kept":            kept_json,
        "removed":         removed_json,
    })
}

// ── internal helpers ────────────────────────────────────────────────────────

/// Per-line score: mean of `f(gram)` across the line's n-grams, where
/// `f(g) = (1 - df[g] / N)` when `invert == true` (rarity), else
/// `f(g) = df[g] / N` (commonness).
///
/// Iterating sorted-by-key keeps the floating-point sum reproducible
/// across runs (HashMap iteration would otherwise leak last-bit
/// non-determinism into the per-line scores).
fn per_line_score(
    bags: &[Vec<String>],
    df: &HashMap<String, usize>,
    total_lines: f32,
    invert: bool,
) -> Vec<f32> {
    let n_logs = bags.len();
    let mut score = vec![0.0f32; n_logs];
    for (i, bag) in bags.iter().enumerate() {
        if bag.is_empty() {
            continue;
        }
        // Sort the n-gram references; stable summation order matters for
        // determinism but does not change the mathematical result.
        let mut sorted_grams: Vec<&str> = bag.iter().map(String::as_str).collect();
        sorted_grams.sort_unstable();
        let mut sum = 0.0f32;
        for g in &sorted_grams {
            let f = *df.get(*g).unwrap_or(&1) as f32 / total_lines;
            sum += if invert { 1.0 - f } else { f };
        }
        score[i] = sum / sorted_grams.len() as f32;
    }
    score
}

/// Mean of `score` across non-empty lines, with sorted-summation for
/// reproducibility.  Returns `0.0` when no line contributed.
fn mean_score(score: &[f32], bags: &[Vec<String>]) -> f32 {
    let mut vals: Vec<f32> = score
        .iter()
        .zip(bags.iter())
        .filter_map(|(s, bag)| if !bag.is_empty() { Some(*s) } else { None })
        .collect();
    if vals.is_empty() {
        return 0.0;
    }
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let sum: f32 = vals.iter().sum();
    sum / vals.len() as f32
}

/// Tokenise: lowercase alphanumeric runs, dropping tokens shorter than
/// `min_word_len`.  Unlike LSA / TextRank / k-NN, **no stop-word
/// filtering** — n-grams derive their signal from phrase structure.
fn tokenize(s: &str, min_word_len: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        if !cur.is_empty() {
            if cur.len() >= min_word_len {
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

/// Build the contiguous-`n`-gram list for a token sequence.
///
/// Returns an empty vec when `tokens.len() < n` — the line carries no
/// n-gram signal at this `n`.
fn build_ngrams(tokens: &[String], n: usize) -> Vec<String> {
    if tokens.len() < n {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(tokens.len() - n + 1);
    for i in 0..=(tokens.len() - n) {
        // Join with a single space — tokens are pure alphanumeric, so the
        // space is a guaranteed-unique separator (no collision with any
        // token character).
        out.push(tokens[i..i + n].join(" "));
    }
    out
}

/// Document frequency: for each n-gram, how many lines contain it (a
/// line counts once even if it contains the gram multiple times).
fn build_doc_frequency(bags: &[Vec<String>]) -> HashMap<String, usize> {
    let mut df: HashMap<String, usize> = HashMap::new();
    for bag in bags {
        let mut seen: HashSet<&str> = HashSet::new();
        for g in bag {
            if seen.insert(g.as_str()) {
                *df.entry(g.clone()).or_insert(0) += 1;
            }
        }
    }
    df
}
