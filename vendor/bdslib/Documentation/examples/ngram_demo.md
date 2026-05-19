# ngram_demo.rs

**File:** `examples/ngram_demo.rs`

Demonstrates `bdslib::analysis::ngram`: n-gram-based anomaly detection (`ngram_anomaly`) and noise removal (`ngram_remove_noise`). Both endpoints share a tokeniser → n-gram extraction → document-frequency pipeline; the difference is whether you score lines by **rarity** (anomaly) or **commonness** (noise).

## What it demonstrates

| Function | Description |
|---|---|
| `ngram_anomaly(&[String])` | Default-config anomaly detection — flags lines using rare n-grams |
| `ngram_anomaly_with(&[String], &NgramAnomalyConfig)` | Caller-tuned anomaly detection |
| `ngram_remove_noise(&[String])` | Default-config noise removal — separates signal from repetition |
| `ngram_remove_noise_with(&[String], &NgramNoiseConfig)` | Caller-tuned noise removal |

Both return `serde_json::Value` shaped as documented in `Documentation/Algorithm/NGRAM_ANOMALY.md` and `Documentation/Algorithm/NGRAM_NOISE.md`.

## Sections

| # | Topic | Behaviour shown |
|---|---|---|
| 1 | Anomaly detection | A clear outlier (info backup line) surfaces among recurring 503 errors |
| 2 | Noise removal | A heartbeat-buried alert pair survives denoising; the heartbeats are removed |
| 3 | Duality | Same corpus through both endpoints — the unique line lands in `anomalies` AND in `kept` |
| 4 | Config knobs | Bigram (`n=2`) vs trigram (`n=3`) — trigrams catch trailing-token differences that bigrams miss |
| 5 | Edge cases | Empty input, single input, lines too short for the configured `n`, all-identical corpus |
| 6 | Full JSON | Pretty-printed output for one tiny corpus — useful as a copy-paste reference |

## NgramAnomalyConfig

| Field | Default | Description |
|---|---|---|
| `n` | `2` | N-gram length. `1` = unigrams (rare-word detection), `2` = bigrams (default), `3` = trigrams |
| `min_word_len` | `2` | Tokens shorter than this are dropped before n-gram construction |
| `anomaly_threshold` | `0.7` | Mean rarity above this flags a line as anomalous |
| `max_anomalies` | `20` | JSON-array cap (true total in `n_anomalies`) |
| `max_novel_ngrams` | `5` | Per-anomaly cap on the explanatory `novel_ngrams` array |

## NgramNoiseConfig

| Field | Default | Description |
|---|---|---|
| `n` | `2` | N-gram length |
| `min_word_len` | `2` | Short-token filter |
| `noise_threshold` | `0.85` | Mean commonness above this classifies a line as noise |
| `max_kept` | `100` | JSON-array cap on `kept` (true total in `n_kept`) |
| `max_removed` | `100` | JSON-array cap on `removed` (true total in `n_removed`) |

## How it works

1. **Tokenise** — lowercase alphanumeric runs, dropping tokens shorter than `min_word_len`. **No stop-word filtering** — n-grams derive their signal from phrase structure, so `"the system"` carries useful information.
2. **N-gram extraction** — sliding window of length `n` over each line's token sequence.
3. **Document frequency** — for each n-gram, count how many lines contain it.
4. **Per-line score** — mean of `(1 - df[g] / N)` for anomaly detection, mean of `df[g] / N` for noise removal.
5. **Threshold cut** — score ≥ threshold ⇒ flagged.
6. **Output assembly** — sort, dedup, cap, render JSON.

## Run

```bash
cargo run --example ngram_demo
```

## See also

- [`Documentation/Algorithm/NGRAM_ANOMALY.md`](../Algorithm/NGRAM_ANOMALY.md) — full algorithmic derivation of the anomaly endpoint.
- [`Documentation/Algorithm/NGRAM_NOISE.md`](../Algorithm/NGRAM_NOISE.md) — same for the noise-removal endpoint.
- [`Documentation/Algorithm/KNN.md`](../Algorithm/KNN.md) — the alternative anomaly detector based on cosine similarity. k-NN catches *vocabulary-disjoint* outliers; n-gram catches *phrase-structure* outliers. They complement each other.
