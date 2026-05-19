# v2/add

Enqueues a single JSON telemetry document for persistence.  Two modes:

- **Async (default)** — pushes the document onto the `"ingest"` channel and
  returns immediately with `{ "queued": 1 }`.  Persistence happens in
  the background batch-ingestion thread; the document is committed when
  either the batch is full (`pipe_batch_size`) or the idle timeout
  (`pipe_timeout_ms`) elapses.
- **Sync (opt-in)** — when `"sync": true` is set, bypasses the queue and
  calls `ShardsManager::add` directly.  The response carries the assigned
  UUIDv7.  Useful for callers that need the id in the same response (e.g.
  CLI scripts that pipe the id into the next command).

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `doc` | object | yes | — | The JSON telemetry document to ingest. Must contain `"timestamp"` (Unix seconds), `"key"` (string), and `"data"` (any JSON value). An optional `"id"` field (UUIDv7 string) may be supplied; one is generated if absent. All other fields are stored as metadata. |
| `sync` | bool | no | `false` | When `true`, run synchronously and return the assigned UUID. |

## Response

**Async mode** (default):

```json
{ "queued": 1 }
```

**Sync mode** (`sync: true`):

```json
{
  "id":     "0192a3b4-c5d6-7e8f-9012-34567890abcd",
  "synced": true
}
```

| Field | Type | Description |
|---|---|---|
| `queued` | integer | Async only — always `1`, confirms channel acceptance. |
| `id` | string | Sync only — the UUIDv7 of the stored record. |
| `synced` | bool | Sync only — always `true`, distinguishes the response shape. |

## Examples

**Default async ingest:**

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/add",
    "params": {
      "doc": {
        "timestamp": 1745042000,
        "key": "server.cpu",
        "host": "web-01",
        "data": { "value": 87.3 }
      }
    },
    "id": 1
  }' | jq
```

**Sync ingest with id round-trip:**

```bash
ID=$(curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/add",
    "params": {
      "sync": true,
      "doc": {
        "timestamp": 1745042000,
        "key": "manual.audit",
        "data": { "value": "deploy v1.42 started" }
      }
    },
    "id": 1
  }' | jq -r '.result.id')

# Use the id in a follow-up call:
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d "{
    \"jsonrpc\": \"2.0\",
    \"method\": \"v2/primary\",
    \"params\": { \"session\": \"-\", \"id\": \"$ID\" },
    \"id\": 2
  }" | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic (sync mode only). |
| `-32001` | Database unavailable. |
| `-32004` | Sync mode only — `ShardsManager::add` failed (validation, dedup, or storage error). |
| `-32099` | Async mode only — ingest channel is full; back off and retry. |

## Notes

- **Async mode** is fire-and-forget: a successful response means the
  document is queued, not yet written to disk.  The ingest channel is
  bounded by `ingest_channel_capacity` (default 100000) — when full the
  call returns `-32099` instead of OOMing the server.
- **Sync mode** is durable on return: the record is in DuckDB
  `telemetry`, in the FTS index (if classified as primary), and in the
  vector index.  It costs more per call (one full pipeline pass instead
  of a single channel push), so prefer async for high-volume ingest.
- **Performance note.** Sync mode does NOT batch — each call is a
  separate Tantivy commit and a separate vector-index lock acquisition.
  For bulk ingest, use [`v2/add.batch`](v2_add_batch.md) instead.
- Use [`v2/add.batch`](v2_add_batch.md) to enqueue multiple documents
  in a single request.
