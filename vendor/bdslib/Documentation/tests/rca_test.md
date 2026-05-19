# rca_test.rs

**File:** `tests/rca_test.rs`  
**Module:** `bdslib::analysis::rca` — root cause analysis

Tests event co-occurrence clustering and causal ranking using the `RcaResult` API.

## Test function

### `test_rca_lifecycle`

A single comprehensive test covering the full RCA lifecycle:

| Step | Description |
|---|---|
| 1 | Query before `init_db` returns `Err("not initialized")` |
| 2 | `init_db()` succeeds |
| 3 | Empty window: `n_events=0`, `n_keys=0`, no clusters, no causes |
| 4 | Telemetry filtering: CPU and memory metric records are excluded from clustering |
| 5 | Two-cluster detection: `{sshd, auditd}` and `{nginx, postgres}` each have Jaccard=1.0 |
| 6 | Causal ranking: `disk_full` (lead≈90s) ranks above `oom_killer` (lead≈30s) before `app_crash` |
| 7 | Tight threshold (1.0): only perfectly co-occurring keys form clusters |
| 8 | Wide buckets (86400s): all events in one bucket → larger clusters |
| 9 | Unknown failure key returns empty `probable_causes` |
| 10 | Result invariants: `start ≤ end`, no `failure_key` in `analyze`, cluster IDs sequential, sorted by cohesion descending |

## Dataset design

The test ingests events structured to produce deterministic cluster and causation results:

| Keys | Relationship | Expected outcome |
|---|---|---|
| `sshd`, `auditd` | Always co-occur in same bucket | Cluster with cohesion=1.0 |
| `nginx`, `postgres` | Co-occur in separate buckets | Cluster with cohesion=1.0 |
| `disk_full`, `nfs_timeout`, `app_error`, `app_crash` | Sequential cascade with 90s / 60s / 30s lead times | Ranked probable causes of `app_crash` |
| `cpu.usage`, `memory.rss` | Numeric telemetry | Excluded from all clusters |

## Key properties verified

- **Telemetry exclusion** — records where `data` is numeric are never clustered
- **Jaccard threshold** — `jaccard_threshold=0.2` (default) merges keys that share at least 20% of their time buckets
- **Causal ranking** — causes are sorted by `lead_secs` descending (earlier leads = stronger causes)
- **Empty-window safety** — no panic or `Err` when there are no events
- **Unknown failure key** — `analyze_failure("nonexistent")` returns `probable_causes=[]`
- **Result invariants** — `start ≤ end`, cluster IDs start at 0 and are sequential

## Notes

Like other singleton-dependent tests, this uses a single `#[test]` function because the underlying `ShardsManager` cannot be reset between test runs.
