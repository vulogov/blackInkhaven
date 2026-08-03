//! SENTINEL-1 CT-P5 — the incremental watch: on save, surface only what the edit
//! touched. This is the "watches itself" payoff, and it is feasible because the
//! graph already knows what a paragraph involves.
//!
//! A [`DirtyScope`] is the set of roster entities + the chapter an edit touched,
//! read from the paragraph's own event cast and the roster names its prose
//! mentions. [`run_scoped`] runs the deterministic engine and keeps only the
//! findings that fall within the scope — a guaranteed subset of the full ledger,
//! delivered as the delta the writer cares about right now.
//!
//! The detectors are already sub-second over the whole project, so `run_scoped`
//! runs the full engine and filters; the scope narrows the *view*. Pushing the
//! scope into each detector's compute is a future optimisation, not a correctness
//! concern.

use uuid::Uuid;

use super::{engine, ContinuityFinding};
use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};
use crate::store::Store;

/// The entities + chapter an edit touched — the slice the incremental engine
/// re-checks instead of the whole book.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct DirtyScope {
    /// Roster entity names (characters + places) the edited paragraph involves —
    /// named in its prose or in its own event cast. Lowercased for matching.
    pub entities_lc: Vec<String>,
    /// 1-based chapter ordinal of the edited paragraph (0 = pre-chapter / unknown).
    pub chapter: u32,
}

impl DirtyScope {
    /// Nothing to re-check (no entities and no chapter).
    pub fn is_empty(&self) -> bool {
        self.entities_lc.is_empty() && self.chapter == 0
    }

    /// Whether a finding falls within this scope: it names one of the scope's
    /// entities, or — for entity-less findings like numeric — sits in the scope's
    /// chapter.
    pub fn touches(&self, f: &ContinuityFinding) -> bool {
        let entity_hit = f
            .entities
            .iter()
            .any(|fe| self.entities_lc.iter().any(|e| e == &fe.to_lowercase()));
        entity_hit || (f.chapter != 0 && f.chapter == self.chapter)
    }
}

/// The 1-based chapter ordinal of `node` — the index of its nearest Chapter
/// ancestor (or itself) in `user_book_chapters` order, matching the numbering the
/// detectors emit. 0 when the node sits under no chapter.
fn chapter_ordinal(h: &Hierarchy, node: &Node) -> u32 {
    let mut cur = Some(node);
    let chapter = loop {
        match cur {
            Some(n) if n.kind == NodeKind::Chapter => break Some(n.id),
            Some(n) => cur = n.parent_id.and_then(|p| h.get(p)),
            None => break None,
        }
    };
    let Some(cid) = chapter else { return 0 };
    h.user_book_chapters()
        .iter()
        .position(|(id, _)| *id == cid)
        .map(|i| (i + 1) as u32)
        .unwrap_or(0)
}

/// Compute the dirty scope for a saved paragraph: the roster entities it involves
/// (its own event cast + the roster names its prose mentions) and its chapter.
pub(crate) fn dirty_scope(layout: &ProjectLayout, h: &Hierarchy, paragraph_id: Uuid) -> DirtyScope {
    let Some(node) = h.get(paragraph_id) else {
        return DirtyScope::default();
    };

    // Roster: character + place entries (id → name).
    let roster: Vec<(Uuid, String)> = super::introduce::roster(h, crate::store::SYSTEM_TAG_CHARACTERS)
        .into_iter()
        .chain(super::introduce::roster(h, crate::store::SYSTEM_TAG_PLACES))
        .collect();

    let mut entities_lc: Vec<String> = Vec::new();

    // 1) The paragraph's own event cast, if it is an event node.
    if let Some(ev) = &node.event {
        for id in ev.characters.iter().chain(ev.places.iter()) {
            if let Some((_, name)) = roster.iter().find(|(rid, _)| rid == id) {
                entities_lc.push(name.to_lowercase());
            }
        }
    }

    // 2) The roster names the prose mentions.
    if node.content_type.as_deref() != Some("jinja") {
        if let Some(rel) = node.file.as_ref() {
            if let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) {
                let text_lc = crate::audiobook::typst_to_plain(&raw).to_lowercase();
                for (_, name) in &roster {
                    let name_lc = name.to_lowercase();
                    if crate::drift::mentions(&text_lc, &name_lc) {
                        entities_lc.push(name_lc);
                    }
                }
            }
        }
    }

    entities_lc.sort();
    entities_lc.dedup();

    DirtyScope { entities_lc, chapter: chapter_ordinal(h, node) }
}

/// Keep only the findings that fall within `scope`. Pure; a guaranteed subset of
/// its input (so a scoped run is always a subset of the full run).
pub(crate) fn filter_to_scope(
    findings: Vec<ContinuityFinding>,
    scope: &DirtyScope,
) -> Vec<ContinuityFinding> {
    findings.into_iter().filter(|f| scope.touches(f)).collect()
}

