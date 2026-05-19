//! TextRank extractive summarisation.
//!
//! Given a list of strings — sentences, log lines, JSON fingerprints, or
//! anything else with a meaningful word distribution — `textrank_summary`
//! returns a short summary built from the highest-ranked inputs.
//!
//! The implementation follows the classic Mihalcea & Tarau formulation:
//!
//! 1. Tokenise each input into a bag of words (lowercase alphanumeric tokens,
//!    stop-words removed, very short tokens dropped).
//! 2. Build an undirected weighted similarity graph between inputs using
//!    cosine similarity on term-frequency vectors.
//! 3. Run weighted PageRank on the graph until scores converge or
//!    [`TextRankConfig::iters`] iterations have run.
//! 4. Pick the top-`k` inputs by PageRank score and return them in their
//!    original input order, joined by spaces.
//!
//! This module is deliberately self-contained: it depends only on `std` plus
//! a tiny embedded English stop-word list, so it can be reused later for
//! summarising clusters of log JSON fingerprints without dragging in NLP
//! crates.
//!
//! ```
//! use bdslib::analysis::textrank::textrank_summary;
//!
//! let lines = [
//!     "The system started successfully.",
//!     "An unrelated heartbeat occurred.",
//!     "The system started successfully again.",
//! ].iter().map(|s| s.to_string()).collect::<Vec<_>>();
//!
//! let summary = textrank_summary(&lines);
//! assert!(!summary.is_empty());
//! ```

use serde::{Deserialize, Serialize};

/// Tuning knobs for the TextRank summariser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextRankConfig {
    /// Hard cap on the number of inputs included in the summary.
    /// Set to `0` to derive the cap from [`Self::ratio`] instead.
    /// Default: `0` (auto).
    pub max_sentences: usize,

    /// When [`Self::max_sentences`] is `0`, this fraction of inputs is kept,
    /// rounded up, with a minimum of `1`.  Clamped to `(0.0, 1.0]`.
    /// Default: `0.3` (≈ one third).
    pub ratio: f32,

    /// Tokens shorter than this many characters are discarded before scoring.
    /// Default: `2`.
    pub min_word_len: usize,

    /// PageRank damping factor.  Standard TextRank value is `0.85`.
    pub damping: f32,

    /// Maximum PageRank iterations.  The loop also exits early once the
    /// L1-norm change between iterations drops below [`Self::tolerance`].
    /// Default: `30`.
    pub iters: usize,

    /// Convergence threshold for the per-iteration L1-norm change in scores.
    /// Default: `1e-4`.
    pub tolerance: f32,
}

impl Default for TextRankConfig {
    fn default() -> Self {
        Self {
            max_sentences: 0,
            ratio: 0.3,
            min_word_len: 2,
            damping: 0.85,
            iters: 30,
            tolerance: 1e-4,
        }
    }
}

/// Extractive summary of `inputs` using default [`TextRankConfig`].
///
/// Returns a single string whose contents are the highest-ranked inputs joined
/// by a single space, in their original input order.
///
/// Edge cases:
/// - Empty input → empty string.
/// - Single input → that input verbatim.
/// - Inputs with no scorable tokens (e.g. only stop-words) → the first input
///   is returned as a graceful fallback.
pub fn textrank_summary(inputs: &[String]) -> String {
    textrank_summary_with(inputs, &TextRankConfig::default())
}

