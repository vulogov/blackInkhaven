//! EPUB import P3 — orchestrator.
//!
//! Parses the package, then materialises one user **Book** → a
//! **Chapter** per spine document → one **Paragraph** holding that
//! chapter's converted prose, mirroring `crate::scrivener::import`'s
//! `Store::create_node` + `io_atomic::write` + `update_paragraph_content`
//! flow. Manifest images are extracted to a sidecar folder; their
//! in-prose references become comments so nothing breaks compilation.
//!
//! Per-item failures are collected in the report — the import never
//! aborts on a single bad chapter.

use std::path::Path;

use anyhow::{Context, Result};
use uuid::Uuid;

use super::package::EpubArchive;
use super::xhtml::xhtml_to_typst;
use crate::config::Config;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;
use crate::store::{InsertPosition, Store};

#[derive(Debug, Clone, Default)]
pub struct EpubImportOpts {
    /// Override the title of the created book. None → the EPUB's
    /// `dc:title`, falling back to "Imported EPUB".
    pub book_name: Option<String>,
    /// Report what would be created without writing anything.
    pub dry_run: bool,
}

#[derive(Debug, Default)]
pub struct EpubImportReport {
    pub book_title: String,
    pub chapters_created: usize,
    pub paragraphs_created: usize,
    pub images_extracted: usize,
    pub errors: Vec<String>,
}

/// Import `epub_path` into `store`. Creates a Book → Chapters →
/// Paragraphs and extracts images to `<project>/<book-slug>-images/`.
pub fn import_epub(
    epub_path: &Path,
    store: &Store,
    cfg: &Config,
    opts: &EpubImportOpts,
) -> Result<EpubImportReport> {
    let bytes = std::fs::read(epub_path)
        .with_context(|| format!("read {}", epub_path.display()))?;
    let mut archive = EpubArchive::open(bytes)?;
    let pkg = archive.package()?;

    let mut report = EpubImportReport::default();
    let book_title = opts
        .book_name
        .clone()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| (!pkg.title.trim().is_empty()).then(|| pkg.title.clone()))
        .unwrap_or_else(|| "Imported EPUB".to_string());
    report.book_title = book_title.clone();

    let image_items: Vec<&super::package::ManifestItem> = pkg
        .manifest
        .values()
        .filter(|m| m.media_type.starts_with("image/"))
        .collect();

    if opts.dry_run {
        report.chapters_created = pkg.spine.len();
        report.paragraphs_created = pkg.spine.len();
        report.images_extracted = image_items.len();
        return Ok(report);
    }

    // 1. The book.
    let book_id = create_node(store, cfg, NodeKind::Book, &book_title, None)?;

    // 2. Each spine document → a Chapter + one Paragraph of prose.
    for (i, href) in pkg.spine.iter().enumerate() {
        let xhtml = match archive.read(href) {
            Some(b) => String::from_utf8_lossy(&b).into_owned(),
            None => {
                report.errors.push(format!("spine document `{href}` missing from the zip"));
                continue;
            }
        };
        let body = neutralize_image_refs(&xhtml_to_typst(&xhtml));
        let chapter_title =
            first_heading(&body).unwrap_or_else(|| format!("Chapter {}", i + 1));

        let chapter_id = match create_node(
            store,
            cfg,
            NodeKind::Chapter,
            &chapter_title,
            Some(book_id),
        ) {
            Ok(id) => {
                report.chapters_created += 1;
                id
            }
            Err(e) => {
                report.errors.push(format!("chapter `{chapter_title}`: {e:#}"));
                continue;
            }
        };

        match create_paragraph(store, cfg, chapter_id, &chapter_title, &body) {
            Ok(()) => report.paragraphs_created += 1,
            Err(e) => report
                .errors
                .push(format!("paragraph in `{chapter_title}`: {e:#}")),
        }
    }

    // 3. Extract images to a sidecar folder beside the project, for the
    //    author to re-link. (Full image-node import is a follow-up.)
    if !image_items.is_empty() {
        let dir = store
            .project_root()
            .join(format!("{}-images", slug::slugify(&book_title)));
        let _ = std::fs::create_dir_all(&dir);
        for item in image_items {
            if let Some(bytes) = archive.read(&item.href) {
                let name = item.href.rsplit('/').next().unwrap_or("image");
                if crate::io_atomic::write(&dir.join(name), &bytes).is_ok() {
                    report.images_extracted += 1;
                }
            }
        }
    }

    Ok(report)
}

