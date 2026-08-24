//! ENSEMBLE (EN-P3) — the Dramatis Personae builder.
//!
//! Joins a book's **cast** (the character roster) with their declared **BONDS
//! relationships** and their **CHAR-1 arc state** into one book-wide structure —
//! "who is in this book, how they connect, and where each arc stands". A pure
//! join ([`assemble`]) over gathered inputs, plus the impure [`build_cast`]
//! driver. Read-only; it never edits anything.

// Scaffolding: the builder + types land before EN-P4's `inkhaven cast` CLI and
// dashboard consume them. Removed once EN-P4 wires them, so the warning-free bar
// guards the whole surface again.
#![allow(dead_code)]

use std::collections::BTreeMap;

use uuid::Uuid;

use crate::bonds::{self, BondFinding, Declared};
use crate::character::{ArcDeclaration, CharStore, CharacterState};
use crate::config::Config;
use crate::ken::grants::normalize_topic;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

/// A single character in the book's cast, with their arc and their bonds.
#[derive(Debug, Clone)]
pub struct CastMember {
    /// Canonical display name (the roster spelling where known).
    pub name: String,
    /// Characters-book node id — the dashboard's jump target, if resolvable.
    pub node: Option<Uuid>,
    /// Where their arc stands (declared shape + observed state chain), if tracked.
    pub arc: Option<ArcSummary>,
    /// Their relationships, sorted by (other, chapter).
    pub bonds: Vec<Tie>,
}

/// Where a character's arc stands: the declared shape + the observed chain tip.
#[derive(Debug, Clone)]
pub struct ArcSummary {
    /// Declared arc shape (`ArcType::as_code`), if declared.
    pub arc_code: Option<String>,
    /// The latest observed state summary, if any states are recorded.
    pub current_state: Option<String>,
    /// The chapter of that latest state (0 when none).
    pub current_chapter: u32,
    /// Observed state changes across the book (the ✦ count).
    pub changes: usize,
    /// Agency score at the latest state, if scored.
    pub latest_agency: Option<f32>,
}

/// One relationship from a character's point of view.
#[derive(Debug, Clone)]
pub struct Tie {
    pub other: String,
    pub kind: String,
    pub chapter: u32,
}

/// The whole book's Dramatis Personae.
#[derive(Debug, Clone)]
pub struct Cast {
    pub book: String,
    /// Sorted by normalized name.
    pub members: Vec<CastMember>,
    /// The book's BONDS findings (so the dashboard can flag a member's bonds).
    pub findings: Vec<BondFinding>,
}

/// Normalized, case-folded key for joining names across the three sources.
fn norm_key(raw: &str) -> Option<String> {
    let n = normalize_topic(raw);
    if n.is_empty() { None } else { Some(n.to_lowercase()) }
}

/// Internal accumulator, one per cast member.
#[derive(Default)]
struct Builder {
    display: Option<String>,
    node: Option<Uuid>,
    arc_code: Option<String>,
    /// (chapter, state_summary, agency) of the latest observed state.
    cur: Option<(u32, String, Option<f32>)>,
    changes: usize,
    bonds: Vec<Tie>,
}

/// Ensure a member exists for `raw`, resolving its display name + node from the
/// roster index the first time we see it. Returns the join key.
fn touch(
    m: &mut BTreeMap<String, Builder>,
    index: &BTreeMap<String, (String, Uuid)>,
    raw: &str,
) -> Option<String> {
    let k = norm_key(raw)?;
    let b = m.entry(k.clone()).or_default();
    if b.display.is_none() {
        match index.get(&k) {
            Some((disp, id)) => {
                b.display = Some(disp.clone());
                b.node = Some(*id);
            }
            None => b.display = Some(normalize_topic(raw)),
        }
    }
    Some(k)
}

/// Pure: join the roster (name → node), the declared bonds, the arc
/// declarations, and the observed state chain into a sorted cast. The cast is the
/// union of characters this book *tracks* — anyone with an arc declaration, a
/// recorded state, or a declared bond — resolved to a roster node where possible.
/// The roster only supplies display names + jump targets; it does not by itself
/// add a member (a project has one global roster, but the cast is per book).
fn assemble(
    roster: &[(Uuid, String)],
    ties: &[Declared],
    declarations: &[ArcDeclaration],
    states: &[CharacterState],
) -> Vec<CastMember> {
    // Roster index: join key → (canonical display, node id).
    let mut index: BTreeMap<String, (String, Uuid)> = BTreeMap::new();
    for (id, name) in roster {
        if let Some(k) = norm_key(name) {
            index.entry(k).or_insert_with(|| (normalize_topic(name), *id));
        }
    }

    let mut m: BTreeMap<String, Builder> = BTreeMap::new();

    // Arc declarations → the declared arc shape.
    for d in declarations {
        if let Some(k) = touch(&mut m, &index, &d.character_name) {
            m.get_mut(&k).unwrap().arc_code = Some(d.arc_type.as_code().to_string());
        }
    }
    // Observed states → the latest chapter's state + the change count.
    for s in states {
        if let Some(k) = touch(&mut m, &index, &s.character_name) {
            let b = m.get_mut(&k).unwrap();
            if s.changed {
                b.changes += 1;
            }
            let take = b.cur.as_ref().map_or(true, |(ch, _, _)| s.chapter_ord >= *ch);
            if take {
                b.cur = Some((s.chapter_ord, s.state_summary.clone(), s.agency_score));
            }
        }
    }
    // Declared bonds → a Tie on both ends.
    for t in ties {
        if let Some(k) = touch(&mut m, &index, &t.a) {
            m.get_mut(&k)
                .unwrap()
                .bonds
                .push(Tie { other: t.b.clone(), kind: t.kind.clone(), chapter: t.at.chapter_ord });
        }
        if let Some(k) = touch(&mut m, &index, &t.b) {
            m.get_mut(&k)
                .unwrap()
                .bonds
                .push(Tie { other: t.a.clone(), kind: t.kind.clone(), chapter: t.at.chapter_ord });
        }
    }

    // BTreeMap iterates by key → members come out sorted by normalized name.
    m.into_values()
        .filter_map(|b| {
            let name = b.display?;
            if name.is_empty() {
                return None;
            }
            let arc = (b.arc_code.is_some() || b.cur.is_some()).then(|| {
                let (chapter, state, agency) = match b.cur {
                    Some((ch, st, ag)) => (ch, Some(st), ag),
                    None => (0, None, None),
                };
                ArcSummary {
                    arc_code: b.arc_code,
                    current_state: state,
                    current_chapter: chapter,
                    changes: b.changes,
                    latest_agency: agency,
                }
            });
            let mut bonds = b.bonds;
            bonds.sort_by(|x, y| x.other.cmp(&y.other).then(x.chapter.cmp(&y.chapter)));
            bonds.dedup_by(|x, y| x.other == y.other && x.kind == y.kind && x.chapter == y.chapter);
            Some(CastMember { name, node: b.node, arc, bonds })
        })
        .collect()
}

