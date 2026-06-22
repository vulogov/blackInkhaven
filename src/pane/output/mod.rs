//! PANE-1 — the Output pane (RFC PANE-1, `Documentation/PROPOSALS/PANE-1_PLAN.md`).
//!
//! A right-side pane for structured, one-way notifications from every subsystem
//! (translation, Bund `print`/`log`, lexicon proposals, variety renderings, and
//! future world fact-checking), complementing the conversational AI pane. This
//! module is the data layer: the universal [`Message`] envelope and the
//! per-project DuckDB-backed [`OutputStore`]. The ratatui pane widget, the
//! `Ctrl+B Tab` cycling chord, and the `ink.io.*` Bund surface build on it.

pub mod install;
pub mod store;
pub mod types;

pub use install::{active, install, uninstall};
pub use store::OutputStore;
pub use types::{kinds, ActionId, Lifetime, Message, Severity};
