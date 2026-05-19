# ObservabilityStorage API

`ObservabilityStorage` is a thread-safe telemetry store backed by [DuckDB](https://duckdb.org). Every event is submitted as a JSON document, stored with a UUIDv7 identifier, and automatically classified as either a **primary** or a **secondary** record based on the semantic similarity of its `data` field to previously stored primaries. Exact `(key, data)` repetitions are deduplicated in-place: only the first occurrence is stored; subsequent submissions append their timestamps to a deduplication log without creating new records.

All methods return `bdslib::common::error::Result<T>` — an alias for `Result<T, easy_error::Error>` defined in the shared [`common::error`](COMMON.md) module.

---

## Document format

Every document submitted to `add` must be a JSON object with three mandatory fields:

| Field | Accepted types | Description |
|---|---|---|
| `timestamp` | integer or numeric string | Event time as Unix seconds |
| `key` | string | Signal identifier or metric name |
| `data` | any JSON value | Measured value or event payload |

An optional `id` field may supply an explicit UUIDv7 string. If absent, one is generated automatically. All other fields are preserved as opaque metadata and returned verbatim by `get_by_id` and `get_by_key`.

---

## Construction

### `ObservabilityStorage::new`

```rust
ObservabilityStorage::new(
    path: &str,
    pool_size: u32,
    embedding: EmbeddingEngine,
) -> Result<ObservabilityStorage>
```

Opens or creates a store at `path` with `ObservabilityStorageConfig::default` (similarity threshold `0.85`).

All four internal tables are created automatically on first open. Pass `":memory:"` for an ephemeral in-process store that is lost when the value is dropped.

`pool_size` controls the maximum number of concurrent DuckDB connections in the connection pool. A value of `4` is suitable for most workloads.

```rust
let embedding = EmbeddingEngine::new(Model::AllMiniLML6V2, None)?;
let store = ObservabilityStorage::new("/var/lib/myapp/telemetry.db", 4, embedding)?;
```

### `ObservabilityStorage::with_config`

```rust
ObservabilityStorage::with_config(
    path: &str,
    pool_size: u32,
    embedding: EmbeddingEngine,
    config: ObservabilityStorageConfig,
) -> Result<ObservabilityStorage>
```

Same as `new` but accepts an explicit `ObservabilityStorageConfig`.

```rust
let store = ObservabilityStorage::with_config(
    ":memory:",
    4,
    embedding,
    ObservabilityStorageConfig { similarity_threshold: 0.92 },
)?;
```

`ObservabilityStorage` is `Clone`; all clones share the same underlying connection pool and embedding model.

---

## Configuration

### `ObservabilityStorageConfig`

```rust
pub struct ObservabilityStorageConfig {
    pub similarity_threshold: f32,
}
```

| Field | Type | Default | Description |
|---|---|---|---|
| `similarity_threshold` | `f32` | `0.85` | Cosine similarity cutoff for primary/secondary classification |

When a new record's embedding has cosine similarity `>= similarity_threshold` against the nearest existing primary embedding, it is stored as a secondary linked to that primary. Otherwise it becomes a new primary.

The valid semantic range is `[0.0, 1.0]`. Values outside this range produce deterministic edge-case behaviour useful for testing:

| Threshold | Effect |
|---|---|
| `> 1.0` (e.g. `1.1`) | No record can ever be secondary — every record becomes a primary |
| `< -1.0` (e.g. `-1.1`) | Every record after the first becomes a secondary of the nearest primary |

---

## Writes

### `add`

```rust
fn add(&self, doc: JsonValue) -> Result<Uuid>
```

Stores a telemetry event and returns its UUID. The call executes four steps in order:

1. **Validation** — `timestamp`, `key`, and `data` must be present with the accepted types. Returns `Err` if any are missing or have the wrong type.
2. **Deduplication** — if a record with the same `key` and `data` already exists, the submitted `timestamp` is appended to the deduplication log and the existing record's UUID is returned. No new record is created.
3. **Embedding** — the string `"key: {key} {data_text}"` is embedded via the attached `EmbeddingEngine`. See [Data-to-text conversion](#data-to-text-conversion) for how each JSON type is converted.
4. **Classification** — the embedding is compared against all stored primary embeddings. If the maximum cosine similarity meets `similarity_threshold`, the record is stored as a secondary and linked to the nearest primary. Otherwise it is stored as a primary and its embedding is persisted.

```rust
// scalar data types
store.add(json!({ "timestamp": 1_700_000_000, "key": "cpu.usage",  "data": 72     }))?;
store.add(json!({ "timestamp": 1_700_000_001, "key": "health.ok",  "data": true   }))?;
store.add(json!({ "timestamp": 1_700_000_002, "key": "req.latency","data": 0.034  }))?;
store.add(json!({ "timestamp": 1_700_000_003, "key": "error.msg",  "data": "disk full" }))?;

// structured data
store.add(json!({
    "timestamp": 1_700_000_004,
    "key": "db.stats",
    "data": { "queries": 1200, "slow": 3, "errors": 0 },
}))?;

// explicit id and metadata fields
let id = store.add(json!({
    "timestamp": 1_700_000_005,
    "key": "mem.rss",
    "data": 1_073_741_824,
    "host": "worker-03",
    "env": "prod",
}))?;
```

#### Data-to-text conversion

The `data` value is reduced to a plain string for both deduplication comparison and embedding input:

| JSON type | Conversion |
|---|---|
| `String` | Used as-is |
| `Number` | Decimal representation, e.g. `"72"`, `"0.034"` |
| `Bool` | `"true"` or `"false"` |
| `Null` | Empty string `""` |
| `Object` or `Array` | JSON fingerprint — a sorted, newline-separated list of `field.path: value` leaf pairs |

Two `data` values are considered identical (and therefore deduplicated) when their text representations are equal.

#### Embedding input

The string passed to `EmbeddingEngine::embed` is:

```
key: {key} {data_text}
```

Including the key in the embedding input means that signals with different keys but similar data values are pulled apart in embedding space, reducing cross-key false secondary assignments.

---

### `add_batch`

```rust
fn add_batch(
    &self,
    docs: &[JsonValue],
) -> Result<Vec<(Uuid, bool, Option<Vec<f32>>)>>
```

Stores a batch of telemetry records and returns one
`(uuid, is_primary, embedding)` triple per input doc in the same
order. The `embedding` field is `Some(vec)` only for new primary
records — callers (typically `Shard::add_batch`) feed it directly
into the vector index without re-embedding.

The batch path applies four optimisations on top of the per-record
algorithm:

1. **Bulk dedup** — every distinct `(key, data_text)` pair in the
   batch is resolved against the DB in a **single tuple-IN
   `SELECT`** (chunked at 1000 pairs to keep individual SQL strings
   bounded). Replaces N per-record SELECTs.
2. **Intra-batch dedup map** — duplicates within the same batch are
   collapsed to the first occurrence's UUID without a DB round-trip.
3. **One ONNX `embed_batch`** for every new primary's embedding,
   amortising transformer-model warmup across the batch.
4. **Single `BEGIN…COMMIT`** transaction containing every
   `telemetry`, `primary_embeddings`, and `primary_secondary` INSERT,
   committed via `StorageEngine::execute_many`.

Returns the same `(id, is_primary, opt_emb)` shape as the
per-record `add`, so callers can branch on `is_primary` to decide
whether to feed the embedding into FTS / vector indexes downstream.

```rust
let docs = vec![
    json!({ "timestamp": 1_700_000_000, "key": "cpu.usage", "data": 72 }),
    json!({ "timestamp": 1_700_000_001, "key": "cpu.usage", "data": 74 }),
];
let results = obs.add_batch(&docs)?;
assert_eq!(results.len(), 2);
```

---

### `delete_by_id`

```rust
fn delete_by_id(&self, id: Uuid) -> Result<()>
```

Removes the record, its deduplication entry, its embedding (if primary), and all primary→secondary links involving `id`. Returns `Ok(())` for unknown IDs.

When a primary is deleted, its linked secondaries remain in the telemetry table as unlinked records — they are not automatically promoted or removed.

```rust
store.delete_by_id(id)?;
```

---

### `delete_by_key`

```rust
fn delete_by_key(&self, key: &str) -> Result<()>
```

Calls `delete_by_id` for every record with `key`, then removes any remaining deduplication entries for that key. Returns `Ok(())` if no records exist.

```rust
store.delete_by_key("cpu.usage")?;
```

---

## Reads

### `get_by_id`

```rust
fn get_by_id(&self, id: Uuid) -> Result<Option<JsonValue>>
```

Returns the full document for `id`, or `None` if not found. The returned object always contains `id`, `timestamp`, `key`, and `data`, plus all metadata fields from the original submission.

```rust
if let Some(doc) = store.get_by_id(id)? {
    println!("key={} data={}", doc["key"], doc["data"]);
}
```

---

### `get_by_key`

```rust
fn get_by_key(&self, key: &str) -> Result<Vec<JsonValue>>
```

Returns all records whose `key` matches, ordered by `timestamp` ascending. Returns an empty `Vec` when no records exist for `key`.

```rust
for doc in store.get_by_key("cpu.usage")? {
    println!("ts={} data={}", doc["timestamp"], doc["data"]);
}
```

---

### `list_ids_by_time_range`

```rust
fn list_ids_by_time_range(
    &self,
    start: SystemTime,
    end: SystemTime,
) -> Result<Vec<Uuid>>
```

Returns UUIDs of all records whose event `timestamp` falls in the half-open interval `[start, end)`, ordered by timestamp ascending. Returns an empty `Vec` when the range is empty.

```rust
use std::time::{Duration, UNIX_EPOCH};

let start = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
let end   = UNIX_EPOCH + Duration::from_secs(1_700_003_600); // one hour window
let ids = store.list_ids_by_time_range(start, end)?;
```

---

## Deduplication

### `get_duplicate_timestamps`

```rust
fn get_duplicate_timestamps(&self, key: &str) -> Result<Vec<SystemTime>>
```

Returns the event timestamps from all duplicate `add` calls for `key`, across every data value stored under that key. Each entry corresponds to one `add` invocation where the `(key, data)` pair matched an existing record.

Returns an empty `Vec` when no duplicates have been seen for `key`, or when `key` does not exist.

```rust
// Submit the same value three times
store.add(json!({ "timestamp": 1000, "key": "k", "data": 42 }))?;
store.add(json!({ "timestamp": 2000, "key": "k", "data": 42 }))?; // duplicate
store.add(json!({ "timestamp": 3000, "key": "k", "data": 42 }))?; // duplicate

// One record in the store, two timestamps in the dedup log
let dups = store.get_duplicate_timestamps("k")?;
assert_eq!(dups.len(), 2);
// dups[0] == UNIX_EPOCH + 2000s
// dups[1] == UNIX_EPOCH + 3000s
```

The dedup log for a given `(key, data)` pair is cleared when that record is removed via `delete_by_id` or `delete_by_key`.

---

## Primary / secondary

Records are classified at insertion time and the classification is permanent. The `is_primary` flag is stored in the `telemetry` table; the primary→secondary mapping is stored in a separate `primary_secondary` table.

### `list_primaries`

```rust
fn list_primaries(&self) -> Result<Vec<Uuid>>
```

Returns UUIDs of all primary records ordered by timestamp ascending.

```rust
let primaries = store.list_primaries()?;
for pid in &primaries {
    println!("primary: {pid}");
}
```

---

### `list_primaries_in_range`

```rust
fn list_primaries_in_range(
    &self,
    start: SystemTime,
    end: SystemTime,
) -> Result<Vec<Uuid>>
```

Returns UUIDs of primary records whose event timestamp falls in `[start, end)`, ordered by timestamp ascending.

```rust
let recent_primaries = store.list_primaries_in_range(start, end)?;
```

---

### `list_secondaries`

```rust
fn list_secondaries(&self, primary_id: Uuid) -> Result<Vec<Uuid>>
```

Returns UUIDs of all secondary records linked to `primary_id`, ordered by their event timestamp ascending. Returns an empty `Vec` for an unknown `primary_id` or a primary that has no secondaries yet.

```rust
let secondaries = store.list_secondaries(primary_id)?;
for sid in secondaries {
    if let Some(doc) = store.get_by_id(sid)? {
        println!("  secondary data={}", doc["data"]);
    }
}
```

---

## Utility re-export

### `json_extract_key`

```rust
pub use crate::common::jsonfingerprint::extract_key as json_extract_key;
```

Extracts a value from a JSON document by dot-notation path, returning `Some(String)` on success or `None` when the path cannot be resolved. This is the same function used internally when `JsonStorageConfig::key_field` is set.

```rust
use bdslib::observability::json_extract_key;
use serde_json::json;

let doc = json!({ "meta": { "host": "worker-03" }, "value": 99 });
assert_eq!(json_extract_key(&doc, "meta.host"), Some("worker-03".to_string()));
assert_eq!(json_extract_key(&doc, "meta.missing"), None);
```

---

## Database schema

Four DuckDB tables are created automatically on first open. All four share the same database file.

### `telemetry`

The primary record store.

| Column | Type | Description |
|---|---|---|
| `id` | `TEXT` (PK) | UUIDv7 string |
| `ts` | `BIGINT` | Event timestamp (Unix seconds) |
| `key` | `TEXT` | Signal identifier |
| `data` | `JSON` | The `data` field value |
| `metadata` | `JSON` | All non-mandatory fields from the submitted document |
| `data_text` | `TEXT` | Plain-text representation of `data` used for deduplication |
| `is_primary` | `INTEGER` | `1` for primary, `0` for secondary |

Indexes: `key`, `ts`, `is_primary`.

### `dedup_tracking`

Stores timestamps of duplicate `add` calls without creating duplicate telemetry records.

| Column | Type | Description |
|---|---|---|
| `key` | `TEXT` (PK part) | Signal identifier |
| `data_text` | `TEXT` (PK part) | Text form of the duplicated data value |
| `timestamps` | `JSON` | Array of Unix-second integers, one per duplicate submission |

Primary key is `(key, data_text)`. Index on `key`.

### `primary_secondary`

Tracks which secondary records are linked to which primary.

| Column | Type | Description |
|---|---|---|
| `primary_id` | `TEXT` (PK part) | UUID of the primary record |
| `secondary_id` | `TEXT` (PK part) | UUID of the secondary record |
| `ts` | `BIGINT` | Event timestamp of the secondary record |

Primary key is `(primary_id, secondary_id)`. Indexes on `primary_id` and `ts`.

### `primary_embeddings`

Persists the 384-dimensional `AllMiniLML6V2` embeddings used for classification.

| Column | Type | Description |
|---|---|---|
| `primary_id` | `TEXT` (PK) | UUID of the primary record |
| `embedding` | `BLOB` | 384 × `f32` values in little-endian byte order |

---

## Classification algorithm

On each non-duplicate `add` call:

1. The string `"key: {key} {data_text}"` is embedded with `AllMiniLML6V2` (384 dimensions).
2. Every row in `primary_embeddings` is loaded and cosine similarity is computed against the new embedding.
3. If no primaries exist yet, the record is classified as primary unconditionally.
4. If the maximum cosine similarity across all primaries is `>= similarity_threshold`, the record is stored as a secondary and linked to the primary with the highest similarity.
5. Otherwise the record is stored as a primary and its embedding is inserted into `primary_embeddings`.

Classification is a read-then-write operation and is not atomic. In concurrent scenarios where two new records are classified simultaneously against an empty store, both may become primaries even if they are semantically similar to each other. The first subsequent record after either is stored will be correctly compared against both.

---

## Thread safety

`ObservabilityStorage` is `Clone`, `Send`, and `Sync`. All clones share the same `Arc`-wrapped connection pool and embedding model. The connection pool serialises concurrent DuckDB access; the embedding model is protected by a `parking_lot::Mutex`.

```rust
use std::sync::Arc;

let store = Arc::new(ObservabilityStorage::new(path, 8, embedding)?);

let s = store.clone();
std::thread::spawn(move || {
    s.add(json!({ "timestamp": 1000, "key": "k", "data": "from thread" })).unwrap();
});
```

---

## Error handling

All methods return `bdslib::common::error::Result<T>`. `add` returns `Err` for validation failures (missing or wrongly-typed mandatory fields, unparseable `id`). All other methods return `Ok(())` or `Ok(empty)` for missing IDs or keys rather than `Err`.

```rust
match store.add(json!({ "key": "k", "data": 1 })) {
    Err(e) if e.to_string().contains("timestamp") => eprintln!("missing timestamp"),
    Err(e) => eprintln!("unexpected error: {e}"),
    Ok(id) => println!("stored as {id}"),
}
```

See [`common::error`](COMMON.md) for the shared error type.

---

## UUIDv7 properties

All record identifiers are [UUID version 7](https://www.rfc-editor.org/rfc/rfc9562#section-5.7).

- **Time-ordered**: a record added later always has a greater UUID, so sorting by `id` is equivalent to sorting by insertion time (not event time).
- **Unique**: two `add` calls with identical documents still produce distinct UUIDs if the first is not deduplicated.
- **Stable on dedup**: when `add` detects a duplicate, it returns the UUID of the original record — not a new one.
