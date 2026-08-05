//! KEN-1 (KEN-P1) — the paragraph-level reading-order walk.
//!
//! Mirrors `chorus::scenes::book_scenes`' scene-break walk (Jinja-excluded,
//! Typst-stripped) but exposes per-paragraph detail KEN needs: each prose
//! paragraph's [`ScenePos`], its tags (for `secret:`/`know:` grants, KEN-P1), its
//! stripped text (for use-detection, KEN-P2), and the scene's declared POV.
#![allow(dead_code)]

use uuid::Uuid;

use super::ScenePos;
use crate::project::ProjectLayout;
use crate::store::NodeKind;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

/// One prose paragraph, positioned in reading order.
pub(crate) struct ParaRef {
    pub id: Uuid,
    pub at: ScenePos,
    pub tags: Vec<String>,
    pub text: String,
    /// The scene's declared POV (`pov:<name>` tag), shared by every paragraph in
    /// the scene.
    pub declared_pov: Option<String>,
}

/// Every prose paragraph of `book` in reading order, tagged with its scene
/// position + the scene's declared POV. Impure (reads paragraph files). Same
/// scene-break / Jinja / Typst rules as `chorus::book_scenes`.
pub(crate) fn book_paras(layout: &ProjectLayout, h: &Hierarchy, book: &Node) -> Vec<ParaRef> {
    let chapters: Vec<&Node> = h
        .children_of(Some(book.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Chapter)
        .collect();

    let mut out = Vec::new();
    for (ci, ch) in chapters.iter().enumerate() {
        let chapter_ord = (ci + 1) as u32;
        let mut scene_idx = 0u32;
        let mut cur: Vec<(Uuid, String, Vec<String>)> = Vec::new();
        for id in h.collect_subtree(ch.id) {
            let Some(n) = h.get(id) else { continue };
            if n.kind != NodeKind::Paragraph || n.content_type.as_deref() == Some("jinja") {
                continue;
            }
            let Some(rel) = n.file.as_ref() else { continue };
            let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) else { continue };
            let text = crate::audiobook::typst_to_plain(&raw);
            if crate::manuscript::is_scene_break(&text) {
                flush(&mut cur, chapter_ord, &mut scene_idx, &mut out);
                continue;
            }
            cur.push((n.id, text, n.tags.clone()));
        }
        flush(&mut cur, chapter_ord, &mut scene_idx, &mut out);
    }
    out
}

fn flush(
    cur: &mut Vec<(Uuid, String, Vec<String>)>,
    chapter_ord: u32,
    scene_idx: &mut u32,
    out: &mut Vec<ParaRef>,
) {
    if cur.is_empty() {
        return;
    }
    *scene_idx += 1;
    let declared_pov = cur
        .iter()
        .flat_map(|(_, _, tags)| tags.iter())
        .find_map(|t| t.strip_prefix("pov:").map(|s| s.to_string()));
    for (id, text, tags) in cur.drain(..) {
        out.push(ParaRef {
            id,
            at: ScenePos { chapter_ord, scene_index: *scene_idx },
            tags,
            text,
            declared_pov: declared_pov.clone(),
        });
    }
}
