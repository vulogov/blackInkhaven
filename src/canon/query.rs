//! CL-P3 — the read side: the queries that make the ledger worth keeping.
//!
//! All pure reads over the current store, no model call:
//! - [`CanonLedger::impact`] — the blast radius: everything that would dangle if
//!   a decision were cut (transitive dependents over `grounds`). "What breaks if
//!   I cut this?"
//! - [`CanonLedger::why`] — the grounds chain a decision rests on (`trace`).
//! - [`CanonLedger::all_decisions`] — every narrative decision, for `list`.
//! - [`CanonLedger::resolve`] — a short/prefix id → a unique unit.

use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Result};
use smysl::{dependents, trace, Commit, Commitment, RelKind, SchemaId, Store, TraceKind, Uid};
use uuid::Uuid;

use super::model::{self, NarrativeKind};
use super::CanonLedger;

/// The superseded unit ids in `store` — the `to` of any `Supersedes` edge whose
/// successor (`from`) is still present. A superseded decision is history, not the
/// live ledger, so every query skips it (CANON-2 CG-P0). Chains collapse
/// correctly: in `A → A' → A''` both `A` and `A'` are superseded (each is a `to`
/// with a present successor), only `A''` is live. Mirrors smysl's own `compact`.
pub(super) fn superseded_uids(store: &Store) -> HashSet<Uid> {
    store
        .relations_of_kind(&RelKind::Supersedes)
        .into_iter()
        .filter(|r| store.contains_uid(&r.from))
        .map(|r| r.to)
        .collect()
}

/// A decision's supersession lineage, newest first: `uid`, then the unit it
/// superseded (regrounding replaces a unit with a new-uid version), and so on
/// back to the original. Used to follow commitment and history across regroundings
/// (CG-P3/P4). Guards against a cycle.
pub(super) fn supersession_chain(store: &Store, uid: Uid) -> Vec<Uid> {
    let edges = store.relations_of_kind(&RelKind::Supersedes);
    let mut chain = vec![uid];
    let mut cur = uid;
    loop {
        match edges.iter().find(|r| r.from == cur).map(|r| r.to) {
            Some(p) if !chain.contains(&p) => {
                chain.push(p);
                cur = p;
            }
            _ => break,
        }
    }
    chain
}

/// The live commitment *record* of a decision, **following supersession** (CG-P4):
/// the winning `Commit` from the newest version in its lineage that carries one
/// (latest by `(wall_ms, counter, agent)` within that version). Carries the agent
/// and timestamp too, which `canon compact` needs to pin the commitment onto the
/// live head before dropping superseded predecessors.
pub(super) fn live_commit(store: &Store, uid: Uid) -> Option<Commit> {
    for u in supersession_chain(store, uid) {
        if let Some(c) = store.commits_of(&u).iter().max_by(|a, b| {
            (a.ts.wall_ms, a.ts.counter, a.agent.as_str())
                .cmp(&(b.ts.wall_ms, b.ts.counter, b.agent.as_str()))
        }) {
            return Some(c.clone());
        }
    }
    None
}

/// Depth-first emit for [`CanonLedger::graph`]: `uid` at `depth`, then everything
/// that rests on it. `path` guards against a cycle re-entering the same node; the
/// depth cap is a belt-and-braces bound on a malformed store.
fn walk_graph(
    uid: Uid,
    depth: usize,
    deps: &HashMap<Uid, Vec<Uid>>,
    views: &HashMap<Uid, CanonView>,
    path: &mut Vec<Uid>,
    rows: &mut Vec<GraphRow>,
) {
    if depth > 64 || path.contains(&uid) {
        return;
    }
    if let Some(v) = views.get(&uid) {
        rows.push(GraphRow { depth, view: v.clone() });
    }
    path.push(uid);
    if let Some(children) = deps.get(&uid) {
        for c in children {
            walk_graph(*c, depth + 1, deps, views, path, rows);
        }
    }
    path.pop();
}

/// The live commitment level, following supersession. A unit's `Commit` records
/// key on its exact uid, but regrounding mints a new uid — so a canonical decision
/// would read as uncommitted after `canon ground` unless we walk its lineage.
pub(super) fn commitment_live(store: &Store, uid: Uid) -> Option<Commitment> {
    live_commit(store, uid).map(|c| c.level)
}

