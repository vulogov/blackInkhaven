//! WORLD-4 — World Simulation (`inkhaven realworld` + the real-time
//! fact-checker). See `Documentation/PROPOSALS/WORLD-4_PLAN.md`.
//!
//! Branch A is a deterministic, layered, on-demand compiler that turns a
//! structured world definition (`world.hjson` + optional `world.bund`) into
//! populated system books. Five MVP layers run in dependency order —
//! astronomy → geology → climate → hydrology → demographics — each a pure
//! function of `(definition, seed)` (layer 5 has seeded-stochastic + AI parts).
//!
//! This module is built incrementally in the 1.3.x tree, one layer per signed
//! increment. **P0 ships the world-definition types and the astronomy layer**:
//! closed-form planetary physics (Kepler's third law, daily-insolation by
//! latitude band, lunar synodic periods, tides), with zero new dependencies.
//! Later phases add storage, materialization, the remaining layers, plakat, and
//! the fact-checker.
//!
//! Authority discipline (the spine of the RFC): the author always wins. The
//! compiler *proposes*; nothing commits without acceptance. Astronomy is the one
//! layer with no proposals — its outputs are closed-form physics, treated as
//! fact and re-asserted every run unless the author hand-overrides.

// Foundation ahead of its CLI / TUI / Bund consumers: the type/output API and
// its re-exports exist before the phases that will consume them.
#![allow(dead_code, unused_imports)]

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
