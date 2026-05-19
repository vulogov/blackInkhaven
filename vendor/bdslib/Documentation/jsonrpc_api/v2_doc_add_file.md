# v2/doc.add.file

Load a text file from the server filesystem, split it into overlapping chunks on paragraph/sentence/word boundaries, and store every chunk as an independently searchable record in the document store. Each chunk is embedded automatically with the shared `AllMiniLML6V2` model.

Returns the UUIDv7 of the document-level metadata record. Each chunk gets its own UUID in the blob store, the JSON metadata store, and the HNSW vector index. The document-level record holds the ordered chunk UUID list under `"chunks"` for context-window expansion during RAG retrieval.

For short documents that fit in a single record, use [`v2/doc.add`](v2_doc_add.md) instead.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Logged but not used for routing. |
| `path` | string | yes | — | Absolute or relative filesystem path to the text file (must be readable by the server process). |
| `name` | string | yes | — | Human-readable document name stored in all metadata records. |
| `slice` | integer | no | `512` | Maximum character count per chunk. Clamped to `≥ 1`. |
| `overlap` | float | no | `20.0` | Overlap as a percentage of `slice`, in `[0.0, 99.0]`. |

## Response

```json
{
  "id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
  "n_chunks": 14
}
```

| Field | Type | Description |
|---|---|---|
| `id` | string | UUIDv7 of the document-level metadata record. |
| `n_chunks` | integer | Number of chunks created. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/doc.add.file",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "path": "/data/runbooks/payment_incident.txt",
      "name": "Payment Service Incident Runbook",
      "slice": 220,
      "overlap": 20.0
    },
    "id": 1
  }' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": {
    "id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
    "n_chunks": 20
  },
  "id": 1
}
```

## bdscmd

```bash
bdscmd doc-add-file \
  --path /data/runbooks/payment_incident.txt \
  --name "Payment Service Incident Runbook" \
  --slice 220 \
  --overlap 20
```

## Error responses

| Code | Condition |
|---|---|
| `-32600` | File does not exist, is not a regular file, or cannot be opened |
| `-32001` | `ShardsManager` singleton not initialised |
| `-32011` | Document store write failed |

## Two-level storage layout

```
Document-level record  (id = <doc-uuid>)
  metadata: { name, path, slice, overlap, n_chunks, chunks: [<uuid-0>, <uuid-1>, …] }
  content:  (empty)

Chunk records  (id = <chunk-uuid-N>)
  metadata: { document_name, document_id: <doc-uuid>, chunk_index: N, n_chunks }
  content:  <chunk text bytes>
```

## RAG context-window expansion

After finding a chunk via [`v2/doc.search`](v2_doc_search.md), expand to adjacent chunks using the ordered `chunks` list in the document-level metadata:

1. Extract `document_id` and `chunk_index` from the chunk's metadata.
2. Call [`v2/doc.get.metadata`](v2_doc_get_metadata.md) with `document_id` to get the `chunks` list.
3. Fetch `chunks[chunk_index - 1]`, `chunks[chunk_index]`, `chunks[chunk_index + 1]` via [`v2/doc.get.content`](v2_doc_get_content.md).
4. Concatenate the fetched content as a ±1 context window.
