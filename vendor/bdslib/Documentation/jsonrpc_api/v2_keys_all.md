# v2/keys.all

Returns the unique, sorted list of primary record keys within a lookback window, optionally filtered by a shell-glob pattern.

This is the pattern-aware counterpart of [`v2/keys`](v2_keys.md). When called with the default pattern `"*"` it returns exactly the same result as `v2/keys`. Provide a narrower pattern to restrict the output to a specific key namespace (e.g. `"server.*"` to list only server metrics).

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Reserved for future result caching; accepted and logged but not currently used. |
| `duration` | string | yes | — | Lookback window from now in humantime format, e.g. `"1h"`, `"30min"`, `"7days"`. Only shards whose time interval overlaps `[now − duration, now + 1s)` are queried. |
| `key` | string | no | `"*"` | Shell-glob pattern matched against the `key` field of primary records. `*` matches any sequence of characters, `?` matches any single character, `[abc]` matches a character class. Pass `"*"` (or omit) to return all keys. |

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
| `keys` | array of strings | Alphabetically sorted, deduplicated list of primary record keys that match the pattern within the time window. Empty array when no keys match. |

## Examples

```bash
# All keys in the last hour (pattern omitted → defaults to "*")
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/keys.all",
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
  "result": {
    "keys": ["http.request", "queue.depth", "server.cpu", "server.memory"]
  },
  "id": 1
}
```

```bash
# Only keys under the "server." namespace
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/keys.all",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "duration": "1h",
      "key": "server.*"
    },
    "id": 2
  }' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": {
    "keys": ["server.cpu", "server.memory"]
  },
  "id": 2
}
```

## Error responses

| Code | Condition |
|---|---|
| `-32001` | `ShardsManager` singleton not initialised |
| `-32004` | Shard query failed |
| `-32600` | Invalid `duration` string |

## Notes

- Pattern matching uses DuckDB's `GLOB` operator, which follows shell conventions. `*` matches any sequence (including empty), `?` matches exactly one character, `[abc]` matches a set.
- Only `is_primary = 1` records are considered; secondary records are excluded.
- Deduplication and sorting are performed using a `BTreeSet` across all matching shards.
- The `session` parameter is stored for future caching integration and has no current effect on results.
- To retrieve the record IDs (not just key names) for a pattern, use [`v2/keys.get`](v2_keys_get.md).
