# bdslib — Test Suite

Reference documentation for the files in the `tests/` directory. Each document describes the test functions, scenarios covered, and key invariants verified by that file.

Run all tests:

```bash
make test
# or
cargo test -- --show-output
```

Run a single test:

```bash
cargo test <test_function_name> -- --show-output
```

---

## Common utilities

| Document | File | Description |
|---|---|---|
| [common_timerange_test](common_timerange_test.md) | `tests/common_timerange_test.rs` | Time range alignment: `minute_range`, `hour_range`, `day_range` — boundary alignment, nesting, contiguity |
| [common_uuid_test](common_uuid_test.md) | `tests/common_uuid_test.rs` | UUIDv7 generation: monotonicity, uniqueness, timestamp round-trip, ordering |

## Storage layer

| Document | File | Description |
|---|---|---|
| [storageengine_test](storageengine_test.md) | `tests/storageengine_test.rs` | Low-level DuckDB SQL engine: CRUD, all DuckDB types, sync, 100-thread concurrency |
| [datastorage_test](datastorage_test.md) | `tests/datastorage_test.rs` | `BlobStorage` and `JsonStorage`: CRUD, key-based dedup, nested paths, SQL safety |
| [frequencytrackingstorage_test](frequencytrackingstorage_test.md) | `tests/frequencytrackingstorage_test.rs` | `FrequencyTracking`: all read/write methods, duplicate events, DISTINCT semantics, ordering, humantime durations, SQL safety, clone sharing, sync, persistence |
| [documentstorage_test](documentstorage_test.md) | `tests/documentstorage_test.rs` | `DocumentStorage`: combined metadata/blob/vector store — add, get, update, delete, unified vector search, `results_to_strings`, persistence |
| [observability_test](observability_test.md) | `tests/observability_test.rs` | `ObservabilityStorage`: dedup, primary/secondary split, time-range queries, metadata |

## Search engines

| Document | File | Description |
|---|---|---|
| [embedding_test](embedding_test.md) | `tests/embedding_test.rs` | `EmbeddingEngine`: cosine similarity math, 384-dimensional AllMiniLML6V2 output, semantic consistency, thread safety |
| [fts_test](fts_test.md) | `tests/fts_test.rs` | `FTSEngine`: Tantivy add/search/drop/sync, BM25 ranking, limit, concurrency |
| [vectorengine_test](vectorengine_test.md) | `tests/vectorengine_test.rs` | `VectorEngine`: HNSW store/search, reranking (MMR/custom), JSON fingerprinting, concurrency |

## Shard management

| Document | File | Description |
|---|---|---|
| [shardsinfo_test](shardsinfo_test.md) | `tests/shardsinfo_test.rs` | `ShardInfoEngine` catalog: add/query by timestamp, half-open interval semantics, concurrent writes |
| [shard_test](shard_test.md) | `tests/shard_test.rs` | `Shard`: FTS + vector + observability consistency, secondary isolation, embedded secondaries in results |
| [shardscache_test](shardscache_test.md) | `tests/shardscache_test.rs` | `ShardsCache`: auto-creation, interval alignment, LRU cache, catalog persistence, span queries |
| [shardsmanager_test](shardsmanager_test.md) | `tests/shardsmanager_test.rs` | `ShardsManager`: config loading, timestamp routing, cross-shard FTS/vector, cross-shard update |
| [shardsmanager_aggregationsearch_test](shardsmanager_aggregationsearch_test.md) | `tests/shardsmanager_aggregationsearch_test.rs` | `ShardsManager::aggregationsearch`: result structure, empty store, telemetry hits with `_score`, document hits with metadata/content, combined population, duration error propagation |
| [shardsmanager_tplstorage_test](shardsmanager_tplstorage_test.md) | `tests/shardsmanager_tplstorage_test.rs` | `ShardsManager` template FrequencyTracking query API: `template_by_id` (5 tests), `templates_by_timestamp` (6 tests), `templates_recent` (7 tests) — UUID lookup, time-range filtering, cross-shard deduplication |
| [shardsmanager_rca_templates_test](shardsmanager_rca_templates_test.md) | `tests/shardsmanager_rca_templates_test.rs` | `RcaTemplatesResult`: G-Forest co-occurrence clustering on drain3 template observations — cluster detection, causal ranking, support thresholding, max_keys cap, cross-shard span, invalid duration |

## Data generation and parsing

