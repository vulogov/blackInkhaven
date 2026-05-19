//! Latent Semantic Analysis (LSA) extractive summarisation.
//!
//! Given a list of strings — sentences, log lines, JSON fingerprints, or any
//! text with a meaningful word distribution — [`lsa_summary`] returns a short
//! summary built from the most semantically representative inputs.
//!
//! ## Algorithm
//!
//! 1. **Tokenise** each input into a bag of words (lowercase alphanumeric
//!    tokens, stop-words removed, very short tokens dropped).
//! 2. **TF-IDF** — compute a smoothed TF-IDF weight for every (term, sentence)
//!    pair.  Documents with broader vocabulary automatically exert more
//!    influence on the concept space than stop-word-only documents.
//! 3. **Gram matrix** — compute B = AᵀA, the (n_sentences × n_sentences)
//!    matrix of pairwise TF-IDF dot products.  Its eigenvectors are exactly
//!    the right singular vectors of the term–sentence matrix A; its eigenvalues
//!    are the squared singular values σ²ₖ.
//! 4. **Truncated eigen-decomposition** — extract the top-`n_concepts`
//!    eigenpairs of B via power iteration with Gram–Schmidt deflation.
//! 5. **Sentence scoring** — follow Steinberger & Ježek (2004):
//!    `score[j] = √(Σₖ λₖ · v_k[j]²)`, where (λₖ, v_k) are the eigenpairs.
//!    Intuitively: a sentence scores high when it contributes strongly to many
//!    important latent concepts.
//! 6. **Selection** — keep the top-*m* sentences by score and return them in
//!    their original input order, joined by a single space.
//!
//! ## References
//!
//! * Steinberger, J., & Ježek, K. (2004).  *Using Latent Semantic Analysis in
//!   Text Summarization and Summary Evaluation.*  Proc. ISIM '04.
//!
//! ## Example
//!
//! ```
//! use bdslib::analysis::lsa::lsa_summary;
//!
//! let lines: Vec<String> = [
//!     "The authentication service timed out repeatedly.",
//!     "An unrelated heartbeat occurred.",
//!     "The authentication service timed out again.",
//! ]
//! .iter()
//! .map(|s| s.to_string())
//! .collect();
//!
//! let summary = lsa_summary(&lines);
//! assert!(!summary.is_empty());
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── configuration ─────────────────────────────────────────────────────────────

/// Tuning knobs for the LSA summariser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LsaConfig {
    /// Hard cap on the number of inputs included in the summary.
    /// Set to `0` to derive the cap from [`Self::ratio`] instead.
    /// Default: `0` (auto).
    pub max_sentences: usize,

    /// When [`Self::max_sentences`] is `0`, this fraction of inputs is kept,
    /// rounded up, with a minimum of `1`.  Clamped to `(0.0, 1.0]`.
    /// Default: `0.3`.
    pub ratio: f32,

    /// Tokens shorter than this many characters are discarded before scoring.
    /// Default: `2`.
    pub min_word_len: usize,

    /// Number of LSA concepts (singular vectors) to extract.  Higher values
    /// capture more subtle themes at the cost of extra computation.
    /// Clamped to `(n_sentences − 1)` to avoid rank overflow.
    /// Default: `3`.
    pub n_concepts: usize,

    /// Power-iteration steps per eigenvector.  50 is sufficient for all
    /// practical input sizes (< a few thousand sentences).
    /// Default: `50`.
    pub power_iters: usize,
}

impl Default for LsaConfig {
    fn default() -> Self {
        Self {
            max_sentences: 0,
            ratio: 0.3,
            min_word_len: 2,
            n_concepts: 3,
            power_iters: 50,
        }
    }
}

// ── public API ────────────────────────────────────────────────────────────────

/// Extractive summary of `inputs` using default [`LsaConfig`].
///
/// Returns a single string whose contents are the highest-ranked inputs joined
/// by a single space, in their original input order.
///
/// Edge cases:
/// - Empty input → empty string.
/// - Single input → that input verbatim.
/// - Inputs with no scorable tokens (only stop-words) → first input as fallback.
pub fn lsa_summary(inputs: &[String]) -> String {
    lsa_summary_with(inputs, &LsaConfig::default())
}

