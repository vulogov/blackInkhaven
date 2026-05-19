# v2/status

Return a live snapshot of the bdsnode process: identity, uptime, wall-clock time, and the current depth of the two ingest queues.

The response is assembled synchronously from in-process state — no database or shard access is performed. The call always succeeds as long as the server is running.

## Parameters

This method accepts no parameters.  The `params` field may be omitted or set to an empty object.

## Response

```json
{
  "node_id":            "0196f3a2-1b4c-7e2d-9f0a-3c5b6d8e1f2a",
  "hostname":           "bds-prod-01.example.com",
  "uptime_secs":        3724,
  "timestamp":          1745003724,
  "logs_queue":         14,
  "json_file_queue":    2,
  "json_file_name":     "/var/log/ingest/2026-04-25T12:00:00.ndjson",
  "syslog_file_queue":  0,
  "syslog_file_name":   null,
  "jsoncache_pct":      72,
  "jsoncache_len":      7234,
  "jsoncache_capacity": 10000,
  "embedding_model":    "AllMiniLML6V2",
  "n_results":          3,
  "n_bunds":            2,
  "recent_scripts": [
    { "id": "0192a3b4-c5d6-7e8f-9012-34567890abcd", "submitted_at": 1745003720 },
    { "id": "0192a3b4-c5d6-7e8f-9012-34567890abce", "submitted_at": 1745003719 }
  ],
  "running_scripts": [
    { "worker": 0, "id": "0192a3b4-c5d6-7e8f-9012-34567890abcd" },
    { "worker": 2, "id": "0192a3b4-c5d6-7e8f-9012-34567890abce" }
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `node_id` | string | Stable node identifier. Set at startup from the `--nodeid` CLI flag, or auto-generated as a UUID v7 when the flag is omitted. |
| `hostname` | string | Hostname of the machine running bdsnode. Resolved at startup from `$HOSTNAME`, then `/etc/hostname`, then the `hostname` command. `"unknown"` if none of those sources are available. |
| `uptime_secs` | integer | Seconds elapsed since bdsnode started. |
| `timestamp` | integer | Current wall-clock time as Unix seconds (UTC). |
| `logs_queue` | integer | Number of JSON telemetry documents currently queued in the `"ingest"` pipe, waiting to be flushed to the shard store by the `bds-add` background thread. |
| `json_file_queue` | integer | Number of file paths currently queued in the `"ingest_file"` pipe, waiting to be processed by the `bds-add-file` background thread. |
| `json_file_name` | string \| null | Absolute path of the file currently being ingested by the `bds-add-file` thread. `null` when no file is being processed (idle, or file ingest is disabled in config). |
| `syslog_file_queue` | integer | Number of syslog file paths currently queued for the `bds-add-file-syslog` background thread. |
| `syslog_file_name` | string \| null | Absolute path of the syslog file currently being ingested. `null` when idle. |
| `jsoncache_pct` | integer | In-memory primary-record JSON cache utilisation as an integer percentage `[0, 100]`. |
| `jsoncache_len` | integer | Number of entries currently held in the JSON cache (including stale entries not yet swept). |
| `jsoncache_capacity` | integer | Maximum number of entries the JSON cache can hold before LRU eviction. |
| `embedding_model` | string \| null | Name of the loaded fastembed `EmbeddingModel` variant (Rust Debug form, e.g. `"AllMiniLML6V2"`). Configured via `embedding_model` in `bds.hjson` and resolved at startup; `null` only in degenerate cases (manager built via `with_embedding`, no global DB initialised, or the field is missing on older bdsnode versions). |
| `n_results` | integer | Number of distinct queue ids tracked in the global `RESULTS` queue (`bdslib::vm::RESULTS`). Each `v2/eval.queued` submission and `v2/results.push` call creates or appends to one queue. |
| `n_bunds` | integer | Number of named BUND VM contexts currently held in `bdslib::vm::context::BUNDS`. Increases on each new `v2/eval` context name; entries are evicted after `bund_ttl_secs` of inactivity. |
| `recent_scripts` | array | Most-recent-first list of the last 5 jobs accepted by the `BundWorkerPool` (`v2/eval.queued`, the scheduler, and any direct `submit_script_with_id` callers). Each entry is `{ "id": "<uuidv7>", "submitted_at": <unix_secs> }`. |
| `running_scripts` | array | One entry per worker thread that is currently executing a script, sorted by `worker` index. Each entry is `{ "worker": <usize>, "id": "<uuidv7>" }`. Idle workers do not appear. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"v2/status","params":{},"id":1}' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": {
    "node_id":         "0196f3a2-1b4c-7e2d-9f0a-3c5b6d8e1f2a",
    "hostname":        "bds-prod-01.example.com",
    "uptime_secs":     3724,
    "timestamp":       1745003724,
    "logs_queue":      14,
    "json_file_queue": 2,
    "json_file_name":  "/var/log/ingest/2026-04-25T12:00:00.ndjson"
  },
  "id": 1
}
```

### Idle node (no ingest activity, no BUND jobs)

```json
{
  "jsonrpc": "2.0",
  "result": {
    "node_id":           "0196f3a2-1b4c-7e2d-9f0a-3c5b6d8e1f2a",
    "hostname":          "bds-prod-01.example.com",
    "uptime_secs":       86400,
    "timestamp":         1745086400,
    "logs_queue":        0,
    "json_file_queue":   0,
    "json_file_name":    null,
    "syslog_file_queue": 0,
    "syslog_file_name":  null,
    "jsoncache_pct":     0,
    "jsoncache_len":     0,
    "jsoncache_capacity": 10000,
    "embedding_model":    "AllMiniLML6V2",
    "n_results":         0,
    "n_bunds":           0,
    "recent_scripts":    [],
    "running_scripts":   []
  },
  "id": 1
}
```

## Error responses

This method does not produce application-level errors.  The only failure mode is an internal server panic (`-32000`), which indicates the server is in an unrecoverable state.

## Notes

- **Node identity.** The `node_id` is fixed for the lifetime of the process.  Use `--nodeid <value>` to assign a stable, human-readable name (e.g. `bds-primary`, `region-eu-west-1`) for use in dashboards or alerting rules.  When the flag is omitted, a fresh UUID v7 is generated each time bdsnode starts.
- **Queue depths.** `logs_queue` and `json_file_queue` reflect messages that have been accepted by the RPC layer but not yet written to storage.  A non-zero queue is normal under load.  A persistently growing queue indicates the ingest thread cannot keep up with the ingestion rate.
- **File ingest disabled.** When the `file_batch_size` / `file_timeout_ms` config keys are absent, the `bds-add-file` thread is not started and `json_file_queue` will always be `0` while `json_file_name` will always be `null`.
- **Polling.** `v2/status` is lightweight and safe to poll at high frequency (e.g. every second) for monitoring dashboards.  It never touches the database.
- **Timestamp vs uptime.** `timestamp` is an absolute Unix epoch value useful for correlating with external systems.  `uptime_secs` is useful for tracking restarts — if `uptime_secs` resets unexpectedly while `node_id` stays the same (fixed `--nodeid`), the process was restarted.
- **BUND runtime fields.** `n_results`, `n_bunds`, `recent_scripts`, and `running_scripts` reflect in-process state of the BUND VM subsystem. They reset to zero on every restart (no persistence). `recent_scripts` is a bounded ring buffer of the last 5 submissions; older entries are evicted FIFO. `running_scripts` shows only workers actively executing a job — idle workers are omitted, so the array length never exceeds `n_workers` (configured via `n_workers` in `bds.hjson`, default 4).
