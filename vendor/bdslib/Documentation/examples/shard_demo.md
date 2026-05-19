# shard_demo.rs

**File:** `examples/shard_demo.rs`

Demonstrates `Shard`: direct access to a single time-partition combining DuckDB, Tantivy FTS, and HNSW vector search.

## What it demonstrates

Shards are the lowest-level storage unit. Each shard holds records for a specific time window and maintains three internal indexes:

- **Telemetry table** (DuckDB) — structured record storage
- **FTS index** (Tantivy) — full-text search over primary records
- **Vector index** (HNSW via VecStore) — semantic similarity search over primary records

## Dataset

The demo ingests three categories of data:

| Category | Keys | Count |
|---|---|---|
| String alerts | `auth_failure`, `disk_warn`, `network_timeout`, `cpu_throttle`, `memory_oom`, `kernel_panic`, `app_crash` | 7 groups × 3 records |
| Numeric metrics | `cpu.usage`, `memory.rss`, `disk.io` | 3 keys, 3 records each |
| Boolean/JSON | `health.check`, `service.config` | 2 keys |
| Deliberate duplicates | various | scattered throughout |

## Sections

| Section | Description |
|---|---|
| 1. Stats | Total records, primary count, secondary count, dedup count |
| 2. Dedup log | Keys with duplicates and their timestamps |
| 3. Primary/secondary breakdown | Per-key primary and secondary counts |
| 4. FTS search | Boolean AND query across primary records |
| 5. Vector search | Semantic query; top 5 results with scores |
| 6. Time-range query | Records in a time window |
| 7. Delete primary | Delete a primary; confirm secondaries are unlinked |
| 8. Delete secondary | Delete a secondary; confirm primary is unaffected |

## Key API

| Method | Description |
|---|---|
| `Shard::new(path, config)` | Open or create a shard at path |
| `add(doc)` | Ingest a document; returns UUID and primary/secondary status |
| `get_by_id(id)` | Retrieve a document by UUID |
| `get_by_key(key)` | All documents for a key |
| `search_fts(query, limit)` | BM25 search; returns full documents with embedded secondaries |
| `search_vector(query, limit)` | HNSW similarity search; returns documents with `_score` |
| `delete_primary(id)` | Remove a primary and its index entries |
| `delete_secondary(id)` | Remove a secondary record |
| `stats()` | Counts of total, primary, secondary, dedup records |
