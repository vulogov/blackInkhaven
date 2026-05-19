# shardsmanager_demo.rs

**File:** `examples/shardsmanager_demo.rs`

Demonstrates `ShardsManager`: the top-level, config-driven API that orchestrates shards, FTS, vector search, embedding, and observability.

## What it demonstrates

`ShardsManager` is the primary entry point for production use. It reads all configuration from an hjson file and exposes a unified API for ingestion and querying across all time-partitioned shards.

## Dataset

720 records ingested across 6 phases:

| Phase | Records | Content |
|---|---|---|
| Historical baseline | 120 | Mixed telemetry + logs, 6 hours ago |
| Morning peak | 150 | Telemetry-heavy, 5 hours ago |
| Incident window | 100 | String alerts + errors, 3 hours ago |
| Recovery phase | 80 | Mixed, 2 hours ago |
| Recent activity | 200 | Recent mixed events |
| IoT burst | 70 | Templated sensor readings |

## Sections

| Section | Description |
|---|---|
| 1. Config | Load from hjson; print active config fields |
| 2. Bulk ingestion | `add_batch` for each phase; print UUIDs returned |
| 3. Catalog inspection | List all shards with paths and record counts |
| 4. Cross-shard FTS | `search_fts(query, duration, limit)` across all shards |
| 5. Cross-shard vector | `search_vector(query, duration, limit)` across all shards |
| 6. Record management | `delete_by_id`, `update` across shard boundaries |
| 7. Lifecycle | Clone sharing, `sync`, `add_batch` on clone |

## Key API

| Method | Description |
|---|---|
| `ShardsManager::with_embedding(path)` | Load from hjson config; enable embedding engine |
| `add(doc)` | Ingest one document; routed by timestamp |
| `add_batch(docs)` | Ingest a slice of documents; returns all UUIDs |
| `delete_by_id(id)` | Delete one record across whichever shard holds it |
| `update(id, doc)` | Delete old record; ingest updated version (may change shards) |
| `search_fts(query, duration, limit)` | BM25 search across all shards in `duration` |
| `search_vector(query, duration, limit)` | Semantic vector search across all shards in `duration` |
| `cache()` | Access the underlying `ShardsCache` |
| `clone()` | Lightweight clone sharing the same `Arc`-backed state |

## Configuration fields (hjson)

```hjson
{
  dbpath: "./db",
  duration: "1h",
  similarity_threshold: 0.85,
  file_batch_size: 500,
  file_timeout_ms: 200
}
```
