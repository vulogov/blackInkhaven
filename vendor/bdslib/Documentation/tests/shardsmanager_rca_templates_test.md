# shardsmanager_rca_templates_test.rs

**File:** `tests/shardsmanager_rca_templates_test.rs`

Integration tests for `RcaTemplatesResult::analyze` and `RcaTemplatesResult::analyze_failure`, verifying the full G-Forest co-occurrence pipeline from template storage through clustering and causal ranking.

## Test fixture

`tmp_manager(duration)` creates a fresh `(TempDir, ShardsManager)` with the given shard duration. The embedding engine is initialised once via `OnceLock` (expensive) and shared across all tests.

`store_tpl(mgr, body, ts)` calls `mgr.tpl_add` with `{"name": body, "timestamp": ts, "type": "tpl"}` and `body.as_bytes()` as the blob, directly inserting a template event into tplstorage without going through drain3.

Template body constants used across tests:

| Constant | Body string |
|---|---|
| `AUTH` | `"user <*> logged in from <*>"` |
| `NET` | `"connection to <*> on port <*> established"` |
| `DB` | `"database query took <*> ms"` |
| `CACHE` | `"cache miss for key <*>"` |
| `FAILURE` | `"service <*> crashed with exit code <*>"` |
| `DISK` | `"disk <*> usage <*>% warning"` |
| `OOM` | `"oom killer invoked process <*> killed"` |

## Tests

### Empty window

| Test | Description |
|---|---|
| `test_analyze_empty_window` | No templates stored — `n_events=0`, `n_keys=0`, `clusters=[]`, `probable_causes=[]`. No error. |

### Cluster detection

| Test | Description |
|---|---|
| `test_two_isolated_clusters_no_cross` | Two isolated pairs (AUTH+NET in buckets 0/2/4; DB+CACHE in buckets 1/3/5) with Jaccard=1.0. Verifies two separate clusters with no cross-membership. |

### Causal ranking

| Test | Description |
|---|---|
| `test_causal_ranking_disk_leads_oom` | DISK fires 90 s before FAILURE, OOM fires 30 s before FAILURE, both in 3 shared buckets. Verifies DISK ranks above OOM (`avg_lead_secs` ordering). |
| `test_unknown_failure_body_returns_empty` | `analyze_failure` with a body not present in the window → `probable_causes=[]`, no error. |

### Metadata invariants

| Test | Description |
|---|---|
| `test_metadata_invariants` | `start ≤ end` and `n_keys ≤ n_events` always hold when at least one event exists in the window. |

### Configuration effects

| Test | Description |
|---|---|
| `test_tight_jaccard_threshold_splits_partial_overlap` | AUTH in buckets 0/2/4, NET in buckets 0/4. `jaccard_threshold=0.9` splits them into separate clusters; `jaccard_threshold=0.5` merges them. |
| `test_wide_buckets_merge_all_templates` | `bucket_secs=86400` collapses all events into one bucket; each body appears in only 1 bucket so `min_support=1` is needed. With `min_support=1`, all bodies merge into one cluster. |
| `test_min_support_filters_rare_body` | AUTH has 3 bucket appearances, RARE has 1. `min_support=2` drops RARE from analysis. |
| `test_max_keys_cap_limits_bodies` | 10 distinct bodies injected; `max_keys=3` retains only the 3 most frequent. |

### Edge cases

| Test | Description |
|---|---|
| `test_no_overlap_with_failure` | Failure body is present but shares no bucket with any other body → `probable_causes=[]`. |
| `test_events_span_multiple_shards` | `shard_duration="5min"`: AUTH events go into one shard, NET events (600 s later) into another. Verifies cross-shard event collection produces `n_events ≥ 4`. |
| `test_invalid_duration_returns_error` | `analyze` with duration `"not-a-duration"` returns an error. |
| `test_failure_body_field_set_in_result` | `failure_body` field in the result matches the input string exactly; `None` when `analyze` (not `analyze_failure`) is called. |