/// A human-facing view of a canon decision, resolved from a stored unit.
#[derive(Debug, Clone)]
pub struct CanonView {
    pub uid: Uid,
    /// The narrative kind, or `None` for a non-narrative unit.
    pub kind: Option<NarrativeKind>,
    pub gist: String,
    /// The author's commitment level (canonicity), or `None` if unmarked (CL-P4).
    pub commitment: Option<Commitment>,
    /// The inkhaven node the decision was derived from — the jump-to-source
    /// handle the CL-P8 canon dashboard uses.
    pub node: Option<Uuid>,
    /// The manuscript breadcrumb (`book/ch1/scene1`), when the source carried one.
    pub locator: Option<String>,
    /// The decisions this one rests on — its `grounds` edges (CANON-2). These are
    /// what `impact`/`why` walk; `canon impact <id>` on a ground returns this
    /// decision. Empty until a decision is grounded (deterministically, by the
    /// opt-in harvest, or by hand). Read by `history` (CG-P4).
    pub grounds: Vec<Uid>,
}

impl CanonLedger {
    /// Build a [`CanonView`] for `uid`, or `None` if the store has no such unit.
    pub(super) fn view_from_unit(store: &Store, uid: Uid) -> Option<CanonView> {
        let unit = store.get(&uid)?;
        let kind = match &unit.core.schema {
            SchemaId::Extension(s) => NarrativeKind::from_schema_str(s),
            _ => None,
        };
        let (node, locator) = unit
            .core
            .source
            .as_ref()
            .and_then(|s| model::parse_node_reference(&s.reference))
            .map(|(n, bc)| (Some(n), bc))
            .unwrap_or((None, None));
        Some(CanonView {
            uid,
            kind,
            gist: unit.core.gist.clone(),
            commitment: commitment_live(store, uid),
            node,
            locator,
            grounds: unit.core.grounds.iter().copied().collect(),
        })
    }

    /// A single decision's view, or `None` when the store has no such unit or it
    /// has been superseded (regrounded → a newer version is live).
    pub fn view(&self, uid: Uid) -> Result<Option<CanonView>> {
        self.with_store(|s| {
            if superseded_uids(s).contains(&uid) {
                return Ok(None);
            }
            Ok(Self::view_from_unit(s, uid))
        })
    }

    /// Every narrative canon decision, ordered stably by id. For `canon list`.
    /// Superseded (regrounded) versions are history and are skipped.
    pub fn all_decisions(&self) -> Result<Vec<CanonView>> {
        self.with_store(|s| {
            let dead = superseded_uids(s);
            let uids: Vec<Uid> = s.units().map(|(u, _)| *u).filter(|u| !dead.contains(u)).collect();
            let mut out: Vec<CanonView> = uids
                .into_iter()
                .filter_map(|u| Self::view_from_unit(s, u))
                .filter(|v| v.kind.is_some())
                .collect();
            out.sort_by(|a, b| a.uid.canonical().cmp(&b.uid.canonical()));
            Ok(out)
        })
    }

    /// The blast radius of a decision: everything that transitively rests on it
    /// (via `grounds`) and would dangle if it were cut. "What breaks if I cut this?"
    pub fn impact(&self, uid: Uid) -> Result<Vec<CanonView>> {
        self.with_store(|s| {
            let dead = superseded_uids(s);
            Ok(dependents(s, uid)
                .into_iter()
                .filter(|d| !dead.contains(d))
                .filter_map(|d| Self::view_from_unit(s, d))
                .collect())
        })
    }

    /// Why a decision is canon: the grounds chain it rests on, nearest first
    /// (self excluded).
    pub fn why(&self, uid: Uid) -> Result<Vec<CanonView>> {
        self.with_store(|s| {
            let dead = superseded_uids(s);
            let lineage = trace(s, uid, TraceKind::Grounds, None);
            Ok(lineage
                .nodes
                .into_iter()
                .filter(|n| n.uid != uid)
                .filter(|n| !dead.contains(&n.uid))
                .filter_map(|n| Self::view_from_unit(s, n.uid))
                .collect())
        })
    }

