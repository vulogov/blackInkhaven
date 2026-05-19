# common module

The `common` module provides shared utilities used across all bdslib engines. Its sub-modules cover errors, JSON fingerprinting, math primitives, time ranges, UUID generation, and the named-channel pipe registry.

```rust
use bdslib::common::error::{Result, Error, err_msg};
use bdslib::common::jsonfingerprint::json_fingerprint;
use bdslib::common::math::{cosine_similarity, dot_product, l2_norm, normalize, euclidean_distance, squared_euclidean};
use bdslib::common::timerange::{TimeRange, minute_range, hour_range, day_range};
use bdslib::common::uuid::{generate_v7, generate_v7_at, timestamp_from_v7};

// Pipe registry — shared producer→consumer channels (bdsnode ingest pipeline).
use bdslib::pipe;   // re-exported at crate root
```

---

## common::error

Shared error and result types used by every bdslib module.

### `Result<T>`

```rust
pub type Result<T> = std::result::Result<T, easy_error::Error>;
```

All public methods in `StorageEngine`, `FTSEngine`, and `EmbeddingEngine` return this type. Using a single alias across the crate means errors propagate with `?` without any conversion between module-specific result types.

### `Error`

Re-export of `easy_error::Error`. Carries a human-readable message and an optional boxed cause:

```rust
use bdslib::common::error::Error;
```

### `err_msg`

Re-export of `easy_error::err_msg`. Constructs an `Error` from any string:

```rust
use bdslib::common::error::err_msg;

return Err(err_msg("something went wrong"));
return Err(err_msg(format!("value out of range: {val}")));
```

---

## common::jsonfingerprint

Converts any JSON value into a flat, human-readable string for use as embedding input or full-text search content. Used internally by `VectorEngine::store_document`, `VectorEngine::search_json`, and `FTSEngine` JSON indexing.

### `json_fingerprint`

```rust
pub fn json_fingerprint(json: &JsonValue) -> String
```

Recursively walks the JSON tree and emits `path: value` pairs for every leaf:

```
{ "title": "Rust", "meta": { "year": 2015, "tags": ["systems", "safe"] } }
→
"title: Rust meta.year: 2015 meta.tags[0]: systems meta.tags[1]: safe"
```

| JSON type | Output |
|---|---|
| Object | Recurse with dot-separated path prefix |
| Array | Recurse with `[i]` index appended to the path |
| String | `path: value` |
| Number / Bool | `path: value` |
| Null | Skipped (no semantic content) |
| Top-level primitive | Emitted as-is without a path prefix |

The function is also re-exported from `bdslib::vectorengine::json_fingerprint` for callers that import it alongside `VectorEngine`.

```rust
use bdslib::common::jsonfingerprint::json_fingerprint;
use serde_json::json;

let fp = json_fingerprint(&json!({
    "title": "The Rust Programming Language",
    "tags": ["systems", "memory-safety"],
    "year": 2019,
}));
// "tags[0]: systems tags[1]: memory-safety title: The Rust Programming Language year: 2019"
```

---

## common::math

Pure vector arithmetic functions. All inputs are `&[f32]`; all fallible functions return `bdslib::common::error::Result<T>`.

None of these functions allocate unless explicitly stated (i.e., `normalize`).

### `dot_product`

```rust
pub fn dot_product(a: &[f32], b: &[f32]) -> Result<f32>
```

Returns the inner product `Σ aᵢ·bᵢ`.

Returns `Err` if `a` and `b` have different lengths.

```rust
let d = dot_product(&[1.0, 2.0], &[3.0, 4.0])?; // 11.0
```

---

### `l2_norm`

```rust
pub fn l2_norm(v: &[f32]) -> f32
```

Returns the Euclidean (L2) norm `√(Σ vᵢ²)`. Returns `0.0` for an empty slice. Never returns `Err`.

```rust
let n = l2_norm(&[3.0, 4.0]); // 5.0
```

---

### `normalize`

```rust
pub fn normalize(v: &[f32]) -> Result<Vec<f32>>
```

Returns a unit-length copy of `v` (each element divided by `l2_norm(v)`).

Returns `Err` if `v` is empty or is the zero vector.

```rust
let u = normalize(&[3.0, 0.0, 4.0])?; // [0.6, 0.0, 0.8]
```

---

### `cosine_similarity`

```rust
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> Result<f32>
```

Returns the cosine similarity `dot(a,b) / (‖a‖·‖b‖)` in the range `[-1.0, 1.0]`.

Returns `Err` on dimension mismatch, empty input, or a zero-norm vector (undefined cosine).

| Score | Meaning |
|---|---|
| `1.0` | Identical direction |
| `0.0` | Orthogonal |
| `-1.0` | Opposite direction |

```rust
let sim = cosine_similarity(&e1, &e2)?;
```