/// Create a Book/Chapter branch node and return its id.
fn create_node(
    store: &Store,
    cfg: &Config,
    kind: NodeKind,
    title: &str,
    parent_id: Option<Uuid>,
) -> Result<Uuid> {
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow::anyhow!("hierarchy: {e}"))?;
    let parent = parent_id.and_then(|id| hierarchy.get(id).cloned());
    let node = store
        .create_node(cfg, &hierarchy, kind, title, parent.as_ref(), None, InsertPosition::End)
        .map_err(|e| anyhow::anyhow!("create {kind:?} `{title}`: {e}"))?;
    Ok(node.id)
}

/// Create a Paragraph node under `parent_id` and write `body` to it
/// (on-disk file + store blob), matching the Scrivener importer.
fn create_paragraph(
    store: &Store,
    cfg: &Config,
    parent_id: Uuid,
    title: &str,
    body: &str,
) -> Result<()> {
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow::anyhow!("hierarchy: {e}"))?;
    let parent = hierarchy
        .get(parent_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("parent {parent_id} missing"))?;
    let mut node = store
        .create_node(cfg, &hierarchy, NodeKind::Paragraph, title, Some(&parent), None, InsertPosition::End)
        .map_err(|e| anyhow::anyhow!("create paragraph: {e}"))?;
    if body.is_empty() {
        return Ok(());
    }
    if let Some(rel) = node.file.as_ref() {
        let abs = store.project_root().join(rel);
        crate::io_atomic::write(&abs, body.as_bytes())
            .map_err(|e| anyhow::anyhow!("write {}: {e}", abs.display()))?;
    }
    store
        .update_paragraph_content(&mut node, body.as_bytes())
        .map_err(|e| anyhow::anyhow!("store update: {e}"))?;
    Ok(())
}

/// The text of the first `= heading` line, if any.
fn first_heading(body: &str) -> Option<String> {
    for line in body.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("= ") {
            let h = rest.trim();
            if !h.is_empty() {
                return Some(h.to_string());
            }
        }
    }
    None
}

/// Turn the `#image("href")` markers the XHTML converter emits into
/// typst comments referencing the extracted basename, so an imported
/// chapter compiles cleanly while still telling the author where the
/// image was.
fn neutralize_image_refs(body: &str) -> String {
    const OPEN: &str = "#image(\"";
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(pos) = rest.find(OPEN) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + OPEN.len()..];
        match after.find("\")") {
            Some(end) => {
                let href = &after[..end];
                let base = href.rsplit('/').next().unwrap_or(href);
                out.push_str(&format!("// [imported image: {base}]"));
                rest = &after[end + 2..];
            }
            None => {
                out.push_str(&rest[pos..]); // malformed — leave verbatim
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_heading_extracts_chapter_title() {
        assert_eq!(first_heading("= Chapter One\n\nbody"), Some("Chapter One".into()));
        assert_eq!(first_heading("no heading here"), None);
        // `==` subheadings don't count as the chapter title.
        assert_eq!(first_heading("== Sub\n\nbody"), None);
    }

    #[test]
    fn neutralize_rewrites_image_refs_to_comments() {
        let out = neutralize_image_refs("see #image(\"img/x.png\") here");
        assert_eq!(out, "see // [imported image: x.png] here");
        // Unterminated marker is left verbatim, no panic.
        let bad = neutralize_image_refs("oops #image(\"x");
        assert!(bad.contains("#image(\"x"));
    }
}
