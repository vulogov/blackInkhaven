use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli::ExportFormat;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::export;
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};

pub fn run(
    project: &Path,
    format: ExportFormat,
    output: Option<&Path>,
    book_name: Option<&str>,
    status_floor: Option<&str>,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;

    let scope = resolve_export_scope(&h, book_name)?;
    let floor_idx = parse_status_floor(status_floor)?;
    let combined = build_combined(&layout, &h, scope.root_id, floor_idx)?;
    let epub_title = scope.title_for_epub(project);

    match format {
        ExportFormat::Typst => write_typst(&combined, output),
        ExportFormat::Pdf => write_pdf(&combined, output),
        ExportFormat::Markdown => write_artefact(
            export::build_markdown(&combined),
            output,
            "markdown",
        ),
        ExportFormat::Tex => write_artefact(
            export::build_tex(&combined),
            output,
            "tex",
        ),
        ExportFormat::Epub => {
            // Markdown is the EPUB intermediate. We re-use the same
            // typst→markdown converter so what the user sees in the
            // .md export is exactly what's inside the .epub.
            let md = export::markdown::typst_to_markdown(&combined);
            let artefact = export::build_epub(&md, &epub_title)
                .map_err(|e| Error::Store(format!("epub: {e:#}")))?;
            write_artefact(artefact, output, "epub")
        }
    }
}

/// Resolved subtree the exporter walks. `root_id = None` means
/// "whole project" (legacy 1.2.2 behaviour); `Some(id)` is a
/// single book picked via `--book-name`.
struct ExportScope<'a> {
    /// `None` → whole project; `Some(id)` → only paragraphs under
    /// that book in DFS preorder.
    root_id: Option<uuid::Uuid>,
    /// Display title used by the EPUB writer's metadata. Borrowed
    /// from the matched book when scope is single-book, otherwise
    /// derived from the project directory name at the call site.
    book_title: Option<&'a str>,
}

impl<'a> ExportScope<'a> {
    fn title_for_epub(&self, project: &Path) -> String {
        if let Some(t) = self.book_title {
            return t.to_string();
        }
        project
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("inkhaven book")
            .to_string()
    }
}

/// Match `--book-name` against the hierarchy. Without the flag,
/// "whole project" is OK only when the project has at most one
/// user book — otherwise we refuse and list the available books so
/// the user knows what to pass.
///
/// Matching tries (in order):
///   1. Case-insensitive title equality.
///   2. Case-insensitive slug equality.
/// System books (Help / Scripts / Typst / …) are excluded from
/// both the match list and the disambiguation list — those don't
/// contain the user's manuscript content.
fn resolve_export_scope<'a>(
    h: &'a Hierarchy,
    book_name: Option<&str>,
) -> Result<ExportScope<'a>> {
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
                Some(book) => Ok(ExportScope {
                    root_id: Some(book.id),
                    book_title: Some(book.title.as_str()),
                }),
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
                        "export: no book matches `--book-name {name}`. Available: {listing}"
                    )))
                }
            }
        }
        None => {
            if user_books.len() > 1 {
                let listing = user_books
                    .iter()
                    .map(|b| format!("`{}`", b.title))
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(Error::Store(format!(
                    "export: project has {n} user books — pass --book-name <name>. Available: {listing}",
                    n = user_books.len(),
                )));
            }
            // Zero or one user books: scope to the whole project,
            // preserving 1.2.2's behaviour for single-book setups
            // (and "just dump what's there" for projects that only
            // contain system books / orphans).
            let book_title = user_books.first().map(|b| b.title.as_str());
            Ok(ExportScope {
                root_id: user_books.first().map(|b| b.id),
                book_title,
            })
        }
    }
}