This is the same computation as `EmbeddingEngine::compare_embeddings` — use this form when you don't have an `EmbeddingEngine` in scope.

---

### `euclidean_distance`

```rust
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> Result<f32>
```

Returns the Euclidean distance `√(Σ (aᵢ−bᵢ)²)`.

Returns `Err` if `a` and `b` have different lengths.

```rust
let d = euclidean_distance(&[0.0, 0.0], &[3.0, 4.0])?; // 5.0
```

---

### `squared_euclidean`

```rust
pub fn squared_euclidean(a: &[f32], b: &[f32]) -> Result<f32>
```

Returns `Σ (aᵢ−bᵢ)²` — the squared Euclidean distance. Cheaper than `euclidean_distance` when you only need to compare distances (avoids the `sqrt`).

Returns `Err` if `a` and `b` have different lengths.

```rust
let d2 = squared_euclidean(&[1.0, 2.0], &[4.0, 6.0])?; // 25.0
```

---

## common::timerange

Computes half-open `[start, end)` time intervals aligned to minute, hour, or UTC day boundaries. All functions take a [`SystemTime`](https://doc.rust-lang.org/std/time/struct.SystemTime.html) and return `Result<TimeRange>`.

### `TimeRange`

```rust
pub struct TimeRange {
    pub start: SystemTime,
    pub end: SystemTime,
}
```

A half-open interval `[start, end)`. Both fields are plain `SystemTime` values; `end - start` equals the interval duration exactly.

---

### `minute_range`

```rust
pub fn minute_range(time: SystemTime, n: u64) -> Result<TimeRange>
```

Returns the `n`-minute interval that contains `time`. The interval is floored to the nearest multiple of `n` minutes since the Unix epoch, so boundaries are stable regardless of when the function is called.

`n` must be a divisor of 60 to ensure boundaries align to the hour:

| Valid values of `n` |
|---|
| 1, 2, 3, 4, 5, 6, 10, 12, 15, 20, 30, 60 |

Returns `Err` if `n` is zero, not a divisor of 60, or `time` predates the Unix epoch.

```rust
use bdslib::common::timerange::minute_range;

// 14:07:42 UTC falls in the [14:05:00, 14:10:00) window
let r = minute_range(time, 5)?;
println!("start: {:?}, end: {:?}", r.start, r.end);
```

Passing `n = 60` produces the same result as `hour_range`.

---

### `hour_range`

```rust
pub fn hour_range(time: SystemTime) -> Result<TimeRange>
```

Returns the one-hour UTC interval that contains `time`. The interval starts at the top of the hour (`HH:00:00`) and ends at the top of the next hour.

Returns `Err` if `time` predates the Unix epoch.

```rust
use bdslib::common::timerange::hour_range;

let r = hour_range(time)?;
// r.end - r.start == 3600 seconds
```

---

### `day_range`

```rust
pub fn day_range(time: SystemTime) -> Result<TimeRange>
```

Returns the UTC calendar day that contains `time`. The interval starts at `00:00:00 UTC` and ends 86 400 seconds later at the start of the next day.

Returns `Err` if `time` predates the Unix epoch.

```rust
use bdslib::common::timerange::day_range;

let r = day_range(time)?;
// r.end - r.start == 86400 seconds
```

---

### Interval properties

All three functions share the same guarantees:

- **Containment** — `time` always satisfies `r.start <= time < r.end`.
- **Alignment** — `r.start` is always a whole multiple of the interval duration since the Unix epoch.
- **Contiguity** — passing `r.end` as `time` to the same function returns the immediately following interval (`next.start == r.end`).
- **Nesting** — a minute range is always contained within its hour range, which is always contained within its day range.

---

## common::uuid

UUIDv7 generation and timestamp extraction. UUIDv7 identifiers are 128-bit, time-ordered, and globally unique — later calls always produce greater values, making them suitable as sortable primary keys.

### `generate_v7`

```rust
pub fn generate_v7() -> Uuid
```

Generates a UUIDv7 using the current system time. Never fails.

```rust
use bdslib::common::uuid::generate_v7;

let id = generate_v7();
```

---

### `generate_v7_at`

```rust
pub fn generate_v7_at(time: SystemTime) -> Uuid
```

Generates a UUIDv7 with its timestamp set to `time`. Useful for back-filling records or creating deterministic identifiers in tests.

If `time` predates the Unix epoch, the epoch itself is used as the timestamp (no panic, no error).

```rust
use bdslib::common::uuid::generate_v7_at;
use std::time::{Duration, UNIX_EPOCH};

let historical = UNIX_EPOCH + Duration::from_secs(1_000_000_000);
let id = generate_v7_at(historical);
```

UUIDs produced from time-ordered inputs sort in the same order:

```rust
let id_old = generate_v7_at(t1); // t1 < t2
let id_new = generate_v7_at(t2);
assert!(id_old < id_new);
```

---

### `timestamp_from_v7`

```rust
pub fn timestamp_from_v7(id: Uuid) -> Option<SystemTime>
```

Extracts the embedded timestamp from a UUIDv7. Returns `None` if `id` is not a version-7 UUID or its timestamp cannot be represented as a `SystemTime`.

UUIDv7 stores timestamps with millisecond precision, so the recovered `SystemTime` may differ from the original by up to 1 ms.

```rust
use bdslib::common::uuid::{generate_v7, timestamp_from_v7};

let id = generate_v7();
if let Some(ts) = timestamp_from_v7(id) {
    println!("created at: {ts:?}");
}
```

---

## Error handling

All fallible functions across `common` sub-modules return `bdslib::common::error::Result<T>` and compose cleanly with `?`:

```rust
use bdslib::common::error::Result;
use bdslib::common::math::{cosine_similarity, normalize};
use bdslib::common::timerange::day_range;
use bdslib::common::uuid::generate_v7;
use std::time::SystemTime;

fn tag_and_score(query: &[f32], doc: &[f32]) -> Result<(uuid::Uuid, SystemTime, f32)> {
    let id  = generate_v7();
    let day = day_range(SystemTime::now())?;
    let sim = cosine_similarity(&normalize(query)?, &normalize(doc)?)?;
    Ok((id, day.start, sim))
}
```

---

## common::pipe

A process-wide registry of named MPMC channels used by `bdsnode`'s
ingest pipeline. Producers (the JSON-RPC `v2/add*` handlers) push
records onto a named channel; consumers (the background ingest
threads) drain them in batches and call `ShardsManager::add_batch`.

Re-exported at the crate root as `bdslib::pipe`.

### `init` and `init_with_capacity`

```rust
pub fn init(names: &[&str]) -> Result<()>;
pub fn init_with_capacity(specs: &[(&str, usize)]) -> Result<()>;
```

`init` creates one **unbounded** channel per name. `init_with_capacity`
gives explicit capacities — `0` means unbounded (back-compat); any
positive value creates a **bounded** channel that returns
`"channel <name> is full"` from `send` / `send_many` when the consumer
can't keep up. `bdsnode` uses `init_with_capacity` for the ingest
channels (`ingest`, `ingest_file`, `ingest_file_syslog`) so a producer
flood can't OOM the server with an unbounded queue. The default
capacity is `100_000`, configurable via `ingest_channel_capacity` in
`bds.hjson`. The `v2/add*` JSON-RPC handlers translate channel-full
errors into JSON-RPC code `-32099` ("ingest channel overloaded — back
off and retry").

Must be called exactly once before any `send` / `recv` call.

### `send` and `send_many`

```rust
pub fn send(name: &str, value: serde_json::Value) -> Result<()>;
pub fn send_many(name: &str, values: Vec<serde_json::Value>) -> Result<()>;
```

`send` pushes one value. `send_many` pushes a whole `Vec` in one
call: each crossbeam `send` takes the channel's internal mutex, so
the bulk helper amortises that lock cost across the whole batch
rather than the call site doing N separate acquisitions. Used by
`v2/add.batch` to enqueue large batches without monopolising a tokio
worker.

For bounded channels both helpers use `try_send` (non-blocking):
- channel full → `Err("channel <name> is full")`
- channel disconnected → `Err("channel <name> is disconnected")`

For unbounded channels neither error fires (until the process runs out
of memory).

### `recv`, `try_recv`, `recv_timeout`, `len`, `receiver`

```rust
pub fn recv(name: &str)         -> Result<Value>;
pub fn try_recv(name: &str)     -> Result<Option<Value>>;
pub fn recv_timeout(name: &str, timeout: Duration) -> Result<Option<Value>>;
pub fn len(name: &str)          -> Result<usize>;
pub fn receiver(name: &str)     -> Result<&'static Receiver<Value>>;
```

The consumer side. `receiver` returns the raw crossbeam `Receiver`
for use inside `crossbeam::select!` (e.g. combining ingest and
shutdown signals on the same loop, as the `bds-add` thread does).
`len` exposes the queue depth — surfaced in `v2/status` so dashboards
can monitor backpressure.

### Example: shape used by `bdsnode`

```rust
// startup
bdslib::pipe::init_with_capacity(&[
    ("ingest",             100_000),
    ("ingest_file",        100_000),
    ("ingest_file_syslog", 100_000),
])?;

// JSON-RPC handler — producer
bdslib::pipe::send_many("ingest", docs).map_err(pipe_err)?;
// pipe_err maps "channel is full" → JSON-RPC -32099, anything else → -32001.

// Background ingest thread — consumer
let rx = bdslib::pipe::receiver("ingest")?;
crossbeam::select! {
    recv(rx)            -> msg => /* batch and flush */,
    recv(shutdown_rx)   -> _   => /* drain and exit */,
    default(timeout)            => /* flush partial batch */,
}
```
