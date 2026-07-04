//! WORLD-4 — World Simulation (`inkhaven realworld` + the real-time
//! fact-checker). See `Documentation/PROPOSALS/WORLD-4_PLAN.md`.
//!
//! Branch A is a deterministic, layered, on-demand compiler that turns a
//! structured world definition (`world.hjson` + optional `world.bund`) into
//! populated system books. Five MVP layers run in dependency order —
//! astronomy → geology → climate → hydrology → demographics — each a pure
//! function of `(definition, seed)` (layer 5 has seeded-stochastic + AI parts).
//!
//! All five physical layers are built and tested (`compile/`), each materialises
//! into its World-book chapter (`materialize.rs`), and the surface is complete:
//! `realworld compile [--layer <name>|all]`, the plakat `map`, the `places`
//! bridge, `magic`, `coherence`, co-location, proposals, and the fast/slow
//! fact-checker. WORLD-7 (1.6.0) unifies a bare `compile` into a one-command
//! whole-world compile + materialise, surfaces every layer in the TUI, and
//! deepens the world→prose bridge.
//!
//! Authority discipline (the spine of the RFC): the author always wins. The
//! compiler *proposes*; nothing commits without acceptance. Astronomy is the one
//! layer with no proposals — its outputs are closed-form physics, treated as
//! fact and re-asserted every run unless the author hand-overrides.


// WORLD-7 — the simulation is now wired end-to-end (compile → materialise →
// surface), so the blanket `unused_imports` allow is retired. A handful of
// foundation items remain ahead of their consumers (the `WorldError`/`Result`
// pair, a few reserved fact-check / storage / utopia helpers), so `dead_code`
// stays scoped here.
#![allow(dead_code)]

pub mod calc_read;
pub mod compile;
pub mod fact_check;
pub mod fact_check_lang;
pub mod fact_check_slow;
pub mod materialize;
pub mod plakat;
pub mod proposals;
pub mod storage;
pub mod timeline_context;
pub mod types;
pub mod weather;
// WORLD-6 — utopian/dystopian coherence checker.
pub mod utopia;

use std::fmt;

/// The crate-local result type for world operations.
pub type Result<T> = std::result::Result<T, WorldError>;

/// Errors from parsing, validating, or compiling a world definition.
#[derive(Debug, Clone)]
pub enum WorldError {
    /// The `world.hjson` could not be parsed into a [`types::WorldDefinition`].
    Parse(String),
    /// The definition parsed but failed an internal-consistency check.
    Validate(String),
    /// A compilation layer failed.
    Compile(String),
}

impl fmt::Display for WorldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorldError::Parse(m) => write!(f, "world definition parse error: {m}"),
            WorldError::Validate(m) => write!(f, "world definition invalid: {m}"),
            WorldError::Compile(m) => write!(f, "world compile error: {m}"),
        }
    }
}

impl std::error::Error for WorldError {}
