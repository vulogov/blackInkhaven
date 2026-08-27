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
use crate::epub::{self, EpubChapter, EpubCover, EpubMeta};
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
    let cfg = Config::load_layered(&layout.config_path())?;
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

    let cover = detect_cover(&layout.root);
    if let Some(c) = &cover {
        eprintln!("  embedding cover ({}, {} KB)", c.media_type, c.bytes.len() / 1024);
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
        cover,
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

/// I-3 — build a rich, multi-chapter EPUB in memory from an already-open store +
/// book (a `nav` entry per chapter, embedded images, footnote popups, cover),
/// returning the bytes. The bytes-returning sibling of [`run`] (which writes to
/// disk) — used by the Bund `ink.export.epub` word and the TUI batch exporter so
/// they no longer go through the retired single-chapter markdown→epub toy.
pub fn build_bytes(
    store: &Store,
    h: &Hierarchy,
    cfg: &Config,
    book: &Node,
    title: Option<&str>,
    author: Option<&str>,
) -> Result<Vec<u8>> {
    let chapters = collect_chapters(store, h, book)?;
    if chapters.is_empty() {
        return Err(Error::Store(format!("epub: `{}` has no chapters to export", book.title)));
    }
    let meta = EpubMeta {
        title: title.map(str::to_string).unwrap_or_else(|| book.title.clone()),
        author: author
            .map(str::to_string)
            .or_else(|| cfg.editor.comment_author.clone())
            .unwrap_or_else(|| "Unknown Author".to_string()),
        language: crate::ai::prompts::iso_from_long(&cfg.language).to_string(),
        identifier: format!("urn:uuid:{}", Uuid::now_v7()),
        cover: detect_cover(store.project_root()),
    };
    epub::build_epub_bytes(&meta, &chapters).map_err(|e| Error::Store(format!("epub: {e:#}")))
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
        let mut images = Vec::new();
        append_branch_prose(store, h, chapter, &mut body, &mut images, false)?;
        // A3 — promote all footnote placeholders in one pass so ids are unique
        // across the whole chapter and the notes collect into a single section.
        let body = epub::finalize_footnotes(&body);
        chapters.push(EpubChapter {
            title: clean_title(&chapter.title),
            body_xhtml: body,
            images,
        });
    }
    Ok(chapters)
}

