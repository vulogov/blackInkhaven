# CANON-2 — "Grounds That Hold" (3.12.0)

*Status: **SHIPPED** on `3.12.0-dev` (CG-P0…P6). Follows CANON-LEDGER-1 (3.11.0).
CG-P0 grounds substrate (supersede + live-only filter) · CG-P1 deterministic
grounding at creation · CG-P2 opt-in harvest proposes grounds · CG-P3 manual
`canon ground`/`unground` + dashboard `g` · CG-P4 `canon log`/`history` + the
commitment-survives-regrounding fix · CG-P5 the pre-cut guard · CG-P6 docs.*

## Why

3.11.0 shipped the Canon Ledger: decisions, commitment levels, and the two
flagship queries — `canon impact` (*"what breaks if I cut this?"*) and `canon
why` (the grounds chain). But those queries **walk the `grounds` dependency
graph, and nothing populates it**: every real ingestion path records a decision
with empty grounds (`document.rs` on-save harvest and `cli/canon.rs` accept both
pass `&[]`; only the unit tests wire edges). So on a real project `impact` and
`why` return nothing — the flagship is inert.

"Grounds That Hold" fixes that (Tier 0), then adds the two things the ledger was
pitched for but did not ship: the **development history** view over smysl's
append-only store (Tier 1), and **impact at the moment of the cut** (Tier 2).
Everything here is deterministic-core and cheap; the one LLM touch (CG-P2) is
opt-in and rides the existing author-confirmed harvest.

## The one design constraint (drives CG-P0)

A decision's `grounds` are part of its `UnitCore`, which is content-addressed:
`Uid = blake3(det_cbor(UnitCore))`. **Grounds are baked into the identity** — you
cannot mutate them in place without changing the Uid. Two consequences:

1. **Grounds are best set at creation.** The primary paths (CG-P1 deterministic,
   CG-P2 LLM) compute grounds *before* `record_decision`, so decisions are born
   with their edges. No mutation needed.
2. **Editing an existing decision's grounds = supersession.** smysl already models
   this (`relink` re-points references onto superseded units; `compact` drops
   superseded units; hop-diffs report `old superseded by new`). So `canon ground`
   (CG-P3, manual) emits a *new* unit — same kind/gist/source, now with the added
   ground — that **supersedes** the old one.

CG-P0 pins the exact smysl supersede API and confirms/adds a **live-only filter**
in the query layer: `all_decisions` today iterates `s.units()` (every unit), so
without a superseded filter a supersede would surface both the old and new
decision. This filter is a prerequisite for CG-P3 and is worth having regardless.

## Phase map (value-ordered: prove the graph free first, then enrich)

- **CG-P0 — the grounds substrate.** Pin the smysl supersede API; add a live-only
  (non-superseded) filter to `all_decisions` / `units_for_node` / the queries;
  a `CanonLedger` helper to re-emit a decision with an added ground (supersede).
  A `grounds` field surfaced on `CanonView` for the dashboard/log. No new user
  surface yet — this makes the rest safe. Guard test: superseding a decision
  shows one live view, and `impact`/`why` follow the live edge.

- **CG-P1 — deterministic grounding at creation (Tier 0b, free).** On accept and
  on the `rel:`-tag on-save harvest, compute grounds with zero-LLM heuristics:
  a `reveal` grounds the `setup` of the same topic; a `plot-point` / `reveal`
  grounds the `world-fact`s whose gist names it reuses (reuse the BONDS/KEN
  Unicode-aware name matcher — multilingual by construction). Conservative: only
  high-confidence overlaps, so the graph seeds itself without noise. This alone
  makes `impact`/`why` non-empty on a tagged project.

- **CG-P2 — LLM harvest proposes grounds (Tier 0a, opt-in).** Extend the harvest
  prompt + `Proposal` so the model, already reading the scene, names which
  *existing ledger decisions or sibling proposals in the same batch* a new
  decision rests on (by gist; resolved to a Uid on accept, fuzzy-matched, skipped
  if ambiguous). Grounds land only through `canon accept` — the author still
  confirms. Multilingual-safe (edges are Uid refs, not prose). Staged view shows
  the proposed grounds so nothing enters unseen.

- **CG-P3 — manual grounds (Tier 0c).** `canon ground <src> --on <dst>` (+ a
  dashboard `g` to link the cursored decision to a picked target) via the CG-P0
  supersede helper — the escape hatch when the author knows an edge the harvest
  missed, and the way to correct a wrong one (`canon unground`, superseding back).

- **CG-P4 — `canon log` / history (Tier 1).** A deterministic read over the
  append-only store: `canon history <id>` shows one decision's commitment
  trajectory (`floated → drafted → … `, with agent + timestamp, from its
  `Commit` records), and `canon log [--since <mark>]` shows what entered or
  changed across the ledger — the *development history of the story's canon* the
  feature was pitched as. Reader-hub dashboard gains the per-decision timeline on
  a keypress. `ink.canon.history` read word.

- **CG-P5 — the pre-cut guard (Tier 2).** When the author deletes a paragraph
  that is a decision's source, compute that decision's `impact` first and warn
  with the blast radius ("source of a *canonical* world-fact that 2 decisions
  rest on — delete anyway?") before the delete proceeds. Puts *"what breaks if I
  cut this"* at the exact moment of cutting. Advisory — it informs, never blocks
  (the permissive principle); a decision with no dependents deletes silently.

- **CG-P6 — docs + release polish.** CANON.md (grounds + history + the guard),
  CONFIGURATION (any new knob), WORD_REFERENCE (`ink.canon.history`),
  RELEASE_NOTES/3.12.0.md, and the Manual (ch19) + Know-Your-Book (ch8) refresh.

**Value core = CG-P0 + CG-P1** (a self-seeding, non-empty dependency graph — the
flagship starts working). CG-P2/P3 enrich it; CG-P4/P5 are the pitched history and
the safety beat.

## The ~1-user gate

Would the author use it? Yes — `impact`/`why` are the reason the ledger exists,
and today they are empty. CG-P4 delivers the "history of story development" framing
the feature was chosen for. CG-P5 is the one moment the author most wants the
answer (deleting a scene). Accretion cost is low: no new store, no new heavy dep
(supersession, name-matching, and the append-only history are all already in the
substrate), deterministic core, one opt-in LLM touch that reuses the existing
harvest. **Fast-track note:** CG-P0 + CG-P1 could ship as **3.11.1** (they fix an
inert shipped query) rather than wait for the 3.12.0 feature bundle — author's call.

## Non-goals / deferred

- **Consolidating CANON with KEN / the Facts bible** (a `reveal` decision vs. a KEN
  reveal; `world-fact`s vs. the continuity bible) — powerful but a separate
  "consolidate" project, not this release.
- A `canon graph` DAG visualization — worth revisiting *after* CG-P1 fills the
  graph, not before.
- Auto-promotion heuristics for commitment level.
