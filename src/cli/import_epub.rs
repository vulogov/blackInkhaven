//! `inkhaven import-epub <file.epub>` — import an EPUB as a user book.
//!
//! The inverse of `inkhaven epub`. Loads the project store, runs the
//! `epub_import` orchestrator, prints a summary, and (per the H4
//! stability lesson) exits non-zero when any item failed so a scripted
//! import can tell a clean run from a partial one.

use std::path::Path;

use crate::config::Config;
use crate::epub_import::{import_epub, EpubImportOpts};
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::Store;

pub fn run(
    project: &Path,
    epub_path: &Path,
    book_name: Option<&str>,
    dry_run: bool,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;

    let opts = EpubImportOpts {
        book_name: book_name.map(|s| s.to_string()),
        dry_run,
    };
    let report = import_epub(epub_path, &store, &cfg, &opts)
        .map_err(|e| Error::Store(format!("import-epub: {e:#}")))?;

    println!(
        "EPUB import {} — book `{}`:\n  chapters:   {}\n  paragraphs: {}\n  images:     {} imported, {} extracted",
        if dry_run { "(dry-run)" } else { "complete" },
        report.book_title,
        report.chapters_created,
        report.paragraphs_created,
        report.images_imported,
        report.images_extracted,
    );
    if let Some(author) = &report.author {
        println!("  author:     {author}  (set your book metadata / manuscript author to match)");
    }
    if report.images_extracted > 0 && !dry_run {
        println!(
            "  ({} unreferenced image(s) extracted to `{}-images/` for hand-placement)",
            report.images_extracted,
            slug::slugify(&report.book_title)
        );
    }
    if !report.errors.is_empty() {
        println!("  errors ({}):", report.errors.len());
        for e in &report.errors {
            println!("    · {e}");
        }
        // H4 — non-zero exit on partial failure.
        if !dry_run {
            return Err(Error::Store(format!(
                "import-epub: {} item(s) failed — see errors above",
                report.errors.len()
            )));
        }
    }
    Ok(())
}
