//! Chapter-prose collection shared across the CLI.
//!
//! The narrative scanners (echo / continuity / tension /
//! doctor) and the manuscript exporter all walk a
//! chapter's subtree and read each paragraph's raw typst
//! from the on-disk `.typ` projection.  Before 1.2.20
//! each had its own copy of that walk; this is now the
//! one place it lives.
//!
//! The reader-exports (epub, audiobook) and the live
//! editor read paragraph content from the bdslib store
//! instead — a deliberately separate source-world (the
//! store is authoritative; these CLIs consume the saved
//! filesystem projection) — so they keep their own
//! store-backed walks.

use uuid::Uuid;

use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::NodeKind;

/// Every paragraph under `chapter_id`, in display order,
/// as its raw typst source read from the filesystem
/// projection.  Subchapters are inlined (the subtree is
/// flattened, then filtered to paragraphs).  Paragraph
/// nodes with no file, or whose file can't be read, are
/// skipped.
pub(crate) fn chapter_paragraphs_raw(
    layout: &ProjectLayout,
    h: &Hierarchy,
    chapter_id: Uuid,
) -> Vec<String> {
    let mut out = Vec::new();
    for id in h.collect_subtree(chapter_id) {
        let Some(p) = h.get(id) else { continue };
        if p.kind != NodeKind::Paragraph {
            continue;
        }
        let Some(rel) = p.file.as_ref() else { continue };
        let abs = layout.root.join(rel);
        if let Ok(text) = std::fs::read_to_string(&abs) {
            out.push(text);
        }
    }
    out
}

/// The chapter's raw prose as one blob: every paragraph's
/// raw typst, each followed by a newline.  This is what
/// the narrative scanners feed to their analysers.
pub(crate) fn chapter_raw_prose(
    layout: &ProjectLayout,
    h: &Hierarchy,
    chapter_id: Uuid,
) -> String {
    let mut body = String::new();
    for p in chapter_paragraphs_raw(layout, h, chapter_id) {
        body.push_str(&p);
        body.push('\n');
    }
    body
}
