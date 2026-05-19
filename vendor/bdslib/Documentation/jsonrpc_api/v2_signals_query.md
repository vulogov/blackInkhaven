# v2/signals_query

Semantic search over the signal store using a plain-text query.

The query is embedded with the shared fastembed model and matched against the metadata embeddings of every shard's signal store. Results are merged across shards and ranked by cosine similarity descending.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUIDv7 session identifier. Accepted and logged; reserved for future caching. |
| `query` | string | yes | — | Plain-text search query, e.g. `"oom kill"`, `"deploy auth-service"`. |
| `limit` | integer | no | `20` | Maximum number of results returned across all shards. |

## Response

```json
{
  "query":   "oom kill auth",
  "count":   2,
  "results": [
    {
      "id":       "0192a3b4-c5d6-7e8f-9012-34567890abcd",
      "metadata": {
        "name":      "oom_killer.fired",
        "severity":  "critical",
        "timestamp": 1745603640,
        "service":   "auth"
      },
      "score": 0.81
    },
    {
      "id":       "0192a3b4-c5d6-7e8f-9012-34567890abce",
      "metadata": {
        "name":      "kernel.warn",
        "severity":  "warn",
        "timestamp": 1745603501,
        "service":   "auth"
      },
      "score": 0.62
    }
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `query` | string | Echoes the request. |
| `count` | integer | Number of returned results (`<= limit`). |
| `results[]` | array | Matched signals, sorted by `score` descending. |
| `results[].id` | string | UUIDv7 of the signal. |
| `results[].metadata` | object | Stored metadata. |
| `results[].score` | number | Cosine similarity in `[0, 1]`; higher = better match. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/signals_query",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "query":   "oom kill auth",
      "limit":   10
    },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32011` | Signal store query failed |

## Notes

- **No time window.** This method searches the entire signal store. Combine with `v2/signals` first if you only want recent matches.
- **Embedding model.** The query is embedded with the same shared fastembed model used everywhere else, so phrasing close to the original signal `name`/`severity`/metadata yields the best matches.
