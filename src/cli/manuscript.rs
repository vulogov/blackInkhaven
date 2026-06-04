//! 1.2.19+ X.1 — `inkhaven manuscript` subcommand.
//!
//! Exports a user book to a submission-ready
//! Shunn-standard-manuscript-format typst document.  The
//! finishing-line companion to the reader-facing ePub +
//! audiobook exports.
//!
//! ```bash
//! $ inkhaven manuscript --book-name "My Novel" \
//!     --author "Jane Author" \
//!     --contact "Jane Author\n12 Wharf Lane\njane@example.com" \
//!     --output my-novel-submission.typ
//! ```
//!
//! Compile to PDF with `typst compile <out>.typ` (the
//! document is self-contained — no project imports).

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::{Error, Result};
use crate::manuscript::{
    build_typst, ManuscriptChapter, ManuscriptMeta,
};
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
    contact: Option<&str>,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)
        .map_err(|e| Error::Store(e.to_string()))?;
    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    let book = resolve_user_book(&h, book_name)?;

    let chapters = collect_chapters(&layout, &h, &book);
    if chapters.is_empty() {
        return Err(Error::Store(format!(
            "manuscript: `{}` has no chapters to export",
            book.title,
        )));
    }

    // Word count across the prose (scene-break markers
    // excluded).
    let word_count: usize = chapters
        .iter()
        .flat_map(|c| &c.paragraphs)
        .filter(|p| !crate::manuscript::is_scene_break(p))
        .map(|p| crate::progress::count_words(p).max(0) as usize)
        .sum();

    let author_name = author
        .map(str::to_string)
        .or_else(|| cfg.editor.comment_author.clone())
        .unwrap_or_else(|| "Author Name".to_string());
    let surname = author_name
        .split_whitespace()
        .last()
        .unwrap_or("Author")
        .to_string();
    let book_title = title
        .map(str::to_string)
        .unwrap_or_else(|| crate::cli::epub::clean_title(&book.title));

    let meta = ManuscriptMeta {
        title: book_title,
        contact: contact
            .map(|c| c.replace("\\n", "\n"))
            .unwrap_or_else(|| author_name.clone()),
        byline: author_name,
        surname,
        word_count,
    };

    let typst = build_typst(&meta, &chapters);

    let dest = output.map(PathBuf::from).unwrap_or_else(|| {
        layout.root.join(format!("{}-manuscript.typ", book.slug))
    });
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(Error::Io)?;
    }
    crate::io_atomic::write(&dest, typst.as_bytes()).map_err(Error::Io)?;

    println!(
        "Manuscript: {} ({} chapters · {} words, ~{} rounded)\n\
         compile to PDF with: typst compile {}",
        dest.display(),
        chapters.len(),
        word_count,
        crate::manuscript::round_word_count(word_count),
        dest.display(),
    );
    Ok(())
}

fn collect_chapters(
    layout: &ProjectLayout,
    h: &Hierarchy,
    book: &Node,
) -> Vec<ManuscriptChapter> {
    let mut out = Vec::new();
    for chapter in h.children_of(Some(book.id)) {
        if chapter.kind != NodeKind::Chapter {
            continue;
        }
        let mut paragraphs = Vec::new();
        gather_paragraphs(layout, h, chapter, &mut paragraphs);
        if !paragraphs.is_empty() {
            out.push(ManuscriptChapter {
                title: crate::cli::epub::clean_title(&chapter.title),
                paragraphs,
            });
        }
    }
    out
}

fn gather_paragraphs(
    layout: &ProjectLayout,
    h: &Hierarchy,
    branch: &Node,
    out: &mut Vec<String>,
) {
    for child in h.children_of(Some(branch.id)) {
        match child.kind {
            NodeKind::Paragraph => {
                let Some(rel) = child.file.as_ref() else { continue };
                let abs = layout.root.join(rel);
                if let Ok(text) = std::fs::read_to_string(&abs) {
                    let plain = crate::audiobook::typst_to_plain(&text);
                    if !plain.trim().is_empty() {
                        out.push(plain.trim().to_string());
                    }
                }
            }
            NodeKind::Subchapter => {
                gather_paragraphs(layout, h, child, out);
            }
            _ => {}
        }
    }
}

/// Resolve the user book.  Mirrors `cli::epub` /
/// `cli::audiobook`.
fn resolve_user_book(h: &Hierarchy, book_name: Option<&str>) -> Result<Node> {
    let user_books: Vec<&Node> = h
        .children_of(None)
        .into_iter()
        .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
        .collect();

    match book_name {
        Some(name) => {
            let needle = name.trim().to_ascii_lowercase();
            user_books
                .iter()
                .copied()
                .find(|b| {
                    b.title.to_ascii_lowercase() == needle
                        || b.slug.to_ascii_lowercase() == needle
                })
                .cloned()
                .ok_or_else(|| {
                    Error::Store(format!(
                        "manuscript: no book matches `--book-name {name}`"
                    ))
                })
        }
        None => match user_books.as_slice() {
            [book] => Ok((*book).clone()),
            [] => Err(Error::Store(
                "manuscript: project has no user books".into(),
            )),
            _ => Err(Error::Store(format!(
                "manuscript: project has {} user books — pass --book-name",
                user_books.len(),
            ))),
        },
    }
}
