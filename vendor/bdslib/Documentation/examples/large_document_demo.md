# large_document_demo.rs

**File:** `examples/large_document_demo.rs`

Demonstrates `DocumentStorage::add_document_from_file`: generating multi-paragraph text documents, ingesting them as overlapping chunks, inspecting chunk structure, and performing RAG (Retrieval-Augmented Generation) context-window expansion. Section 7 adds semantic search via `EmbeddingEngine` to complete the full RAG pipeline.

## What it demonstrates

`add_document_from_file` reads a file, splits it on paragraph → sentence → word boundaries into overlapping chunks (each a first-class document with its own UUIDv7), and stores every chunk in `BlobStorage` + `JsonStorage`. An optional `EmbeddingEngine` indexes each chunk's content and metadata fingerprint in the shared HNSW vector index. The document-level metadata record holds the ordered chunk UUID list, enabling context expansion by fetching neighbouring chunks from any retrieval hit.

## Sections

| Section | Description |
|---|---|
| 1. Generate document files | Write three multi-paragraph text documents (Rust, Distributed Systems, Deep Learning) to temporary files; show filename and character count |
| 2. Ingest with `add_document_from_file` | Open `DocumentStorage::new`, ingest each file with different `slice` and `overlap` settings; show `doc_id` and `n_chunks` per document |
| 3. Document-level metadata | `get_metadata(doc_id)` — show `name`, `path`, `slice`, `overlap`, `n_chunks`, and the first/last entries of the `chunks` UUID list |
| 4. Per-chunk inspection | For the first three chunks of the Rust document: `get_metadata(chunk_id)` shows `document_name`, `document_id`, `chunk_index`, `n_chunks`; `get_content(chunk_id)` shows a content preview |
| 5. Overlap evidence | Compare adjacent chunks to show that the end of chunk `i` and the start of chunk `i+1` share words, confirming the overlap carries context across boundaries |
| 6. RAG context window expansion | Simulate a retrieval hit on the middle chunk of the Rust document; extract `document_id` from chunk metadata; load the document-level `chunks` list; fetch chunks at `index ±1`; assemble and display the expanded context |
| 7. Semantic search with `EmbeddingEngine` | Create `DocumentStorage::with_embedding`; re-ingest the same three files (chunks indexed automatically); `search_document_text` returns ranked chunk results; expand the top hit to a ±1 context window |
| 8. Fingerprinted output | `search_document_text_strings` — same search, each result serialised through `json_fingerprint` into a flat `"path: value"` string ready for re-embedding or FTS ingestion |
| 9. sync + reopen | `sync()` flushes the HNSW index; drop and reopen from the same path using `with_embedding` (cached model, no download); verify that a chunk blob is still accessible and vector search still returns results |

## Key API

| Method | Description |
|---|---|
| `DocumentStorage::new(root)` | Open or create a store without an embedding engine |
| `DocumentStorage::with_embedding(root, engine)` | Open or create a store with automatic embedding |
| `add_document_from_file(path, name, slice, overlap)` | Load file, split into overlapping chunks, store blobs + metadata; return document UUIDv7 |
| `get_metadata(id)` | Retrieve `Option<JsonValue>` for any UUID — document-level or chunk-level |
| `get_content(id)` | Retrieve `Option<Vec<u8>>` for any chunk UUID |
| `search_document_text(query, limit)` | Embed plain-text query and search the HNSW index (requires `with_embedding`) |
| `search_document_text_strings(query, limit)` | Same, but returns `json_fingerprint` strings |
| `sync()` | Flush the HNSW vector index to disk |

## Two-level storage layout

`add_document_from_file` creates two kinds of records:

**Per-chunk records** (one per chunk, keyed by a fresh UUIDv7):

```json
{
  "document_name": "Rust: Memory Safety Without Garbage Collection",
  "document_id":   "018f4b2c-...",
  "chunk_index":   3,
  "n_chunks":      9
}
```

Blob content: raw UTF-8 bytes of the chunk text.

**Document-level record** (one per file, keyed by the document UUIDv7):

```json
{
  "name":     "Rust: Memory Safety Without Garbage Collection",
  "path":     "/tmp/.../rust.txt",
  "slice":    300,
  "overlap":  20.0,
  "n_chunks": 9,
  "chunks":   ["018f4b2c-...", "018f4b2d-...", ..., "018f4b34-..."]
}
```

The `chunks` array is ordered — `chunks[0]` is the first chunk in the file, `chunks[n_chunks-1]` is the last.

## RAG retrieval pattern (sections 6 and 7)

```
search_document_text(query, limit)
    └─ returns chunk-level results, each with:
           "metadata": { document_name, document_id, chunk_index, n_chunks }
           "document":  raw chunk text
           "score":     cosine similarity

For each hit:
    1. read  document_id  from hit["metadata"]["document_id"]
    2. get_metadata(document_id)  →  ordered chunks list
    3. lo = chunk_index - 1   (clamp to 0)
       hi = chunk_index + 1   (clamp to n_chunks - 1)
    4. get_content(chunks[lo]), get_content(chunks[chunk_index]), get_content(chunks[hi])
    5. concatenate  →  expanded context window ready for the LLM
```

## Overlap behaviour (section 5)

With `overlap=20.0`, approximately the last 20 % of each chunk (measured in characters) is shared with the start of the next chunk. The chunking algorithm operates at atom boundaries (sentences or words), so the actual overlap is the largest set of trailing atoms whose total character count does not exceed the overlap budget. This ensures no atom is split mid-word while still maintaining the target overlap fraction.

## Vector index entries per document

When `EmbeddingEngine` is attached, `add_document_from_file` stores these entries in the shared HNSW index:

| Key | Embedded text |
|---|---|
| `"{chunk_id}:content"` | Raw chunk text |
| `"{chunk_id}:meta"` | `json_fingerprint` of per-chunk metadata |
| `"{doc_id}:meta"` | `json_fingerprint` of document-level metadata |

Both `:content` and `:meta` slots compete in every search, so a query can surface a chunk via either its text or its metadata fingerprint.

## Note on network access

Section 7 and later use `EmbeddingEngine::new(Model::AllMiniLML6V2, None)`, which downloads the model (~23 MB) on first run and caches it in the fastembed default directory (`~/.cache/huggingface/hub`). Sections 1–6 use `DocumentStorage::new` (no embedding engine) and work fully offline.

## Running

```bash
cargo run --example large_document_demo
```
