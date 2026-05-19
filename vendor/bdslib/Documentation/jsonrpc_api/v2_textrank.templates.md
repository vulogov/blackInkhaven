# v2/textrank.templates

Build an extractive summary of every drain3 template observed in the lookback window using the TextRank algorithm.

For each template observed in `[now − duration, now]`, bdsnode constructs a JSON object combining the template's `metadata` and `body`, runs that object through `bdslib::common::jsonfingerprint::json_fingerprint` to flatten it into a single string, and feeds the resulting list of strings to `bdslib::analysis::textrank::textrank_summary_with`. The response carries the joined summary string — the highest-ranked fingerprints in their original order, separated by spaces.

Use this when you need a one-glance picture of "what the templates in this window are about" — for dashboard headers, alert previews, or as the seed text for a follow-up LLM prompt.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUIDv7 session identifier. Accepted and logged; reserved for future result caching. |
| `duration` | string | yes | — | Lookback window from now in humantime format, e.g. `"30min"`, `"1h"`, `"7days"`. Only templates whose FrequencyTracking observation falls in `[now − duration, now]` are summarised. |
| `max_sentences` | integer | no | `0` | Hard cap on the number of fingerprints kept in the summary. Set to `0` to derive the cap from `ratio` instead. |
| `ratio` | number | no | `0.3` | When `max_sentences == 0`, this fraction of the input fingerprints is kept (rounded up, minimum 1). Clamped to `(0.0, 1.0]`. |
| `min_word_len` | integer | no | `2` | Tokens shorter than this many characters are dropped before scoring. |
| `damping` | number | no | `0.85` | PageRank damping factor. Standard TextRank value. |
| `iters` | integer | no | `30` | Maximum PageRank iterations. The loop also exits early once the L1-norm change between iterations drops below `tolerance`. |
| `tolerance` | number | no | `1e-4` | Early-exit threshold on the per-iteration L1-norm change in scores. |

## Response

```json
{
  "duration": "1h",
  "max_sentences": 3,
  "ratio": 0.3,
  "summary": "level: error code: 503 body: upstream timeout service: <*> level: error code: 503 body: upstream timeout service: <*> level: warn code: 429 body: rate limit exceeded service: <*>"
}
```

| Field | Type | Description |
|---|---|---|
| `duration` | string | Lookback window string echoed from the request. |
| `max_sentences` | integer | Cap echoed from the request (`0` means auto-sized via `ratio`). |
| `ratio` | number | Ratio echoed from the request (effective when `max_sentences == 0`). |
| `summary` | string | TextRank summary built from the highest-ranked template fingerprints in their original observation order. Empty string when no templates were observed in the window. |

## Algorithm

1. Call `ShardsManager::templates_by_timestamp(now − duration, now)` to fetch every observed template (`{id, metadata, body}`).
2. For each template, merge the metadata object with the body string under a dedicated `"body"` key and run the merged object through `json_fingerprint`. The result is a flat `path: value` string per template.
3. Tokenise each fingerprint, build a cosine-similarity graph between fingerprints, run weighted PageRank, and pick the top fingerprints by score.
4. Return them in their original input order joined by single spaces.

The Rust API is `ShardsManager::textrank_templates(session_id, lookback, &TextRankConfig)`; this RPC is a thin wrapper that converts the humantime string and JSON config to the typed Rust call.

## Example

Auto-sized summary (~30% of templates kept):

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/textrank.templates",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "duration": "1h"
    },
    "id": 1
  }' | jq
```

Capped summary with custom tuning:

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/textrank.templates",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "duration": "6h",
      "max_sentences": 5,
      "min_word_len": 3,
      "damping": 0.85
    },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32004` | Template query or summarisation failed |
| `-32600` | Invalid `duration` string |

## Notes

- **Empty window.** When no templates were observed in the window, `summary` is the empty string. No error is raised.
- **Single template.** A window with exactly one template returns that template's fingerprint verbatim — TextRank degenerates gracefully on inputs of length one.
- **Stop-words.** TextRank uses a small embedded English stop-word list; if every fingerprint is composed entirely of stop-words and short tokens, the summariser falls back to the first fingerprint to guarantee a non-empty result.
- **Determinism.** The algorithm is fully deterministic — same inputs always produce the same summary.
- **Source method.** Templates are fetched via `templates_by_timestamp`, which queries only shards overlapping the window — observation rows always live in the current-time shard, so old shards are skipped automatically.
- The `session` parameter is stored for future caching integration and has no current effect on results.
