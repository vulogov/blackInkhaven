# VectorEngine API

`VectorEngine` provides a thread-safe HNSW vector store with optional automatic text embedding. It wraps [`vecstore::VecStore`](https://docs.rs/vecstore) behind an `Arc<Mutex<_>>`, making instances cheaply cloneable and safe to share across threads.

When constructed with an [`EmbeddingEngine`](EMBEDDINGENGINE.md), it can embed JSON documents automatically via `store_document` and `search_json`.

All methods return `bdslib::common::error::Result<T>` — an alias for `Result<T, easy_error::Error>` defined in the shared [`common::error`](COMMON.md) module.

---

## Construction

### `new`

```rust
VectorEngine::new(path: &str) -> Result<VectorEngine>
```

Open or create a vector store at `path`. The directory and index files are created automatically if they do not exist.

`store_document` and `search_json` are **not available** on engines created with `new`. Use [`with_embedding`](#with_embedding) instead.

```rust
use bdslib::VectorEngine;

let engine = VectorEngine::new("/data/vectors")?;
```

### `with_embedding`

```rust
VectorEngine::with_embedding(path: &str, engine: EmbeddingEngine) -> Result<VectorEngine>
```

Open or create a vector store at `path`, attaching an `EmbeddingEngine` for automatic text embedding. Enables `store_document` and `search_json`.

```rust
use bdslib::{VectorEngine, EmbeddingEngine, embedding::Model};

let emb = EmbeddingEngine::new(Model::AllMiniLML6V2, None)?;
let engine = VectorEngine::with_embedding("/data/vectors", emb)?;
```

`VectorEngine` is `Clone`. Cloning is cheap — all clones share the same underlying store and embedding model via `Arc`.

---

## Writing

### `store_vector`

```rust
fn store_vector(&self, id: &str, vector: Vec<f32>, metadata: Option<JsonValue>) -> Result<()>
```

Store an `id → vector` association. `metadata` is an optional JSON object whose fields are returned in search results. If a record with the same `id` already exists it is replaced (upsert).

```rust
use serde_json::json;

engine.store_vector(
    "doc-1",
    vec![0.1, 0.2, 0.3],
    Some(json!({ "title": "Introduction", "year": 2024 })),
)?;

// No metadata
engine.store_vector("doc-2", vec![0.4, 0.5, 0.6], None)?;
```

### `store_vectors_batch`

```rust
fn store_vectors_batch(
    &self,
    entries: Vec<(String, Vec<f32>, Option<JsonValue>)>,
) -> Result<()>
```

Bulk-upsert `(id, vector, metadata)` triples under a **single
store-lock acquisition**. Equivalent to calling `store_vector` N
times, but pays the inner `Mutex<Option<VecStore>>` lock cost once
for the whole batch.

Used by `Shard::add_batch` to coalesce per-primary HNSW upserts
inside one critical section — the largest single perf win for
high-volume primary-heavy ingestion. For an empty input it is a
no-op. On the first failed upsert the helper returns immediately;
entries already upserted in this call are not rolled back (HNSW has
no transaction primitive).

```rust
use serde_json::json;

let entries = vec![
    ("doc-1".to_string(), vec![0.1, 0.2, 0.3],
     Some(json!({ "title": "A" }))),
    ("doc-2".to_string(), vec![0.4, 0.5, 0.6],
     Some(json!({ "title": "B" }))),
    ("doc-3".to_string(), vec![0.7, 0.8, 0.9], None),
];
engine.store_vectors_batch(entries)?;
```

### `store_document`

```rust
fn store_document(&self, id: &str, document: JsonValue) -> Result<()>
```

Embed `document` using the attached `EmbeddingEngine` and store the resulting vector under `id`. The full JSON is persisted as metadata and returned in search results.

The document is converted to a canonical fingerprint string via [`json_fingerprint`](#json_fingerprint) before embedding. Use the same JSON shape for queries as for stored documents so field paths align.

Returns `Err` if no `EmbeddingEngine` was provided at construction time.

```rust
use serde_json::json;

engine.store_document("rust-book", json!({
    "title": "The Rust Programming Language",
    "author": "Steve Klabnik",
    "tags": ["systems", "memory-safety"],
    "year": 2019,
}))?;
```

---

## Searching

### `search`

```rust
fn search(&self, query_vector: Vec<f32>, limit: usize) -> Result<Vec<SearchResult>>
```

Return the `limit` nearest neighbours to `query_vector`, ordered by descending similarity score.

```rust
let results = engine.search(query_vector, 5)?;
for r in &results {
    println!("{} — score {:.4}", r.id, r.score);
}
```

### `search_reranked`

```rust
fn search_reranked(
    &self,
    query_vector: Vec<f32>,
    query_text: &str,
    limit: usize,
    candidate_pool: usize,
    reranker: &dyn Reranker,
) -> Result<Vec<SearchResult>>
```

Search for `candidate_pool` nearest neighbours, re-rank with `reranker`, and return the top `limit` results.

`query_text` is forwarded to the reranker for semantic scoring. Pass an empty string for rerankers that do not use query text (e.g. MMR).

```rust
use vecstore::reranking::IdentityReranker;

let reranker = IdentityReranker;
let results = engine.search_reranked(query_vec, "query text", 5, 20, &reranker)?;
```

### `search_json`

```rust
fn search_json(&self, query: &JsonValue, limit: usize) -> Result<Vec<SearchResult>>
```

Fingerprint `query` using [`json_fingerprint`](#json_fingerprint), embed the result, and return the `limit` nearest stored documents.

Use the same JSON structure as was passed to `store_document` so that field paths in the query align with field paths in the index.

Returns `Err` if no `EmbeddingEngine` was provided at construction time.

```rust
use serde_json::json;

let results = engine.search_json(&json!({ "title": "Rust", "tags": ["systems"] }), 5)?;
```

### `search_json_reranked`

```rust
fn search_json_reranked(
    &self,
    query: &JsonValue,
    limit: usize,
    candidate_pool: usize,
    reranker: &dyn Reranker,
) -> Result<Vec<SearchResult>>
```

Fingerprint `query`, embed it, search `candidate_pool` neighbours, re-rank, and return the top `limit` results.

The fingerprint string is passed as query text to the reranker so cross-encoder rerankers receive meaningful input.

Returns `Err` if no `EmbeddingEngine` was provided at construction time.

```rust
use serde_json::json;
use vecstore::reranking::MMRReranker;

let reranker = MMRReranker::new(0.7); // 70% relevance, 30% diversity
let results = engine.search_json_reranked(
    &json!({ "title": "embeddings", "domain": "nlp" }),
    5,
    20,
    &reranker,
)?;
```

---

## Persistence

### `sync`

```rust
fn sync(&self) -> Result<()>
```

Flush the in-memory index and all records to disk. Call after a batch of writes to guarantee durability across process restarts.

```rust
engine.store_vector("v1", vec![0.1, 0.2], None)?;
engine.store_vector("v2", vec![0.3, 0.4], None)?;
engine.sync()?;
```

---

## Search results

Search methods return `Vec<SearchResult>` where `SearchResult` is a re-export of `vecstore::Neighbor`:

```rust
pub struct Neighbor {
    pub id: String,
    pub score: f32,
    pub metadata: Metadata,
}
```

`score` is a **cosine similarity** value in `[-1.0, 1.0]`. Higher is more similar:

| Score | Meaning |
|---|---|
| `≈ 1.0` | Nearly identical to the query |
| `≈ 0.0` | Orthogonal — unrelated |
| `< 0.0` | Opposite direction |

Results are ordered by descending `score` (most similar first).

`metadata.fields` is a `HashMap<String, serde_json::Value>` containing the JSON fields stored alongside the vector.

---

## Re-rankers

`vecstore::reranking` provides built-in reranker implementations:

| Type | Constructor | Strategy |
|---|---|---|
| `IdentityReranker` | `IdentityReranker` (unit struct) | Return candidates unchanged |
| `MMRReranker` | `MMRReranker::new(lambda: f32)` | Maximal Marginal Relevance — balance relevance and diversity |
| `RRFReranker` | `RRFReranker::new(k: f32)` | Reciprocal Rank Fusion |
| `ScoreReranker<F>` | `ScoreReranker::new(score_fn)` | Custom score function |

```rust
use vecstore::reranking::{IdentityReranker, MMRReranker, RRFReranker};
```

---

## `json_fingerprint`

```rust
pub fn json_fingerprint(json: &JsonValue) -> String
```

Convert any JSON value into a flat, human-readable string for embedding. The algorithm walks the JSON tree and emits `path: value` pairs for every leaf, preserving field-name context at every depth.

```
{ "title": "Rust", "meta": { "year": 2015, "tags": ["systems", "safe"] } }
→
"title: Rust meta.year: 2015 meta.tags[0]: systems meta.tags[1]: safe"
```

| JSON type | Output |
|---|---|
| Object | Recurse with dot-separated path prefix |
| Array | Recurse with `[i]` index appended to the path |
| String | `path: value` |
| Number / Bool | `path: value` |
| Null | Skipped (no semantic content) |
| Top-level primitive | Emitted as-is without a path prefix |

`json_fingerprint` is public and can be called directly when you need to inspect or log the fingerprint produced for a given document.

---

## Thread safety

`VectorEngine` is `Send + Sync`. All store operations are serialised by an internal `parking_lot::Mutex`. For high write throughput, consider sharding across multiple independent `VectorEngine` instances.

```rust
use std::sync::Arc;

let engine = Arc::new(VectorEngine::with_embedding("/data/vecs", emb)?);

let e = engine.clone();
std::thread::spawn(move || {
    e.store_vector("thread-doc", vec![0.1, 0.9], None).unwrap();
});
```

---

## Error handling

All methods return `bdslib::common::error::Result<T>`. Use `?` to propagate or match for inspection:

```rust
match engine.search(query_vec, 10) {
    Ok(results) => println!("{} results", results.len()),
    Err(e)      => eprintln!("search failed: {e}"),
}
```

See [`common::error`](COMMON.md) for the shared error type.
