//! CANON-LEDGER-1 (CL-P0) — the story's development-history ledger, backed by
//! the `smysl` crate (github.com/vulogov/smysl, format `smysl/1.0`).
//!
//! This phase is the **storage substrate only**: an owned wrapper over a
//! `smysl::Store`, persisted per project at `<project>/canon.cbor`. It mirrors
//! the ergonomics of the vector index ([`crate::storage::vector::VectorEngine`]):
//! lazy open on first use, a dirty flag, atomic writes via [`crate::io_atomic`],
//! and an off-thread background flush so a paragraph save never blocks the
//! render thread.
//!
//! The ledger is a **derived** artifact — re-derivable by re-harvest from the
//! manuscript — so a crash mid-flush keeps the last good `canon.cbor` and loses
//! at most the pending append; no user prose is at risk. A corrupt/unreadable
//! file degrades to an empty store (with a warning) rather than failing the open.
//!
//! Later phases build on this substrate: the narrative unit model + host-node
//! bridge (CL-P1), on-save harvest (CL-P2), and the impact / why / diff queries
//! (CL-P3). CL-P0 provides only open / append / count / flush.
//!
//! CL-P2 wired the write path (harvest-on-save); CL-P3 adds the read side
//! (`impact` / `why` / `all_decisions` / `resolve`, in `query.rs`) and the
//! `inkhaven canon` CLI that consumes it; CL-P4 adds commitment + the SMY-W057
//! advisory (`commit.rs`), CL-P5 adds merge + CommitmentFork surfacing
//! (`merge.rs`), CL-P6 adds budget-fit grounded context (`context.rs`). The
//! last substrate method still ahead of its consumer is `count` (awaiting a
//! stats surface); it carries a local `#[allow(dead_code)]` rather than a
//! blanket module one, so genuinely dead code still surfaces.

use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use uuid::Uuid;

use smysl::{
    canonical_uid, compact, from_cbor_seq, relink, to_cbor_seq, Commit, Hlc, Record, RelKind,
    Relation, Status, Store, Uid, UnitCoreBuilder,
};

mod commit;
mod context;
mod grounding;
mod harvest;
mod harvest_llm;
mod merge;
mod model;
mod query;
pub use commit::CommitmentWarning;
pub use context::PackedContext;
pub use harvest::harvest_tags;
pub use harvest_llm::{language_name, parse_proposals, system_prompt, Proposal, StagedCanon};
pub use merge::{CommitmentForkView, MergeSummary};
pub use model::NarrativeKind;
pub use query::{CanonView, DecisionHistory, LogEntry};

/// CANON-3 (CG3-P1) — the outcome of a deterministic backfill
/// ([`CanonLedger::reground_deterministic`]).
#[derive(Debug, Clone, Default)]
pub struct BackfillReport {
    /// Grounds edges added (0 on a dry run).
    pub edges_added: usize,
    /// Distinct decisions that gained (or would gain) an edge.
    pub decisions_touched: usize,
    /// On a dry run, the `(decision gist, ground gist)` edges that would be added.
    pub preview: Vec<(String, String)>,
}

/// CANON-3 (CG3-P2) — the outcome of a compaction ([`CanonLedger::compact`]).
#[derive(Debug, Clone, Default)]
pub struct CompactReport {
    pub records_before: usize,
    pub records_after: usize,
    /// Superseded unit versions dropped.
    pub dropped_units: usize,
}

/// One decision to record via [`CanonLedger::record_grounded_batch`], with the
/// grounds the LLM harvest proposed (CG-P2) carried as gist references — resolved
/// to Uids at record time and unioned with the deterministic inference (CG-P1).
pub struct NewDecision {
    pub kind: NarrativeKind,
    pub gist: String,
    pub node: Uuid,
    pub breadcrumb: String,
    /// Gists of sibling decisions this one rests on, as the model named them.
    /// Empty for a deterministic-only path (e.g. a decision with no model grounds).
    pub proposed_grounds: Vec<String>,
}

/// After this many consecutive background-flush failures, give up the pass
/// (leaving `dirty` set for the next trigger) rather than spinning — the same
/// no-spin guarantee the vector index makes.
const MAX_SYNC_RETRIES: u32 = 5;

/// The background-flush retry policy after one `drain_dirty` attempt. Pure, so
/// the no-spin behaviour is unit-testable without a failing store: returns the
/// new consecutive-failure count, an optional backoff sleep, and whether to give
/// up this pass. Mirrors [`crate::storage::vector`]'s `sync_retry_step`.
fn sync_retry_step(succeeded: bool, failures: u32) -> (u32, Option<std::time::Duration>, bool) {
    if succeeded {
        return (0, None, false);
    }
    let f = failures + 1;
    if f >= MAX_SYNC_RETRIES {
        (f, None, true)
    } else {
        (f, Some(std::time::Duration::from_millis(100 * f as u64)), false)
    }
}

