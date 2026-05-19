# v2/count

Returns the total number of telemetry records stored across all shards. Supports optional time window filtering.

## Parameters

All parameters are optional. See [time window parameters](README.md#time-window-parameters) for details.

| Parameter | Type | Description |
|---|---|---|
| `duration` | string | Lookback window from now, e.g. `"1h"`, `"24h"`, `"7d"` |
| `start_ts` | integer | Range start as Unix seconds |
| `end_ts` | integer | Range end as Unix seconds |

## Response

```json
{
  "count": 42380
}
```

| Field | Type | Description |
|---|---|---|
| `count` | integer | Total number of records in the queried window |

## Examples

```bash
# total across all data
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"v2/count","params":{},"id":1}' | jq

# last hour
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"v2/count","params":{"duration":"1h"},"id":1}' | jq

# explicit range
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"v2/count","params":{"start_ts":1745000000,"end_ts":1745086399},"id":1}' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": {
    "count": 42380
  },
  "id": 1
}
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

- When a time window is specified, only shards that overlap the window are queried.
- Count includes both primary and secondary records.
