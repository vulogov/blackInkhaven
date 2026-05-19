# v2/fulltext

Full-text search across all shards that fall within a lookback window, returning the highest-scored matching primary IDs and their BM25 relevance scores.

Documents are ranked by Tantivy's BM25 algorithm. Each shard contributes up to `limit` candidates; results from all shards are merged, re-ranked globally by score, and truncated to `limit` before the response is returned. Only primary records are indexed for full-text search — secondary records are not directly searchable.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Reserved for future result caching; accepted and logged but not used for routing or filtering. |
| `query` | string | yes | — | Full-text query in [Tantivy query syntax](https://docs.rs/tantivy/latest/tantivy/query/struct.QueryParser.html). Supports term queries (`cpu`), phrase queries (`"disk full"`), boolean operators (`cpu AND usage`), and field-scoped terms. |
| `duration` | string | yes | — | Lookback window from now in humantime format, e.g. `"1h"`, `"30min"`, `"7days"`. Only shards whose time interval overlaps `[now − duration, now + 1s)` are searched. |
| `limit` | integer | no | `10` | Maximum number of results to return. Results are already ranked by score; the top `limit` hits are returned. |

## Response

```json
{
  "results": [
    { "id": "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b", "score": 4.872 },
    { "id": "018f1a2b-0000-7fff-aaaa-bbbbccccdddd", "score": 3.210 },
    { "id": "018f1a2c-1111-7e5f-2222-333344445555", "score": 1.005 }
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `results` | array | Ordered list of matches, highest score first. Empty array when no documents match. |
| `results[].id` | string | UUID v7 of the matching primary record. |
| `results[].score` | number | BM25 relevance score. Higher is more relevant. Scores are shard-local and not normalised; use them only for relative ordering within a single response. |

## Example

```bash
# Find top 5 records mentioning "disk" in the last hour
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/fulltext",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "query": "disk",
      "duration": "1h",
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
      { "id": "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b", "score": 6.314 },
      { "id": "018f1a2c-dead-7e5f-beef-0c1d2e3f4a5b", "score": 4.100 },
      { "id": "018f1a2d-0001-7e5f-0002-0c1d2e3f4a5b", "score": 2.987 }
    ]
  },
  "id": 1
}
```

```bash
# Boolean query — "cpu" AND "high" in the last 30 minutes
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/fulltext",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "query": "cpu AND high",
      "duration": "30min"
    },
    "id": 2
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32001` | `ShardsManager` singleton not initialised |
| `-32002` | Full-text search failed (e.g. malformed query syntax) |

## Notes

- The `session` parameter is stored for future caching integration. Currently it has no effect on results.
- Tantivy BM25 scores are computed independently per shard. When results from multiple shards are merged, scores remain comparable in direction (higher = better match) but not in absolute magnitude across shards.
- For the same query with full document bodies and linked secondaries, use [`v2/fulltext.get`](v2_fulltext_get.md).
- Tantivy query syntax reference: terms are tokenised and stemmed; phrase queries require double quotes; `AND`, `OR`, `NOT` are supported; parentheses group sub-expressions.