/// Walk a chapter/subchapter's children in display
/// order, appending paragraph prose + subheadings to
/// `body`.  `is_sub` adds the subchapter's own title as
/// an `<h2>` before recursing.
///
/// 1.2.22 D.3.b — kept separate from
/// `cli::audiobook::append_branch_prose` (xhtml + `<h2>` vs
/// plain text); see the rationale there.
fn append_branch_prose(
    store: &Store,
    h: &Hierarchy,
    branch: &Node,
    body: &mut String,
    images: &mut Vec<epub::EpubImage>,
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
                // A3 — footnotes are finalized once per chapter (see
                // collect_chapters), so ids stay unique across paragraphs.
                let xhtml = epub::typst_to_xhtml_pending(&raw);
                body.push_str(&xhtml);
            }
            NodeKind::Subchapter => {
                append_branch_prose(store, h, child, body, images, true)?;
            }
            // Inline figure (R-images) — embed the bytes as
            // a `<figure>`. Mirrors the R.1.b cover path,
            // but per-node and collected into the manifest.
            NodeKind::Image => {
                append_image(store, child, body, images)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Embed one `NodeKind::Image` as an EPUB inline figure:
/// read its bytes from bdslib, register an `EpubImage`
/// resource keyed by the node id (unique → no cross-chapter
/// filename collisions), and append a `<figure>` referencing
/// it. A missing-bytes image logs a warning and is skipped
/// rather than failing the whole export (matches the
/// best-effort cover behaviour).
fn append_image(
    store: &Store,
    node: &Node,
    body: &mut String,
    images: &mut Vec<epub::EpubImage>,
) -> Result<()> {
    let bytes = match store.image_bytes(node.id)? {
        Some(b) => b,
        None => {
            eprintln!(
                "epub: image `{}` has no bytes in bdslib — skipping",
                node.title
            );
            return Ok(());
        }
    };
    let ext = node.image_ext.as_deref().unwrap_or("png");
    let href = format!("img-{}.{}", node.id, ext);
    let alt = node
        .image_alt
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(&node.title);
    body.push_str(&format!(
        "<figure><img src=\"{href}\" alt=\"{alt}\"/>",
        href = epub::escape_xml(&href),
        alt = epub::escape_xml(alt),
    ));
    if let Some(cap) = node.image_caption.as_deref().filter(|s| !s.trim().is_empty()) {
        body.push_str(&format!(
            "<figcaption>{}</figcaption>",
            epub::escape_xml(cap),
        ));
    }
    body.push_str("</figure>\n");
    images.push(epub::EpubImage {
        id: format!("img-{}", node.id),
        href,
        media_type: epub::image_media_type(ext).to_string(),
        bytes,
    });
    Ok(())
}

/// 1.2.20+ R.1.b — find a cover image in the project
/// root by convention.  Checks `cover.png`, `cover.jpg`,
/// `cover.jpeg` in that order; the first that exists +
/// reads becomes the EPUB cover.  Zero-config: drop a
/// `cover.png` next to `inkhaven.hjson` and it ships.
/// A read error logs a warning and is treated as
/// "no cover" rather than failing the whole export.
fn detect_cover(root: &Path) -> Option<EpubCover> {
    for (name, ext, mt) in [
        ("cover.png", "png", "image/png"),
        ("cover.jpg", "jpg", "image/jpeg"),
        ("cover.jpeg", "jpg", "image/jpeg"),
    ] {
        let path = root.join(name);
        if !path.is_file() {
            continue;
        }
        match std::fs::read(&path) {
            Ok(bytes) if !bytes.is_empty() => {
                return Some(EpubCover {
                    bytes,
                    media_type: mt.to_string(),
                    file_ext: ext.to_string(),
                });
            }
            Ok(_) => {
                tracing::warn!(
                    target: "inkhaven::epub",
                    "cover `{}` is empty — skipping",
                    path.display(),
                );
            }
            Err(e) => {
                tracing::warn!(
                    target: "inkhaven::epub",
                    "cover `{}` unreadable ({e}) — skipping",
                    path.display(),
                );
            }
        }
    }
    None
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

    // ── detect_cover (R.1.b) ──────────────────────────

    #[test]
    fn detect_cover_none_when_absent() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(detect_cover(tmp.path()).is_none());
    }

    #[test]
    fn detect_cover_finds_png() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("cover.png"), b"\x89PNG fake").unwrap();
        let cover = detect_cover(tmp.path()).expect("cover detected");
        assert_eq!(cover.media_type, "image/png");
        assert_eq!(cover.file_ext, "png");
        assert_eq!(cover.bytes, b"\x89PNG fake");
    }

    #[test]
    fn detect_cover_prefers_png_over_jpg() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("cover.png"), b"png").unwrap();
        std::fs::write(tmp.path().join("cover.jpg"), b"jpg").unwrap();
        assert_eq!(detect_cover(tmp.path()).unwrap().media_type, "image/png");
    }

    #[test]
    fn detect_cover_jpeg_maps_to_jpeg_mime() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("cover.jpeg"), b"jpegbytes").unwrap();
        let cover = detect_cover(tmp.path()).unwrap();
        assert_eq!(cover.media_type, "image/jpeg");
        assert_eq!(cover.file_ext, "jpg");
    }

    #[test]
    fn detect_cover_skips_empty_file() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("cover.png"), b"").unwrap();
        assert!(detect_cover(tmp.path()).is_none());
    }
}
