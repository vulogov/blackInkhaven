# v2/aggregationsearch

Perform a parallel vector search over time-scoped telemetry shards and a semantic search over the embedded document store in a single call. Both searches use the shared `AllMiniLML6V2` model and run concurrently via `rayon::join`. Results are merged into a single JSON object with two keys:

- `"observability"` — full telemetry documents from the shard store, vector-ranked by cosine similarity descending, each carrying `_score`
- `"documents"` — ranked document-store hits (runbooks, post-mortems, any stored documents), each carrying `id`, `score`, `metadata`, and `document` content

Use this method when a single query should simultaneously retrieve live incident telemetry and the matching operational runbooks or reference documents — the core RAG retrieval pattern for operations AI assistants.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Logged but not used for routing. |
| `duration` | string | yes | — | Lookback window for the telemetry search in [`humantime`](https://docs.rs/humantime) format, e.g. `"1h"`, `"30min"`, `"7days"`. Only shards whose time interval overlaps `[now − duration, now + 1s)` are searched. The document store is not time-bounded. |
| `query` | string | yes | — | Plain-text query used for both searches. Embedded with `AllMiniLML6V2`. |

## Response

```json
{
  "observability": [
    {
      "id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
      "timestamp": 1745045200,
      "key": "log.error",
      "data": "circuit breaker opened on payment-service",
      "_score": 0.8209,
      "secondaries": []
    }
  ],
  "documents": [
    {
      "id": "019dcf6d-e5fb-7143-9aaf-3edc8d63613f",
      "score": 0.766,
      "metadata": {
        "name": "Circuit Breaker Runbook",
        "category": "runbook",
        "service": "payment"
      },
      "document": "Circuit Breaker Quick Reference…"
    },
    {
      "id": "019dcf6d-e6a7-71e2-96f8-5eea9d8797f0",
      "score": 0.608,
      "metadata": {
        "document_name": "Payment Service Incident Runbook",
        "document_id": "019dcf6d-e64d-7c13-b4c6-f7aa7698882c",
        "chunk_index": 2,
        "n_chunks": 20
      },
      "document": "When the circuit breaker opens, immediately page the on-call engineer…"
    }
  ]
}
```

### `observability` hit fields

| Field | Type | Description |
|---|---|---|
| `id` | string | UUIDv7 of the telemetry primary record. |
| `timestamp` | integer | Event time as Unix seconds. |
| `key` | string | Telemetry record key, e.g. `"log.error"` or `"cpu.usage"`. |
| `data` | any | Telemetry payload. |
| `_score` | number | Cosine similarity score in `[0, 1]`. Results are sorted descending. |
| `secondaries` | array | Secondary records linked to this primary (may be empty). |

### `documents` hit fields

| Field | Type | Description |
|---|---|---|
| `id` | string | UUID of the matching document or chunk. |
| `score` | number | Cosine similarity score in `[0, 1]`. |
| `metadata` | object | Stored metadata. Chunked docs include `document_id`, `chunk_index`, `n_chunks`. Whole docs have user-supplied fields. |
| `document` | string | UTF-8 decoded content text. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/aggregationsearch",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "duration": "2h",
      "query": "circuit breaker payment service open triage"
    },
    "id": 1
  }' | jq
```

## bdscmd

```bash
bdscmd aggregation-search \
  --query "circuit breaker payment service open triage" \
  --duration 2h
```

## Error responses

| Code | Condition |
|---|---|
| `-32001` | `ShardsManager` singleton not initialised |
| `-32004` | Search failed (e.g. invalid duration, embedding model error) |
| `-32000` | Internal task panic |

## Notes

- **Duration scoping applies only to observability.** The document store has no time dimension; the same documents appear at any window width. Widening `duration` increases the number of telemetry shards searched without changing the document results.
- **Document limit.** The document store search uses the library default of 10 results (`DEFAULT_DOC_LIMIT`). To retrieve more, call [`v2/doc.search`](v2_doc_search.md) directly with a higher `limit`.
- **Chunked vs whole-doc results.** Document hits whose `metadata` contains `"document_id"` are individual chunks from a file ingested with [`v2/doc.add.file`](v2_doc_add_file.md). Use `document_id` + `chunk_index` with [`v2/doc.get.metadata`](v2_doc_get_metadata.md) and [`v2/doc.get.content`](v2_doc_get_content.md) to expand the context window to adjacent chunks.
- **Parallel execution.** Both searches start simultaneously inside `spawn_blocking`. The handler returns only when both complete. If either search fails the entire call returns an error.
