# knn_demo.rs

**File:** `examples/knn_demo.rs`

Demonstrates `bdslib::analysis::knn`: k-Nearest-Neighbour clustering and anomaly detection over a list of strings — log lines, JSON fingerprints, sentences, anything tokenisable.

## What it demonstrates

| Function | Description |
|---|---|
| `knn_summary(&[String])` | Run k-NN intelligence with the default [`KnnConfig`] |
| `knn_summary_with(&[String], &KnnConfig)` | Same, but with caller-tuned config |

Both return a `serde_json::Value` shaped as documented in `Documentation/Algorithm/KNN.md`.

## Sections

| # | Topic | Behaviour shown |
|---|---|---|
| 1 | Two themes + isolated outliers | Two real clusters surface, two info/debug lines fall into `anomalies` |
| 2 | Pure noise corpus | Every line is its own anomaly when nothing shares vocabulary |
| 3 | Dense same-template corpus | 30 nearly-identical lines collapse into one cluster (members capped to 4) |
| 4 | Config knob effects | Same corpus, five different `KnnConfig` values, side-by-side outcomes |
| 5 | Edge cases | Empty input, single input, stop-word-only input |
| 6 | Full JSON output | Pretty-printed full structure for a 3-line corpus — useful as a copy-paste reference |

## KnnConfig

| Field | Default | Description |
|---|---|---|
| `k` | `5` | Number of nearest neighbours per input. Clamped to `[1, n-1]` at runtime. |
| `min_word_len` | `2` | Tokens shorter than this are dropped before scoring. |
| `anomaly_threshold` | `0.2` | Inputs whose top-1 cosine similarity is below this threshold are flagged as anomalies. |
| `max_cluster_members` | `10` | Maximum number of cluster members included in the JSON (the true `size` is always reported). |
| `max_anomalies` | `20` | Maximum number of anomalies in the JSON output (sorted by isolation; true `n_anomalies` is always reported). |

## How it works

1. **Tokenise** — lowercase alphanumeric tokens; stop-words and tokens shorter than `min_word_len` are dropped.
2. **TF-IDF** — smoothed `log((N+1)/(df+1)) + 1` IDF × normalized term frequency.
3. **Pairwise cosine similarity** — full N×N matrix (only the upper triangle is computed, then mirrored).
4. **Top-k neighbours per input** — index sort by similarity descending; record the top-1 similarity (anomaly signal) and the top-k average (density / centrality).
5. **Anomaly cut** — any input with top-1 similarity below `anomaly_threshold` is classified as an outlier.
6. **k-NN graph clustering** — union-find on the directed-then-symmetrised k-NN graph. Edges between anomalies and the rest are skipped so isolated lines stay isolated.
7. **Representatives** — each cluster's densest member becomes its representative.

For the long-form derivation, see [`Documentation/Algorithm/KNN.md`](../Algorithm/KNN.md).

## Run

```bash
cargo run --example knn_demo
```
