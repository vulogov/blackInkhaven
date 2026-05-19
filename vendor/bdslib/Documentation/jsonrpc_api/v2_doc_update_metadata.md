# v2/doc.update.metadata

Replace the JSON metadata for a document in-place. The blob content and HNSW vector index entries are not modified.

If the new metadata should be re-embedded and reflected in future vector searches, re-index manually with the Rust `doc_store_metadata_vector` method (not yet exposed as a JSON-RPC endpoint).

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Logged but not used for routing. |
| `id` | string | yes | — | UUIDv7 of the document to update. |
| `metadata` | object | yes | — | Replacement JSON metadata. Overwrites the entire existing metadata record. |

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
    "method": "v2/doc.update.metadata",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
      "metadata": {"name": "Circuit Breaker Runbook", "revision": 2, "reviewed_by": "ops-team"}
    },
    "id": 1
  }' | jq
```

## bdscmd

```bash
bdscmd doc-update-metadata \
  --id 018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b \
  --metadata '{"name":"Circuit Breaker Runbook","revision":2}'
```

## Error responses

| Code | Condition |
|---|---|
| `-32600` | `id` is not a valid UUID string |
| `-32001` | `ShardsManager` singleton not initialised |
| `-32011` | Document store write failed |

## Notes

- The HNSW `":meta"` vector slot is **not** updated automatically. The existing embedding continues to serve search results until re-indexed.
- The full metadata object is replaced, not merged — include all fields you want to retain.
