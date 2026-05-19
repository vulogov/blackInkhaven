# v2/primaries.explore.telemetry

Returns keys that have more than one primary record carrying a **numeric** `data` value within the specified duration window, together with each key's record count and UUIDs.

This is a telemetry-focused variant of [`v2/primaries.explore`](v2_primaries_explore.md). It applies the same grouping and count-≥-2 filter, but first discards any primary record whose `data` is not a number. Only keys with at least two numeric primaries appear in the result. This makes it the right starting point for [`v2/trends`](v2_trends.md) analysis — it surfaces exactly the keys that the trend engine can compute statistics for.

## Numeric data definition

A primary record is considered numeric when either of these conditions is true:

- `data` is a bare JSON number (e.g. `87.3`, `42`).
- `data` is a JSON object and `data.value` is a JSON number (e.g. `{"host": "web-01", "value": 87.3}`).

All other `data` shapes (strings, arrays, objects without a `.value` number, booleans, null) are excluded.

## Parameters

| Parameter | Type | Required | Description |
|---|---|---|---|
| `session` | string | yes | UUID v7 session identifier. Accepted and logged; reserved for future result caching. |
| `duration` | string | yes | Lookback window from now in humantime format, e.g. `"1h"`, `"30min"`, `"7days"`. Only shards whose time interval overlaps `[now − duration, now + 1s)` are queried. |

## Response

A JSON array, one entry per qualifying key, sorted alphabetically by `key`. Returns an empty array `[]` when no key has more than one numeric primary record in the window.

```json
[
  {
    "key": "server.cpu",
    "count": 360,
    "primary_id": [
      "018f1a3a-0000-7e5f-8a9b-0c1d2e3f0001",
      "018f1a3a-0001-7e5f-8a9b-0c1d2e3f0002"
    ]
  },
  {
    "key": "server.memory",
    "count": 360,
    "primary_id": [
      "018f1a4b-0000-7e5f-8a9b-0c1d2e3f0003",
      "018f1a4b-0001-7e5f-8a9b-0c1d2e3f0004"
    ]
  }
]
```

| Field | Type | Description |
|---|---|---|
| `[].key` | string | Telemetry key (signal identifier / metric name). |
| `[].count` | integer | Number of primary records with numeric data for this key within the duration window. Always ≥ 2. |
| `[].primary_id` | array of strings | UUID v7 strings of all qualifying primary records for this key. Order reflects insertion order across shards. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/primaries.explore.telemetry",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "duration": "1h"
    },
    "id": 1
  }' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": [
    {
      "key": "server.cpu",
      "count": 360,
      "primary_id": [
        "018f1a3a-0000-7e5f-8a9b-0c1d2e3f0001",
        "018f1a3a-0001-7e5f-8a9b-0c1d2e3f0002"
      ]
    },
    {
      "key": "server.memory",
      "count": 360,
      "primary_id": [
        "018f1a4b-0000-7e5f-8a9b-0c1d2e3f0003",
        "018f1a4b-0001-7e5f-8a9b-0c1d2e3f0004"
      ]
    }
  ],
  "id": 1
}
```

## Error responses

| Code | Condition |
|---|---|
| `-32001` | `ShardsManager` singleton not initialised |
| `-32004` | Primary listing or data query failed |
| `-32600` | Invalid `duration` string |

## Notes

- **Result is a bare array.** The response is a JSON array at the top level, matching the shape of [`v2/primaries.explore`](v2_primaries_explore.md).
- **Count ≥ 2 filter.** Keys with only one numeric primary in the window are excluded. Use [`v2/primaries.explore`](v2_primaries_explore.md) to include all primary records regardless of data type.
- **Data type filtering happens in memory.** All primary records for the window are fetched and the numeric check is applied in the server process, not pushed into DuckDB SQL.
- **Workflow.** Use this method to discover which keys are suitable for [`v2/trends`](v2_trends.md), then call `v2/trends` with each returned key for full statistical analysis.
- The `session` parameter is stored for future caching integration and has no current effect on results.