/// Like [`textrank_summary`] but with a caller-provided configuration.
pub fn textrank_summary_with(inputs: &[String], cfg: &TextRankConfig) -> String {
    if inputs.is_empty() {
        return String::new();
    }
    if inputs.len() == 1 {
        return inputs[0].clone();
    }

    let ranked = textrank_rank(inputs, cfg);
    let k = target_count(inputs.len(), cfg);

    // Pick top-k by score, then re-sort by original index so the summary
    // preserves the natural reading order of the inputs.
    let mut top: Vec<usize> = ranked.iter().take(k).map(|(idx, _)| *idx).collect();
    top.sort_unstable();

    if top.is_empty() {
        return inputs[0].clone();
    }

    top.iter()
        .map(|i| inputs[*i].as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// PageRank scores for every input, sorted by score descending.
///
/// Exposed for tests and callers that need the ranking itself rather than the
/// joined summary string.  Returned tuples are `(input_index, score)`.
pub fn textrank_rank(inputs: &[String], cfg: &TextRankConfig) -> Vec<(usize, f32)> {
    let n = inputs.len();
    if n == 0 {
        return vec![];
    }
    if n == 1 {
        return vec![(0, 1.0)];
    }

    let bags: Vec<Vec<String>> = inputs.iter().map(|s| tokenize(s, cfg.min_word_len)).collect();

    // Cosine similarity matrix on term-frequency bags.
    let mut sim = vec![vec![0.0f32; n]; n];
    for i in 0..n {
        for j in (i + 1)..n {
            let s = cosine_sim(&bags[i], &bags[j]);
            sim[i][j] = s;
            sim[j][i] = s;
        }
    }

    // Row-normalise to a stochastic transition matrix.  Rows of zero
    // similarity (an input that overlaps with nothing else) are spread
    // uniformly so PageRank stays well-defined.
    let mut trans = vec![vec![0.0f32; n]; n];
    for i in 0..n {
        let row_sum: f32 = sim[i].iter().sum();
        if row_sum > 0.0 {
            for j in 0..n {
                trans[i][j] = sim[i][j] / row_sum;
            }
        } else {
            let uniform = 1.0 / n as f32;
            for j in 0..n {
                trans[i][j] = uniform;
            }
        }
    }

    // Weighted PageRank.
    let damping = cfg.damping.clamp(0.0, 1.0);
    let teleport = (1.0 - damping) / n as f32;
    let mut score = vec![1.0f32 / n as f32; n];
    let mut next = vec![0.0f32; n];
    for _ in 0..cfg.iters.max(1) {
        for j in 0..n {
            let mut sum = 0.0f32;
            for i in 0..n {
                sum += trans[i][j] * score[i];
            }
            next[j] = teleport + damping * sum;
        }
        let delta: f32 = score.iter().zip(next.iter()).map(|(a, b)| (a - b).abs()).sum();
        std::mem::swap(&mut score, &mut next);
        if delta < cfg.tolerance {
            break;
        }
    }

    let mut ranked: Vec<(usize, f32)> = score.into_iter().enumerate().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    ranked
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn target_count(n: usize, cfg: &TextRankConfig) -> usize {
    if cfg.max_sentences > 0 {
        return cfg.max_sentences.min(n);
    }
    let ratio = cfg.ratio.clamp(f32::MIN_POSITIVE, 1.0);
    let k = ((n as f32 * ratio).ceil() as usize).max(1);
    k.min(n)
}

fn tokenize(s: &str, min_word_len: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let push = |cur: &mut String, out: &mut Vec<String>| {
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
            push(&mut cur, &mut out);
        }
    }
    push(&mut cur, &mut out);
    out
}

fn cosine_sim(a: &[String], b: &[String]) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    use std::collections::HashMap;
    let mut counts_a: HashMap<&str, u32> = HashMap::new();
    let mut counts_b: HashMap<&str, u32> = HashMap::new();
    for w in a {
        *counts_a.entry(w.as_str()).or_insert(0) += 1;
    }
    for w in b {
        *counts_b.entry(w.as_str()).or_insert(0) += 1;
    }
    let mut dot: f32 = 0.0;
    for (w, ca) in &counts_a {
        if let Some(cb) = counts_b.get(w) {
            dot += (*ca as f32) * (*cb as f32);
        }
    }
    let mag_a: f32 = counts_a.values().map(|c| (*c as f32).powi(2)).sum::<f32>().sqrt();
    let mag_b: f32 = counts_b.values().map(|c| (*c as f32).powi(2)).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }
    dot / (mag_a * mag_b)
}

/// Compact English stop-word list. Intentionally short — TextRank is robust
/// to a generous stop-word list, but we want to keep enough vocabulary to
/// score short log-style inputs meaningfully.
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
