//! 3.18 LI-2 — ConLang suite benchmark.
//!
//! Wall-clock for the `language` commands that load a language book and run
//! the translation / analysis engines against it, on the fixture's generated
//! `Benchlang` (`inkhaven gen-fixture … --conlang-words 300`). Before LI-2
//! every one of these rebuilt phonology, morphology, typology and the whole
//! dictionary from the store per call (one `SELECT` per paragraph) and rebuilt
//! the gloss / reverse / root indexes per call; the four variants cover
//! one-block load (`syllabify`), the forward translator over a ten-clause
//! paragraph (`translate`), the reverse translator (`reverse`) and the
//! manuscript-wide Oracle scan (`scan_manuscript`).
//!
//! Like the other benches this measures the whole subprocess (≈20 ms of
//! process start + project open per iteration), so it tracks regressions in
//! the load + engine path rather than micro-costs.

mod common;

use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};

const LANG: &str = "Benchlang";

const PARAGRAPH: &str = "The harbor froze in January. Helena watched the garden from the tower. \
Marcus walked to the river with a lantern. The council met at dawn. \
A storm broke over the eastern hills. Selene read the old letter twice. \
Brennan carried water from the well. The bells rang across the square. \
The children hid in the orchard. Night came early that winter.";

fn bench_conlang(c: &mut Criterion) {
    let fixture = common::fixture_path();

    // The fixture must carry the language; say so plainly instead of timing
    // a failing command.
    let listed = common::run_inkhaven_capture(&fixture, &["language", "list"]);
    if !listed.contains(LANG) {
        panic!(
            "fixture at {} has no `{LANG}` language — regenerate it with \
             `inkhaven gen-fixture <path> --conlang-words 300`",
            fixture.display()
        );
    }

    // A real conlang sentence for the reverse path: the forward translation
    // of a pool sentence (JSON `target`), falling back to a generated word.
    let json = common::run_inkhaven_capture(
        &fixture,
        &["language", "translate", LANG, "Helena watched the garden.", "--json"],
    );
    let target = serde_json::from_str::<serde_json::Value>(&json)
        .ok()
        .and_then(|v| v.get("target").and_then(|t| t.as_str()).map(String::from))
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "kata".to_string());

    let mut group = c.benchmark_group("conlang");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));

    group.bench_function("syllabify", |b| {
        b.iter(|| common::run_inkhaven_against(&fixture, &["language", "syllabify", LANG, "--word", "kata"]))
    });
    group.bench_function("translate_paragraph", |b| {
        b.iter(|| common::run_inkhaven_against(&fixture, &["language", "translate", LANG, PARAGRAPH]))
    });
    group.bench_function("reverse", |b| {
        b.iter(|| common::run_inkhaven_against(&fixture, &["language", "reverse", LANG, &target]))
    });
    group.bench_function("scan_manuscript", |b| {
        b.iter(|| common::run_inkhaven_against(&fixture, &["language", "scan-manuscript", LANG]))
    });

    group.finish();
}

criterion_group!(benches, bench_conlang);
criterion_main!(benches);
