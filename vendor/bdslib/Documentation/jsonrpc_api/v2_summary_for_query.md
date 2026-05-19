# v2/summary_for_query

Build an extractive TextRank summary of every primary observability record matching a vector query.

The supplied `query` string is wrapped as a JSON string value, embedded with the shared fastembed model, and used to perform a semantic vector search across every shard registered in the catalog (lookback `365days`). For each matching primary, bdsnode extracts a text body from `data["value"]` (preferred) or `data["raw"]` (fallback); records whose `data` is a bare number or whose `data["value"]` is a number are silently dropped — those are numeric measurements meant for `v2/trends`, not text. The collected bodies are fed to `bdslib::analysis::textrank::textrank_summary_with`, and the highest-ranked bodies are returned joined as a single string.

Use this when you need a focused summary of "what the records relevant to *X* are saying" — for query-aware alert previews, RAG pre-context, or seed text for a follow-up LLM prompt.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUIDv7 transaction identifier. Accepted and logged; reserved for future result caching. |
| `query` | string | yes | — | Plain-text vector query. Wrapped server-side as a JSON string and embedded with the shared model. |
| `max_sentences` | integer | no | `0` | Hard cap on the number of bodies kept in the summary. Set to `0` to derive the cap from `ratio` instead. |
| `ratio` | number | no | `0.3` | When `max_sentences == 0`, this fraction of the matched bodies is kept (rounded up, minimum 1). Clamped to `(0.0, 1.0]`. |
| `min_word_len` | integer | no | `2` | Tokens shorter than this many characters are dropped before scoring. |
| `damping` | number | no | `0.85` | PageRank damping factor. |
| `iters` | integer | no | `30` | Maximum PageRank iterations. Loop also exits early once the per-iteration L1-norm score change drops below `tolerance`. |
| `tolerance` | number | no | `1e-4` | Early-exit threshold on the per-iteration L1-norm change in scores. |

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
  "max_sentences": 2,
  "ratio": 0.3,
  "summary": "nginx upstream timeout 502 service=auth nginx upstream timeout 502 service=billing"
}
```

| Field | Type | Description |
|---|---|---|
| `query` | string | Query echoed from the request. |
| `max_sentences` | integer | Cap echoed from the request (`0` means auto-sized via `ratio`). |
| `ratio` | number | Ratio echoed from the request (effective when `max_sentences == 0`). |
| `summary` | string | TextRank summary built from the highest-ranked text bodies among the vector-matched primaries. Empty string when no matching record had a text body. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/summary_for_query",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "query": "nginx upstream timeout",
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

- **No time window parameter.** `summary_for_query` scans a default lookback of `365days` so callers don't have to specify a window. Use `v2/search.get` directly if you need a custom time bound.
- **Numeric exclusion is silent.** Records skipped by the body extractor are not reported back; if the response is empty the typical cause is "every matching record was a numeric measurement".
- **Empty matches.** When the vector search returns no matches, `summary` is the empty string. No error is raised.
- **Determinism.** Vector search + TextRank are both deterministic — same query against the same store always produces the same summary.
- The `session` parameter is stored for future caching integration and has no current effect on results.
