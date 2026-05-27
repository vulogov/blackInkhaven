//! 1.2.10+ — standalone TUI configuration editor.
//!
//! Launched as `inkhaven config -p <dir>`.  Provides a
//! schema-aware, tree-pane + edit-pane view of
//! `<dir>/inkhaven.hjson`.
//!
//! **Phase 1**: read-only walk-through — tree + detail
//! pane + help pane + unknown-fields chip.  No widgets
//! that mutate, no save, no backup.
//!
//! See `Documentation/PROPOSALS/CONFIG_TUI.md` for the
//! full design.

mod app;
mod help;
mod hjson_index;
mod save;
mod schema;
mod widgets;

use std::path::Path;

use anyhow::Result;

/// Entry point — called from the `inkhaven config`
/// subcommand dispatcher.  Initialises the terminal,
/// builds the schema, parses the live HJSON, runs the
/// event loop, restores the terminal on exit.
pub fn run(project: &Path) -> Result<()> {
    app::run(project)
}
