# bdslib — Algorithms

Reference documentation for the algorithms that ship inside bdslib.
Most live under `bdslib::analysis` and run as queries against stored
records; one — primary/secondary classification — sits in the data path
itself and is invoked on every write.

Each algorithm has its own file in this directory: an exhaustive
explanation of what problem it solves, how it works in classical form,
how bdslib adapts it, the full pipeline with code excerpts, complexity
and determinism guarantees, worked examples, edge-case behaviour, and
references.

These documents are *deep dives* — for quick test summaries see
[`Documentation/tests/`](../tests/), for runnable demos see
[`Documentation/examples/`](../examples/), and for a one-line API
overview see the top-level [`Documentation/README.md`](../README.md). For
the broader storage layout these algorithms operate over, see
[`Documentation/DATABASE.md`](../DATABASE.md).

---

## What's here, by question they answer

These algorithms cover essentially every "what is this corpus about?",
"what is happening in my system?", or "is this record really new?"
question that doesn't require labelled training data or external models:

| Question | Algorithm | What you get |
|---|---|---|
| *"Is this record genuinely new, or a near-duplicate of something we already have?"* | [Primary / Secondary](PRIMARY_SECONDARY.md) | A `(uuid, is_primary)` verdict on every incoming record — the deduplication backbone of the storage subsystem. |
| *"What does this batch of strings boil down to?"* (extractive) | [TextRank](TEXTRANK.md) | A short summary string built from the most central inputs in their original input order. |
| *"What does this batch of strings boil down to?"* (concept-based) | [LSA](LSA.md) | Same shape as TextRank, but ranking by SVD-derived latent concepts — better at multi-theme corpora. |
| *"Which inputs cluster together, and which are outliers?"* | [k-NN](KNN.md) | Structured JSON with clusters, anomalies, density-ranked representatives. |
| *"Which lines use unusual phrases?"* (phrase-structure outliers) | [N-gram anomaly](NGRAM_ANOMALY.md) | List of anomalous lines with mean-rarity score and explanatory novel n-grams. |
| *"Strip the heartbeats — show me only the lines that say something distinctive."* | [N-gram noise removal](NGRAM_NOISE.md) | A `kept` (signal) array + a `removed` (noise) array, separated by mean n-gram commonness. |
| *"What event keys are correlated in time, and what plausibly caused this failure?"* | [RCA Jaccard](RCA_JACCARD.md) | Co-occurrence clusters + ranked precursor candidates with mean lead-time. |
| *"What is this stream talking about? give me keywords."* | [LDA](LDA.md) | A sorted, deduplicated keyword list per topic, distilled into one keyword string per key. |

---

## Pick the right one

If you're choosing between them for a new feature, the quickest filter is
**what shape of output you need**:

- **Primary/Secondary** isn't a choice — it runs on every write,
  invisibly, before any query algorithm sees the data. Read its doc to
  understand what "primary record" means in every other algorithm's
  input contract.
- **Need a representative *string***? — TextRank or LSA. Pick TextRank
  for tighter algorithmic guarantees and dependency-free simplicity;
  pick LSA when you need explicit control over how many concepts to
  surface, or when the corpus is heterogeneous enough that "centrality"
  alone isn't enough.
- **Need *structured JSON* with clusters and outliers?** — k-NN. The
  only query-time algorithm here that produces a full per-input verdict
  (cluster id + density, or anomaly flag).
- **Need to flag *lines using unusual phrases*?** — N-gram anomaly.
  Catches phrase-structure outliers that k-NN's vocabulary-overlap
  score smooths over (a line built from common words but in an
  unusual combination).
- **Need to *remove repetitive noise* before downstream processing?** —
  N-gram noise removal. The dual of n-gram anomaly: same pipeline,
  scored by commonness, used to strip heartbeat-style traffic.
- **Need to find *temporal correlations* between event keys?** — RCA
  Jaccard. Every other algorithm here ignores time; RCA is the one
  built around it. Use the failure-key form to extract probable causes
  ranked by lead-time.
- **Need a list of *keywords* without representative inputs?** — LDA.
  The only algorithm here that produces derived terms that may not
  appear together in any single input.

A second axis is **what you're feeding it**:

| Input | Best fit |
|---|---|
| Every incoming `(key, data, timestamp)` record | Primary / Secondary (always — runs in the data path) |
| `&[String]` of arbitrary text | TextRank, LSA, k-NN, N-gram anomaly, N-gram noise |
| Stored events / drain3 templates with timestamps | RCA Jaccard |
| Stored records under one or more keys | LDA |

The text-based three (TextRank, LSA, k-NN) all share the same
tokeniser, the same stop-word list, and the same TF-IDF or TF
representation choices, so swapping between them is a one-call change.

---

## How they fit together

The algorithms compose well. A few patterns we use in bdsweb and the
JSON-RPC layer:

- **Cluster, then summarise.** Run k-NN over a corpus to discover
  clusters and identify anomalies, then feed each cluster's members
  back through TextRank or LSA to get a representative string per
  cluster.
- **RCA, then keyword the leaders.** Run RCA Jaccard to find the
  precursor of a failure, then run LDA on the records under that
  precursor key to learn what was happening at the time.
- **Per-key topics.** `TopicSummary::query_all_keys` is the easiest
  way to get a "wall of keywords" view of every active stream in the
  system — useful as the seed for an Ollama RAG prompt or a one-glance
  dashboard tile.

---

## Algorithm reference

Each file in this directory follows the same 11-section structure so
you can navigate them without re-orienting:

