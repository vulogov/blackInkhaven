# v2/rca.templates

Run a Root Cause Analysis (RCA) over drain3 log-template observations stored in the shard template store. Template events are grouped into co-occurrence clusters using Jaccard-similarity-based union-find, and — when a specific failure template body is named — each co-occurring template body is ranked by how consistently it precedes that failure, yielding a list of probable root causes.

Unlike `v2/rca`, which operates on raw primary records keyed by `"key"`, this method operates on drain3 template bodies (e.g., `"user <*> logged in from <*>"`). Each time drain3 stores or updates a template in a shard's tplstorage, the triggering log entry's timestamp is recorded in FrequencyTracking. This accumulated history of template observations is what `v2/rca.templates` analyses.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Accepted and logged; reserved for future result caching. |
| `duration` | string | yes | — | Lookback window from now in humantime format, e.g. `"1h"`, `"30min"`, `"7days"`. Only template events whose observation timestamp falls in `[now − duration, now)` are analysed. |
| `failure_body` | string | no | `null` | When provided, rank all co-occurring template bodies by how far in advance they precede this body's events. Must be the exact drain3 template string, including `<*>` wildcards. Omit (or pass `null`) to run clustering only. |
| `bucket_secs` | integer | no | `300` | Width in seconds of the non-overlapping time buckets used for co-occurrence counting. Template events within the same bucket are considered to have co-occurred. |
| `min_support` | integer | no | `2` | Minimum number of distinct time buckets a template body must appear in to be eligible for analysis. Bodies below this threshold are skipped before the co-occurrence matrix is built. |
| `jaccard_threshold` | number | no | `0.2` | Minimum Jaccard similarity between two template bodies for them to be placed in the same cluster. Lower values produce larger, looser clusters. Range `[0, 1]`. |
| `max_keys` | integer | no | `200` | Upper bound on distinct template bodies to analyse. Bodies are ranked by total observation count (most frequent first) before the cap is applied. |

## Response

```json
{
  "failure_body": "service <*> crashed with exit code <*>",
  "start": 1745000000,
  "end": 1745003600,
  "n_events": 15,
  "n_keys": 4,
  "clusters": [
    {
      "id": 0,
      "members": [
        "disk <*> usage <*>% warning threshold reached",
        "disk <*> write error ENOSPC",
        "service <*> crashed with exit code <*>"
      ],
      "support": 3,
      "cohesion": 1.0
    },
    {
      "id": 1,
      "members": [
        "session opened for user <*> by service <*>",
        "user <*> logged in from <*>"
      ],
      "support": 3,
      "cohesion": 1.0
    }
  ],
  "probable_causes": [
    {
      "body": "disk <*> usage <*>% warning threshold reached",
      "co_occurrence_count": 3,
      "jaccard": 1.0,
      "avg_lead_secs": 120.0
    },
    {
      "body": "disk <*> write error ENOSPC",
      "co_occurrence_count": 3,
      "jaccard": 1.0,
      "avg_lead_secs": 60.0
    }
  ]
}
```

### Top-level fields

| Field | Type | Description |
|---|---|---|
| `failure_body` | string \| null | The failure template body supplied to the request, or `null` when none was provided. |
| `start` | integer | Unix seconds of the earliest template event timestamp seen in the analysis window. |
| `end` | integer | Unix seconds of the latest template event timestamp seen in the analysis window. |
| `n_events` | integer | Total template events analysed across all eligible bodies. |
| `n_keys` | integer | Number of distinct template bodies after support thresholding. |
| `clusters` | array | Co-occurrence clusters, sorted by `cohesion` descending, then `support` descending. |
| `probable_causes` | array | Probable root-cause template bodies ranked by `avg_lead_secs` descending. Empty when `failure_body` was not given or was not observed in the window. |

### `clusters[]` fields

| Field | Type | Description |
|---|---|---|
| `id` | integer | Sequential cluster id assigned after sorting (0 = highest cohesion). |
| `members` | array of string | Template body strings belonging to this cluster, sorted alphabetically. |
| `support` | integer | Minimum bucket-frequency among all members — how many distinct time buckets the whole cluster is simultaneously visible in. |
| `cohesion` | number | Average pairwise Jaccard similarity among members. `1.0` means every member always appears in exactly the same buckets. |

### `probable_causes[]` fields

| Field | Type | Description |
|---|---|---|
| `body` | string | Template body of this candidate. |
| `co_occurrence_count` | integer | Number of individual template events that share a time bucket with a failure event. |
| `jaccard` | number | Jaccard similarity between this body and the failure body over their bucket sets. |
| `avg_lead_secs` | number | Mean seconds by which this body's earliest event in each shared bucket precedes the earliest failure event in that bucket. Positive = fires before the failure (causal signal). Negative = fires after the failure (consequence). |

## Example — clustering only

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/rca.templates",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "duration": "2h"
    },
    "id": 1
  }' | jq
```

## Example — with failure body

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/rca.templates",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "duration": "2h",
      "failure_body": "service <*> crashed with exit code <*>",
      "bucket_secs": 300,
      "jaccard_threshold": 0.3
    },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32004` | RCA query failed (shard error or analysis error) |
| `-32600` | Invalid `duration` string |

## Notes

- **Template events vs. raw records.** This method reads from the per-shard `tplstorage` FrequencyTracking tables, not from the main observability store. Template events are created by the drain3 miner whenever a new log line causes a template to be stored or updated.
- **Co-occurrence bucketing.** A template body is counted at most once per time bucket even if multiple drain events with that body fall within it, preventing high-frequency bodies from dominating Jaccard scores.
- **`failure_body` format.** The failure body must match the exact drain3 template string stored in tplstorage, including `<*>` wildcard tokens. Use `v2/tpl.templates_recent` to discover available template bodies.
- **Empty window.** When no template events exist in the window, `n_events` and `n_keys` are `0`, `clusters` and `probable_causes` are empty. No error is raised.
- The `session` parameter is stored for future caching integration and has no current effect on results.
