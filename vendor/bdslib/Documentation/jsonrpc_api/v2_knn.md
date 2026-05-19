# v2/knn

Run k-Nearest-Neighbour intelligence over every primary record observed
in the lookback window.

For each primary record whose `ts` falls in `[now − duration, now)`,
bdsnode builds a fingerprint string by combining the record's `key`
(with `.`/`_`/`-` replaced by spaces) with `json_fingerprint(data)`.
The resulting strings are fed to
`bdslib::analysis::knn::knn_summary_with`, and that function's JSON
output is returned **verbatim** — see
[`Documentation/Algorithm/KNN.md`](../Algorithm/KNN.md) for the full
output shape (clusters, anomalies, density-ranked representatives).

Sibling endpoints share the same fingerprinting pipeline:

- [`v2/anomaly.recent`](v2_anomaly_recent.md) — phrase-rarity outliers
  (n-gram).
- [`v2/denoise.recent`](v2_denoise_recent.md) — n-gram noise removal.
- **`v2/knn`** — vocabulary-overlap clusters + isolated outliers.

Use `v2/knn` when you need a structured per-record verdict (which
record belongs to which cluster, which records have no useful
neighbours, which is the densest representative of each cluster).
For phrase-structure outliers prefer `v2/anomaly.recent`; for a
cleaned-up signal corpus prefer `v2/denoise.recent`.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | no | `""` | UUIDv7 transaction identifier (echoed only). |
| `duration` | string | yes | — | Lookback window in humantime (`"30min"`, `"1h"`, `"7d"`). |
| `k` | integer | no | `5` | Number of nearest neighbours per input. Clamped to `[1, n-1]` at runtime. |
| `min_word_len` | integer | no | `2` | Tokens shorter than this are dropped before TF-IDF vectorisation. |
| `anomaly_threshold` | number | no | `0.2` | Inputs whose top-1 cosine similarity is below this threshold are flagged as anomalies. Range `[0, 1]`. |
| `max_cluster_members` | integer | no | `10` | Cap on each cluster's `members` array in the response (true `size` is always reported). |
| `max_anomalies` | integer | no | `20` | Cap on the response `anomalies` array (true `n_anomalies` is always reported). |

## Fingerprinting

Identical to [`v2/anomaly.recent`](v2_anomaly_recent.md) and
[`v2/denoise.recent`](v2_denoise_recent.md) — for each primary record
`(key, data)`, the fingerprint is:

```
"<key with .  _  - → spaces>  <json_fingerprint(data)>"
```

`json_fingerprint` flattens every JSON leaf into `"field: value"`
pairs sorted by key. Records whose fingerprint is empty are silently
skipped.

## Response

Returned **verbatim** from `knn_summary_with` — see the full schema in
[KNN.md § 5](../Algorithm/KNN.md#5-output-contract). Sketch:

```json
{
  "n_logs":            120,
  "k":                 5,
  "anomaly_threshold": 0.2,
  "n_clusters":        3,
  "n_anomalies":       4,
  "clusters": [
    {
      "id":   0,
      "size": 14,
      "representative": {
        "idx":     2,
        "text":    "log app  level: error  msg: upstream timeout ...",
        "density": 0.832
      },
      "members": [
        { "idx": 2, "text": "...", "density": 0.832 },
        { "idx": 7, "text": "...", "density": 0.794 }
      ]
    }
  ],
  "anomalies": [
    {
      "idx":            31,
      "text":           "log app  msg: manual intervention required",
      "max_similarity": 0.04
    }
  ],
  "representatives": [
    { "idx": 2, "text": "...", "density": 0.832, "cluster": 0 }
  ]
}
```

Important field semantics:

- The `text` field on each cluster member, representative, and anomaly
  is the **fingerprint string** scored by k-NN — not the original
  record JSON. The `idx` is its position in the per-call fingerprint
  vector (not a stable record id). To resolve back to the original
  primary, run `v2/primaries.get` for the matching `key` within the
  same window.
- `n_clusters` and `n_anomalies` are the true totals; `members[]` per
  cluster and `anomalies[]` are bounded by `max_cluster_members` and
  `max_anomalies` respectively.
- Member arrays inside each cluster are sorted by `density` descending;
  the cluster representative is the densest member.
- The `anomalies` array is sorted by `max_similarity` ascending
  (most-isolated first).

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method":  "v2/knn",
    "params":  {
      "session":  "-",
      "duration": "1h",
      "k":        5,
      "anomaly_threshold": 0.2
    },
    "id": 1
  }' | jq
```

Extract the cluster representatives only:

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"v2/knn","params":{"duration":"1h"},"id":1}' \
  | jq '.result.representatives'
```

Count clusters vs anomalies at three different lookback windows:

```bash
for d in 15min 1h 24h; do
  curl -s -X POST http://127.0.0.1:9000 \
    -H 'Content-Type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"method\":\"v2/knn\",\"params\":{\"duration\":\"$d\"},\"id\":1}" \
    | jq "{window: \"$d\", n_clusters: .result.n_clusters, n_anomalies: .result.n_anomalies}"
done
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32004` | Shard scan or k-NN analysis failed |
| `-32600` | Invalid `duration` string |

## Notes

- **Empty window.** When no primary records were observed in the
  window the response is the documented empty shape (`n_logs=0`,
  empty arrays). No error is raised.
- **Determinism.** The underlying `knn_summary_with` is byte-for-byte
  deterministic given the same input fingerprints (sorted-key
  summation in the implementation). Re-running the same query with
  no new records produces identical JSON.
- **vs n-gram endpoints.** k-NN scores by **vocabulary overlap**
  (TF-IDF + cosine similarity); n-gram scores by **phrase
  structure** (sliding-window n-gram document frequency). The two
  often agree on the most extreme outliers but disagree on
  borderline cases — a line built from common words in an unusual
  combination is anomalous to n-gram but normal to k-NN, and vice
  versa. Run both when you want maximum coverage of "weird stuff".
- **Algorithm reference.** Full derivation, complexity analysis,
  worked examples, and edge-case behaviour live in
  [`Documentation/Algorithm/KNN.md`](../Algorithm/KNN.md).
