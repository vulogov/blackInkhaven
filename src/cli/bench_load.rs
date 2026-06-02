//! 1.2.18+ I.1.3 — in-process load-path profiler.
//!
//! `inkhaven _bench-load` (hidden) opens the project
//! with coarse phase timers + reports per-phase
//! millis on stdout.  The I.1.2.b in-process bench
//! hook, doubling as the I.1.3 profiling instrument
//! since a true sampling flamegraph needs dtrace
//! (SIP-blocked without sudo on macOS).
//!
//! Phases measured here (callable publicly):
//!
//!   * `store_open` — `Store::open` end-to-end.  Set
//!     `INKHAVEN_PERF_TRACE=1` for the embedding-engine
//!     vs. DuckDB-open sub-breakdown emitted from
//!     `store/mod.rs`.
//!   * `hierarchy_load` — `Hierarchy::load`.  Set the
//!     same env var for the list_metadata / parse / sort
//!     sub-breakdown from `store/hierarchy.rs`.
//!   * `flatten` — `Hierarchy::flatten` (tree-pane row
//!     materialisation), averaged over `iterations`.
//!   * `search` — `Store::search_text`, averaged over
//!     `iterations`.
//!
//! The output is a stable, line-oriented summary that
//! `Documentation/PROPOSALS/PERF.md` quotes verbatim.

use std::path::Path;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::error::Result;
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;

pub fn run(project: &Path, query: &str, iterations: usize) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;

    println!("inkhaven _bench-load — v{}", env!("CARGO_PKG_VERSION"));
    println!("project:    {}", project.display());
    println!("iterations: {iterations}");
    println!();

    // ── store_open ────────────────────────────────────
    let t = Instant::now();
    let store = Store::open(layout.clone(), &cfg)?;
    let store_open = t.elapsed();
    report("store_open", store_open);

    // ── hierarchy_load ────────────────────────────────
    let t = Instant::now();
    let hierarchy = Hierarchy::load(&store)?;
    let hierarchy_load = t.elapsed();
    report("hierarchy_load", hierarchy_load);

    let node_count = hierarchy.iter().count();
    println!("nodes:      {node_count}");

    // ── flatten (averaged) ────────────────────────────
    let mut flatten_total = Duration::ZERO;
    let mut flatten_rows = 0usize;
    for _ in 0..iterations {
        let t = Instant::now();
        let rows = hierarchy.flatten();
        flatten_total += t.elapsed();
        flatten_rows = rows.len();
    }
    report_avg("flatten", flatten_total, iterations);
    println!("flatten_rows: {flatten_rows}");

    // ── search (averaged) ─────────────────────────────
    let mut search_total = Duration::ZERO;
    let mut hits = 0usize;
    for _ in 0..iterations {
        let t = Instant::now();
        let results = store.search_text(query, 10)?;
        search_total += t.elapsed();
        hits = results.len();
    }
    report_avg("search", search_total, iterations);
    println!("search_hits: {hits} (query: {query:?})");

    println!();
    println!(
        "total_cold_load: {:.2}ms (store_open + hierarchy_load)",
        (store_open + hierarchy_load).as_secs_f64() * 1000.0,
    );

    Ok(())
}

fn report(label: &str, elapsed: Duration) {
    println!("{label:<16} {:>10.2}ms", elapsed.as_secs_f64() * 1000.0);
}

fn report_avg(label: &str, total: Duration, iterations: usize) {
    let avg = total.as_secs_f64() * 1000.0 / iterations.max(1) as f64;
    println!("{label:<16} {avg:>10.2}ms (avg of {iterations})");
}
