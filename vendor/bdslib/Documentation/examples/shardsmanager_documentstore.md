# shardsmanager_documentstore.rs

**File:** `examples/shardsmanager_documentstore.rs`

Demonstrates `ShardsManager` with its embedded `DocumentStorage` accessed through the `doc_*` helper methods defined in `shardsmanager_docstore.rs`. The scenario is an operations AI assistant for a payment-processing platform: telemetry (metrics, logs, events) arrives continuously via the shard system while runbooks and post-mortems live in the embedded document store. Sections 6 and 7 show the complete RAG pattern — a telemetry alert triggers a semantic chunk search, a context window is expanded from the runbook chunk list, and the two result sets are assembled into a single prompt-ready context block.

## What it demonstrates

`ShardsManager::with_embedding` now initialises a `DocumentStorage` at `{dbpath}/docstore` using the same `EmbeddingEngine` as the shard cache. All `doc_*` methods on `ShardsManager` delegate directly to that store. Small documents (runbooks short enough to be a single record) are stored with `doc_add`. Large documents (multi-phase runbooks, post-mortems) are stored with `doc_add_from_file`, which splits them into overlapping chunks and indexes each chunk in the shared HNSW index. Both kinds of documents compete in the same `doc_search_text` / `doc_search_json` search calls.

## Sections

| Section | Description |
|---|---|
| 1. Construction | Write hjson config; `ShardsManager::with_embedding`; show docstore path (`{dbpath}/docstore`) and the shared embedding model |
| 2. Telemetry ingestion | `add_batch` for 4 phases × 30 records (peak → incident → mitigation → recovery) focused on payment-service incident messages |
| 3. Small runbooks (`doc_add`) | Store three short procedure documents (circuit breaker, DB connection pool, memory pressure); `doc_get_metadata` + `doc_get_content` round-trip |
| 4. Large documents (`doc_add_from_file`) | Write payment incident runbook (3 374 chars, 20 chunks at slice=220, overlap=20 %) and memory post-mortem (3 197 chars, 14 chunks at slice=260, overlap=15 %); inspect chunk-level metadata and content |
| 5. Semantic document search | `doc_search_text` for circuit-breaker, OOM, and DB-pool topics; `doc_search_json` with structured metadata query — results show score, chunk index, and content preview |
| 6. RAG: alert → runbook retrieval | `search_fts` finds live circuit-breaker telemetry records (Step 1); query is constructed from alert context (Step 2); `doc_search_text` ranks runbook chunks (Step 3); top chunked hit's `document_id` is used to load the ordered chunk list and expand to a ±1 context window (Step 4); telemetry + runbook assembled into a prompt block (Step 5) |
| 7. Hybrid telemetry+document search | `vectorsearch` over shard store for "connection pool exhausted"; `doc_search_text` for the same failure mode; context expansion on the top doc hit; combined output shows live incident UUIDs alongside the matching runbook passage |
| 8. Document management | `doc_update_metadata` adds a revision field; `doc_update_content` appends a note; `doc_delete` removes a document; `doc_get_metadata` confirms deletion; remaining documents still searchable |
| 9. Fingerprinted output + `doc_sync` | `doc_search_text_strings` returns flat `json_fingerprint` strings ready for re-embedding or FTS ingestion; `doc_sync` flushes HNSW index; clone sharing confirmed |

## Key API

All `doc_*` methods are defined in `src/shardsmanager_docstore.rs` as an `impl ShardsManager` block.

| Method | Description |
|---|---|
| `mgr.docstore()` | Borrow the embedded `DocumentStorage` directly |
| `mgr.doc_add(metadata, content)` | Store a small document; auto-embeds both metadata and content |
| `mgr.doc_add_from_file(path, name, slice, overlap)` | Load, chunk, and store a large text file |
| `mgr.doc_get_metadata(id)` | `Option<JsonValue>` for any UUID (chunk or document level) |
| `mgr.doc_get_content(id)` | `Option<Vec<u8>>` for any chunk UUID |
| `mgr.doc_update_metadata(id, meta)` | Replace metadata in-place |
| `mgr.doc_update_content(id, content)` | Replace content in-place |
| `mgr.doc_delete(id)` | Remove from all three sub-stores |
| `mgr.doc_search_text(query, limit)` | Embed plain-text query → ranked chunk/doc results |
| `mgr.doc_search_json(query, limit)` | Embed `json_fingerprint(query)` → ranked results |
| `mgr.doc_search_strings(query_vec, limit)` | Pre-computed vector search → fingerprint strings |
| `mgr.doc_search_text_strings(query, limit)` | Text query → fingerprint strings |
| `mgr.doc_store_metadata_vector(id, vec, meta)` | Post-hoc `:meta` vector indexing |
| `mgr.doc_store_content_vector(id, vec)` | Post-hoc `:content` vector indexing |
| `mgr.doc_sync()` | Flush HNSW index to disk |