fn write_artefact(
    artefact: export::Artefact,
    output: Option<&Path>,
    fmt_label: &str,
) -> Result<()> {
    match output {
        Some(path) => {
            artefact.write_to(path).map_err(|e| {
                Error::Store(format!("write {fmt_label}: {e:#}"))
            })?;
            eprintln!("wrote {} ({fmt_label})", path.display());
        }
        None => match &artefact {
            export::Artefact::Markdown(s) | export::Artefact::Tex(s) => {
                print!("{s}");
            }
            export::Artefact::Epub(_) => {
                return Err(Error::Store(
                    "epub export needs --output <path.epub> (binary archive)".into(),
                ));
            }
        },
    }
    Ok(())
}

/// Concatenate every paragraph's `.typ` file in DFS preorder. Branch nodes
/// don't emit anything themselves — paragraphs carry the headings via the
/// `= Title` template `inkhaven add paragraph` writes. The user controls
/// document structure by ordering paragraphs at each level (book-level
/// paragraphs come first → that's where Typst config like `#set page(...)`
/// belongs).
///
/// `root_id = None` walks the whole hierarchy. `Some(id)` restricts the
/// walk to that book's subtree — used by `--book-name` to keep system
/// books and sibling user books out of the export.
fn build_combined(
    layout: &ProjectLayout,
    h: &Hierarchy,
    root_id: Option<uuid::Uuid>,
    status_floor: Option<usize>,
) -> Result<String> {
    export::assemble_typst_source_filtered(layout, h, root_id, status_floor)
        .map_err(|e| Error::Store(format!("assemble: {e:#}")))
}

/// Parse `--status` against the canonical workflow ladder.
/// Lowercased; returns the **index** into [`STATUS_LADDER`] (a
/// higher index = more advanced). None → no floor applied.
fn parse_status_floor(s: Option<&str>) -> Result<Option<usize>> {
    let Some(raw) = s else { return Ok(None) };
    let lowered = raw.trim().to_ascii_lowercase();
    match STATUS_LADDER
        .iter()
        .position(|name| *name == lowered.as_str())
    {
        Some(i) => Ok(Some(i)),
        None => Err(Error::Store(format!(
            "export: unknown --status `{raw}`. Valid: {}",
            STATUS_LADDER.join(", ")
        ))),
    }
}

/// Canonical status ladder, lowest → highest. Index used by
/// `--status` to compare a paragraph's status against the floor.
/// `none` is the implicit zero rung — paragraphs with no status
/// set sit there.
const STATUS_LADDER: &[&str] = &[
    "none", "napkin", "first", "second", "third", "final", "ready",
];

fn write_typst(combined: &str, output: Option<&Path>) -> Result<()> {
    match output {
        Some(path) => {
            std::fs::write(path, combined.as_bytes()).map_err(Error::Io)?;
            eprintln!("wrote {} bytes to {}", combined.len(), path.display());
        }
        None => {
            print!("{combined}");
        }
    }
    Ok(())
}

fn write_pdf(combined: &str, output: Option<&Path>) -> Result<()> {
    let output = output.ok_or_else(|| {
        Error::Store("PDF export needs --output <path.pdf>".into())
    })?;
    if crate::typst_compile::typst_external_path().is_none() {
        return Err(Error::Store(
            "the `typst` binary is not on PATH — install it from https://typst.app/ \
             or run `inkhaven export typst -o file.typ` and compile manually"
                .into(),
        ));
    }

    // Write the intermediate .typ alongside the requested PDF so the user can
    // inspect / re-compile manually if something is off.
    let typ_path: PathBuf = output.with_extension("typ");
    std::fs::write(&typ_path, combined.as_bytes()).map_err(Error::Io)?;

    let status = Command::new("typst")
        .arg("compile")
        .arg(&typ_path)
        .arg(output)
        .status()
        .map_err(|e| Error::Store(format!("failed to spawn `typst`: {e}")))?;
    if !status.success() {
        return Err(Error::Store(format!(
            "`typst compile` exited with {status}; intermediate source kept at {}",
            typ_path.display()
        )));
    }
    eprintln!("wrote {} (source: {})", output.display(), typ_path.display());
    Ok(())
}

