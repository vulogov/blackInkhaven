# Inkhaven bench harness (criterion)

*1.2.18 I.1.2+*

Criterion-based subprocess benchmarks driving the
1.2.18 I.1 performance pass.  Each bench spawns the
release-built `inkhaven` binary against a generated
fixture project and measures wall-clock time with
statistical analysis.

## Quick start

```bash
$ cargo build --release
$ ./target/release/inkhaven gen-fixture /tmp/inkhaven-bench
$ INKHAVEN_BENCH_FIXTURE=/tmp/inkhaven-bench cargo bench
```

Reports land under `target/criterion/` as HTML +
JSON.  The HTML index is browsable; the JSON is what
the CI gate (1.2.18 I.1.7) parses.

## Running a single bench

```bash
$ INKHAVEN_BENCH_FIXTURE=/tmp/inkhaven-bench \
    cargo bench --bench startup
$ INKHAVEN_BENCH_FIXTURE=/tmp/inkhaven-bench \
    cargo bench --bench search
```

## Fixture sizing

`gen-fixture` defaults to 5 books × 20 chapters × 100
paragraphs = 10 000 paragraphs.  Take ~20 minutes on
M3 Max because embeddings dominate.  For iteration
during development, scale down:

```bash
$ inkhaven gen-fixture /tmp/inkhaven-bench-small \
    --books 1 --chapters 5 --paragraphs 50 --force
```

The 250-paragraph variant runs in ~30 seconds and
captures most cache-miss patterns the full fixture
exercises.

## Scenarios

| Bench         | What it measures | Variants |
|---------------|------------------|----------|
| `startup`     | Wall-clock for `inkhaven list books` (touches the full project-load path) | cold (fresh spawn each time), warm (caches primed) |
| `search`      | Wall-clock for `inkhaven search <query>` against the fixture | common_phrase, named_entity, rare_phrase |

### Deferred to I.1.2.b

Three scenarios from the 1.2.18 plan need in-process
access (the bench framework can't drive them via
subprocess until inkhaven splits into a lib+bin crate):

* **tree_scroll** — 1000-row Page Down on the tree
  pane.
* **ai_envelope** — RAG retrieval + prompt build.
* **paragraph_save** — full save round-trip including
  `io_atomic` + re-embed.

These land alongside the lib-refactor work or via a
hidden `inkhaven _bench <op>` subcommand that exercises
the in-process paths and reports stdout timing.

## Criterion knobs

Each bench sets `sample_size(10)` + `measurement_time(20s)`.
Subprocess spawning is too slow for criterion's default
100 samples — the bench would take ~20 minutes per
scenario.  Ten samples + a 20-second cap gives stable
medians without burning the day.

To override for a one-off:

```bash
$ INKHAVEN_BENCH_FIXTURE=... cargo bench --bench startup -- \
    --sample-size 20 --measurement-time 60
```

## Interpreting results

Criterion's HTML report shows:

* **median** with IQR shading — robust to subprocess
  outliers.
* **change**: the percent delta vs. the last baseline.
  Green = faster, red = slower.

The 1.2.18 I.1.7 CI gate parses
`target/criterion/<bench>/<variant>/new/estimates.json`
and fails the build when any variant regresses >20%
from the main baseline.

## CI integration

`.github/workflows/bench.yml` (lands in I.1.7) handles:

1. `cargo build --release`
2. `./target/release/inkhaven gen-fixture` (cached
   across runs)
3. `cargo bench` for every scenario
4. `tools/bench-report.rs` posts per-scenario deltas
   as a PR comment
5. Non-zero exit on any >20% regression

For local-only runs, the bench output is enough; the
report-tool integration is a CI-only convenience.

## Adding a new bench

1. New file under `benches/<name>.rs`.
2. `mod common;` at the top for fixture-path + binary
   helpers.
3. `criterion_group!` + `criterion_main!` at the
   bottom.
4. Add `[[bench]]` entry in `Cargo.toml` with
   `harness = false`.
5. Document the variants + interpretation in this
   README.

The subprocess-spawn pattern in `common::run_inkhaven_against`
is the right tool when the operation has a CLI surface.
For in-process scenarios, either:

* Wait for the lib-refactor follow-up + use direct
  module access; or
* Add a hidden `_bench <op>` CLI subcommand that runs
  the in-process operation + reports stdout timing,
  then bench the subcommand.
