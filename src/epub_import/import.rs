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
    /// The EPUB's `dc:creator`, surfaced so the author can set book metadata
    /// (inkhaven has no per-book author field to store it in yet).
    pub author: Option<String>,
    pub chapters_created: usize,
    pub paragraphs_created: usize,
    /// Images referenced in a chapter and imported as `NodeKind::Image` nodes
    /// under that chapter (so they render in the assembled book).
    pub images_imported: usize,
    /// Manifest images NOT referenced by any chapter (cover art, orphans),
    /// written to a sidecar folder for the author to place by hand.
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
    report.author = pkg.author.clone().filter(|a| !a.trim().is_empty());

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

    // Manifest hrefs imported as image nodes — skipped by the sidecar step below.
    let mut imported_hrefs: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 2. Each spine document → a Chapter + one Paragraph of prose, and its
    //    referenced images → `NodeKind::Image` nodes under that chapter.
    // Bound the import: a hostile EPUB can list a huge spine of itemrefs that
    // each re-decompress a 64 MiB entry, burning CPU + memory. Per-entry size is
    // already capped by the archive reader; these cap the aggregate.
    const MAX_SPINE_DOCS: usize = 10_000;
    const MAX_TOTAL_XHTML_BYTES: usize = 512 * 1024 * 1024; // across the whole import
    if pkg.spine.len() > MAX_SPINE_DOCS {
        report.errors.push(format!(
            "EPUB has {} spine documents — importing only the first {}",
            pkg.spine.len(),
            MAX_SPINE_DOCS
        ));
    }
    let mut total_xhtml_bytes: usize = 0;
    for (i, href) in pkg.spine.iter().take(MAX_SPINE_DOCS).enumerate() {
        let xhtml = match archive.read(href) {
            Some(b) => {
                total_xhtml_bytes = total_xhtml_bytes.saturating_add(b.len());
                if total_xhtml_bytes > MAX_TOTAL_XHTML_BYTES {
                    report.errors.push(format!(
                        "stopped: EPUB spine decompressed past {} MiB — refusing the rest",
                        MAX_TOTAL_XHTML_BYTES / (1024 * 1024)
                    ));
                    break;
                }
                String::from_utf8_lossy(&b).into_owned()
            }
            None => {
                report.errors.push(format!("spine document `{href}` missing from the zip"));
                continue;
            }
        };
        // Resolve the chapter's `<img>` references (relative to this document)
        // to manifest images before the refs are neutralised out of the prose.
        let chapter_images: Vec<(String, Option<String>)> = extract_img_srcs(&xhtml)
            .into_iter()
            .map(|(src, alt)| (resolve_href(href, &src), alt))
            .filter(|(res, _)| pkg.manifest.values().any(|m| m.href == *res))
            .collect();

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

        // Import each referenced image as an Image node under the chapter. The
        // hierarchy is reloaded per node (via `create_image`) so ordering stays
        // correct across successive inserts.
        for (res, alt) in chapter_images {
            let Some(bytes) = archive.read(&res) else { continue };
            let fname = res.rsplit('/').next().unwrap_or("image");
            let (stem, ext) = match fname.rsplit_once('.') {
                Some((s, e)) if !e.is_empty() => (s, e.to_ascii_lowercase()),
                _ => (fname, "png".to_string()),
            };
            let title = alt.filter(|a| !a.trim().is_empty()).unwrap_or_else(|| stem.to_string());
            match create_image(store, cfg, chapter_id, &title, &ext, &bytes) {
                Ok(()) => {
                    report.images_imported += 1;
                    imported_hrefs.insert(res);
                }
                Err(e) => report.errors.push(format!("image `{fname}`: {e:#}")),
            }
        }
    }

    // 3. Any manifest images NOT referenced by a chapter (cover art, orphans)
    //    go to a sidecar folder for the author to place by hand.
    let orphans: Vec<&super::package::ManifestItem> =
        image_items.iter().copied().filter(|m| !imported_hrefs.contains(&m.href)).collect();
    if !orphans.is_empty() {
        let dir = store
            .project_root()
            .join(format!("{}-images", slug::slugify(&book_title)));
        let _ = std::fs::create_dir_all(&dir);
        for item in orphans {
            if let Some(bytes) = archive.read(&item.href) {
                // Take only the final path component, splitting on BOTH separators
                // — a hostile OPF `href` can use `\` to slip past a `/`-only split
                // on Windows (`..\..\evil.png`) — and refuse traversal, so
                // `dir.join(name)` can never escape the sidecar folder.
                let base = item.href.rsplit(['/', '\\']).next().unwrap_or("image");
                let name = if base.is_empty() || base == ".." || base == "." {
                    "image"
                } else {
                    base
                };
                if crate::io_atomic::write(&dir.join(name), &bytes).is_ok() {
                    report.images_extracted += 1;
                }
            }
        }
    }

    Ok(report)
}

