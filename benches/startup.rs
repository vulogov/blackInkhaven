//! 1.2.18 I.1.2 — startup-time benchmark.
//!
//! Measures the wall-clock time from `inkhaven <args>`
//! invocation to exit for a fast, project-touching
//! subcommand.  Uses `list books` because it requires
//! the full project-load path (open metadata.db,
//! reconstruct hierarchy, open vector store) but
//! doesn't render anything or wait on user input.
//!
//! Two variants:
//!
//!   * **cold**: each iteration is a fresh subprocess
//!     spawn.  Captures process startup + dynamic
//!     linker + first DB open.
//!   * **warm**: the OS filesystem cache + DuckDB
//!     internal cache get a one-shot priming run
//!     before each iteration is timed.
//!
//! Both are reported as median + IQR by criterion.
//! The realistic user-perceived metric is "warm" since
//! most launches in a writing session find caches hot;
//! "cold" is the first-of-the-day case.

mod common;

use std::time::{Duration, Instant};

use criterion::{criterion_group, criterion_main, Criterion};

fn bench_startup(c: &mut Criterion) {
    let fixture = common::fixture_path();

    let mut group = c.benchmark_group("startup");
    // Subprocess overhead is ~20 ms; ten iterations per
    // sample gives criterion enough signal without
    // bench-runs becoming a coffee break.
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));

    group.bench_function("cold_list", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let elapsed = common::run_inkhaven_against(
                    &fixture,
                    &["list"],
                );
                total += elapsed;
            }
            total
        });
    });

    group.bench_function("warm_list", |b| {
        // Prime caches once before each iteration so the
        // measurement reflects the steady-state user
        // experience (second + subsequent launches).
        let _ = common::run_inkhaven_against(&fixture, &["list"]);
        b.iter_custom(|iters| {
            let start = Instant::now();
            for _ in 0..iters {
                let _ = common::run_inkhaven_against(
                    &fixture,
                    &["list"],
                );
            }
            start.elapsed()
        });
    });

    group.finish();
}

criterion_group!(benches, bench_startup);
criterion_main!(benches);
