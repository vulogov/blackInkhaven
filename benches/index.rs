//! 2.0 harness — index-build / embedding throughput.
//!
//! The dominant cost of building the HNSW index is embedding each paragraph.
//! `reindex` skips content-unchanged paragraphs, so it can't measure the embed
//! cost directly; instead `_bench-embed --count N` runs the embedding engine on
//! N sample texts (multilingual) and reports the internal time (model-load
//! excluded as a warm-up). This bench drives it via `iter_custom` so criterion's
//! per-embed figure is the true throughput.
//!
//! Requires the fastembed model to be present in the cache (any prior
//! `gen-fixture` / normal run downloads it). No fixture project needed.

mod common;

use criterion::{Criterion, criterion_group, criterion_main};
use std::time::Duration;

fn bench_index(c: &mut Criterion) {
    let mut group = c.benchmark_group("index");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));

    group.bench_function("embed", |b| {
        b.iter_custom(|iters| {
            let n = iters.to_string();
            let out = common::run_inkhaven_bare_capture(&["_bench-embed", "--count", &n]);
            common::parse_micros(&out, "embed_total_us:")
        });
    });

    group.finish();
}

criterion_group!(benches, bench_index);
criterion_main!(benches);
