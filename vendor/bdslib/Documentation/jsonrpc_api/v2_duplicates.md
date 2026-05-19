# v2/duplicates

Returns a map of primary record UUID → list of duplicate timestamps. A duplicate is an exact-match record: same `key` and `data_text`, but ingested at a different timestamp. Supports optional time window filtering.

This method is useful for auditing data quality: identifying which primary records have been re-ingested multiple times with different timestamps.

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
  "duplicates": {
    "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b": [1745042010, 1745042020],
    "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5c": [1745043100]
  }
}
```

| Field | Type | Description |
|---|---|---|
| `duplicates` | object | Keys are primary record UUID strings. Values are arrays of Unix seconds representing each duplicate occurrence. |

Returns `{"duplicates": {}}` when no duplicates exist in the queried window.

Primary records with zero duplicates are omitted from the map.

## Examples

```bash
# all duplicates across all data
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"v2/duplicates","params":{},"id":1}' | jq

# duplicates ingested in the last hour
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"v2/duplicates","params":{"duration":"1h"},"id":1}' | jq

# duplicates in an explicit range
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"v2/duplicates","params":{"start_ts":1745000000,"end_ts":1745086399},"id":1}' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": {
    "duplicates": {
      "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b": [1745042010, 1745042020],
      "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5c": [1745043100]
    }
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
| `-32004` | Deduplication entry query failed |
| `-32600` | Invalid `duration` string |

## Notes

- Duplicate detection is exact-match only (same `key` + `data_text`). Near-duplicate detection (embedding similarity) produces secondary records instead; see [`v2/secondaries`](v2_secondaries.md).
- The duplicate timestamps are stored in the `dedup_tracking` table and are joined against `telemetry` to resolve the primary UUID.
- The `duplications` field on individual [`v2/primary`](v2_primary.md) and [`v2/secondary`](v2_secondary.md) responses shows the same data per-record.
- Use `bdscli generate --duplicate <pct>` to inject synthetic duplicates for testing.
