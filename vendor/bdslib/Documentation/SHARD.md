# Shard API

`Shard` is a unified telemetry shard that combines three storage engines into a single, coherent API:

| Engine | Backend | Scope |
|---|---|---|
| [`ObservabilityStorage`](OBSERVABILITYENGINE.md) | DuckDB | All records (primary + secondary) |
| [`FTSEngine`](FTSENGINE.md) | Tantivy | Primary records only |
| [`VectorEngine`](VECTORENGINE.md) | HNSW | Primary records only |

Every telemetry event is written to `ObservabilityStorage`. If the record is classified as **primary**, it is also indexed in the FTS and vector engines. **Secondary** records are stored in `ObservabilityStorage` only; they are surfaced in search results as an embedded `"secondaries"` array inside their parent primary.

All three engines share the same UUID namespace. The UUIDv7 returned by [`add`](#add) is the identifier used in all three stores.

`Shard` is `Clone`; all clones share the same underlying DuckDB connection pool, Tantivy writer, and HNSW vector index.

All methods return `bdslib::common::error::Result<T>` — an alias for `Result<T, easy_error::Error>` defined in the shared [`common::error`](COMMON.md) module.

---

## Construction

### `Shard::new`

```rust
Shard::new(
    path: &str,
    pool_size: u32,
    embedding: EmbeddingEngine,
) -> Result<Shard>
```

Opens or creates a shard rooted at `path` with `ObservabilityStorageConfig::default` (similarity threshold `0.85`). Three sub-directories are created automatically:

| Sub-path | Engine |
|---|---|
| `{path}/obs.db` | ObservabilityStorage (DuckDB) |
| `{path}/fts/` | FTSEngine (Tantivy) |
| `{path}/vec/` | VectorEngine (HNSW) |

`pool_size` is forwarded to `ObservabilityStorage` and controls the maximum number of concurrent DuckDB connections. A value of `4` is suitable for most workloads.

```rust
use bdslib::{EmbeddingEngine, Shard};
use fastembed::EmbeddingModel;

let embedding = EmbeddingEngine::new(EmbeddingModel::AllMiniLML6V2, None)?;
let shard = Shard::new("/var/lib/myapp/shard0", 4, embedding)?;
```

---

### `Shard::with_config`

```rust
Shard::with_config(
    path: &str,
    pool_size: u32,
    embedding: EmbeddingEngine,
    config: ObservabilityStorageConfig,
) -> Result<Shard>
```

Same as `new` but accepts an explicit `ObservabilityStorageConfig`, which controls the similarity threshold used for primary/secondary classification.

```rust
use bdslib::observability::ObservabilityStorageConfig;

let shard = Shard::with_config(
    "/var/lib/myapp/shard0",
    4,
    embedding,
    ObservabilityStorageConfig { similarity_threshold: 0.92 },
)?;
```

The similarity threshold controls how aggressively records are grouped as secondaries:

| Threshold | Effect |
|---|---|
| `0.85` (default) | Semantically close records become secondaries |
| `> 1.0` (e.g. `1.1`) | Every record becomes a primary — useful for testing |
| `< -1.0` (e.g. `-1.1`) | Every record after the first becomes a secondary — useful for testing |

---

## Document format

Documents submitted to `add` must be JSON objects satisfying the `ObservabilityStorage` requirements:

| Field | Accepted types | Required | Description |
|---|---|---|---|
| `timestamp` | integer or numeric string | Yes | Event time as Unix seconds |
| `key` | string | Yes | Signal identifier or metric name |
| `data` | any JSON value | Yes | Measured value or event payload |
| `id` | UUIDv7 string | No | Explicit record identifier; auto-generated if absent |

All other fields are preserved as opaque metadata and returned verbatim by `get` and `get_by_key`.

---

## Writes

### `add`

```rust
fn add(&self, doc: JsonValue) -> Result<Uuid>
```

Stores a telemetry event and returns its UUIDv7. The operation proceeds in the following order:

1. **Validation** — `timestamp`, `key`, and `data` must be present with the accepted types.
2. **Deduplication** — if a record with the same `key` and `data` already exists, the submitted `timestamp` is appended to the deduplication log and the existing UUID is returned. The FTS and vector indexes are not touched.
3. **ObservabilityStorage** — the document is stored and classified as primary or secondary based on embedding similarity.
4. **Index update** — if the record is classified as **primary**, its JSON fingerprint is indexed in the FTS engine and its embedding is stored in the vector engine. Secondary records skip this step.

```rust
// Primary record — indexed in FTS and vector
let id = shard.add(json!({
    "timestamp": 1_700_000_000,
    "key": "cpu.usage",
    "data": 72,
}))?;

// Semantically similar — stored as secondary, not indexed
shard.add(json!({
    "timestamp": 1_700_000_001,
    "key": "cpu.usage",
    "data": 74,
}))?;

// Exact duplicate — deduplication log updated, original UUID returned
let same_id = shard.add(json!({
    "timestamp": 1_700_000_002,
    "key": "cpu.usage",
    "data": 72,
}))?;
assert_eq!(id, same_id);
```

---

### `add_batch`

```rust
fn add_batch(&self, docs: Vec<JsonValue>) -> Result<Vec<Uuid>>
```

Stores a batch of telemetry events with three batched optimisations:

1. `ObservabilityStorage::add_batch` — one bulk dedup `SELECT` for
   every `(key, data_text)` pair (replaces N per-record SELECTs), one
   ONNX `embed_batch` for all new primaries, one `BEGIN…COMMIT`
   transaction for every INSERT.
2. `VectorEngine::store_vectors_batch` — every primary's vector is
   upserted under **one HNSW lock acquisition** rather than one per
   record.
3. `FTSEngine::add_documents_batch` — one Tantivy `commit()` for the
   whole batch (Tantivy's per-record commit is the antipattern this
   path is built to avoid).

Returns UUIDs in the same order as the input documents. Use
`add_batch` for any non-trivial volume; the per-record overhead of
`add` makes it ~5–10× more expensive per record than `add_batch`
once batches reach a few hundred records.

```rust
let docs = vec![
    json!({ "timestamp": 1_700_000_000, "key": "cpu.usage", "data": 72 }),
    json!({ "timestamp": 1_700_000_001, "key": "cpu.usage", "data": 74 }),
    json!({ "timestamp": 1_700_000_002, "key": "mem.free",  "data": 4096 }),
];
let ids = shard.add_batch(docs)?;
assert_eq!(ids.len(), 3);
```

---

### `delete`

```rust
fn delete(&self, id: Uuid) -> Result<()>
```

Removes a record from all engines where it is present. If the record was a primary, it is also removed from the FTS and vector indexes. Secondary records are removed from `ObservabilityStorage` only; their parent primary remains in all indexes.

Deleting a primary leaves its linked secondaries in `ObservabilityStorage` as unlinked records. They are not automatically promoted to primary or removed.

Returns `Ok(())` for unknown IDs.

```rust
// Delete a primary — removed from obs, FTS, and vector
shard.delete(primary_id)?;

// Delete a secondary — removed from obs only; primary stays indexed
shard.delete(secondary_id)?;
```

---

## Reads

### `get`

```rust
fn get(&self, id: Uuid) -> Result<Option<JsonValue>>
```

Returns the full JSON record for `id`, or `None` if not found. The returned object contains `id`, `timestamp`, `key`, and `data`, plus all metadata fields from the original submission. No `"secondaries"` field is included; use [`search_fts`](#search_fts) or [`search_vector`](#search_vector) to retrieve primaries with their secondaries attached.

```rust
if let Some(doc) = shard.get(id)? {
    println!("key={} data={}", doc["key"], doc["data"]);
}
```

---

### `get_by_key`

```rust
fn get_by_key(&self, key: &str) -> Result<Vec<JsonValue>>
```

Returns all records whose `key` matches, ordered by timestamp ascending. Returns an empty `Vec` when no records exist for `key`. Includes both primary and secondary records; no `"secondaries"` field is injected.

```rust
for doc in shard.get_by_key("cpu.usage")? {
    println!("ts={} data={}", doc["timestamp"], doc["data"]);
}
```

---

## Search

Both search methods return **primary records only**, each enriched with a `"secondaries"` array containing the full JSON of every secondary linked to that primary.

### `search_fts`

```rust
fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<JsonValue>>
```

Full-text search over the JSON fingerprints of primary records. `query` uses [Tantivy query syntax](https://docs.rs/tantivy/latest/tantivy/query/index.html) — terms, phrases, boolean operators, and field qualifiers are all supported.

Results are returned in Tantivy relevance order (highest BM25 score first). Each result is the full JSON of the matching primary with a `"secondaries"` field appended.

```rust
// Boolean term query
let results = shard.search_fts("cpu AND usage", 10)?;

// Phrase query
let results = shard.search_fts("\"disk full\"", 5)?;

for doc in &results {
    println!(
        "primary: key={} secondaries={}",
        doc["key"],
        doc["secondaries"].as_array().map(|a| a.len()).unwrap_or(0),
    );
    for s in doc["secondaries"].as_array().unwrap_or(&vec![]) {
        println!("  secondary: data={}", s["data"]);
    }
}
```

The FTS body for each primary is its **JSON fingerprint** — a sorted, newline-separated list of `field.path: value` leaf pairs. The fingerprint includes all top-level fields (`timestamp`, `key`, `data`, and metadata), so metadata fields are searchable.

---

### `search_vector`

```rust
fn search_vector(&self, query: &JsonValue, limit: usize) -> Result<Vec<JsonValue>>
```

Semantic vector search over primary records with MMR reranking.

The search proceeds in three steps:

1. **Embedding** — the JSON fingerprint of `query` is computed and embedded via the attached `EmbeddingEngine`.
2. **HNSW retrieval** — a candidate pool of `max(limit × 2, 10)` nearest neighbours is retrieved from the vector index.
3. **MMR reranking** — candidates are reranked with `MMRReranker(λ = 0.7)`, which balances relevance against diversity, and the top `limit` results are selected.

Each result is the full JSON of a matching primary with two injected fields:

| Field | Type | Description |
|---|---|---|
| `"_score"` | `f64` | Cosine similarity to the query (1.0 = identical, 0.0 = orthogonal) |
| `"secondaries"` | array | Full JSON of every secondary linked to this primary |

Results are ordered by descending `_score`.

```rust
let query = json!({
    "timestamp": 0,
    "key": "cpu.usage",
    "data": 80,
});

let results = shard.search_vector(&query, 5)?;

for doc in &results {
    println!(
        "primary: key={} score={:.3} secondaries={}",
        doc["key"],
        doc["_score"],
        doc["secondaries"].as_array().map(|a| a.len()).unwrap_or(0),
    );
}
```

Use the same JSON structure as was passed to `add` so that field paths in the query align with field paths in the index. The query does not need to be a stored document — any JSON object with similar field structure will produce meaningful similarity scores.

---

## Passthrough accessor

### `observability`

```rust
fn observability(&self) -> &ObservabilityStorage
```

Borrows the underlying `ObservabilityStorage` for direct access to APIs not exposed on `Shard`: deduplication queries, primary/secondary listing, time-range filtering, and the `is_primary` predicate.

```rust
let obs = shard.observability();

// Check classification
let primary = obs.is_primary(id)?;

// List secondaries for a primary
let secondary_ids = obs.list_secondaries(primary_id)?;

// Time-range query
use std::time::{Duration, UNIX_EPOCH};
let ids = obs.list_ids_by_time_range(
    UNIX_EPOCH + Duration::from_secs(1_700_000_000),
    UNIX_EPOCH + Duration::from_secs(1_700_003_600),
)?;

// Deduplication log
let dup_ts = obs.get_duplicate_timestamps("cpu.usage")?;
```

See [ObservabilityStorage API](OBSERVABILITYENGINE.md) for the full API reference.

---

## Search result shape

Both `search_fts` and `search_vector` return `Vec<JsonValue>` where each element is a primary record document with additional injected fields. The base fields are those stored by `ObservabilityStorage::add`:

| Field | Always present | Description |
|---|---|---|
| `id` | Yes | UUIDv7 of the primary record |
| `timestamp` | Yes | Event timestamp (Unix seconds) |
| `key` | Yes | Signal identifier |
| `data` | Yes | Measured value or event payload |
| `_score` | `search_vector` only | Cosine similarity to the query vector |
| `secondaries` | Yes (may be empty array) | Full JSON of every linked secondary record |

Each element of `secondaries` has the same shape as a top-level `get` result: `id`, `timestamp`, `key`, `data`, and any metadata fields — but no `_score` or nested `secondaries`.

---

## Storage layout

A shard is a directory containing three sub-paths:

```
{path}/
  obs.db     # DuckDB file — all records, dedup log, primary-secondary links, embeddings
  fts/       # Tantivy index directory — primary records only
  vec/       # HNSW index files — primary records only
```

The three indexes are independent files. The DuckDB file is the source of truth for record data; the FTS and vector indexes store identifiers and search-optimised representations only. If the FTS or vector index is lost or corrupted, it can be rebuilt by re-indexing all primary records from `ObservabilityStorage`.

---

## Thread safety

`Shard` is `Clone`, `Send`, and `Sync`. All clones share:

- The same `Arc`-wrapped DuckDB connection pool (via `ObservabilityStorage`)
- The same `Arc<Mutex<IndexWriter>>` (via `FTSEngine`)
- The same `Arc<Mutex<VecStore>>` (via `VectorEngine`)

Concurrent writes from multiple threads or clones are safe. The FTS writer serialises commits through its mutex; the vector store does the same.

```rust
use std::sync::Arc;

let shard = Arc::new(Shard::new(path, 8, embedding)?);

let s = shard.clone();
std::thread::spawn(move || {
    s.add(json!({ "timestamp": 1000, "key": "k", "data": "from thread" })).unwrap();
});
```

---

## Error handling

All methods return `bdslib::common::error::Result<T>`. `add` returns `Err` for validation failures (missing or wrongly-typed mandatory fields in the underlying `ObservabilityStorage::add`). All read, search, and delete methods return `Ok(empty)` or `Ok(None)` for missing records rather than `Err`.

```rust
match shard.add(json!({ "key": "k", "data": 1 })) {
    Err(e) if e.to_string().contains("timestamp") => eprintln!("missing timestamp"),
    Err(e) => eprintln!("unexpected error: {e}"),
    Ok(id) => println!("stored as {id}"),
}
```

See [`common::error`](COMMON.md) for the shared error type.

---

## Related documentation

- [ObservabilityStorage API](OBSERVABILITYENGINE.md) — deduplication, primary/secondary classification, time-range queries
- [FTSEngine API](FTSENGINE.md) — Tantivy query syntax, index management
- [VectorEngine API](VECTORENGINE.md) — HNSW search, reranking, raw vector operations
- [EmbeddingEngine API](EMBEDDINGENGINE.md) — embedding model construction and text embedding