| Document | File | Description |
|---|---|---|
| [generator_test](generator_test.md) | `tests/generator_test.rs` | `Generator`: telemetry, log, mixed, and templated document generation; placeholder types; time window |
| [logparser_test](logparser_test.md) | `tests/logparser_test.rs` | Log parsing: syslog, CLF, Apache, nginx, Python tracebacks; validation; grok; file ingestion |

## Analytics

| Document | File | Description |
|---|---|---|
| [telemetrytrend_test](telemetrytrend_test.md) | `tests/telemetrytrend_test.rs` | `TelemetryTrend`: statistics, S-H-ESD anomaly detection, breakout detection, generator integration |
| [lda_test](lda_test.md) | `tests/lda_test.rs` | LDA topic modelling: corpus analysis, keyword invariants, k clamping, empty/numeric corpora |
| [rca_test](rca_test.md) | `tests/rca_test.rs` | RCA: co-occurrence clustering, causal ranking, telemetry exclusion, threshold and bucket effects |
| [textrank_test](textrank_test.md) | `tests/textrank_test.rs` | TextRank summariser: empty/single/duplicate inputs, central-topic ranking, length cap (`max_sentences`/`ratio`), unicode, log-fingerprint clustering |
| [lsa_test](lsa_test.md) | `tests/lsa_test.rs` | LSA summariser: empty/single/duplicate inputs, repeated-topic ranking, unicode, length cap, config knobs (`n_concepts`, `power_iters`), LSA+TextRank agreement |
| [knn_test](knn_test.md) | `tests/knn_test.rs` | k-NN intelligence: JSON shape, edge cases, two-theme clustering, anomaly detection + sort order, density bounds, config knob effects (`k`, `max_cluster_members`, `max_anomalies`, `anomaly_threshold`, `min_word_len`), determinism |
| [ngram_test](ngram_test.md) | `tests/ngram_test.rs` | N-gram anomaly detection + noise removal: shape, isolated-outlier flagging, all-identical-corpus removal, monotonic threshold behaviour, sort order, member-array caps, duality (anomaly vs noise on the same corpus), determinism |
| [shardsmanager_primary_textrank_test](shardsmanager_primary_textrank_test.md) | `tests/shardsmanager_primary_textrank_test.rs` | `summary_for_recent`/`summary_for_query`: numeric exclusion, `value`/`raw` precedence, window semantics, empty-store fallbacks |
| [shardsmanager_lsa_primary_textrank_test](shardsmanager_lsa_primary_textrank_test.md) | `tests/shardsmanager_lsa_primary_textrank_test.rs` | `summary_lsa_for_recent`/`summary_lsa_for_query`: numeric exclusion, `value`/`raw` fallback, window boundary, `max_sentences` cap, `n_concepts` tuning, empty-store safety |
| [shardsmanager_scripts_test](shardsmanager_scripts_test.md) | `tests/shardsmanager_scripts_test.rs` | `script_add`/`scripts`/`script`/`update_script`/`script_delete`: metadata validation (`name`+`schedule`), full body round-trip, idempotent delete, persistence across reopen |
| [result_queue_test](result_queue_test.md) | `tests/result_queue_test.rs` | `ResultQueue`: FIFO order, TTL sweep, missing-id behaviour, JSON round-trip, concurrent-pusher safety |
| [vm_workers_test](vm_workers_test.md) | `tests/vm_workers_test.rs` | `BundWorkerPool`: singleton init, UUIDv7 job handles, workbench-to-RESULTS bridge, per-type value delivery, VM isolation, concurrent-submission safety |
| [vm_ephemeral_test](vm_ephemeral_test.md) | `tests/vm_ephemeral_test.rs` | `WorkerPool` (ephemeral): per-job fresh Bund VM, strict word-dictionary isolation, independent `EPHEMERAL_PIPE` channel, concurrent-submission safety |

## Global singleton

| Document | File | Description |
|---|---|---|
| [globals_test](globals_test.md) | `tests/globals_test.rs` | `init_db` / `get_db` / `sync_db`: initialization lifecycle, double-init guard, config resolution |

---

## Notes on singleton tests

Several tests (`globals_test`, `lda_test`, `rca_test`, `telemetrytrend_test`) wrap all sub-scenarios in a **single `#[test]` function**. This is intentional: the process-wide `ShardsManager` `OnceLock` cannot be reset between tests, so sequential execution within one function is required to prevent initialization races when tests run in parallel.
