//! CHORUS-1 — scene walking, shared by the discipline pillars (POV/head-hop
//! CH-P4, tense CH-P5, register CH-P6). A scene is a run of prose paragraphs
//! between scene breaks (`crate::manuscript::is_scene_break`).

use uuid::Uuid;

use crate::project::ProjectLayout;
use crate::store::NodeKind;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

/// One scene: its chapter, its 1-based index within the chapter, the id of its
/// first paragraph (for jump-anchoring), the concatenated stripped prose, and
/// the declared POV (`pov:<name>` paragraph tag) if any.
pub(crate) struct Scene {
    pub chapter_ord: u32,
    pub scene_index: u32,
    pub first_para: Uuid,
    pub text: String,
    pub declared_pov: Option<String>,
}

/// Every scene in `book`, in reading order. Impure (reads paragraph files);
/// Jinja templates are excluded, prose is Typst-stripped.
pub(crate) fn book_scenes(layout: &ProjectLayout, h: &Hierarchy, book: &Node) -> Vec<Scene> {
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
                push_scene(&mut cur, chapter_ord, &mut scene_idx, &mut out);
                continue;
            }
            cur.push((n.id, text, n.tags.clone()));
        }
        push_scene(&mut cur, chapter_ord, &mut scene_idx, &mut out);
    }
    out
}

/// Each chapter's prose as one stripped blob, `(chapter_ord, text)` in reading
/// order — for the chapter-granular register axis (CH-P6). Built by grouping the
/// scene walk, so it shares the same Jinja-excluding, Typst-stripping rules.
pub(crate) fn chapter_texts(layout: &ProjectLayout, h: &Hierarchy, book: &Node) -> Vec<(u32, String)> {
    let mut by_chapter: std::collections::BTreeMap<u32, String> = std::collections::BTreeMap::new();
    for s in book_scenes(layout, h, book) {
        let entry = by_chapter.entry(s.chapter_ord).or_default();
        if !entry.is_empty() {
            entry.push('\n');
        }
        entry.push_str(&s.text);
    }
    by_chapter.into_iter().collect()
}

fn push_scene(
    cur: &mut Vec<(Uuid, String, Vec<String>)>,
    chapter_ord: u32,
    scene_idx: &mut u32,
    out: &mut Vec<Scene>,
) {
    if cur.is_empty() {
        return;
    }
    *scene_idx += 1;
    let first_para = cur[0].0;
    let declared_pov = cur
        .iter()
        .flat_map(|(_, _, tags)| tags.iter())
        .find_map(|t| t.strip_prefix("pov:").map(|s| s.to_string()));
    let text: String = cur.iter().map(|(_, t, _)| t.as_str()).collect::<Vec<_>>().join("\n");
    out.push(Scene { chapter_ord, scene_index: *scene_idx, first_para, text, declared_pov });
    cur.clear();
}
