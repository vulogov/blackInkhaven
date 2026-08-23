//! BD-P2 — the deterministic check (the core, ≈$0 — no LLM). Given the declared
//! bonds and the derived co-scenes, produce the findings. Mirrors `ken::check`:
//! pure invariant fns + an impure `run` driver that self-gates to silence when
//! nothing is declared.
//!
//! Three catches, all "here is a concrete defect", never "consider the
//! relationship":
//! - `unwritten_bond` (Notice) — a declared pair barely (or never) on the page
//!   together: asserted, not dramatised.
//! - `unearned_shift` (Break) — a declared bond's state changes with no shared
//!   scene to turn it: the flagship, the relationship plot-hole.
//! - `dropped_bond` (Notice) — an established bond goes dormant for a long
//!   stretch, then resurfaces.

use std::collections::BTreeMap;

use super::gather;
use super::{BondFinding, CoScene, Declared, Severity};
use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

type Pair = (String, String);

/// Declared states per pair, in reading order.
fn group_declared(declared: &[Declared]) -> BTreeMap<Pair, Vec<&Declared>> {
    let mut m: BTreeMap<Pair, Vec<&Declared>> = BTreeMap::new();
    for d in declared {
        m.entry((d.a.clone(), d.b.clone())).or_default().push(d);
    }
    for v in m.values_mut() {
        v.sort_by_key(|d| d.at);
    }
    m
}

/// Shared scenes per pair, in reading order.
fn group_coscenes(coscenes: &[CoScene]) -> BTreeMap<Pair, Vec<&CoScene>> {
    let mut m: BTreeMap<Pair, Vec<&CoScene>> = BTreeMap::new();
    for c in coscenes {
        m.entry((c.a.clone(), c.b.clone())).or_default().push(c);
    }
    for v in m.values_mut() {
        v.sort_by_key(|c| c.at);
    }
    m
}

/// `unwritten_bond` — a declared pair with fewer than `min_co_presence` shared
/// scenes. One finding per pair, anchored at the earliest declaration.
fn unwritten_bonds(
    declared: &BTreeMap<Pair, Vec<&Declared>>,
    coscenes: &BTreeMap<Pair, Vec<&CoScene>>,
    min_co_presence: u32,
) -> Vec<BondFinding> {
    let mut out = Vec::new();
    for (pair, decls) in declared {
        let n = coscenes.get(pair).map_or(0, |v| v.len()) as u32;
        if n < min_co_presence {
            let first = decls[0];
            out.push(BondFinding {
                kind: "unwritten_bond",
                severity: Severity::Notice,
                chapter: first.at.chapter_ord,
                anchor: Some(first.anchor),
                a: pair.0.clone(),
                b: pair.1.clone(),
                message: format!(
                    "Declared {} bond between {} and {}, but they share {} scene(s) on the \
                     page (want ≥ {}). Asserted, not dramatised.",
                    first.kind, pair.0, pair.1, n, min_co_presence
                ),
            });
        }
    }
    out
}

/// `unearned_shift` — a declared bond's state changes between two adjacent
/// declarations with no shared scene in the transition window `(prev, next]`.
/// The flagship catch.
fn unearned_shifts(
    declared: &BTreeMap<Pair, Vec<&Declared>>,
    coscenes: &BTreeMap<Pair, Vec<&CoScene>>,
) -> Vec<BondFinding> {
    let mut out = Vec::new();
    for (pair, decls) in declared {
        for w in decls.windows(2) {
            let (prev, next) = (w[0], w[1]);
            if prev.kind == next.kind {
                continue; // same state — not a shift
            }
            let turned = coscenes
                .get(pair)
                .is_some_and(|cs| cs.iter().any(|c| c.at > prev.at && c.at <= next.at));
            if !turned {
                out.push(BondFinding {
                    kind: "unearned_shift",
                    severity: Severity::Break,
                    chapter: next.at.chapter_ord,
                    anchor: Some(next.anchor),
                    a: pair.0.clone(),
                    b: pair.1.clone(),
                    message: format!(
                        "Bond between {} and {} shifts {} → {} (ch. {} → ch. {}) with no shared \
                         scene to turn it.",
                        pair.0, pair.1, prev.kind, next.kind, prev.at.chapter_ord,
                        next.at.chapter_ord
                    ),
                });
            }
        }
    }
    out
}