/// Run the deterministic engine (timeline excluded — the review pass owns that
/// line) and keep only the findings touching `scope`.
pub(crate) fn run_scoped(
    store: &Store,
    cfg: &Config,
    layout: &ProjectLayout,
    h: &Hierarchy,
    scope: &DirtyScope,
) -> Vec<ContinuityFinding> {
    let sel = engine::selector(&[], &["timeline".to_string()]);
    let full = engine::run(store, cfg, layout, h, &sel);
    filter_to_scope(full, scope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::continuity_intel::Severity;

    fn finding(kind: &'static str, chapter: u32, entities: &[&str]) -> ContinuityFinding {
        let entities: Vec<String> = entities.iter().map(|s| s.to_string()).collect();
        ContinuityFinding {
            kind,
            severity: Severity::Warning,
            chapter,
            anchor: None,
            dedup_key: ContinuityFinding::make_dedup_key(kind, &entities, chapter),
            entities,
            message: String::new(),
            source: kind,
        }
    }

    #[test]
    fn touches_by_entity_case_insensitive() {
        let scope = DirtyScope { entities_lc: vec!["mara".into()], chapter: 3 };
        assert!(scope.touches(&finding("co_location", 0, &["Mara"])), "entity match ignores chapter");
        assert!(!scope.touches(&finding("co_location", 0, &["Joren"])));
    }

    #[test]
    fn touches_by_chapter_for_entityless_findings() {
        let scope = DirtyScope { entities_lc: vec!["mara".into()], chapter: 3 };
        // A numeric finding carries raw phrases, not roster names — it's in scope
        // only via its chapter.
        assert!(scope.touches(&finding("numeric", 3, &["five miles north", "five miles south"])));
        assert!(!scope.touches(&finding("numeric", 4, &["five miles north", "five miles south"])));
        // Chapter 0 never matches by chapter.
        let book_scope = DirtyScope { entities_lc: vec![], chapter: 0 };
        assert!(!book_scope.touches(&finding("numeric", 0, &["x"])));
    }

    #[test]
    fn filter_is_a_subset_touching_the_scope() {
        let scope = DirtyScope { entities_lc: vec!["mara".into()], chapter: 3 };
        let all = vec![
            finding("co_location", 0, &["Mara"]), // entity hit
            finding("numeric", 3, &["a", "b"]),   // chapter hit
            finding("numeric", 9, &["c", "d"]),   // neither
            finding("introduce", 0, &["Joren"]),  // neither
        ];
        let n = all.len();
        let kept = filter_to_scope(all, &scope);
        assert_eq!(kept.len(), 2);
        assert!(kept.len() <= n, "scoped run is a subset of the full run");
        assert!(kept.iter().all(|f| scope.touches(f)));
    }

    #[test]
    fn dirty_scope_picks_up_the_event_cast() {
        use crate::store::hierarchy::Hierarchy;

        fn node(v: serde_json::Value) -> Node {
            serde_json::from_value(v).expect("test node")
        }
        fn base(id: Uuid, kind: &str, title: &str, parent: Option<Uuid>) -> serde_json::Value {
            serde_json::json!({
                "id": id, "kind": kind, "title": title, "slug": title,
                "path": [], "parent_id": parent, "order": 1, "file": null,
                "modified_at": "2026-01-01T00:00:00Z",
            })
        }

        let chars_book = Uuid::now_v7();
        let mara = Uuid::now_v7();
        let user_book = Uuid::now_v7();
        let chapter = Uuid::now_v7();
        let scene = Uuid::now_v7();

        let mut chars = base(chars_book, "book", "Characters", None);
        chars["system_tag"] = serde_json::json!("characters");
        // The scene paragraph is an event whose cast is Mara.
        let mut scene_v = base(scene, "paragraph", "Scene", Some(chapter));
        scene_v["event"] = serde_json::json!({ "start_ticks": 0, "characters": [mara], "places": [] });

        let h = Hierarchy::from_nodes_for_test(vec![
            node(base(user_book, "book", "Velmaron", None)),
            node(base(chapter, "chapter", "Chapter One", Some(user_book))),
            node(scene_v),
            node(chars),
            node(base(mara, "paragraph", "Mara", Some(chars_book))),
        ]);

        let layout = ProjectLayout::new(std::path::Path::new("/nonexistent"));
        let scope = dirty_scope(&layout, &h, scene);
        assert_eq!(scope.entities_lc, vec!["mara".to_string()], "event cast is in scope");
        assert_eq!(scope.chapter, 1, "first user-book chapter");
    }
}
