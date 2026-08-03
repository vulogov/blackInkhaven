//! SENTINEL-1 CT-P1 — the *referenced-before-introduced* invariant.
//!
//! The one continuity break nobody detected: an entity **named in the prose
//! before it is introduced**. Deterministic and graph-grounded — no LLM.
//!
//! An entity's *introduction* is its **first scene**: the earliest narrative
//! paragraph (in [`Hierarchy::flatten`] reading order) that one of its timeline
//! events links to (`node.event.characters`/`.places` → the event's
//! `linked_paragraphs`, or the event paragraph itself when the metadata rides a
//! narrative paragraph). A *reference* is any [`crate::drift::mentions`] hit in
//! the manuscript prose. When the first reference lands in an earlier chapter
//! than the introduction — by more than a tolerance — that's the flag.
//!
//! The comparison is intentionally chapter-granular: a name dropped a paragraph
//! or two before its introducing scene inside the *same* chapter is ordinary
//! foreshadowing, not a break; a name that appears chapters early is the thing a
//! reader trips over. `tolerance_chapters` (config in CT-P3) widens that.
//!
//! It leans on the graph the way the RFC intends — the timeline cast is what
//! tells us where a character actually steps on-stage. An entity that never
//! appears in any scene has no determinable introduction and is skipped (no
//! false positives). Multilingual for free: names come from the project's own
//! Characters/Places books and `drift::mentions` is Unicode-aware, so Cyrillic
//! and accented names match exactly as Latin ones do.

use std::collections::HashMap;

use uuid::Uuid;

use super::{ContinuityFinding, Severity};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};

/// An entity and where its story introduces it (its first scene).
#[derive(Debug, Clone)]
pub(crate) struct EntityIntro {
    pub name: String,
    /// 1-based chapter ordinal of the introducing scene (0 = pre-chapter).
    pub intro_chapter: u32,
    /// Reading-order position (paragraph index) of the introducing scene.
    pub intro_pos: usize,
}

/// One manuscript paragraph in reading order, ready for mention-matching.
#[derive(Debug, Clone)]
pub(crate) struct Mention {
    /// Reading-order position (paragraph index, monotone across the manuscript).
    pub pos: usize,
    /// 1-based chapter ordinal (0 = pre-chapter front matter).
    pub chapter: u32,
    /// The paragraph the reference sits in — the finding's jump anchor.
    pub anchor: Uuid,
    /// Lowercased plain text of the paragraph.
    pub text_lc: String,
}

/// A human chapter label for the message. Numeric, so it reads the same in every
/// project language.
fn chap_label(n: u32) -> String {
    if n == 0 { "the opening".to_string() } else { format!("ch. {n}") }
}

/// The pure invariant: for each entity, the earliest paragraph that mentions it;
/// if that reference precedes the introduction by more than `tolerance_chapters`
/// chapters, emit one `introduce` finding anchored to the early reference.
///
/// `mentions` need not be pre-sorted — this orders a borrowed view by position.
/// Pure.
pub(crate) fn referenced_before_introduced(
    entities: &[EntityIntro],
    mentions: &[Mention],
    tolerance_chapters: u32,
) -> Vec<ContinuityFinding> {
    let mut ordered: Vec<&Mention> = mentions.iter().collect();
    ordered.sort_by_key(|m| m.pos);

    let mut out = Vec::new();
    for e in entities {
        let name_lc = e.name.to_lowercase();
        // The first paragraph, in reading order, that names this entity.
        let Some(first) = ordered
            .iter()
            .find(|m| crate::drift::mentions(&m.text_lc, &name_lc))
        else {
            continue;
        };
        // Referenced before the introducing scene, by more than the tolerance
        // (measured in chapters — same-chapter foreshadowing never flags).
        let earlier = first.pos < e.intro_pos
            && e.intro_chapter.saturating_sub(first.chapter) > tolerance_chapters;
        if !earlier {
            continue;
        }
        let entities_v = vec![e.name.clone()];
        let chapter = first.chapter;
        out.push(ContinuityFinding {
            kind: "introduce",
            severity: Severity::Warning,
            chapter,
            anchor: Some(first.anchor),
            dedup_key: ContinuityFinding::make_dedup_key("introduce", &entities_v, chapter),
            entities: entities_v,
            message: format!(
                "'{}' is referenced in {} but not introduced until {}.",
                e.name,
                chap_label(first.chapter),
                chap_label(e.intro_chapter),
            ),
            source: "introduce",
        });
    }
    out
}

