//! CL-P4 — author commitment marking + the SMY-W057 support-check advisory.
//!
//! The commitment axis (`Floated → Drafted → Committed → Canonical → Retconned`)
//! is how settled a decision is — canonicity, the quantity fiction actually
//! tracks (the epistemic `Status` stays inert, at `Speculative`). A commitment is
//! a smysl `Commit` record; "how settled is this unit" is the latest commit by
//! `(ts, agent)` (smysl rule U), read back via `commitment_of`.
//!
//! [`CanonLedger::commitment_warnings`] surfaces the SMY-W057 shape: a decision
//! committed *above* the weakest explicitly-committed thing it rests on — the
//! "canonical scene built on sand" advisory. It mirrors smysl's rule and skips:
//! a unit with no commitment is not checked (silence ≠ `Floated`), a `Retconned`
//! unit is skipped, and an *unmarked* ground is not treated as weak.

use anyhow::{anyhow, Result};
use smysl::{AgentId, Commit, Commitment, Hlc, Record, Uid};

use super::query::CanonView;
use super::CanonLedger;

/// A SMY-W057-shaped advisory: `unit` (at `level`) rests on `weakest_ground`
/// (at the lower `ground_level`).
#[derive(Debug, Clone)]
pub struct CommitmentWarning {
    pub unit: CanonView,
    pub level: Commitment,
    pub weakest_ground: CanonView,
    pub ground_level: Commitment,
}

impl CanonLedger {
    /// Record an author commitment for `uid` at `level`. `agent` is an agent id
    /// (`human:name`); a bare name is taken as `human:<name>`. The new commit
    /// advances the unit's clock so it becomes the latest (rule U), then the
    /// ledger is flushed. Idempotent-ish: re-committing the same level appends a
    /// fresh, later commit that resolves to the same answer.
    pub fn commit(&self, uid: Uid, level: Commitment, agent: &str) -> Result<()> {
        let agent_str = if agent.contains(':') {
            agent.to_string()
        } else {
            format!("human:{agent}")
        };
        let agent_id =
            AgentId::new(agent_str).map_err(|e| anyhow!("canon: invalid agent id {agent:?}: {e}"))?;

        // Advance from the unit's latest existing commit (any agent) so this one
        // is strictly the newest on the unit; seed from zero when it's the first.
        let prev = self.with_store(|s| {
            Ok(s.commits_of(&uid)
                .iter()
                .map(|c| c.ts.clone())
                .max()
                .unwrap_or_else(|| Hlc::zero(agent_id.clone())))
        })?;
        let ts = Hlc::now(&prev, &agent_id);

        self.append(&[Record::Commit(Commit::new(uid, level, agent_id, ts))])?;
        self.sync()
    }

    /// The current (latest-wins) commitment level for `uid`, or `None` if unmarked.
    /// (Consumer: a `canon show` surface in CL-P8; the list/why/impact views read
    /// commitment through `CanonView` instead.)
    #[allow(dead_code)]
    pub fn commitment(&self, uid: Uid) -> Result<Option<Commitment>> {
        self.with_store(|s| Ok(s.commitment_of(&uid)))
    }