1. What problem the algorithm solves here
2. The classical algorithm
3. How bdslib uses it
4. The full pipeline, step by step (with code excerpts from the actual
   implementation)
5. Output contract
6. Configuration knobs (every `*Config` field, what tightening or
   loosening it does)
7. Complexity and scaling (per-phase cost tables and memory tables)
8. Determinism guarantees (what's reproducible, what isn't, why)
9. Worked examples (operationally realistic walkthroughs)
10. Failure modes and edge cases (explicit table)
11. References (citations grade — every classical paper that backs a
    design choice in the code)

| Document | Module | What it covers |
|---|---|---|
| [PRIMARY_SECONDARY.md](PRIMARY_SECONDARY.md) | `bdslib::observability` | Primary/secondary record classification: exact-match deduplication + cosine-similarity-thresholded primary detection; the deduplication backbone in the data path. ~1320 lines, runs on every write. |
| [TEXTRANK.md](TEXTRANK.md) | `bdslib::analysis::textrank` | TextRank extractive summarisation: TF cosine graph + weighted PageRank; ~265 lines of Rust, no external dependencies beyond `serde` |
| [LSA.md](LSA.md) | `bdslib::analysis::lsa` | Latent Semantic Analysis summarisation: TF-IDF → centred Gram matrix → power-iteration SVD with deflation → Steinberger-Ježek scoring; ~415 lines, no external linear-algebra crates |
| [KNN.md](KNN.md) | `bdslib::analysis::knn` | k-Nearest-Neighbour intelligence: TF-IDF + cosine similarity → top-k neighbours → cluster discovery (union-find on k-NN graph) → anomaly detection (low top-1 similarity); structured JSON output, ~360 lines |
| [NGRAM_ANOMALY.md](NGRAM_ANOMALY.md) | `bdslib::analysis::ngram::ngram_anomaly` | N-gram anomaly detection: tokenise → sliding-window n-grams → document-frequency table → mean rarity per line → threshold cut + explanatory novel n-grams; structured JSON output |
| [NGRAM_NOISE.md](NGRAM_NOISE.md) | `bdslib::analysis::ngram::ngram_remove_noise` | N-gram noise removal: same pipeline as anomaly detection, scored by *commonness* — splits the corpus into `kept` (signal) and `removed` (noise) for downstream cleanup |
| [RCA_JACCARD.md](RCA_JACCARD.md) | `bdslib::analysis::rca`, `bdslib::analysis::rca_templates` | Jaccard-based root-cause analysis: bucketed co-occurrence → Jaccard threshold + union-find clustering → mean-lead-time causal ranking; the *temporal* analysis algorithm of the family |
| [LDA.md](LDA.md) | `bdslib::analysis::latentdirichletallocation` | Latent Dirichlet Allocation topic modelling: per-key corpus → `json_fingerprint`-flattened text → collapsed Gibbs sampling → deduplicated alphabetical keyword set; delegates to the external `latentdirichletallocation` crate |

---

## What is *not* here

Some `bdslib::analysis::*` modules don't have a dedicated algorithm
document because their value is in the integration, not in any novel
algorithmic content:

- `telemetrytrend` — applies the well-known S-H-ESD anomaly detection
  and breakout detection (via the `augurs` and `breakout` crates) to
  a stored numeric stream. See `Documentation/tests/telemetrytrend_test.md`
  for behaviour; the underlying algorithms are the published S-H-ESD
  and E-Divisive references.
- `shardsmanager_primary_textrank`, `shardsmanager_lsa_primary_textrank`,
  `shardsmanager_templates_textrank` — thin shard-aware wrappers
  around the core algorithms, applied to specific record-extraction
  shapes. The algorithm doc is the same; the wrapper docs live with
  the JSON-RPC method docs.

---

## Source layout

For each algorithm referenced above:

```
src/
├── observability.rs                  ← Primary/Secondary classifier (data-path)
└── analysis/
    ├── knn.rs                        ← k-NN intelligence
    ├── latentdirichletallocation.rs  ← LDA topic modelling
    ├── lsa.rs                        ← LSA extractive summarisation
    ├── ngram.rs                      ← N-gram anomaly + noise removal (dual endpoints)
    ├── rca.rs                        ← RCA over event records
    ├── rca_templates.rs              ← RCA over drain3 templates (same algorithm)
    └── textrank.rs                   ← TextRank extractive summarisation
```

Each `analysis/*` module is self-contained, depends only on `serde` +
`std` for the text-based algorithms (LDA additionally depends on the
`latentdirichletallocation` crate), and never panics on user-supplied
input. `observability.rs` depends on `StorageEngine` (DuckDB) and
`EmbeddingEngine` (fastembed) — it is the only algorithm in this set
that touches I/O directly.

---

## Further reading

- [`Documentation/README.md`](../README.md) — top-level project
  overview, architecture, and the full doc index.
- [`Documentation/jsonrpc_api/README.md`](../jsonrpc_api/README.md) —
  the `v2/topics`, `v2/topics.all`, `v2/rca`, `v2/rca.templates`,
  `v2/textrank.templates`, `v2/summary_for_recent`, `v2/summary_for_query`,
  `v2/summary_lsa_for_recent`, and `v2/summary_lsa_for_query` methods,
  which expose these algorithms over JSON-RPC.
- [`Documentation/tests/README.md`](../tests/README.md) — the test
  suite for every analysis module: edge cases, determinism, config
  knob coverage.
- [`Documentation/examples/README.md`](../examples/README.md) — runnable
  end-to-end demos for each algorithm (`textrank_demo`, `lsa_demo`,
  `knn_demo`, `rca_demo`, etc.).
