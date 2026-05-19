# v2/doc.search

Semantic search in the document store by embedding a plain-text query with the shared `AllMiniLML6V2` model. Searches both `":meta"` and `":content"` HNSW slots, deduplicates hits by UUID, and returns the `limit` most relevant documents sorted by cosine similarity descending.

Both small documents (stored with `v2/doc.add`) and chunked documents (stored with `v2/doc.add.file`) compete in the same result set.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Logged but not used for routing. |
| `query` | string | yes | — | Plain-text query string. Embedded with the same model as stored documents. |
| `limit` | integer | no | `10` | Maximum number of results to return. |

## Response

```json
{
  "results": [
    {
      "id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
      "metadata": { "document_name": "Payment Service Incident Runbook", "chunk_index": 2, "n_chunks": 20, "document_id": "…" },
      "document": "When the circuit breaker opens, immediately page the on-call engineer…",
      "score": 0.934
    }
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `results` | array | Ranked results, sorted by `score` descending. |
| `results[].id` | string | UUID of the matching document or chunk. |
| `results[].metadata` | object | JSON metadata for this record. Chunk records include `document_id` and `chunk_index`; small-doc records have a flat metadata structure. |
| `results[].document` | string | UTF-8 decoded content text. |
| `results[].score` | number | Cosine similarity score in `[0, 1]`. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/doc.search",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "query": "circuit breaker payment service recovery",
      "limit": 5
    },
    "id": 1
  }' | jq
```

## bdscmd

```bash
bdscmd doc-search --query "circuit breaker payment service recovery" --limit 5
```

## Error responses

| Code | Condition |
|---|---|
| `-32001` | `ShardsManager` singleton not initialised |
| `-32011` | Document store search failed |

## RAG retrieval pattern

```
mgr.doc_search_text(query, limit)
    → result[].metadata["document_id"]   # present only for chunked docs
    → doc_get_metadata(document_id)       # fetch ordered "chunks" list
    → doc_get_content(chunks[i-1..=i+1]) # expand ±1 context window
```

Chunk metadata contains `document_id` only for records created by `v2/doc.add.file`. If no result has `document_id`, all results are whole-document records and the content field is the full context.
