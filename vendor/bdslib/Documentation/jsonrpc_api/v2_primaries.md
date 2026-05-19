# v2/primaries

Returns the UUIDs of all primary records across all shards. Supports optional time window filtering.

Use this method to enumerate primary record identifiers, then fetch full documents with [`v2/primary`](v2_primary.md).

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
  "ids": [
    "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
    "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5c"
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `ids` | array of strings | UUID v7 strings of all primary records in the queried window, in shard-insertion order |

## Examples

```bash
# all primaries
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"v2/primaries","params":{},"id":1}' | jq

# primaries from the last 6 hours
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"v2/primaries","params":{"duration":"6h"},"id":1}' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": {
    "ids": [
      "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
      "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5c"
    ]
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
| `-32004` | Primary listing query failed |
| `-32600` | Invalid `duration` string |

## Notes

- Records are returned in the order they appear across shards; no global sort is applied.
- Each UUID encodes the event timestamp (UUID v7), so the IDs themselves carry temporal information.