    /// Resolve a canonical- or short-form id prefix to the unique unit it names.
    /// Errors when nothing matches or the prefix is ambiguous.
    pub fn resolve(&self, prefix: &str) -> Result<Uid> {
        self.with_store(|s| {
            let dead = superseded_uids(s);
            let matches: Vec<Uid> = s
                .units()
                .map(|(u, _)| *u)
                .filter(|u| !dead.contains(u))
                .filter(|u| u.canonical().starts_with(prefix) || u.short().starts_with(prefix))
                .collect();
            match matches.as_slice() {
                [one] => Ok(*one),
                [] => Err(anyhow!("no canon decision matches id '{prefix}'")),
                many => Err(anyhow!("id '{prefix}' is ambiguous ({} decisions match)", many.len())),
            }
        })
    }

    /// CANON-2 (CG-P4) — a decision's development history: its current view, the
    /// grounds it rests on, and its commitment trajectory over time (every `Commit`
    /// across its supersession lineage, oldest first — so a re-grounding never
    /// loses the earlier commitments). `None` if the id names no live decision.
    pub fn history(&self, uid: Uid) -> Result<Option<DecisionHistory>> {
        self.with_store(|s| {
            if superseded_uids(s).contains(&uid) {
                return Ok(None); // a superseded version is history, not a live decision
            }
            let Some(decision) = Self::view_from_unit(s, uid) else {
                return Ok(None);
            };
            let grounds = decision.grounds.iter().filter_map(|g| Self::view_from_unit(s, *g)).collect();
            let mut trajectory: Vec<CommitEvent> = Vec::new();
            for u in supersession_chain(s, uid) {
                for c in s.commits_of(&u) {
                    trajectory.push(CommitEvent {
                        level: c.level,
                        agent: c.agent.as_str().to_string(),
                        wall_ms: c.ts.wall_ms,
                        counter: c.ts.counter,
                    });
                }
            }
            trajectory.sort_by_key(|e| (e.wall_ms, e.counter));
            Ok(Some(DecisionHistory { decision, grounds, trajectory }))
        })
    }

    /// CANON-UI-1 (A1) — the set of live manuscript nodes that source at least one
    /// canon decision, for the editor's decision-source marker. Built from the live
    /// decisions' source references (superseded versions already filtered).
    pub fn decision_source_nodes(&self) -> Result<HashSet<Uuid>> {
        Ok(self.all_decisions()?.into_iter().filter_map(|v| v.node).collect())
    }

    /// CANON-3 (CG3-P3) — the grounds DAG as an indented walk: each **root** (a
    /// decision that rests on nothing — a foundation) followed by what transitively
    /// **rests on** it, depth-first, so the foundations and their blast radius read
    /// top-down. `root = Some(id)` shows just that decision's subtree. Live-only; a
    /// decision that rests on several grounds appears under each (it's a DAG). A
    /// path cycle-guard + depth cap keep it finite even on a malformed store.
    pub fn graph(&self, root: Option<Uid>) -> Result<Vec<GraphRow>> {
        self.with_store(|s| {
            let dead = superseded_uids(s);
            let live: Vec<CanonView> = s
                .units()
                .map(|(u, _)| *u)
                .filter(|u| !dead.contains(u))
                .filter_map(|u| Self::view_from_unit(s, u))
                .filter(|v| v.kind.is_some())
                .collect();
            let views: HashMap<Uid, CanonView> = live.iter().map(|v| (v.uid, v.clone())).collect();
            // Reverse edges: for each ground, the live decisions that rest on it.
            let mut deps: HashMap<Uid, Vec<Uid>> = HashMap::new();
            for v in &live {
                for g in &v.grounds {
                    if views.contains_key(g) {
                        deps.entry(*g).or_default().push(v.uid);
                    }
                }
            }
            for children in deps.values_mut() {
                children.sort_by_key(|u| u.canonical());
                children.dedup();
            }

            let mut rows = Vec::new();
            let mut path = Vec::new();
            match root {
                Some(r) => {
                    if !views.contains_key(&r) {
                        return Err(anyhow!("no live canon decision matches that id"));
                    }
                    walk_graph(r, 0, &deps, &views, &mut path, &mut rows);
                }
                None => {
                    // Roots: live decisions resting on no live ground (the foundations).
                    let mut roots: Vec<Uid> = live
                        .iter()
                        .filter(|v| !v.grounds.iter().any(|g| views.contains_key(g)))
                        .map(|v| v.uid)
                        .collect();
                    roots.sort_by_key(|u| u.canonical());
                    for r in roots {
                        walk_graph(r, 0, &deps, &views, &mut path, &mut rows);
                    }
                    // Any decision never reached (a cycle with no root) — list it flat.
                    let seen: HashSet<Uid> = rows.iter().map(|r| r.view.uid).collect();
                    let mut orphans: Vec<Uid> =
                        live.iter().map(|v| v.uid).filter(|u| !seen.contains(u)).collect();
                    orphans.sort_by_key(|u| u.canonical());
                    for u in orphans {
                        if let Some(v) = views.get(&u) {
                            rows.push(GraphRow { depth: 0, view: v.clone() });
                        }
                    }
                }
            }
            Ok(rows)
        })
    }

