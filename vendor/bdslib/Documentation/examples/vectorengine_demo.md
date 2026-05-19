# vectorengine_demo.rs

**File:** `examples/vectorengine_demo.rs`

Demonstrates `VectorEngine`: raw vector storage, document-based storage with automatic embedding, similarity search, reranking strategies, and JSON fingerprinting.

## What it demonstrates

`VectorEngine` has two modes: raw vector mode and document mode (requires an `EmbeddingEngine`).

## Sections

| Section | Description |
|---|---|
| 1. Raw vector storage | `store_vector(id, vec, metadata)` and `search(query_vec, limit)` |
| 2. Metadata roundtrip | Extra fields in metadata survive and are returned in search results |
| 3. Upsert | Storing the same ID again replaces the vector and metadata |
| 4. `search_reranked` | `IdentityReranker` (preserves search order), `MMRReranker` (diverse results), `ScoreReranker` (custom scoring) |
| 5. Document store | `store_document(id, json)` automatically embeds text and stores the vector |
| 6. `search_json` | Embed query string and search; results include full JSON metadata |
| 7. `search_json_reranked` | MMR reranking for diversity across JSON document results |
| 8. `json_fingerprint` | Convert a JSON value to a human-readable string for deduplication |
| 9. Clone sharing | Cloned engines share the same HNSW index |

## Key API

| Method | Description |
|---|---|
| `VectorEngine::new(path)` | Create a raw-vector store |
| `VectorEngine::with_embedding(path, engine)` | Create a document store with embedding |
| `store_vector(id, vec, meta)` | Store a raw vector with metadata |
| `store_document(id, json)` | Embed a JSON document and store its vector |
| `search(query_vec, limit)` | HNSW nearest-neighbour search |
| `search_reranked(query_vec, limit, pool, reranker)` | Search with reranking |
| `search_json(text, limit)` | Embed query and search documents |
| `search_json_reranked(text, limit, pool, reranker)` | Embed query and search with reranking |
| `json_fingerprint(value)` | Flatten JSON into a canonical string |
| `sync()` | Persist the HNSW index to disk |

## Reranking strategies

| Reranker | Description |
|---|---|
| `IdentityReranker` | Pass-through; results in same order as HNSW |
| `MMRReranker(lambda)` | Maximal Marginal Relevance — penalises redundant results |
| `ScoreReranker(fn)` | Custom scoring function applied after HNSW |

## `json_fingerprint` format

| Input | Output |
|---|---|
| `"hello"` | `"hello"` |
| `42` | `"42"` |
| `{"a": 1}` | `"a: 1"` |
| `{"a": {"b": "x"}}` | `"a.b: x"` |
| `[1, 2]` | `"[0]: 1\n[1]: 2"` |
| `{"tags": ["x", "y"]}` | `"tags[0]: x\ntags[1]: y"` |
