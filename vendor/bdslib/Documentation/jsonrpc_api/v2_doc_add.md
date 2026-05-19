# v2/doc.add

Store a single document in the embedded document store with JSON metadata and plain-text content. Both the metadata fingerprint and the content are embedded automatically using the shared `AllMiniLML6V2` model and indexed in the HNSW vector store under `"{uuid}:meta"` and `"{uuid}:content"` slots.

Use this method for short documents that fit in a single record. For large text files that should be split into overlapping chunks, use [`v2/doc.add.file`](v2_doc_add_file.md) instead.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Logged but not used for routing. |
| `metadata` | object | yes | — | Arbitrary JSON object stored as document metadata. |
| `content` | string | yes | — | Document content text. Stored as raw bytes in the blob store. |

## Response

```json
{ "id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b" }
```

| Field | Type | Description |
|---|---|---|
| `id` | string | UUIDv7 assigned to the stored document. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/doc.add",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "metadata": {"name": "Circuit Breaker Runbook", "category": "runbook", "service": "payment"},
      "content": "Step 1: check the circuit breaker status. Step 2: verify downstream services."
    },
    "id": 1
  }' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": { "id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b" },
  "id": 1
}
```

## bdscmd

```bash
bdscmd doc-add \
  --metadata '{"name":"Circuit Breaker Runbook","category":"runbook","service":"payment"}' \
  --content "Step 1: check the circuit breaker status."
```

## Error responses

| Code | Condition |
|---|---|
| `-32001` | `ShardsManager` singleton not initialised |
| `-32011` | Document store write failed |

## Notes

- The returned `id` can be used with [`v2/doc.get`](v2_doc_get.md), [`v2/doc.update.metadata`](v2_doc_update_metadata.md), [`v2/doc.update.content`](v2_doc_update_content.md), and [`v2/doc.delete`](v2_doc_delete.md).
- Both HNSW slots are written atomically before returning. The document is immediately searchable via [`v2/doc.search`](v2_doc_search.md) and [`v2/doc.search.json`](v2_doc_search_json.md).