    /// CANON-2 (CG-P4) — the ledger's commitment log: every `Commit` ever made,
    /// oldest first, as the story's canon settling over time. Includes commits on
    /// now-superseded versions (the honest development record); the gist is that of
    /// the committed version.
    pub fn log(&self) -> Result<Vec<LogEntry>> {
        self.with_store(|s| {
            let mut out: Vec<LogEntry> = Vec::new();
            for rec in s.iter() {
                if let smysl::Record::Commit(c) = rec {
                    let gist = s.get(&c.unit).map(|u| u.core.gist.clone()).unwrap_or_default();
                    out.push(LogEntry {
                        uid: c.unit,
                        gist,
                        level: c.level,
                        agent: c.agent.as_str().to_string(),
                        wall_ms: c.ts.wall_ms,
                        counter: c.ts.counter,
                    });
                }
            }
            out.sort_by_key(|e| (e.wall_ms, e.counter));
            Ok(out)
        })
    }
}

/// One commitment event in a decision's trajectory (CG-P4).
#[derive(Debug, Clone)]
pub struct CommitEvent {
    pub level: Commitment,
    pub agent: String,
    pub wall_ms: u64,
    pub counter: u32,
}

/// A decision's development history (CG-P4): the decision, its grounds, and its
/// commitment trajectory over time.
#[derive(Debug, Clone)]
pub struct DecisionHistory {
    pub decision: CanonView,
    pub grounds: Vec<CanonView>,
    pub trajectory: Vec<CommitEvent>,
}

/// One row of the grounds DAG walk (CG3-P3): a decision at an indentation `depth`
/// (0 = a foundation), where deeper rows *rest on* the row above them.
#[derive(Debug, Clone)]
pub struct GraphRow {
    pub depth: usize,
    pub view: CanonView,
}

