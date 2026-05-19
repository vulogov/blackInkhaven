# v2/search.get

Semantic vector search across all shards that fall within a lookback window, returning complete primary documents sorted by event timestamp descending (most recent first), with `limit` applied after sorting.

Unlike [`v2/search`](v2_search.md), which ranks results by cosine-similarity score, this method surfaces the freshest matching records first. Each returned document includes its linked secondary records embedded inline. It is suited for dashboards where recency matters more than relevance rank.

Only primary records are indexed for vector search. Secondary records are not directly searchable.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Reserved for future result caching; accepted and logged but not currently used for routing or filtering. |
| `query` | string | yes | — | Free-text query. Embedded with the same model as stored documents. |
| `duration` | string | yes | — | Lookback window from now in humantime format, e.g. `"1h"`, `"30min"`, `"7days"`. Only shards whose time interval overlaps `[now − duration, now + 1s)` are searched. |
| `limit` | integer | no | `10` | Maximum number of results to return after timestamp sorting. |

## Response

```json
{
  "results": [
    {
      "id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
      "timestamp": 1745045200,
      "key": "disk.io",
      "data": "high latency spike detected",
      "_score": 0.874,
      "secondaries": [
        {
          "id": "018f1a2e-aaaa-7e5f-bbbb-ccccddddeeee",
          "timestamp": 1745045210,
          "key": "disk.io",
          "data": "latency normalised"
        }
      ]
    }
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `results` | array | Matching primary records ordered by `timestamp` descending. Empty array when no documents match. |
| `results[].id` | string | UUID v7 of the matching primary record. |
| `results[].timestamp` | integer | Event time of the record as Unix seconds. |
| `results[].key` | string | Signal identifier / metric name. |
| `results[].data` | any | Measured value. |
| `results[]._score` | number | Cosine similarity score from the vector index. |
| `results[].secondaries` | array | Full documents of all secondary records linked to this primary. Empty array when there are none. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/search.get",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "query": "authentication failure",
      "duration": "6h",
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
      {
        "id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
        "timestamp": 1745045200,
        "key": "auth.error",
        "data": "invalid credentials for user admin",
        "_score": 0.934,
        "secondaries": []
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
| `-32004` | Vector search failed (e.g. embedding model error) |

## Notes

- **Primary records only.** Secondary records are not indexed in the vector store but are returned inline in each primary's `"secondaries"` array.
- **Timestamp ordering.** Results are sorted by `timestamp` descending after merging across shards, so a weakly matching recent record may appear before a strongly matching older one. Use [`v2/search`](v2_search.md) for score-first ordering without document bodies.
- **Per-shard candidate pool.** Each shard contributes up to `limit` vector candidates (via MMR reranking) before cross-shard merging. Only the `limit` most recent are returned after re-sorting.
- The `session` parameter is stored for future caching integration and has no current effect on results.
