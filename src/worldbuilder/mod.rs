//! WBLD-1 — `inkhaven worldbuilder`: an interactive TUI companion to the
//! `realworld` pipeline and the World system book.
//!
//! A third [`crate::tui_host`] consumer beside the Research and Linguistic
//! companions. It is a *front-end*, not a replacement: every world change it
//! makes lands in `world.hjson`, compiled by the existing `realworld compile`
//! chain unchanged. It is also a world-fact research surface — facts it records
//! are tagged `fact:world` in the shared Facts book, examinable later in
//! `inkhaven research`. It never generates prose; the author decides, and the
//! worldbuilder measures, validates, and records.
//!
//! WB-P0 (this cut): the shell — four-pane frame, focus/Tab model, session
//! sidecar, CLI entry. Later phases (WB-P1…P12) add the trees, chat, plausibility
//! score, world-shaping commands, the realworld bridge, maps, world-fact
//! research, the interview, the magic-ledger editor, sessions/journey, and export.

use std::path::Path;

use anyhow::Result;

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;

mod app;
mod commands;
mod focus;
mod interview;
mod map;
mod prompt;
mod render;
mod session;

/// Parsed `inkhaven worldbuilder` flags.
pub struct WorldbuilderInvocation {
    /// Open with this named session; `None` → `default`.
    pub session: Option<String>,
    /// Jump straight to interview mode (WB-P8).
    pub interview: bool,
    /// Open map-first; until the plakat inverse flow lands, also opens the
    /// interview (WB-P8).
    pub from_map: bool,
}

/// Entry point for `Command::Worldbuilder`.
pub(crate) fn run(project: &Path, inv: WorldbuilderInvocation) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized().map_err(anyhow::Error::from)?;
    let cfg = Config::load_layered(&layout.config_path()).map_err(anyhow::Error::from)?;
    let store = Store::open(layout.clone(), &cfg).map_err(anyhow::Error::from)?;
    let hierarchy = Hierarchy::load(&store).map_err(anyhow::Error::from)?;

    launch_tui(layout, cfg, store, hierarchy, inv)
}

fn launch_tui(
    layout: ProjectLayout,
    cfg: Config,
    store: Store,
    hierarchy: Hierarchy,
    inv: WorldbuilderInvocation,
) -> Result<()> {
    // Shared lifecycle: raw mode + alternate screen + crash-restore, restored
    // however the body returns.
    crate::tui_host::with_terminal(|terminal| {
        let mut app = app::WorldbuilderApp::new(layout, cfg, store, hierarchy, inv)?;
        app.run(terminal)
    })
}
