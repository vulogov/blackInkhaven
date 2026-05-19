# v2/denoise.recent

Run n-gram noise removal over every primary record observed in the lookback window.

For each primary record whose `ts` falls in `[now − duration, now)`, bdsnode builds a fingerprint string by combining the record's `key` (with `.`/`_`/`-` replaced by spaces) with `json_fingerprint(data)`. The resulting strings are fed to `bdslib::analysis::ngram::ngram_remove_noise_with`, and that function's JSON output is returned verbatim — see [`Documentation/Algorithm/NGRAM_NOISE.md`](../Algorithm/NGRAM_NOISE.md) for the full output shape.

This is the **denoising** endpoint: separates the corpus into `kept` (signal — lines using distinctive phrases) and `removed` (noise — lines made of heavily-repeated phrases). It is the dual of [`v2/anomaly.recent`](v2_anomaly_recent.md): same fingerprinting and same n-gram pipeline, scored on the opposite axis.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | no | `""` | UUIDv7 transaction identifier (echoed only). |
| `duration` | string | yes | — | Lookback window in humantime (`"30min"`, `"1h"`, `"7d"`). |
| `n` | integer | no | `2` | N-gram length. `1` = unigrams, `2` = bigrams (default), `3` = trigrams. |
| `min_word_len` | integer | no | `2` | Tokens shorter than this are dropped before n-gram construction. |
| `noise_threshold` | number | no | `0.85` | Mean commonness above this classifies a line as noise. Range `[0, 1]`. |
| `max_kept` | integer | no | `100` | Cap on the `kept` array (sample view in input order); `n_kept` always reports the true total. |
| `max_removed` | integer | no | `100` | Cap on the `removed` array (sorted by commonness desc); `n_removed` always reports the true total. |

## Fingerprinting

Identical to [`v2/anomaly.recent`](v2_anomaly_recent.md) — for each primary record `(key, data)`, the fingerprint is:

```
"<key with .  _  - → spaces>  <json_fingerprint(data)>"
```

`json_fingerprint` flattens every JSON leaf into `"field: value"` pairs sorted by key. Records whose fingerprint is empty are silently skipped.

## Response

Returned **verbatim** from `ngram_remove_noise_with` — see the full schema in [NGRAM_NOISE.md § 5](../Algorithm/NGRAM_NOISE.md#5-output-contract). Sketch:

```json
{
  "n_logs":          120,
  "n":               2,
  "n_unique_ngrams": 543,
  "noise_threshold": 0.85,
  "n_kept":          18,
  "n_removed":       102,
  "kept": [
    { "idx": 4,  "text": "log alerts  msg: memory pressure on node5 ...", "commonness": 0.21 },
    { "idx": 17, "text": "log alerts  msg: disk failure detected ...",    "commonness": 0.18 }
  ],
  "removed": [
    { "idx": 0, "text": "monitor heartbeats  msg: heartbeat ok node1 ...", "commonness": 0.91 },
    { "idx": 1, "text": "monitor heartbeats  msg: heartbeat ok node2 ...", "commonness": 0.91 }
  ]
}
```

The `text` field is the **fingerprint string** scored at that commonness (not the original primary record). The `idx` is its position in the per-call fingerprint vector (not a stable record id).

`n_kept + n_removed == n_logs` for every output (every line lands in exactly one bucket). The `kept` array preserves input order so it can be read sequentially as the denoised corpus; the `removed` array is sorted from most-noise-like to least.

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method":  "v2/denoise.recent",
    "params":  { "session": "-", "duration": "1h", "noise_threshold": 0.5 },
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

- **Empty window.** When no primary records were observed in the window the response is the documented empty shape (`n_logs=0`, empty arrays). No error is raised.
- **Default threshold is strict.** `noise_threshold = 0.85` is intentionally conservative — it only removes lines whose n-grams are present in 85%+ of the corpus on average. Many operational streams need a lower value (0.3–0.6) to surface meaningful denoising; tune to match the actual noise commonness of your data.
- **Determinism.** Byte-for-byte reproducible given the same fingerprint inputs.
- **vs `v2/anomaly.recent`.** Same pipeline, opposite axis. A line that's anomalous to one will *survive* (be kept) by the other; a line removed by this endpoint will *not* be flagged as anomalous by the other. Use `v2/denoise.recent` as a pre-processing step to clean the corpus before downstream summarisation (`v2/summary_for_recent`, `v2/summary_lsa_for_recent`) or chat retrieval.
- **Chained workflow.** `v2/denoise.recent` → take the `kept` array → feed into a downstream summariser yields a summary that reflects the signal in the corpus, not the heartbeat-style noise floor.
