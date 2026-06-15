//! 1.3.1 SUBMISSION-1 — `inkhaven docx` subcommand.
//!
//! Exports a user book to a Shunn-format Word document (`.docx`) — the
//! format agents actually require.  Same model as `inkhaven manuscript`
//! (the shared [`crate::cli::manuscript::build_model`]), emitted as
//! hand-rolled OOXML instead of typst.
//!
//! ```bash
//! $ inkhaven docx --book-name "My Novel" --author "Jane Author" \
//!     --contact "Jane Author\n12 Wharf Lane\njane@example.com" \
//!     --font times --output my-novel.docx
//! ```

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::{Error, Result};
use crate::export::docx::{build_docx, DocxFont};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;

pub fn run(
    project: &Path,
    book_name: Option<&str>,
    output: Option<&Path>,
    title: Option<&str>,
    author: Option<&str>,
    contact: Option<&str>,
    font: Option<&str>,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)
        .map_err(|e| Error::Store(e.to_string()))?;
    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    let book = crate::cli::resolve_user_book(&h, book_name, "docx")
        .map_err(Error::Store)?
        .clone();

    let font = match font {
        Some(f) => DocxFont::parse(f).ok_or_else(|| {
            Error::Store(format!("docx: unknown --font `{f}` (times|courier)"))
        })?,
        None => DocxFont::TimesNewRoman,
    };

    let (meta, chapters) =
        crate::cli::manuscript::build_model(&layout, &cfg, &h, &book, title, author, contact)?;
    let bytes =
        build_docx(&meta, &chapters, font).map_err(|e| Error::Store(e.to_string()))?;

    let dest = output.map(PathBuf::from).unwrap_or_else(|| {
        layout.root.join(format!("{}-manuscript.docx", book.slug))
    });
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(Error::Io)?;
    }
    crate::io_atomic::write(&dest, &bytes).map_err(Error::Io)?;

    println!(
        "Docx: {} ({} chapters · {} words, ~{} rounded)",
        dest.display(),
        chapters.len(),
        meta.word_count,
        crate::manuscript::round_word_count(meta.word_count),
    );
    Ok(())
}
