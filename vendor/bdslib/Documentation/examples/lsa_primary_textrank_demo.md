# lsa_primary_textrank_demo.rs

**File:** `examples/lsa_primary_textrank_demo.rs`

Demonstrates `ShardsManager::summary_lsa_for_recent` and `ShardsManager::summary_lsa_for_query` applied to a synthetic mix of recurring text events and numeric measurements. Mirrors `primary_textrank_demo.rs` but uses the LSA ranking backend.

## What it demonstrates

| Function | Description |
|---|---|
| `ShardsManager::summary_lsa_for_recent(txn, duration, &LsaConfig)` | LSA summary of text-bearing primaries in a time window |
| `ShardsManager::summary_lsa_for_query(txn, query, &LsaConfig)` | LSA summary of primary records matching a vector query |

## Sections

| # | Topic | Behaviour shown |
|---|---|---|
| Ingest | Mix of 3 nginx errors, 3 login events, 10 numeric measurements, 1 cron line | Setup for all sections |
| 1 | `summary_lsa_for_recent` default config, 1h window | Recurring theme surfaces; numerics excluded |
| 2 | `summary_lsa_for_recent` max_sentences=2 | Hard cap limits output to 2 bodies |
| 3 | `summary_lsa_for_recent` n_concepts=1, max_sentences=2 | Single-concept mode captures one dominant theme |
| 4 | `summary_lsa_for_query` "nginx upstream error" | Query-focused LSA summary over vector matches |
| 5 | `summary_lsa_for_query` "user login authentication" | Different theme surfaced by different query |
| 6 | `summary_lsa_for_query` empty store | Returns empty string, no panic |

## LsaConfig knobs exercised

| Field | Values used | Effect |
|---|---|---|
| `max_sentences` | `0`, `2` | Auto-sizing vs hard cap |
| `n_concepts` | `1`, `3` | Single-theme vs multi-theme concept extraction |
| `min_word_len` | `2` (default) | Short token filtering |
| `ratio` | `0.3` (default) | Auto-size fraction |
| `power_iters` | `50` (default) | Eigenvector convergence steps |

## Run

```bash
cargo run --example lsa_primary_textrank_demo
```
