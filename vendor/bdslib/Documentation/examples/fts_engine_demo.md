# fts_engine_demo.rs

**File:** `examples/fts_engine_demo.rs`

Demonstrates the `FTSEngine`: indexing text documents and querying with Tantivy BM25 full-text search.

## What it demonstrates

| Operation | Description |
|---|---|
| `FTSEngine::new(":memory:")` | Create an in-memory Tantivy index |
| `FTSEngine::new(path)` | Create a file-backed persistent index |
| `add_document(id, text)` | Index a text document under a given UUID |
| `search(query, limit)` | BM25 full-text search; returns `(UUID, score)` pairs |
| `drop_document(id)` | Remove a document from the index |
| `sync()` | Flush the index to disk (for file-backed engines) |

## Sections in the demo

1. **Single-term search** — index five documents; search for a single keyword
2. **AND query** — Tantivy boolean AND: `"rust AND storage"`
3. **Phrase query** — exact phrase match: `"\"fast storage\""`
4. **Limit** — request at most 2 results from a query that matches more
5. **Delete and re-search** — drop a document; confirm it no longer appears
6. **UUIDv7 ordering** — later-added documents have larger UUIDs, enabling chronological sort
7. **sync** — demonstrate flushing a file-backed index

## Key concepts

**BM25 scoring** — results are ranked by term frequency and inverse document frequency. Higher scores indicate better matches.

**Tantivy query syntax** — the `search` method passes the query string directly to Tantivy. Supported forms: single terms, `AND`/`OR`/`NOT` boolean operators, phrase queries (`"..."`),.

**In-memory vs file-backed** — `:memory:` creates a non-persistent index suitable for tests or ephemeral use. A filesystem path creates a persistent index that survives process restarts.

## Example output

```
search "storage": [uuid1 (1.23), uuid3 (0.98)]
search "rust AND fast": [uuid2 (2.10)]
after drop: search "deleted term": []
```
