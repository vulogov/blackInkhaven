# v2/primaries.explore

Returns keys that have more than one primary record within the specified duration window, together with each key's record count and the UUIDs of those primary records.

This method is designed for data exploration: it surfaces the active metric namespaces in a time window and shows which keys are generating repeated independent readings (as opposed to duplicates or secondaries). Keys with exactly one primary in the window are excluded.

## Parameters

| Parameter | Type | Required | Description |
|---|---|---|---|
| `session` | string | yes | UUID v7 session identifier. Accepted and logged; reserved for future result caching. |
| `duration` | string | yes | Lookback window from now in humantime format, e.g. `"1h"`, `"30min"`, `"7days"`. Only shards whose time interval overlaps `[now − duration, now + 1s)` are queried. |

## Response

A JSON array, one entry per qualifying key, sorted alphabetically by `key`. Returns an empty array `[]` when no key has more than one primary record in the window.

```json
[
  {
    "key": "http.request",
    "count": 4120,
    "primary_id": [
      "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
      "018f1a2c-3c4d-7e5f-8a9b-0c1d2e3f4a5b"
    ]
  },
  {
    "key": "server.cpu",
    "count": 360,
    "primary_id": [
      "018f1a3a-0000-7e5f-8a9b-0c1d2e3f0001",
      "018f1a3a-0001-7e5f-8a9b-0c1d2e3f0002"
    ]
  }
]
```

| Field | Type | Description |
|---|---|---|
| `[].key` | string | Telemetry key (signal identifier / metric name). |
| `[].count` | integer | Number of primary records for this key within the duration window. Always ≥ 2. |
| `[].primary_id` | array of strings | UUID v7 strings of all primary records for this key in the window. Order reflects insertion order across shards. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/primaries.explore",
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
      "key": "http.request",
      "count": 4120,
      "primary_id": [
        "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
        "018f1a2c-3c4d-7e5f-8a9b-0c1d2e3f4a5c"
      ]
    },
    {
      "key": "server.cpu",
      "count": 360,
      "primary_id": [
        "018f1a3a-0000-7e5f-8a9b-0c1d2e3f0001",
        "018f1a3a-0001-7e5f-8a9b-0c1d2e3f0002"
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
| `-32004` | Primary listing or key grouping query failed |
| `-32600` | Invalid `duration` string |

## Notes

- **Count ≥ 2 filter.** Keys with exactly one primary record in the window are excluded. Use [`v2/keys`](v2_keys.md) to list all active keys regardless of primary count.
- **Result is a bare array.** Unlike most methods, the response is a JSON array at the top level, not wrapped in an object.
- **ID ordering.** Within each key, `primary_id` entries appear in the order records were encountered across shards (shards iterated in chronological order by start time, within each shard ordered by `ts` ascending).
- **Cross-shard deduplication.** A given UUID can appear in at most one shard, so IDs are never duplicated across shards.
- The `session` parameter is stored for future caching integration and has no current effect on results.