/// `dropped_bond` — a declared pair whose on-page activity (declarations +
/// shared scenes) has a gap wider than `dormancy_window` chapters that then
/// resurfaces. One finding per pair (the first qualifying gap).
fn dropped_bonds(
    declared: &BTreeMap<Pair, Vec<&Declared>>,
    coscenes: &BTreeMap<Pair, Vec<&CoScene>>,
    dormancy_window: u32,
) -> Vec<BondFinding> {
    let mut out = Vec::new();
    for (pair, decls) in declared {
        // Activity chapters = declaration chapters ∪ shared-scene chapters.
        let mut chapters: Vec<u32> = decls.iter().map(|d| d.at.chapter_ord).collect();
        if let Some(cs) = coscenes.get(pair) {
            chapters.extend(cs.iter().map(|c| c.at.chapter_ord));
        }
        chapters.sort_unstable();
        chapters.dedup();

        for w in chapters.windows(2) {
            let (before, after) = (w[0], w[1]);
            if after.saturating_sub(before) > dormancy_window {
                // Anchor at whatever resurfaces the bond at `after`.
                let anchor = coscenes
                    .get(pair)
                    .and_then(|cs| cs.iter().find(|c| c.at.chapter_ord == after).map(|c| c.anchor))
                    .or_else(|| decls.iter().find(|d| d.at.chapter_ord == after).map(|d| d.anchor));
                out.push(BondFinding {
                    kind: "dropped_bond",
                    severity: Severity::Notice,
                    chapter: after,
                    anchor,
                    a: pair.0.clone(),
                    b: pair.1.clone(),
                    message: format!(
                        "Bond between {} and {} goes dormant for {} chapters (ch. {} → ch. {}) \
                         before resurfacing.",
                        pair.0, pair.1, after - before, before, after
                    ),
                });
                break; // one dropped_bond per pair
            }
        }
    }
    out
}

/// The impure driver: gather (declared + derived) then run the three checks.
/// Self-gates to `[]` when nothing is declared (BONDS stays silent on books that
/// don't tag relationships). Does NOT gate on `cfg.bonds.enabled` — that governs
/// the review pass (checked by the `collect` caller); the standalone command runs
/// regardless, like `continuity check`. Mirrors `ken::check::run`.
pub fn run(layout: &ProjectLayout, h: &Hierarchy, cfg: &Config, book: &Node) -> Vec<BondFinding> {
    let (declared, coscenes, _paras) = gather::build_bonds(layout, h, book);
    if declared.is_empty() {
        return Vec::new();
    }
    let dbp = group_declared(&declared);
    let cbp = group_coscenes(&coscenes);

    let mut out = Vec::new();
    out.extend(unwritten_bonds(&dbp, &cbp, cfg.bonds.min_co_presence));
    out.extend(unearned_shifts(&dbp, &cbp));
    out.extend(dropped_bonds(&dbp, &cbp, cfg.bonds.dormancy_window));

    // Most severe first, then reading order, then pair — matches KEN's ordering.
    out.sort_by(|x, y| {
        y.severity
            .rank()
            .cmp(&x.severity.rank())
            .then(x.chapter.cmp(&y.chapter))
            .then(x.a.cmp(&y.a))
            .then(x.b.cmp(&y.b))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bonds::ScenePos;
    use uuid::Uuid;

    fn pos(c: u32, s: u32) -> ScenePos {
        ScenePos { chapter_ord: c, scene_index: s }
    }
    fn decl(kind: &str, at: ScenePos) -> Declared {
        Declared::new(kind, "Mara", "Kell", at, Uuid::from_u128(at.chapter_ord as u128))
    }
    fn cos(at: ScenePos) -> CoScene {
        CoScene::new("Mara", "Kell", at, Uuid::from_u128(1000 + at.chapter_ord as u128))
    }

    #[test]
    fn unwritten_bond_fires_when_barely_co_present() {
        let d = vec![decl("ally", pos(1, 0))];
        let c: Vec<CoScene> = vec![]; // never together
        let f = unwritten_bonds(&group_declared(&d), &group_coscenes(&c), 2);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, "unwritten_bond");
        // Two shared scenes clears the threshold.
        let c2 = vec![cos(pos(1, 1)), cos(pos(2, 0))];
        assert!(unwritten_bonds(&group_declared(&d), &group_coscenes(&c2), 2).is_empty());
    }

    #[test]
    fn unearned_shift_fires_when_no_scene_turns_it() {
        // ally in ch.1, enemy in ch.9, never share a scene between → unearned.
        let d = vec![decl("ally", pos(1, 0)), decl("enemy", pos(9, 0))];
        let none: Vec<CoScene> = vec![];
        let f = unearned_shifts(&group_declared(&d), &group_coscenes(&none));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, "unearned_shift");
        assert_eq!(f[0].severity, Severity::Break);
        assert_eq!(f[0].chapter, 9);
        // A shared scene in the window (ch.5) earns the turn → no finding.
        let turned = vec![cos(pos(5, 0))];
        assert!(unearned_shifts(&group_declared(&d), &group_coscenes(&turned)).is_empty());
        // A shared scene BEFORE the first state (ch.0-ish) doesn't count.
        let early = vec![cos(pos(1, 0))]; // == prev.at, not > prev.at
        assert_eq!(unearned_shifts(&group_declared(&d), &group_coscenes(&early)).len(), 1);
    }

    #[test]
    fn dropped_bond_fires_on_a_wide_gap_that_resurfaces() {
        let d = vec![decl("ally", pos(1, 0))];
        // together ch.1 and ch.10 → gap 9 > window 6, resurfaces.
        let c = vec![cos(pos(1, 0)), cos(pos(10, 0))];
        let f = dropped_bonds(&group_declared(&d), &group_coscenes(&c), 6);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, "dropped_bond");
        assert_eq!(f[0].chapter, 10);
        // A gap within the window → nothing.
        let c2 = vec![cos(pos(1, 0)), cos(pos(5, 0))];
        assert!(dropped_bonds(&group_declared(&d), &group_coscenes(&c2), 6).is_empty());
    }
}
