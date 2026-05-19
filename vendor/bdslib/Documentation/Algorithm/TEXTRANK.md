# TextRank extractive summarisation (`bdslib::analysis::textrank`)

`textrank_summary(&[String]) -> String` is bdslib's TextRank implementation
for picking the most representative inputs from a list of strings — log
lines, JSON fingerprints, sentences, anything tokenisable. The output is
a short summary built by concatenating the highest-ranked inputs in their
original input order.

This document covers:

1. [What problem TextRank solves here](#1-what-problem-textrank-solves-here)
2. [The classical TextRank algorithm](#2-the-classical-textrank-algorithm)
3. [How bdslib uses TextRank](#3-how-bdslib-uses-textrank)
4. [The full pipeline, step by step](#4-the-full-pipeline-step-by-step)
5. [Output contract](#5-output-contract)
6. [Configuration knobs](#6-configuration-knobs)
7. [Complexity and scaling](#7-complexity-and-scaling)
8. [Determinism and convergence](#8-determinism-and-convergence)
9. [Worked examples](#9-worked-examples)
10. [Failure modes and edge cases](#10-failure-modes-and-edge-cases)
11. [References](#11-references)

---

## 1. What problem TextRank solves here

You have a batch of short text snippets — say, the last hour of log
lines, or the bodies of every drain3-discovered template, or every
primary observability record matching a vector query. You want a
**short, readable summary** that surfaces the most representative items
without committing to topic modelling, supervised classifiers, or external
language models.

Classical extractive summarisation gives you exactly that: pick a small
subset of the inputs that, taken together, are "central" to the corpus.
TextRank is the canonical unsupervised, language-agnostic way to do it:

- No training data.
- No fixed vocabulary.
- No `k` to pre-decide (auto-sized by ratio, or capped by an explicit
  hard limit).
- Linear-algebra simple — converges in tens of milliseconds for typical
  log-batch sizes.

`textrank_summary` returns the chosen inputs as a single
space-separated string in original input order; `textrank_rank` returns
the full per-input ranking as `(input_index, score)` pairs sorted by
score descending.

---

## 2. The classical TextRank algorithm

TextRank was introduced by Mihalcea & Tarau (2004) as an adaptation of
Brin & Page's PageRank to text-summarisation tasks. The intuition is
direct: treat each sentence as a node in a graph, connect sentences
that "look alike" with weighted edges, and let PageRank's random-walk
score tell you which nodes are most central.

The mechanism in three lines:

```
graph     = (V = sentences, E = pairwise similarity edges, weights w_ij)
score(v)  = (1 - d) / |V| + d · Σ_{u ∈ neighbours(v)}  (w_uv / Σ_x w_ux) · score(u)
```

`d` is the **damping factor** (the probability of following an edge vs.
teleporting to a random node — standard value 0.85). `score(v)` is
computed iteratively until it converges.

A sentence wins a high score when it is **strongly connected to other
strongly-connected sentences**. PageRank's eigenvalue interpretation
gives the same intuition: the steady-state distribution of a random walk
on the graph concentrates on nodes that the walker visits often, which
are exactly the central / representative ones.

The weight function is implementation-defined. The original paper uses
word-overlap normalised by sentence length; bdslib uses cosine similarity
on bag-of-words term-frequency vectors, which is more sensitive to repeated
keywords and works just as well for short structured inputs (log lines,
JSON fingerprints).

---

## 3. How bdslib uses TextRank

The implementation is deliberately self-contained — no NLP crates, no
embedding model, no SVD. That keeps it cheap and dependency-free, so it
slots into hot paths like the live "summarise the recent logs" view in
bdsweb.

Three signals fall out of the construction:

| Signal | Definition | Used for |
|---|---|---|
| `sim[i][j]` | cosine similarity of TF vectors of inputs `i` and `j` | edge weights of the TextRank graph |
| `score[i]` | steady-state PageRank score | "how central is this input?" |
| top-k by score | the `k` highest-ranked inputs | the actual summary, re-sorted into input order |

The two thresholds you actually have to set are `damping` (almost
always 0.85, the standard) and the cap on summary length (`max_sentences`
or `ratio`). Everything else either follows from those or is a
convergence knob.

---

## 4. The full pipeline, step by step

Given an input slice `inputs: &[String]` of length `n` and a
`TextRankConfig`:

### Step 1 — Tokenise

Each input is normalised to lowercase, broken on non-alphanumeric
boundaries, and filtered to drop:

- Tokens shorter than `cfg.min_word_len` characters (default 2)
- Tokens in the built-in English stop-word list

```rust
fn tokenize(s: &str, min_word_len: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() {
            for lc in c.to_lowercase() { cur.push(lc); }
        } else {
            push(&mut cur, &mut out, min_word_len);
        }
    }
    push(&mut cur, &mut out, min_word_len);
    out
}
```

The result is a `Vec<Vec<String>>`: one bag-of-words per input. Note that
this is plain TF, not TF-IDF — TextRank historically uses term frequency
and the PageRank step does the global weighting via random walks, so
adding IDF would double-count the centrality signal.

### Step 2 — Pairwise cosine similarity

For every pair `(i, j)` with `i < j`, compute cosine similarity on the
term-frequency bags:

```
cos(a, b) = (a · b) / (‖a‖ · ‖b‖)
```

```rust
fn cosine_sim(a: &[String], b: &[String]) -> f32 {
    let mut counts_a: HashMap<&str, u32> = HashMap::new();
    let mut counts_b: HashMap<&str, u32> = HashMap::new();
    for w in a { *counts_a.entry(w.as_str()).or_insert(0) += 1; }
    for w in b { *counts_b.entry(w.as_str()).or_insert(0) += 1; }

    let mut dot = 0.0;
    for (w, ca) in &counts_a {
        if let Some(cb) = counts_b.get(w) {
            dot += (*ca as f32) * (*cb as f32);
        }
    }
    let mag_a = counts_a.values().map(|c| (*c as f32).powi(2)).sum::<f32>().sqrt();
    let mag_b = counts_b.values().map(|c| (*c as f32).powi(2)).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 { return 0.0; }
    dot / (mag_a * mag_b)
}
```

The result is filled into a symmetric `n × n` matrix `sim` with the upper
triangle computed once and mirrored. Diagonal entries stay 0 — a node
does not vote for itself in the PageRank graph.

### Step 3 — Row-normalise to a stochastic transition matrix

PageRank requires a stochastic matrix where each row sums to 1
(probability of transitioning out). For each row `i`:

```
trans[i][j] = sim[i][j] / Σ_x sim[i][x]    if Σ_x sim[i][x] > 0
            = 1 / n                         otherwise (no overlap with anyone)
```

The fallback is critical: a node with zero similarity to everyone else
would otherwise be a dead end, breaking the random-walk interpretation.
Spreading uniformly preserves the stochastic structure and keeps the
isolated node "visible" — but it gets only its share of the teleport
probability, so it ranks low.

### Step 4 — Weighted PageRank (power iteration)

Initialise every node to `1 / n` and run the update rule:

```
next[j] = (1 - d) / n + d · Σ_i  trans[i][j] · score[i]
```

```rust
let damping  = cfg.damping.clamp(0.0, 1.0);
let teleport = (1.0 - damping) / n as f32;
let mut score = vec![1.0f32 / n as f32; n];
let mut next  = vec![0.0f32; n];
for _ in 0..cfg.iters.max(1) {
    for j in 0..n {
        let mut sum = 0.0f32;
        for i in 0..n {
            sum += trans[i][j] * score[i];
        }
        next[j] = teleport + damping * sum;
    }
    let delta: f32 = score.iter().zip(next.iter())
        .map(|(a, b)| (a - b).abs()).sum();
    std::mem::swap(&mut score, &mut next);
    if delta < cfg.tolerance { break; }
}
```

Two stopping conditions, whichever fires first:

- A hard cap of `cfg.iters` iterations (default 30) — guarantees bounded
  runtime.
- The L1-norm change `Σ_i |score[i] − next[i]|` drops below
  `cfg.tolerance` (default 1e-4) — the standard PageRank convergence test.

For the kinds of similarity matrices TextRank produces, 30 iterations is
plenty. Most batches converge in under 10.

### Step 5 — Pick the top-k and restore reading order

Sort by score descending, take the top `k = target_count(n, cfg)`,
then **re-sort by original input index** so the output reads naturally:

```rust
let ranked = textrank_rank(inputs, cfg);   // (idx, score) sorted by score desc
let k      = target_count(n, cfg);
let mut top: Vec<usize> = ranked.iter().take(k).map(|(idx, _)| *idx).collect();
top.sort_unstable();                        // restore input order

top.iter()
    .map(|i| inputs[*i].as_str())
    .collect::<Vec<_>>()
    .join(" ")
```

`target_count` resolves the size of the summary:

```rust
fn target_count(n: usize, cfg: &TextRankConfig) -> usize {
    if cfg.max_sentences > 0 {
        return cfg.max_sentences.min(n);
    }
    let ratio = cfg.ratio.clamp(f32::MIN_POSITIVE, 1.0);
    ((n as f32 * ratio).ceil() as usize).max(1).min(n)
}
```

So `max_sentences > 0` is a hard cap; otherwise a fraction of the corpus
(default 30%, rounded up, minimum 1) is kept.

---

## 5. Output contract

`textrank_summary` returns a `String`:

```
"<input_a> <input_b> <input_c>"
```

— the chosen inputs concatenated with single spaces, in their **original
input order** (not score order). This makes the summary readable without
forcing the caller to re-stitch sentence ordering.

`textrank_rank` returns the full ranking:

```rust
Vec<(usize, f32)>
// sorted by score descending; index points back into `inputs`
```

Edge-case guarantees (also covered by the test suite):

| Input | Behaviour |
|---|---|
| `[]` | `""` |
| `[only_one]` | the single input verbatim |
| All-stop-word inputs | the **first input** as a graceful fallback |
| Inputs with disjoint vocabulary | every node is a sink → uniform fallback rows kick in, ranks come out near-uniform; the picker just falls through to top-k by tiny score differences |

---

## 6. Configuration knobs

```rust
pub struct TextRankConfig {
    pub max_sentences: usize,  // default: 0 (auto via ratio)
    pub ratio:         f32,    // default: 0.3
    pub min_word_len:  usize,  // default: 2
    pub damping:       f32,    // default: 0.85
    pub iters:         usize,  // default: 30
    pub tolerance:     f32,    // default: 1e-4
}
```

| Knob | Effect |
|---|---|
| **`max_sentences`** | When > 0, hard caps the number of inputs in the summary. Wins over `ratio`. |
| **`ratio`** | When `max_sentences == 0`, the fraction of inputs kept (rounded up, minimum 1). Clamped to `(0.0, 1.0]`. |
| **`min_word_len`** | Drops short tokens before computing similarity. Helps when log lines contain noisy short identifiers. |
| **`damping`** | Probability of "following an edge" instead of teleporting. Higher ⇒ rankings more sensitive to graph structure. The standard 0.85 has decades of empirical support; rarely worth touching. |
| **`iters`** | Hard ceiling on power-iteration steps. The convergence test usually fires earlier. Smaller values save time at the cost of slightly noisier scores. |
| **`tolerance`** | L1-norm threshold for early termination. Tighten it for higher-precision rankings (e.g. when ties matter); loosen it for faster convergence. |

---

## 7. Complexity and scaling

For a corpus of size `n` with average bag size `m` (unique tokens per
input):

| Phase | Cost |
|---|---|
| Tokenise | `O(n · m)` |
| Pairwise cosine similarity | `O(n² · m̄)` where `m̄` is the average shared-vocabulary size |
| Row normalisation | `O(n²)` |
| PageRank power iteration | `O(iters · n²)` per iteration, but typically converges in 10-15 |
| Sort and pick top-k | `O(n log n)` |

The dominant terms are `O(n² · m̄)` for similarity and `O(iters · n²)`
for power iteration. In practice TextRank handles **a few thousand
inputs in well under a second** on commodity hardware. Beyond that, the
similarity matrix becomes the bottleneck and you'd want to switch to a
sparse representation or an approximate nearest-neighbour cut.

Memory:

| Structure | Size |
|---|---|
| Token bags | `O(n · m)` |
| Similarity matrix | `O(n²)` `f32` cells |
| Transition matrix | `O(n²)` `f32` cells |

Both matrices are dense `Vec<Vec<f32>>` for cache locality.

---

## 8. Determinism and convergence

TextRank is deterministic given a deterministic similarity matrix and a
deterministic init vector. The implementation:

- Initialises every node to `1 / n` (deterministic).
- Computes cosine similarity from `HashMap`-backed bag counts. Since the
  numerator (`dot`) is always small enough that ULP-level rounding from
  iteration order rarely changes the chosen top-k, and since the
  PageRank update averages over many edges, the final ranking is stable
  in practice — but if you need byte-for-byte determinism across runs
  (e.g. for snapshot tests), sort the bag keys before iterating, the
  same way `knn.rs` does. The current implementation does not
  guarantee this; tests rely on the algorithm being *robust* to
  last-bit perturbations rather than exactly reproducible.

Convergence:

- The transition matrix is row-stochastic and aperiodic (the teleport
  term ensures every state has a self-loop in the limit), so PageRank
  has a unique steady-state distribution by the Perron-Frobenius theorem.
- Power iteration converges geometrically with rate `damping = 0.85`,
  which means the L1 error roughly halves every 4-5 iterations for
  typical similarity graphs.
- The implementation guards against pathological non-convergence with a
  hard cap of `cfg.iters` iterations. Non-convergence within that cap
  produces a usable but slightly noisy ranking — the relative order of
  high-score nodes is essentially correct even before full convergence.

---

## 9. Worked examples

### Example A — operational log burst

Input (7 lines):

```
2026-05-08T10:00:01 ERROR upstream timeout service=auth code=503
2026-05-08T10:00:02 ERROR upstream timeout service=billing code=503
2026-05-08T10:00:04 ERROR upstream timeout service=catalog code=503
2026-05-08T10:00:05 ERROR upstream timeout service=auth code=503
2026-05-08T10:00:07 INFO  worker started pid=4123
2026-05-08T10:00:08 WARN  rate limit exceeded service=auth code=429
2026-05-08T10:00:09 INFO  metrics flushed count=12
```

Default config (`ratio=0.3`, so `k = ceil(7 * 0.3) = 3`):

The `upstream timeout` lines all share the tokens `upstream`, `timeout`,
`error`, `service`, `503` — they form a tightly-connected sub-graph and
support each other's PageRank scores. The two `INFO` and one `WARN`
lines have only individual links to one or two timeout lines, so they
score lower. The summary returns three of the four timeout lines, in
their original input order.

### Example B — short paragraph

Input (7 sentences about distributed systems): the densest words are
`distributed`, `systems`, `consensus`, `algorithms`, `partitions`. The
sentence "Raft and Paxos are widely used consensus algorithms in
distributed systems." is the most central — it shares tokens with five
other sentences. With `max_sentences = 3`, the summary picks it plus
two more high-scoring ones, producing a coherent topic abstract.

### Example C — JSON fingerprints

Input (7 fingerprints with two recurring login patterns and isolated
heartbeats):

```
event=login user=alice ip=10.0.0.1 result=success
event=login user=alice ip=10.0.0.1 result=success
event=login user=bob   ip=10.0.0.2 result=failure reason=bad-password
event=login user=bob   ip=10.0.0.2 result=failure reason=bad-password
event=login user=bob   ip=10.0.0.2 result=failure reason=bad-password
event=heartbeat node=worker-3 status=ok
event=heartbeat node=worker-7 status=ok
```

With `max_sentences = 2`, TextRank picks two of the three identical Bob
failure lines — they are the largest connected sub-graph and share
exactly the same vocabulary, so their PageRank scores are tied and
maximal. The Alice success and the heartbeats are excluded.

This is the canonical "find the dominant cluster's representative
strings" use case, and it composes well with downstream LLM prompting:
the TextRank summary is small enough to fit in any context window while
preserving the operational essence of the burst.

---

## 10. Failure modes and edge cases

| Input | Behaviour |
|---|---|
| `[]` | Returns `""`. |
| `[one]` | Returns the single input verbatim. |
| All-stop-word inputs | Tokenisation produces empty bags; cosine similarities are all 0; row normalisation falls through to the uniform-row branch; PageRank converges to a uniform distribution; `textrank_summary` returns the **first input** as a graceful fallback. |
| `damping ≈ 0.0` | Almost all probability goes to teleport — every node ends up near `1/n`, ranking becomes meaningless. |
| `damping ≈ 1.0` | No teleport — disconnected components don't share probability and isolated nodes get zero score. |
| `tolerance = 0.0` | The early-exit never fires; the loop runs the full `iters` count. |
| Identical duplicates | Form a perfect-similarity sub-clique. They share the highest PageRank scores; the summary picks duplicates first (which is correct — they *are* the most representative line). |
| Very large `n` | The `O(n²)` similarity matrix is the bottleneck. Above ~5–10k inputs, switch to a sparse construction or pre-cluster with k-NN before TextRanking each cluster. |

The function never panics on user-supplied input.

---

## 11. References

- Mihalcea, R., & Tarau, P. (2004). *TextRank: Bringing Order into Text.*
  Proceedings of EMNLP 2004 — the original paper that adapted PageRank
  to extractive summarisation and keyword extraction.
- Brin, S., & Page, L. (1998). *The Anatomy of a Large-Scale Hypertextual
  Web Search Engine.* Computer Networks and ISDN Systems, 30(1-7),
  107-117 — the foundational PageRank paper.
- Page, L., Brin, S., Motwani, R., & Winograd, T. (1999). *The PageRank
  Citation Ranking: Bringing Order to the Web.* Stanford InfoLab.
- Salton, G. (1989). *Automatic Text Processing: The Transformation,
  Analysis, and Retrieval of Information by Computer.* Addison-Wesley —
  the canonical reference for cosine similarity on term-frequency
  vectors.
- Manning, C. D., Raghavan, P., & Schütze, H. (2008). *Introduction to
  Information Retrieval*, Cambridge University Press — chapters 6
  (term weighting) and 21 (link analysis), particularly §21.2 on
  PageRank's eigenvalue interpretation.

## See also

- [`Documentation/tests/textrank_test.md`](../tests/textrank_test.md) —
  every test case and what it verifies.
- [`Documentation/examples/textrank_demo.md`](../examples/textrank_demo.md) —
  runnable demo walkthrough.
- [`Documentation/Algorithm/LSA.md`](LSA.md) — the SVD-based alternative
  used in `bdslib::analysis::lsa`. Same input shape, different ranking
  signal; the two algorithms agree on dominant topics in unbalanced
  corpora.
- [`Documentation/Algorithm/KNN.md`](KNN.md) — k-NN clustering and
  anomaly detection over the same corpus shape; complements TextRank
  when you need cluster discovery + outlier flagging rather than just
  a representative summary.
- `src/analysis/textrank.rs` — the implementation itself, ~265 lines.
