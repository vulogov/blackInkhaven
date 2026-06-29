//! CHAR-1 (C-P6) — Planning-Board gap detection. Deterministic, no LLM. Reads
//! the Planning book's scene cards (their `characters` + `arc_function` fields,
//! added in C-P2), resolves each card's chapter to an ordinal in the manuscript,
//! and flags arc-coverage gaps for every character that declares an arc:
//!
//! - `no_scenes`     — a declared arc that no scene card names.
//! - `all_first_half`— every linked card sits in the first half of the book.
//! - `no_final_act`  — no linked card sits in the final act (last quarter).
//!
//! These are *planning* gaps (the outline, not the prose) — advisory only.

use std::collections::HashMap;

use anyhow::Result;

use crate::store::node::Node;
use crate::store::{NodeKind, Store};
use crate::store::hierarchy::Hierarchy;

use super::store::CharStore;

/// Pure gap classifier: given a declared character's linked scene-card chapter
/// ordinals and the manuscript's chapter count, return `(finding_type, prose)`
/// pairs. `all_first_half` subsumes `no_final_act`, so at most one half/act gap
/// fires alongside the `no_scenes` case.
pub(super) fn planning_gaps(scene_chapters: &[u32], total: u32) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    if scene_chapters.is_empty() {
        out.push((
            "no_scenes",
            "declares an arc but no scene card on the Planning Board names this character".to_string(),
        ));
        return out;
    }
    if total == 0 {
        return out;
    }
    let all_first_half = scene_chapters.iter().all(|&c| c * 2 <= total);
    let in_final_act = scene_chapters.iter().any(|&c| c * 4 > total * 3);
    if all_first_half {
        out.push((
            "all_first_half",
            "every arc-linked scene card sits in the first half of the book; the arc has no \
             second-half presence on the Planning Board"
                .to_string(),
        ));
    } else if !in_final_act {
        out.push((
            "no_final_act",
            "no arc-linked scene card sits in the final act; the arc's resolution is unplanned"
                .to_string(),
        ));
    }
    out
}

/// The manuscript book's chapters, with a slug→ord and title→ord lookup (ord is
/// 1-based position). Used to resolve a scene card's `chapter` reference.
fn chapter_index(h: &Hierarchy, book: &Node) -> (u32, HashMap<String, u32>) {
    let mut map = HashMap::new();
    let mut total = 0u32;
    for (idx, ch) in h
        .children_of(Some(book.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Chapter)
        .enumerate()
    {
        let ord = (idx + 1) as u32;
        total = ord;
        map.insert(ch.slug.to_lowercase(), ord);
        map.insert(ch.title.trim().to_lowercase(), ord);
    }
    (total, map)
}

/// Resolve a scene card's `chapter` string (a slug or a title) to a manuscript
/// chapter ordinal.
fn resolve_chapter(chapter: &str, index: &HashMap<String, u32>) -> Option<u32> {
    let key = chapter.trim().to_lowercase();
    if key.is_empty() {
        return None;
    }
    index.get(&key).copied()
}

/// Rebuild the scene-card→character links and the planning-gap findings for a
/// book. Returns the number of gap findings written.
pub(crate) fn run_planning(
    store: &CharStore,
    main_store: &Store,
    h: &Hierarchy,
    book: &Node,
) -> Result<usize> {
    store.clear_scene_links(&book.slug)?;
    store.clear_planning_findings(&book.slug)?;

    let (total, index) = chapter_index(h, book);
    let now = chrono::Utc::now().to_rfc3339();

    // Link every (scene card, character) pair, carrying the card's arc function
    // and resolved chapter ordinal.
    for (id, scene) in crate::cli::plan::load_scenes(main_store, h) {
        if scene.characters.is_empty() {
            continue;
        }
        let ord = resolve_chapter(&scene.chapter, &index);
        let scene_id = id.to_string();
        for raw in &scene.characters {
            let name = raw.trim();
            if name.is_empty() {
                continue;
            }
            store.upsert_scene_link(
                &book.slug,
                &scene_id,
                name,
                scene.arc_function.as_deref(),
                ord,
            )?;
        }
    }

    // For every declared arc, classify its scene-card coverage.
    let mut findings = 0;
    for decl in store.all_declarations(&book.slug)? {
        let chapters = store.scene_chapters(&book.slug, &decl.character_name)?;
        for (ftype, desc) in planning_gaps(&chapters, total) {
            store.upsert_planning_finding(&book.slug, &decl.character_name, ftype, &desc, &now)?;
            findings += 1;
        }
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_scenes_when_unlinked() {
        let g = planning_gaps(&[], 10);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].0, "no_scenes");
    }

    #[test]
    fn all_first_half_subsumes_final_act() {
        // 10 chapters; cards only in 1..=5.
        let g = planning_gaps(&[1, 3, 5], 10);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].0, "all_first_half");
    }

    #[test]
    fn no_final_act_when_spread_but_no_last_quarter() {
        // 12 chapters; final act is ord > 9. Cards reach ch.7 (second half, not
        // final act) → no_final_act, NOT all_first_half.
        let g = planning_gaps(&[2, 7], 12);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].0, "no_final_act");
    }

    #[test]
    fn no_gap_when_arc_reaches_final_act() {
        // 12 chapters; a card at ch.11 is in the final act (11*4=44 > 36).
        let g = planning_gaps(&[2, 7, 11], 12);
        assert!(g.is_empty());
    }

    #[test]
    fn no_act_gap_without_chapters() {
        // Linked but the book has no chapters yet → only the no_scenes path
        // would apply, and that's excluded since the slice is non-empty.
        let g = planning_gaps(&[1], 0);
        assert!(g.is_empty());
    }

    #[test]
    fn resolve_matches_slug_or_title() {
        let mut idx = HashMap::new();
        idx.insert("ch-one".to_string(), 1);
        idx.insert("the beginning".to_string(), 1);
        assert_eq!(resolve_chapter("Ch-One", &idx), Some(1));
        assert_eq!(resolve_chapter("  the beginning ", &idx), Some(1));
        assert_eq!(resolve_chapter("nowhere", &idx), None);
        assert_eq!(resolve_chapter("", &idx), None);
    }
}
