# v2/doc.delete

Remove a document from all three sub-stores: the JSON metadata store, the blob store, and the HNSW vector index. Both `"{id}:meta"` and `"{id}:content"` vector slots are deleted.

The operation is idempotent — calling it for an unknown UUID returns `{"deleted": true}` (no-op in each sub-store).

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Logged but not used for routing. |
| `id` | string | yes | — | UUIDv7 of the document to remove. |

## Response

```json
{ "deleted": true }
```

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/doc.delete",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b"
    },
    "id": 1
  }' | jq
```

## bdscmd

```bash
bdscmd doc-delete --id 018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b
```

## Error responses

| Code | Condition |
|---|---|
| `-32600` | `id` is not a valid UUID string |
| `-32001` | `ShardsManager` singleton not initialised |
| `-32011` | Document store delete failed |

## Notes

- Deleting a document-level record created by `v2/doc.add.file` does **not** automatically delete its chunk records. Delete each chunk UUID individually using its UUID from the `chunks` list in the document metadata.
- After deletion, the UUID is no longer returned by `v2/doc.get.metadata`, `v2/doc.get.content`, or any search method.