    /// The SMY-W057-shaped advisories: decisions committed above the weakest
    /// explicitly-committed thing they rest on. Deterministic; no model call.
    pub fn commitment_warnings(&self) -> Result<Vec<CommitmentWarning>> {
        self.with_store(|s| {
            // CANON-2: only live decisions, and follow supersession for commitment.
            // A regrounded decision's commitment lives on an earlier uid (grounds
            // are part of the content hash, so regrounding mints a new uid with no
            // commit of its own); without `commitment_live` a canonical-on-floated
            // situation on a regrounded decision would escape the check, and the
            // stale superseded version would be checked in its place.
            let dead = super::query::superseded_uids(s);
            let uids: Vec<Uid> =
                s.units().map(|(u, _)| *u).filter(|u| !dead.contains(u)).collect();
            let mut out = Vec::new();
            for u in uids {
                let Some(level) = super::query::commitment_live(s, u) else { continue }; // unmarked → not checked
                if level == Commitment::Retconned {
                    continue; // a retcon resting on something weaker is expected
                }
                let grounds: Vec<Uid> = match s.get(&u) {
                    Some(unit) => unit.core.grounds.iter().copied().collect(),
                    None => continue,
                };
                // The weakest ground that is *explicitly* committed below `level`.
                let mut weakest: Option<(Uid, Commitment)> = None;
                for g in grounds {
                    if let Some(gl) = super::query::commitment_live(s, g) {
                        if gl < level && weakest.map_or(true, |(_, wl)| gl < wl) {
                            weakest = Some((g, gl));
                        }
                    }
                }
                if let Some((gu, gl)) = weakest {
                    if let (Some(uv), Some(gv)) =
                        (Self::view_from_unit(s, u), Self::view_from_unit(s, gu))
                    {
                        out.push(CommitmentWarning {
                            unit: uv,
                            level,
                            weakest_ground: gv,
                            ground_level: gl,
                        });
                    }
                }
            }
            Ok(out)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canon::NarrativeKind;
    use uuid::Uuid;

    #[test]
    fn commit_sets_commitment_and_flags_building_on_sand() {
        let dir = tempfile::tempdir().unwrap();
        let path_s = dir.path().join("canon.cbor").to_str().unwrap().to_string();
        let led = CanonLedger::new(&path_s);
        let n = Uuid::from_u128(0x42);

        let base = led
            .record_decision(NarrativeKind::WorldFact, "there is a hidden sea gate", n, "ch1", &[])
            .unwrap();
        let ending = led
            .record_decision(NarrativeKind::PlotPoint, "the escape uses the sea gate", n, "ch12", &[base])
            .unwrap();

        // Unmarked → no commitment, and nothing to warn about.
        assert_eq!(led.commitment(ending).unwrap(), None);
        assert!(led.commitment_warnings().unwrap().is_empty());

        // Commit the ending as canonical, but the premise it rests on only floated.
        led.commit(ending, Commitment::Canonical, "author").unwrap();
        led.commit(base, Commitment::Floated, "author").unwrap();
        assert_eq!(led.commitment(ending).unwrap(), Some(Commitment::Canonical));

        // W057: a canonical decision resting on a floated premise is flagged.
        let warns = led.commitment_warnings().unwrap();
        assert_eq!(warns.len(), 1, "one canonical-on-sand warning");
        assert_eq!(warns[0].unit.uid, ending);
        assert_eq!(warns[0].level, Commitment::Canonical);
        assert_eq!(warns[0].weakest_ground.uid, base);
        assert_eq!(warns[0].ground_level, Commitment::Floated);

        // Promote the premise to canonical → the warning clears.
        led.commit(base, Commitment::Canonical, "author").unwrap();
        assert!(led.commitment_warnings().unwrap().is_empty(), "no longer built on sand");

        // Persists + latest-wins across reopen.
        let led2 = CanonLedger::new(&path_s);
        assert_eq!(led2.commitment(base).unwrap(), Some(Commitment::Canonical));
    }

    #[test]
    fn check_follows_regrounding_and_skips_superseded() {
        // Regression (CANON-2): after `canon ground`, the check must still flag a
        // canonical decision resting on a floated one — the commitment lives on the
        // pre-reground uid — and must not double-count or report the dead version.
        let dir = tempfile::tempdir().unwrap();
        let path_s = dir.path().join("canon.cbor").to_str().unwrap().to_string();
        let led = CanonLedger::new(&path_s);
        let n = Uuid::from_u128(0x77);

        let base = led
            .record_decision(NarrativeKind::WorldFact, "there is a hidden sea gate", n, "ch1", &[])
            .unwrap();
        let ending = led
            .record_decision(NarrativeKind::PlotPoint, "the escape uses the sea gate", n, "ch12", &[base])
            .unwrap();
        let extra = led
            .record_decision(NarrativeKind::WorldFact, "the tide turns at dawn", n, "ch3", &[])
            .unwrap();
        led.commit(ending, Commitment::Canonical, "author").unwrap();
        led.commit(base, Commitment::Floated, "author").unwrap();
        assert_eq!(led.commitment_warnings().unwrap().len(), 1, "flagged before regrounding");

        // Reground the canonical ending onto a second premise → it gets a new uid
        // with no commit of its own. The check must follow the lineage.
        let ending2 = led.reground(ending, &[extra]).unwrap();
        assert_ne!(ending2, ending);
        let warns = led.commitment_warnings().unwrap();
        assert_eq!(warns.len(), 1, "still exactly one warning after regrounding (not 0, not 2)");
        assert_eq!(warns[0].unit.uid, ending2, "reported against the live version, not the superseded one");
        assert_eq!(warns[0].level, Commitment::Canonical, "commitment followed the regrounding");
        assert_eq!(warns[0].weakest_ground.uid, base);
        assert_eq!(warns[0].ground_level, Commitment::Floated);
    }
}
