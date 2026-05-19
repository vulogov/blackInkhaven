# globals_demo.rs

**File:** `examples/globals_demo.rs`

Demonstrates the process-wide database singleton: `init_db`, `get_db`, and `sync_db`.

## What it demonstrates

| Function | Description |
|---|---|
| `init_db(path)` | Initialize the global `ShardsManager` from an hjson config file |
| `get_db()` | Retrieve a reference to the initialized `ShardsManager` |
| `sync_db()` | Flush all shard caches to disk (checkpoint) |

## Sections in the demo

1. **Error before init** — `get_db()` returns an error before `init_db` is called
2. **Double-init guard** — a second `init_db` returns an "already initialized" error
3. **Batched ingestion** — 50 mixed telemetry+log records ingested via `add_batch`
4. **IoT batch** — 20 additional records from a templated generator
5. **FTS search** — `search_fts("error", "1h", 5)` across all shards
6. **Vector search** — `search_vector("connection timeout", "1h", 5)` across all shards
7. **sync** — explicit `sync_db()` call before exit
8. **Singleton proof** — helper that calls `get_db()` from a separate context and confirms same instance

## Key concepts

**OnceLock singleton** — the global `ShardsManager` is stored in a `OnceLock`. It is initialized once per process and then shared immutably by all callers via cloning the inner `Arc`.

**`BDS_CONFIG` fallback** — `init_db(None)` reads the path from the `BDS_CONFIG` environment variable. `init_db(Some(path))` overrides this.

**`sync_db()` safety** — calling `sync_db()` before init is a no-op (returns `Ok(())`), making it safe to call unconditionally in shutdown handlers.

## Example output

```
get_db before init: Err(not initialized)
init_db: Ok
double init: Err(already initialized)
ingested 70 records
fts results: 3 matches for "error"
vector results: 5 matches for "connection timeout"
sync: Ok
```
