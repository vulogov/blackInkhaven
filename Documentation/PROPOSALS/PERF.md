# Inkhaven I.1.3 — performance hotspot analysis

**Status:** investigation complete (2026-06-02).
**Method:** timing-instrumented phase profiling
(`INKHAVEN_PERF_TRACE=1` + `inkhaven _bench-load`).

## Why not a sampling flamegraph?

The 1.2.18 plan called for `cargo flamegraph` per
scenario.  In this environment that's not possible:
macOS System Integrity Protection is on, and `dtrace`
(which `cargo-flamegraph` shells out to on macOS)
requires elevated privileges SIP won't grant without
`sudo` + a reboot into a reduced-security mode.

Rather than fake an SVG, I.1.3 uses **timing
instrumentation**: coarse `Instant` checkpoints around
each phase of the project-load + search path, gated
behind `INKHAVEN_PERF_TRACE=1` so they're zero-cost in
normal use.  The `inkhaven _bench-load` hidden
subcommand (also the I.1.2.b in-process bench hook)
drives them and averages flatten + search over N
iterations.

This gives a real, defensible, *reproducible* hotspot
breakdown — and because it measures wall-clock per
named phase rather than sampling the call stack, the
attribution is exact, not statistical.

## Measurement setup

* Host: Apple M3 Max, macOS (SIP on).
* Build: `cargo build --release` (lto=thin,
  codegen-units=1).
* Fixtures: `inkhaven gen-fixture` at two sizes:
  * **small** — 1 book × 3 chapters × 30 paragraphs =
    90 user paragraphs (114 nodes incl. system books).
  * **2k** — 2 books × 10 chapters × 100 paragraphs =
    2000 user paragraphs (2046 nodes).
* Command:
  `INKHAVEN_PERF_TRACE=1 inkhaven --project <fix> _bench-load --iterations 20`

## Raw numbers

| Phase                          | 114 nodes | 2046 nodes | Scaling |
|--------------------------------|-----------|------------|---------|
| `store.open.embedding_engine`  | 474.56 ms | 469.62 ms  | **constant** |
| `store.open.duckdb_open`       |  17.32 ms |  15.77 ms  | constant |
| `store.open.ensure_system_books`|  1.48 ms |  15.07 ms  | ~linear |
| `hierarchy.load.list_metadata` |  ~0.2 ms  |   4.63 ms  | linear |
| `hierarchy.load.parse_nodes`   |  ~0.1 ms  |   1.66 ms  | linear |
| `hierarchy.load.sort`          |  ~0.1 ms  |   1.49 ms  | n log n |
| `flatten` (per call)           |   0.10 ms |  31.78 ms  | **O(n²)** |
| `search` (per call)            |   5.37 ms |  79.37 ms  | ~linear-ish |
| **total cold load**            | 493.81 ms | 507.33 ms  | constant-dominated |

## Findings

### #1 — Eager embedding-engine load dominates startup (470 ms, ~92 %)

`Store::open` calls `build_embedding_engine` **eagerly,
unconditionally**, before anything else.  Loading the
fastembed ONNX model takes ~470 ms and is
**project-size-independent** (474 ms at 114 nodes,
470 ms at 2046 — it's pure model load).

This cost is paid by *every* command that opens a
project, including ones that never embed or search:
`inkhaven list`, `inkhaven add`, the TUI launch before
the first search, etc.

> **This is the headline hotspot.**  It is NOT the
> "lazy tree-row materialisation" the 1.2.18 plan's
> I.1.4 hypothesized — the data redirects the fix.

**Fix (revised I.1.4): lazy embedding-engine init.**
Defer `build_embedding_engine` until the first
operation that actually needs vectors (search, embed,
reindex).  `inkhaven list` startup drops from ~500 ms
to ~35 ms.  TUI launch lands on the cursor immediately;
the model loads in the background (or on the first
`Ctrl+F` semantic search).

Estimated win: **~470 ms off every non-search cold
start.**

### #2 — `Hierarchy::flatten` is O(n²) (31 ms @ 2k, ~760 ms projected @ 10k)

`flatten` walks the tree depth-first; for each node it
calls `children_of(parent_id)`, which does a full
`iter().filter()` scan of every node.  That's O(n) per
node → **O(n²) overall**.

The numbers confirm it: 18× the nodes (114 → 2046)
produced **318× the time** (0.10 → 31.78 ms) ≈ 18².
Projected to 10 000 nodes: `(10000/2046)² × 31.78 ≈
760 ms` *per flatten call*.

`flatten` / `flatten_with_collapsed` is called on the
tree-pane render path — potentially every frame.  At
10K nodes that's the difference between smooth scrolling
and a visibly janky tree.

> The 1.2.18 plan's "lazy tree-row materialisation"
> instinct was directionally right (the tree is a
> hotspot) but wrong about the mechanism: it's not the
> *load* that's slow (hierarchy.load is 6.8 ms at 2k),
> it's the *per-render flatten*.

**Fix (revised I.1.5): parent→children index.**  Build
a `HashMap<Option<Uuid>, Vec<Uuid>>` (sorted by order)
once in `Hierarchy::load`; `children_of` becomes an
O(1) map lookup + the children already in order.
`flatten` drops to O(n).  Invalidate / rebuild the
index on tree mutation (create / move / delete) — those
are rare relative to renders.

Estimated win: **flatten from O(n²) to O(n)** —
~760 ms → ~5 ms at 10K.

### #3 — Search scales with corpus, dominated by query embedding (79 ms @ 2k)

`search_text` embeds the query string (fastembed
forward pass) + runs HNSW + lexical retrieval.  79 ms
at 2046 nodes, 5 ms at 114 — most of the delta is the
HNSW search over a larger index, plus the constant
query-embedding forward pass.

Search is an **explicit user action** (Ctrl+F, the
search CLI), not a per-frame or per-launch cost, so
79 ms is acceptable.  Lower priority.

**Possible future fix (deferred):** cache the query
embedding for repeated identical queries; warm the
HNSW index lazily.  Not worth a phase this cycle.

### Non-issues (measured, ruled out)

* **`hierarchy.load`** — 6.8 ms total at 2k
  (list_metadata 4.6 + parse 1.7 + sort 1.5).  The
  proposal feared this; the data clears it.  No fix
  needed.
* **`duckdb_open`** — 16 ms, constant.  Fine.
* **DuckDB write batching (proposal I.1.6)** — the save
  path wasn't profiled here (`_bench-load` is a
  read-path instrument).  Deferred: needs a
  `_bench-save` companion + is a smaller win than #1/#2.
  Re-evaluate after #1 + #2 land.

