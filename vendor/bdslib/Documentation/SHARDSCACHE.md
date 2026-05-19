# ShardsCache API

`ShardsCache` is a time-partitioned shard manager that automatically provisions, opens, catalogs, and caches [`Shard`](SHARD.md) instances.

Each shard covers a non-overlapping, epoch-aligned time interval of configurable duration. Records are written to the shard whose interval contains the record's timestamp; searches and range queries span whichever shards cover the requested window.

```
root_path/
  shards_info.db          # ShardInfoEngine catalog (DuckDB)
  1747998000_1748001600/  # Shard for hour 0  (auto-created on first write)
    obs.db
    fts/
    vec/
  1748001600_1748005200/  # Shard for hour 1
    ...
  ...
```

`ShardsCache` is `Clone`; all clones share the same in-memory shard map, catalog, and all underlying engine resources.

All methods return `bdslib::common::error::Result<T>` — an alias for `Result<T, easy_error::Error>` defined in [`common::error`](COMMON.md).

---

## Construction

### `ShardsCache::new`

```rust
ShardsCache::new(
    root_path: &str,
    shard_duration: &str,
    pool_size: u32,
    embedding: EmbeddingEngine,
) -> Result<ShardsCache>
```

Opens or creates a shard cache rooted at `root_path` with [`ObservabilityStorageConfig::default`](OBSERVABILITYENGINE.md#configuration) (similarity threshold `0.85`).

`shard_duration` is a human-readable string parsed by [`humantime`](https://docs.rs/humantime). Common formats:

| String | Duration |
|---|---|
| `"1h"` | 1 hour |
| `"30min"` | 30 minutes |
| `"1day"` | 24 hours |
| `"7days"` | 1 week |
| `"3600s"` | 1 hour (in seconds) |

The catalog database is stored at `{root_path}/shards_info.db`. The root directory is created automatically if it does not exist.

Returns `Err` if `shard_duration` cannot be parsed or resolves to zero.

```rust
use bdslib::{EmbeddingEngine, ShardsCache};
use fastembed::EmbeddingModel;

let embedding = EmbeddingEngine::new(EmbeddingModel::AllMiniLML6V2, None)?;
let cache = ShardsCache::new("/var/lib/myapp/shards", "1h", 4, embedding)?;
```

---

### `ShardsCache::with_config`

```rust
ShardsCache::with_config(
    root_path: &str,
    shard_duration: &str,
    pool_size: u32,
    embedding: EmbeddingEngine,
    obs_config: ObservabilityStorageConfig,
) -> Result<ShardsCache>
```

Same as `new` but accepts an explicit [`ObservabilityStorageConfig`](OBSERVABILITYENGINE.md#configuration), which controls the similarity threshold used for primary/secondary classification inside each shard.

```rust
use bdslib::observability::ObservabilityStorageConfig;

let cache = ShardsCache::with_config(
    "/var/lib/myapp/shards",
    "1day",
    4,
    embedding,
    ObservabilityStorageConfig { similarity_threshold: 0.90 },
)?;
```

---

## Core API

### `shard`

```rust
fn shard(&self, timestamp: SystemTime) -> Result<Shard>
```

Return the [`Shard`](SHARD.md) whose interval `[start, end)` covers `timestamp`. The lookup follows a three-level cascade:

1. **In-memory cache** — O(n) scan over the cached interval map. Returns immediately on hit.
2. **Catalog ([`ShardInfoEngine`](SHARDSINFOENGINE.md))** — if the cache misses, the catalog is queried for a shard covering `timestamp`. On a catalog hit, the shard is opened from its stored path and inserted into the cache.
3. **Auto-create** — if neither the cache nor the catalog covers `timestamp`, a new shard directory is provisioned, registered in the catalog, opened, and inserted into the cache.

**Interval alignment**: auto-created shard boundaries are computed by flooring `timestamp` to the nearest `shard_duration` multiple relative to the Unix epoch. For a 1-hour duration, `1_748_003_000` maps to `[1_748_001_600, 1_748_005_200)`. All intervals of the same duration are non-overlapping and contiguous.

**Directory naming**: auto-created shards are stored at `{root_path}/{start_secs}_{end_secs}`.

The returned `Shard` is a cheap clone that shares all underlying resources with the cached instance.

```rust
use std::time::{Duration, UNIX_EPOCH};

// Route a telemetry record to the correct hourly shard.
let ts = UNIX_EPOCH + Duration::from_secs(1_748_003_000);
let shard = cache.shard(ts)?;
shard.add(json!({ "timestamp": 1_748_003_000, "key": "cpu.usage", "data": 88 }))?;

// A second call for a timestamp in the same hour hits the cache.
let same_shard = cache.shard(UNIX_EPOCH + Duration::from_secs(1_748_004_500))?;
// same_shard shares the same underlying storage as shard.
```

---

### `shards_span`

```rust
fn shards_span(
    &self,
    start_ts: SystemTime,
    end_ts: SystemTime,
) -> Result<Vec<Shard>>
```

Return one `Shard` per aligned interval that overlaps the half-open window `[start_ts, end_ts)`.

The method steps in `shard_duration` increments starting from the aligned floor of `start_ts`, calling `shard()` at each step. Each shard is auto-created if not already present. Returns an empty `Vec` when `start_ts >= end_ts`.

Typical uses: aggregate queries across several time periods, bulk backfill, reporting windows.

```rust
use std::time::{Duration, UNIX_EPOCH};

// Collect all hourly shards for a 4-hour incident window.
let start = UNIX_EPOCH + Duration::from_secs(1_748_001_600);
let end   = UNIX_EPOCH + Duration::from_secs(1_748_012_400);
let shards = cache.shards_span(start, end)?;
println!("{} shards cover the incident window", shards.len()); // 3

// Aggregate total primaries across the window.
let mut total_primaries = 0usize;
for shard in &shards {
    total_primaries += shard.observability().list_primaries()?.len();
}
println!("total primaries in window: {total_primaries}");
```

---

### `current`

```rust
fn current(&self, duration: &str) -> Result<Vec<Shard>>
```

Return one `Shard` per aligned interval that overlaps the window `[now, now + duration)`, where `now` is `SystemTime::now()`.

`duration` uses the same human-readable format as the constructor (`"1h"`, `"30min"`, `"2days"`, etc.). Returns `Err` if the string cannot be parsed.

This is a convenience wrapper around `shards_span(now, now + duration)` and is suitable for real-time ingestion or live dashboards.

```rust
// Write real-time telemetry to whatever shard covers right now.
let live_shards = cache.current("1s")?;
if let Some(shard) = live_shards.first() {
    let now_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    shard.add(json!({ "timestamp": now_ts, "key": "cpu.usage", "data": 72 }))?;
}

// Get shards covering the next 3 hours (for pre-warming or scheduling).
let upcoming = cache.current("3h")?;
println!("{} shards in the next 3 hours", upcoming.len()); // typically 3–4
```

---

### `sync`

```rust
fn sync(&self) -> Result<()>
```

Flush all cached shards to disk. Calls [`Shard::sync`](SHARD.md#sync) on every cached shard — which in turn issues a DuckDB CHECKPOINT, a Tantivy commit, and an HNSW save. All shards are attempted regardless of intermediate errors; the first error is returned after all shards have been processed.

```rust
cache.sync()?; // flush before a checkpoint or before passing to another process
```

---

### `close`

```rust
fn close(&self) -> Result<()>
```

Flush all cached shards to disk and evict them from the in-memory cache. The catalog and on-disk shard data are unaffected; a subsequent `shard()` call will reopen from the catalog.

After `close`, `cached_count()` returns 0. Underlying engine resources (DuckDB connection pool, Tantivy `IndexWriter` lock, HNSW file handles) are released only when all caller-held `Shard` clones are dropped.

```rust
// Checkpoint before handoff to another process or node.
cache.close()?;
assert_eq!(cache.cached_count(), 0);

// Later, shard() reopens from the catalog — no data loss.
let shard = cache.shard(ts)?;
```

---

## Accessors

### `info`

```rust
fn info(&self) -> &ShardInfoEngine
```

Borrow the underlying [`ShardInfoEngine`](SHARDSINFOENGINE.md) catalog for direct access to shard metadata: listing all registered shards, checking whether a timestamp is covered, and querying shard paths.

```rust
// Check whether a timestamp is already covered by a registered shard.
let ts = UNIX_EPOCH + Duration::from_secs(1_748_003_000);
if cache.info().shard_exists_at(ts)? {
    println!("shard already exists for this timestamp");
}

// List catalog entries covering a timestamp.
for info in cache.info().shards_at(ts)? {
    println!("shard id={} path={}", info.shard_id, info.path);
    let start = info.start_time.duration_since(UNIX_EPOCH).unwrap().as_secs();
    let end   = info.end_time.duration_since(UNIX_EPOCH).unwrap().as_secs();
    println!("  covers [{start}, {end})");
}
```

---

### `cached_count`

```rust
fn cached_count(&self) -> usize
```

Return the number of shards currently in the in-memory cache. Zero after construction (before the first `shard()` call) and after `close()`.

```rust
assert_eq!(cache.cached_count(), 0);
cache.shard(ts)?;
assert_eq!(cache.cached_count(), 1);
cache.close()?;
assert_eq!(cache.cached_count(), 0);
```

---

## Typical usage patterns

### Real-time ingestion

Route each incoming record to the shard that covers its timestamp. `shard()` auto-creates the correct partition on first use and returns the cached instance on subsequent calls.

```rust
fn ingest(cache: &ShardsCache, record: serde_json::Value) -> Result<()> {
    let ts_secs = record["timestamp"].as_u64().unwrap();
    let ts = UNIX_EPOCH + Duration::from_secs(ts_secs);
    cache.shard(ts)?.add(record)?;
    Ok(())
}
```

### Periodic flush

Sync all open shards on a timer without closing them. Keeps the in-memory cache warm while ensuring durability.

```rust
loop {
    std::thread::sleep(Duration::from_secs(60));
    cache.sync()?;
}
```

### Time-window aggregation

Collect all shards that overlap a reporting window, then aggregate across them.

```rust
let window_start = UNIX_EPOCH + Duration::from_secs(start_epoch);
let window_end   = UNIX_EPOCH + Duration::from_secs(end_epoch);

let mut total_records = 0usize;
for shard in cache.shards_span(window_start, window_end)? {
    let obs = shard.observability();
    let ids = obs.list_ids_by_time_range(window_start, window_end)?;
    total_records += ids.len();
}
println!("records in window: {total_records}");
```

### Lifecycle: shutdown and restart

```rust
// Graceful shutdown.
cache.close()?;
// All shard Shard clones held by callers must also be dropped before the
// process exits for the IndexWriter lock to be released.

// On restart: create a new ShardsCache pointing at the same root.
// The catalog at {root_path}/shards_info.db is reopened automatically.
// Existing shards are accessible via shard() without re-indexing.
let cache = ShardsCache::new(root_path, "1h", 4, embedding)?;
let shard = cache.shard(ts)?; // opens from catalog, not auto-creates
```

---

## Storage layout

```
{root_path}/
  shards_info.db              ← ShardInfoEngine catalog (single DuckDB file)
  {start_secs}_{end_secs}/    ← one directory per shard, named by epoch range
    obs.db                    ← ObservabilityStorage (DuckDB)
    fts/                      ← FTSEngine (Tantivy index directory)
    vec/                      ← VectorEngine (HNSW index files)
  ...
```

The catalog (`shards_info.db`) and each shard's directory are independent. The catalog is the authoritative registry; shard directories can be inspected, backed up, or archived independently.

---

## Interval alignment

All shard boundaries are multiples of `shard_duration` seconds measured from the Unix epoch (1970-01-01T00:00:00Z). For a 1-hour duration:

| Timestamp | Unix seconds | Shard interval |
|---|---|---|
| 2025-05-23 04:00:00 UTC | 1748001600 | [1748001600, 1748005200) |
| 2025-05-23 04:13:20 UTC | 1748002400 | [1748001600, 1748005200) |
| 2025-05-23 05:00:00 UTC | 1748005200 | [1748005200, 1748008800) |

Two records with timestamps in the same aligned interval are always written to the same shard, regardless of which `ShardsCache` clone or thread submitted them.

---

## Thread safety

`ShardsCache` is `Clone`, `Send`, and `Sync`. All clones share:

- An `Arc<parking_lot::Mutex<HashMap<…, Shard>>>` — the in-memory shard map
- A `ShardInfoEngine` backed by a shared `Arc<StorageEngine>` connection pool
- All `Shard` internals (DuckDB pool, Tantivy writer, HNSW index) via `Arc`

The shard map mutex is held for the duration of each `shard()` call, including any I/O needed to open or create a shard. This serializes concurrent calls that would otherwise race to create the same shard interval.

```rust
use std::sync::Arc;

let cache = Arc::new(ShardsCache::new(root, "1h", 8, embedding)?);

let mut handles = vec![];
for i in 0..4u64 {
    let c = cache.clone();
    handles.push(std::thread::spawn(move || {
        let ts = UNIX_EPOCH + Duration::from_secs(1_748_000_000 + i * 120);
        c.shard(ts).unwrap().add(json!({
            "timestamp": 1_748_000_000 + i * 120,
            "key": "metric",
            "data": i,
        })).unwrap();
    }));
}
for h in handles { h.join().unwrap(); }
```

---

## Error handling

| Condition | Method | Behaviour |
|---|---|---|
| Unparseable `shard_duration` string | `new`, `with_config` | `Err` |
| Zero `shard_duration` | `new`, `with_config` | `Err` |
| `timestamp` predates Unix epoch | `shard`, `shards_span` | `Err` |
| `start_ts >= end_ts` | `shards_span` | `Ok(vec![])` |
| Unparseable `duration` string | `current` | `Err` |
| Shard I/O failure | all | `Err` |
| `sync` partial failure | `sync`, `close` | `Err` (first error; all shards still attempted) |

---

## Related documentation

- [Shard API](SHARD.md) — the per-shard storage and search API
- [ObservabilityStorage API](OBSERVABILITYENGINE.md) — deduplication, primary/secondary classification
- [FTSEngine API](FTSENGINE.md) — Tantivy full-text search
- [VectorEngine API](VECTORENGINE.md) — HNSW vector search and reranking
- [EmbeddingEngine API](EMBEDDINGENGINE.md) — text embedding model
