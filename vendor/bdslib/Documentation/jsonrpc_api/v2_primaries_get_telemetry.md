# v2/primaries.get.telemetry

Returns the extracted numeric measurement for every primary record whose `key` matches exactly within a lookback window, sorted by timestamp ascending. Records that carry no numeric value are silently skipped.

This is the numeric-only variant of [`v2/primaries.get`](v2_primaries_get.md). Instead of returning the raw `data` payload it extracts a single `f64` value per record, making the output directly usable for charting or statistical processing without additional client-side parsing.

## Numeric extraction

For each primary record the server attempts:

1. `data` itself is a JSON number → use it directly.
2. `data` is a JSON object and `data["value"]` is a number → use `data["value"]`.

Records where neither condition holds are excluded from the result.

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
      "value": 149.71
    },
    {
      "id": "018f1a3a-beef-7e5f-cafe-0c1d2e3f4a5b",
      "timestamp": 1745042060,
      "value": 312.45
    }
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `results` | array | Numeric primary records ordered by `timestamp` ascending. Empty array when no matching records carry a numeric value. |
| `results[].id` | string | UUID v7 of the primary record. |
| `results[].timestamp` | integer | Event time as Unix seconds. |
| `results[].value` | number | Extracted numeric measurement (`f64`). |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/primaries.get.telemetry",
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
        "value": 149.71
      },
      {
        "id": "018f1a3a-beef-7e5f-cafe-0c1d2e3f4a5b",
        "timestamp": 1745042060,
        "value": 312.45
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
- The `key` parameter is an exact match. For shell-glob pattern queries use [`v2/keys.get`](v2_keys_get.md).
- Records whose `data` is not a number and does not contain a numeric `data["value"]` field are silently omitted. The result may therefore contain fewer entries than [`v2/primaries.get`](v2_primaries_get.md) for the same parameters.
- To discover which keys have numeric data suitable for this method, use [`v2/primaries.explore.telemetry`](v2_primaries_explore_telemetry.md).
- The `session` parameter is stored for future caching integration and has no current effect on results.