/// True when `node` sits inside (or is) a system book — the Characters/Places/…
/// scaffolding, which the manuscript sweep must skip.
fn under_system_book(h: &Hierarchy, node: &Node) -> bool {
    if node.kind == NodeKind::Book && node.system_tag.is_some() {
        return true;
    }
    h.ancestors(node)
        .iter()
        .any(|a| a.kind == NodeKind::Book && a.system_tag.is_some())
}

/// Plain lowercased prose for a manuscript paragraph, or `None` for
/// non-prose (Jinja templates, missing/unreadable files).
fn para_text_lc(layout: &ProjectLayout, node: &Node) -> Option<String> {
    if node.content_type.as_deref() == Some("jinja") {
        return None;
    }
    let rel = node.file.as_ref()?;
    let raw = std::fs::read_to_string(layout.root.join(rel)).ok()?;
    Some(crate::audiobook::typst_to_plain(&raw).to_lowercase())
}

/// Direct-child entries (id + trimmed title) of the system book carrying
/// `system_tag` — the Characters or Places roster.
fn roster(h: &Hierarchy, system_tag: &str) -> Vec<(Uuid, String)> {
    let Some(book) = h
        .iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(system_tag))
    else {
        return Vec::new();
    };
    h.children_of(Some(book.id))
        .iter()
        .filter_map(|n| {
            let t = n.title.trim();
            (!t.is_empty()).then(|| (n.id, t.to_string()))
        })
        .collect()
}