/// Thread-safe handle to a project's canon ledger. The `smysl::Store` is opened
/// lazily on the first operation and held in memory; `append` marks the ledger
/// dirty, and `sync` / `sync_in_background` flush it to `canon.cbor`.
///
/// `Clone` is cheap (shared `Arc`s) so the handle can be moved into background
/// tasks the way `VectorEngine` is.
#[derive(Clone)]
pub struct CanonLedger {
    path: String,
    store: Arc<Mutex<Option<Store>>>,
    dirty: Arc<AtomicBool>,
    /// True while a background flush thread is running, so a burst of appends
    /// spawns at most one such thread (it coalesces later writes).
    sync_in_flight: Arc<AtomicBool>,
    /// Whether saving a paragraph auto-harvests its authored tags into the
    /// ledger (the `canon.harvest_on_save` config knob). Set from config at
    /// `Store::open`; `true` until then, matching the config default.
    harvest_on_save: Arc<AtomicBool>,
}

impl CanonLedger {
    /// Construct a handle for the ledger at `path` (`<project>/canon.cbor`). The
    /// store is not opened until the first operation, so opening a project that
    /// never touches canon costs nothing.
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            store: Arc::new(Mutex::new(None)),
            dirty: Arc::new(AtomicBool::new(false)),
            sync_in_flight: Arc::new(AtomicBool::new(false)),
            harvest_on_save: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Set whether saving a paragraph auto-harvests its tags (config
    /// `canon.harvest_on_save`). Called from [`crate::store::Store::open`].
    pub fn set_harvest_on_save(&self, on: bool) {
        self.harvest_on_save.store(on, Ordering::Relaxed);
    }

    /// Whether on-save tag harvest is enabled (see [`Self::set_harvest_on_save`]).
    pub fn harvest_on_save(&self) -> bool {
        self.harvest_on_save.load(Ordering::Relaxed)
    }

    /// Append records to the ledger and mark it dirty. smysl content-addresses
    /// units, so re-deriving the same decision is idempotent at the graph level.
    pub fn append(&self, records: &[Record]) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }
        let dirty = self.dirty.clone();
        self.with_store(|s| {
            s.append(records)
                .map_err(|e| anyhow!("canon: append failed: {e}"))?;
            dirty.store(true, Ordering::Release);
            Ok(())
        })
    }

    /// Total record count in the ledger. (Consumer: a stats surface in CL-P8.)
    #[allow(dead_code)]
    pub fn count(&self) -> Result<usize> {
        self.with_store(|s| Ok(s.len()))
    }

    /// CL-P1 — record a canon decision derived from an inkhaven node, returning
    /// its content-addressed `Uid`. `grounds` names the decisions this one rests
    /// on (empty for a base fact). Narrative units sit at `Status::Speculative` —
    /// the epistemic axis is inert for fiction; canonicity is the commitment axis
    /// (CL-P4). Marks the ledger dirty via [`Self::append`].
    pub fn record_decision(
        &self,
        kind: NarrativeKind,
        gist: &str,
        node: Uuid,
        breadcrumb: &str,
        grounds: &[Uid],
    ) -> Result<Uid> {
        let core = UnitCoreBuilder::new(kind.schema_id(), gist, Status::Speculative)
            .source(model::node_source(node, breadcrumb))
            .grounds(grounds.iter().copied())
            .build()
            .map_err(|e| anyhow!("canon: invalid {kind:?} decision: {e}"))?;
        let uid = canonical_uid(&core);
        self.append(&[Record::Unit(core)])?;
        Ok(uid)
    }

    /// CANON-2 (CG-P1/P2) — record a batch of new decisions, each **born with its
    /// grounds**: the deterministically-inferred edges (kind rules + salient-word
    /// overlap of gists, free, no model — CG-P1) unioned with any the LLM harvest
    /// *proposed* by gist reference (resolved against this batch + the ledger,
    /// unresolved/ambiguous refs dropped — CG-P2). Ground-providers (world-facts,
    /// setups) are recorded first, so a dependent in the same batch (a reveal, a
    /// plot point) can rest on a same-batch provider as well as on the existing
    /// ledger. Grounds are set at creation, so no supersession is needed. Returns
    /// the recorded uids (in recording order). Caller flushes (`sync`).
    pub fn record_grounded_batch(
        &self,
        items: &[NewDecision],
        language: &crate::prose::ProseLanguage,
    ) -> Result<Vec<Uid>> {
        let lang = grounding::stemmer_language_name(language);
        // Existing live decisions are the initial candidate grounds.
        let mut candidates: Vec<grounding::Candidate> = self
            .all_decisions()?
            .into_iter()
            .filter_map(|v| {
                v.kind.map(|k| grounding::Candidate { uid: v.uid, kind: k, gist: v.gist })
            })
            .collect();
        let mut order: Vec<usize> = (0..items.len()).collect();
        order.sort_by_key(|&i| grounding::ground_rank(items[i].kind));
        let mut out = Vec::with_capacity(items.len());
        for &i in &order {
            let it = &items[i];
            let mut grounds = grounding::infer_grounds(it.kind, &it.gist, lang, &candidates);
            // Union the model-proposed grounds (CG-P2), resolved by gist against
            // the batch-so-far + ledger (the item itself is not yet a candidate,
            // so a ref can't resolve to self).
            for gref in &it.proposed_grounds {
                if let Some(uid) = grounding::resolve_gist_ref(gref, &candidates) {
                    if !grounds.contains(&uid) {
                        grounds.push(uid);
                    }
                }
            }
            let uid = self.record_decision(it.kind, &it.gist, it.node, &it.breadcrumb, &grounds)?;
            candidates.push(grounding::Candidate { uid, kind: it.kind, gist: it.gist.clone() });
            out.push(uid);
        }
        Ok(out)
    }

    /// CANON-3 (CG3-P1) — deterministically backfill grounds over the WHOLE live
    /// ledger, so a ledger recorded before 3.12 (or otherwise under-grounded) gets
    /// the edges CG-P1 would have drawn at creation, with no re-harvest. A
    /// fixpoint: each round infers each live decision's deterministic grounds over
    /// the others and regrounds the first one with a missing edge, then recomputes
    /// (regrounding mints new uids + relinks dependents). `dry_run` reports what it
    /// would add without writing. Idempotent — a second run is a no-op. Flushes on
    /// apply.
    pub fn reground_deterministic(
        &self,
        language: &crate::prose::ProseLanguage,
        dry_run: bool,
    ) -> Result<BackfillReport> {
        let lang = grounding::stemmer_language_name(language);
        // The deterministic grounds a live decision `v` is missing, as candidate
        // uids drawn from `live` (excluding self).
        let missing_for = |v: &CanonView, live: &[CanonView]| -> Vec<Uid> {
            let Some(kind) = v.kind else { return Vec::new() };
            let others: Vec<grounding::Candidate> = live
                .iter()
                .filter(|c| c.uid != v.uid)
                .filter_map(|c| {
                    c.kind.map(|k| grounding::Candidate { uid: c.uid, kind: k, gist: c.gist.clone() })
                })
                .collect();
            grounding::infer_grounds(kind, &v.gist, lang, &others)
                .into_iter()
                .filter(|g| !v.grounds.contains(g))
                .collect()
        };

        if dry_run {
            let live = self.all_decisions()?;
            let mut preview = Vec::new();
            let mut touched = 0usize;
            for v in &live {
                let missing = missing_for(v, &live);
                if missing.is_empty() {
                    continue;
                }
                touched += 1;
                for g in &missing {
                    if let Some(gv) = live.iter().find(|x| &x.uid == g) {
                        preview.push((v.gist.clone(), gv.gist.clone()));
                    }
                }
            }
            return Ok(BackfillReport { edges_added: 0, decisions_touched: touched, preview });
        }

        let mut edges_added = 0usize;
        let mut decisions_touched = 0usize;
        // Each round grounds exactly one decision, and a decision is grounded at
        // most once (its gist/kind are stable, so its inferred edges don't change),
        // so this converges in ≤ (live count) rounds. The cap is belt-and-braces
        // against a pathological relink cascade — the no-hang posture, mirroring
        // `walk_graph`'s depth cap — never expected to bind.
        let max_rounds = self.all_decisions()?.len().saturating_mul(2).saturating_add(16);
        for _ in 0..max_rounds {
            let live = self.all_decisions()?;
            let mut applied = false;
            for v in &live {
                let missing = missing_for(v, &live);
                if missing.is_empty() {
                    continue;
                }
                self.reground(v.uid, &missing)?;
                edges_added += missing.len();
                decisions_touched += 1;
                applied = true;
                break; // uids changed under us — recompute over the live set
            }
            if !applied {
                break;
            }
        }
        self.sync()?;
        Ok(BackfillReport { edges_added, decisions_touched, preview: Vec::new() })
    }

    /// CANON-3 (CG3-P2) — reclaim the append-only churn: drop superseded unit
    /// versions that regrounding / ungrounding left behind (smysl `compact`). Pure
    /// bookkeeping — live decisions keep their ids and edges. Two safeguards before
    /// dropping: **relink** first, so every live reference points at a live unit
    /// (compact keeps a superseded unit anything live still references); and
    /// **carry each live head's commitment onto its own uid** — a regrounded
    /// decision's commitment lives on a superseded predecessor (that's how
    /// `commitment_live` finds it), and compact drops that predecessor's supersedes
    /// edge, so without this the commitment would be lost. Orphaned `Commit` records
    /// on dropped units are filtered out too. Flushes.
    pub fn compact(&self) -> Result<CompactReport> {
        let dirty = self.dirty.clone();
        let report = self.with_store(|s| {
            // (1) Relink so live references point at live units → predecessors become
            // droppable. relink may itself supersede a committed live unit, so this
            // must precede the commitment carry-forward.
            let mut mutated = false;
            let relinked = relink(s);
            if !relinked.records.is_empty() {
                s.append(&relinked.records).map_err(|e| anyhow!("canon compact: relink {e}"))?;
                mutated = true;
            }
            // (2) Pin each live head's inherited commitment onto its own uid.
            let dead = query::superseded_uids(s);
            let live: Vec<Uid> = s.units().map(|(u, _)| *u).filter(|u| !dead.contains(u)).collect();
            let mut carries: Vec<Record> = Vec::new();
            for u in &live {
                if !s.commits_of(u).is_empty() {
                    continue; // already committed on its own uid
                }
                if let Some(c) = query::live_commit(s, *u) {
                    let ts = Hlc::now(&c.ts, &c.agent);
                    carries.push(Record::Commit(Commit::new(*u, c.level, c.agent.clone(), ts)));
                }
            }
            if !carries.is_empty() {
                s.append(&carries).map_err(|e| anyhow!("canon compact: carry commitment {e}"))?;
                mutated = true;
            }
            // (3) Compact, then drop orphaned commits on dropped units, and rebuild.
            let before = s.iter().count();
            let compacted = compact(s);
            let dropped_units = compacted.dropped.len();
            if dropped_units == 0 && !mutated {
                // Nothing superseded and no relink/carry — leave the store untouched
                // rather than rewrite `canon.cbor` with identical bytes.
                return Ok(CompactReport { records_before: before, records_after: before, dropped_units: 0 });
            }
            let dropped = compacted.dropped;
            let records: Vec<Record> = compacted
                .records
                .into_iter()
                .filter(|r| !matches!(r, Record::Commit(c) if dropped.contains(&c.unit)))
                .collect();
            let after = records.len();
            *s = Store::from_records(records);
            dirty.store(true, Ordering::Release);
            Ok(CompactReport { records_before: before, records_after: after, dropped_units })
        })?;
        self.sync()?;
        Ok(report)
    }

    /// CANON-2 (CG-P0) — the supersede primitive. A unit's `grounds` are part of
    /// its content address, so grounds cannot be mutated in place: this rebuilds
    /// the decision (same kind / gist / source) with `mutate`d grounds into a NEW
    /// unit that **supersedes** the old one, then relinks so anything that rested
    /// on the old decision follows to the new one (smysl `relink`, append-only).
    /// `mutate` returns `true` if it changed the set; a no-op returns the existing
    /// `Uid` unchanged. Caller flushes (`sync`).
    fn supersede_grounds(
        &self,
        uid: Uid,
        mutate: impl FnOnce(&mut std::collections::BTreeSet<Uid>) -> bool,
    ) -> Result<Uid> {
        let dirty = self.dirty.clone();
        self.with_store(|s| {
            let unit = s
                .get(&uid)
                .ok_or_else(|| anyhow!("canon: no decision matches id {}", uid.short()))?;
            let core = &unit.core;
            let mut grounds: std::collections::BTreeSet<Uid> = core.grounds.iter().copied().collect();
            let changed = mutate(&mut grounds);
            grounds.remove(&uid); // a decision never grounds on itself
            if !changed {
                return Ok(uid);
            }
            let mut builder = UnitCoreBuilder::new(core.schema.clone(), core.gist.clone(), core.status)
                .grounds(grounds.iter().copied());
            if let Some(src) = core.source.clone() {
                builder = builder.source(src);
            }
            let new_core = builder.build().map_err(|e| anyhow!("canon: reground {}: {e}", uid.short()))?;
            let new_uid = canonical_uid(&new_core);
            s.append(&[
                Record::Unit(new_core),
                Record::Relation(Relation::new(RelKind::Supersedes, new_uid, uid)),
            ])
            .map_err(|e| anyhow!("canon: reground append failed: {e}"))?;
            // Re-point every decision that rested on the old unit onto the new one
            // (append-only corrections + their own supersedes edges, cascading).
            let relinked = relink(s);
            if !relinked.records.is_empty() {
                s.append(&relinked.records)
                    .map_err(|e| anyhow!("canon: reground relink failed: {e}"))?;
            }
            dirty.store(true, Ordering::Release);
            Ok(new_uid)
        })
    }

    /// CANON-2 — add grounds to an existing decision (CG-P0 primitive; consumer:
    /// the manual `canon ground` command, CG-P3). Returns the new (or unchanged,
    /// on a no-op) `Uid`.
    pub fn reground(&self, uid: Uid, added: &[Uid]) -> Result<Uid> {
        self.supersede_grounds(uid, |grounds| {
            let before = grounds.len();
            grounds.extend(added.iter().copied());
            grounds.len() != before
        })
    }

    /// CANON-2 (CG-P3) — remove grounds from a decision (correcting a wrong or
    /// inferred edge), superseding back. Returns the new (or unchanged) `Uid`.
    pub fn unground(&self, uid: Uid, removed: &[Uid]) -> Result<Uid> {
        self.supersede_grounds(uid, |grounds| {
            let before = grounds.len();
            for r in removed {
                grounds.remove(r);
            }
            grounds.len() != before
        })
    }

    /// CL-P1 — the node bridge (reverse lookup): the `Uid`s of every LIVE canon
    /// unit derived from `node`, via the `inkhaven:<uuid>` source-reference prefix
    /// (superseded versions are skipped). Consumed by the CL-P6 query→canon
    /// context bridge.
    pub fn units_for_node(&self, node: Uuid) -> Result<Vec<Uid>> {
        let prefix = model::node_prefix(node);
        self.with_store(|s| {
            let dead = query::superseded_uids(s);
            Ok(s.units_with_source_prefix(&prefix)
                .into_iter()
                .filter(|u| !dead.contains(u))
                .collect())
        })
    }

    /// The live canon units sourced by any of `nodes`, computing the superseded set
    /// **once** for the whole batch (vs. per node). Used by the pre-cut delete guard
    /// over a whole subtree, where per-node scans would be O(nodes × relations).
    pub fn units_for_nodes(&self, nodes: &[Uuid]) -> Result<Vec<Uid>> {
        self.with_store(|s| {
            let dead = query::superseded_uids(s);
            let mut out = Vec::new();
            for node in nodes {
                let prefix = model::node_prefix(*node);
                out.extend(
                    s.units_with_source_prefix(&prefix).into_iter().filter(|u| !dead.contains(u)),
                );
            }
            Ok(out)
        })
    }

    /// Flush to disk, but only when there are unpersisted writes. The clean-path
    /// fast return skips the lock entirely.
    pub fn sync(&self) -> Result<()> {
        if !self.dirty.load(Ordering::Acquire) {
            return Ok(());
        }
        Self::drain_dirty(&self.store, &self.dirty, &self.path)
    }

    /// Flush **off the calling thread** (single-flight). The write is atomic
    /// (temp + rename via `io_atomic`) and the ledger is derived, so backgrounding
    /// it keeps a routine paragraph save from freezing the render thread while the
    /// store serializes. Clean (`!dirty`) states don't spawn; the quit-path
    /// `sync()` stays synchronous, so the ledger always converges.
    pub fn sync_in_background(&self) {
        if !self.dirty.load(Ordering::Acquire) {
            return;
        }
        if self.sync_in_flight.swap(true, Ordering::AcqRel) {
            return;
        }
        let store = self.store.clone();
        let dirty = self.dirty.clone();
        let in_flight = self.sync_in_flight.clone();
        let path = self.path.clone();
        std::thread::spawn(move || {
            // Isolate a panic in the flush from the process-global crash hook so it
            // can't tear down a live terminal, and reset the single-flight flag on
            // panic so a later append can spawn a fresh flush.
            let panic_reset = in_flight.clone();
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::crash::suppress_panic_report(|| {
                    Self::run_sync_loop(&store, &dirty, &in_flight, &path);
                })
            }));
            if outcome.is_err() {
                panic_reset.store(false, Ordering::Release);
                tracing::error!(
                    target: "inkhaven::canon",
                    "background canon flush panicked (isolated + flag reset)"
                );
            }
        });
    }

    /// The retry/backoff flush loop, extracted so the spawn site can wrap it in
    /// panic isolation. Owns nothing; takes shared references.
    fn run_sync_loop(
        store: &Mutex<Option<Store>>,
        dirty: &AtomicBool,
        in_flight: &AtomicBool,
        path: &str,
    ) {
        let mut failures: u32 = 0;
        loop {
            let (next_failures, backoff, give_up) = match Self::drain_dirty(store, dirty, path) {
                Ok(()) => sync_retry_step(true, failures),
                Err(e) => {
                    tracing::warn!(
                        target: "inkhaven::canon",
                        "background canon flush failed (attempt {}): {e}",
                        failures + 1,
                    );
                    sync_retry_step(false, failures)
                }
            };
            failures = next_failures;
            if let Some(delay) = backoff {
                std::thread::sleep(delay);
            }
            // Release the flag, then re-check: an append that set `dirty` during
            // our write is flushed on this same thread. Release BEFORE any give-up
            // so the next append can spawn a fresh flush once an I/O fault clears.
            in_flight.store(false, Ordering::Release);
            if give_up {
                break;
            }
            if !dirty.load(Ordering::Acquire) {
                break;
            }
            if in_flight.swap(true, Ordering::AcqRel) {
                break;
            }
        }
    }

    /// The shared flush body for [`Self::sync`] and [`Self::sync_in_background`]:
    /// under the store lock, re-check dirty (a racing flush may have drained it),
    /// then serialize and write atomically, clearing the flag on success.
    fn drain_dirty(store: &Mutex<Option<Store>>, dirty: &AtomicBool, path: &str) -> Result<()> {
        let guard = store.lock();
        if !dirty.load(Ordering::Acquire) {
            return Ok(());
        }
        let Some(s) = guard.as_ref() else {
            // Shouldn't happen — an append lazily opens the store before it can
            // flip dirty — but stay defensive.
            dirty.store(false, Ordering::Release);
            return Ok(());
        };
        match Self::save_store(s, path) {
            Ok(()) => {
                dirty.store(false, Ordering::Release);
                Ok(())
            }
            Err(e) => Err(anyhow!("failed to flush canon ledger: {e}")),
        }
    }

    fn with_store<R, F: FnOnce(&mut Store) -> Result<R>>(&self, f: F) -> Result<R> {
        let mut guard = self.store.lock();
        if guard.is_none() {
            *guard = Some(
                Self::load_store(&self.path)
                    .map_err(|e| anyhow!("failed to open canon ledger at {:?}: {e}", self.path))?,
            );
        }
        let store = guard.as_mut().expect("set immediately above when None");
        f(store)
    }

    /// Load the ledger from `canon.cbor`, or an empty store when the file is
    /// absent. A corrupt/unreadable file degrades to empty (the ledger is
    /// re-derivable) with a warning, rather than failing the open.
    fn load_store(path: &str) -> Result<Store> {
        let p = Path::new(path);
        if !p.is_file() {
            return Ok(Store::new());
        }
        let bytes = std::fs::read(p).map_err(|e| anyhow!("read {p:?}: {e}"))?;
        match from_cbor_seq(&bytes) {
            Ok((records, _)) => Ok(Store::from_records(records)),
            Err(e) => {
                tracing::warn!(
                    target: "inkhaven::canon",
                    "canon ledger at {p:?} unreadable ({e}); starting empty — a re-harvest will repopulate it"
                );
                Ok(Store::new())
            }
        }
    }

    /// Serialize the store's records to CBOR and write `canon.cbor` atomically.
    fn save_store(store: &Store, path: &str) -> Result<()> {
        let records: Vec<Record> = store.iter().cloned().collect();
        let bytes = to_cbor_seq(&records);
        crate::io_atomic::write(Path::new(path), &bytes)
            .map_err(|e| anyhow!("write canon ledger {path:?}: {e}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smysl::{KernelType, Status, UnitCore, UnitCoreBuilder};

    /// A minimal, shape-valid unit record. `Speculative` needs neither grounds
    /// nor source, so it builds from schema + gist alone — enough to exercise the
    /// store wrapper without the CL-P1 narrative model.
    fn sample_unit(gist: &str) -> Record {
        let core: UnitCore = UnitCoreBuilder::new(KernelType::Claim, gist, Status::Speculative)
            .build()
            .expect("a speculative claim with a gist is shape-valid");
        Record::Unit(core)
    }

    #[test]
    fn append_flush_reopen_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("canon.cbor");
        let path_s = path.to_str().unwrap().to_string();

        {
            let led = CanonLedger::new(&path_s);
            assert_eq!(led.count().unwrap(), 0, "a fresh ledger is empty");
            led.append(&[sample_unit("the harbour freezes over each winter")])
                .unwrap();
            led.append(&[sample_unit("the lighthouse keeper counts the ships")])
                .unwrap();
            assert_eq!(led.count().unwrap(), 2);
            led.sync().unwrap();
            assert!(path.is_file(), "sync wrote canon.cbor");
        }

        // Reopen — records persisted and load (no rebuild).
        {
            let led = CanonLedger::new(&path_s);
            assert_eq!(led.count().unwrap(), 2, "reopened ledger preserves its records");
        }
    }

    #[test]
    fn absent_file_opens_empty_and_clean_sync_is_a_noop() {
        let dir = tempfile::tempdir().unwrap();
        let path_s = dir.path().join("canon.cbor").to_str().unwrap().to_string();
        let led = CanonLedger::new(&path_s);
        // No file yet → empty store, and a clean sync writes nothing.
        assert_eq!(led.count().unwrap(), 0);
        led.sync().unwrap();
        assert!(!Path::new(&path_s).exists(), "a clean ledger writes no file");
    }

    #[test]
    fn narrative_schema_strings_are_valid_extension_ids() {
        use smysl::SchemaId;
        for k in NarrativeKind::ALL {
            let id = SchemaId::parse(k.schema_str()).expect("valid extension id");
            assert!(matches!(id, SchemaId::Extension(_)), "{k:?} is an extension schema");
        }
    }

    #[test]
    fn node_reference_shapes() {
        use super::model::{node_prefix, node_reference};
        let n = Uuid::from_u128(0x1234);
        assert_eq!(node_reference(n, "ch3/scene2"), format!("inkhaven:{n}#ch3/scene2"));
        assert_eq!(node_reference(n, ""), format!("inkhaven:{n}"));
        assert_eq!(node_prefix(n), format!("inkhaven:{n}"));
    }

    #[test]
    fn record_decision_and_node_reverse_lookup() {
        let dir = tempfile::tempdir().unwrap();
        let path_s = dir.path().join("canon.cbor").to_str().unwrap().to_string();
        let led = CanonLedger::new(&path_s);

        let node_a = Uuid::from_u128(0xA);
        let node_b = Uuid::from_u128(0xB);

        // A base world-fact on node A, then a plot-point on node B grounded on it.
        let base = led
            .record_decision(NarrativeKind::WorldFact, "the harbour freezes each winter", node_a, "ch1/scene1", &[])
            .unwrap();
        let plot = led
            .record_decision(NarrativeKind::PlotPoint, "escape by sea is impossible in winter", node_b, "ch9/scene3", &[base])
            .unwrap();
        assert_ne!(base, plot, "distinct decisions get distinct uids");

        // The node bridge isolates units by their source node.
        assert_eq!(led.units_for_node(node_a).unwrap(), vec![base], "node A → its world-fact");
        assert_eq!(led.units_for_node(node_b).unwrap(), vec![plot], "node B → its plot-point");
        assert!(led.units_for_node(Uuid::from_u128(0xC)).unwrap().is_empty(), "unknown node → none");

        // Persists across reopen.
        led.sync().unwrap();
        let led2 = CanonLedger::new(&path_s);
        assert_eq!(led2.units_for_node(node_a).unwrap(), vec![base], "node bridge survives reload");
    }

    #[test]
    fn harvest_to_record_to_lookup_flow() {
        // CL-P2 end to end at the canon layer (no Store/DuckDB): authored tags →
        // harvest_tags → record_decision → units_for_node.
        let dir = tempfile::tempdir().unwrap();
        let path_s = dir.path().join("canon.cbor").to_str().unwrap().to_string();
        let led = CanonLedger::new(&path_s);
        let node = Uuid::from_u128(0x7);

        let tags = vec!["rel:friend:Alice:Bob".to_string(), "pov:Alice".to_string()];
        let decisions = super::harvest_tags(&tags);
        assert_eq!(decisions.len(), 1, "one rel: tag harvests to one decision");
        for (kind, gist) in &decisions {
            led.record_decision(*kind, gist, node, "ch1/scene1", &[]).unwrap();
        }
        assert_eq!(
            led.units_for_node(node).unwrap().len(),
            1,
            "the harvested tag became one canon unit under its node"
        );
    }

    #[test]
    fn grounded_batch_wires_impact_and_why_deterministically() {
        // CG-P1 end to end: a mixed batch, given dependents-first to prove the
        // providers-first ordering fixes it, comes out with edges so impact/why
        // are non-empty — the whole point of "Grounds That Hold".
        use crate::prose::ProseLanguage;
        let dir = tempfile::tempdir().unwrap();
        let path_s = dir.path().join("canon.cbor").to_str().unwrap().to_string();
        let led = CanonLedger::new(&path_s);
        let n = Uuid::from_u128(0x33);

        let mk = |kind, gist: &str, bc: &str| NewDecision {
            kind,
            gist: gist.to_string(),
            node: n,
            breadcrumb: bc.to_string(),
            proposed_grounds: Vec::new(),
        };
        let items = vec![
            mk(NarrativeKind::Reveal, "the tremors are the beast waking", "ch9"),
            mk(NarrativeKind::PlotPoint, "the escape uses the sea gate in the leviathan's flank", "ch12"),
            mk(NarrativeKind::WorldFact, "the city floats on a sleeping leviathan", "ch1"),
            mk(NarrativeKind::Setup, "the tremors shake the lower city", "ch3"),
        ];
        led.record_grounded_batch(&items, &ProseLanguage::En).unwrap();

        let all = led.all_decisions().unwrap();
        assert_eq!(all.len(), 4, "four decisions, no supersession (grounds set at birth)");

        // The leviathan world-fact has the plot-point in its blast radius.
        let leviathan = all.iter().find(|v| v.gist.contains("floats")).unwrap().uid;
        assert!(
            led.impact(leviathan).unwrap().iter().any(|v| v.gist.contains("sea gate")),
            "the plot-point grounds the leviathan world-fact"
        );
        // The reveal rests on the tremor setup (dependents-first input, correctly ordered).
        let reveal = all.iter().find(|v| v.gist.contains("beast waking")).unwrap().uid;
        assert!(
            led.why(reveal).unwrap().iter().any(|v| v.gist.contains("shake the lower city")),
            "the reveal grounds the tremor setup even though it was listed first"
        );
    }

    #[test]
    fn deterministic_backfill_grounds_a_pre_312_ledger() {
        // Simulate a pre-3.12 ledger: decisions recorded with NO grounds (&[]).
        use crate::prose::ProseLanguage;
        let dir = tempfile::tempdir().unwrap();
        let path_s = dir.path().join("canon.cbor").to_str().unwrap().to_string();
        let led = CanonLedger::new(&path_s);
        let n = Uuid::from_u128(0x88);

        let base = led
            .record_decision(NarrativeKind::WorldFact, "the city floats on a sleeping leviathan", n, "ch1", &[])
            .unwrap();
        led.record_decision(NarrativeKind::PlotPoint, "the escape uses the sea gate in the leviathan's flank", n, "ch12", &[]).unwrap();
        led.record_decision(NarrativeKind::Setup, "the tremors shake the lower city", n, "ch3", &[]).unwrap();
        led.record_decision(NarrativeKind::Reveal, "the tremors are the beast waking", n, "ch9", &[]).unwrap();

        // Nothing is grounded yet — impact is empty.
        assert!(led.impact(base).unwrap().is_empty(), "no edges before backfill");

        // Dry run reports edges but writes nothing.
        let dry = led.reground_deterministic(&ProseLanguage::En, true).unwrap();
        assert_eq!(dry.edges_added, 0);
        assert!(!dry.preview.is_empty(), "dry run previews the edges it would add");
        assert!(led.impact(base).unwrap().is_empty(), "dry run wrote nothing");

        // Apply: the edges get wired.
        let rep = led.reground_deterministic(&ProseLanguage::En, false).unwrap();
        assert!(rep.edges_added >= 2, "at least the leviathan and tremor edges");

        let live = led.all_decisions().unwrap();
        let leviathan = live.iter().find(|v| v.gist.contains("floats")).unwrap().uid;
        assert!(
            led.impact(leviathan).unwrap().iter().any(|v| v.gist.contains("sea gate")),
            "the plot-point now rests on the leviathan world-fact"
        );
        let reveal = live.iter().find(|v| v.gist.contains("beast waking")).unwrap().uid;
        assert!(
            led.why(reveal).unwrap().iter().any(|v| v.gist.contains("shake the lower city")),
            "the reveal now rests on the tremor setup"
        );

        // Idempotent — a second apply adds nothing.
        assert_eq!(led.reground_deterministic(&ProseLanguage::En, false).unwrap().edges_added, 0);
    }

    #[test]
    fn compact_drops_superseded_versions_but_keeps_commitment_and_grounds() {
        let dir = tempfile::tempdir().unwrap();
        let path_s = dir.path().join("canon.cbor").to_str().unwrap().to_string();
        let led = CanonLedger::new(&path_s);
        let n = Uuid::from_u128(0x99);

        let base = led
            .record_decision(NarrativeKind::WorldFact, "the harbour freezes each winter", n, "ch1", &[])
            .unwrap();
        let plot = led
            .record_decision(NarrativeKind::PlotPoint, "escape by sea waits for the thaw", n, "ch9", &[])
            .unwrap();
        led.commit(plot, smysl::Commitment::Canonical, "author").unwrap();
        // Reground the committed plot onto base → plot is superseded; its commitment
        // now lives on the superseded uid (found via commitment_live).
        let plot2 = led.reground(plot, &[base]).unwrap();
        assert_ne!(plot2, plot);
        let pre = led.all_decisions().unwrap();
        assert_eq!(
            pre.iter().find(|v| v.gist.contains("thaw")).unwrap().commitment,
            Some(smysl::Commitment::Canonical)
        );

        // Compact: the superseded plot version is dropped, records shrink.
        let rep = led.compact().unwrap();
        assert!(rep.dropped_units >= 1, "the superseded version was dropped");
        assert!(rep.records_after < rep.records_before, "records shrank");

        // The live decision, its grounds, and its commitment all survive.
        let post = led.all_decisions().unwrap();
        assert_eq!(post.len(), 2, "base + the live plot");
        let live = post.iter().find(|v| v.gist.contains("thaw")).unwrap();
        assert_eq!(live.commitment, Some(smysl::Commitment::Canonical), "commitment survived compaction");
        assert!(led.why(live.uid).unwrap().iter().any(|v| v.gist.contains("freezes")), "grounds survived");

        // Idempotent + durable: nothing left to drop, commitment still there.
        assert_eq!(led.compact().unwrap().dropped_units, 0, "second compact is a no-op");
        let led2 = CanonLedger::new(&path_s);
        assert_eq!(
            led2.all_decisions().unwrap().iter().find(|v| v.gist.contains("thaw")).unwrap().commitment,
            Some(smysl::Commitment::Canonical),
            "commitment persists across reopen after compaction"
        );
    }

    #[test]
    fn model_proposed_grounds_draw_an_edge_overlap_would_miss() {
        // CG-P2: a plot point that shares NO salient word with the world-fact —
        // deterministic overlap gives nothing — but the model asserted the
        // dependency by gist reference, so accept wires it.
        use crate::prose::ProseLanguage;
        let dir = tempfile::tempdir().unwrap();
        let path_s = dir.path().join("canon.cbor").to_str().unwrap().to_string();
        let led = CanonLedger::new(&path_s);
        let n = Uuid::from_u128(0x44);

        let items = vec![
            NewDecision {
                kind: NarrativeKind::WorldFact,
                gist: "the moons rise together at midsummer".into(),
                node: n,
                breadcrumb: "ch1".into(),
                proposed_grounds: Vec::new(),
            },
            NewDecision {
                kind: NarrativeKind::PlotPoint,
                gist: "the escape uses the sea gate".into(),
                node: n,
                breadcrumb: "ch12".into(),
                // No shared word, but the model says it rests on the moons fact.
                proposed_grounds: vec!["the moons rise together at midsummer".into()],
            },
        ];
        led.record_grounded_batch(&items, &ProseLanguage::En).unwrap();

        let all = led.all_decisions().unwrap();
        let moons = all.iter().find(|v| v.gist.contains("moons")).unwrap().uid;
        assert!(
            led.impact(moons).unwrap().iter().any(|v| v.gist.contains("sea gate")),
            "the model-proposed ground wired the edge deterministic overlap would have missed"
        );
    }

    #[test]
    fn retry_backs_off_then_gives_up_without_spinning() {
        // success resets and never backs off
        let (f, backoff, give_up) = sync_retry_step(true, 4);
        assert_eq!(f, 0);
        assert!(backoff.is_none() && !give_up);
        // failures back off linearly, then give up at the ceiling
        let mut failures = 0u32;
        let mut gave_up = false;
        for step in 1..=MAX_SYNC_RETRIES {
            let (nf, backoff, give_up) = sync_retry_step(false, failures);
            failures = nf;
            assert_eq!(nf, step);
            if step < MAX_SYNC_RETRIES {
                assert_eq!(backoff.unwrap().as_millis() as u64, 100 * step as u64);
                assert!(!give_up);
            } else {
                assert!(backoff.is_none() && give_up);
                gave_up = true;
            }
        }
        assert!(gave_up, "must give up at the retry ceiling");
    }
}
