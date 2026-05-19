# ephemeral_demo.rs

**File:** `examples/ephemeral_demo.rs`

Demonstrates `bdslib::vm::ephemeral::WorkerPool` — a pool of Bund VM workers where each job runs in a completely fresh VM instance, guaranteeing zero cross-job state leakage.

## What it demonstrates

| Method / Type | Description |
|---|---|
| `WorkerPool::start(n)` | Initialise the ephemeral worker pool with `n` workers; independent from `BundWorkerPool` |
| `submit_ephemeral(script)` | Submit a Bund script; returns a UUIDv7 job handle; VM is discarded after the job completes |
| `results(id)` | Poll the global `RESULTS` queue for values produced by job `id` |
| `EPHEMERAL_PIPE` | The crossbeam channel that backs this pool; separate from `WORKERS_PIPE` |

## Sections

| # | Topic | Behaviour shown |
|---|---|---|
| 1 | Pool creation | `WorkerPool::start(4)` — 4 ephemeral workers, independent of any `BundWorkerPool` |
| 2 | Arithmetic result | Submit `"2 3 + ."` → poll results until `json(5)` appears |
| 3 | VM isolation | job1 defines `:square { dup * } register  9 square .` → `81`; job2 runs `"7 7 * ."` → `49` in its own fresh VM; the word `square` is absent from job2 |
| 4 | String result | Submit a string literal script; poll for the expected `json` string value |
| 5 | List result | Submit a list literal script; poll for the expected `json` array value |
| 6 | Concurrent submissions | 8 threads each submit a script pushing `1..=8`; all 8 results collected without loss |

## Run

```bash
cargo run --example ephemeral_demo
```
