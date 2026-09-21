# CANON-3 — "Tend the Ledger" (3.13.0)

*Status: PLAN. Follows CANON-2 (3.12.0, shipped). On `3.13.0-dev`.*

## Why

CANON-2 made the dependency graph populate itself — but only for decisions
recorded *from 3.12.0 on*, and every `canon ground`/`unground` leaves superseded
versions behind (append-only). And now that the graph holds real edges, there is
no way to *see* the whole shape at once. Three maintenance/visibility tools close
that out — small, deterministic, and free, in keeping with the ledger's grain.

## Phases

- **CG3-P1 — `canon reground` (deterministic backfill).** Re-run CG-P1's grounding
  over the *existing* ledger, so a pre-3.12 (or under-grounded) ledger gets its
  edges without a full re-harvest. A fixpoint pass: compute each live decision's
  deterministic grounds (`grounding::infer_grounds` over the other live decisions),
  add any missing via `reground`, and repeat until nothing changes (regrounding
  mints new uids + relinks, so the pass recomputes over the live set each round —
  ledgers are small, so O(n²) regroundings is fine). `--dry-run` reports the edges
  it *would* add without writing. CLI `inkhaven canon reground [--dry-run]`.
  Language resolved as in `accept`. Idempotent: a second run is a no-op.

- **CG3-P2 — `canon compact`.** Expose smysl `compact` (drops superseded units
  nothing surviving references — regrounding already relinked dependents, so the
  old versions are droppable) to reclaim the append-only churn. `CanonLedger::
  compact()` → `smysl::compact(store)` → rebuild from `Compacted.records` → atomic
  write. Reports `dropped` / `records_before → after`. CLI `inkhaven canon compact`.
  Pure bookkeeping — no decision changes, identities of live units unchanged.

- **CG3-P3 — `canon graph` (see the shape).** A textual DAG of the grounds graph:
  each **root** (a decision nothing it depends on — a base world-fact/setup) with
  what transitively **rests on** it, indented, so the foundations and their blast
  radius read at a glance. Deterministic, over the live ledger (`all_decisions` +
  each view's `grounds`, or `dependents`/`topo`). CLI `inkhaven canon graph
  [<id>]` (whole ledger, or the subtree rooted at one decision). Optional
  `ink.canon.graph` Bund read word returning the adjacency (`{uid, gist,
  grounds:[…]}` per decision). No new TUI modal in this pass (the reader-hub
  dashboard already lists decisions; a visual graph is a later nicety if wanted).

- **CG3-P4 — docs + cut.** CANON.md (a "Maintenance" section: reground / compact /
  graph), WORD_REFERENCE (`ink.canon.graph` if added), RELEASE_NOTES/3.13.0.md,
  Manual ch19 + Know-Your-Book ch8 touch. Then cut 3.13.0 (README regen per the
  publish rule).

## The ~1-user gate

Would the author use it? `reground` is the migration path for their own pre-3.12
ledger (the graph is empty on it until this runs); `compact` keeps `canon.cbor`
lean after a bout of hand-grounding; `graph` is the "show me the whole structure"
view the ledger has earned now that edges exist. All deterministic, all free, no
new deps (smysl `compact` + the existing grounding/query layers). Low accretion —
three commands over machinery already in place.

## Non-goals

- A visual/interactive TUI graph (curses canvas) — the textual DAG covers the need
  at ~1-user scale; revisit only on request.
- Auto-compaction on save — compaction stays an explicit, opt-in maintenance act
  (like `reindex`), never a silent background rewrite of the ledger.