Combined with the shard-level search methods:

| Method | Used in demo |
|---|---|
| `mgr.search_fts(duration, query)` | Section 6 — FTS alert for "circuit breaker" |
| `mgr.vectorsearch(duration, query, limit)` | Section 7 — semantic telemetry search |
| `mgr.add_batch(docs)` | Section 2 — bulk telemetry ingestion |

## RAG retrieval pattern (sections 6 and 7)

```
[Telemetry alert fires]
    ↓ mgr.search_fts("6h", "circuit breaker")
[N matching telemetry records with timestamps and message text]
    ↓ construct query from alert context
[RAG query string]
    ↓ mgr.doc_search_text(query, 4)
[Ranked chunk results: score, document_name, chunk_index, n_chunks, content]
    ↓ find first chunked result (has "document_id" in metadata)
    ↓ mgr.doc_get_metadata(document_id)  →  ordered "chunks" list
    ↓ lo = chunk_index - 1,  hi = chunk_index + 1  (clamped)
    ↓ mgr.doc_get_content(chunks[lo..=hi])
[Expanded context window: 3 adjacent chunks concatenated]
    ↓ combine
┌────────────────────────────────────────────
│ [TELEMETRY ALERT CONTEXT]
│   log.error: automatic circuit breaker opened on payment service
│   log.error: circuit breaker still open on payment service
│   log.info:  circuit breaker closed payment service recovered
│
│ [RUNBOOK CONTEXT — Payment Service Incident Runbook  chunks 0–2/20]
│   "This runbook covers P1 and P2 incidents affecting the payment
│    processing service. Follow this procedure for any alert involving
│    payment service circuit breaker trips…"
└────────────────────────────────────────────
```

## Small vs large document distinction

`doc_add` stores a single blob with no chunking. The metadata has a flat structure with whatever fields the caller provides (`name`, `category`, `service`, `severity`). The whole content is the context.

`doc_add_from_file` stores one record per chunk plus one document-level record. Chunk metadata contains `document_name`, `document_id`, `chunk_index`, `n_chunks`. The document-level record contains `name`, `path`, `slice`, `overlap`, `n_chunks`, `chunks` (ordered UUID list). The context window expansion pattern (Step 4 in section 6) applies only to chunked documents.

The code handles both cases: it finds the first result whose metadata contains `"document_id"` (a chunk) and expands it; if all results are small documents it uses the whole content of the top result as context.

## Shared embedding engine

`ShardsManager::with_embedding` clones the `EmbeddingEngine` before handing one copy to `ShardsCache` and one to `DocumentStorage`. Both copies share the same underlying `Arc`-wrapped model weights — no extra memory, no extra download. All embedding-based methods (`doc_search_text`, `doc_search_json`, shard-level `search_vector`) use the same model, which means query embeddings are comparable across both search paths.

## Document corpus used in the demo

| Document | Type | Storage | Size | Chunks |
|---|---|---|---|---|
| Circuit Breaker Quick Reference | Runbook | `doc_add` | 990 bytes | 1 (whole doc) |
| Database Connection Pool Emergency | Runbook | `doc_add` | 814 bytes | 1 (whole doc) |
| Memory Pressure Quick Response | Runbook | `doc_add` | 839 bytes | 1 (whole doc) |
| Payment Service Incident Runbook | Runbook | `doc_add_from_file` | 3 374 chars | 20 (slice=220, overlap=20 %) |
| Memory Exhaustion Post-Mortem | Post-mortem | `doc_add_from_file` | 3 197 chars | 14 (slice=260, overlap=15 %) |

## Running

```bash
cargo run --example shardsmanager_documentstore
```

Requires network access on first run to download `AllMiniLML6V2` (~23 MB). Subsequent runs use the cached model from `~/.cache/huggingface/hub`.
