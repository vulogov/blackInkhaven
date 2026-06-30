//! RESRCH-1 — the Research Assistant (`inkhaven research`). A separate TUI
//! application screen for AI-assisted research that transfers verified findings
//! into the Facts (ground truth) or Notes (speculative) system books, with a
//! mandatory confirmation step. It shares the project's on-disk state (the
//! document store, the HNSW index, the Facts / Notes book files, `.inkhaven/`)
//! but has its own event loop, layout, and keymap — no shared Rust state with
//! the writing mode.
//!
//! Architecture (per the RESRCH-1 audit): the event loop is the same
//! **synchronous crossterm `poll()`/`read()`** loop the writing TUI uses;
//! streaming reuses `ai::stream::spawn_chat_stream` →
//! `tokio::sync::mpsc::UnboundedReceiver<StreamMsg>` drained with `try_recv()`
//! each tick. The tokio runtime Handle is already entered in `main()`, so the
//! stream task spawns cleanly. Facts live in the Facts *book* indexed by the
//! shared HNSW — there is no `facts.duckdb`.
//!
//! R-P1 — the entry point: terminal lifecycle, the minimum-width guard, the
//! outer layout skeleton (placeholder panes), and `q` / `Ctrl+C` exit.

mod app;
mod chat;
mod facts_tree;
mod focus;
mod insert;
mod llm;
mod picker;
mod render;
mod thread;

pub(crate) use focus::Focus;

use std::io;
use std::path::Path;

use anyhow::Result;
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;

use app::ResearchApp;

/// How `inkhaven research` was invoked (parsed from the CLI flags).
pub(crate) struct ResearchInvocation {
    pub thread: Option<String>,
    pub list_threads: bool,
    pub export_thread: Option<String>,
    pub format: Option<String>,
    pub out: Option<String>,
}

/// The minimum terminal width the main layout renders at; below it, a resize
/// message is shown instead (RFC §4).
pub(crate) const MIN_WIDTH: u16 = 80;

/// Launch the Research Assistant, or run a non-interactive thread operation.
pub(crate) fn run(project: &Path, inv: ResearchInvocation) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized().map_err(anyhow::Error::from)?;
    let cfg = Config::load_layered(&layout.config_path()).map_err(anyhow::Error::from)?;

    // Non-interactive paths (R-P19 fleshes these out over the thread store).
    if inv.list_threads {
        return app::list_threads_cli(&layout, inv.format.as_deref());
    }
    if let Some(name) = inv.export_thread.as_deref() {
        return app::export_thread_cli(&layout, name, inv.format.as_deref(), inv.out.as_deref());
    }

    let store = Store::open(layout.clone(), &cfg).map_err(anyhow::Error::from)?;
    let hierarchy = Hierarchy::load(&store).map_err(anyhow::Error::from)?;

    launch_tui(layout, cfg, store, hierarchy, inv.thread)
}

/// Set up the terminal, run the event loop, and restore the terminal on exit
/// (or panic). Mirrors the writing TUI's lifecycle, minus mouse capture (the
/// research mode is keyboard-only).
fn launch_tui(
    layout: ProjectLayout,
    cfg: Config,
    store: Store,
    hierarchy: Hierarchy,
    thread: Option<String>,
) -> Result<()> {
    crate::crash::set_terminal_restore(Some(Box::new(|| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    })));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // R-P3: resolve which thread to open (the picker fires for >1 thread when
    // no --thread was given). `None` → the user cancelled; exit cleanly.
    let result = match picker::resolve_thread(&mut terminal, &layout, thread)? {
        Some(name) => {
            let mut app = ResearchApp::new(layout, cfg, store, hierarchy, Some(name))?;
            app.run(&mut terminal)
        }
        None => Ok(()),
    };

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    crate::crash::set_terminal_restore(None);

    result
}
