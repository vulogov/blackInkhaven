# documentstorage_demo.rs

**File:** `examples/documentstorage_demo.rs`

Demonstrates `DocumentStorage`: a combined store that keeps JSON metadata, raw byte content, and a unified HNSW vector index under one roof. Every document gets a UUIDv7 and is searchable via pre-computed vectors, with optional automatic embedding when an `EmbeddingEngine` is attached.

## What it demonstrates

`DocumentStorage` layers three sub-stores — `JsonStorage` for metadata, `BlobStorage` for raw content, and `VectorEngine` for similarity search — behind a single API. The unified vector index holds both a `":meta"` entry and a `":content"` entry per document; search deduplicates by UUID and returns the best score from either signal.

## Sections

| Section | Description |
|---|---|
| 1. Creating a store | `DocumentStorage::new(path)` — open or create a store at a filesystem path |
| 2. `add_document` | Store three documents with varied JSON metadata and byte content; returns a UUIDv7 per document |
| 3. `get_metadata` / `get_content` | Round-trip: retrieve the JSON metadata and raw bytes for a stored document by UUID |
| 4. `update_metadata` / `update_content` | Replace metadata or content in-place; UUID is unchanged |
| 5. `delete_document` | Remove a document from all three sub-stores at once |
| 6. `add_document_with_vectors` | Insert a document with caller-supplied pre-computed 3-D unit vectors instead of embedding |
| 7. `search_document` | Unified HNSW search over the shared vector index; results are JSON objects with `id`, `metadata`, `document`, and `score` |
| 8. `results_to_strings` | Apply `json_fingerprint` to each search result to produce flat canonical strings |
| 9. `search_document_strings` | One-shot: search and return fingerprint strings directly |
| 10. `store_metadata_vector` / `store_content_vector` | Post-hoc indexing: (re-)index a vector for an existing document's metadata or content slot |
| 11. Sync + reopen | Call `sync()` to flush the HNSW index, then open the same path in a new handle and re-run a search — documents survive the restart |
| 12. Clone sharing | Add a document via the original handle; search via a cloned handle — all clones share the same underlying state |
| 13. Freeform text search | `DocumentStorage::with_embedding` + `add_document` + `search_document_text` — index five books by embedding their metadata and content automatically, then retrieve them with plain-text and JSON queries showing combined metadata and raw document content in each result |

## Key API

| Method | Description |
|---|---|
| `DocumentStorage::new(root)` | Open or create a store (no embedding; vector indexing skipped on `add_document`) |
| `DocumentStorage::with_embedding(root, engine)` | Open or create a store with an `EmbeddingEngine` for automatic vector indexing |
| `add_document(metadata, content)` | Store metadata + blob; embed automatically if engine present; returns UUIDv7 |
| `add_document_with_vectors(metadata, content, meta_vec, content_vec)` | Store with caller-supplied pre-computed vectors |
| `get_metadata(id)` | Return `Option<JsonValue>` for the document's metadata |
| `get_content(id)` | Return `Option<Vec<u8>>` for the document's raw bytes |
| `update_metadata(id, metadata)` | Replace metadata; vector index is not updated automatically |
| `update_content(id, content)` | Replace raw content |
| `delete_document(id)` | Remove from metadata store, blob store, and vector index |
| `search_document(query_vec, limit)` | HNSW search over unified index; deduplicates by UUID |
| `search_document_strings(query_vec, limit)` | Same as `search_document` but returns `json_fingerprint` strings |
| `search_document_json(query, limit)` | Embed a JSON query and search (requires `with_embedding`) |
| `search_document_text(query, limit)` | Embed a plain-text query and search (requires `with_embedding`) |
| `store_metadata_vector(id, vec, metadata)` | Explicitly (re-)index the `":meta"` vector slot |
| `store_content_vector(id, vec)` | Explicitly (re-)index the `":content"` vector slot |
| `results_to_strings(results)` | Free function: apply `json_fingerprint` to a slice of search results |
| `sync()` | Flush the HNSW vector index to disk |

## Search result format

Each element returned by `search_document` (and its variants) is a JSON object with four keys:

| Key | Type | Description |
|---|---|---|
| `"id"` | string | UUIDv7 of the document |
| `"metadata"` | object or null | The stored JSON metadata, or `null` if the document was deleted after indexing |
| `"document"` | string | The raw content decoded as UTF-8 (invalid bytes replaced) |
| `"score"` | number | Cosine similarity in `[0.0, 1.0]`; best score across `":meta"` and `":content"` entries |

Example:

```json
{
  "id": "0192e4a7-3b1f-7000-8000-000000000001",
  "metadata": {"title": "Getting started", "author": "alice"},
  "document": "This is the document body.",
  "score": 0.97
}
```

## Freeform text search (section 13)

Section 13 is the only section that requires an embedding model. It shows the full automatic pipeline:

1. **Create** `DocumentStorage::with_embedding(root, engine)` — both write and search paths share the same `EmbeddingEngine`.
2. **Index** five books with `add_document(metadata, content)`. No vectors are computed by the caller; the engine embeds `json_fingerprint(metadata)` as `"{uuid}:meta"` and the UTF-8 content as `"{uuid}:content"` in the shared HNSW index.
3. **Freeform query** via `search_document_text("concurrent memory-safe systems programming", 3)` — the query string is embedded on the fly, then both `":meta"` and `":content"` slots of every stored document compete; results are deduplicated by UUID and each entry contains the full `metadata` object **and** the raw `document` content.
4. **JSON metadata query** via `search_document_json({"domain": "ml", "topic": "neural networks training"}, 2)` — `json_fingerprint` converts the query object to `"domain: ml topic: neural networks training"` before embedding, so field names contribute to the semantic signal.
5. **Fingerprint strings** via `search_document_text_strings(query, 2)` — same search but each result is passed through `json_fingerprint`, producing flat canonical strings ready for re-embedding or FTS ingestion.

Output for each result includes `score`, `metadata` fields (title, author, domain), and the full `document` content string — demonstrating how raw data and metadata are combined in every search response.

```
  [0.821]  The Rust Programming Language              domain=systems
           content: Memory safety without garbage collection. Ownership, borrowing, and lifetimes…
```

> **Network access**: `EmbeddingEngine::new(Model::AllMiniLML6V2, None)` downloads the model (~23 MB) on first run and caches it in the fastembed default directory (`~/.cache/huggingface/hub`). Subsequent runs use the cached model.

## Note on pre-computed vs. automatic embedding

`search_document` and `search_document_strings` accept a **pre-computed** query vector and work with any `DocumentStorage` instance, including those created with `new()`.

`search_document_json` and `search_document_text` (and their `*_strings` variants) embed the query automatically and require a `DocumentStorage` built with `with_embedding(root, engine)`. Calling them on a store without an embedding engine returns an error.

The same distinction applies on the write side: `add_document` indexes vectors automatically only when an `EmbeddingEngine` is present. Use `add_document_with_vectors` or the explicit `store_metadata_vector` / `store_content_vector` methods to index vectors on a store created with `new()`.
