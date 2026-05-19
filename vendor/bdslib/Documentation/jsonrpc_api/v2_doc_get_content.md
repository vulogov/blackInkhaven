# v2/doc.get.content

Retrieve the raw content text for a document or chunk by UUID, without fetching the metadata.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Logged but not used for routing. |
| `id` | string | yes | — | UUIDv7 of the document or chunk. |

## Response

```json
{
  "id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
  "content": "Step 1: check the circuit breaker status…"
}
```

| Field | Type | Description |
|---|---|---|
| `id` | string | The requested UUID. |
| `content` | string | UTF-8 decoded content text. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/doc.get.content",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b"
    },
    "id": 1
  }' | jq
```

## bdscmd

```bash
bdscmd doc-get-content --id 018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b
```

## Error responses

| Code | Condition |
|---|---|
| `-32600` | `id` is not a valid UUID string |
| `-32010` | No content blob found for the given UUID |
| `-32001` | `ShardsManager` singleton not initialised |
| `-32011` | Document store read failed |

## Notes

- For document-level records created by `v2/doc.add.file`, the content blob is empty; fetch individual chunk UUIDs from the `chunks` list in the document metadata.
- This is the preferred method for fetching chunks during context-window expansion in RAG pipelines — one call per adjacent chunk index.