/// Like [`lsa_summary`] but with a caller-provided configuration.
pub fn lsa_summary_with(inputs: &[String], cfg: &LsaConfig) -> String {
    if inputs.is_empty() {
        return String::new();
    }
    if inputs.len() == 1 {
        return inputs[0].clone();
    }

    let ranked = lsa_rank(inputs, cfg);
    let k = target_count(inputs.len(), cfg);

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

/// LSA scores for every input, sorted by score descending.
///
/// Returns `(input_index, score)` pairs.  Exposed for tests and callers that
/// need the full ranking rather than only the joined summary string.
pub fn lsa_rank(inputs: &[String], cfg: &LsaConfig) -> Vec<(usize, f32)> {
    let n = inputs.len();
    if n == 0 {
        return vec![];
    }
    if n == 1 {
        return vec![(0, 1.0)];
    }

    let bags: Vec<Vec<String>> = inputs
        .iter()
        .map(|s| tokenize(s, cfg.min_word_len))
        .collect();

    // TF-IDF vectors (sparse, one per sentence).
    let tfidf = match build_tfidf(&bags) {
        Some(v) => v,
        None => {
            // No scorable vocabulary — fallback: uniform rank.
            let uniform = 1.0 / n as f32;
            return (0..n).map(|i| (i, uniform)).collect();
        }
    };

    // Gram matrix B = AᵀA: B[i][j] = tfidf_i · tfidf_j
    let mut gram = build_gram(&tfidf, n);

    // Clamp n_concepts to achievable rank.
    let k = cfg.n_concepts.max(1).min(n.saturating_sub(1).max(1));

    // Top-k eigenpairs via power iteration with deflation.
    let eigenpairs = top_eigenvecs(&mut gram, k, cfg.power_iters, n);

    if eigenpairs.is_empty() {
        let uniform = 1.0 / n as f32;
        return (0..n).map(|i| (i, uniform)).collect();
    }

    // Steinberger-Ježek score: √(Σₖ λₖ · v_k[j]²)
    let mut scores: Vec<f32> = (0..n)
        .map(|j| {
            let sq_sum: f32 = eigenpairs
                .iter()
                .map(|(lambda, v)| lambda * v[j] * v[j])
                .sum();
            sq_sum.max(0.0).sqrt()
        })
        .collect();

    // Guard against all-zero scores (degenerate input).
    let total: f32 = scores.iter().sum();
    if total == 0.0 {
        let uniform = 1.0 / n as f32;
        scores = vec![uniform; n];
    }

    let mut ranked: Vec<(usize, f32)> = scores.into_iter().enumerate().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    ranked
}

// ── internal helpers ──────────────────────────────────────────────────────────

/// Build smoothed TF-IDF vectors for every sentence.
///
/// Returns `None` when the global vocabulary is empty (all stop-words / no tokens).
fn build_tfidf(bags: &[Vec<String>]) -> Option<Vec<HashMap<String, f32>>> {
    let n = bags.len();

    // Document frequency: how many sentences contain each term.
    let mut df: HashMap<&str, usize> = HashMap::new();
    for bag in bags {
        let mut seen: HashMap<&str, bool> = HashMap::new();
        for w in bag {
            if *seen.entry(w.as_str()).or_insert(false) {
                continue;
            }
            seen.insert(w.as_str(), true);
            *df.entry(w.as_str()).or_insert(0) += 1;
        }
    }

    if df.is_empty() {
        return None;
    }

    // For each sentence: TF (normalized) × smoothed IDF.
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

/// Compute the **centred** n×n similarity matrix B where
/// `B[i][j] = cosine_sim(tfidf_i, tfidf_j)` for i ≠ j and `B[i][i] = 0`.
///
/// Two design choices combine here:
///
/// * **L2-normalisation** (cosine) removes the effect of vector magnitude.
///   Without it, sentences with unique high-IDF terms dominate because their
///   unnormalised dot-product with themselves is large.
///
/// * **Zero diagonal** (centred Gram / cross-similarity matrix) puts sentences
///   that share no vocabulary with any other sentence into the null space of B.
///   Without it, an isolated sentence owns its own unit-norm eigenvector
///   (eigenvalue 1), which always outscores cluster members whose shared-concept
///   contribution is split across `cluster_size` sentences.
///
/// Together these choices ensure that the dominant eigenvectors reflect genuine
/// thematic clusters, and that isolated or off-topic sentences score 0 rather
/// than 1.
fn build_gram(tfidf: &[HashMap<String, f32>], n: usize) -> Vec<Vec<f32>> {
    let norms: Vec<f32> = tfidf
        .iter()
        .map(|v| v.values().map(|x| x * x).sum::<f32>().sqrt())
        .collect();

    let mut gram = vec![vec![0.0f32; n]; n];
    for i in 0..n {
        for j in (i + 1)..n {
            let mut dot = 0.0f32;
            for (term, wi) in &tfidf[i] {
                if let Some(wj) = tfidf[j].get(term) {
                    dot += wi * wj;
                }
            }
            let ni = norms[i].max(1e-12);
            let nj = norms[j].max(1e-12);
            let sim = dot / (ni * nj);
            gram[i][j] = sim;
            gram[j][i] = sim;
            // diagonal stays 0 — self-similarity is not included
        }
    }
    gram
}

/// Extract `k` dominant eigenpairs of the symmetric n×n matrix `b` via power
/// iteration with Gram–Schmidt deflation.
///
/// Modifies `b` in place (deflation removes each extracted component).
/// Returns `(eigenvalue, eigenvector)` pairs in descending eigenvalue order.
fn top_eigenvecs(
    b: &mut Vec<Vec<f32>>,
    k: usize,
    power_iters: usize,
    n: usize,
) -> Vec<(f32, Vec<f32>)> {
    let mut result: Vec<(f32, Vec<f32>)> = Vec::with_capacity(k);

    for component in 0..k {
        // Initialise with a deterministic non-uniform vector so the first
        // iteration is unlikely to be orthogonal to the dominant eigenvector.
        let mut v: Vec<f32> = (0..n).map(|i| 1.0 + 0.1 * (i + component) as f32).collect();
        normalise_in_place(&mut v);

        for _ in 0..power_iters {
            let bv = mat_vec(b, &v, n);
            let norm = vec_norm(&bv);
            if norm < 1e-12 {
                break;
            }
            let new_v: Vec<f32> = bv.into_iter().map(|x| x / norm).collect();
            // Convergence: compare dot product (cos ≈ 1 for same direction,
            // allowing for sign flips).
            let cos: f32 = dot(&new_v, &v).abs();
            v = new_v;
            if cos > 1.0 - 1e-8 {
                break;
            }
        }

        // Rayleigh quotient: λ = vᵀBv
        let bv = mat_vec(b, &v, n);
        let lambda = dot(&v, &bv);

        if lambda < 1e-10 {
            break; // remaining eigenvalues are negligible
        }

        // Deflate: B ← B − λ · v · vᵀ
        for i in 0..n {
            for j in 0..n {
                b[i][j] -= lambda * v[i] * v[j];
            }
        }

        result.push((lambda, v));
    }

    result
}

// ── small linear-algebra helpers (no extra crate required) ────────────────────

fn mat_vec(m: &[Vec<f32>], v: &[f32], n: usize) -> Vec<f32> {
    (0..n).map(|i| (0..n).map(|j| m[i][j] * v[j]).sum()).collect()
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

fn vec_norm(v: &[f32]) -> f32 {
    dot(v, v).sqrt()
}

fn normalise_in_place(v: &mut Vec<f32>) {
    let norm = vec_norm(v);
    if norm > 1e-12 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

fn target_count(n: usize, cfg: &LsaConfig) -> usize {
    if cfg.max_sentences > 0 {
        return cfg.max_sentences.min(n);
    }
    let ratio = cfg.ratio.clamp(f32::MIN_POSITIVE, 1.0);
    ((n as f32 * ratio).ceil() as usize).max(1).min(n)
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

/// Compact English stop-word list shared with the `textrank` module.
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
