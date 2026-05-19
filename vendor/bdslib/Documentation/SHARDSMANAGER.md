# ShardsManager API

`ShardsManager` is a high-level, config-driven document store that routes JSON records to the correct time-partitioned shard based on each document's embedded `"timestamp"` field.

It wraps a [`ShardsCache`](SHARDSCACHE.md) and adds:

- **hjson configuration** — a single file specifies the database root, shard duration, pool size, and similarity threshold
- **Write API** — `add`, `add_batch`, `delete_by_id`, `update`
- **Cross-shard search** — `search_fts` and `search_vector` query every catalog-registered shard that overlaps a lookback window, then merge and rank results

```
config.hjson
  dbpath: "/var/lib/myapp/db"
  shard_duration: "1h"
  pool_size: 4

/var/lib/myapp/db/
  shards_info.db            # ShardInfoEngine catalog
  1748001600_1748005200/    # Shard for hour 0
    obs.db  fts/  vec/
  1748005200_1748008800/    # Shard for hour 1
    ...
```

All methods return `bdslib::common::error::Result<T>`.

`ShardsManager` is `Clone`; all clones share the same underlying `ShardsCache` (in-memory shard map, catalog connection, and engine resources).

---

## Configuration file

`ShardsManager` reads an [hjson](https://hjson.github.io/) configuration file. Hjson is a superset of JSON that allows comments and unquoted string values.

| Key | Type | Required | Description |
|---|---|---|---|
| `dbpath` | string | **yes** | Filesystem root for all shard directories and the catalog |
| `shard_duration` | string | **yes** | Width of each time partition, parsed by [`humantime`](https://docs.rs/humantime) — e.g. `"1h"`, `"30min"`, `"1day"` |
| `pool_size` | integer | no (default `4`) | DuckDB connection-pool size per shard |
| `similarity_threshold` | float | no (default `0.85`) | Cosine-similarity threshold used to classify a new record as primary or secondary |

```hjson
// Infrastructure telemetry store
{
  dbpath: /var/lib/myapp/shards
  shard_duration: 1h
  pool_size: 4
  similarity_threshold: 0.85
}
```

---

## Construction

### `ShardsManager::new`

```rust
ShardsManager::new(config_path: &str) -> Result<ShardsManager>
```

Read and parse the hjson config at `config_path`, then open or create the shard store. Resolves the embedding model from two optional config keys:

| hjson key | Default | Effect |
|---|---|---|
| `embedding_model` | `"AllMiniLML6V2"` | fastembed `EmbeddingModel` variant name (Rust Debug form, case-insensitive). |
| `embedding_cache_dir` | fastembed default (`~/.cache/huggingface/hub` or `$HF_HOME`) | Override for the model cache location. |

Both are optional; existing deployments keep working unchanged. The resolved variant name is exposed via [`embedding_model_name`](#embedding_model_name) and reported in `v2/status` so operators can confirm which model is loaded.

Model weights download on first use; subsequent calls load from cache.

Returns `Err` if the file cannot be read, the hjson is invalid, a required key is missing, `shard_duration` cannot be parsed, or `embedding_model` does not match any known fastembed variant.

```rust
use bdslib::ShardsManager;

let mgr = ShardsManager::new("/etc/myapp/shards.hjson")?;
```

> **Dimension lock-in.** The HNSW vector index dimension is fixed at first
> vector insert. Switching `embedding_model` on an existing dbpath will
> break vector search. To switch, rebuild the dbpath:
> `bdsnode --new --config bds.hjson`. See
> [`EMBEDDINGENGINE.md`](EMBEDDINGENGINE.md#dimension-lock-in) for the full
> note.

---

### `ShardsManager::with_embedding`

```rust
ShardsManager::with_embedding(
    config_path: &str,
    embedding: EmbeddingEngine,
) -> Result<ShardsManager>
```

Same as `new` but accepts a pre-loaded [`EmbeddingEngine`](EMBEDDINGENGINE.md). Use this constructor to share a single model instance across multiple `ShardsManager` instances or in tests where loading the model once is preferable.

`with_embedding` ignores the `embedding_model` / `embedding_cache_dir` config keys (the model is supplied directly), so [`embedding_model_name`](#embedding_model_name) returns `None` for managers built this way.

```rust
use bdslib::{EmbeddingEngine, ShardsManager};
use fastembed::EmbeddingModel;

let embedding = EmbeddingEngine::new(EmbeddingModel::AllMiniLML6V2, None)?;
let mgr = ShardsManager::with_embedding("/etc/myapp/shards.hjson", embedding)?;
```

---

### `embedding_model_name`

```rust
fn embedding_model_name(&self) -> Option<String>
```

The variant name of the loaded embedding model (e.g. `"AllMiniLML6V2"`, `"BGESmallENV15"`). `Some(...)` when the manager was built via `new`; `None` when built via `with_embedding` (the model identity is opaque to that constructor).

Used by `v2/status` to surface the resolved model name without re-parsing the config.

---

## Write API

Every document stored by `ShardsManager` must contain a numeric `"timestamp"` field (Unix seconds). This field determines which shard receives the record.

### `add`

```rust
fn add(&self, doc: serde_json::Value) -> Result<Uuid>
```

Add a JSON document to the shard whose interval covers `doc["timestamp"]`.

The document must contain:

| Field | Type | Description |
|---|---|---|
| `"timestamp"` | integer | Unix seconds — determines the target shard |
| `"key"` | string | Record type or metric name |
| `"data"` | any | Payload |

Internally calls [`ShardsCache::shard`](SHARDSCACHE.md#shard) with the extracted timestamp, then [`Shard::add`](SHARD.md#add). The target shard is auto-created if it does not exist in the catalog.

Returns the UUIDv7 assigned to the record. Duplicate `(key, data)` pairs in the same shard return the existing UUID without modifying the search indexes.

```rust
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

let ts = SystemTime::now()
    .duration_since(UNIX_EPOCH).unwrap().as_secs();

let id = mgr.add(json!({
    "timestamp": ts,
    "key": "cpu.usage",
    "data": 82,
}))?;
```

Returns `Err` if `"timestamp"` is absent or not a non-negative integer.

---

### `add_batch`

```rust
fn add_batch(&self, docs: Vec<serde_json::Value>) -> Result<Vec<Uuid>>
```

Add a batch of documents, routing each to its timestamp-appropriate shard. Returns UUIDs in the same order as the input. Stops and returns `Err` on the first document that fails validation or causes a storage error.

The implementation does three things in order:

1. **Validate + tag** every input doc, sorting by aligned shard
   start time so all docs for the same shard are contiguous.
2. **Open every shard upfront** (cache lookups serialise on the
   shard-cache mutex; doing them sequentially avoids contention).
3. **Per-shard `add_batch` in parallel** via `rayon`. Each shard's
   storage engines (DuckDB pool, Tantivy index, VecStore) are
   independent, so concurrent writes across shards don't contend on
   shared state. For a single-shard batch this degrades gracefully to
   sequential execution; for multi-shard backfills (e.g. a 24h batch
   into 1h shards) the speedup approaches the number of shards times
   the per-shard speed.

After the parallel pass returns, the per-doc `jsoncache` insert and
result-id assembly happen sequentially.

```rust
let ids = mgr.add_batch(vec![
    json!({ "timestamp": ts,     "key": "mem.usage",  "data": 61 }),
    json!({ "timestamp": ts + 1, "key": "disk.io",    "data": 42 }),
    json!({ "timestamp": ts + 2, "key": "net.rx",     "data": 1024 }),
])?;
assert_eq!(ids.len(), 3);
```

Inside each per-shard call, `Shard::add_batch` itself batches the
DuckDB writes into one transaction, batches the Tantivy index updates
into one commit, and batches the HNSW upserts under one lock — so even
single-shard batches see big wins over per-record `add`. See
[`SHARD.md`](SHARD.md) for the per-shard batched optimisations and
[`OBSERVABILITYENGINE.md`](OBSERVABILITYENGINE.md) for the bulk dedup
inside `ObservabilityStorage::add_batch`.

---

### `delete_by_id`

```rust
fn delete_by_id(&self, id: Uuid) -> Result<()>
```

Delete the record with `id` from whichever catalog-registered shard contains it. Iterates all shards in catalog order (start time ascending) until the record is found, then delegates to [`Shard::delete`](SHARD.md#delete).

If the record was a primary, it is also removed from that shard's FTS and vector indexes. Its linked secondaries remain in `ObservabilityStorage` as unlinked records.

Returns `Ok(())` if no shard contains the record (no-op).

```rust
mgr.delete_by_id(id)?;

// A second call on the same ID is safe.
mgr.delete_by_id(id)?; // Ok(())
```

**Performance note**: `delete_by_id` scans shards in start-time order and opens each from the catalog on cache miss. For stores with many shards, prefer inserting a tombstone record or retaining a record→shard mapping to avoid the full scan.

---

### `update`

```rust
fn update(&self, id: Uuid, doc: serde_json::Value) -> Result<Uuid>
```

Replace the record `id` with new content. Deletes the existing record from its current shard, then inserts the new document. If the new document's `"timestamp"` maps to a different shard interval, the record is moved to that shard.

Returns the UUID of the newly inserted record.

```rust
// Update in-place (same shard — timestamp stays within the same hour).
let new_id = mgr.update(old_id, json!({
    "timestamp": ts,
    "key": "cpu.usage",
    "data": 91,          // updated value
}))?;

// Cross-shard move — old record deleted from hour 0, new record written to hour 2.
let moved_id = mgr.update(old_id, json!({
    "timestamp": ts + 7200,  // 2 hours later → different shard
    "key": "cpu.usage",
    "data": 91,
}))?;
```

Returns `Err` if the new document's `"timestamp"` is invalid.

---

## Search API

Both search methods use a **lookback window** anchored at `now`:

```
window = [now − duration, now + 1s)
```

Only shards already registered in the catalog that overlap this window are queried. No empty shards are auto-created. Shards are queried in start-time ascending order.

### `search_fts`

```rust
fn search_fts(&self, duration: &str, query: &str) -> Result<Vec<serde_json::Value>>
```

Full-text search across all catalog-registered shards that overlap `[now − duration, now + 1s)`.

`duration` uses the same human-readable format as `shard_duration` in the config (`"1h"`, `"30min"`, `"7days"`, etc.).

`query` uses [Tantivy query syntax](https://docs.rs/tantivy/latest/tantivy/query/struct.QueryParser.html) — boolean operators, phrase queries, and field-qualified terms are all supported.

Each returned document is the full JSON of a matching primary record with a `"secondaries"` array containing every secondary linked to that primary. Results are returned in Tantivy relevance order within each shard, shards ordered oldest-first.

```rust
// All error events in the last 6 hours.
let errors = mgr.search_fts("6h", "error")?;

// Exact phrase in the last 24 hours.
let oom = mgr.search_fts("1day", "\"out of memory\"")?;

// Boolean: connection issues in the last 2 hours.
let net = mgr.search_fts("2h", "timeout OR \"packet loss\"")?;
```

Returns `Err` if `duration` cannot be parsed.

**Result shape:**

```json
{
  "timestamp": 1748003600,
  "key": "log.error",
  "data": "connection timeout to upstream service",
  "secondaries": [
    { "timestamp": 1748003610, "key": "log.error", "data": "retry 1 failed" }
  ]
}
```

---

### `search_vector`

```rust
fn search_vector(&self, duration: &str, query: &serde_json::Value) -> Result<Vec<serde_json::Value>>
```

Semantic vector search across all catalog-registered shards that overlap `[now − duration, now + 1s)`. `duration` uses the same format as `search_fts`.

`query` is a JSON document whose fingerprint is embedded and used for HNSW nearest-neighbour search within each shard. Candidate pools from all matching shards are collected, then the combined result list is sorted by `_score` descending, then `timestamp` descending.

Each returned document includes:
- `"_score"` — cosine similarity (higher is more similar)
- `"secondaries"` — linked secondary records

```rust
// Semantic search for service degradation over the last 6 hours.
let results = mgr.search_vector("6h", &json!({
    "key": "log.error",
    "data": "service degradation memory exhaustion",
}))?;

for doc in &results {
    let score = doc["_score"].as_f64().unwrap();
    let key   = doc["key"].as_str().unwrap();
    let ts    = doc["timestamp"].as_u64().unwrap();
    println!("score={score:.4}  key={key}  ts={ts}");
}
```

Returns `Err` if `duration` cannot be parsed.

**Result shape:**

```json
{
  "timestamp": 1748006200,
  "key": "log.error",
  "data": "out of memory warning triggered on worker-02",
  "_score": 0.7014,
  "secondaries": []
}
```

---

## Accessors

### `cache`

```rust
fn cache(&self) -> &ShardsCache
```

Borrow the underlying [`ShardsCache`](SHARDSCACHE.md) for direct access to shard routing, catalog queries, flush control, and cache statistics.

```rust
// How many shards are currently open?
println!("open shards: {}", mgr.cache().cached_count());

// Flush all open shards to disk.
mgr.cache().sync()?;

// Inspect the catalog directly.
for info in mgr.cache().info().list_all()? {
    let start = info.start_time.duration_since(UNIX_EPOCH)?.as_secs();
    println!("shard {}: [{start}, ...)", info.shard_id);
}
```

---

## Typical usage patterns

### Real-time ingestion

```rust
fn ingest(mgr: &ShardsManager, record: serde_json::Value) -> Result<()> {
    mgr.add(record)?;
    Ok(())
}
```

The target shard is determined by the `"timestamp"` field. First use of a new time interval creates the shard directory and registers it in the catalog automatically.

### Batch import

```rust
let records: Vec<serde_json::Value> = load_from_file("events.jsonl");
let ids = mgr.add_batch(records)?;
println!("imported {} records", ids.len());

mgr.cache().sync()?; // flush after bulk load
```

### Lookback dashboard query

```rust
// Last 24 hours of error events sorted by FTS relevance.
let hits = mgr.search_fts("1day", "error OR critical OR fatal")?;

// Last 4 hours of semantically related records sorted by similarity.
let related = mgr.search_vector("4h", &json!({
    "key": "log.error",
    "data": "database connection failure",
}))?;
```

### Updating a stale record

```rust
// Correct a value in an already-stored record.
let new_id = mgr.update(old_id, json!({
    "timestamp": original_ts,
    "key": "cpu.usage",
    "data": corrected_value,
}))?;
```

### Periodic flush

```rust
loop {
    std::thread::sleep(std::time::Duration::from_secs(60));
    mgr.cache().sync()?;
}
```

### Graceful shutdown

```rust
mgr.cache().sync()?;
mgr.cache().close()?;
// All ShardsManager and ShardsCache clones must be dropped before exit
// to release the Tantivy IndexWriter lock on each shard directory.
```

---

## Storage layout

`ShardsManager` does not add any files of its own; all storage is managed by the underlying `ShardsCache`:

```
{dbpath}/
  shards_info.db              ← ShardInfoEngine catalog
  {start_secs}_{end_secs}/    ← one directory per shard
    obs.db                    ← ObservabilityStorage (DuckDB)
    fts/                      ← FTSEngine (Tantivy)
    vec/                      ← VectorEngine (HNSW)
  ...
```

The config file itself (`config.hjson`) is read once during construction and is not monitored for changes at runtime.

---

## Thread safety

`ShardsManager` is `Clone`, `Send`, and `Sync`. All clones share the same `ShardsCache` and its underlying resources (shard map mutex, catalog connection pool, per-shard engine handles). Write and read operations may be issued concurrently from any number of threads or clones.

```rust
use std::sync::Arc;

let mgr = Arc::new(ShardsManager::with_embedding(config_path, embedding)?);

let mut handles = vec![];
for i in 0..8u64 {
    let m = mgr.clone();
    handles.push(std::thread::spawn(move || {
        let ts = now_secs() - i * 3600;
        m.add(json!({ "timestamp": ts, "key": "metric", "data": i })).unwrap();
    }));
}
for h in handles { h.join().unwrap(); }
```

---

## Error handling

| Condition | Method | Behaviour |
|---|---|---|
| Config file not found or unreadable | `new`, `with_embedding` | `Err` — message contains `"cannot read config"` |
| Malformed hjson | `new`, `with_embedding` | `Err` — message contains `"invalid config"` |
| Missing required field (`dbpath`, `shard_duration`) | `new`, `with_embedding` | `Err` |
| Unparseable `shard_duration` in config | `new`, `with_embedding` | `Err` |
| Document missing `"timestamp"` field | `add`, `add_batch`, `update` | `Err` — message contains `"timestamp"` |
| `"timestamp"` is not a non-negative integer | `add`, `add_batch`, `update` | `Err` |
| Unparseable `duration` string | `search_fts`, `search_vector` | `Err` — message contains `"invalid duration"` |
| Shard I/O failure | all | `Err` |
| `id` not found in any shard | `delete_by_id` | `Ok(())` — no-op |

---

## Related documentation

- [ShardsCache API](SHARDSCACHE.md) — time-partitioned shard manager and cache lifecycle
- [Shard API](SHARD.md) — per-shard storage, search, and sync
- [ShardInfoEngine API](SHARDSINFOENGINE.md) — catalog of shard metadata
- [ObservabilityStorage API](OBSERVABILITYENGINE.md) — deduplication and primary/secondary classification
- [FTSEngine API](FTSENGINE.md) — Tantivy full-text search
- [VectorEngine API](VECTORENGINE.md) — HNSW vector search and reranking
- [EmbeddingEngine API](EMBEDDINGENGINE.md) — text embedding model
