# DocumentStorage API

`DocumentStorage` combines three independent stores — [`JsonStorage`](COMMON.md) (metadata), [`BlobStorage`](COMMON.md) (raw bytes), and a single [`VectorEngine`](VECTORENGINE.md) (HNSW index) — into one rooted store keyed by a common UUIDv7.

Every document is identified by one UUID. Two vector entries share that UUID as their prefix: `"{uuid}:meta"` for the metadata-fingerprint embedding and `"{uuid}:content"` for the content embedding. Both live in the same HNSW index, so a single search query can surface a document via either signal. Search results deduplicate raw hits by UUID (keeping the best score per document), sort by descending cosine similarity, and resolve each UUID to its full metadata and content before returning.

When created via [`new`](#new) (no embedding engine), [`add_document`](#add_document) still writes to the JSON and blob stores; vector indexing is silently skipped. Supply pre-computed vectors via [`add_document_with_vectors`](#add_document_with_vectors), or enable automatic embedding with [`with_embedding`](#with_embedding).

All methods return `bdslib::common::error::Result<T>` — an alias for `Result<T, easy_error::Error>` defined in the shared [`common::error`](COMMON.md) module.

---

## Directory layout

`DocumentStorage::new` (and `with_embedding`) creates the following directory tree under `root` automatically:

```
{root}/
├── metadata.db    — JsonStorage: per-document JSON metadata
├── blobs.db       — BlobStorage: raw document bytes
└── vectors/       — VectorEngine: combined HNSW index
                     IDs: "{uuid}:meta" and "{uuid}:content"
```

The three sub-stores are fully independent on disk. `metadata.db` and `blobs.db` are DuckDB databases; `vectors/` is a vecstore HNSW index directory.

---

## Construction

### `new`

```rust
DocumentStorage::new(root: &str) -> Result<DocumentStorage>
```

Open or create a `DocumentStorage` rooted at `root`. The full directory tree (including `vectors/`) is created automatically if it does not exist.

`add_document` is available but will **not** index vectors — the embedding engine is absent. Use [`with_embedding`](#with_embedding) to enable automatic vector indexing, or [`add_document_with_vectors`](#add_document_with_vectors) to supply pre-computed vectors.

```rust
use bdslib::DocumentStorage;

let store = DocumentStorage::new("/data/documents")?;
```

### `with_embedding`

```rust
DocumentStorage::with_embedding(root: &str, engine: EmbeddingEngine) -> Result<DocumentStorage>
```

Open or create a `DocumentStorage` rooted at `root`, attaching an `EmbeddingEngine` for automatic vector indexing. Enables [`add_document`](#add_document) to embed and index both the metadata fingerprint and the raw content on every insert.

```rust
use bdslib::{DocumentStorage, EmbeddingEngine, embedding::Model};

let emb = EmbeddingEngine::new(Model::AllMiniLML6V2, None)?;
let store = DocumentStorage::with_embedding("/data/documents", emb)?;
```

`DocumentStorage` is `Clone`. Cloning is cheap — all clones share the same underlying sub-stores and embedding model via `Arc`.

---

## Writing

### `add_document`

```rust
fn add_document(&self, metadata: JsonValue, content: &[u8]) -> Result<Uuid>
```

Store a document and return its generated UUIDv7.

- `metadata` is written verbatim to `metadata.db`.
- `content` is written as raw bytes to `blobs.db`.
- If an `EmbeddingEngine` is attached, the `json_fingerprint` of `metadata` is embedded and stored under `"{uuid}:meta"` and `content` (decoded as UTF-8) is embedded and stored under `"{uuid}:content"` in the shared vector index. Vector indexing errors are **silently discarded** (`let _ = ...`) — a missing engine does not cause `add_document` to fail.

```rust
use bdslib::DocumentStorage;
use serde_json::json;

let id = store.add_document(
    json!({ "title": "Rust in Action", "author": "Tim McNamara", "year": 2021 }),
    b"Chapter 1: Introducing Rust ...",
)?;

println!("stored document {id}");
```

### `add_document_with_vectors`

```rust
fn add_document_with_vectors(
    &self,
    metadata: JsonValue,
    content: &[u8],
    meta_vec: Vec<f32>,
    content_vec: Vec<f32>,
) -> Result<Uuid>
```

Store a document using caller-supplied pre-computed vectors. This is the embedding-free path — no `EmbeddingEngine` is required and all four stores are always written.

- `meta_vec` is stored under `"{uuid}:meta"` with `metadata` as vecstore metadata.
- `content_vec` is stored under `"{uuid}:content"` with no extra metadata.

Use this when you generate embeddings externally (batch jobs, GPU pipelines, etc.) and want deterministic, testable behaviour without relying on an attached engine.

```rust
use bdslib::DocumentStorage;
use serde_json::json;

// Pre-computed 3-dimensional embeddings (illustrative; real models produce
// 384- or 768-dimensional vectors).
let meta_vec    = vec![0.1_f32, 0.8, 0.3];
let content_vec = vec![0.2_f32, 0.7, 0.4];

let id = store.add_document_with_vectors(
    json!({ "title": "HNSW Explained", "domain": "vector-search" }),
    b"Hierarchical Navigable Small World graphs ...",
    meta_vec,
    content_vec,
)?;
```

### `add_document_from_file`

```rust
fn add_document_from_file(
    &self,
    path: &str,
    name: &str,
    slice: usize,
    overlap: f32,
) -> Result<Uuid>
```

Load a text file from `path`, split it into overlapping chunks on natural sentence and paragraph boundaries, and store each chunk as an independently searchable record. Returns the UUIDv7 of the document-level metadata record.

This is the primary entry point for RAG (Retrieval-Augmented Generation) ingestion. Every chunk is a first-class document in `BlobStorage` and `JsonStorage`, and (when an `EmbeddingEngine` is attached) both its content and metadata fingerprint are indexed in the shared HNSW vector index. The document-level record holds the ordered list of chunk UUIDs so a retriever can expand context by fetching neighbouring chunks.

**Parameters**

| Parameter | Type | Description |
|---|---|---|
| `path` | `&str` | Filesystem path to read (`std::fs::read_to_string`) |
| `name` | `&str` | Human-readable document name stored in all metadata records |
| `slice` | `usize` | Maximum character count per chunk (clamped to `≥ 1`) |
| `overlap` | `f32` | Overlap as a percentage of `slice` (clamped to `[0.0, 99.0]`). E.g. `25.0` means the last ≈ 25 % of a chunk reappears at the start of the next. |

**Chunking algorithm**

The text is first split into *atoms* — sentences, paragraphs, or individual words (never exceeding `slice` characters each). Atoms are accumulated into a sliding window that grows until adding the next atom would exceed `slice`. The overlap region (the last `overlap_chars` characters of the current chunk) carries over to the start of the next window, so adjacent chunks share context at their boundaries.

The hierarchy used when splitting:
1. Paragraph boundaries (`\n\n`)
2. Sentence boundaries (`.`, `!`, `?` followed by whitespace + uppercase or non-alpha — avoids splitting on `Mr.`, `e.g.`)
3. Soft line breaks (`\n`)
4. Word boundaries (whitespace)
5. Hard cut at `slice` characters for tokens that cannot be split further

**Storage layout**

For each chunk `i`:

| Store | Key | Value |
|---|---|---|
| `BlobStorage` | chunk UUIDv7 | Raw UTF-8 bytes of the chunk text |
| `JsonStorage` | chunk UUIDv7 | `{"document_name", "document_id", "chunk_index", "n_chunks"}` |
| VectorEngine | `"{chunk_id}:content"` | Embedding of the chunk text (if engine present) |
| VectorEngine | `"{chunk_id}:meta"` | Embedding of the chunk metadata fingerprint (if engine present) |

For the document level:

| Store | Key | Value |
|---|---|---|
| `JsonStorage` | doc UUIDv7 | `{"name", "path", "slice", "overlap", "n_chunks", "chunks": [uuid, …]}` |
| VectorEngine | `"{doc_id}:meta"` | Embedding of the document metadata fingerprint (if engine present) |

The `chunks` array is ordered — `chunks[0]` is the first chunk in the file, `chunks[n_chunks-1]` is the last. All chunk UUIDs are also monotonically ordered by time, so lexicographic sort preserves document order.

**Example**

```rust
use bdslib::DocumentStorage;

let store = DocumentStorage::new("/data/rag")?;

// Split a 50 000-character essay into ≤ 512-character chunks
// with 20 % overlap between adjacent chunks.
let doc_id = store.add_document_from_file(
    "/data/papers/attention_is_all_you_need.txt",
    "Attention Is All You Need",
    512,
    20.0,
)?;

// Retrieve document-level metadata (chunk list, path, …)
let meta = store.get_metadata(doc_id)?.unwrap();
let chunk_ids: Vec<&str> = meta["chunks"]
    .as_array().unwrap()
    .iter()
    .map(|v| v.as_str().unwrap())
    .collect();

println!("stored {} chunks", meta["n_chunks"]);

// Fetch the first chunk
let first_id: uuid::Uuid = chunk_ids[0].parse().unwrap();
let first_chunk = store.get_content(first_id)?.unwrap();
println!("{}", String::from_utf8_lossy(&first_chunk));
```

**RAG retrieval pattern**

```rust
// Semantic search over all chunk vectors.
let results = store.search_document(query_vec, 5)?;

for r in &results {
    println!("chunk: {:.4}  {}", r["score"], r["metadata"]["document_name"]);
    println!("  doc_id:      {}", r["metadata"]["document_id"]);
    println!("  chunk_index: {}", r["metadata"]["chunk_index"]);
    println!("  content:     {}", &r["document"].as_str().unwrap()[..80]);
}
```

To expand context, fetch neighbouring chunks using `chunk_index ± 1` from the document-level `chunks` array.

**Errors**

Returns `Err` if `path` cannot be read. Storage errors from sub-store writes are propagated. Vector indexing failures are silently discarded (`let _ = …`) — a missing embedding engine does not cause the call to fail.

---

### `update_metadata`

```rust
fn update_metadata(&self, id: Uuid, metadata: JsonValue) -> Result<()>
```

Replace the metadata stored under `id` and set its `updated_at` timestamp to now. Returns `Ok(())` even when `id` does not exist (no-op).

The vector index is **not** updated. After calling `update_metadata`, call [`store_metadata_vector`](#store_metadata_vector) if the updated metadata should be re-embedded and re-indexed.

```rust
use serde_json::json;

store.update_metadata(id, json!({ "title": "Rust in Action", "edition": 2 }))?;
```

### `update_content`

```rust
fn update_content(&self, id: Uuid, content: &[u8]) -> Result<()>
```

Replace the raw content stored under `id` and set its `updated_at` timestamp to now. Returns `Ok(())` even when `id` does not exist (no-op).

```rust
store.update_content(id, b"Revised Chapter 1: Introducing Rust ...")?;
```

### `delete_document`

```rust
fn delete_document(&self, id: Uuid) -> Result<()>
```

Remove the document from all three stores. Both `"{uuid}:meta"` and `"{uuid}:content"` vector entries are deleted from the HNSW index, and the metadata and blob records are removed from their respective DuckDB databases. Returns `Ok(())` for non-existent `id` (no-op in each sub-store).

```rust
store.delete_document(id)?;
```

### `store_metadata_vector`

```rust
fn store_metadata_vector(&self, id: Uuid, meta_vec: Vec<f32>, metadata: JsonValue) -> Result<()>
```

Explicitly (re-)index a metadata vector for `id`. Stored under `"{id}:meta"` in the shared HNSW index with `metadata` persisted as vecstore metadata. Use this after [`update_metadata`](#update_metadata) to keep the vector index in sync.

```rust
// After updating metadata, re-embed and re-index.
let new_meta = json!({ "title": "Rust in Action", "edition": 2 });
store.update_metadata(id, new_meta.clone())?;

let new_vec = embedding_engine.embed_json(&new_meta)?;
store.store_metadata_vector(id, new_vec, new_meta)?;
```

### `store_content_vector`

```rust
fn store_content_vector(&self, id: Uuid, content_vec: Vec<f32>) -> Result<()>
```

Explicitly (re-)index a content vector for `id`. Stored under `"{id}:content"` in the shared HNSW index with no extra metadata. Use this after [`update_content`](#update_content) to keep the vector index in sync.

```rust
store.update_content(id, b"Revised Chapter 1 ...")?;
let new_vec = embedding_engine.embed_text("Revised Chapter 1 ...")?;
store.store_content_vector(id, new_vec)?;
```

---

## Reading

### `get_metadata`

```rust
fn get_metadata(&self, id: Uuid) -> Result<Option<JsonValue>>
```

Return the metadata stored under `id`, or `None` if no such document exists.

```rust
if let Some(meta) = store.get_metadata(id)? {
    println!("title: {}", meta["title"]);
}
```

### `get_content`

```rust
fn get_content(&self, id: Uuid) -> Result<Option<Vec<u8>>>
```

Return the raw content bytes stored under `id`, or `None` if no such document exists.

```rust
if let Some(bytes) = store.get_content(id)? {
    let text = String::from_utf8_lossy(&bytes);
    println!("{text}");
}
```

---

## Searching

All three search methods query the same shared HNSW index. Raw hits for both `":meta"` and `":content"` entries compete together. Hits are deduplicated by UUID (keeping the highest score per document), sorted by descending cosine similarity, and then the top `limit` documents are resolved to full records by loading metadata from `JsonStorage` and content from `BlobStorage`.

### `search_document`

```rust
fn search_document(&self, query_vec: Vec<f32>, limit: usize) -> Result<Vec<JsonValue>>
```

Return the `limit` most relevant documents for a pre-computed query vector.

The internal candidate pool is `limit * 4` to give both `":meta"` and `":content"` slots a fair chance to compete before deduplication.

```rust
// query_vec must match the dimensionality used when storing vectors.
let results = store.search_document(query_vec, 5)?;
for r in &results {
    println!("{} — score {:.4}", r["id"], r["score"]);
}
```

### `search_document_json`

```rust
fn search_document_json(&self, query: &JsonValue, limit: usize) -> Result<Vec<JsonValue>>
```

Fingerprint `query` with [`json_fingerprint`](VECTORENGINE.md#json_fingerprint), embed the result, and return the `limit` most relevant documents.

Returns `Err` if no `EmbeddingEngine` was provided at construction time.

Use the same JSON structure as was passed to `add_document` so that field paths in the query align with field paths in the index.

```rust
use serde_json::json;

let results = store.search_document_json(
    &json!({ "title": "Rust", "domain": "systems" }),
    5,
)?;
```

### `search_document_text`

```rust
fn search_document_text(&self, query: &str, limit: usize) -> Result<Vec<JsonValue>>
```

Embed `query` as plain text and return the `limit` most relevant documents. Internally wraps the string in a JSON string value and delegates to `search_document_json`.

Returns `Err` if no `EmbeddingEngine` was provided at construction time.

```rust
let results = store.search_document_text("memory safety in systems programming", 5)?;
```

---

## Search result format

Each element returned by the search methods is a JSON object with four keys:

```json
{
  "id":       "018f4b2c-1234-7abc-8def-000000000001",
  "metadata": { "title": "Rust in Action", "author": "Tim McNamara" },
  "document": "Chapter 1: Introducing Rust ...",
  "score":    0.97
}
```

| Key | Type | Description |
|---|---|---|
| `"id"` | string | UUIDv7 that identifies the document |
| `"metadata"` | object or null | JSON metadata from `metadata.db`; `null` if the document was deleted between indexing and retrieval |
| `"document"` | string | Raw content bytes decoded as UTF-8 (invalid bytes replaced with U+FFFD) |
| `"score"` | number | Cosine similarity in `[0.0, 1.0]`; higher is more similar |

Results are ordered by descending `score` — the most relevant document is first. Because the index holds both `":meta"` and `":content"` entries, the score for a given document is the maximum of its two slot scores.

---

## String output

`results_to_strings` applies [`json_fingerprint`](VECTORENGINE.md#json_fingerprint) to each result object, flattening it into a `"path: value"` space-joined string. This format is convenient for feeding search results directly back into an embedding pipeline or a full-text index without an extra mapping step.

### `results_to_strings`

```rust
pub fn results_to_strings(results: &[JsonValue]) -> Vec<String>
```

Free function, re-exported from `bdslib`. Convert a slice of search-result JSON objects (as returned by any `search_document*` method) into a `Vec<String>` by applying `json_fingerprint` to each element.

```rust
use bdslib::documentstorage::results_to_strings;

let results = store.search_document(query_vec, 5)?;
let strings = results_to_strings(&results);

// Each string looks like:
// "id: 018f4b2c-... metadata.title: Rust in Action document: Chapter 1 ... score: 0.97"
for s in &strings {
    println!("{s}");
}
```

### `search_document_strings`

```rust
fn search_document_strings(&self, query_vec: Vec<f32>, limit: usize) -> Result<Vec<String>>
```

Like [`search_document`](#search_document), but returns each result serialised to a `json_fingerprint` string instead of a raw `JsonValue`.

```rust
let strings = store.search_document_strings(query_vec, 5)?;
// Pass directly to a re-embedding or FTS ingestion step.
fts_index.add_documents(&strings)?;
```

### `search_document_json_strings`

```rust
fn search_document_json_strings(&self, query: &JsonValue, limit: usize) -> Result<Vec<String>>
```

Like [`search_document_json`](#search_document_json), but returns fingerprint strings. Returns `Err` if no `EmbeddingEngine` was provided at construction time.

```rust
use serde_json::json;

let strings = store.search_document_json_strings(
    &json!({ "domain": "nlp", "topic": "embeddings" }),
    5,
)?;
```

### `search_document_text_strings`

```rust
fn search_document_text_strings(&self, query: &str, limit: usize) -> Result<Vec<String>>
```

Like [`search_document_text`](#search_document_text), but returns fingerprint strings. Returns `Err` if no `EmbeddingEngine` was provided at construction time.

```rust
let strings = store.search_document_text_strings("vector similarity search", 5)?;
```

---

## Persistence

### `sync`

```rust
fn sync(&self) -> Result<()>
```

Flush the in-memory HNSW vector index to disk. Call after a batch of writes to guarantee durability of vector data across process restarts.

The DuckDB-backed stores (`metadata.db`, `blobs.db`) checkpoint automatically; `sync` is only necessary for the vecstore index.

```rust
store.add_document_with_vectors(meta1, content1.as_bytes(), mv1, cv1)?;
store.add_document_with_vectors(meta2, content2.as_bytes(), mv2, cv2)?;
store.sync()?; // flush HNSW index before process exits
```

---

## Graceful degradation

`add_document` uses `let _ = self.vectors.store_document(...)` for both vector insert calls. Errors from a missing embedding engine — or any other vector-store failure — are intentionally discarded. The metadata and blob writes always proceed and the returned `Uuid` is always valid.

`add_document_with_vectors` calls `self.vectors.store_vector(...)` without `let _` and propagates errors normally — because no embedding engine is involved, the only realistic failures are I/O errors that the caller should see.

| Method | Behaviour without EmbeddingEngine |
|---|---|
| `add_document` | Writes metadata + blob; silently skips vector indexing |
| `add_document_with_vectors` | Always indexes both vectors (no engine needed) |
| `add_document_from_file` | Writes all chunk/doc records; silently skips vector indexing |
| `search_document` | Works; results are empty if no vectors were ever stored |
| `search_document_json` | Returns `Err` |
| `search_document_text` | Returns `Err` |
| `search_document_json_strings` | Returns `Err` |
| `search_document_text_strings` | Returns `Err` |

---

## Thread safety

`DocumentStorage` is `Send + Sync`. All three sub-stores are independently thread-safe and backed by `Arc`. Cloning is cheap and all clones share the same underlying state.

```rust
use std::sync::Arc;

let store = Arc::new(DocumentStorage::with_embedding("/data/docs", emb)?);

let s = store.clone();
std::thread::spawn(move || {
    let id = s.add_document(
        serde_json::json!({ "source": "thread" }),
        b"content from another thread",
    ).unwrap();
    println!("thread stored {id}");
});
```

For high write throughput, consider sharding across multiple independent `DocumentStorage` instances (each rooted at a different directory), mirroring the pattern recommended for [`VectorEngine`](VECTORENGINE.md#thread-safety).

---

## Error handling

All methods return `bdslib::common::error::Result<T>`. Use `?` to propagate or match for inspection:

```rust
match store.search_document_text("systems programming", 10) {
    Ok(results) => println!("{} results", results.len()),
    Err(e)      => eprintln!("search failed: {e}"),
}
```

See [`common::error`](COMMON.md) for the shared error type.
