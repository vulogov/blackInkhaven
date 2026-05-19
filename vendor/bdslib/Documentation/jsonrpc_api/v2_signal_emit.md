# v2/signal.emit

Emit a new signal — a named event with severity, timestamp, and arbitrary metadata — into the per-shard signal store.

Signals are routed to the shard whose interval contains `timestamp`. Once stored, signals are searchable via `v2/signals` (recent listing) and `v2/signals_query` (semantic search).

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUIDv7 session identifier. Accepted and logged; reserved for future caching. |
| `name` | string | yes | — | Short, machine-readable signal name (e.g. `"oom_killer.fired"`, `"deploy.started"`). |
| `severity` | string | yes | — | Free-form severity level. Common values: `"info"`, `"warn"`, `"error"`, `"critical"`. |
| `timestamp` | integer | yes | — | Unix seconds. Determines which time shard the signal lands in. |
| `metadata` | object | no | `{}` | Arbitrary extra fields to merge into the stored metadata. The reserved keys `name`, `severity`, and `timestamp` always take precedence over anything in this map. |

## Response

```json
{ "id": "0192a3b4-c5d6-7e8f-9012-34567890abcd" }
```

| Field | Type | Description |
|---|---|---|
| `id` | string | UUIDv7 of the stored signal. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/signal.emit",
    "params": {
      "session":   "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "name":      "deploy.completed",
      "severity":  "info",
      "timestamp": 1745603600,
      "metadata":  { "service": "auth", "version": "2.4.1", "operator": "ci" }
    },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32011` | Signal store write failed |

## Notes

- **Reserved keys.** Anything you put in `metadata` under `name`, `severity`, or `timestamp` is silently overwritten by the top-level fields.
- **Time routing.** A `timestamp` far outside any existing shard's interval triggers a new shard to be auto-created; this is a normal occurrence on first ingest of historical data.
