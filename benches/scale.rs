//! 2.0 harness — scale sweep.
//!
//! Runs `list` and `search` against one or more fixtures so the report shows how
//! latency scales with corpus size. Point it at several fixtures with
//!
//! ```bash
//! INKHAVEN_BENCH_FIXTURES="1k=/tmp/f1k:10k=/tmp/f10k:50k=/tmp/f50k" \
//!   cargo bench --bench scale
//! ```
//!
//! (label=path, colon-separated). With that unset it falls back to the single
//! `INKHAVEN_BENCH_FIXTURE` labelled `default`. Generate sizes with e.g.
//! `inkhaven gen-fixture /tmp/f50k --books 25 --chapters 20 --paragraphs 100`.

mod common;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::path::PathBuf;
use std::time::Duration;

/// The fixtures to sweep, as `(label, path)`.
fn fixtures() -> Vec<(String, PathBuf)> {
    if let Ok(spec) = std::env::var("INKHAVEN_BENCH_FIXTURES") {
        let list: Vec<(String, PathBuf)> = spec
            .split(':')
            .filter_map(|entry| {
                let (label, path) = entry.split_once('=')?;
                Some((label.to_string(), PathBuf::from(path)))
            })
            .collect();
        if !list.is_empty() {
            return list;
        }
    }
    vec![("default".to_string(), common::fixture_path())]
}

fn bench_scale(c: &mut Criterion) {
    let fixtures = fixtures();

    let mut group = c.benchmark_group("scale");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(15));

    for (label, path) in &fixtures {
        group.bench_with_input(BenchmarkId::new("list", label), path, |b, p| {
            b.iter_custom(|iters| {
                let mut total = Duration::ZERO;
                for _ in 0..iters {
                    total += common::run_inkhaven_against(p, &["list"]);
                }
                total
            });
        });
        group.bench_with_input(BenchmarkId::new("search", label), path, |b, p| {
            // Prime caches so we measure steady-state retrieval.
            let _ = common::run_inkhaven_against(p, &["search", "the harbor", "--limit", "10"]);
            b.iter_custom(|iters| {
                let mut total = Duration::ZERO;
                for _ in 0..iters {
                    total += common::run_inkhaven_against(p, &["search", "the harbor", "--limit", "10"]);
                }
                total
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_scale);
criterion_main!(benches);
