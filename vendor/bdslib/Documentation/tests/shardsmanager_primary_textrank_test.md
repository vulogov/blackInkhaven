# shardsmanager_primary_textrank_test.rs

**File:** `tests/shardsmanager_primary_textrank_test.rs`  
**Module:** `bdslib::shardsmanager_primary_textrank` — extractive TextRank over primary records

Verifies `ShardsManager::summary_for_recent` and `ShardsManager::summary_for_query`.

## Test functions

### `summary_for_recent`

| Test | What it verifies |
|---|---|
| `summary_for_recent_empty_window_returns_empty_string` | Empty window → empty summary, no panic |
| `summary_for_recent_skips_numeric_data` | All-numeric window (`data` is bare number, or `data["value"]` is number) → empty summary |
| `summary_for_recent_extracts_value_string` | `data["value"]` strings reach the summariser; recurring login pattern surfaces |
| `summary_for_recent_falls_back_to_raw_when_value_missing` | `data["raw"]` is used when `value` is absent |
| `summary_for_recent_mixes_text_and_numeric_skips_numeric` | Mixed window: text drives the summary, numeric values do not leak |
| `summary_for_recent_respects_lookback_window` | Records outside `[now − lookback, now)` are excluded |

### `summary_for_query`

| Test | What it verifies |
|---|---|
| `summary_for_query_empty_store_returns_empty` | Empty store → empty summary |
| `summary_for_query_skips_numeric_results` | Numeric-only matches → empty summary |
| `summary_for_query_returns_text_for_relevant_records` | Vector-matched text records contribute to the summary |

## Key properties verified

- **Numeric exclusion** — bare numeric `data` and `data["value"]` numbers never appear in the summary string.
- **Field precedence** — `data["value"]` is preferred over `data["raw"]`; `raw` is the fallback.
- **Window semantics** — `summary_for_recent` only sees records whose `ts` falls in `[now − lookback, now)`.
- **Empty cases** — empty window, all-numeric window, and store-with-no-matches all return `""` cleanly.

## Run

```bash
cargo test --test shardsmanager_primary_textrank_test -- --show-output
```

## Notes

These tests use the standard `tmp_manager(duration)` fixture (a per-test `TempDir` + dedicated `ShardsManager` instance) and the shared `ENGINE` `OnceLock` so they can run in parallel without colliding on the global singleton.

The fixture uses `similarity_threshold: 0.99`; tests choose either *distinct* text records (so every one stays a primary) or rely on the dedup behaviour where multiple identical records collapse into a single primary plus secondaries.
