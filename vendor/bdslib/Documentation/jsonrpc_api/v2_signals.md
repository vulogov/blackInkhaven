# v2/signals

List signals observed within a humantime lookback window, with full metadata resolved per signal.

The handler queries the signal-store FrequencyTracking layer for IDs whose observation timestamp falls in `[now − duration, now]`, then resolves each ID to its current metadata.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUIDv7 session identifier. Accepted and logged; reserved for future caching. |
| `duration` | string | no | `"1h"` | Lookback window in humantime format (e.g. `"30min"`, `"6h"`, `"7days"`). |

## Response

```json
{
  "duration": "1h",
  "count":    3,
  "signals":  [
    {
      "id": "0192a3b4-c5d6-7e8f-9012-34567890abcd",
      "metadata": {
        "name":      "deploy.completed",
        "severity":  "info",
        "timestamp": 1745603600,
        "service":   "auth"
      }
    },
    {
      "id": "0192a3b4-c5d6-7e8f-9012-34567890abce",
      "metadata": null
    }
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `duration` | string | Echoes the request. |
| `count` | integer | Number of returned signals. |
| `signals[]` | array | One entry per signal observation in the window. |
| `signals[].id` | string | UUIDv7 of the signal. |
| `signals[].metadata` | object \| null | Current metadata, or `null` if the underlying record was deleted between the FrequencyTracking lookup and the metadata resolve (rare). |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/signals",
    "params": {
      "session":  "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "duration": "6h"
    },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32011` | Signal store query failed |

## Notes

- **Order is not stable.** The signals array order reflects the FrequencyTracking iteration order; sort client-side by `metadata.timestamp` if you need chronology.
- **Duplicate observations.** The same signal observed multiple times within the window appears once in the result.
