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
pub(crate) mod rag;
mod render;
mod scholarly;
mod thread;
mod verdicts;
mod verify;
mod sync;
mod web;
mod wikidata;

mod agentic;
mod batch;
mod snowball;
mod geonames;
mod gutenberg;
mod archive;
mod wikisource;
/// SCRIPTURE — the /bible · /quran · /bookofmormon adapters; `pub(crate)` so the
/// Index Locorum can reuse the Bible book-name canonicalizer for loci.
pub(crate) mod scripture;
/// SCHOLAR — the contradiction/relation engine; `pub(crate)` so the manuscript
/// editor (`Ctrl+V ?` confront) can reuse the graded judge.
pub(crate) mod contradiction;
mod scholar_report;
mod socrates;

pub(crate) use focus::Focus;

/// RESRCH-UNDISPUTED — the tag marking a Facts paragraph as an authorial
/// ("undisputed") fact: glyphed in the tree, excluded from `/factcheck`, checked
/// by `/undisputed`. A `Node.tags` value; see the RESRCH-UNDISPUTED track.
pub(super) const UNDISPUTED_TAG: &str = "fact:undisputed";

/// RESRCH-6 (R6-P5) — the tag marking an agentic-emitted fact as *reviewed*: the
/// author has triaged it, so it leaves the `/review` queue and never reappears.
/// A `Node.tags` value; the queue = agentic facts without this tag.
pub(super) const REVIEWED_TAG: &str = "fact:reviewed";

use std::path::Path;

use anyhow::Result;

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
    /// RESRCH-6 — `--agentic <topic>`: autonomously decompose a topic and emit
    /// the findings as Facts into the Facts book (gated by `research.agentic`).
    pub agentic: Option<String>,
    /// RESRCH-6 (snowball) — `--snowball <seed>`: follow a seed paper's citations
    /// (backward references + forward citers) on OpenAlex and report the
    /// neighborhood.
    pub snowball: Option<String>,
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
    /// RESRCH-SCRIPTURE — `--bible <ref>`: ingest a public-domain Bible passage
    /// (by project language) non-interactively and exit.
    pub bible: Option<String>,
    /// RESRCH-SCRIPTURE — `--quran <surah>`: ingest a public-domain Qur'an surah.
    pub quran: Option<String>,
    /// RESRCH-SCRIPTURE — `--bookofmormon <ref>`: ingest a public-domain Book of
    /// Mormon passage.
    pub bookofmormon: Option<String>,
    /// SCHOLAR P1 — `--contradict`: scan the Facts book for source-attributed
    /// contradictions non-interactively and exit.
    pub contradict: bool,
    /// SCHOLAR — `--converge`: scan the Facts book for converging (triangulated)
    /// evidence non-interactively and exit.
    pub converge: bool,
    /// SCHOLAR — `--socrates [topic]`: the Dialectician's Socratic questions over
    /// the Facts corpus, non-interactively.
    pub socrates: Option<String>,
    /// SCHOLAR P3 — `--report`: print the persisted, topic-clustered report of the
    /// accumulated contradiction / convergence / relation findings, and exit.
    pub report: bool,
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
    if let Some(query) = inv.bible.as_deref() {
        let store = Store::open(layout.clone(), &cfg).map_err(anyhow::Error::from)?;
        return app::scripture_cli(&layout, &cfg, &store, scripture::Work::Bible, query);
    }
    if let Some(query) = inv.quran.as_deref() {
        let store = Store::open(layout.clone(), &cfg).map_err(anyhow::Error::from)?;
        return app::scripture_cli(&layout, &cfg, &store, scripture::Work::Quran, query);
    }
    if let Some(query) = inv.bookofmormon.as_deref() {
        let store = Store::open(layout.clone(), &cfg).map_err(anyhow::Error::from)?;
        return app::scripture_cli(&layout, &cfg, &store, scripture::Work::BookOfMormon, query);
    }
    if inv.contradict {
        let store = Store::open(layout.clone(), &cfg).map_err(anyhow::Error::from)?;
        return app::contradict_cli(&layout, &cfg, &store, false);
    }
    if inv.converge {
        let store = Store::open(layout.clone(), &cfg).map_err(anyhow::Error::from)?;
        return app::contradict_cli(&layout, &cfg, &store, true);
    }
    if let Some(topic) = inv.socrates.as_deref() {
        let store = Store::open(layout.clone(), &cfg).map_err(anyhow::Error::from)?;
        return app::socrates_cli(&layout, &cfg, &store, topic);
    }
    if inv.report {
        let store = Store::open(layout.clone(), &cfg).map_err(anyhow::Error::from)?;
        return app::report_cli(&layout, &cfg, &store);
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
    // RESRCH-6 — `--agentic <topic>`: autonomous deep research → Facts book.
    if let Some(topic) = inv.agentic.as_deref() {
        let store = Store::open(layout.clone(), &cfg).map_err(anyhow::Error::from)?;
        return agentic::run(&layout, &cfg, &store, topic, inv.out.as_deref());
    }
    // RESRCH-6 (snowball) — `--snowball <seed>`: follow a paper's citations.
    if let Some(seed) = inv.snowball.as_deref() {
        return snowball::run(&cfg, seed, inv.out.as_deref());
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
    // The raw-mode / alternate-screen lifecycle + crash-restore hook are shared
    // with the other companion TUIs; only the picker + app wiring is ours.
    crate::tui_host::with_terminal(|terminal| {
        // R-P3: resolve which thread to open (the picker fires for >1 thread when
        // no --thread was given). `None` → the user cancelled; exit cleanly.
        match picker::resolve_thread(terminal, &layout, thread)? {
            Some(name) => {
                let mut app = ResearchApp::new(layout, cfg, store, hierarchy, Some(name))?;
                app.run(terminal)
            }
            None => Ok(()),
        }
    })
}
