# v2/primaries.get

Returns the `data` payload of every primary record whose `key` matches exactly within a lookback window, sorted by timestamp ascending.

Use this method to retrieve raw telemetry values for a specific key without fetching full documents or secondary records. For numeric keys this provides the time-ordered series suitable for direct charting or further processing. For non-numeric keys the `data` field is returned as-is (string, object, array, etc.).

## Parameters

| Parameter | Type | Required | Description |
|---|---|---|---|
| `session` | string | yes | UUID v7 session identifier. Reserved for future result caching; accepted and logged but not currently used. |
| `duration` | string | yes | Lookback window from now in humantime format, e.g. `"1h"`, `"30min"`, `"7days"`. Only shards whose time interval overlaps `[now − duration, now + 1s)` are queried. |
| `key` | string | yes | Exact key to match against the `key` field of primary records (e.g. `"server.cpu"`, `"queue.depth"`). No glob wildcards — use `v2/keys.get` for pattern matching. |

## Response

```json
{
  "results": [
    {
      "id": "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
      "timestamp": 1745042000,
      "data": 87.3
    },
    {
      "id": "018f1a3a-beef-7e5f-cafe-0c1d2e3f4a5b",
      "timestamp": 1745042060,
      "data": { "host": "web-01", "value": 91.2 }
    }
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `results` | array | Primary records ordered by `timestamp` ascending. Empty array when no records match. |
| `results[].id` | string | UUID v7 of the primary record. |
| `results[].timestamp` | integer | Event time as Unix seconds. |
| `results[].data` | any | The `data` payload stored with the record — may be a number, string, object, or array depending on how the record was ingested. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/primaries.get",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "duration": "1h",
      "key": "queue.depth"
    },
    "id": 1
  }' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": {
    "results": [
      {
        "id": "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
        "timestamp": 1745042000,
        "data": 149.71
      },
      {
        "id": "018f1a3a-beef-7e5f-cafe-0c1d2e3f4a5b",
        "timestamp": 1745042060,
        "data": 312.45
      }
    ]
  },
  "id": 1
}
```

## Error responses

| Code | Condition |
|---|---|
| `-32001` | `ShardsManager` singleton not initialised |
| `-32004` | Shard query failed |
| `-32600` | Invalid `duration` string |

## Notes

- Only `is_primary = 1` records are returned; secondary records are excluded.
- The `key` parameter is matched exactly. To query using shell-glob patterns, use [`v2/keys.get`](v2_keys_get.md).
- Results are sorted by `timestamp` ascending across all matching shards.
- The `session` parameter is stored for future caching integration and has no current effect on results.
- To retrieve the full document (including metadata fields and linked secondaries), use [`v2/primary`](v2_primary.md) with each returned `id`.
