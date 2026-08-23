//! BD-P1 — gather: the DECLARED bonds (`rel:` tags) and the DERIVED co-presence
//! (which characters share a scene). Mirrors `ken::grants`. All pure except the
//! `build_bonds` driver, which reads the manuscript once (via the shared KEN
//! walk) and the character roster + timeline.

use std::collections::{BTreeMap, HashMap, HashSet};

use uuid::Uuid;

use super::{parse_rel_tag, CoScene, Declared, ScenePos};
use crate::ken::grants::normalize_topic;
use crate::ken::walk::{self, ParaRef};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;
use crate::world::timeline_context::{self, TlEvent};

/// Case-insensitive character-name roster: the canonical spellings (from the
/// Characters book) plus a normalized-lowercase index, so a `rel:` tag or a
/// `pov:` value resolves to the roster's spelling regardless of case/spacing.
/// (No alias field exists in the schema — a character is one roster node whose
/// title is its name; alias resolution is a documented non-goal for v1.)
pub(crate) struct Roster {
    canonical: Vec<String>,
    lc: HashMap<String, String>,
}

impl Roster {
    pub(crate) fn new(pairs: &[(Uuid, String)]) -> Self {
        let mut canonical = Vec::new();
        let mut lc = HashMap::new();
        for (_, name) in pairs {
            let n = normalize_topic(name);
            if n.is_empty() {
                continue;
            }
            lc.entry(n.to_lowercase()).or_insert_with(|| n.clone());
            canonical.push(n);
        }
        canonical.sort();
        canonical.dedup();
        Roster { canonical, lc }
    }

    /// Resolve a raw name (from a tag / POV) to its canonical roster spelling.
    pub(crate) fn resolve(&self, raw: &str) -> Option<String> {
        self.lc.get(&normalize_topic(raw).to_lowercase()).cloned()
    }

    pub(crate) fn names(&self) -> &[String] {
        &self.canonical
    }
}

/// Declared bonds from `rel:<kind>:<A>:<B>` tags on manuscript paragraphs. Both
/// ends must resolve to a known character (an unresolvable end is skipped, like
/// KEN's unattributable bare `know:`), and a self-bond (`A == B`) is dropped. Pure.
pub(crate) fn bonds_from_tags(paras: &[ParaRef], roster: &Roster) -> Vec<Declared> {
    let mut out = Vec::new();
    for p in paras {
        for tag in &p.tags {
            let Some((kind, a, b)) = parse_rel_tag(tag) else { continue };
            let (Some(a), Some(b)) = (roster.resolve(&a), roster.resolve(&b)) else { continue };
            if a != b {
                out.push(Declared::new(&kind, &a, &b, p.at, p.id));
            }
        }
    }
    out
}

/// Derive, per scene, the set of characters present (the "cast"), then emit a
/// `CoScene` for every unordered pair that shares it. Co-presence is the UNION of
/// three signals so a real on-page pairing is rarely missed (a false "present"
/// only makes BONDS *quieter*, which is the safe bias for an advisory reader):
///   1. the scene's POV character,
///   2. any roster character named (whole-word) in the scene's prose, and
///   3. the explicit participants of any timeline event linked into the scene.
/// Pure.
pub(crate) fn coscenes_from_paras(
    paras: &[ParaRef],
    roster: &Roster,
    events: &[TlEvent],
    names_by_id: &HashMap<Uuid, String>,
) -> Vec<CoScene> {
    let mut anchor: BTreeMap<ScenePos, Uuid> = BTreeMap::new();
    let mut text_lc: BTreeMap<ScenePos, String> = BTreeMap::new();
    let mut cast: BTreeMap<ScenePos, HashSet<String>> = BTreeMap::new();

    // Pass 1: per-scene anchor + concatenated lowercased text + POV cast.
    for p in paras {
        anchor.entry(p.at).or_insert(p.id);
        let t = text_lc.entry(p.at).or_default();
        t.push_str(&p.text.to_lowercase());
        t.push('\n');
        if let Some(pov) = p.declared_pov.as_ref().and_then(|v| roster.resolve(v)) {
            cast.entry(p.at).or_default().insert(pov);
        }
    }

    // Pass 2: prose mentions — a roster name named whole-word in the scene text.
    for (sp, t) in &text_lc {
        let c = cast.entry(*sp).or_default();
        for name in roster.names() {
            if crate::drift::mentions(t, &name.to_lowercase()) {
                c.insert(name.clone());
            }
        }
    }

    // Pass 3: timeline participants — map each event to a scene via its first
    // linked paragraph that resolves, then add its explicit characters.
    let pos: HashMap<Uuid, ScenePos> = paras.iter().map(|p| (p.id, p.at)).collect();
    for e in events {
        let Some((anc, sp)) =
            e.linked_paragraphs.iter().find_map(|p| pos.get(p).map(|sp| (*p, *sp)))
        else {
            continue;
        };
        anchor.entry(sp).or_insert(anc);
        let c = cast.entry(sp).or_default();
        for cid in &e.characters {
            if let Some(name) = names_by_id.get(cid) {
                c.insert(name.clone());
            }
        }
    }

    // Emit one CoScene per unordered pair co-present in a scene.
    let mut out = Vec::new();
    for (sp, c) in &cast {
        if c.len() < 2 {
            continue;
        }
        let anc = anchor.get(sp).copied().unwrap_or_else(Uuid::nil);
        let mut names: Vec<&String> = c.iter().collect();
        names.sort();
        for i in 0..names.len() {
            for j in (i + 1)..names.len() {
                out.push(CoScene::new(names[i], names[j], *sp, anc));
            }
        }
    }
    out
}

