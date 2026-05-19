# v2/topics.all

Run Latent Dirichlet Allocation (LDA) topic modelling over every distinct primary key found within a lookback window, returning one [`TopicSummary`](v2_topics.md) per key.

Unlike [`v2/topics`](v2_topics.md), which targets a single named key, this method discovers all keys active in the window automatically and runs LDA across each one using the same configuration. It is suited for broad corpus exploration — understanding what themes are present across an entire telemetry stream without knowing the key names upfront.

Keys are collected from all shards that overlap the window, deduplicated, and processed in alphabetical order.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Accepted and logged; reserved for future result caching. |
| `duration` | string | yes | — | Lookback window from now in humantime format, e.g. `"1h"`, `"30min"`, `"7days"`. Only shards whose time interval overlaps `[now − duration, now)` are queried. |
| `k` | integer | no | `3` | Number of LDA topics per key. Clamped to the per-key corpus size when there are fewer documents than `k`. |
| `alpha` | number | no | `0.1` | Dirichlet prior on document-topic distributions. Smaller values produce sparser, more focused topic assignments per document. |
| `beta` | number | no | `0.01` | Dirichlet prior on topic-word distributions. Smaller values produce sparser per-topic vocabularies. |
| `seed` | integer | no | `42` | RNG seed for reproducible Gibbs sampling. Applied identically across all keys. |
| `iters` | integer | no | `200` | Number of collapsed Gibbs sampling iterations per key. |
| `top_n` | integer | no | `10` | Number of top words to extract from each topic before merging into the per-key keyword set. |

## Response

```json
{
  "topics": [
    {
      "key": "http.request",
      "start": 1745000000,
      "end": 1745003600,
      "n_docs": 1204,
      "n_topics": 3,
      "keywords": "404, get, latency, path, post, status, timeout, url"
    },
    {
      "key": "syslog",
      "start": 1745000000,
      "end": 1745003600,
      "n_docs": 842,
      "n_topics": 3,
      "keywords": "authentication, connection, error, failed, host, session, ssh"
    }
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `topics` | array | One `TopicSummary` object per distinct key found in the window, ordered alphabetically by key. Empty array if no primary records exist in the window. |
| `topics[].key` | string | The telemetry key for this summary. |
| `topics[].start` | integer | Start of the queried window as Unix seconds (inclusive). |
| `topics[].end` | integer | End of the queried window as Unix seconds (exclusive). |
| `topics[].n_docs` | integer | Number of primary documents for this key used as LDA input. |
| `topics[].n_topics` | integer | Number of topics actually modelled (≤ `k`; clamped when the corpus is small). |
| `topics[].keywords` | string | Alphabetically sorted, comma-separated keyword list distilled from all topics for this key. Empty string when the corpus is empty or contains no tokenisable words. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/topics.all",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "duration": "1h",
      "k": 5,
      "iters": 300
    },
    "id": 1
  }' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": {
    "topics": [
      {
        "key": "http.request",
        "start": 1745000000,
        "end": 1745003600,
        "n_docs": 1204,
        "n_topics": 5,
        "keywords": "404, get, latency, path, post, status, timeout, url, user"
      },
      {
        "key": "syslog",
        "start": 1745000000,
        "end": 1745003600,
        "n_docs": 842,
        "n_topics": 5,
        "keywords": "authentication, connection, disk, error, failed, host, session, ssh, sudo"
      }
    ]
  },
  "id": 1
}
```

## Error responses

| Code | Condition |
|---|---|
| `-32004` | LDA query failed for one of the keys (DB unavailable, shard error, or LDA training error). Processing stops at the first failure. |
| `-32600` | Invalid `duration` string |

## Notes

- **Latency scales with key count.** LDA training runs synchronously per key inside a single blocking thread. With many distinct keys and large corpora, response time can be substantial. Narrow `duration` or reduce `iters` if latency is a concern.
- **Same config across all keys.** The `k`, `alpha`, `beta`, `seed`, `iters`, and `top_n` values are used identically for every key. To tune LDA per-key, use [`v2/topics`](v2_topics.md) individually.
- **Empty-corpus keys.** Keys with no tokenisable documents produce a summary with `n_docs = 0`, `n_topics = 0`, and `keywords = ""`. No error is raised for empty keys.
- **Key discovery.** Keys are discovered by scanning primary records across shards in the window. Only keys with at least one primary record appear in the output.
- The `session` parameter is stored for future caching integration and has no current effect on results.
