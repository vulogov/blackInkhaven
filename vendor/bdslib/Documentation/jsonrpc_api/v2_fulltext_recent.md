# v2/fulltext.recent

Full-text search across all shards that fall within a lookback window, returning matching primary IDs with their event timestamps and BM25 relevance scores, sorted by timestamp descending (most recent first).

Unlike [`v2/fulltext`](v2_fulltext.md), which ranks results purely by relevance score, this method surfaces the freshest matching records first. It is suited for monitoring dashboards and alert feeds where recent activity is more actionable than the best BM25 match from hours ago.

Only primary records are indexed for full-text search. Secondary records are not directly searchable.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Reserved for future result caching; accepted and logged but not currently used for routing or filtering. |
| `query` | string | yes | — | Full-text query in [Tantivy query syntax](https://docs.rs/tantivy/latest/tantivy/query/struct.QueryParser.html). Supports term queries (`cpu`), phrase queries (`"disk full"`), boolean operators (`cpu AND usage`), and field-scoped terms. |
| `duration` | string | yes | — | Lookback window from now in humantime format, e.g. `"1h"`, `"30min"`, `"7days"`. Only shards whose time interval overlaps `[now − duration, now + 1s)` are searched. |
| `limit` | integer | no | `10` | Maximum number of results to return after timestamp sorting. |

## Response

```json
{
  "results": [
    { "id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b", "timestamp": 1745045200, "score": 2.310 },
    { "id": "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b", "timestamp": 1745042000, "score": 4.872 },
    { "id": "018f1a2a-aaaa-7e5f-bbbb-ccccddddeeee", "timestamp": 1745038400, "score": 1.105 }
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `results` | array | Matching primary records ordered by `timestamp` descending. Empty array when no documents match. |
| `results[].id` | string | UUID v7 of the matching primary record. |
| `results[].timestamp` | integer | Event time of the record as Unix seconds. |
| `results[].score` | number | BM25 relevance score for this record. Higher is more relevant within the same shard, but scores are not normalised across shards. |

## Example

```bash
# Most recent 5 records mentioning "disk" in the last 2 hours
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/fulltext.recent",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "query": "disk",
      "duration": "2h",
      "limit": 5
    },
    "id": 1
  }' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": {
    "results": [
      { "id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b", "timestamp": 1745045200, "score": 2.310 },
      { "id": "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b", "timestamp": 1745042000, "score": 4.872 },
      { "id": "018f1a2a-aaaa-7e5f-bbbb-ccccddddeeee", "timestamp": 1745038400, "score": 1.105 }
    ]
  },
  "id": 1
}
```

```bash
# Phrase query with default limit of 10
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/fulltext.recent",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "query": "\"out of memory\"",
      "duration": "6h"
    },
    "id": 2
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32001` | `ShardsManager` singleton not initialised |
| `-32002` | Full-text search or timestamp lookup failed (e.g. malformed query syntax) |

## Notes

- **Primary records only.** Secondary records are not indexed in Tantivy. To retrieve the secondaries linked to a result, call [`v2/secondaries`](v2_secondaries.md) with the returned `id`.
- **Sorting trade-off.** Results are ranked by `timestamp` descending after the FTS pass, so a highly relevant record from earlier in the window may appear after a weakly matching recent one. Use [`v2/fulltext`](v2_fulltext.md) when score-first ordering is preferred.
- **Timestamp source.** Each hit's timestamp is fetched from DuckDB via an indexed primary-key lookup immediately after the Tantivy search. Records deleted between the two operations are silently omitted from the response.
- **Per-shard candidate pool.** Each shard contributes up to `limit` FTS candidates before cross-shard merging. With many shards spanning the `duration` window, the effective candidate pool before truncation is `limit × number_of_shards`. Only the `limit` most recent are returned.
- The `session` parameter is stored for future caching integration and has no current effect on results.
