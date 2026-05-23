//! `inkhaven build` — run the same flow as the TUI's Ctrl+B B
//! without launching the TUI: book assembly + (optional) typst
//! compile. Useful for automation and for end-to-end tests that
//! need to verify the synthesised `settings.typ` actually
//! compiles.

use std::path::Path;

use crate::assemble;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};
use crate::typst_compile;

/// Entry point for the `inkhaven build` subcommand.
///
/// * `book_name = None` — auto-picks the single user book; errors
///   when the project has more than one user book.
/// * `book_name = Some("…")` — case-insensitive title / slug match
///   against the user books (system books are excluded).
/// * `compile = false` — assemble only (writes the artefacts tree).
/// * `compile = true` — assemble, then run `typst compile` against
///   the produced root `.typ`. Prints the stderr of typst on failure.
pub fn run(project: &Path, book_name: Option<&str>, compile: bool) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;
    let book = resolve_user_book(&h, book_name)?;

    eprintln!("Assembling `{}` (slug: {})…", book.title, book.slug);
    let mut progress = |done: usize, total: usize, file: &Path| {
        eprintln!("  [{done}/{total}] {}", file.display());
    };
    let report = assemble::assemble_book(&store, &layout, &cfg, &book, &mut progress)
        .map_err(|e| Error::Store(format!("assemble: {e:#}")))?;
    eprintln!(
        "Assembly OK · root: {} ({} files)",
        report.root_typ.display(),
        report.files_written,
    );

    if !compile {
        return Ok(());
    }

    let mut handle = typst_compile::spawn_with_config(&cfg, &report.root_typ)
        .map_err(|e| Error::Store(format!("typst spawn: {e:#}")))?;
    // Blocking wait — no spinner to drive, no Esc to listen for.
    loop {
        match handle.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(80)),
            Err(e) => {
                return Err(Error::Store(format!("typst try_wait: {e}")));
            }
        }
    }
    let outcome = typst_compile::finish(handle)
        .map_err(|e| Error::Store(format!("typst finish: {e:#}")))?;
    if outcome.success {
        println!("PDF: {}", outcome.pdf_path.display());
        Ok(())
    } else {
        let body = if outcome.stderr.trim().is_empty() {
            outcome.stdout.clone()
        } else {
            outcome.stderr.clone()
        };
        Err(Error::Store(format!(
            "typst compile failed:\n{body}"
        )))
    }
}

/// Pick the user book to assemble. Mirrors `resolve_export_scope`
/// in `cli/export.rs` but always requires a concrete user book
/// (you can't assemble "the whole project" — only a book).
fn resolve_user_book<'a>(
    h: &'a Hierarchy,
    book_name: Option<&str>,
) -> Result<Node> {
    let user_books: Vec<&Node> = h
        .children_of(None)
        .into_iter()
        .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
        .collect();

    match book_name {
        Some(name) => {
            let needle = name.trim().to_ascii_lowercase();
            let pick = user_books.iter().copied().find(|b| {
                b.title.to_ascii_lowercase() == needle
                    || b.slug.to_ascii_lowercase() == needle
            });
            match pick {
                Some(book) => Ok(book.clone()),
                None => {
                    let listing = user_books
                        .iter()
                        .map(|b| format!("`{}` (slug: {})", b.title, b.slug))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let listing = if listing.is_empty() {
                        "no user books in this project".into()
                    } else {
                        listing
                    };
                    Err(Error::Store(format!(
                        "build: no book matches `--book-name {name}`. Available: {listing}"
                    )))
                }
            }
        }
        None => match user_books.as_slice() {
            [book] => Ok((*book).clone()),
            [] => Err(Error::Store(
                "build: project has no user books — add one with `inkhaven add book <title>`".into(),
            )),
            _ => {
                let listing = user_books
                    .iter()
                    .map(|b| format!("`{}`", b.title))
                    .collect::<Vec<_>>()
                    .join(", ");
                Err(Error::Store(format!(
                    "build: project has {n} user books — pass --book-name <name>. Available: {listing}",
                    n = user_books.len(),
                )))
            }
        },
    }
}

