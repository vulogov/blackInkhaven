//! 1.2.18+ R.1 — `inkhaven epub` subcommand.
//!
//! Exports a user book to a standards-compliant EPUB 3
//! file.  Walks the book's chapters in display order,
//! gathers each chapter's prose (its descendant
//! paragraphs, in order), converts typst → XHTML, and
//! assembles the container via `crate::epub`.
//!
//! ```bash
//! $ inkhaven epub --book-name "My Novel" --output my-novel.epub
//! ```
//!
//! Author + title default sensibly (book title;
//! `editor.comment_author`); `--author` / `--title`
//! override.  Language is mapped from the project's
//! `language` field to an ISO code.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::config::Config;
use crate::epub::{self, EpubChapter, EpubMeta};
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};

pub fn run(
    project: &Path,
    book_name: Option<&str>,
    output: Option<&Path>,
    title: Option<&str>,
    author: Option<&str>,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;
    let book = crate::cli::resolve_user_book(&h, book_name, "epub")
        .map_err(Error::Store)?
        .clone();

    let chapters = collect_chapters(&store, &h, &book)?;
    if chapters.is_empty() {
        return Err(Error::Store(format!(
            "epub: `{}` has no chapters to export",
            book.title,
        )));
    }

    let meta = EpubMeta {
        title: title
            .map(str::to_string)
            .unwrap_or_else(|| book.title.clone()),
        author: author
            .map(str::to_string)
            .or_else(|| cfg.editor.comment_author.clone())
            .unwrap_or_else(|| "Unknown Author".to_string()),
        language: crate::ai::prompts::iso_from_long(&cfg.language).to_string(),
        identifier: format!("urn:uuid:{}", Uuid::now_v7()),
    };

    let dest = output
        .map(PathBuf::from)
        .unwrap_or_else(|| layout.root.join(format!("{}.epub", book.slug)));

    eprintln!(
        "Exporting `{}` → EPUB ({} chapter{})…",
        book.title,
        chapters.len(),
        if chapters.len() == 1 { "" } else { "s" },
    );
    let report = epub::write_epub(&meta, &chapters, &dest)
        .map_err(|e| Error::Store(format!("epub write: {e:#}")))?;

    println!(
        "EPUB: {} ({} chapters · {} KB)",
        dest.display(),
        report.chapters,
        report.bytes / 1024,
    );
    Ok(())
}

/// Gather one `EpubChapter` per top-level Chapter under
/// `book`, in display order.  Each chapter's body is the
/// concatenated XHTML of its descendant paragraphs (in
/// pre-order), with subchapter titles surfaced as `<h2>`.
fn collect_chapters(
    store: &Store,
    h: &Hierarchy,
    book: &Node,
) -> Result<Vec<EpubChapter>> {
    let mut chapters = Vec::new();
    for chapter in h.children_of(Some(book.id)) {
        if chapter.kind != NodeKind::Chapter {
            continue;
        }
        let mut body = String::new();
        append_branch_prose(store, h, chapter, &mut body, false)?;
        chapters.push(EpubChapter {
            title: clean_title(&chapter.title),
            body_xhtml: body,
        });
    }
    Ok(chapters)
}

/// Walk a chapter/subchapter's children in display
/// order, appending paragraph prose + subheadings to
/// `body`.  `is_sub` adds the subchapter's own title as
/// an `<h2>` before recursing.
fn append_branch_prose(
    store: &Store,
    h: &Hierarchy,
    branch: &Node,
    body: &mut String,
    is_sub: bool,
) -> Result<()> {
    if is_sub {
        body.push_str(&format!(
            "<h2>{}</h2>\n",
            epub::escape_xml(&clean_title(&branch.title)),
        ));
    }
    for child in h.children_of(Some(branch.id)) {
        match child.kind {
            NodeKind::Paragraph => {
                let raw = read_paragraph(store, child)?;
                let xhtml = epub::typst_to_xhtml(&raw);
                body.push_str(&xhtml);
            }
            NodeKind::Subchapter => {
                append_branch_prose(store, h, child, body, true)?;
            }
            // Images are skipped in R.1 (cover + inline
            // image support is an R.1.b polish).
            _ => {}
        }
    }
    Ok(())
}

fn read_paragraph(store: &Store, node: &Node) -> Result<String> {
    let bytes = store
        .get_content(node.id)
        .map_err(|e| Error::Store(format!("epub: read paragraph {}: {e}", node.id)))?
        .unwrap_or_default();
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Strip a leading `NN. ` ordinal prefix from a node
/// title (gen-fixture + many real projects prefix chapter
/// titles with their order).  `Chapter 3: The Box` →
/// `The Box`; `001. Approach` → `Approach`.  Best-effort
/// + conservative — only strips recognised patterns.
/// Shared with `cli::audiobook` (R.2).
pub(crate) fn clean_title(title: &str) -> String {
    let t = title.trim();
    // `Chapter N: Rest` → `Rest`.
    if let Some(idx) = t.find(": ") {
        let head = &t[..idx];
        if head
            .to_ascii_lowercase()
            .starts_with("chapter")
        {
            return t[idx + 2..].trim().to_string();
        }
    }
    // `NNN. Rest` → `Rest`.
    if let Some(idx) = t.find(". ") {
        let head = &t[..idx];
        if !head.is_empty() && head.chars().all(|c| c.is_ascii_digit()) {
            return t[idx + 2..].trim().to_string();
        }
    }
    t.to_string()
}

/// Resolve the user book to export.  Mirrors
/// `cli::build::resolve_user_book`.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_title_strips_chapter_prefix() {
        assert_eq!(clean_title("Chapter 3: The Lacquered Box"), "The Lacquered Box");
        assert_eq!(clean_title("Chapter 12: Crossing"), "Crossing");
    }

    #[test]
    fn clean_title_strips_numeric_prefix() {
        assert_eq!(clean_title("001. Approach"), "Approach");
        assert_eq!(clean_title("42. Reverie"), "Reverie");
    }

    #[test]
    fn clean_title_leaves_plain_titles() {
        assert_eq!(clean_title("The Wharf"), "The Wharf");
        // A colon that isn't a chapter prefix stays.
        assert_eq!(clean_title("A Tale: Continued"), "A Tale: Continued");
    }

    #[test]
    fn clean_title_trims_whitespace() {
        assert_eq!(clean_title("  Padded  "), "Padded");
    }
}