## Revised fix plan

The proposal's I.1.4–I.1.6 hypotheses are superseded by
the measured data:

| Phase  | Proposal hypothesis | **Revised (data-driven)** | Est. win | Status |
|--------|--------------------|--------------------------|----------|--------|
| I.1.4  | lazy tree-row materialisation | **lazy embedding-engine init** | ~470 ms / cold start | ✅ **landed** |
| I.1.5  | background embedding refresh (save) | **parent→children index (flatten O(n²)→O(n))** | ~755 ms / flatten @ 10K | ✅ **landed** |
| I.1.6  | DuckDB write batching | deferred — profile save path first (`_bench-save`), smaller win | TBD | deferred |

### I.1.4 result (measured)

`EmbeddingEngine::new` now stores its construction
parameters + defers the ~470 ms `TextEmbedding::try_new`
to the first `embed` / `embed_batch`.  Re-profiled the
2046-node fixture warm:

| Phase                          | before  | **after** |
|--------------------------------|---------|-----------|
| `store.open.embedding_engine`  | 469.62 ms | **0.03 ms** |
| `store_open` (total)           | 500.51 ms | **29.22 ms** |
| `total_cold_load`              | 507.33 ms | **35.93 ms** |
| `inkhaven list` (subprocess)   | 486 ms  | **85.6 ms** |

The one-time ~470 ms model load now happens on the first
search / save / reindex — exactly where it's needed —
instead of on every project open.  Search correctness
unchanged (the first query pays the load, amortised
across a session).  `inkhaven list` / `add` / TUI launch
all land ~470 ms faster.

### I.1.5 result (measured)

`Hierarchy` now builds a parent→children index
(`HashMap<Option<Uuid>, Vec<Uuid>>`) once in `load`, in
a single O(n) pass over the already-sorted `order` vec.
`children_of` becomes an O(k) map lookup (k = child
count) instead of an O(n) full scan, which collapses
`flatten`'s O(n²) into O(n).  The buckets come out
pre-sorted (a parent's children share a depth, so the
global `(depth, order, slug)` sort already orders them),
so there's no per-bucket sort.

Re-profiled the 2046-node fixture:

| Phase                       | before    | **after** |
|-----------------------------|-----------|-----------|
| `flatten` (per call)        | 31.98 ms  | **0.05 ms** |
| `hierarchy.load.build_index`| —         | 0.08 ms (new, one-time) |
| `hierarchy_load` (total)    | 6.82 ms   | 6.98 ms (≈ flat) |

640× faster flatten at 2k.  Projected to 10K (linear
now): ~0.25 ms vs. the ~760 ms O(n²) problem.  `flatten`
runs on the tree-pane render path, so this is the
difference between smooth + janky scrolling on a large
project.

The index also speeds up every other `children_of`
consumer for free: `has_children` (now O(1)),
`next_order`, `walk_ids` / `collect_subtree`,
`find_by_path`.

`Hierarchy` is immutable after construction (mutations
reload via `Hierarchy::load`), so the index never needs
invalidating — no cache-coherence complexity.

Correctness: 7 `index_tests` assert children order
(order-field beats insertion order), preorder-DFS
flatten, has_children, next_order, and empty-hierarchy
well-formedness.

> A dedicated criterion `tree_scroll` bench is deferred:
> measuring a 0.05 ms in-process flatten under criterion
> needs the lib-refactor (a subprocess bench's ~85 ms
> startup would drown the signal).  The `_bench-load`
> instrument already captures the win precisely.

I.1.7 (CI gates) landed: `.github/workflows/bench.yml`
runs the suite, publishes a `bench-baseline` artifact on
main, and on PRs compares via `inkhaven _bench-report`
(exit 2 + `::error::` + PR comment on any >20%
regression).  The gate is intentionally loose because
the regressions it exists to catch — re-introducing the
eager engine load or the O(n²) flatten — are
order-of-magnitude, robust to shared-runner noise.

The two big wins — lazy engine init + the flatten index
— are independent, each tied to a different bench
scenario (`startup` for #1, a future `tree_scroll` for
#2), and each carries a clear before/after threshold.

## Reproducing

```bash
$ cargo build --release
$ ./target/release/inkhaven gen-fixture /tmp/perf-2k \
    --books 2 --chapters 10 --paragraphs 100 --force
$ INKHAVEN_PERF_TRACE=1 \
    ./target/release/inkhaven --project /tmp/perf-2k \
    _bench-load --iterations 20
```

The `[perf]` lines on stderr give the store-open +
hierarchy-load sub-phase breakdown; the stdout summary
gives the averaged flatten + search timings.
