//! CL-P5 — merge another ledger, and surface CommitmentForks (SMY-W058).
//!
//! A commitment *fork* is concurrent disagreement: two agents' latest
//! commitments to the same decision differ about how settled it is, with
//! neither causally after the other. A single local store with serialized
//! commits (CL-P4) never forks — so forks arise only when an **independently
//! committed** ledger merges in: the author on another device/session, or a
//! model-reader that committed proposed levels on its own store. [`merge_from`]
//! is that entry point; [`commitment_forks`] surfaces the disagreements it (or
//! any prior merge) produced, for the author to resolve — the ledger records the
//! contention rather than silently picking a winner.

use std::sync::atomic::Ordering;

use anyhow::{anyhow, Result};
use smysl::{
    from_cbor_seq, merge, review_with, Commitment, DetectionKind, MergeOptions, ReviewOptions,
    ReviewSubject, Store,
};

use super::query::CanonView;
use super::CanonLedger;

/// What a `merge_from` did.
#[derive(Debug, Clone)]
pub struct MergeSummary {
    pub added: usize,
    pub duplicates: usize,
    /// Commitment forks present in the merged ledger.
    pub forks: usize,
}

/// A surfaced commitment fork: the decision and the disagreeing `(agent, level)`
/// commitments recorded on it.
#[derive(Debug, Clone)]
pub struct CommitmentForkView {
    pub unit: CanonView,
    pub positions: Vec<(String, Commitment)>,
    pub resolved: bool,
}

impl CanonLedger {
    /// Merge the canon ledger at `other_path` (another `canon.cbor`) into this
    /// one — a set-union by content address, order-independent, recording any
    /// concurrent commitment disagreements as forks rather than picking a winner.
    /// Flushes on success.
    pub fn merge_from(&self, other_path: &str) -> Result<MergeSummary> {
        let bytes =
            std::fs::read(other_path).map_err(|e| anyhow!("read {other_path:?}: {e}"))?;
        let (records, _) =
            from_cbor_seq(&bytes).map_err(|e| anyhow!("parse {other_path:?}: {e}"))?;
        let other = Store::from_records(records);

        let report = self.with_store(|s| {
            merge(s, &other, MergeOptions::default()).map_err(|e| anyhow!("canon merge: {e}"))
        })?;
        self.dirty.store(true, Ordering::Release);
        self.sync()?;

        let forks = report
            .contentions
            .iter()
            .filter(|c| c.detected.kind == DetectionKind::CommitmentFork)
            .count();
        Ok(MergeSummary {
            added: report.added,
            duplicates: report.duplicates,
            forks,
        })
    }

    /// The commitment forks in the ledger: decisions whose agents disagree on how
    /// settled they are (SMY-W058), each with the disagreeing `(agent, level)`
    /// commitments. Detected on demand, so it reflects the current store.
    pub fn commitment_forks(&self) -> Result<Vec<CommitmentForkView>> {
        self.with_store(|s| {
            let mut out = Vec::new();
            for item in review_with(s, &ReviewOptions::default()) {
                let ReviewSubject::Contention(c) = item.subject else { continue };
                if c.detected.kind != DetectionKind::CommitmentFork {
                    continue;
                }
                let Some(unit) = Self::view_from_unit(s, c.over) else { continue };
                let positions: Vec<(String, Commitment)> = s
                    .commits_of(&c.over)
                    .iter()
                    .map(|cm| (cm.agent.to_string(), cm.level))
                    .collect();
                out.push(CommitmentForkView {
                    unit,
                    positions,
                    resolved: item.resolved,
                });
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
    fn merging_independent_commitments_surfaces_a_fork() {
        let dir = tempfile::tempdir().unwrap();
        let a_path = dir.path().join("a.cbor").to_str().unwrap().to_string();
        let b_path = dir.path().join("b.cbor").to_str().unwrap().to_string();
        let n = Uuid::from_u128(0x55);

        // Store A: the author commits the decision canonical.
        let a = CanonLedger::new(&a_path);
        let xa = a
            .record_decision(NarrativeKind::WorldFact, "the sea gate is sealed in winter", n, "ch1", &[])
            .unwrap();
        a.commit(xa, Commitment::Canonical, "human:author").unwrap();
        assert!(a.commitment_forks().unwrap().is_empty(), "one agent → no fork");

        // Store B: a reader commits the SAME decision (same content → same uid),
        // independently, only floated.
        let b = CanonLedger::new(&b_path);
        let xb = b
            .record_decision(NarrativeKind::WorldFact, "the sea gate is sealed in winter", n, "ch1", &[])
            .unwrap();
        assert_eq!(xa, xb, "identical content → identical content-addressed uid");
        b.commit(xb, Commitment::Floated, "model:reader").unwrap();

        // Merge B into A → the two independent commitments fork.
        let summary = a.merge_from(&b_path).unwrap();
        assert!(summary.forks >= 1, "merge reports a commitment fork");

        let forks = a.commitment_forks().unwrap();
        assert_eq!(forks.len(), 1, "exactly one forked decision");
        assert_eq!(forks[0].unit.uid, xa);
        assert!(
            forks[0].positions.len() >= 2,
            "both agents' commitments are shown: {:?}",
            forks[0].positions
        );
        assert!(
            forks[0].positions.iter().any(|(a, _)| a.starts_with("human:"))
                && forks[0].positions.iter().any(|(a, _)| a.starts_with("model:")),
            "the fork names both the human and the model position"
        );
    }
}
