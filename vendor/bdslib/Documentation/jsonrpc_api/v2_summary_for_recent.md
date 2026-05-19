# v2/summary_for_recent

Build an extractive TextRank summary of every primary observability record observed in the lookback window.

For each primary record whose event timestamp falls in `[now − duration, now)`, bdsnode extracts a text body from `data["value"]` (preferred) or `data["raw"]` (fallback). Records whose `data` is a bare number or whose `data["value"]` is a number are silently dropped — those are numeric measurements meant for `v2/trends`, not text. The collected bodies are fed to `bdslib::analysis::textrank::textrank_summary_with`, and the highest-ranked bodies are returned joined as a single string.

Use this when you need a one-glance picture of "what the text records in this window are about" — for dashboard headers, alert previews, or as the seed text for a follow-up LLM prompt.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUIDv7 transaction identifier. Accepted and logged; reserved for future result caching. |
| `duration` | string | yes | — | Lookback window from now in humantime format, e.g. `"30min"`, `"1h"`, `"7days"`. Only primaries whose `ts` falls in `[now − duration, now)` are considered. |
| `max_sentences` | integer | no | `0` | Hard cap on the number of bodies kept in the summary. Set to `0` to derive the cap from `ratio` instead. |
| `ratio` | number | no | `0.3` | When `max_sentences == 0`, this fraction of the input bodies is kept (rounded up, minimum 1). Clamped to `(0.0, 1.0]`. |
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
  "duration": "1h",
  "max_sentences": 0,
  "ratio": 0.3,
  "summary": "nginx upstream timeout 502 service=auth nginx upstream timeout 502 service=billing user alice logged in successfully"
}
```

| Field | Type | Description |
|---|---|---|
| `duration` | string | Lookback window echoed from the request. |
| `max_sentences` | integer | Cap echoed from the request (`0` means auto-sized via `ratio`). |
| `ratio` | number | Ratio echoed from the request (effective when `max_sentences == 0`). |
| `summary` | string | TextRank summary built from the highest-ranked text bodies in their original input order. Empty string when the window contained no text-bearing primaries. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/summary_for_recent",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "duration": "1h",
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
| `-32004` | Primary query or summarisation failed |
| `-32600` | Invalid `duration` string |

## Notes

- **Empty window.** When no text-bearing primaries were observed in the window, `summary` is the empty string. No error is raised.
- **Single record.** A window with exactly one text-bearing primary returns its body verbatim — TextRank degenerates gracefully on inputs of length one.
- **Numeric exclusion is silent.** Records skipped by the body extractor are not reported back; if the response is empty the typical cause is "every record was a numeric measurement".
- **Determinism.** The algorithm is fully deterministic — same inputs always produce the same summary.
- The `session` parameter is stored for future caching integration and has no current effect on results.
