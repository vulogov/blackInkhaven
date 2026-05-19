# v2/keys

Returns the unique, sorted list of keys present on primary telemetry records across all shards that overlap the specified lookback window.

Keys are the `key` field stored on each primary document — they identify the logical type or category of a record (e.g. `"server.cpu"`, `"http.request"`).

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Reserved for future result caching; accepted and logged but not currently used for routing or filtering. |
| `duration` | string | yes | — | Lookback window from now in humantime format, e.g. `"1h"`, `"30min"`, `"7days"`. Only shards whose time interval overlaps `[now − duration, now + 1s)` are queried. |

## Response

```json
{
  "keys": [
    "http.request",
    "server.cpu",
    "server.memory"
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `keys` | array of strings | Alphabetically sorted, deduplicated list of primary record keys within the time window. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/keys",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "duration": "30min"
    },
    "id": 1
  }' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": {
    "keys": ["http.request", "server.cpu", "server.memory"]
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
| `-32004` | Key query failed |
| `-32600` | Invalid `duration` string |

## Notes

- Deduplication and sorting are performed using a `BTreeSet`, so the result is always alphabetically ordered regardless of which shards contributed entries.
- Only `is_primary = 1` records are considered; secondary records are excluded.
- The `session` parameter is stored for future caching integration and has no current effect on results.
- To retrieve records matching a specific key pattern, use [`v2/keys.get`](v2_keys_get.md).
