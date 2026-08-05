//! KEN-1 (KEN-P1) — the grant spine: *when could a character first know a topic?*
//!
//! Two deterministic sources, merged:
//! - **Declared** (`secret:<topic>`, `know:<topic>`, `know:<topic>@<char>` tags) —
//!   the author's ground truth, the tension-tag declare-then-check pattern.
//! - **Presence** — a character in a [`TlEvent`]'s participant list knows that
//!   event (its title) from the event's first linked paragraph onward.
//!
//! Grants are kept un-deduped; [`super::earliest_grant`] picks the earliest per
//! `(character, topic)` at query time (KEN-P2).
#![allow(dead_code)]

use std::collections::HashMap;

use uuid::Uuid;

use super::walk::{self, ParaRef};
use super::{Grant, GrantSource, KnowledgeItem, ScenePos};
use crate::project::ProjectLayout;
use crate::store::node::Node;
use crate::store::hierarchy::Hierarchy;
use crate::world::timeline_context::{self, TlEvent};

/// Trim + collapse internal whitespace; case preserved (KEN-P2 matches
/// case-insensitively but records the declared form).
pub(crate) fn normalize_topic(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Parse `secret:` / `know:` tags across the walk into declared grants + the
/// secret set. `know:<topic>` grants the scene's declared POV character;
/// `know:<topic>@<char>` grants `<char>` explicitly; a bare `know:` in a scene
/// with no POV is skipped (unattributable). Pure.
pub(crate) fn grants_from_tags(paras: &[ParaRef]) -> (Vec<Grant>, Vec<KnowledgeItem>) {
    let mut grants = Vec::new();
    let mut items = Vec::new();
    for p in paras {
        for tag in &p.tags {
            if let Some(rest) = tag.strip_prefix("secret:") {
                let topic = normalize_topic(rest);
                if !topic.is_empty() {
                    items.push(KnowledgeItem { topic, secret: true });
                }
            } else if let Some(rest) = tag.strip_prefix("know:") {
                let (topic_raw, who) = match rest.split_once('@') {
                    Some((t, c)) => (t, Some(normalize_topic(c))),
                    None => (rest, None),
                };
                let topic = normalize_topic(topic_raw);
                if topic.is_empty() {
                    continue;
                }
                let character = who.or_else(|| p.declared_pov.as_deref().map(normalize_topic));
                if let Some(character) = character.filter(|c| !c.is_empty()) {
                    grants.push(Grant { character, topic, at: p.at, source: GrantSource::Declared });
                }
            }
        }
    }
    (grants, items)
}

/// Presence grants: every character present at an event knows its subject (title)
/// from the event's first linked paragraph's scene position. `names` maps a
/// character node id → name; `pos` maps a paragraph id → its scene position. Pure.
pub(crate) fn grants_from_events(
    events: &[TlEvent],
    names: &HashMap<Uuid, String>,
    pos: &HashMap<Uuid, ScenePos>,
) -> Vec<Grant> {
    let mut out = Vec::new();
    for e in events {
        let Some(&at) = e.linked_paragraphs.iter().find_map(|p| pos.get(p)) else {
            continue;
        };
        let topic = normalize_topic(&e.title);
        if topic.is_empty() {
            continue;
        }
        for cid in &e.characters {
            if let Some(name) = names.get(cid) {
                out.push(Grant {
                    character: name.clone(),
                    topic: topic.clone(),
                    at,
                    source: GrantSource::Presence,
                });
            }
        }
    }
    out
}

/// The impure driver: walk the book, gather declared + presence grants + the
/// secret set, and return them alongside the paragraph walk (which KEN-P2 reuses
/// for use-detection — one read of the manuscript).
pub(crate) fn build_grants(
    layout: &ProjectLayout,
    h: &Hierarchy,
    book: &Node,
) -> (Vec<Grant>, Vec<KnowledgeItem>, Vec<ParaRef>) {
    let paras = walk::book_paras(layout, h, book);
    let pos: HashMap<Uuid, ScenePos> = paras.iter().map(|p| (p.id, p.at)).collect();

    let (mut grants, items) = grants_from_tags(&paras);

    let names: HashMap<Uuid, String> =
        crate::continuity_intel::introduce::roster(h, crate::store::SYSTEM_TAG_CHARACTERS)
            .into_iter()
            .collect();
    let events = timeline_context::gather_events(h);
    grants.extend(grants_from_events(&events, &names, &pos));

    (grants, items, paras)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn para(id: u128, ch: u32, scene: u32, pov: Option<&str>, tags: &[&str]) -> ParaRef {
        ParaRef {
            id: Uuid::from_u128(id),
            at: ScenePos { chapter_ord: ch, scene_index: scene },
            tags: tags.iter().map(|s| s.to_string()).collect(),
            text: String::new(),
            declared_pov: pov.map(|s| s.to_string()),
        }
    }

    #[test]
    fn declared_tags_grant_pov_or_explicit_character_and_collect_secrets() {
        let paras = vec![
            // secret declared + a bare know: → grants the scene POV (Mara).
            para(1, 3, 1, Some("Mara"), &["secret:the  betrayal", "know:the betrayal"]),
            // explicit @char overrides POV.
            para(2, 4, 1, Some("Mara"), &["know:the map@Joren"]),
            // bare know: with no POV → unattributable, skipped.
            para(3, 5, 1, None, &["know:the heir"]),
        ];
        let (grants, items) = grants_from_tags(&paras);

        // topic whitespace collapsed.
        assert_eq!(items, vec![KnowledgeItem { topic: "the betrayal".into(), secret: true }]);
        assert_eq!(grants.len(), 2, "the no-POV bare know: is skipped");
        let mara = grants.iter().find(|g| g.character == "Mara").unwrap();
        assert_eq!((mara.topic.as_str(), mara.at.chapter_ord, mara.source), ("the betrayal", 3, GrantSource::Declared));
        let joren = grants.iter().find(|g| g.character == "Joren").unwrap();
        assert_eq!(joren.topic, "the map");
    }

    #[test]
    fn presence_grants_every_participant_at_the_event_position() {
        let alice = Uuid::from_u128(10);
        let bob = Uuid::from_u128(11);
        let para_id = Uuid::from_u128(100);
        let mut names = HashMap::new();
        names.insert(alice, "Alice".to_string());
        names.insert(bob, "Bob".to_string());
        // Carol (id 12) is intentionally absent from the roster.
        let mut pos = HashMap::new();
        pos.insert(para_id, ScenePos { chapter_ord: 6, scene_index: 2 });

        let events = vec![TlEvent {
            id: Uuid::from_u128(200),
            title: "The Murder".into(),
            start_ticks: 0,
            end_ticks: None,
            linked_paragraphs: vec![Uuid::from_u128(999), para_id], // first unknown → falls to para_id
            characters: vec![alice, bob, Uuid::from_u128(12)],
            places: vec![],
        }];
        let grants = grants_from_events(&events, &names, &pos);

        assert_eq!(grants.len(), 2, "only rostered participants get a grant");
        assert!(grants.iter().all(|g| g.topic == "The Murder" && g.source == GrantSource::Presence));
        assert!(grants.iter().all(|g| g.at == ScenePos { chapter_ord: 6, scene_index: 2 }));
        assert!(grants.iter().any(|g| g.character == "Alice") && grants.iter().any(|g| g.character == "Bob"));
    }
}