/// One entry in the ledger-wide commitment log (CG-P4).
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub uid: Uid,
    pub gist: String,
    pub level: Commitment,
    pub agent: String,
    pub wall_ms: u64,
    pub counter: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impact_why_and_list_over_a_grounded_ledger() {
        let dir = tempfile::tempdir().unwrap();
        let path_s = dir.path().join("canon.cbor").to_str().unwrap().to_string();
        let led = CanonLedger::new(&path_s);
        let n = Uuid::from_u128(0x11);

        // base ← plot (plot rests on base); a standalone trait.
        let base = led
            .record_decision(NarrativeKind::WorldFact, "the harbour freezes each winter", n, "ch1", &[])
            .unwrap();
        let plot = led
            .record_decision(NarrativeKind::PlotPoint, "escape by sea is impossible in winter", n, "ch9", &[base])
            .unwrap();
        let _solo = led
            .record_decision(NarrativeKind::CharacterTrait, "the keeper is stubborn", n, "ch2", &[])
            .unwrap();

        // impact(base): cutting the world-fact dangles the plot-point that rests on it.
        let impact = led.impact(base).unwrap();
        assert!(impact.iter().any(|v| v.uid == plot), "the plot-point is in base's blast radius");
        assert!(!impact.iter().any(|v| v.uid == base), "the target itself is excluded");

        // why(plot): the plot-point rests on the world-fact.
        let why = led.why(plot).unwrap();
        assert!(why.iter().any(|v| v.uid == base), "why(plot) names the world-fact it grounds on");

        // list: three narrative decisions, each carrying its node + gist.
        let all = led.all_decisions().unwrap();
        assert_eq!(all.len(), 3);
        assert!(all.iter().all(|v| v.kind.is_some() && v.node == Some(n)));

        // resolve: the canonical id round-trips; a bad id errors.
        assert_eq!(led.resolve(&base.canonical()).unwrap(), base);
        assert!(led.resolve("b3:zzzzzzzznotreal").is_err());
    }

    #[test]
    fn reground_supersedes_the_old_version_and_relinks_dependents() {
        let dir = tempfile::tempdir().unwrap();
        let path_s = dir.path().join("canon.cbor").to_str().unwrap().to_string();
        let led = CanonLedger::new(&path_s);
        let n = Uuid::from_u128(0x22);

        let base = led
            .record_decision(NarrativeKind::WorldFact, "the city floats on a leviathan", n, "ch1", &[])
            .unwrap();
        let _plot = led
            .record_decision(NarrativeKind::PlotPoint, "the escape uses the sea gate", n, "ch12", &[base])
            .unwrap();
        let extra = led
            .record_decision(NarrativeKind::WorldFact, "the tremors are the beast waking", n, "ch3", &[])
            .unwrap();

        // Reground the *depended-upon* world-fact to also rest on `extra` — the
        // hard case, because the plot-point that rested on the old `base` must be
        // relinked to follow it.
        let base2 = led.reground(base, &[extra]).unwrap();
        assert_ne!(base2, base, "regrounding mints a new content-addressed uid");

        // The old version is superseded → gone from every live view.
        assert!(led.view(base).unwrap().is_none(), "old base is superseded, not viewable");
        let all = led.all_decisions().unwrap();
        assert!(!all.iter().any(|v| v.uid == base), "old base is not listed");
        assert!(all.iter().any(|v| v.uid == base2), "the new base is listed");
        assert_eq!(all.len(), 3, "superseded originals don't inflate the list");

        // base2 now rests on `extra`.
        assert!(led.why(base2).unwrap().iter().any(|v| v.uid == extra), "why(base2) names extra");

        // relink cascaded: the plot-point that rested on the OLD base now follows
        // to base2 — impact/why walk the live edge, and the view is not superseded.
        let live_plot = all.iter().find(|v| v.gist.contains("sea gate")).expect("plot listed").uid;
        assert!(
            led.why(live_plot).unwrap().iter().any(|v| v.uid == base2),
            "the plot-point was relinked onto base2"
        );
        assert!(
            led.impact(base2).unwrap().iter().any(|v| v.uid == live_plot),
            "impact(base2) reaches the relinked plot-point"
        );

        // Adding a ground already present is a no-op: same uid, no new version.
        assert_eq!(led.reground(base2, &[extra]).unwrap(), base2, "no-op reground returns the same uid");
        assert_eq!(led.all_decisions().unwrap().len(), 3, "no-op adds no decision");
    }

    #[test]
    fn unground_removes_an_edge_by_superseding_back() {
        let dir = tempfile::tempdir().unwrap();
        let path_s = dir.path().join("canon.cbor").to_str().unwrap().to_string();
        let led = CanonLedger::new(&path_s);
        let n = Uuid::from_u128(0x55);

        let base = led
            .record_decision(NarrativeKind::WorldFact, "the harbour freezes each winter", n, "ch1", &[])
            .unwrap();
        let plot = led
            .record_decision(NarrativeKind::PlotPoint, "escape by sea waits for the thaw", n, "ch9", &[base])
            .unwrap();
        assert!(led.why(plot).unwrap().iter().any(|v| v.uid == base), "plot rests on base to start");

        let plot2 = led.unground(plot, &[base]).unwrap();
        assert_ne!(plot2, plot, "ungrounding mints a new version");

        let live_plot =
            led.all_decisions().unwrap().into_iter().find(|v| v.gist.contains("escape")).unwrap().uid;
        assert!(led.why(live_plot).unwrap().is_empty(), "the ground is gone");
        assert!(
            led.impact(base).unwrap().iter().all(|v| !v.gist.contains("escape")),
            "base no longer counts the plot in its blast radius"
        );
        // Removing an absent ground is a no-op.
        assert_eq!(led.unground(live_plot, &[base]).unwrap(), live_plot, "no-op unground");
    }

    #[test]
    fn graph_walks_foundations_then_what_rests_on_them() {
        let dir = tempfile::tempdir().unwrap();
        let path_s = dir.path().join("canon.cbor").to_str().unwrap().to_string();
        let led = CanonLedger::new(&path_s);
        let n = Uuid::from_u128(0xA1);

        // base ← plot ← reveal (a chain, grounds set explicitly).
        let base = led
            .record_decision(NarrativeKind::WorldFact, "the city floats on a leviathan", n, "ch1", &[])
            .unwrap();
        let plot = led
            .record_decision(NarrativeKind::PlotPoint, "the escape uses the sea gate", n, "ch12", &[base])
            .unwrap();
        led.record_decision(NarrativeKind::Reveal, "the tremors are the beast", n, "ch9", &[plot]).unwrap();

        // Whole graph: the foundation first, then what rests on it, depth-first.
        let rows = led.graph(None).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].depth, 0);
        assert!(rows[0].view.gist.contains("floats"), "foundation is the leviathan world-fact");
        assert_eq!(rows[1].depth, 1);
        assert!(rows[1].view.gist.contains("sea gate"));
        assert_eq!(rows[2].depth, 2);
        assert!(rows[2].view.gist.contains("tremors"));

        // A subtree rooted at the plot-point: it, then the reveal beneath it.
        let sub = led.graph(Some(plot)).unwrap();
        assert_eq!(sub.len(), 2);
        assert_eq!(sub[0].view.uid, plot);
        assert_eq!(sub[0].depth, 0);
        assert_eq!(sub[1].depth, 1);
    }

    #[test]
    fn commitment_and_history_survive_regrounding() {
        let dir = tempfile::tempdir().unwrap();
        let path_s = dir.path().join("canon.cbor").to_str().unwrap().to_string();
        let led = CanonLedger::new(&path_s);
        let n = Uuid::from_u128(0x66);

        let base = led
            .record_decision(NarrativeKind::WorldFact, "the harbour freezes each winter", n, "ch1", &[])
            .unwrap();
        let plot = led
            .record_decision(NarrativeKind::PlotPoint, "escape by sea waits for the thaw", n, "ch9", &[])
            .unwrap();
        led.commit(plot, Commitment::Canonical, "author").unwrap();
        assert_eq!(led.view(plot).unwrap().unwrap().commitment, Some(Commitment::Canonical));

        // Regrounding mints a new uid whose own commits are empty — the commitment
        // must still read canonical by following the supersession lineage (CG-P4).
        let plot2 = led.reground(plot, &[base]).unwrap();
        assert_ne!(plot2, plot);
        let live =
            led.all_decisions().unwrap().into_iter().find(|v| v.gist.contains("thaw")).unwrap();
        assert_eq!(
            live.commitment,
            Some(Commitment::Canonical),
            "commitment follows the decision across a regrounding"
        );

        // history: the new ground + the earlier commitment, preserved.
        let h = led.history(live.uid).unwrap().expect("history for a live decision");
        assert!(h.grounds.iter().any(|g| g.gist.contains("freezes")), "history shows the added ground");
        assert_eq!(h.trajectory.len(), 1, "the one commitment survives in the trajectory");
        assert_eq!(h.trajectory[0].level, Commitment::Canonical);
        // and the superseded version has no live history (contract guard).
        assert!(led.history(plot).unwrap().is_none(), "a superseded version yields no history");

        // log: the one commitment event, resolved to the committed gist.
        let log = led.log().unwrap();
        assert_eq!(log.len(), 1);
        assert!(log[0].gist.contains("thaw") && log[0].level == Commitment::Canonical);
    }
}
