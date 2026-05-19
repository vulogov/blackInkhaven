# k-Nearest-Neighbour intelligence (`bdslib::analysis::knn`)

`knn_summary(&[String]) -> serde_json::Value` is bdslib's k-Nearest-Neighbour
analysis pass over a list of strings. It produces a structured JSON document
summarising the corpus: which lines belong to which clusters, which lines
are isolated outliers (anomalies), and which line best represents each
cluster.

This document covers:

1. [What problem k-NN solves here](#1-what-problem-k-nn-solves-here)
2. [The classical k-NN algorithm](#2-the-classical-k-nn-algorithm)
3. [How bdslib uses k-NN as an unsupervised summariser](#3-how-bdslib-uses-k-nn-as-an-unsupervised-summariser)
4. [The full pipeline, step by step](#4-the-full-pipeline-step-by-step)
5. [Output JSON contract](#5-output-json-contract)
6. [Configuration knobs](#6-configuration-knobs)
7. [Complexity and scaling](#7-complexity-and-scaling)
8. [Determinism guarantees](#8-determinism-guarantees)
9. [Worked examples](#9-worked-examples)
10. [Failure modes and edge cases](#10-failure-modes-and-edge-cases)
11. [References](#11-references)

---

## 1. What problem k-NN solves here

You have a batch of short text snippets — log lines, JSON fingerprints,
template bodies, sentences. You want to know:

- **Which snippets are recurring patterns** (cluster together)?
- **Which snippets are one-offs** (do not match any pattern)?
- **What is the most representative example** of each pattern?

These are textbook unsupervised-learning questions, but you do not want
to commit to picking `k` clusters up front (as in k-means), or to tuning a
density threshold (as in DBSCAN), or to providing labelled training data
(as in supervised k-NN classification). You want a single function that
takes a corpus and returns a usable structured summary.

`knn_summary` adapts the classical k-NN distance-graph idea to this
unsupervised setting: each input finds its `k` most similar peers, and the
shape of the resulting graph yields clusters and anomalies "for free".

---

## 2. The classical k-NN algorithm

In its most familiar form, k-NN is a **lazy classifier**: given a labelled
training set and an unlabelled query point, find the `k` nearest training
points by some distance metric and assign the query the majority label.

```
classify(query, training_set, k):
    distances = [distance(query, t) for t in training_set]
    top_k     = argsort(distances)[:k]
    labels    = [t.label for t in top_k]
    return mode(labels)
```

Two things make k-NN special:

1. **No training step.** All "learning" happens at query time — every
   training example is implicitly a model.
2. **The distance metric is everything.** Choosing the right metric for
   the data (Euclidean for continuous coordinates, cosine for sparse text,
   Hamming for binary features, …) determines whether k-NN works at all.

For text, the canonical setup is:

- Represent each document as a **TF-IDF vector** — a sparse map
  `{ term → weight }` reflecting "how important is this term in this
  document compared to the corpus as a whole".
- Use **cosine similarity** as the distance: `cos(u, v) = (u · v) / (‖u‖ ‖v‖)`.
  Cosine ignores absolute magnitude, so a long log line and a short
  fingerprint can still match if they discuss the same concepts.

`knn_summary` uses exactly this representation, but it doesn't classify —
it uses the k-NN graph itself as a clustering signal, with anomaly
detection layered on top.

---

## 3. How bdslib uses k-NN as an unsupervised summariser

There is no labelled training set. Every input is both a query and a
training example: each line looks at every other line, picks its top-`k`
similar peers, and the resulting graph encodes the corpus structure.

Three signals fall out of this construction:

| Signal | Definition | Used for |
|---|---|---|
| `top1_sim[i]` | similarity to the closest peer | **anomaly detection** — if even the best peer is barely related, the line is an outlier |
| `density[i]` | average similarity to the top-`k` peers | **representative selection** — high density means "many of my neighbours look like me", i.e. I sit at the centre of a cluster |
| `top-k(i)` | indices of the `k` most similar peers, by descending similarity | **cluster discovery** — connected components of the union of these adjacency lists are the clusters |

The two thresholds you actually have to set (`k` and
`anomaly_threshold`) are the only knobs that matter. Everything else
either follows from those or is an output-bounding cap.

---

## 4. The full pipeline, step by step

Given an input slice `logs: &[String]` of length `n` and a `KnnConfig`:

### Step 1 — Tokenise

Each input is normalised to lowercase, broken on non-alphanumeric
boundaries, and filtered to drop:

- Tokens shorter than `cfg.min_word_len` characters
- Tokens in the built-in English stop-word list (`the`, `and`, `is`, …)

```rust
fn tokenize(s: &str, min_word_len: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() {
            for lc in c.to_lowercase() { cur.push(lc); }
        } else {
            flush(&mut cur, &mut out, min_word_len);
        }
    }
    flush(&mut cur, &mut out, min_word_len);
    out
}
```

The result is a `Vec<Vec<String>>`: one bag-of-words per input.

### Step 2 — TF-IDF

For each input we build a sparse `HashMap<String, f32>` of term weights.

**Document frequency**: how many inputs contain each term.

```
df[term] = |{ i : term ∈ bag[i] }|
```

**Term frequency** (per input): normalised by the bag length.

```
tf[i][term] = count[i][term] / |bag[i]|
```

**Smoothed inverse document frequency**: damps very common terms without
exploding rare ones.

```
idf[term] = ln((N + 1) / (df[term] + 1)) + 1
```

**TF-IDF weight**:

```
w[i][term] = tf[i][term] · idf[term]
```

The final vector for each input is the sparse map `{ term → w[i][term] }`
for every term that occurs in that input.

If the global vocabulary is empty (every bag is empty), we short-circuit
to an "all anomalies" response — there is no signal to compare on.

### Step 3 — Pairwise cosine similarity

Cosine similarity of two TF-IDF vectors `u`, `v`:

```
cos(u, v) = (u · v) / (‖u‖₂ · ‖v‖₂)
```

`u · v` is the inner product on the shared keys; `‖u‖₂` is the L2 norm.

We compute the upper triangle once and mirror it into a `Vec<Vec<f32>>`
of size N×N. Two implementation details matter:

- We iterate the smaller bag's keys when computing `u · v` to minimise
  hashmap probes.
- We iterate keys in **sorted order** so the floating-point sum is
  reproducible. HashMap iteration is randomised by Rust's default
  `RandomState`, and floating-point addition is not associative — without
  this sort, two runs can produce densities that differ in the last bit,
  which then perturbs representative selection.

```rust
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
```

Diagonal entries stay at zero — self-similarity is irrelevant for k-NN.

### Step 4 — Top-`k` neighbours, top-1, density

For each `i`, sort `(j, sim[i][j])` by `sim` descending (skipping `j == i`)
and take the first `k_eff = min(cfg.k, n - 1)` entries:

```rust
let mut pairs: Vec<(usize, f32)> = (0..n)
    .filter(|&j| j != i)
    .map(|j| (j, sim[i][j]))
    .collect();
pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

top1_sim[i] = pairs.first().map(|p| p.1).unwrap_or(0.0);

let topk: Vec<usize> = pairs.iter().take(k_eff).map(|(j, _)| *j).collect();
density[i] = pairs.iter().take(k_eff).map(|(_, s)| *s).sum::<f32>() / k_eff as f32;
neighbours.push(topk);
```

After this step, every input has three derived statistics and a list of
its `k_eff` peers.

### Step 5 — Anomaly detection

A line is an **anomaly** when even its single most-similar peer is too
distant:

```
anomaly[i]  ⇔  top1_sim[i] < cfg.anomaly_threshold
```

Anomalies are excluded from the clustering step so a single asymmetric
edge cannot pull a real outlier into an otherwise-coherent cluster.

### Step 6 — k-NN graph clustering (union-find)

Treat the directed top-`k` adjacency as undirected: for every non-anomaly
`i`, for every `j` in `neighbours[i]` that is also a non-anomaly, union
`i` and `j`. The result is **connected components of the k-NN graph**:

```rust
let mut uf = UnionFind::new(n);
for i in 0..n {
    if anomaly_set.contains(&i) { continue; }
    for &j in &neighbours[i] {
        if anomaly_set.contains(&j) { continue; }
        uf.union(i, j);
    }
}
```

Why undirected union (instead of *mutual* k-NN, where both `j ∈ top-k(i)`
**and** `i ∈ top-k(j)` are required)? Mutual k-NN is more conservative
but fragments dense corpora: when 30 nearly-identical lines all have
similar pairwise similarities, top-`k` membership is partly arbitrary
along tie-break boundaries, and mutual reciprocation rarely holds across
the full set. The undirected union-find lets transitive chains merge
those into a single cluster, which matches user intuition for log
analysis ("these 30 lines are obviously the same template").

The implementation uses path compression and union-by-rank so the
amortised cost per operation is effectively constant.

### Step 7 — Cluster bookkeeping

Walk every non-anomaly index, find its root, and bucket:

```rust
let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
for i in 0..n {
    if anomaly_set.contains(&i) { continue; }
    let root = uf.find(i);
    groups.entry(root).or_default().push(i);
}
```

Sort the resulting clusters:

1. Largest first (by `members.len()`)
2. Tie-broken by maximum density of any member (descending)

Pick a **representative** per cluster as the highest-density member:

```rust
let rep = *members.iter().max_by(|&&a, &&b| {
    density[a].partial_cmp(&density[b]).unwrap_or(Ordering::Equal)
}).unwrap_or(&members[0]);
```

Sort each cluster's members by density descending and truncate to
`cfg.max_cluster_members` to bound the JSON output (the true `size` is
always reported separately).

Finally, sort anomalies by `top1_sim` ascending (most isolated first) and
truncate to `cfg.max_anomalies`.

### Step 8 — JSON assembly

Build the result object using `serde_json::json!`:

```rust
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
```

---

## 5. Output JSON contract

```json
{
  "n_logs":            37,
  "k":                 5,
  "anomaly_threshold": 0.2,
  "n_clusters":        3,
  "n_anomalies":       4,
  "clusters": [
    {
      "id":   0,
      "size": 14,
      "representative": {
        "idx":     2,
        "text":    "ERROR upstream timeout service=auth code=503",
        "density": 0.832
      },
      "members": [
        { "idx": 2, "text": "...", "density": 0.832 },
        { "idx": 7, "text": "...", "density": 0.794 }
      ]
    }
  ],
  "anomalies": [
    { "idx": 31, "text": "INFO daily backup complete", "max_similarity": 0.04 }
  ],
  "representatives": [
    { "idx": 2, "text": "...", "density": 0.832, "cluster": 0 }
  ]
}
```

Field semantics:

| Field | Type | Description |
|---|---|---|
| `n_logs` | integer | Size of the input corpus. |
| `k` | integer | The effective `k` actually used (clamped to `[1, n-1]`). |
| `anomaly_threshold` | number | Echoed from the request — convenient for downstream UIs. |
| `n_clusters` | integer | Number of distinct clusters. |
| `n_anomalies` | integer | True total of anomalies, **not** capped by `max_anomalies`. |
| `clusters[].id` | integer | Dense `0..n_clusters` cluster identifier. |
| `clusters[].size` | integer | True member count, **not** capped by `max_cluster_members`. |
| `clusters[].representative` | object | The densest member of the cluster. |
| `clusters[].members` | array | Up to `max_cluster_members` entries, sorted by density descending. |
| `anomalies[].max_similarity` | number | The line's `top1_sim` — its closest peer's similarity. |
| `representatives` | array | One entry per cluster, in cluster order, mirroring `clusters[].representative` with an extra `cluster` index. |

Member and anomaly arrays are bounded so the JSON stays compact for very
large corpora; rely on `size` and `n_anomalies` for the true counts.

---

## 6. Configuration knobs

```rust
pub struct KnnConfig {
    pub k:                   usize,  // default: 5
    pub min_word_len:        usize,  // default: 2
    pub anomaly_threshold:   f32,    // default: 0.2
    pub max_cluster_members: usize,  // default: 10
    pub max_anomalies:       usize,  // default: 20
}
```

| Knob | Effect |
|---|---|
| **`k`** | Wider `k` ⇒ more transitive merges in the cluster graph and smoother density estimates, but more compute per input (`O(n)` per row). Narrower `k` ⇒ tighter clusters but more singleton fragments. Clamped to `[1, n-1]`. |
| **`min_word_len`** | Drops short tokens (digits, single letters) from the vocabulary. Helps when log lines contain lots of noisy short identifiers. |
| **`anomaly_threshold`** | Higher ⇒ stricter; more lines flagged as anomalies. Lower ⇒ more permissive; almost everything ends up in a cluster. |
| **`max_cluster_members`** | Caps the JSON size of the `members` array per cluster. The full `size` is always reported. |
| **`max_anomalies`** | Caps the JSON size of the `anomalies` array. The full `n_anomalies` is always reported. |

---

## 7. Complexity and scaling

For a corpus of size `n` with average bag size `m` (unique terms per
input):

| Phase | Cost |
|---|---|
| Tokenise | `O(n · m)` |
| Build TF-IDF | `O(n · m)` for both DF and per-input vector construction |
| L2 norms | `O(n · m)` |
| Pairwise cosine | `O(n² · m̄)` where `m̄` is the average size of the smaller of two bags |
| Top-`k` per row | `O(n² log n)` (sort once per row); could be reduced to `O(n²)` with a partial-quickselect, but for typical batch sizes the constant factor on `sort_by` wins |
| Anomaly detection | `O(n)` |
| Union-find clustering | `O(n · k · α(n))` ≈ `O(n · k)` in practice |

The dominant term is `O(n² · m̄)` from the cosine matrix. In practice this
is fine up to a few thousand inputs (sub-second wall time on modern CPUs).
Beyond that, the same algorithm composes cleanly with an
ANN (approximate-nearest-neighbour) index: replace the full cosine matrix
with index queries that return the top-`k` neighbours directly.

Memory:

| Structure | Size |
|---|---|
| TF-IDF maps | `O(n · m)` |
| Cosine matrix | `O(n²)` `f32` cells |
| Neighbours list | `O(n · k)` |

---

## 8. Determinism guarantees

`knn_summary` is **byte-for-byte deterministic**: the same `&[String]`
input produces the same JSON output across runs. This is harder than it
looks because:

- HashMap iteration in Rust is randomised by `RandomState` per process.
- Floating-point addition is not associative, so summation order matters.

The implementation guarantees determinism by:

1. Sorting TF-IDF keys before computing every dot product.
2. Sorting TF-IDF keys before computing every L2 norm.
3. Using a stable sort everywhere indices are ordered by similarity or
   density (Rust's `sort_by` is stable).
4. Sorting clusters by `(size desc, max-density desc)` before assigning
   `id` values, so `clusters[].id` is the same across runs.
5. Picking the representative with `max_by` — when multiple members share
   the maximum density, the last-encountered member wins, and member
   order in `groups[root]` is the deterministic input order.

The `deterministic_output_for_identical_input` test asserts
`assert_eq!(knn_summary(&logs), knn_summary(&logs))` directly.

---

## 9. Worked examples

### Example A — two themes plus outliers

Input (10 lines):

```
ERROR disk failure detected on storage-1 sector 4096
ERROR disk failure detected on storage-2 sector 8192
ERROR disk failure detected on storage-3 sector 2048
ERROR disk failure detected on storage-4 sector 1024
WARN  network timeout to upstream auth service
WARN  network timeout to upstream billing service
WARN  network timeout to upstream catalog service
WARN  network timeout to upstream payment service
INFO  scheduled backup completed successfully
DEBUG metric flush count=12345 latency=4ms
```

With default config (`k=5`, `anomaly_threshold=0.2`):

- Cluster 0 — size 8 — covers both `disk failure` and `network timeout`
  lines. With `k=5`, the dense pairwise overlap on shared structural words
  causes a transitive merge.
- 2 anomalies — `INFO scheduled backup ...` and `DEBUG metric flush ...`
  — both have `max_similarity = 0.0` because they share no scorable
  vocabulary with the rest.

To get two distinct clusters, lower `k` to 3:

```rust
let cfg = KnnConfig { k: 3, ..KnnConfig::default() };
```

Now the disk-failure cluster and the network-timeout cluster sit in
separate connected components.

### Example B — pure noise

Input:

```
alpha-event-1 done
beta-event-2 done
gamma-event-3 done
delta-event-4 done
```

With `cfg = { k: 2, anomaly_threshold: 0.99 }`:

- 0 clusters
- 4 anomalies, sorted by `max_similarity` ascending

Every line shares only the token `done` with every other, so cosine
similarity is moderate but below the strict threshold. Nothing clusters,
everything outliers.

### Example C — dense same-template corpus

Input — 30 lines:

```
disk failure storage node 000 sector 4096 data block
disk failure storage node 001 sector 4608 data block
…
disk failure storage node 029 sector 18944 data block
```

With `cfg = { k: 5, max_cluster_members: 4 }`:

- 1 cluster of size 30
- The JSON `members` array shows only the top-4 by density
- 0 anomalies — every line has near-perfect similarity with many peers

This is the case where mutual k-NN would have fragmented; the undirected
k-NN graph correctly merges all 30.

---

## 10. Failure modes and edge cases

| Input | Behaviour |
|---|---|
| `[]` | Returns the documented empty shape. `n_logs = 0`, all arrays empty, `k = 0`. |
| `[one]` | One trivial cluster with the line as its own representative; `density = 1.0`. |
| All-stop-words / sub-`min_word_len` | Falls through to the all-anomalies response. `n_clusters = 0`, `n_anomalies = n`. |
| `cfg.k > n - 1` | Silently clamped to `n - 1`. The reported `k` reflects the clamp. |
| `cfg.k = 0` | Internally bumped to `1`. |
| `cfg.anomaly_threshold ≥ 1.0` | Every line becomes an anomaly (no peer can clear the bar). |
| `cfg.anomaly_threshold ≤ 0.0` | No anomalies (every line clears). |
| Identical duplicate lines | Cluster as expected — duplicates have similarity `1.0`. |
| Very large `n` | The `O(n²)` cosine matrix dominates; switch to an ANN index for `n ≫ 10_000`. |

The function never panics on user-supplied input.

---

## 11. References

- Cover, T. M., & Hart, P. E. (1967). *Nearest neighbor pattern classification.*
  IEEE Transactions on Information Theory, 13(1), 21–27.
- Salton, G., & Buckley, C. (1988). *Term-weighting approaches in automatic
  text retrieval.* Information Processing & Management, 24(5), 513–523.
- Manning, C. D., Raghavan, P., & Schütze, H. (2008).
  *Introduction to Information Retrieval*, Cambridge University Press —
  chapters 6 (term weighting) and 14 (vector space classification),
  particularly §14.3 on k-NN over text.
- Tarjan, R. E., & van Leeuwen, J. (1984). *Worst-case analysis of set
  union algorithms.* Journal of the ACM, 31(2), 245–281 — the
  union-find with path compression and rank used by the cluster step.

## See also

- [`Documentation/tests/knn_test.md`](../tests/knn_test.md) — every test case and what it verifies.
- [`Documentation/examples/knn_demo.md`](../examples/knn_demo.md) — runnable demo walkthrough.
- `src/analysis/knn.rs` — the implementation itself, ~360 lines.
