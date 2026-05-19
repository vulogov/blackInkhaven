# v2/fulltext.get

Full-text search across all shards that fall within a lookback window, returning complete primary documents with their linked secondary records.

This method uses the same Tantivy BM25 index as [`v2/fulltext`](v2_fulltext.md) but fetches the full stored document for every hit rather than returning only IDs and scores. Use it when you need to display or process the matched records directly. For large result sets or latency-sensitive use cases where only IDs are needed, prefer `v2/fulltext`.

## Parameters

| Parameter | Type | Required | Description |
|---|---|---|---|
| `session` | string | yes | UUID v7 session identifier. Reserved for future result caching; accepted and logged but not used for routing or filtering. |
| `query` | string | yes | Full-text query in [Tantivy query syntax](https://docs.rs/tantivy/latest/tantivy/query/struct.QueryParser.html). Supports term queries (`cpu`), phrase queries (`"disk full"`), boolean operators (`cpu AND usage`), and field-scoped terms. |
| `duration` | string | yes | Lookback window from now in humantime format, e.g. `"1h"`, `"30min"`, `"7days"`. Only shards whose time interval overlaps `[now − duration, now + 1s)` are searched. |

## Response

```json
{
  "results": [
    {
      "id": "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
      "timestamp": 1745042000,
      "key": "host.disk",
      "data": { "host": "db-02", "mount": "/var", "used_pct": 91.4 },
      "source": "agent-v2",
      "secondaries": [
        {
          "id": "018f1a2b-aaaa-7e5f-bbbb-ccccddddeeee",
          "timestamp": 1745042060,
          "key": "host.disk",
          "data": { "host": "db-02", "mount": "/var", "used_pct": 92.1 },
          "source": "agent-v2"
        }
      ]
    }
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `results` | array | Ordered list of matching primary documents, in Tantivy relevance order within each shard (shards iterated oldest-first). Empty array when no documents match. |
| `results[].id` | string | UUID v7 of the primary record. |
| `results[].timestamp` | integer | Event time as Unix seconds. |
| `results[].key` | string | Dotted metric or log key (e.g. `host.disk`, `syslog`). |
| `results[].data` | any | Original payload stored with the record. |
| *(metadata fields)* | any | Any extra fields from the original ingested document appear inline at the top level (not nested under a `"metadata"` key). |
| `results[].secondaries` | array | Full documents of all secondary records linked to this primary. Each secondary has the same shape as a primary entry (without `secondaries`). Empty array if none exist. |

## Example

```bash
# Find all records about "disk" in the last 2 hours
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/fulltext.get",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "query": "disk",
      "duration": "2h"
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
        "id": "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
        "timestamp": 1745042000,
        "key": "host.disk",
        "data": { "host": "db-02", "mount": "/var", "used_pct": 91.4 },
        "source": "agent-v2",
        "secondaries": []
      }
    ]
  },
  "id": 1
}
```

```bash
# Phrase query — exact phrase "out of space" in the last 6 hours
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/fulltext.get",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "query": "\"out of space\"",
      "duration": "6h"
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

- Results are ordered by Tantivy relevance within each shard; shards are iterated oldest-first. Unlike [`v2/fulltext`](v2_fulltext.md) there is no global cross-shard re-ranking by score — the document bodies are returned in per-shard relevance order.
- Each shard returns at most 100 matching documents. To constrain the total result size, narrow the `duration` window or use `v2/fulltext` with a `limit` to retrieve IDs first, then fetch individual records via [`v2/primary`](v2_primary.md).
- The `session` parameter is stored for future caching integration. Currently it has no effect on results.
- Secondary records are attached automatically. A primary with no secondaries has `"secondaries": []`.
