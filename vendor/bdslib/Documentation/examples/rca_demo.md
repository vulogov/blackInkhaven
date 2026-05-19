# rca_demo.rs

**File:** `examples/rca_demo.rs`

Demonstrates `RcaResult`: root cause analysis using event co-occurrence clustering and causal ranking by lead time.

## What it demonstrates

| Method | Description |
|---|---|
| `RcaResult::analyze(duration, config)` | Cluster all non-telemetry events in the window |
| `RcaResult::analyze_failure(key, duration, config)` | Cluster events and rank probable causes of a named failure key |

## Dataset structure

The demo ingests 124 records designed to produce clear cluster structure:

| Cluster | Keys | Pattern |
|---|---|---|
| Auth | `sshd`, `pam`, `auditd` | Always co-occur in the same 5-minute bucket |
| Web | `nginx`, `haproxy` | Co-occur in separate buckets |
| Database | `postgres`, `redis` | Co-occur; postgres leads redis by ~90s |
| Failure chain | `disk_warn` → `disk_full` → `nfs_timeout` → `app_error` → `app_crash` | Sequential cascade |
| Noise | `cpu.usage`, `memory.rss` | Numeric telemetry — excluded from RCA |

14 telemetry records are included to demonstrate automatic filtering.

## RcaConfig

| Field | Default | Description |
|---|---|---|
| `bucket_secs` | 300 | Time bucket size for co-occurrence calculation |
| `min_support` | 2 | Minimum number of co-occurrences to form a cluster |
| `jaccard_threshold` | 0.2 | Minimum Jaccard similarity to merge into a cluster |
| `max_keys` | 200 | Maximum number of distinct keys to analyze |

## Output sections

1. **`analyze`** — all clusters with keys, cohesion score, and co-occurrence counts (displayed via `comfy-table`)
2. **`analyze_failure("app_crash")`** — same clusters plus `probable_causes` ranked by lead time before the failure key

## Example output

```
┌──────────────┬────────────────────────────┬──────────┐
│ cluster_id   │ keys                       │ cohesion │
├──────────────┼────────────────────────────┼──────────┤
│ 0            │ sshd, pam, auditd          │ 1.00     │
│ 1            │ nginx, haproxy             │ 1.00     │
│ 2            │ postgres, redis            │ 0.80     │
└──────────────┴────────────────────────────┴──────────┘

probable causes of "app_crash":
  1. disk_full    lead_secs=90
  2. nfs_timeout  lead_secs=60
  3. app_error    lead_secs=30
```
