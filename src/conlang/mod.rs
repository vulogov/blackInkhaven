//! ConLang Development Suite (LANG-1) — a flagship 1.3.x feature layered on
//! the existing 1.2.13 `Language` system book. The book stays the system of
//! record; these engines reconstruct an in-memory model from its HJSON
//! chapters. See `Documentation/PROPOSALS/LANG-1_PLAN.md`.
//!
//! P1.1 (this increment): the phonology substrate — phoneme inventory,
//! classes, syllable templates, deterministic phonotactic constraints, and a
//! seeded word generator. Pure, deterministic, dependency-free.

pub mod analysis;
pub mod contact;
pub mod distribution;
pub mod creative;
pub mod features;
pub mod naturalness;
pub mod pairs;
pub mod sketch;
pub mod suggest;
pub mod metrics;
pub mod trace;
pub mod universals;
pub mod diachronic;
pub mod gaps;
pub mod generate;
pub mod grammar;
pub mod grammar_check;
pub mod harmony;
pub mod interchange;
pub mod lexicon;
pub mod links;
pub mod morphology;
pub mod output;
pub mod phonology;
pub mod syntax;
pub mod translate;
pub mod types;
pub mod variety;
pub mod writing;

pub use types::{Phonology, TemplateRole};
