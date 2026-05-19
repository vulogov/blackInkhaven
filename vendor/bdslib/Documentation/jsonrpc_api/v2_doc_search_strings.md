# v2/doc.search.strings

Semantic search in the document store by embedding a plain-text query, returning each result serialised as a `json_fingerprint` string rather than a structured JSON object.

Fingerprinted strings are suitable for direct re-ingestion into the full-text search engine, for use as context lines in LLM prompts, or for embedding pipelines that need a flat text representation of the document.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Logged but not used for routing. |
| `query` | string | yes | — | Plain-text query string. Embedded with the same model as stored documents. |
| `limit` | integer | no | `10` | Maximum number of results to return. |

## Response

```json
{
  "results": [
    "id: 018f1a2d-3c4d-7e5f-8a9b-0c1d2e3f4a5b\nscore: 0.934\ndocument_name: Payment Service Incident Runbook\nchunk_index: 2\ndocument: When the circuit breaker opens…",
    "id: 018f1a2d-0000-7e5f-8a9b-0c1d2e3f4a5b\nscore: 0.871\nname: Circuit Breaker Quick Reference\ndocument: Open the dashboard and check…"
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `results` | array of strings | Each element is a flat `json_fingerprint` string representing one result, sorted by score descending. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/doc.search.strings",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "query": "memory pressure OOM killer",
      "limit": 5
    },
    "id": 1
  }' | jq
```

## bdscmd

```bash
bdscmd doc-search-strings --query "memory pressure OOM killer" --limit 5
```

## Error responses

| Code | Condition |
|---|---|
| `-32001` | `ShardsManager` singleton not initialised |
| `-32011` | Document store search failed |

## Notes

- Use [`v2/doc.search`](v2_doc_search.md) when you need structured JSON results with individual fields accessible; use this method when you need flat strings for downstream text processing.
- The fingerprint format is `"key: value\nkey2: value2\n…"` — each field on its own line.
