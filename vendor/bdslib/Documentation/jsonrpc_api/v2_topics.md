# v2/topics

Run Latent Dirichlet Allocation (LDA) topic modelling over telemetry documents for a given key within a lookback window and return a topic summary.

LDA analyses the text content of matching primary records, groups the vocabulary into `k` latent topics via collapsed Gibbs sampling, and distils a single sorted keyword list from all topics. The result is useful for understanding dominant themes in a stream of telemetry or log data over a time period.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Accepted and logged; reserved for future result caching. |
| `key` | string | yes | — | Exact telemetry key to query (e.g. `"syslog"`, `"http.request"`). Only primary records whose `key` field matches exactly are used as LDA input. |
| `duration` | string | yes | — | Lookback window from now in humantime format, e.g. `"1h"`, `"30min"`, `"7days"`. Only shards whose time interval overlaps `[now − duration, now)` are queried. |
| `k` | integer | no | `3` | Number of LDA topics. Clamped to the corpus size when there are fewer documents than `k`. |
| `alpha` | number | no | `0.1` | Dirichlet prior on document-topic distributions. Smaller values produce sparser, more focused topic assignments per document. |
| `beta` | number | no | `0.01` | Dirichlet prior on topic-word distributions. Smaller values produce sparser per-topic vocabularies. |
| `seed` | integer | no | `42` | RNG seed for reproducible Gibbs sampling runs. |
| `iters` | integer | no | `200` | Number of collapsed Gibbs sampling iterations. More iterations improve convergence at the cost of latency. |
| `top_n` | integer | no | `10` | Number of top words to extract from each topic before merging into the final keyword set. |

## Response

```json
{
  "key": "syslog",
  "start": 1745000000,
  "end": 1745003600,
  "n_docs": 842,
  "n_topics": 3,
  "keywords": "authentication, connection, disk, error, failed, host, io, memory, session, timeout"
}
```

| Field | Type | Description |
|---|---|---|
| `key` | string | The telemetry key that was queried. |
| `start` | integer | Start of the queried window as Unix seconds (inclusive). |
| `end` | integer | End of the queried window as Unix seconds (exclusive). |
| `n_docs` | integer | Number of documents used as LDA input. |
| `n_topics` | integer | Number of topics actually modelled (≤ `k`; clamped when the corpus is small). |
| `keywords` | string | Alphabetically sorted, comma-separated keyword list distilled from all topics. Each keyword appears at most once. Empty string when the corpus is empty or contains no tokenisable words. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/topics",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "key": "syslog",
      "duration": "1h",
      "k": 5,
      "iters": 300,
      "top_n": 15
    },
    "id": 1
  }' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": {
    "key": "syslog",
    "start": 1745000000,
    "end": 1745003600,
    "n_docs": 842,
    "n_topics": 5,
    "keywords": "authentication, connection, disk, error, failed, host, io, memory, session, ssh, sudo, timeout, user, write"
  },
  "id": 1
}
```

## Error responses

| Code | Condition |
|---|---|
| `-32004` | LDA query failed (DB unavailable, shard error, or LDA training error) |
| `-32600` | Invalid `duration` string |

## Notes

- **Text extraction.** Each document is converted to a plain-text string by prepending the key name (dots, underscores, and hyphens replaced by spaces) to a `json_fingerprint` of its `data` subtree. This gives LDA both field-name context and content signals from all scalar leaf values.
- **Empty corpus.** When no matching documents are found, `n_docs` and `n_topics` are `0` and `keywords` is an empty string. No error is raised.
- **Latency.** LDA training runs synchronously in a blocking thread. With large corpora (thousands of documents) and many iterations, response times can be several seconds. Reduce `iters` or narrow `duration` if latency is a concern.
- **Keyword deduplication.** Keywords that appear in multiple topics are included only once in the final list.
- **Exact key match.** Only primary records whose `key` field equals `key` exactly are included. Shell-glob or prefix matching is not supported; use [`v2/keys`](v2_keys.md) to discover available keys.
- The `session` parameter is stored for future caching integration and has no current effect on results.
