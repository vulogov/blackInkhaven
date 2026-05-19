# shardscache_demo.rs

**File:** `examples/shardscache_demo.rs`

Demonstrates `ShardsCache`: time-aligned shard management with an LRU memory cache and a persistent catalog.

## What it demonstrates

`ShardsCache` sits one level above `Shard`. It automatically partitions records into time-aligned buckets (e.g., 1-hour shards), caches open shard handles in memory, and persists the shard directory in a redb catalog.

## Dataset

Four synthetic hourly windows are ingested:

| Window | Label | Records |
|---|---|---|
| 4 hours ago | Startup | 30 telemetry |
| 3 hours ago | Peak | 50 mixed |
| 2 hours ago | Incident | 40 string alerts |
| 1 hour ago | Recovery | 20 mixed |

## Sections

| Section | Description |
|---|---|
| 1. Ingestion | Add records; each routes to the correct 1-hour shard |
| 2. Cache inspection | Print `cached_count()` and catalog entries |
| 3. Per-shard FTS | Search within a single shard using `shard(ts)` + `search_fts` |
| 4. Per-shard vector | Semantic search within a single shard |
| 5. Cross-shard span query | `shards_span(start, end)` returns all shards covering a range |
| 6. `current()` | Returns shards covering the most recent window |
| 7. Lifecycle | `sync()` → `close()` → reopen → verify data persists |

## Key API

| Method | Description |
|---|---|
| `ShardsCache::new(root, duration)` | Open/create shard tree with given bucket size |
| `shard(timestamp)` | Get or create the shard covering `timestamp` |
| `shards_span(start, end)` | All shards overlapping the time range |
| `current(duration)` | Shards covering the most recent `duration` |
| `cached_count()` | Number of shards currently in the memory cache |
| `sync()` | Flush all cached shards to disk |
| `close()` | Flush and evict all shards from the cache |

## Key concepts

**Interval alignment** — a record at 14:37:22 with a 1-hour bucket goes into the shard for `[14:00:00, 15:00:00)`.

**Cache vs. catalog** — open shard handles live in the LRU cache. The catalog (redb) tracks all ever-created shards so they can be reopened after `close()` or process restart.

**`shards_span`** — cross-shard queries use this to collect all shards overlapping a time range, then fan out to each shard and merge results.
