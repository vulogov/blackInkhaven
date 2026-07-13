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
mod command;
pub(crate) mod deadlinks;
mod extract;
mod factcheck;
mod facts_tree;
mod focus;
mod imports;
mod insert;
mod llm;
mod picker;
mod provenance;
mod rag;
mod render;
mod scholarly;
mod thread;
mod verdicts;
mod verify;
mod sync;
mod web;
mod wikidata;

mod batch;
mod geonames;
mod gutenberg;
mod archive;
mod wikisource;
mod contradiction;

pub(crate) use focus::Focus;

/// RESRCH-UNDISPUTED — the tag marking a Facts paragraph as an authorial
/// ("undisputed") fact: glyphed in the tree, excluded from `/factcheck`, checked
/// by `/undisputed`. A `Node.tags` value; see the RESRCH-UNDISPUTED track.
pub(super) const UNDISPUTED_TAG: &str = "fact:undisputed";

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
    /// RESRCH-2 (R2-B) — `--import <path>`: ingest a document non-interactively.
    pub import: Option<String>,
    /// RESRCH-3 (R3-D) — `--sync <folder>`: register a folder for
    /// re-import-on-change and import it now.
    pub sync: Option<String>,
    /// RESRCH-2 (R2-F) — `--batch <file>`: research a question list headlessly.
    pub batch: Option<String>,
    /// R2-F — `--auto-confirm`: insert facts clearing the confidence threshold.
    pub auto_confirm: bool,
    /// R2-F — `--confidence <0..1>`: the auto-insert threshold (default 0.7).
    pub confidence: Option<f64>,
    /// RESRCH-5 (R5-D) — `--bibliography`: emit the Sources Research chapter as
    /// BibTeX (`--out` file, else stdout) and exit.
    pub bibliography: bool,
    /// RESRCH-GUTENBERG (PG-P2) — `--gutenberg <query|PG#>`: ingest a public-domain
    /// Project Gutenberg book non-interactively and exit.
    pub gutenberg: Option<String>,
    /// RESRCH-ARCHIVE — `--archive <query>`: ingest a public-domain Internet Archive
    /// text non-interactively and exit.
    pub archive: Option<String>,
    /// RESRCH-WIKISOURCE — `--wikisource <query>`: ingest a public-domain Wikisource
    /// page (book language) non-interactively and exit.
    pub wikisource: Option<String>,
    /// SCHOLAR P1 — `--contradict`: scan the Facts book for source-attributed
    /// contradictions non-interactively and exit.
    pub contradict: bool,
}

/// Launch the Research Assistant, or run a non-interactive thread operation.
pub(crate) fn run(project: &Path, inv: ResearchInvocation) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized().map_err(anyhow::Error::from)?;
    let cfg = Config::load_layered(&layout.config_path()).map_err(anyhow::Error::from)?;

    // Non-interactive paths (R-P19 fleshes these out over the thread store).
    if let Some(path) = inv.import.as_deref() {
        let store = Store::open(layout.clone(), &cfg).map_err(anyhow::Error::from)?;
        return app::import_cli(&layout, &cfg, &store, path);
    }
    if let Some(folder) = inv.sync.as_deref() {
        let store = Store::open(layout.clone(), &cfg).map_err(anyhow::Error::from)?;
        return sync_cli(&layout, &cfg, &store, folder);
    }
    if let Some(query) = inv.gutenberg.as_deref() {
        let store = Store::open(layout.clone(), &cfg).map_err(anyhow::Error::from)?;
        return app::gutenberg_cli(&layout, &cfg, &store, query);
    }
    if let Some(query) = inv.archive.as_deref() {
        let store = Store::open(layout.clone(), &cfg).map_err(anyhow::Error::from)?;
        return app::archive_cli(&layout, &cfg, &store, query);
    }
    if let Some(query) = inv.wikisource.as_deref() {
        let store = Store::open(layout.clone(), &cfg).map_err(anyhow::Error::from)?;
        return app::wikisource_cli(&layout, &cfg, &store, query);
    }
    if inv.contradict {
        let store = Store::open(layout.clone(), &cfg).map_err(anyhow::Error::from)?;
        return app::contradict_cli(&layout, &cfg, &store);
    }
    if inv.bibliography {
        let store = Store::open(layout.clone(), &cfg).map_err(anyhow::Error::from)?;
        let hierarchy = Hierarchy::load(&store).map_err(anyhow::Error::from)?;
        let entries = app::collect_research_bibentries(&store, &hierarchy);
        let (bibtex, n) = crate::sources::compile_bibtex(&entries);
        match inv.out.as_deref() {
            Some(p) => {
                std::fs::write(p, &bibtex).map_err(|e| anyhow::anyhow!("write {p}: {e}"))?;
                eprintln!("wrote {n} entr{} → {p}", if n == 1 { "y" } else { "ies" });
            }
            None => print!("{bibtex}"),
        }
        return Ok(());
    }
    if let Some(bpath) = inv.batch.as_deref() {
        let store = Store::open(layout.clone(), &cfg).map_err(anyhow::Error::from)?;
        return batch::run(
            &layout,
            &cfg,
            &store,
            bpath,
            inv.auto_confirm,
            inv.confidence.unwrap_or(0.7),
            inv.out.as_deref(),
        );
    }
    if inv.list_threads {
        return app::list_threads_cli(&layout, inv.format.as_deref());
    }
    if let Some(name) = inv.export_thread.as_deref() {
        return app::export_thread_cli(&layout, name, inv.format.as_deref(), inv.out.as_deref());
    }

    let store = Store::open(layout.clone(), &cfg).map_err(anyhow::Error::from)?;
    let hierarchy = Hierarchy::load(&store).map_err(anyhow::Error::from)?;

    // R3-D — re-import any synced folder whose newest file changed since last sync.
    reimport_changed_folders(&layout, &cfg, &store);

    launch_tui(layout, cfg, store, hierarchy, inv.thread)
}

/// R3-D — `--sync <folder>`: register the folder and import it now.
fn sync_cli(layout: &ProjectLayout, cfg: &Config, store: &Store, folder: &str) -> Result<()> {
    let now = chrono::Utc::now().timestamp();
    let abs = sync::SyncManifest::register(layout, folder, now)?;
    app::import_cli(layout, cfg, store, &abs)?;
    println!("synced folder registered — re-imported on change at each launch");
    Ok(())
}

/// R3-D — on launch, re-import registered folders whose newest importable file is
/// newer than the last sync (idempotent; folder import refreshes same-named
/// sources). Best-effort — never blocks launch.
fn reimport_changed_folders(layout: &ProjectLayout, cfg: &Config, store: &Store) {
    let manifest = sync::SyncManifest::load(layout);
    for (abs, last_sync) in &manifest.folders {
        let path = std::path::Path::new(abs);
        if !path.is_dir() {
            continue;
        }
        if sync::newest_mtime(path) > *last_sync {
            if app::import_cli(layout, cfg, store, abs).is_ok() {
                sync::SyncManifest::mark_synced(layout, abs, chrono::Utc::now().timestamp());
            }
        }
    }
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
