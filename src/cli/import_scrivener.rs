//! `inkhaven import-scrivener` — pull a Scrivener `.scriv`
//! package into the open project (1.2.4+).
//!
//! Pure CLI shim around `crate::scrivener::import_scrivener_project`.
//! See `src/scrivener/` for the layered guts.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::scrivener::{import_scrivener_project, ImportOpts};
use crate::store::Store;

pub fn run(
    project: &Path,
    scriv_path: &Path,
    draft_as_book: Option<&str>,
    skip_research: bool,
    dry_run: bool,
) -> Result<()> {
    if !scriv_path.is_dir() {
        return Err(Error::Store(format!(
            "Scrivener path `{}` is not a directory — pass the `.scriv` \
             package directory (Apple package format).",
            scriv_path.display()
        )));
    }
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;

    let opts = ImportOpts {
        draft_as_book: draft_as_book.map(str::to_owned),
        skip_research,
        dry_run,
    };
    let report = import_scrivener_project(scriv_path, &store, &cfg, &opts)
        .map_err(|e| Error::Store(format!("import-scrivener: {e:#}")))?;

    println!(
        "Scrivener import {}:\n  books: {}\n  chapters: {}\n  subchapters: {}\n  paragraphs: {}\n  skipped: {}",
        if dry_run { "(dry-run)" } else { "complete" },
        report.books_created,
        report.chapters_created,
        report.subchapters_created,
        report.paragraphs_created,
        report.paragraphs_skipped,
    );
    if !report.errors.is_empty() {
        println!("  errors ({}):", report.errors.len());
        for err in &report.errors {
            println!("    · {err}");
        }
    }
    Ok(())
}
