# v2/shards

Returns a list of shards known to the `ShardsManager`, each with its time boundaries, filesystem path, and primary/secondary record counts. Supports optional time window filtering to restrict which shards are included.

## Parameters

All parameters are optional. See [time window parameters](README.md#time-window-parameters) for details.

| Parameter | Type | Description |
|---|---|---|
| `duration` | string | Lookback window from now, e.g. `"1h"`, `"24h"`, `"7d"` |
| `start_ts` | integer | Range start as Unix seconds |
| `end_ts` | integer | Range end as Unix seconds |

When a time window is provided, only shards that overlap the window are returned. Counts within those shards are also filtered to the window.

## Response

A JSON array of shard objects:

```json
[
  {
    "id": "018f1a2b-0000-7000-8000-000000000001",
    "path": "/var/lib/bdslib/shards/2025-04-19.db",
    "start_ts": 1745020800,
    "end_ts": 1745107200,
    "primary_count": 18400,
    "secondary_count": 3210
  },
  {
    "id": "018f1a2b-0000-7000-8000-000000000002",
    "path": "/var/lib/bdslib/shards/2025-04-20.db",
    "start_ts": 1745107200,
    "end_ts": 1745193600,
    "primary_count": 21050,
    "secondary_count": 4780
  }
]
```

| Field | Type | Description |
|---|---|---|
| `id` | string | UUID of the shard |
| `path` | string | Absolute filesystem path to the shard's DuckDB file |
| `start_ts` | integer | Shard interval start as Unix seconds (inclusive) |
| `end_ts` | integer | Shard interval end as Unix seconds (exclusive) |
| `primary_count` | integer | Number of primary records in this shard (within the queried window) |
| `secondary_count` | integer | Number of secondary records in this shard (within the queried window) |

Returns an empty array `[]` when no shards exist or none fall within the requested window.

## Examples

```bash
# all shards
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"v2/shards","params":{},"id":1}' | jq

# shards covering the last 24 hours
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"v2/shards","params":{"duration":"24h"},"id":1}' | jq

# shards in an explicit range
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"v2/shards","params":{"start_ts":1745000000,"end_ts":1745086399},"id":1}' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32001` | `ShardsManager` singleton not initialised |
| `-32002` | Shard index query failed |
| `-32003` | Shard open failed |
| `-32004` | Record count query failed |
| `-32600` | Invalid `duration` string |

## Notes

- Shard boundaries (`start_ts` / `end_ts`) are fixed at shard creation time and represent the full capacity of the shard, not the actual event range within it. Use [`v2/timeline`](v2_timeline.md) to get the real event span.
- When a time window is specified, `primary_count` and `secondary_count` reflect only records whose `ts` falls within that window, even if the shard itself spans a wider interval.
- Shards are returned in the order reported by the shard index (chronological by `start_time`).
