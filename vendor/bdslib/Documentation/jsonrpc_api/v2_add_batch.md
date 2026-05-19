# v2/add.batch

Enqueues a list of JSON telemetry documents into the `"ingest"` crossbeam channel for asynchronous persistence by the batch-ingestion thread.

The whole list is pushed via `bdslib::pipe::send_many`, which takes the channel's internal mutex once per item rather than once per call site — so the tokio worker is freed up sooner and other RPCs aren't blocked while a large batch is being enqueued. The consumer thread can interleave these documents with records from concurrent `v2/add` callers when forming the next storage batch.

The call returns as soon as all documents are accepted by the channel.

## Parameters

| Parameter | Type | Required | Description |
|---|---|---|---|
| `docs` | array of objects | yes | The JSON telemetry documents to ingest. Each must contain `"timestamp"` (Unix seconds), `"key"` (string), and `"data"` (any JSON value). An optional `"id"` (UUID v7) may be supplied; one is generated if absent. All other fields are stored as metadata. |

## Response

```json
{ "queued": 42 }
```

| Field | Type | Description |
|---|---|---|
| `queued` | integer | Number of documents accepted by the channel (equals `len(docs)`). |

Returns `{ "queued": 0 }` for an empty `docs` array.

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/add.batch",
    "params": {
      "docs": [
        {"timestamp": 1745042000, "key": "server.cpu",    "host": "web-01", "value": 87.3},
        {"timestamp": 1745042001, "key": "server.memory", "host": "web-01", "value": 4096},
        {"timestamp": 1745042002, "key": "server.cpu",    "host": "web-02", "value": 23.1}
      ]
    },
    "id": 1
  }' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": { "queued": 3 },
  "id": 1
}
```

## Error responses

| Code | Condition |
|---|---|
| `-32001` | The `"ingest"` channel is disconnected (pipe registry not initialized or consumer dropped). Sending stops on the first failure; already-queued documents in the same request are not rolled back. |
| `-32099` | Ingest channel full — back off and retry. The channel is bounded by `ingest_channel_capacity` (default 100000); when at capacity the call returns this error instead of OOMing the server. Sending stops on the first failure; documents successfully pushed earlier in the same request are retained. |

## Notes

- The `"ingest"` channel is bounded by `ingest_channel_capacity` (default 100000). Set the config to `0` for the legacy unbounded behaviour. Bounded channels apply backpressure cleanly: at capacity the RPC returns `-32099` (retry-able) instead of letting an unbounded queue grow until the process OOMs.
- Documents are forwarded to the batch thread; the thread combines them with records from concurrent `v2/add` calls into the same DuckDB transaction (subject to `pipe_batch_size`, default 500, and `pipe_timeout_ms`, default 500).
- Persistence is asynchronous — a successful response means all documents are queued, not yet written to disk. The background sync task (default cadence 60 s — see `sync_interval_secs`) bounds how long writes can sit in the WAL before checkpoint.
- For single-document ingestion use [`v2/add`](v2_add.md). For sync ingest with a UUID round-trip in the same call, use `v2/add` with `"sync": true`.
