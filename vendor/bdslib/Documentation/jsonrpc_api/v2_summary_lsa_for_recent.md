# v2/summary_lsa_for_recent

Build an extractive LSA summary of every primary observability record observed in the lookback window.

For each primary record whose event timestamp falls in `[now − duration, now)`, bdsnode extracts a text body from `data["value"]` (preferred) or `data["raw"]` (fallback). Records whose `data` is a bare number or whose `data["value"]` is a number are silently dropped — those are numeric measurements meant for `v2/trends`, not text. The collected bodies are fed to `bdslib::analysis::lsa::lsa_summary_with` (Steinberger-Ježek 2004 LSA scoring), and the highest-ranked bodies are returned joined as a single string.

Use this as an alternative to `v2/summary_for_recent` when you want concept-space ranking (SVD-based) rather than graph-based PageRank.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUIDv7 transaction identifier. Accepted and logged; reserved for future result caching. |
| `duration` | string | yes | — | Lookback window from now in humantime format, e.g. `"30min"`, `"1h"`, `"7days"`. Only primaries whose `ts` falls in `[now − duration, now)` are considered. |
| `max_sentences` | integer | no | `0` | Hard cap on the number of bodies kept in the summary. Set to `0` to derive the cap from `ratio` instead. |
| `ratio` | number | no | `0.3` | When `max_sentences == 0`, this fraction of the input bodies is kept (rounded up, minimum 1). Clamped to `(0.0, 1.0]`. |
| `min_word_len` | integer | no | `2` | Tokens shorter than this many characters are dropped before scoring. |
| `n_concepts` | integer | no | `3` | Number of LSA concepts (singular vectors) extracted from the term-sentence matrix. Higher values capture more subtle themes at the cost of extra computation. |
| `power_iters` | integer | no | `50` | Power-iteration steps per eigenvector. 50 is sufficient for all practical input sizes. |

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
| `summary` | string | LSA summary built from the highest-ranked text bodies in their original input order. Empty string when the window contained no text-bearing primaries. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/summary_lsa_for_recent",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "duration": "1h",
      "max_sentences": 5,
      "n_concepts": 3
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

## Algorithm

1. **Body extraction** — collect non-numeric text bodies from primary records in the window.
2. **Tokenise** — lowercase alphanumeric tokens, stop-words removed, short tokens dropped.
3. **TF-IDF** — smoothed `log((N+1)/(df+1)) + 1` IDF × normalized term frequency.
4. **Centred Gram matrix** — off-diagonal cosine similarities only (diagonal = 0), ensuring isolated records map to the null space.
5. **Truncated SVD** — top-`n_concepts` eigenpairs via power iteration with Gram-Schmidt deflation.
6. **Steinberger-Ježek score** — `score[j] = √(Σₖ λₖ · v_k[j]²)`.
7. **Selection** — top-k by score, re-sorted into original window order.

## Notes

- **Empty window.** When no text-bearing primaries were observed in the window, `summary` is the empty string. No error is raised.
- **Single record.** A window with exactly one text-bearing primary returns its body verbatim.
- **Numeric exclusion is silent.** Records skipped by the body extractor are not reported back.
- **Determinism.** The algorithm is fully deterministic — same inputs always produce the same summary.
- **vs TextRank.** LSA uses concept-space SVD; TextRank uses graph-based PageRank over pairwise similarity. Both surface recurring themes; LSA is generally faster on large inputs and naturally handles multi-theme corpora via `n_concepts`.
