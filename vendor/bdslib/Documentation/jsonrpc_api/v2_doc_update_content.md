# v2/doc.update.content

Replace the content text for a document in-place. The metadata and HNSW vector index entries are not modified.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Logged but not used for routing. |
| `id` | string | yes | — | UUIDv7 of the document to update. |
| `content` | string | yes | — | Replacement content text. Overwrites the existing blob. |

## Response

```json
{ "updated": true }
```

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/doc.update.content",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
      "content": "Updated procedure: Step 1 — verify circuit breaker state via dashboard."
    },
    "id": 1
  }' | jq
```

## bdscmd

```bash
bdscmd doc-update-content \
  --id 018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b \
  --content "Updated procedure: Step 1 — verify circuit breaker state via dashboard."
```

## Error responses

| Code | Condition |
|---|---|
| `-32600` | `id` is not a valid UUID string |
| `-32001` | `ShardsManager` singleton not initialised |
| `-32011` | Document store write failed |

## Notes

- The HNSW `":content"` vector slot is **not** updated automatically. The existing embedding continues to serve search results until re-indexed.
- To keep the vector index in sync after updating content, re-embed the new text and call the Rust `doc_store_content_vector` method (not yet exposed as a JSON-RPC endpoint).
