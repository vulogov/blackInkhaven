//! 2.0 harness — export cost.
//!
//! Times `inkhaven export <format>` for the in-process formats (no external
//! tool): markdown (typst→md), typst (concat), epub (bundled zip). Runs against
//! a small dedicated **single-book** fixture generated at setup (so no
//! `--book-name` is needed), regenerated idempotently each run. The whole export
//! path — load → assemble → convert → write — is measured end to end.
//!
//! (`export pdf`/`html` are excluded: pdf needs the `typst` CLI on PATH, html
//! needs a `--output` directory.)

mod common;

use criterion::{Criterion, criterion_group, criterion_main};
use std::time::Duration;

fn bench_export(c: &mut Criterion) {
    let fixture = common::ensure_export_fixture();
    let outdir = std::env::temp_dir().join("inkhaven-bench-export-out");
    let _ = std::fs::create_dir_all(&outdir);

    let mut group = c.benchmark_group("export");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));

    for (fmt, ext) in [("markdown", "md"), ("typst", "typ"), ("epub", "epub")] {
        let out = outdir.join(format!("book.{ext}"));
        let out_s = out.to_str().expect("utf-8 path").to_string();
        group.bench_function(fmt, |b| {
            b.iter_custom(|iters| {
                let mut total = Duration::ZERO;
                for _ in 0..iters {
                    total += common::run_inkhaven_against(&fixture, &["export", fmt, "-o", &out_s]);
                }
                total
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_export);
criterion_main!(benches);
