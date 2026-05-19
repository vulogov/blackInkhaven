# v2/rca

Run a Root Cause Analysis (RCA) over non-telemetry event records in a lookback window. Events are grouped into co-occurrence clusters using Jaccard-similarity-based union-find, and — when a specific failure key is named — each co-occurring event key is ranked by how consistently it precedes that failure, yielding a list of probable root causes.

Telemetry records are excluded automatically: a record is considered telemetry when its `data` field is a bare JSON number, or when `data["value"]` is a JSON number. Everything else is treated as a discrete event.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Accepted and logged; reserved for future result caching. |
| `duration` | string | yes | — | Lookback window from now in humantime format, e.g. `"1h"`, `"30min"`, `"7days"`. Only shards whose time interval overlaps `[now − duration, now)` are queried. |
| `failure_key` | string | no | `null` | When provided, rank all co-occurring event keys by how far in advance they precede this key's events. Omit (or pass `null`) to run clustering only without causal ranking. |
| `bucket_secs` | integer | no | `300` | Width in seconds of the non-overlapping time buckets used for co-occurrence counting. Events within the same bucket are considered to have co-occurred. |
| `min_support` | integer | no | `2` | Minimum number of distinct time buckets a key must appear in to be eligible for analysis. Keys below this threshold are skipped before the co-occurrence matrix is built. |
| `jaccard_threshold` | number | no | `0.2` | Minimum Jaccard similarity between two keys for them to be placed in the same cluster. Lower values produce larger, looser clusters; higher values produce tighter, smaller ones. Range `[0, 1]`. |
| `max_keys` | integer | no | `200` | Upper bound on distinct event keys to analyse. Keys are ranked by total primary-record count (most frequent first) before the cap is applied. |

## Response

```json
{
  "failure_key": "app_crash",
  "start": 1745000000,
  "end": 1745003600,
  "n_events": 847,
  "n_keys": 12,
  "clusters": [
    {
      "id": 0,
      "members": ["disk_full", "oom_killer"],
      "support": 5,
      "cohesion": 0.83
    },
    {
      "id": 1,
      "members": ["nginx.error", "postgres.deadlock"],
      "support": 7,
      "cohesion": 0.67
    }
  ],
  "probable_causes": [
    {
      "key": "disk_full",
      "co_occurrence_count": 5,
      "jaccard": 0.71,
      "avg_lead_secs": 142.4
    },
    {
      "key": "oom_killer",
      "co_occurrence_count": 5,
      "jaccard": 0.62,
      "avg_lead_secs": 38.1
    }
  ]
}
```

### Top-level fields

| Field | Type | Description |
|---|---|---|
| `failure_key` | string \| null | The failure key supplied to the request, or `null` when none was provided. |
| `start` | integer | Unix seconds of the earliest event timestamp seen in the analysis window. |
| `end` | integer | Unix seconds of the latest event timestamp seen in the analysis window. |
| `n_events` | integer | Total non-telemetry primary records analysed across all eligible keys. |
| `n_keys` | integer | Number of distinct event keys after telemetry filtering and support thresholding. |
| `clusters` | array | Co-occurrence clusters, sorted by `cohesion` descending, then `support` descending. |
| `probable_causes` | array | Probable root-cause candidates ranked by `avg_lead_secs` descending. Empty when `failure_key` was not given or was not observed in the window. |

### `clusters[]` fields

| Field | Type | Description |
|---|---|---|
| `id` | integer | Sequential cluster id assigned after sorting (0 = highest cohesion). |
| `members` | array of string | Event keys belonging to this cluster, sorted alphabetically. |
| `support` | integer | Minimum bucket-frequency among all members — how many distinct time buckets the whole cluster is visible in. |
| `cohesion` | number | Average pairwise Jaccard similarity among members. `1.0` means every member always appears in exactly the same buckets; `0.0` means no two members ever appear together. |

### `probable_causes[]` fields

| Field | Type | Description |
|---|---|---|
| `key` | string | Event key of this candidate. |
| `co_occurrence_count` | integer | Number of individual events (not buckets) that share a time bucket with a failure event. |
| `jaccard` | number | Jaccard similarity between this key and the failure key over their bucket sets. |
| `avg_lead_secs` | number | Mean seconds by which this key's events precede the earliest failure event in the same bucket. Positive = fires before the failure (causal signal). Negative = fires after the failure (consequence or correlated side-effect). |

## Example — clustering only

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/rca",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "duration": "3h"
    },
    "id": 1
  }' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": {
    "failure_key": null,
    "start": 1745000000,
    "end": 1745010800,
    "n_events": 412,
    "n_keys": 8,
    "clusters": [
      {
        "id": 0,
        "members": ["auditd", "sshd"],
        "support": 6,
        "cohesion": 1.0
      },
      {
        "id": 1,
        "members": ["nginx", "postgres"],
        "support": 6,
        "cohesion": 1.0
      }
    ],
    "probable_causes": []
  },
  "id": 1
}
```

## Example — with failure key and custom tuning

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/rca",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "duration": "3h",
      "failure_key": "app_crash",
      "bucket_secs": 60,
      "jaccard_threshold": 0.3
    },
    "id": 1
  }' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": {
    "failure_key": "app_crash",
    "start": 1745000000,
    "end": 1745010800,
    "n_events": 412,
    "n_keys": 8,
    "clusters": [
      {
        "id": 0,
        "members": ["disk_full", "oom_killer", "app_crash"],
        "support": 5,
        "cohesion": 0.78
      }
    ],
    "probable_causes": [
      {
        "key": "disk_full",
        "co_occurrence_count": 5,
        "jaccard": 0.71,
        "avg_lead_secs": 142.4
      },
      {
        "key": "oom_killer",
        "co_occurrence_count": 5,
        "jaccard": 0.62,
        "avg_lead_secs": 38.1
      }
    ]
  },
  "id": 1
}
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32004` | RCA query failed (DB unavailable, shard error, or analysis error) |
| `-32600` | Invalid `duration` string |

## Notes

- **Telemetry exclusion.** Records are excluded when `data` is a bare JSON number or when `data["value"]` is a JSON number. Only discrete event records participate in clustering and causal ranking.
- **Co-occurrence bucketing.** A key is counted at most once per time bucket even if multiple events with that key fall within it. This prevents high-frequency keys from dominating the Jaccard scores.
- **Jaccard similarity.** `J(A, B) = |A ∩ B| / |A ∪ B|` where the sets are the bucket-ids each key appears in. Keys that always fire together score 1.0; keys that never share a bucket score 0.0.
- **Causal lead time.** For each shared bucket, `avg_lead_secs` uses the earliest failure timestamp in that bucket as the reference. A candidate event that fires 90 seconds before the first failure event in its bucket contributes `+90` to the accumulator. Averaging across all shared buckets yields the final score.
- **Empty window.** When no non-telemetry primary records exist in the window, `n_events` and `n_keys` are `0`, `clusters` is empty, and `probable_causes` is empty. No error is raised.
- **`failure_key` not observed.** If `failure_key` is provided but no records with that key exist in the window, `probable_causes` is empty and no error is raised.
- **Key frequency cap.** When the number of distinct event keys exceeds `max_keys`, the least frequent keys are dropped. The most frequent keys are retained so the strongest signals are always included.
- The `session` parameter is stored for future caching integration and has no current effect on results.