/// The impure driver: gather the roster, the manuscript's reading-order mentions,
/// and each entity's first-scene position from the timeline, then run the pure
/// invariant. Project-wide (the Characters/Places books are shared across books).
pub(crate) fn scan(
    layout: &ProjectLayout,
    h: &Hierarchy,
    tolerance_chapters: u32,
) -> Vec<ContinuityFinding> {
    // Roster: character + place entries, id → name.
    let mut names: HashMap<Uuid, String> = HashMap::new();
    for (id, name) in roster(h, crate::store::SYSTEM_TAG_CHARACTERS)
        .into_iter()
        .chain(roster(h, crate::store::SYSTEM_TAG_PLACES))
    {
        names.insert(id, name);
    }
    if names.is_empty() {
        return Vec::new();
    }

    // One reading-order pass over the manuscript (system books skipped): collect
    // the mentions and record each narrative paragraph's (pos, chapter) so the
    // timeline's linked paragraphs can be resolved to a reading position.
    let mut mentions: Vec<Mention> = Vec::new();
    let mut narrative_pos: HashMap<Uuid, (usize, u32)> = HashMap::new();
    let mut pos = 0usize;
    let mut chapter = 0u32;
    for (node, _depth) in h.flatten() {
        if under_system_book(h, node) {
            continue;
        }
        match node.kind {
            NodeKind::Chapter => chapter += 1,
            NodeKind::Paragraph => {
                let this = pos;
                pos += 1;
                narrative_pos.insert(node.id, (this, chapter));
                if let Some(text_lc) = para_text_lc(layout, node) {
                    mentions.push(Mention { pos: this, chapter, anchor: node.id, text_lc });
                }
            }
            _ => {}
        }
    }

    // Introduction = the earliest narrative reading position an entity's events
    // touch (the event's linked paragraphs, or the event paragraph itself when
    // it is a narrative paragraph).
    let mut intro: HashMap<Uuid, (usize, u32)> = HashMap::new();
    for ev_node in h.iter() {
        let Some(ev) = &ev_node.event else { continue };
        let involved: Vec<Uuid> =
            ev.characters.iter().chain(ev.places.iter()).copied().collect();
        if involved.is_empty() {
            continue;
        }
        let mut anchors: Vec<Uuid> = ev_node.linked_paragraphs.clone();
        anchors.push(ev_node.id); // the event paragraph itself, if it is narrative
        for anchor in anchors {
            let Some(&(apos, achap)) = narrative_pos.get(&anchor) else { continue };
            for ent in &involved {
                let slot = intro.entry(*ent).or_insert((apos, achap));
                if apos < slot.0 {
                    *slot = (apos, achap);
                }
            }
        }
    }

    let entities: Vec<EntityIntro> = names
        .iter()
        .filter_map(|(id, name)| {
            intro.get(id).map(|&(intro_pos, intro_chapter)| EntityIntro {
                name: name.clone(),
                intro_chapter,
                intro_pos,
            })
        })
        .collect();

    referenced_before_introduced(&entities, &mentions, tolerance_chapters)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(pos: usize, chapter: u32, text: &str) -> Mention {
        Mention { pos, chapter, anchor: Uuid::now_v7(), text_lc: text.to_lowercase() }
    }
    fn intro(name: &str, chapter: u32, pos: usize) -> EntityIntro {
        EntityIntro { name: name.to_string(), intro_chapter: chapter, intro_pos: pos }
    }

    #[test]
    fn flags_reference_before_introduction() {
        // "Aldous" is named in ch.2 but not introduced (first scene) until ch.5.
        let mentions = vec![
            m(3, 2, "A boat crossed; Aldous the ferryman was spoken of."),
            m(20, 5, "Aldous finally stepped from the mist."),
        ];
        let entities = vec![intro("Aldous", 5, 20)];
        let out = referenced_before_introduced(&entities, &mentions, 0);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, "introduce");
        assert_eq!(out[0].severity, Severity::Warning);
        assert_eq!(out[0].chapter, 2, "anchored at the early reference");
        assert_eq!(out[0].anchor, Some(mentions[0].anchor));
        assert_eq!(out[0].entities, vec!["Aldous".to_string()]);
    }

    #[test]
    fn introduced_then_mentioned_is_clean() {
        // Introduced in ch.3, then referenced later — the normal case.
        let mentions = vec![
            m(5, 3, "Mara opened the door."),
            m(9, 4, "Mara remembered the door."),
        ];
        let entities = vec![intro("Mara", 3, 5)];
        assert!(referenced_before_introduced(&entities, &mentions, 0).is_empty());
    }

    #[test]
    fn same_chapter_foreshadow_is_not_flagged() {
        // Named a paragraph before the introducing scene, same chapter → fine.
        let mentions = vec![
            m(4, 3, "Someone mentioned Joren."),
            m(6, 3, "Joren arrived."),
        ];
        let entities = vec![intro("Joren", 3, 6)];
        assert!(referenced_before_introduced(&entities, &mentions, 0).is_empty());
    }

    #[test]
    fn tolerance_suppresses_one_chapter_early() {
        // Referenced one chapter early: flags at tolerance 0, clean at tolerance 1.
        let mentions = vec![m(4, 4, "Nadia was expected."), m(8, 5, "Nadia entered.")];
        let entities = vec![intro("Nadia", 5, 8)];
        assert_eq!(referenced_before_introduced(&entities, &mentions, 0).len(), 1);
        assert!(referenced_before_introduced(&entities, &mentions, 1).is_empty());
    }

    #[test]
    fn russian_names_match() {
        // Unicode word-boundary matching → Cyrillic names work like any other.
        let mentions = vec![
            m(2, 1, "В деревне про Алдоус говорили ещё до его прихода."),
            m(30, 6, "Алдоус наконец вышел из тумана."),
        ];
        let entities = vec![intro("Алдоус", 6, 30)];
        let out = referenced_before_introduced(&entities, &mentions, 0);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].chapter, 1);
    }
}
