# vm_ephemeral_test.rs

**File:** `tests/vm_ephemeral_test.rs`  
**Module:** `bdslib::vm::ephemeral` — WorkerPool, Worker, submit_ephemeral

## Test functions

| Test | What it verifies |
|---|---|
| `pool_has_correct_worker_count` | `WorkerPool::start(4)` succeeds (singleton, no panic) |
| `submit_returns_a_uuid` | `submit_ephemeral` returns a non-nil UUIDv7 |
| `integer_workbench_value_reaches_results` | Submit `"99 ."` → results queue contains `json(99)` |
| `string_workbench_value_reaches_results` | Submit `"\"world\" ."` → results queue contains `json("world")` |
| `list_workbench_value_reaches_results` | Submit `"[ 10 20 30 ] ."` → results queue contains `json([10,20,30])` |
| `multiple_workbench_items_all_reach_results` | Submit `"7 . 8 . 9 ."` → 3 items appear in the results queue |
| `separate_scripts_have_isolated_results` | Two scripts get distinct UUIDs and isolated queues |
| `arithmetic_result_reaches_results` | Submit `"3 4 + ."` → results queue contains `json(7)` |
| `no_workbench_push_leaves_empty_queue` | A script without `"."` leaves an empty results queue |
| `workers_are_isolated_per_job` | job1 defines `:double { 2 * } register  5 double .` → `10`; job2 runs `"100 ."` → `100`; the word `double` is absent from job2, proving fresh VMs per job |
| `concurrent_submissions_do_not_lose_results` | 8 threads × 1 submit each — all results appear, none lost |

## Key properties verified

- **Strict VM isolation** — every job receives a completely fresh Bund VM with no inherited dictionary or stack state. Words defined in one job are invisible to all others.
- **Independent channel** — `WorkerPool` and `EPHEMERAL_PIPE` are entirely separate from `BundWorkerPool`/`WORKERS_PIPE`; the two pool types can coexist in the same process without interference.
- **Workbench-to-RESULTS bridge** — workbench values are drained and pushed to the global `RESULTS` queue via the same `dynamic_to_json` path used by `workers.rs`.
- **Singleton guard** — `WorkerPool` is initialised once via `OnceLock`; all tests share one pool of 4 workers without re-initialisation races.
- **Polling helper** — tests retry up to a bounded timeout for results to appear, decoupling assertions from worker scheduling latency.

## Run

```bash
cargo test --test vm_ephemeral_test -- --show-output
```

## Notes

The `workers_are_isolated_per_job` test is the definitive isolation check: it verifies that a named word registered in job1 does not bleed into job2, confirming that each worker allocates a new Bund VM for every submitted script. All tests share the process-wide `WorkerPool` singleton and global `RESULTS` queue; UUID-keyed results prevent cross-test collisions when tests run in parallel.
