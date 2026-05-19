# Latent Semantic Analysis summarisation (`bdslib::analysis::lsa`)

`lsa_summary(&[String]) -> String` is bdslib's LSA-based extractive
summariser. It picks a small subset of the input strings — the ones
that contribute most to the dominant *latent concepts* in the corpus —
and returns them concatenated in original input order.

This is the SVD-based companion to TextRank's PageRank approach. They
share the same input shape, the same tokeniser, and the same output
contract; they differ in how "central" is measured.

This document covers:

1. [What problem LSA solves here](#1-what-problem-lsa-solves-here)
2. [The classical LSA algorithm](#2-the-classical-lsa-algorithm)
3. [How bdslib uses LSA](#3-how-bdslib-uses-lsa)
4. [The full pipeline, step by step](#4-the-full-pipeline-step-by-step)
5. [Output contract](#5-output-contract)
6. [Configuration knobs](#6-configuration-knobs)
7. [Complexity and scaling](#7-complexity-and-scaling)
8. [Determinism guarantees](#8-determinism-guarantees)
9. [Worked examples](#9-worked-examples)
10. [Failure modes and edge cases](#10-failure-modes-and-edge-cases)
11. [References](#11-references)

---

## 1. What problem LSA solves here

You have a batch of short text snippets — log lines, JSON fingerprints,
sentences — and you want a **short summary that captures the dominant
themes**. TextRank does this with random walks on a similarity graph;
LSA does it with linear algebra on a TF-IDF matrix.

LSA's edge: it makes "themes" explicit. Where TextRank tells you "this
sentence is well-connected to other sentences", LSA tells you "this
sentence contributes strongly to *concept #1*, which itself accounts for
most of the variance in the corpus". For multi-theme corpora this is the
more principled signal — you can ask for `n_concepts` topics and trust
that LSA isolates the strongest `n` of them.

The goal in this module is not full topic modelling — for that, see
LDA. LSA here is used for **extractive summarisation**: given a corpus,
pick the inputs that, summed up, span the dominant latent concepts.

---

## 2. The classical LSA algorithm

LSA was introduced by Deerwester et al. (1990) for information retrieval.
The recipe:

1. Build a term-document matrix `A` where `A[i][j] = weight(term i, doc j)`.
2. Take its **truncated singular value decomposition**:

   ```
   A ≈ U_k · Σ_k · V_kᵀ
   ```

   where `Σ_k = diag(σ₁, σ₂, …, σ_k)` keeps only the `k` largest
   singular values. The columns of `U_k` are *concept directions* in
   term-space; the columns of `V_k` are *concept directions* in
   document-space; the singular values quantify how much variance each
   concept explains.
3. Use the `k`-dimensional embedding for downstream tasks (similarity
   retrieval, classification, summarisation).

For **summarisation**, Steinberger & Ježek (2004) gave a clean scoring
rule on top of the SVD:

```
score(j) = √( Σ_k  σ_k² · v_k[j]² )
```

— a sentence (column `j`) scores high when it has large coefficients in
many high-eigenvalue concepts. Pick the top-scoring sentences as the
summary.

bdslib computes the SVD via the **Gram trick**: instead of factoring `A`
directly, it factors `B = AᵀA`, an `n × n` matrix whose eigenpairs
`(λ_k, v_k)` are exactly the squared singular values `σ_k²` and the
right singular vectors of `A`. This avoids implementing a thin SVD
explicitly — power iteration with deflation gives the top eigenpairs of
a symmetric positive semi-definite matrix in `O(k · iters · n²)`.

Two implementation details matter:

- **Cosine normalisation.** The Gram matrix uses cosine similarity, not
  raw dot products. Without it, sentences with unique high-IDF terms
  dominate because their unnormalised self-products are large.
- **Centred Gram (zero diagonal).** The diagonal of the Gram matrix is
  set to 0. Otherwise, an isolated sentence (no shared vocabulary with
  anyone else) owns its own unit-norm eigenvector with eigenvalue 1,
  which always outscores cluster members whose contribution is split
  across the cluster. With diagonal=0, isolated sentences land in the
  null space of `B` and score 0; cluster members exclusively drive the
  dominant eigenvectors.

These two choices together turn the textbook LSA score into a robust
log-summarisation signal — the "café" outlier in the Unicode test cannot
beat a "système" cluster member, regardless of vocabulary uniqueness.

---

## 3. How bdslib uses LSA

The implementation is self-contained — no `ndarray`, no `nalgebra`, no
external SVD library. Power iteration with Gram-Schmidt deflation is
50 lines of Rust and converges in a few dozen iterations on the matrix
sizes typical for log batches (hundreds to thousands of inputs).

The pipeline produces three signals from the SVD:

| Signal | Definition | Used for |
|---|---|---|
| `λ_k` (eigenvalue of `B`) | squared singular value `σ_k²` of the term-sentence matrix | concept importance weight |
| `v_k[j]` | sentence `j`'s coefficient in concept `k` | per-sentence contribution |
| `score[j]` | `√(Σ_k λ_k · v_k[j]²)` | sentence ranking signal |

The summariser then picks the top-`m` sentences by score and returns
them in original input order.

---

## 4. The full pipeline, step by step

Given an input slice `inputs: &[String]` of length `n` and an
`LsaConfig`:

### Step 1 — Tokenise

Same as TextRank and k-NN: lowercase alphanumeric tokens, stop-words
removed, tokens shorter than `cfg.min_word_len` dropped.

```rust
fn tokenize(s: &str, min_word_len: usize) -> Vec<String> { ... }
```

The result is a `Vec<Vec<String>>`: one bag-of-words per input.

### Step 2 — TF-IDF weights

For each input we build a sparse `HashMap<String, f32>`:

**Document frequency**: how many inputs contain each term.

```
df[term] = |{ i : term ∈ bag[i] }|
```

**Smoothed inverse document frequency**:

```
idf[term] = ln((N + 1) / (df[term] + 1)) + 1
```

The "+1" smoothing ensures `idf > 0` even for terms that appear in every
document, and prevents division by zero for terms absent from the entire
corpus (which would not happen here, but is the standard form).

**Normalised term frequency**:

```
tf[i][term] = count[i][term] / |bag[i]|
```

**TF-IDF weight**:

```
w[i][term] = tf[i][term] · idf[term]
```

If the global vocabulary is empty (every bag is empty after stop-word
filtering), the function returns the first input as a graceful fallback
— there is no signal to factor.

### Step 3 — Centred cosine Gram matrix

The Gram matrix `B = AᵀA` of the term-sentence matrix has entries

```
B[i][j] = Σ_t A[t][i] · A[t][j]
```

— the unnormalised dot product of sentences `i` and `j`. bdslib computes
the **centred** version:

```
B[i][i] = 0                               (zero diagonal)
B[i][j] = cos(w[i], w[j])    for i ≠ j     (cosine similarity)
```

```rust
fn build_gram(tfidf: &[HashMap<String, f32>], n: usize) -> Vec<Vec<f32>> {
    let norms: Vec<f32> = tfidf.iter()
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
            // diagonal stays 0 — self-similarity not included
        }
    }
    gram
}
```

This is the most important design decision in the module. Two reasons:

- **L2 normalisation** removes the magnitude bias that would let a
  sentence with a single highly-discriminative term dominate.
- **Zero diagonal** removes the "I love myself" eigenvector. A sentence
  with no shared vocabulary used to own a unit-norm eigenvector with
  eigenvalue 1 and outscore real cluster members; with diagonal=0 it
  joins the null space of `B` instead.

Without these two together, the Steinberger-Ježek score works on
synthetic textbook corpora but fails on real log batches where one stray
line has unique tokens.

### Step 4 — Truncated eigen-decomposition (power iteration with deflation)

Extract the top `k = min(cfg.n_concepts, n - 1)` eigenpairs of `B`.

For each component:

1. **Initialise** with a deterministic non-uniform vector — `v[i] = 1 +
   0.1 · (i + component)` — so the first iteration is unlikely to be
   orthogonal to the dominant eigenvector.

   ```rust
   let mut v: Vec<f32> = (0..n)
       .map(|i| 1.0 + 0.1 * (i + component) as f32)
       .collect();
   normalise_in_place(&mut v);
   ```

2. **Power iterate.** Compute `Bv`, normalise, and repeat. Convergence
   is detected by the cosine of the angle between successive iterates
   (with a sign-flip allowance, since power iteration may flip sign at
   each step):

   ```rust
   for _ in 0..power_iters {
       let bv = mat_vec(b, &v, n);
       let norm = vec_norm(&bv);
       if norm < 1e-12 { break; }
       let new_v: Vec<f32> = bv.into_iter().map(|x| x / norm).collect();
       let cos: f32 = dot(&new_v, &v).abs();
       v = new_v;
       if cos > 1.0 - 1e-8 { break; }
   }
   ```

3. **Compute the eigenvalue** via the Rayleigh quotient `λ = vᵀBv`.

   ```rust
   let bv = mat_vec(b, &v, n);
   let lambda = dot(&v, &bv);
   if lambda < 1e-10 { break; }    // remaining eigenvalues negligible
   ```

4. **Deflate** by subtracting the rank-1 projection so the next iteration
   converges to the next-largest eigenvalue:

   ```
   B ← B - λ · v · vᵀ
   ```

   ```rust
   for i in 0..n {
       for j in 0..n {
           b[i][j] -= lambda * v[i] * v[j];
       }
   }
   ```

The `λ < 1e-10` early exit catches the negative eigenvalues that show
up after the dominant cluster eigenvalues have been deflated out — the
centred Gram matrix is no longer positive semi-definite once you remove
the cluster components, and any remaining negative eigenvalues
contribute no useful concept information.

### Step 5 — Steinberger-Ježek scoring

For each sentence `j`, compute

```
score[j] = √( Σ_k  λ_k · v_k[j]² )
```

```rust
let mut scores: Vec<f32> = (0..n).map(|j| {
    let sq_sum: f32 = eigenpairs.iter()
        .map(|(lambda, v)| lambda * v[j] * v[j])
        .sum();
    sq_sum.max(0.0).sqrt()
}).collect();
```

A sentence scores high when it has large coefficients (`v_k[j]²`) in
many high-eigenvalue (`λ_k`) concepts. The `.max(0.0)` guards against
negative residuals from the deflation step.

If every score is exactly zero (degenerate input — every sentence is in
the null space of `B`), fall back to a uniform distribution.

### Step 6 — Pick top-m and restore reading order

Same as TextRank: sort by score descending, take the top `m =
target_count(n, cfg)`, then re-sort by original input index so the
summary reads naturally:

```rust
let mut top: Vec<usize> = ranked.iter().take(k).map(|(idx, _)| *idx).collect();
top.sort_unstable();   // restore input order

top.iter()
    .map(|i| inputs[*i].as_str())
    .collect::<Vec<_>>()
    .join(" ")
```

Where `target_count` is the same `max_sentences > 0 ? min(max_sentences, n) :
max(ceil(n · ratio), 1)` rule.

---

## 5. Output contract

`lsa_summary` returns a `String` — chosen inputs concatenated with single
spaces, in **original input order**. Identical contract to `textrank_summary`.

`lsa_rank` returns `Vec<(usize, f32)>` — `(input_index, score)` pairs,
sorted by score descending.

Edge-case guarantees (all in the test suite):

| Input | Behaviour |
|---|---|
| `[]` | `""` |
| `[only_one]` | the single input verbatim |
| All-stop-word inputs | the first input as fallback |
| `cfg.n_concepts` ≥ `n` | clamped to `n - 1` |
| Identical duplicates | rank near-tied; the picker grabs as many as fit in the cap |

---

## 6. Configuration knobs

```rust
pub struct LsaConfig {
    pub max_sentences: usize,  // default: 0 (auto via ratio)
    pub ratio:         f32,    // default: 0.3
    pub min_word_len:  usize,  // default: 2
    pub n_concepts:    usize,  // default: 3
    pub power_iters:   usize,  // default: 50
}
```

| Knob | Effect |
|---|---|
| **`max_sentences`** | Hard cap on summary length when > 0; otherwise `ratio` decides. |
| **`ratio`** | Fraction of inputs kept when `max_sentences == 0`. Clamped to `(0.0, 1.0]`. |
| **`min_word_len`** | Drops short tokens before TF-IDF. |
| **`n_concepts`** | Number of latent concepts (top eigenpairs) to extract. Higher captures more subtle themes at the cost of `O(n_concepts · n²)` extra work. Clamped to `n - 1`. The default 3 is a good general choice; raise to 5–10 for very heterogeneous corpora. |
| **`power_iters`** | Maximum iterations per eigenvector. 50 is sufficient for all practical input sizes — convergence usually fires in 10–30. Raise only if you hit pathological matrices. |

---

## 7. Complexity and scaling

For a corpus of size `n`, average bag size `m`, and `k = n_concepts`:

| Phase | Cost |
|---|---|
| Tokenise | `O(n · m)` |
| Build TF-IDF | `O(n · m)` |
| Build Gram matrix | `O(n² · m̄)` where `m̄` is the average shared-vocabulary size |
| Power iteration with deflation | `O(k · power_iters · n²)` |
| Score and sort | `O(k · n + n log n)` |

The Gram matrix construction `O(n² · m̄)` and the eigen-decomposition
`O(k · power_iters · n²)` dominate. For the standard config (`k = 3`,
`power_iters = 50`), eigen-decomposition cost is roughly `150 · n²`
floating-point operations — well within sub-second budgets up to several
thousand inputs.

Memory:

| Structure | Size |
|---|---|
| TF-IDF maps | `O(n · m)` |
| Gram matrix | `O(n²)` `f32` cells |
| Eigenvectors | `O(k · n)` |

The Gram matrix is mutated in place during deflation — no second copy.

---

## 8. Determinism guarantees

`lsa_summary` is deterministic given:

- A deterministic tokeniser (alphabetical iteration of input chars — yes).
- A deterministic init vector for power iteration (`1 + 0.1·(i +
  component)` — yes).
- A deterministic similarity matrix.

The current implementation does not sort TF-IDF keys before computing
Gram dot products, so HashMap iteration order can introduce last-bit
floating-point differences across runs. In practice this rarely changes
the chosen top-m (the score gaps between picked and unpicked sentences
are wide), but if you need byte-for-byte determinism, the same fix as
in `knn.rs` applies: sort keys before iterating in the dot-product loop.

Power-iteration convergence:

- The centred Gram matrix is symmetric (B = Bᵀ) but **not** positive
  semi-definite — the zero diagonal pushes some eigenvalues negative.
  Power iteration finds the eigenvalue with the largest absolute value
  first, then deflation removes it.
- After all positive cluster eigenvalues are extracted, deflation leaves
  only negative eigenvalues, which trigger the `λ < 1e-10` early exit.
  This is the right stopping condition: any remaining concept would have
  zero or negative weight in the Steinberger-Ježek score and contribute
  nothing useful.
- Geometric convergence rate is `λ_{k+1} / λ_k`. For typical TF-IDF
  Gram matrices the gap is wide, so power iteration converges in 10–30
  steps per component.

---

## 9. Worked examples

### Example A — operational log burst

Same 7-line burst as the TextRank example. With default config:

- Concept 1 (largest eigenvalue) corresponds to the "upstream timeout"
  cluster; the four 503 lines have large `v_1[j]` coefficients and
  dominate the score.
- Concept 2 picks up the secondary "service" axis — the rate-limit line
  and the 503 lines all share `service=auth`.
- Concept 3 fades into noise (negligible eigenvalue).

The `score[j] = √(Σ_k λ_k v_k[j]²)` puts the four 503 lines at the top,
and the auto-sized `ratio = 0.3` keeps three of them. LSA's output
matches TextRank's on this input — the two algorithms agree on the
dominant theme.

### Example B — multi-theme corpus

Six lines: 4 disk-failure lines + 2 unrelated noise lines.

- Concept 1: disk-failure axis. The four lines load on it.
- Concept 2: noise axis. The two unrelated lines load on it.
- The Steinberger-Ježek score puts the four disk-failure lines on top
  because `λ_1 ≫ λ_2`.

With `max_sentences = 2`, LSA picks two of the four disk-failure lines.
The unrelated noise stays out, exactly as it does for TextRank.

### Example C — heterogeneous corpus

A corpus where two themes are roughly balanced in size — e.g., 5
network errors and 5 disk errors, no clear dominant theme.

- `n_concepts = 1`: forces LSA to pick one theme. Returns lines from
  the higher-eigenvalue cluster only.
- `n_concepts = 3` (default): both themes contribute. LSA picks
  representatives from each, reflecting the multi-modal structure of
  the corpus.

This `n_concepts` knob is LSA's main expressive advantage over TextRank
— you can ask explicitly for a "single dominant theme" or a "balanced
sample across themes" view.

---

## 10. Failure modes and edge cases

| Input | Behaviour |
|---|---|
| `[]` | `""` |
| `[one]` | the single input verbatim |
| All-stop-word inputs | empty global vocabulary → first input as fallback |
| `cfg.n_concepts` > `n - 1` | clamped to `n - 1` |
| `cfg.n_concepts == 0` | clamped to 1 |
| Disconnected corpus (no shared vocabulary anywhere) | all eigenvalues are zero in the centred Gram matrix; scores degenerate to uniform; the picker just picks the first `m` inputs |
| Identical duplicates | tied highest scores; picker takes them first (correct) |
| Very large `n` | The `O(n²)` Gram matrix is the bottleneck. Above ~5–10k inputs, switch to a sparse Lanczos solver |

The function never panics on user-supplied input.

---

## 11. References

- Deerwester, S., Dumais, S. T., Furnas, G. W., Landauer, T. K., &
  Harshman, R. (1990). *Indexing by Latent Semantic Analysis.* Journal
  of the American Society for Information Science, 41(6), 391–407 —
  the original LSA paper.
- Steinberger, J., & Ježek, K. (2004). *Using Latent Semantic Analysis
  in Text Summarization and Summary Evaluation.* Proc. ISIM '04 — the
  scoring rule `score(j) = √(Σ_k λ_k · v_k[j]²)` used here.
- Golub, G. H., & Van Loan, C. F. (2013). *Matrix Computations* (4th
  ed.), Johns Hopkins University Press — chapters 8 (the symmetric
  eigenvalue problem) and 10 (Lanczos and power iteration).
- Berry, M. W., Dumais, S. T., & O'Brien, G. W. (1995). *Using Linear
  Algebra for Intelligent Information Retrieval.* SIAM Review, 37(4),
  573–595 — applied LSA in IR settings, including the term-document
  matrix construction used here.
- Salton, G., & Buckley, C. (1988). *Term-weighting approaches in
  automatic text retrieval.* Information Processing & Management, 24(5),
  513–523 — the smoothed TF-IDF weighting scheme.

## See also

- [`Documentation/tests/lsa_test.md`](../tests/lsa_test.md) —
  every test case and what it verifies.
- [`Documentation/examples/lsa_demo.md`](../examples/lsa_demo.md) —
  runnable demo walkthrough.
- [`Documentation/Algorithm/TEXTRANK.md`](TEXTRANK.md) — the PageRank
  alternative with the same input/output contract. LSA is generally
  faster on large inputs and more expressive on multi-theme corpora;
  TextRank is simpler and has slightly tighter convergence guarantees.
- [`Documentation/Algorithm/KNN.md`](KNN.md) — k-NN clustering and
  anomaly detection. Complementary: use LSA to summarise, k-NN to
  cluster + flag outliers.
- `src/analysis/lsa.rs` — the implementation itself, ~415 lines.
