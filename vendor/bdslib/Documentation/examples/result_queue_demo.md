# result_queue_demo.rs

**File:** `examples/result_queue_demo.rs`

Demonstrates `bdslib::vm::result_queue::ResultQueue` — per-id FIFO queues of `rust_dynamic` values with creation timestamps and TTL-based eviction.

## What it demonstrates

| Method | Description |
|---|---|
| `ResultQueue::push(id, value)` | Append to a queue; auto-creates with fresh timestamp |
| `ResultQueue::pop(id)` | Remove and return the front value; `None` for empty/missing |
| `ResultQueue::len(id)` | Current queue length (`0` for unknown id) |
| `ResultQueue::n_queues()` | Total number of registered queues |
| `ResultQueue::ids()` | Snapshot of every registered queue UUID |
| `ResultQueue::created_at(id)` | Unix-second creation timestamp for diagnostics |
| `ResultQueue::sweep_expired(ttl_secs)` | Drop queues older than `ttl_secs`; returns eviction count |

## Sections

| # | Topic | Behaviour shown |
|---|---|---|
| 1 | Basic FIFO | 4 pushes drained in insertion order; drained queue stays tracked |
| 2 | JSON values | `Value::json(payload)` round-trips identically through `cast_json` |
| 3 | Multiple queues | Two queues isolated by id; `n_queues` and `ids` reflect population |
| 4 | TTL sweep | A queue older than the TTL is evicted; a fresh queue is retained; `ttl=0` is a no-op |

## Run

```bash
cargo run --example result_queue_demo
```
