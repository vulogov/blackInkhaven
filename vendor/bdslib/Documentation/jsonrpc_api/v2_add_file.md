# v2/add.file

Submit a file for background ingestion. The server validates the file is accessible and non-empty, then enqueues its full path on the `"ingest_file"` channel. A background thread reads the file line-by-line, parses each line as a JSON telemetry document, and persists the records via `ShardsManager::add_batch`.

The call returns immediately after validation — the file is ingested asynchronously. Use `v2/count` or `v2/keys` to confirm records have arrived.

## Parameters

| Parameter | Type | Required | Description |
|---|---|---|---|
| `session` | string | yes | UUID v7 session identifier. Reserved for future result caching; accepted and logged but not currently used. |
| `path` | string | yes | Absolute path to the file to ingest. The server checks that the path refers to a regular file, that the file is non-empty, and that it is readable before queuing it. |

## File format

Each non-empty line of the file must be a JSON object that is a valid telemetry document:

```json
{"timestamp": 1745042000, "key": "server.cpu", "data": 87.3}
{"timestamp": 1745042060, "key": "server.cpu", "data": {"host": "web-01", "value": 91.2}}
```

| Field | Required | Description |
|---|---|---|
| `timestamp` | yes | Unix seconds (non-negative integer). |
| `key` | yes | Non-empty string identifying the metric or log type. |
| `data` | yes | Any non-null JSON value. |

Lines that fail JSON parsing or fail validation are silently skipped by the background thread.

## Response

```json
{ "queued": "/var/log/telemetry/batch_001.jsonl" }
```

| Field | Type | Description |
|---|---|---|
| `queued` | string | The path that was accepted and enqueued for ingestion. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/add.file",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "path": "/var/log/telemetry/batch_001.jsonl"
    },
    "id": 1
  }' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": { "queued": "/var/log/telemetry/batch_001.jsonl" },
  "id": 1
}
```

## Error responses

| Code | Condition |
|---|---|
| `-32001` | `"ingest_file"` pipe not available (server startup error) or disconnected |
| `-32099` | `"ingest_file"` channel full — back off and retry. The channel is bounded by `ingest_channel_capacity` (default 100000); when at capacity the call returns this error instead of blocking. |
| `-32600` | File does not exist, is not a regular file, is empty, or cannot be opened for reading |

## Notes

- Ingestion is asynchronous. The method returns as soon as the path is enqueued; records may not be queryable immediately.
- The background thread processes files in the order they are received. Batch size and flush timeout are controlled by `file_batch_size` and `file_timeout_ms` in the hjson config (defaults: 100 records, 5000 ms).
- The `"ingest_file"` channel is bounded by `ingest_channel_capacity` (default 100000). Set the config to `0` to revert to the legacy unbounded behaviour.
- For single-record or low-volume ingestion use [`v2/add`](v2_add.md) or [`v2/add.batch`](v2_add_batch.md) instead.
