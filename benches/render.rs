//! 2.0 harness — editor frame-render cost.
//!
//! `_bench-render --frames N` draws N editor frames against a headless
//! `TestBackend` and prints the *internal* render time (process startup
//! excluded). This bench drives it via `iter_custom`: it asks the child to draw
//! `iters` frames and reports the child's own timing, so criterion's per-frame
//! figure is the true draw cost — the number the event-driven-redraw work moves.
//!
//! Requires `INKHAVEN_BENCH_FIXTURE=<path>` (see `benches/README.md`).

mod common;

use criterion::{Criterion, criterion_group, criterion_main};
use std::time::Duration;

fn bench_render(c: &mut Criterion) {
    let fixture = common::fixture_path();

    let mut group = c.benchmark_group("render");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));

    group.bench_function("editor_frame", |b| {
        b.iter_custom(|iters| {
            let n = iters.to_string();
            let out = common::run_inkhaven_capture(&fixture, &["_bench-render", "--frames", &n]);
            common::parse_micros(&out, "render_total_us:")
        });
    });

    group.finish();
}

criterion_group!(benches, bench_render);
criterion_main!(benches);
