# v2/tpl.templates_recent

Return all drain3 log-template documents whose FrequencyTracking observation timestamp falls within the lookback window `[now − duration, now]`. Queries all shards and deduplicates results by UUID. Equivalent to `v2/tpl.templates_by_timestamp` but accepts a humantime duration string instead of absolute Unix timestamps.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Accepted and logged. |
| `duration` | string | no | `"1h"` | Lookback window from now in humantime format, e.g. `"1h"`, `"30min"`, `"7days"`. |

## Response

```json
{
  "templates": [
    {
      "id": "019682ab-1234-7000-8000-000000000001",
      "metadata": {
        "name": "user <*> logged in from <*>",
        "timestamp": 1745001234,
        "type": "tpl"
      },
      "body": "user <*> logged in from <*>"
    }
  ]
}
```

An empty array is returned when no template observations fall within the window:

```json
{ "templates": [] }
```

### `templates[]` fields

| Field | Type | Description |
|---|---|---|
| `id` | string | UUID v7 of the template document. |
| `metadata` | object | Template metadata as stored in tplstorage. Typically includes `name`, `timestamp`, and `type`. |
| `body` | string | The drain3 template pattern string, e.g. `"user <*> logged in from <*>"`. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/tpl.templates_recent",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "duration": "2h"
    },
    "id": 1
  }' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": {
    "templates": [
      {
        "id": "019682ab-1234-7000-8000-000000000001",
        "metadata": { "name": "user <*> logged in from <*>", "timestamp": 1745001234, "type": "tpl" },
        "body": "user <*> logged in from <*>"
      },
      {
        "id": "019682ab-5678-7000-8000-000000000002",
        "metadata": { "name": "disk <*> usage <*>% warning threshold reached", "timestamp": 1745002100, "type": "tpl" },
        "body": "disk <*> usage <*>% warning threshold reached"
      }
    ]
  },
  "id": 1
}
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32011` | Template query failed (shard error) |

## Notes

- The default `duration` of `"1h"` means templates whose triggering log entry was processed within the last hour.
- Results are deduplicated by UUID across all shards. Each UUID appears at most once in the response.
- Use the returned `body` strings as input to `v2/rca.templates`'s `failure_body` parameter to investigate which templates consistently precede a given failure pattern.
- For exact Unix-second ranges use `v2/tpl.templates_by_timestamp` instead.
