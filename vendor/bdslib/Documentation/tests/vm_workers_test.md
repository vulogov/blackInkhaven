# vm_workers_test.rs

**File:** `tests/vm_workers_test.rs`  
**Module:** `bdslib::vm::workers` — BundWorkerPool, BundWorker, submit_script

## Test functions

| Test | What it verifies |
|---|---|
| `pool_has_correct_worker_count` | `BundWorkerPool::start(4)` succeeds (singleton, no panic) |
| `submit_returns_a_uuid` | `submit_script` returns a non-nil UUIDv7 |
| `integer_workbench_value_reaches_results` | Submit `"99 ."` → results queue contains `json(99)` |
| `string_workbench_value_reaches_results` | Submit `"\"hello\" ."` → results queue contains `json("hello")` |
| `float_workbench_value_reaches_results` | Submit `"3.14 ."` → results queue contains `json(3.14)` |
| `bool_workbench_value_reaches_results` | Submit `"true ."` → results queue contains `json(true)` |
| `list_workbench_value_reaches_results` | Submit `"[ 1 2 3 ] ."` → results queue contains `json([1,2,3])` |
| `multiple_workbench_items_all_reach_results` | Submit `"1 . 2 . 3 ."` → 3 items appear in the results queue |
| `separate_scripts_have_isolated_results` | Two scripts get distinct UUIDs and isolated queues |
| `arithmetic_result_reaches_results` | Submit `"6 7 * ."` → results queue contains `json(42)` |
| `no_workbench_push_leaves_empty_queue` | A script without `"."` leaves an empty results queue |
| `concurrent_submissions_do_not_lose_results` | 8 threads × 1 submit each — all results appear, none lost |

## Key properties verified

- **Singleton pool** — `BundWorkerPool` is initialised via a `OnceLock` guard; all tests share one pool of 4 workers. Repeated calls to `start` do not panic.
- **Natural least-busy dispatch** — workers share a single crossbeam channel; idle workers compete for jobs, providing implicit load balancing without a dedicated scheduler.
- **Ephemeral per-job VMs** — each job receives a fresh Bund VM; there is no cross-job state leakage.
- **Workbench-to-RESULTS bridge** — workbench values are converted to JSON via `dynamic_to_json` and pushed to the global `RESULTS` queue keyed by the job UUID.
- **Polling helper** — tests use a polling helper that retries up to 5–10 s for results to appear, decoupling test timing from worker scheduling latency.

## Run

```bash
cargo test --test vm_workers_test -- --show-output
```

## Notes

All tests share the process-wide `BundWorkerPool` singleton and the global `RESULTS` queue. Jobs are identified by UUIDv7 so concurrent tests do not collide. The concurrent-submissions test (`concurrent_submissions_do_not_lose_results`) spawns 8 threads simultaneously and verifies that every submitted result is retrievable, confirming thread-safe queue access under contention.
