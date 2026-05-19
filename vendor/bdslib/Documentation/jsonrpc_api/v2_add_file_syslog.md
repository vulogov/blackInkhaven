# v2/add.file.syslog

Submit an RFC 3164 syslog file for background ingestion. The server validates the file is accessible and non-empty, then enqueues its full path on the `"ingest_file_syslog"` channel. A background thread parses each line using the syslog parser, converts the structured fields into a telemetry document, and persists the records via `ShardsManager::add_batch`.

The call returns immediately after validation — the file is ingested asynchronously. Use `v2/count` or `v2/keys` to confirm records have arrived.

## Parameters

| Parameter | Type | Required | Description |
|---|---|---|---|
| `session` | string | yes | UUID v7 session identifier. Accepted and logged but not used for routing. |
| `path` | string | yes | Absolute path to the syslog file. The server checks that the path refers to a regular file, that the file is non-empty, and that it is readable before queuing it. |

## File format

Each line must be a valid RFC 3164 syslog message:

```
Jan 15 08:23:01 web-01 sshd[12345]: Failed password for root from 10.0.0.5 port 22 ssh2
Jan 15 08:23:15 db-02 kernel: Out of memory: Kill process 9876 (postgres) score 823 or sacrifice child
```

The parser extracts:

| Field | Source |
|---|---|
| `timestamp` | Parsed from the syslog date prefix; missing year defaults to the current year |
| `key` | Set to `"syslog"` |
| `data` | Object containing `host`, `process`, `pid` (when present), and `message` |

Lines that cannot be parsed are silently skipped by the background thread.

## Response

```json
{ "queued": "/var/log/syslog" }
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
    "method": "v2/add.file.syslog",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "path": "/var/log/syslog"
    },
    "id": 1
  }' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": { "queued": "/var/log/syslog" },
  "id": 1
}
```

## bdscmd

```bash
bdscmd add-file-syslog --path /var/log/syslog
```

## Error responses

| Code | Condition |
|---|---|
| `-32001` | `"ingest_file_syslog"` pipe not available (server startup error) or disconnected |
| `-32099` | `"ingest_file_syslog"` channel full — back off and retry. The channel is bounded by `ingest_channel_capacity` (default 100000); when at capacity the call returns this error instead of blocking. |
| `-32600` | File does not exist, is not a regular file, is empty, or cannot be opened for reading |

## Notes

- Ingestion is asynchronous. The method returns as soon as the path is enqueued; records may not be queryable immediately.
- The `"ingest_file_syslog"` channel is bounded by `ingest_channel_capacity` (default 100000). Set the config to `0` to revert to the legacy unbounded behaviour.
- For JSON telemetry files use [`v2/add.file`](v2_add_file.md) instead.
- For single records or low-volume ingestion use [`v2/add`](v2_add.md) or [`v2/add.batch`](v2_add_batch.md).
