# v2/search

Semantic vector search across all shards that fall within a lookback window, returning matching primary IDs with their event timestamps and cosine-similarity scores, sorted by score descending (most relevant first).

Unlike [`v2/fulltext`](v2_fulltext.md), which uses BM25 term matching, this method embeds the query with the same model used during ingestion (AllMiniLML6V2) and retrieves the nearest neighbours via HNSW. Results are ranked by semantic similarity rather than keyword frequency.

Only primary records are indexed for vector search. Secondary records are not directly searchable.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Reserved for future result caching; accepted and logged but not currently used for routing or filtering. |
| `query` | string | yes | — | Free-text query. Embedded with the same model as stored documents. |
| `duration` | string | yes | — | Lookback window from now in humantime format, e.g. `"1h"`, `"30min"`, `"7days"`. Only shards whose time interval overlaps `[now − duration, now + 1s)` are searched. |
| `limit` | integer | no | `10` | Maximum number of results to return after score sorting. |

## Response

```json
{
  "results": [
    { "id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b", "timestamp": 1745045200, "score": 0.912 },
    { "id": "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b", "timestamp": 1745042000, "score": 0.874 }
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `results` | array | Matching primary records ordered by `score` descending. Empty array when no documents match. |
| `results[].id` | string | UUID v7 of the matching primary record. |
| `results[].timestamp` | integer | Event time of the record as Unix seconds. |
| `results[].score` | number | Cosine similarity score in `[0, 1]`. Higher is more similar to the query. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/search",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "query": "disk I/O latency spike",
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
      { "id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b", "timestamp": 1745045200, "score": 0.912 },
      { "id": "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b", "timestamp": 1745042000, "score": 0.874 }
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

- **Primary records only.** Secondary records are not indexed in the vector store. To retrieve the secondaries linked to a result, call [`v2/secondaries`](v2_secondaries.md) with the returned `id`, or use [`v2/search.get`](v2_search_get.md) which returns full documents including embedded secondaries.
- **Score ordering.** Results are sorted by cosine similarity descending after merging across shards. A record with a high score from an older shard may appear before a lower-scoring recent record. Use [`v2/search.get`](v2_search_get.md) for timestamp-first ordering.
- **Per-shard candidate pool.** Each shard contributes up to `limit` vector candidates (via MMR reranking with candidate pool `max(limit × 2, 10)`) before cross-shard merging. Only the `limit` highest-scored are returned.
- The `session` parameter is stored for future caching integration and has no current effect on results.
