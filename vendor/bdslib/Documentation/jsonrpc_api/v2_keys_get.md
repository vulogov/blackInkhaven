# v2/keys.get

Returns all primary records whose key matches a shell-glob pattern within a lookback window, with each record's linked secondary IDs. Results are sorted by timestamp ascending.

Shell-glob patterns follow DuckDB conventions: `*` matches any sequence of characters, `?` matches any single character, `[abc]` matches a character class.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Reserved for future result caching; accepted and logged but not currently used for routing or filtering. |
| `duration` | string | yes | — | Lookback window from now in humantime format, e.g. `"1h"`, `"30min"`, `"7days"`. Only shards whose time interval overlaps `[now − duration, now + 1s)` are queried. |
| `key` | string | yes | — | Shell-glob pattern to match against the `key` field of primary records, e.g. `"server.*"`, `"http.requ?st"`, `"*.error"`. |

## Response

```json
{
  "results": [
    {
      "timestamp": 1745045100,
      "primary_id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
      "secondary_ids": [
        "018f1a2e-aaaa-7e5f-bbbb-ccccddddeeee"
      ]
    },
    {
      "timestamp": 1745045200,
      "primary_id": "018f1a3a-beef-7e5f-cafe-0c1d2e3f4a5b",
      "secondary_ids": []
    }
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `results` | array | Matching primary records ordered by `timestamp` ascending. Empty array when no records match. |
| `results[].timestamp` | integer | Event time of the primary record as Unix seconds. |
| `results[].primary_id` | string | UUID v7 of the matching primary record. |
| `results[].secondary_ids` | array of strings | UUID v7 strings of all secondary records linked to this primary, ordered by their timestamp ascending. Empty array when there are none. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/keys.get",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "duration": "1h",
      "key": "server.*"
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
        "timestamp": 1745045100,
        "primary_id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
        "secondary_ids": ["018f1a2e-aaaa-7e5f-bbbb-ccccddddeeee"]
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
| `-32004` | Key pattern query failed |
| `-32600` | Invalid `duration` string |

## Notes

- **Pattern matching.** The `key` pattern uses DuckDB's `GLOB` operator, which follows shell conventions. To match all keys under a namespace, use `"server.*"`. To match exactly one character, use `"??"` (two characters). A literal `*`, `?`, or `[` in a key name must be escaped — this is not currently supported by the server; keys with those characters cannot be matched exactly.
- **Primary records only.** Secondary records are not matched by the key pattern but are returned as `secondary_ids` within each primary's result entry.
- **Timestamp ordering.** Results are sorted by `timestamp` ascending across all matching shards.
- **No document bodies.** To retrieve full document bodies, call [`v2/primary`](v2_primary.md) with each `primary_id`, or use [`v2/search.get`](v2_search_get.md) for semantic retrieval with inline secondaries.
- The `session` parameter is stored for future caching integration and has no current effect on results.
