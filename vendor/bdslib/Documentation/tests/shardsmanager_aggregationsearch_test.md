# shardsmanager_aggregationsearch_test.rs

**File:** `tests/shardsmanager_aggregationsearch_test.rs`  
**Module:** `bdslib::ShardsManager::aggregationsearch` — parallel telemetry vector search + document store semantic search

Tests the combined aggregation search method that fires both a time-scoped vector search over the shard store and a semantic search over the embedded document store in parallel via `rayon::join`, returning results under `"observability"` and `"documents"`.

## Result structure

| Test | Description |
|---|---|
| `test_aggregationsearch_returns_both_keys` | Result object has both `"observability"` and `"documents"` keys |
| `test_aggregationsearch_both_keys_are_arrays` | Both keys hold JSON arrays |

## Empty store

| Test | Description |
|---|---|
| `test_aggregationsearch_empty_store_returns_empty_arrays` | No data ingested → both arrays are empty |

## Observability (telemetry) results

| Test | Description |
|---|---|
| `test_aggregationsearch_observability_finds_telemetry` | Matching telemetry record appears in `"observability"` |
| `test_aggregationsearch_observability_results_have_score` | Every hit carries `"_score"` |
| `test_aggregationsearch_observability_scores_descending` | Hits are ordered by `_score` descending |
| `test_aggregationsearch_observability_results_have_id_and_timestamp` | Every hit carries `"id"` and `"timestamp"` |

## Document store results

| Test | Description |
|---|---|
| `test_aggregationsearch_documents_finds_added_doc` | Matching document appears in `"documents"` |
| `test_aggregationsearch_documents_results_have_id_and_score` | Every hit carries `"id"` and `"score"` |
| `test_aggregationsearch_documents_results_have_metadata_and_content` | Every hit carries `"metadata"` and `"document"` |

## Combined results

| Test | Description |
|---|---|
| `test_aggregationsearch_populates_both_sides_independently` | A single call with both telemetry and a document in the store populates both sides |

## Error handling

| Test | Description |
|---|---|
| `test_aggregationsearch_invalid_duration_errors` | An invalid `duration` string propagates as `Err` |

## Key concepts tested

- **Parallel execution** — both searches complete before the function returns; neither result depends on the other
- **Independent result sets** — telemetry hits and document hits are drawn from completely separate stores; a match on one side does not affect the other
- **Vector-ranked observability** — telemetry results include `_score` (cosine similarity) and are sorted descending
- **Semantic documents** — document results include `id`, `score`, `metadata`, and `document` content
- **Duration scoping** — the `duration` parameter bounds only the telemetry search; the document store has no time window
- **Error propagation** — an invalid `duration` fails the whole call because the telemetry arm cannot parse the lookback window

## Shared infrastructure

All tests use a shared `OnceLock<EmbeddingEngine>` initialised once per process with `AllMiniLML6V2`. Each test creates its own `TempDir` and `ShardsManager` instance via `tmp_manager()` so there is no state shared between individual tests.
