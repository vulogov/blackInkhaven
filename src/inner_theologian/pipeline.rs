//! INNER-THEOLOGIAN-1 (IT-P6) — the fast-track scan pipeline. Walks a book's
//! chapters, runs the deterministic detector (IT-P3) over each chapter's prose
//! with the Characters roster (CHAR-1), and persists the findings to
//! `inner_theologian.db`. Zero-AI; safe on the review-pass hot path.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use anyhow::Result;

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::prose::resolve_prose_language;
use crate::store::NodeKind;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

use super::detect::{DetectWindows, detect_chapter};
use super::store::TheologianStore;

fn chapters_of<'a>(h: &'a Hierarchy, book: &Node) -> Vec<&'a Node> {
    h.children_of(Some(book.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Chapter)
        .collect()
}

/// A chapter's ordered prose paragraphs `(para_id, stripped_text)` — Jinja
/// excluded, Typst stripped (mirrors CHAR-1's walk).
fn chapter_paragraphs(layout: &ProjectLayout, h: &Hierarchy, chapter_id: uuid::Uuid) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for id in h.collect_subtree(chapter_id) {
        let Some(p) = h.get(id) else { continue };
        if p.kind != NodeKind::Paragraph || p.content_type.as_deref() == Some("jinja") {
            continue;
        }
        if let Some(rel) = p.file.as_ref() {
            if let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) {
                out.push((id.to_string(), crate::audiobook::typst_to_plain(&raw)));
            }
        }
    }
    out
}

fn hash_str(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// Run the fast-track scan across the whole book. Replaces each chapter's prior
/// findings. Returns the number of findings emitted.
pub(crate) fn run_fast_scan(
    store: &TheologianStore,
    layout: &ProjectLayout,
    h: &Hierarchy,
    cfg: &Config,
    book: &Node,
    win: DetectWindows,
    sacred_levity: bool,
) -> Result<usize> {
    let (lang, _note) = resolve_prose_language(None, &cfg.language);
    let roster = crate::character::character_names(h);
    let now = chrono::Utc::now().to_rfc3339();

    let mut count = 0;
    for (idx, ch) in chapters_of(h, book).iter().enumerate() {
        let ord = (idx + 1) as u32;
        let paras = chapter_paragraphs(layout, h, ch.id);
        let findings = detect_chapter(ord, &paras, &roster, &lang, win, sacred_levity);
        store.clear_chapter(&book.slug, ord)?;
        let hashes: HashMap<&str, u64> =
            paras.iter().map(|(id, t)| (id.as_str(), hash_str(t))).collect();
        for f in &findings {
            let th = hashes.get(f.para_id.as_str()).copied().unwrap_or(0);
            store.upsert_finding(&book.slug, f, th, &now)?;
            count += 1;
        }
    }
    Ok(count)
}