/// The impure driver: gather the roster, the declared bonds + findings, and the
/// CHAR-1 arc data for `book`, then [`assemble`] them. Degrades cleanly — an
/// absent CharStore just means no arc summaries.
pub fn build_cast(layout: &ProjectLayout, h: &Hierarchy, cfg: &Config, book: &Node) -> Cast {
    let roster = crate::continuity_intel::introduce::roster(h, crate::store::SYSTEM_TAG_CHARACTERS);
    let ties = bonds::ties(layout, h, book);
    let findings = bonds::check::run(layout, h, cfg, book);

    let slug = book.slug.as_str();
    let mut declarations: Vec<ArcDeclaration> = Vec::new();
    let mut states: Vec<CharacterState> = Vec::new();
    if let Ok(cs) = CharStore::open(&layout.root) {
        declarations = cs.all_declarations(slug).unwrap_or_default();
        if let Ok(names) = cs.characters_with_states(slug) {
            for name in names {
                if let Ok(chain) = cs.states_for_character(slug, &name) {
                    states.extend(chain);
                }
            }
        }
    }

    let members = assemble(&roster, &ties, &declarations, &states);
    Cast { book: book.title.clone(), members, findings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bonds::ScenePos;
    use crate::character::ArcType;

    fn decl(name: &str) -> ArcDeclaration {
        ArcDeclaration {
            character_name: name.into(),
            arc_type: ArcType::Corruption,
            desired_state_start: String::new(),
            desired_midpoint_state: None,
            desired_state_end: String::new(),
        }
    }
    fn state(name: &str, ch: u32, changed: bool, summary: &str) -> CharacterState {
        CharacterState {
            character_name: name.into(),
            chapter_ord: ch,
            state_summary: summary.into(),
            changed,
            change_description: None,
            agency_score: Some(0.5),
            active_count: 0,
            passive_count: 0,
            utterance_count: None,
            chapter_hedge_density: None,
            chapter_interiority_ratio: None,
        }
    }
    fn tie(kind: &str, ch: u32) -> Declared {
        Declared::new(kind, "Mara", "Kell", ScenePos { chapter_ord: ch, scene_index: 0 }, Uuid::from_u128(9))
    }

    #[test]
    fn assemble_joins_roster_arc_and_bonds() {
        let (mara, kell) = (Uuid::from_u128(1), Uuid::from_u128(2));
        let roster = vec![(mara, "Mara".to_string()), (kell, "Kell".to_string())];
        let ties = vec![tie("ally", 1)];
        let decls = vec![decl("Mara")];
        let states = vec![state("Mara", 1, false, "guarded"), state("Mara", 5, true, "broken")];

        let cast = assemble(&roster, &ties, &decls, &states);
        // Kell (bond only) + Mara (arc + bond) — sorted by name → Kell first.
        assert_eq!(cast.len(), 2);
        let mara_m = cast.iter().find(|c| c.name == "Mara").unwrap();
        assert_eq!(mara_m.node, Some(mara), "resolved to the roster node");
        let arc = mara_m.arc.as_ref().unwrap();
        assert_eq!(arc.arc_code.as_deref(), Some(ArcType::Corruption.as_code()));
        assert_eq!(arc.current_chapter, 5, "latest state wins");
        assert_eq!(arc.current_state.as_deref(), Some("broken"));
        assert_eq!(arc.changes, 1);
        assert_eq!(mara_m.bonds.len(), 1);
        assert_eq!(mara_m.bonds[0].other, "Kell");

        // Kell has the reciprocal bond but no arc.
        let kell_m = cast.iter().find(|c| c.name == "Kell").unwrap();
        assert!(kell_m.arc.is_none());
        assert_eq!(kell_m.bonds[0].other, "Mara");
    }

    #[test]
    fn a_bond_only_character_off_roster_still_appears_without_a_node() {
        // No roster at all — the pair still forms a two-person cast, nodeless.
        let cast = assemble(&[], &[tie("enemy", 3)], &[], &[]);
        assert_eq!(cast.len(), 2);
        assert!(cast.iter().all(|c| c.node.is_none() && c.arc.is_none()));
        assert!(cast.iter().any(|c| c.name == "Mara"));
    }
}
