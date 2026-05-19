# v2/doc.search.json

Semantic search in the document store by embedding a structured JSON query object. The query object is converted to a `"path: value"` string via `json_fingerprint` before embedding, so field names contribute to the semantic signal alongside values.

Use this when your query naturally has structure — e.g., `{"service": "payment", "severity": "P1"}` — and you want the field names to influence the semantic match.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Logged but not used for routing. |
| `query` | object | yes | — | JSON object used as the semantic query. Converted to a fingerprint string before embedding. |
| `limit` | integer | no | `10` | Maximum number of results to return. |

## Response

Same structure as [`v2/doc.search`](v2_doc_search.md):

```json
{
  "results": [
    {
      "id": "018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
      "metadata": { "name": "DB Connection Pool Emergency", "category": "runbook", "service": "database" },
      "document": "When connection pool utilisation exceeds 90%…",
      "score": 0.891
    }
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `results` | array | Ranked results, sorted by `score` descending. |
| `results[].id` | string | UUID of the matching document or chunk. |
| `results[].metadata` | object | JSON metadata for this record. |
| `results[].document` | string | UTF-8 decoded content text. |
| `results[].score` | number | Cosine similarity score in `[0, 1]`. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/doc.search.json",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "query": {"service": "database", "failure_mode": "connection pool exhausted"},
      "limit": 5
    },
    "id": 1
  }' | jq
```

## bdscmd

```bash
bdscmd doc-search-json \
  --query '{"service":"database","failure_mode":"connection pool exhausted"}' \
  --limit 5
```

## Error responses

| Code | Condition |
|---|---|
| `-32001` | `ShardsManager` singleton not initialised |
| `-32011` | Document store search failed |

## Notes

- The `json_fingerprint` transformation produces a multi-line string like `"service: database\nfailure_mode: connection pool exhausted"`. This is then embedded as a single text sequence.
- For plain-text queries, [`v2/doc.search`](v2_doc_search.md) is simpler and produces equivalent results for unstructured queries.