/// The impure driver: walk the book once, gather declared bonds + derived
/// co-scenes, and return them alongside the paragraph walk (BD-P2 reuses the
/// paras). Mirrors `ken::grants::build_grants`.
pub(crate) fn build_bonds(
    layout: &ProjectLayout,
    h: &Hierarchy,
    book: &Node,
) -> (Vec<Declared>, Vec<CoScene>, Vec<ParaRef>) {
    let paras = walk::book_paras(layout, h, book);
    let pairs = crate::continuity_intel::introduce::roster(h, crate::store::SYSTEM_TAG_CHARACTERS);
    let roster = Roster::new(&pairs);
    let names_by_id: HashMap<Uuid, String> =
        pairs.iter().map(|(id, name)| (*id, normalize_topic(name))).collect();
    let events = timeline_context::gather_events(h);

    let declared = bonds_from_tags(&paras, &roster);
    let coscenes = coscenes_from_paras(&paras, &roster, &events, &names_by_id);
    (declared, coscenes, paras)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roster() -> Roster {
        Roster::new(&[
            (Uuid::from_u128(1), "Mara".into()),
            (Uuid::from_u128(2), "Kell".into()),
            (Uuid::from_u128(3), "Ser Danel".into()),
        ])
    }

    fn para(id: u128, ch: u32, sc: u32, tags: &[&str], text: &str, pov: Option<&str>) -> ParaRef {
        ParaRef {
            id: Uuid::from_u128(id),
            at: ScenePos { chapter_ord: ch, scene_index: sc },
            tags: tags.iter().map(|s| s.to_string()).collect(),
            text: text.into(),
            declared_pov: pov.map(String::from),
        }
    }

    #[test]
    fn roster_resolves_case_insensitively() {
        let r = roster();
        assert_eq!(r.resolve("mara").as_deref(), Some("Mara"));
        assert_eq!(r.resolve("  KELL ").as_deref(), Some("Kell"));
        assert_eq!(r.resolve("ser danel").as_deref(), Some("Ser Danel"));
        assert!(r.resolve("Nobody").is_none());
    }

    #[test]
    fn bonds_from_tags_keeps_resolvable_pairs_only() {
        let r = roster();
        let paras = vec![
            para(10, 1, 1, &["rel:ally:mara:kell"], "…", None),
            para(11, 2, 1, &["rel:enemy:Mara:Ghost"], "…", None), // Ghost not in roster
            para(12, 3, 1, &["rel:kin:Mara:Mara"], "…", None),    // self-bond
            para(13, 4, 1, &["pov:Mara"], "…", None),             // not a rel tag
        ];
        let bonds = bonds_from_tags(&paras, &r);
        assert_eq!(bonds.len(), 1, "only the fully-resolvable non-self bond");
        assert_eq!((bonds[0].a.as_str(), bonds[0].b.as_str()), ("Kell", "Mara"));
        assert_eq!(bonds[0].kind, "ally");
    }

    #[test]
    fn coscenes_from_mentions_and_pov() {
        let r = roster();
        let paras = vec![
            // Scene (1,1): Mara is POV, Kell is named → co-present.
            para(20, 1, 1, &["pov:Mara"], "Kell laughed at the gate.", Some("Mara")),
            // Scene (2,1): only Mara named → no pair.
            para(21, 2, 1, &[], "Mara walked alone.", None),
        ];
        let cos = coscenes_from_paras(&paras, &r, &[], &HashMap::new());
        assert_eq!(cos.len(), 1, "one shared scene");
        assert_eq!((cos[0].a.as_str(), cos[0].b.as_str()), ("Kell", "Mara"));
        assert_eq!(cos[0].at, ScenePos { chapter_ord: 1, scene_index: 1 });
    }

    #[test]
    fn coscenes_from_timeline_participants() {
        let r = roster();
        let names_by_id: HashMap<Uuid, String> =
            [(Uuid::from_u128(1), "Mara".to_string()), (Uuid::from_u128(2), "Kell".to_string())]
                .into_iter()
                .collect();
        // A scene whose prose names nobody, but a linked timeline event puts both
        // Mara and Kell there.
        let paras = vec![para(30, 5, 1, &[], "They rode in silence.", None)];
        let events = vec![TlEvent {
            id: Uuid::from_u128(99),
            title: "the crossing".into(),
            start_ticks: 0,
            end_ticks: None,
            linked_paragraphs: vec![Uuid::from_u128(30)],
            characters: vec![Uuid::from_u128(1), Uuid::from_u128(2)],
            places: vec![],
        }];
        let cos = coscenes_from_paras(&paras, &r, &events, &names_by_id);
        assert_eq!(cos.len(), 1);
        assert_eq!((cos[0].a.as_str(), cos[0].b.as_str()), ("Kell", "Mara"));
    }
}
