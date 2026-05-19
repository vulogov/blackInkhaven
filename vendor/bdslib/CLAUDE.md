# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**bdslib** is a Rust library (Edition 2024) providing multifunctional programmatic data storage. It wraps DuckDB with a connection pool and a dynamic type layer, with a large dependency set spanning analytics, full-text search, vector embeddings, NLP, time series forecasting, and media processing.

## Commands

```bash
make all        # cargo build
make rebuild    # clean + build
make test       # cargo test -- --show-output
make clean      # clean artifacts and update deps
```

Run a single test:
```bash
cargo test test_storage_engine_full_lifecycle -- --show-output
```

## Architecture

The library exposes a layered set of storage primitives, all built on `StorageEngine`.

### Foundation — `StorageEngine` (`src/storageengine.rs`)

Wraps a `duckdb::r2d2::Pool` (configurable size). `Clone`-able and thread-safe via `Arc`.

Constructor:
```rust
StorageEngine::new(path: &str, init_sql: &str, pool_size: u32) -> Result<StorageEngine>
```
`path` is a filesystem path or `":memory:"`. `init_sql` is executed once to initialize the schema.

Core methods:
- `select_all(sql)` → `Vec<Vec<rust_dynamic::value::Value>>` — collect all rows
- `select_foreach(sql, callback)` — stream rows via callback
- `execute(sql)` — DML (INSERT/UPDATE/DELETE)
- `execute_many(stmts)` — batch inside a single `BEGIN … COMMIT`
- `sync()` — DuckDB CHECKPOINT (flush WAL to disk)

**Type bridge**: `row_to_dynamic()` maps DuckDB types to `rust_dynamic::value::Value`. Use `.cast_int()` for BIGINT, `.cast_string()` for TEXT, `.cast_bin()` for BLOB.

**Error handling**: All methods return `Result<T>` (`crate::common::error::Result`).

### Primitive stores (`src/datastorage.rs`)

- **`BlobStorage`** — keyed binary blob store (UUID or string key).
- **`JsonStorage`** — keyed JSON document store with optional logical-key deduplication.

### Frequency tracking (`src/frequencytrackingstorage.rs`)

**`FrequencyTracking`** records `(timestamp, id)` observation pairs, allowing event-rate analysis over time. Duplicate observations at the same second are stored separately.

```rust
FrequencyTracking::new(path, pool_size) -> Result<FrequencyTracking>
```

Key methods:
| Method | Description |
|---|---|
| `add(id)` | Record `id` at wall-clock now |
| `add_with_timestamp(ts, id)` | Record `id` at explicit Unix-second `ts` |
| `by_id(id)` | All timestamps (ascending) for `id` |
| `by_timestamp(ts)` | Distinct IDs observed at exact second `ts` |
| `time_range(start, end)` | Distinct IDs in inclusive `[start, end]` |
| `recent(duration)` | Distinct IDs in `[now−duration, now]`; duration is a humantime string like `"1h"` |
| `sync()` | DuckDB CHECKPOINT |

Tests: `tests/frequencytrackingstorage_test.rs` (25 tests).
Demo: `examples/frequencytracking_demo.rs`.

### Document store (`src/documentstorage.rs`)

**`DocumentStorage`** combines JSON metadata (`JsonStorage`), raw content (`BlobStorage`), and a vector index. Auto-embeds via `EmbeddingEngine`.

### Sharded telemetry (`src/shard.rs`, `src/shardscache.rs`, `src/shardsmanager.rs`)

- **`Shard`** — time-partitioned unit: observability table + FTS + vector index + `tplstorage` (template store).
- **`ShardsCache`** — manages multiple `Shard` instances keyed by `[start, end)` intervals.
- **`ShardsManager`** — high-level API; routes records by `"timestamp"` field; driven by an hjson config file.

Config keys: `dbpath`, `shard_duration`, `pool_size`, `similarity_threshold`, `drain_enabled`, `drain_load_duration`.

### Drain3 log-template mining (`src/common/drain.rs`)

**`DrainParser`** — prefix-tree log template miner. Default: `depth=3`, `sim_threshold=0.5`, `max_children=100`.

Key methods: `parse(line)` → `ParseResult<'_>`, `parse_json(doc)` → `ParseJsonResult` (global DB), `parse_json_with_callback(doc, fn)` (explicit store), `load_templates(duration)` (global DB), `from_tpl_list(entries)` (pre-fetched list), `seed_cluster(tokens)` (direct injection).

`ShardsManager::drain_parse_json(parser, doc)` and `ShardsManager::drain_load(duration)` are the instance-scoped equivalents.

## Integration Tests

Tests live in `tests/storageengine_test.rs`. Each test creates its own DuckDB instance (`:memory:` or `tempfile`):
- `test_storage_engine_full_lifecycle` — basic CRUD
- `test_concurrent_access` — 100-thread Rayon parallel stress test
- `test_type_conversions` — BLOB/binary handling

## Key Dependencies

| Crate | Purpose |
|---|---|
| `duckdb` | SQL engine with R2D2 pooling |
| `rust_dynamic` | Polymorphic value type used throughout |
| `redb` | Embedded key-value store |
| `tantivy` | Full-text search |
| `vecstore` | Vector storage |
| `fastembed` | Vector embeddings |
| `augurs` | Time series (ETS, MSTL, outlier detection, DTW, clustering) |
| `rayon` | Data parallelism |
| `ndarray` | Numerical arrays |
| `serde` + `bincode`/`serde_json`/`serde_cbor`/`rmp-serde` | Multi-format serialization |
