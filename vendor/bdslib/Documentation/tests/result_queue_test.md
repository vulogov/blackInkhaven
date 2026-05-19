# result_queue_test.rs

**File:** `tests/result_queue_test.rs`  
**Module:** `bdslib::vm::result_queue` — per-id FIFO queues of `rust_dynamic` values

## Test functions

| Test | What it verifies |
|---|---|
| `pop_on_unknown_id_returns_none` | `pop` on an unregistered id returns `None`, no panic |
| `len_on_unknown_id_returns_zero` | `len` on an unregistered id returns `0`, no panic |
| `push_then_pop_returns_same_value` | Round-trip: pushed value pops out unchanged |
| `push_creates_queue_and_stamps_timestamp` | First `push` creates the queue and stamps `created_at ≈ now` |
| `fifo_order_preserved_within_queue` | Five sequential pushes pop in the same order; extra pop returns `None` |
| `separate_ids_keep_separate_queues` | Different ids are isolated; `n_queues` reflects the count |
| `empty_queue_is_kept_until_swept` | A drained queue is still tracked; `created_at` persists |
| `n_queues_and_ids_track_population` | `n_queues` + `ids` agree with the registered set |
| `sweep_with_zero_ttl_is_noop` | `sweep_expired(0)` does nothing |
| `sweep_keeps_fresh_queues` | Queues younger than TTL are retained |
| `sweep_evicts_aged_queues` | A queue older than TTL is evicted (~2.2s sleep + `ttl=1`) |
| `sweep_only_evicts_expired_queues_not_fresh_ones` | Selective eviction: aged queue gone, fresh queue retained |
| `concurrent_pushers_do_not_lose_values` | 8 threads × 100 pushes — no losses, drains the expected count |
| `json_typed_value_round_trips` | `Value::json(payload)` round-trips identically through `cast_json` |

## Key properties verified

- **Atomic per-queue state** — pushes from concurrent threads do not lose values.
- **TTL semantics** — eviction is opportunistic (caller-driven via `sweep_expired`); fresh queues are never touched.
- **No surprise eviction on drain** — a drained queue is kept until the sweeper runs, so subsequent pushes share the same TTL window.
- **Timestamp stability** — `created_at` is set once on first push and never refreshed by subsequent pushes (verified indirectly by the sweep tests).

## Run

```bash
cargo test --test result_queue_test -- --show-output
```

## Notes

These tests do not depend on the global `ShardsManager`; each test instantiates its own `ResultQueue`. Tests run in parallel safely. The two sweep tests sleep ~2.2 s each so they're the slowest in the suite (~5 s total).