/// Create an `Image` node under `parent_id`, writing `bytes` as its content.
/// Reloads the hierarchy so `create_image_node`'s ordering is correct across
/// successive inserts under the same parent.
fn create_image(
    store: &Store,
    cfg: &Config,
    parent_id: Uuid,
    title: &str,
    ext: &str,
    bytes: &[u8],
) -> Result<()> {
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow::anyhow!("hierarchy: {e}"))?;
    let parent = hierarchy
        .get(parent_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("parent {parent_id} missing"))?;
    store
        .create_image_node(cfg, &hierarchy, title, ext, bytes, Some(&parent), InsertPosition::End)
        .map_err(|e| anyhow::anyhow!("create image `{title}`: {e}"))?;
    Ok(())
}

/// Extract `(src, alt)` from every `<img>` tag in an XHTML document.
fn extract_img_srcs(xhtml: &str) -> Vec<(String, Option<String>)> {
    let lower = xhtml.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(rel) = lower[i..].find("<img") {
        let start = i + rel;
        let end = xhtml[start..].find('>').map(|e| start + e).unwrap_or(xhtml.len());
        let tag = &xhtml[start..end];
        if let Some(src) = attr_value(tag, "src") {
            out.push((src, attr_value(tag, "alt")));
        }
        i = end.max(start + 4);
    }
    out
}

/// Value of `name="…"` / `name='…'` in a tag slice (case-insensitive name).
fn attr_value(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let key = format!("{name}=");
    let after = lower.find(&key)? + key.len();
    let bytes = tag.as_bytes();
    let quote = *bytes.get(after)? as char;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &tag[after + 1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

/// Resolve a relative image `src` against the referencing document's `base`
/// href (both OPF-root-relative), collapsing `.` / `..` segments.
fn resolve_href(base: &str, src: &str) -> String {
    let src = src.split(['#', '?']).next().unwrap_or(src);
    if let Some(abs) = src.strip_prefix('/') {
        return abs.to_string();
    }
    let mut parts: Vec<&str> = match base.rfind('/') {
        Some(i) => base[..i].split('/').filter(|s| !s.is_empty()).collect(),
        None => Vec::new(),
    };
    for seg in src.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
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
/// image was. Uses a **block** comment (`/* … */`) — a line comment
/// (`//`) would swallow any prose after an inline image on the same
/// line when the chapter compiles.
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
                // `base` is the last path segment, so it never contains a `/`
                // and thus never a `*/` that could close the comment early.
                let base = href.rsplit('/').next().unwrap_or(href);
                out.push_str(&format!("/* [imported image: {base}] */"));
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
    fn neutralize_rewrites_image_refs_to_block_comments() {
        // A4 — a block comment, so prose after an inline image survives
        // compilation (a `//` line comment would swallow " here").
        let out = neutralize_image_refs("see #image(\"img/x.png\") here");
        assert_eq!(out, "see /* [imported image: x.png] */ here");
        assert!(!out.contains("//"), "no line comment that would eat trailing prose");
        // Trailing prose after the image survives; only the closing */ appears.
        assert!(out.ends_with("] */ here"), "{out}");
        // Unterminated marker is left verbatim, no panic.
        let bad = neutralize_image_refs("oops #image(\"x");
        assert!(bad.contains("#image(\"x"));
    }

    #[test]
    fn extract_img_srcs_reads_src_and_alt() {
        let x = r#"<p>hi</p><img src="../images/fig1.png" alt="Figure 1"/><img src='cover.jpg'>"#;
        let got = extract_img_srcs(x);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0], ("../images/fig1.png".into(), Some("Figure 1".into())));
        assert_eq!(got[1], ("cover.jpg".into(), None));
    }

    #[test]
    fn resolve_href_collapses_dot_segments() {
        // src relative to the chapter document's directory.
        assert_eq!(resolve_href("OEBPS/text/ch1.xhtml", "../images/f.png"), "OEBPS/images/f.png");
        assert_eq!(resolve_href("OEBPS/ch1.xhtml", "img/f.png"), "OEBPS/img/f.png");
        // Absolute (root-relative) and fragment/query stripping.
        assert_eq!(resolve_href("a/b.xhtml", "/x/y.png"), "x/y.png");
        assert_eq!(resolve_href("a/b.xhtml", "./f.png#frag"), "a/f.png");
    }
}
