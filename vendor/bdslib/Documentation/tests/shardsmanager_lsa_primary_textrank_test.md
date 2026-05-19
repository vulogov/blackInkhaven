# shardsmanager_lsa_primary_textrank_test.rs

**File:** `tests/shardsmanager_lsa_primary_textrank_test.rs`  
**Module:** `bdslib::shardsmanager_lsa_primary_textrank` — LSA summarisation over primary records

Verifies the contract of `ShardsManager::summary_lsa_for_recent` and `ShardsManager::summary_lsa_for_query`.

## Test functions

### summary_lsa_for_recent

| Test | What it verifies |
|---|---|
| `lsa_recent_empty_window_returns_empty_string` | No records in window → empty string, no panic |
| `lsa_recent_skips_numeric_data` | All-numeric records (bare number or `{ "value": N }`) → empty string |
| `lsa_recent_extracts_value_string` | `data["value"]` text bodies drive the summary; recurring theme (login) surfaces |
| `lsa_recent_falls_back_to_raw_when_value_missing` | `data["raw"]` is used when `value` is absent |
| `lsa_recent_mixes_text_and_numeric_skips_numeric` | Numeric records are silently dropped; text records drive LSA output; numeric values don't appear in the summary string |
| `lsa_recent_respects_lookback_window` | Record outside the lookback window is excluded; in-window record appears |
| `lsa_recent_respects_n_concepts_and_max_sentences` | `max_sentences=2` caps output to at most 2 records; `n_concepts` tuning is exercised without panic |

### summary_lsa_for_query

| Test | What it verifies |
|---|---|
| `lsa_query_empty_store_returns_empty` | Empty store → empty string |
| `lsa_query_skips_numeric_results` | Vector query matching only numeric records → empty string |
| `lsa_query_returns_text_for_relevant_records` | Query matching repeated text records surfaces their tokens in the summary |

## Key properties verified

- **Numeric exclusion** — `data` bare number and `data["value"]` number are both silently skipped.
- **`data["raw"]` fallback** — when `value` is absent the `raw` field provides the body.
- **Lookback window** — records outside `[now - duration, now)` are excluded.
- **`max_sentences` cap** — LSA output is capped to the requested number of sentences.
- **Empty-store safety** — no panic when no records exist.

## Run

```bash
cargo test --test shardsmanager_lsa_primary_textrank_test -- --show-output
```

## Notes

Each test creates an isolated `TempDir` with its own DuckDB path, so tests are independent and can run in parallel. A single `OnceLock<EmbeddingEngine>` is shared across tests to avoid repeated model-load overhead.
