# v2/summary_lsa_for_query

Build an extractive LSA summary of primary observability records matching a plain-text vector query.

The query is embedded with the same model used for `v2/search`. Matching primary records are pulled from all shards (default lookback 365 days). Text bodies are extracted via the same rule as `v2/summary_lsa_for_recent`; numeric measurements are silently skipped. The collected bodies are fed to `bdslib::analysis::lsa::lsa_summary_with` and the highest-ranked ones are returned joined as a single string.

Use this when you need a summary scoped to a specific failure mode or topic rather than a time window.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUIDv7 transaction identifier. Accepted and logged; reserved for future result caching. |
| `query` | string | yes | — | Plain-text vector query embedded server-side. Semantically similar primary records are retrieved. |
| `max_sentences` | integer | no | `0` | Hard cap on the number of bodies in the summary. `0` defers to `ratio`. |
| `ratio` | number | no | `0.3` | Fraction of bodies kept when `max_sentences == 0`. Clamped to `(0.0, 1.0]`. |
| `min_word_len` | integer | no | `2` | Tokens shorter than this many characters are dropped before scoring. |
| `n_concepts` | integer | no | `3` | Number of LSA concepts (singular vectors) to extract. |
| `power_iters` | integer | no | `50` | Power-iteration steps per eigenvector. |

## Body extraction

| `data` shape | Action |
|---|---|
| `12.5` (bare number) | skipped — numeric measurement |
| `{ "value": 12.5 }` | skipped — numeric measurement |
| `{ "value": "text…" }` | extracted as the body |
| `{ "raw": "text…" }` (when `value` missing/non-string) | extracted as the body |
| anything else | skipped |

## Response

```json
{
  "query": "nginx upstream timeout",
  "max_sentences": 0,
  "ratio": 0.3,
  "summary": "nginx upstream timeout 502 service=auth nginx upstream timeout 502 service=billing"
}
```

| Field | Type | Description |
|---|---|---|
| `query` | string | Query string echoed from the request. |
| `max_sentences` | integer | Cap echoed from the request. |
| `ratio` | number | Ratio echoed from the request. |
| `summary` | string | LSA summary of matching text records. Empty string when no text-bearing records matched the query. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/summary_lsa_for_query",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "query": "nginx upstream connection refused",
      "max_sentences": 5
    },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32004` | Vector search or summarisation failed |

## Notes

- **Default lookback.** The vector search covers 365 days — long enough to span any realistic operational archive.
- **Empty result.** When no matching records have a text body, `summary` is the empty string. No error is raised.
- **Determinism.** Fully deterministic given the same indexed records and query.
- **vs `v2/summary_for_query`.** Same body-extraction and lookup logic; differs only in the ranking backend (LSA vs TextRank).
