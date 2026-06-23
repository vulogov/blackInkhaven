//! INNER_SOCRATES-1 — Examined Authorship via Socratic Interrogation.
//! See `Documentation/PROPOSALS/INNER_SOCRATES-1_PLAN.md`.
//!
//! A structured Socratic interrogation of the author's own manuscript — the third
//! piece of Inkhaven's *examined authorship* triad (alongside the ConLang suite
//! and WORLD-4). It reads alongside the author and surfaces **questions** about
//! assumptions, framings, tensions, and significance the prose carries. It is
//! non-prescriptive by structural commitment: every finding is a question, never
//! a correction. The author examines their prose; Inner Socrates never edits it.
//!
//! Built incrementally in the 1.3.x tree, one signed phase per increment, as a
//! sibling of `src/world/`. **P0 ships the pure foundation**: the value types
//! (severity, the 7 Fast + 8 Slow categories, the finding, the persona), the
//! **intent ledger** with lazy consultation (the same pattern as WORLD-4's magic
//! ledger), and the first deterministic Fast-track detectors. Storage, the LLM
//! Slow track, the TUI chord family, and multilingual support land in later
//! phases. Zero new dependencies — everything is inherited from PANE-1 + WORLD-4.

// Foundation ahead of its storage / TUI / Bund consumers.
#![allow(dead_code, unused_imports)]

pub mod fast;
pub mod intent;
pub mod lang;
pub mod output;
pub mod slow;
pub mod storage;
pub mod text;
pub mod timeline;
pub mod types;

use std::fmt;

/// The crate-local result type for Inner Socrates operations.
pub type Result<T> = std::result::Result<T, SocratesError>;

/// Errors from Inner Socrates operations.
#[derive(Debug, Clone)]
pub enum SocratesError {
    /// A persona / ledger / config file could not be parsed.
    Parse(String),
    /// An operation needed something not present (e.g. an LLM provider).
    Unavailable(String),
}

impl fmt::Display for SocratesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SocratesError::Parse(m) => write!(f, "inner-socrates parse error: {m}"),
            SocratesError::Unavailable(m) => write!(f, "inner-socrates unavailable: {m}"),
        }
    }
}

impl std::error::Error for SocratesError {}
