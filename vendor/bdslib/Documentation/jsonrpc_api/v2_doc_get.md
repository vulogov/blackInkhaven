# v2/doc.get

Retrieve both the JSON metadata and the content text for a document by UUID. Equivalent to calling [`v2/doc.get.metadata`](v2_doc_get_metadata.md) and [`v2/doc.get.content`](v2_doc_get_content.md) in a single round trip.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Logged but not used for routing. |
| `id` | string | yes | — | UUIDv7 of the document or chunk to retrieve. |

## Response

```json
{
  "id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
  "metadata": { "name": "Circuit Breaker Runbook", "category": "runbook" },
  "content": "Step 1: check the circuit breaker status…"
}
```

| Field | Type | Description |
|---|---|---|
| `id` | string | The requested UUID. |
| `metadata` | object | JSON metadata stored for this document. |
| `content` | string | UTF-8 decoded content text. Empty string if no blob was stored. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/doc.get",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b"
    },
    "id": 1
  }' | jq
```

## bdscmd

```bash
bdscmd doc-get --id 018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b
```

## Error responses

| Code | Condition |
|---|---|
| `-32600` | `id` is not a valid UUID string |
| `-32010` | No document with the given UUID exists |
| `-32001` | `ShardsManager` singleton not initialised |
| `-32011` | Document store read failed |
