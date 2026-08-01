//! 2.0 harness — SEMNET graph edge-store throughput.
//!
//! The knowledge-graph layer's two hot paths: bulk edge insertion (a
//! `graph rebuild` / `graph lexical` writes thousands of edges) and the
//! reverse-index neighbour query (`graph neighbors` / `contradicting` / the
//! Inner-family grounding). Both are driven through the hidden `_bench-graph`,
//! which builds an isolated temp edge store and reports its *internal* insert /
//! query time — so the figure excludes process startup and the criterion
//! per-unit cost is the true edge throughput / query latency.
//!
//! Self-contained: no fixture project, no network. Regressions worth catching
//! here are structural (losing the reverse index → O(n) neighbour scans;
//! making insert O(n²)) — order-of-magnitude, not marginal.

mod common;

use criterion::{Criterion, criterion_group, criterion_main};
use std::time::Duration;

fn bench_graph(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(15));

    // Edge-insert throughput: insert `iters` edges (queries skipped so the
    // measurement is pure insertion).
    group.bench_function("insert", |b| {
        b.iter_custom(|iters| {
            let n = iters.to_string();
            let out = common::run_inkhaven_bare_capture(&["_bench-graph", "--edges", &n, "--queries", "0"]);
            common::parse_micros(&out, "edge_insert_total_us:")
        });
    });

    // Reverse-index neighbour-query latency against a fixed 5k-edge store: run
    // `iters` queries (the store build is overhead, excluded from the metric).
    group.bench_function("neighbor_query", |b| {
        b.iter_custom(|iters| {
            let q = iters.to_string();
            let out = common::run_inkhaven_bare_capture(&["_bench-graph", "--edges", "5000", "--queries", &q]);
            common::parse_micros(&out, "edge_query_total_us:")
        });
    });

    group.finish();
}

criterion_group!(benches, bench_graph);
criterion_main!(benches);
