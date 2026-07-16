//! LING-1 L-P0 — the Linguistic companion (`inkhaven linguistic`).
//!
//! A full-screen TUI, sibling to the Research companion, for developing,
//! verifying, analysing and researching a project's constructed languages. It
//! rides the shared [`crate::tui_host`] shell (terminal lifecycle + event loop)
//! and the shared [`crate::system_tree`] left-pane tree, scoped to the
//! `Language` system book.
//!
//! L-P0 is the shell: it opens the Languages book, shows the tree, previews the
//! selected node, and offers a grounded AI chat. The analysis verbs (metrics,
//! typology, the Oracle) land in later waves; the CLI `inkhaven language …`
//! family remains the non-interactive host for them.

use std::path::Path;

use anyhow::Result;

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::Store;

mod app;

pub use app::LinguisticApp;

/// Arguments for `inkhaven linguistic` (kept minimal for L-P0; grows with the
/// non-interactive paths in later waves).
#[derive(Debug, Default)]
pub struct LinguisticInvocation {
    /// `--language <name>`: open with this language selected. Without it, the
    /// tree opens at the Languages book root.
    pub language: Option<String>,
    /// `--session <name>`: open (or create) a named chat session. Defaults to
    /// `default`. Wired into the grounded chat in the next L-P0 increment.
    #[allow(dead_code)]
    pub session: Option<String>,
}

/// Entry point for the `linguistic` subcommand.
pub(crate) fn run(project: &Path, inv: LinguisticInvocation) -> Result<()> {
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
    inv: LinguisticInvocation,
) -> Result<()> {
    // Shared lifecycle: raw mode + alternate screen + crash-restore, restored
    // however the body returns.
    crate::tui_host::with_terminal(|terminal| {
        let mut app = LinguisticApp::new(layout, cfg, store, hierarchy, inv)?;
        app.run(terminal)
    })
}
