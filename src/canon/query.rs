//! CL-P3 — the read side: the queries that make the ledger worth keeping.
//!
//! All pure reads over the current store, no model call:
//! - [`CanonLedger::impact`] — the blast radius: everything that would dangle if
//!   a decision were cut (transitive dependents over `grounds`). "What breaks if
//!   I cut this?"
//! - [`CanonLedger::why`] — the grounds chain a decision rests on (`trace`).
//! - [`CanonLedger::all_decisions`] — every narrative decision, for `list`.
//! - [`CanonLedger::resolve`] — a short/prefix id → a unique unit.

use std::collections::HashSet;

use anyhow::{anyhow, Result};
use smysl::{dependents, trace, Commitment, RelKind, SchemaId, Store, TraceKind, Uid};
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
    /// opt-in harvest, or by hand). (Read by the CG-P4 log/dashboard.)
    #[allow(dead_code)]
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
            commitment: store.commitment_of(&uid),
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
}
