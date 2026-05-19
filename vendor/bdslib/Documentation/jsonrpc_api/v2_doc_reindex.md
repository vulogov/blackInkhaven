# v2/doc.reindex

Rebuild the document store HNSW vector index from the persisted DuckDB metadata and blob stores. For every record in `metadata.db` the method re-embeds the JSON metadata fingerprint as `"{uuid}:meta"` and the blob content as `"{uuid}:content"`, then saves the index to disk.

Use this method to recover search after an unclean shutdown where DuckDB survived (auto-checkpoint) but the vecstore index was not flushed — the symptom is documents visible via `v2/doc.get` or `v2/doc.get.metadata` that do not appear in `v2/doc.search` results.

## Parameters

| Parameter | Type | Required | Description |
|---|---|---|---|
| `session` | string | yes | UUID v7 session identifier. Accepted and logged but not used for routing. |

## Response

```json
{ "indexed": 42 }
```

| Field | Type | Description |
|---|---|---|
| `indexed` | integer | Number of documents whose vector entries were (re-)written. Equals the total document count in the store. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/doc.reindex",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d"
    },
    "id": 1
  }' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": { "indexed": 42 },
  "id": 1
}
```

## bdscmd

```bash
bdscmd doc-reindex
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | `ShardsManager` singleton not initialised |
| `-32011` | Metadata scan, embedding, or vector store write failed |

## Notes

- Reindexing is a full rebuild: every existing vector entry for every document UUID is overwritten (upsert semantics). There is no incremental mode.
- The call blocks until all documents are embedded and the index is flushed to disk. For large document stores this may take several seconds.
- Under normal operation this method is not needed: `v2/doc.add` and `v2/doc.add.file` both embed and flush the index atomically before returning.
- Documents added via `v2/doc.update.metadata` or `v2/doc.update.content` do **not** automatically re-embed. Call `v2/doc.reindex` after bulk metadata or content updates to keep the vector index in sync.
