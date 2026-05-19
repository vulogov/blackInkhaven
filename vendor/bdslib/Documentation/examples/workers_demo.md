# workers_demo.rs

**File:** `examples/workers_demo.rs`

Demonstrates `bdslib::vm::workers::BundWorkerPool` — a process-wide pool of Bund VM workers that execute scripts asynchronously and deliver results to the global `RESULTS` queue.

## What it demonstrates

| Method / Type | Description |
|---|---|
| `BundWorkerPool::start(n)` | Initialise the singleton worker pool with `n` workers |
| `submit_script(script)` | Submit a Bund script string; returns a UUIDv7 job handle |
| `results(id)` | Poll the global `RESULTS` queue for values produced by job `id` |
| `cast_json()` | Extract the JSON payload from a `rust_dynamic` result value |

## Sections

| # | Topic | Behaviour shown |
|---|---|---|
| 1 | Pool creation | `BundWorkerPool::start(4)` — 4 workers ready, singleton guard prevents double-init |
| 2 | Arithmetic result | Submit `"6 7 * ."` → poll results until `json(42)` appears |
| 3 | String result | Submit `"\"hello from BUND\" ."` → poll results for `json("hello from BUND")` |
| 4 | Multiple workbench pushes | Submit `"1 . 2 . 3 ."` → 3 separate items in the results queue |
| 5 | List result | Submit `"[ 10 20 30 ] ."` → results queue contains `json([10,20,30])` |
| 6 | Named function (fibonacci) | Define and call a recursive fibonacci function; verify `fib(8) = 21` |
| 7 | Concurrent submissions | 8 threads each submit an `i²` script (0²..7²); all 8 results are collected without loss |

## Run

```bash
cargo run --example workers_demo
```
