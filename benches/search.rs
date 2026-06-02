//! 1.2.18 I.1.2 — search-time benchmark.
//!
//! Measures the wall-clock time of `inkhaven search
//! <query>` against the generated 10K-paragraph
//! fixture.  `store.search_text` combines lexical +
//! HNSW semantic retrieval; this is the user-facing
//! "search the project" surface.
//!
//! Three query variants exercise different parts of the
//! retrieval stack:
//!
//!   * **common_phrase** — "the harbor" / "the garden"
//!     — words that appear in many sentences across the
//!     SENTENCE_POOL.  Stresses ranking + de-dup.
//!   * **named_entity** — "Helena Marcus" — recurring
//!     character names.  Exercises tokenisation +
//!     lexical match.
//!   * **rare_phrase** — "lacquered box" — appears in
//!     only a few sentences.  Exercises selectivity.
//!
//! Subprocess overhead is constant across variants;
//! the deltas tell the real story.

mod common;

use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};

fn bench_search(c: &mut Criterion) {
    let fixture = common::fixture_path();

    let mut group = c.benchmark_group("search");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));

    let queries = [
        ("common_phrase", "the harbor"),
        ("named_entity", "Helena Marcus"),
        ("rare_phrase", "lacquered box"),
    ];

    for (label, query) in queries {
        group.bench_function(label, |b| {
            // Prime caches once so all three variants
            // measure the steady-state retrieval cost,
            // not first-open overhead.
            let _ = common::run_inkhaven_against(
                &fixture,
                &["search", query, "--limit", "10"],
            );
            b.iter_custom(|iters| {
                let mut total = Duration::ZERO;
                for _ in 0..iters {
                    let elapsed = common::run_inkhaven_against(
                        &fixture,
                        &["search", query, "--limit", "10"],
                    );
                    total += elapsed;
                }
                total
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_search);
criterion_main!(benches);
