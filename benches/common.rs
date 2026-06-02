//! Shared helpers for the 1.2.18 I.1.2 bench harness.
//!
//! Each bench file `mod common;`-imports this module
//! (Cargo's bench layout doesn't have a shared-helper
//! convention; `mod` from each bench is the standard
//! workaround).
//!
//! ## Fixture discovery
//!
//! Benches expect a pre-generated inkhaven project at
//! the path in `INKHAVEN_BENCH_FIXTURE`.  Generate one
//! with:
//!
//! ```bash
//! $ cargo build --release
//! $ ./target/release/inkhaven gen-fixture /tmp/inkhaven-bench
//! $ INKHAVEN_BENCH_FIXTURE=/tmp/inkhaven-bench cargo bench
//! ```
//!
//! The fixture is NOT committed to the repo (~50 MB at
//! the default 10K-paragraph shape).  CI generates it
//! once per workflow run + caches the result.
//!
//! ## Subprocess timing
//!
//! Until inkhaven splits into a lib+bin crate (deferred;
//! large refactor), benches measure via subprocess: spawn
//! `./target/release/inkhaven <args>` and time the
//! wait-exit cycle.  Adds ~20 ms of process-startup
//! overhead per iteration but the overhead is constant +
//! captured in every variant, so regression deltas stay
//! meaningful.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Path to the generated fixture project.  Panics with
/// a clear message when `INKHAVEN_BENCH_FIXTURE` is
/// unset — the bench can't run without one.
pub fn fixture_path() -> PathBuf {
    match std::env::var("INKHAVEN_BENCH_FIXTURE") {
        Ok(p) => {
            let path = PathBuf::from(p);
            if !path.join("metadata.db").exists() {
                panic!(
                    "INKHAVEN_BENCH_FIXTURE points at `{}` but no \
                     `metadata.db` was found there.  Run \
                     `./target/release/inkhaven gen-fixture <path>` \
                     to generate one.",
                    path.display(),
                );
            }
            path
        }
        Err(_) => panic!(
            "INKHAVEN_BENCH_FIXTURE is unset.  Generate a fixture with \
             `./target/release/inkhaven gen-fixture /tmp/inkhaven-bench` \
             and re-run with `INKHAVEN_BENCH_FIXTURE=/tmp/inkhaven-bench \
             cargo bench`."
        ),
    }
}

/// Path to the release-built inkhaven binary.  Assumes
/// `cargo bench` is run after `cargo build --release`.
pub fn inkhaven_binary() -> PathBuf {
    // `cargo bench` runs from the workspace root, so
    // `target/release/inkhaven` is correct unless
    // CARGO_TARGET_DIR was set.  Honour the env if
    // present.
    let target_dir = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("target"));
    let bin = target_dir.join("release").join("inkhaven");
    if !bin.exists() {
        panic!(
            "expected the release binary at `{}` — run \
             `cargo build --release` before `cargo bench`.",
            bin.display(),
        );
    }
    bin
}

/// Spawn `inkhaven <args>` against `project_path` and
/// return the wall-clock duration.  Stdout + stderr are
/// captured (and ignored) so the benches don't spam
/// the criterion output.  Panics on non-zero exit so a
/// silent failure can't pollute the measurement.
pub fn run_inkhaven_against(
    project: &Path,
    args: &[&str],
) -> Duration {
    let bin = inkhaven_binary();
    let start = Instant::now();
    let output = std::process::Command::new(&bin)
        .arg("--project")
        .arg(project)
        .args(args)
        .output()
        .unwrap_or_else(|e| {
            panic!("spawn {} {args:?}: {e}", bin.display())
        });
    let elapsed = start.elapsed();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "inkhaven {args:?} exited {:?}: {}",
            output.status.code(),
            stderr.trim(),
        );
    }
    elapsed
}

/// Same as `run_inkhaven_against` but takes no project
/// arg — for subcommands that don't need one (or that
/// take their own path positional, like `gen-fixture`).
pub fn run_inkhaven_bare(args: &[&str]) -> Duration {
    let bin = inkhaven_binary();
    let start = Instant::now();
    let output = std::process::Command::new(&bin)
        .args(args)
        .output()
        .unwrap_or_else(|e| {
            panic!("spawn {} {args:?}: {e}", bin.display())
        });
    let elapsed = start.elapsed();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "inkhaven {args:?} exited {:?}: {}",
            output.status.code(),
            stderr.trim(),
        );
    }
    elapsed
}
