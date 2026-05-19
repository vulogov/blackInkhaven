# v2/tpl.search

Semantic vector search over templates in shards overlapping `[now − duration, now]`.

The query is embedded with the shared fastembed model and matched against the metadata + body embeddings of every shard's tplstorage. Results are merged across shards, sorted by score descending, and truncated to `limit`.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUIDv7 session identifier. Accepted and logged; reserved for future caching. |
| `duration` | string | no | `"1h"` | Lookback window in humantime format. |
| `query` | string | yes | — | Plain-text search query. |
| `limit` | integer | no | `10` | Maximum number of results returned across all shards. |

## Response

```json
{
  "results": [
    {
      "id":       "0192a3b4-c5d6-7e8f-9012-34567890abcd",
      "metadata": {
        "name":       "runbook.disk_full",
        "type":       "template",
        "tags":       ["runbook", "disk"]
      },
      "document": "1. Identify the volume via `df -h`. 2. Rotate logs in /var/log. 3. Restart filesystem.",
      "score":    0.83
    }
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `results[]` | array | Matched templates, sorted by `score` descending (higher = better). |
| `results[].id` | string | UUIDv7 of the template. |
| `results[].metadata` | object | Stored metadata document. |
| `results[].document` | string | Template body text decoded as UTF-8. |
| `results[].score` | number | Cosine similarity in `[0, 1]`. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/tpl.search",
    "params": {
      "session":  "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "duration": "24h",
      "query":    "disk recovery steps",
      "limit":    5
    },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32011` | Template store query failed |

## Notes

- **Multi-shard windows scan in parallel.** When the lookback overlaps multiple shards, per-shard searches dispatch via rayon and results are merged before sorting/truncation.
- **Manual + drain3 templates are indexed together.** Both kinds appear in results; differentiate via `metadata.type`.
- **Use `v2/tpl.list` for browsing.** This method ranks by relevance, not chronology.
