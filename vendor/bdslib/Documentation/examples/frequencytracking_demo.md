# frequencytracking_demo.rs

**File:** `examples/frequencytracking_demo.rs`

Demonstrates `FrequencyTracking`: recording event observations at explicit or current timestamps, then querying by ID, by exact timestamp, by time range, and by lookback duration.

## What it demonstrates

| Operation | Description |
|---|---|
| `FrequencyTracking::new(path, pool_size)` | Open or create a persistent store; `":memory:"` for in-process |
| `add(id)` | Record an observation of `id` at wall-clock now |
| `add_with_timestamp(ts, id)` | Record an observation at an explicit Unix-seconds timestamp |
| `by_id(id)` | All timestamps (ascending) at which `id` was ever recorded |
| `by_timestamp(ts)` | Distinct IDs observed at one exact second |
| `time_range(start, end)` | Distinct IDs with at least one observation in `[start, end]` |
| `recent(duration)` | Distinct IDs in the window `[now − duration, now]` |
| `sync()` | Flush the DuckDB WAL to disk via `CHECKPOINT` |

## Scenario

A 10-minute window of synthetic SRE events is loaded with explicit timestamps (base = `now − 10 min`, step = 60 s):

| ID | Frequency | Pattern |
|---|---|---|
| `api.login` | 9 | Fires most minutes |
| `api.search` | 5 | Fires every other minute |
| `api.export` | 2 | Sparse (T+3, T+8) |
| `alert.cpu` | 4 | Mid-window burst including a double-fire at T+5 |
| `alert.disk` | 1 | Single event at T+7 |
| `drain.cluster.0` | 6 | Even minutes (T+0, T+2, …) |
| `drain.cluster.1` | 5 | Odd minutes (T+1, T+3, …) |

Three additional live events (`api.login`, `api.search`, `drain.cluster.0`) are inserted at the current time via `add()` to populate the short-duration `recent()` window.

## Sections

### Section 3 — `by_id`

Shows total occurrence counts and relative minute offsets for five IDs. Demonstrates that duplicate events at the same timestamp (the `alert.cpu` double-fire at T+5) each appear as a separate entry:

```
api.login    9 occurrences  ["T+0m", "T+1m", ..., "T+10m"]
alert.cpu    4 occurrences  ["T+4m", "T+5m", "T+5m", "T+6m"]
```

### Section 4 — `by_timestamp`

Probes five specific timestamps and lists the distinct IDs active at each second:

```
T+0min  →  ["api.login", "drain.cluster.0"]
T+5min  →  ["alert.cpu", "api.login", "drain.cluster.1"]
```

### Section 5 — `time_range`

Four named windows query specific sub-ranges. Demonstrates that IDs firing multiple times in a window appear exactly once (`DISTINCT`), and that boundary timestamps are inclusive:

```
[first 3 minutes]   ["api.login", "api.search", "drain.cluster.0", "drain.cluster.1"]
[alert burst T+4…6] ["alert.cpu", "api.login", "api.search", "drain.cluster.0", "drain.cluster.1"]
```

### Section 6 — `recent`

Four duration strings (`"30s"`, `"5min"`, `"15min"`, `"1h"`) demonstrate the lookback window expanding to cover progressively more historical events. The `"30s"` window contains only the three live events just recorded; wider windows progressively include earlier observations:

```
recent(30s)   →  3 IDs: ["api.login", "api.search", "drain.cluster.0"]
recent(5min)  →  7 IDs: [all seven IDs]
```

## Key concepts

**Duplicate events** — calling `add()` or `add_with_timestamp()` for the same `(id, ts)` pair always creates a new row. `by_id()` returns every occurrence, so its length equals the total event count for that ID.

**DISTINCT reads** — `by_timestamp`, `time_range`, and `recent` return each ID at most once per query regardless of how many times it fired in the window. Use `by_id` to retrieve the full per-ID timeline.

**humantime durations** — `recent()` accepts any string recognised by `humantime::parse_duration`: `"30s"`, `"5min"`, `"1h"`, `"7days"`, etc.

**Clone semantics** — all clones of a `FrequencyTracking` share the same underlying DuckDB connection pool.

## Example flow

```rust
let ft = FrequencyTracking::new(":memory:", 4)?;

// Write observations.
ft.add_with_timestamp(1_000, "api.login")?;
ft.add_with_timestamp(1_060, "api.login")?;
ft.add_with_timestamp(1_000, "api.search")?;

// All timestamps for "api.login" (ascending).
let ts = ft.by_id("api.login")?;          // [1000, 1060]

// Who was active at T=1000?
let ids = ft.by_timestamp(1_000)?;        // ["api.login", "api.search"]

// IDs in a 2-minute window.
let ids = ft.time_range(1_000, 1_120)?;   // ["api.login", "api.search"]

// IDs seen in the last hour.
let ids = ft.recent("1h")?;
```
