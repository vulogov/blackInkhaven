# v2/anomaly.recent

Run n-gram anomaly detection over every primary record observed in the lookback window.

For each primary record whose `ts` falls in `[now − duration, now)`, bdsnode builds a fingerprint string by combining the record's `key` (with `.`/`_`/`-` replaced by spaces) with `json_fingerprint(data)`. The resulting strings are fed to `bdslib::analysis::ngram::ngram_anomaly_with`, and that function's JSON output is returned verbatim — see [`Documentation/Algorithm/NGRAM_ANOMALY.md`](../Algorithm/NGRAM_ANOMALY.md) for the full output shape.

This is the **phrase-structure** anomaly detector, complementary to `v2/summary_for_recent` (extractive ranking) and to a `v2/search`-based outlier scan (vocabulary similarity). Use it when you want to surface lines that use *unusual phrases* — combinations of common words that don't typically occur together.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | no | `""` | UUIDv7 transaction identifier (echoed only). |
| `duration` | string | yes | — | Lookback window in humantime (`"30min"`, `"1h"`, `"7d"`). |
| `n` | integer | no | `2` | N-gram length. `1` = unigrams, `2` = bigrams (default), `3` = trigrams. |
| `min_word_len` | integer | no | `2` | Tokens shorter than this are dropped before n-gram construction. |
| `anomaly_threshold` | number | no | `0.7` | Mean rarity above this flags a line as anomalous. Range `[0, 1]`. |
| `max_anomalies` | integer | no | `20` | Cap on the `anomalies` array; `n_anomalies` always reports the true total. |
| `max_novel_ngrams` | integer | no | `5` | Per-anomaly cap on the explanatory `novel_ngrams` array. |

## Fingerprinting

For each primary record `(key, data)`, the fingerprint is:

```
"<key with .  _  - → spaces>  <json_fingerprint(data)>"
```

`json_fingerprint` flattens every JSON leaf into `"field: value"` pairs sorted by key, so the n-gram analyser sees both schema (field names like `value`, `raw`, `severity`) and content (actual values) as ordinary tokens. Records whose fingerprint is empty (e.g. a record whose `data` flattens to nothing meaningful) are silently skipped.

## Response

Returned **verbatim** from `ngram_anomaly_with` — see the full schema in [NGRAM_ANOMALY.md § 5](../Algorithm/NGRAM_ANOMALY.md#5-output-contract). Sketch:

```json
{
  "n_logs":            120,
  "n":                 2,
  "n_unique_ngrams":   543,
  "anomaly_threshold": 0.7,
  "n_anomalies":       7,
  "mean_rarity":       0.41,
  "anomalies": [
    {
      "idx":          84,
      "text":         "log app  level: error  msg: manual intervention required ...",
      "rarity":       0.93,
      "novel_ngrams": ["manual intervention", "intervention required", "stuck queue"]
    }
  ]
}
```

The `text` field is the **fingerprint string** that scored as anomalous (not the original primary record). The `idx` is its position in the per-call fingerprint vector (not a stable record id). To resolve back to a primary record, run `v2/primaries.get` with the matching `key` and timestamp range.

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method":  "v2/anomaly.recent",
    "params":  { "session": "-", "duration": "1h", "anomaly_threshold": 0.6 },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32004` | Shard scan or n-gram analysis failed |
| `-32600` | Invalid `duration` string |

## Notes

- **Empty window.** When no primary records were observed in the window the response is the documented empty shape (`n_logs=0`, empty `anomalies`). No error is raised.
- **Determinism.** The underlying `ngram_anomaly_with` is byte-for-byte deterministic given the same input fingerprints (sorted-key summation in the implementation). Re-running the same query with no new records arriving produces identical JSON.
- **vs `v2/summary_*`.** `v2/summary_*` ranks lines by *centrality* (TextRank or LSA); `v2/anomaly.recent` flags lines by *rarity*. They surface complementary signals — a centrally-ranked line is often the dominant theme; an anomalous line is the off-pattern outlier.
- **Companion endpoint.** [`v2/denoise.recent`](v2_denoise_recent.md) is the dual: same fingerprinting, scored by *commonness* instead of *rarity*, used to strip repetitive noise from the corpus.
