# SENTINEL-1 — Continuity Intelligence: the book watches itself (RFC)

*The 2.2.0 flagship. Status: RFC. Nothing built.*

## Summary

Inkhaven can already *check* a manuscript's continuity — but only if you know
which of six separate commands to run. A "character in two places at once" is
`inkhaven realworld co-location`. A timeline orphan is the timeline critique. A
number that contradicts itself is `doctor --scan`. A character whose eye colour
changed is `continuity extract` + `continuity-drift`. A tavern described two ways
is `drift scan`. A thread introduced and dropped is `tension scan`. Each has its
own sidecar, severity, and mental model. Nobody watches them together, and
nothing watches them *as you write*.

SENTINEL is that layer. Its thesis:

> **Continuity already exists in pieces. SENTINEL makes it one always-watching
> concern — unified over the SEMNET graph, incremental as you type, with the one
> invariant nobody had.**

Three things, all built on what's already in-tree:

- **Unify** — one engine that runs every existing *deterministic* continuity
  detector, normalises their findings into one shape, dedupes, and reports them
  in one ledger, one review-pass line, one config namespace.
- **Complete** — add the single missing deterministic invariant: an entity
  **referenced before it's introduced**.
- **Watch** — make it **incremental**: on save, re-check only what the edit
  touched (the paragraph's characters / places / timeline slice, read off the
  graph), so continuity is checked continuously and cheaply.

SENTINEL is the payoff of 2.0 (SEMNET) and 2.0.1 (GRAPHMIND): the graph is what
makes *unified* and *incremental* continuity feasible — the edited paragraph's
edges tell the engine exactly what to re-check. It is **advisory** (never edits
prose), its core is **deterministic and free** (no LLM), and it adds **no new
runtime crates**.

---

## What already exists (so SENTINEL unifies, it does not rebuild)

This is the whole reason the RFC is honest — five of the "detectors" you'd
imagine building are already here. SENTINEL *orchestrates* them.

| Continuity break | Already detected by | Kind |
| ---------------- | ------------------- | ---- |
| Character in two places at once | `co_location_conflicts` — `src/world/timeline_context.rs:179` | **deterministic, graph-grounded** |
| Timeline orphan / fuzzy-precision overlap | `timeline::critique::run` — `src/timeline/critique/mod.rs:80` | deterministic |
| Numeric / directional self-contradiction | `continuity::detect_contradictions` — `src/continuity.rs:467` | deterministic |
| A character's established fact changed (eye colour, hometown…) | `continuity_bible::detect_drift` — `src/continuity_bible.rs:157` (facts extracted by an LLM pass into `.inkhaven/continuity.json`) | deterministic detect over LLM-extracted facts |
| Same entity described inconsistently ("cramped" vs "airy") | `drift.rs` (WORLD-2) — retrieve-then-LLM-judge | LLM-adjudicated |
| Prose contradicts a structured world fact | `world::fact_check::check_paragraph` — `src/world/fact_check.rs:111` (fast) + `fact_check_slow` COHERENCE (slow) | deterministic + LLM |
| Thread introduced, never resolved | `tension.rs` `unresolved-tension` + thread audit (`Ctrl+V Shift+A`) | LLM-tag + deterministic match |

**The genuinely empty slot:** *an entity referenced before it's introduced.*
Nothing detects it. It is deterministic and graph-friendly — SENTINEL adds it.

**The genuinely new capability:** *nobody watches these together or
incrementally.* Every one is on-demand (a CLI), on-save (a doctor scan), or an
explicit keybind; the config is scattered across `editor.echo_*`,
`timeline.critique`, `continuity extract`, and doctor `--class` names, with **no
`continuity:` namespace at all**. SENTINEL is the unified, proactive front.

---

## The model

SENTINEL speaks one vocabulary over the existing detectors:

- A **`ContinuityFinding`** is the normalised shape every detector maps to:
  `{ kind, severity, chapter, anchor (paragraph id, for jump), entities: [names],
  message, source (which detector), dedup_key }`.
- A **detector source** is one existing engine wrapped behind a thin adapter
  (`co_location`, `timeline_critique`, `numeric`, `char_facts`,
  `referenced_before_introduced`, …). Each adapter *calls* the existing function
  and maps its native finding type into `ContinuityFinding`. **No detection
  logic is re-implemented.**
- The **ledger** is the deduped, ranked set of current findings — computed on
  demand (the detectors are deterministic and their inputs are cached), surfaced
  as the dashboard, the review-pass line, and `continuity check`.
- A **dirty scope** is the set of entities / chapters an edit touched, used by
  the incremental engine to re-check a slice instead of the whole book.

Everything is **advisory**: SENTINEL reports; it never edits prose. Its core
detectors are deterministic; the fuzzy ones (drift, fact-check-coherence) it can
*invoke* on demand but never runs automatically (they cost LLM calls).

---

## Pillar 1 — Unify (the engine)

A new `src/continuity_intel/` module with a `ContinuityFinding` type and an
adapter per existing deterministic detector. `run(store, cfg, layout, book) ->
Vec<ContinuityFinding>` fans out to:

- `co_location_conflicts` (reuse `world::timeline_context::gather_events` +
  `co_location_conflicts`),
- `timeline::critique::run`,
- `continuity::detect_contradictions` (over the manuscript's sentences),
- `continuity_bible::detect_drift` (over the existing `.inkhaven/continuity.json`
  — SENTINEL reads it, it doesn't re-extract; extraction stays `continuity
  extract`),
- Pillar-2's new detector,

then **normalises → dedupes** (`dedup_key` folds a co-location and a fact-check
travel-time complaint about the same pair into one) **→ ranks** (Contradiction >
Warning > Info; earlier chapters first). The output is *the* continuity picture.

The LLM detectors (`drift`, `fact_check_slow` coherence, `tension`) are **not**
in the deterministic sweep — they stay their own commands, and SENTINEL's
dashboard offers a one-key "run the slow coherence pass on this scope" that
*invokes* them (Pillar 4), never re-implementing them.

## Pillar 2 — Complete (the missing invariant)

**Referenced before introduced.** Deterministic. For each declared entity (a
Characters/Places system-book entry, cross-checked with `Declares` edges), find:
its **introduction position** (the reading-order position of its entry / first
scene) and its **first mention** in the manuscript prose. Manuscript prose is
walked in `Hierarchy::flatten()` reading order; a mention is
`drift::mentions(prose_lc, name_lc)` (`src/drift.rs:196`, Unicode word-boundary,
already the tree's mention primitive). If first-mention position precedes the
introduction position by more than a tolerance, flag it: *"‘Aldous the ferryman'
is named in ch.2 but not introduced until ch.5."* Multilingual for free (the
mention primitive is Unicode-aware; names come from the project's own system
books).

## Pillar 3 — Watch (incremental / proactive)

Today continuity is checked in bulk. SENTINEL makes it **incremental**: on a
paragraph save, compute the edit's **dirty scope** from the graph — the
characters / places the paragraph involves (`EventInvolves` edges / `node.event`)
and the entities it mentions — and re-run only the detectors that depend on that
scope (co-location for those characters' timeline slice; the referenced-before
check for those entities; numeric/direction locally). The delta lands in the
Output pane immediately, ambiently, at deterministic sub-second cost. This is the
"watches itself" experience, and it is **only feasible because the graph already
knows what a paragraph touches** — the SEMNET/GRAPHMIND payoff.

Gated + throttled per the permissive principle (a `continuity.ambient` toggle +
cooldown, like the prose/poet ambient scans).

## Pillar 4 — The surface

- **`inkhaven continuity check`** — the unified deterministic ledger (extends the
  existing `continuity` command family, beside `extract`/`list`), per-detector
  `--only`/`--skip`, `--json`, nonzero exit on Contradiction-severity for CI.
- **The review pass** — a `continuity` line in `Ctrl+B Shift+C`
  (`run_continuity_check` into `run_unified_check` **and** `cli/check.rs`),
  emitting `kinds::CONTINUITY` to the Output pane.
- **The dashboard** — a `Ctrl+B` chord opening a scrollable **continuity ledger**
  modal (findings by kind, jump-to-paragraph), plus the one-key "run the slow
  coherence pass on this scope".
- **The `continuity:` config namespace** — the missing unified config: per-
  detector toggles + thresholds + the ambient flag, folding the scattered knobs
  under one roof (without breaking the existing `timeline.critique` etc.).
- **`ink.continuity.*` Bund words** — `findings` / `check` (the BUND-2.2 pattern),
  so scripts and hooks read the ledger.

---

## Multilingual

SENTINEL inherits the coverage of the detectors it unifies. The new
referenced-before-introduced invariant is language-safe (Unicode mention
matching; names from the project's own books). The numeric detector is EN/FR/ES;
co-location, timeline critique, and the character-fact drift are multilingual as
built. SENTINEL never claims coverage a detector doesn't have — a finding carries
its source, and the "does it work in Russian?" answer is "as well as each
underlying detector, and the new one works everywhere."

---

## Principles

- **Advisory, deterministic, free at the core.** The always-on sweep is zero-AI;
  the LLM coherence/drift passes are explicit, cost-capped, opt-in.
- **Unify, don't duplicate.** Every deterministic detector is *called*, never
  re-implemented; the adapters are thin normalisers.
- **Graph-grounded.** The dirty-scope and the entity enumeration come from the
  SEMNET graph — SENTINEL is the reason the graph earns its keep.
- **No new crates; warning-free; the 1.2.15 bar.**

## What SENTINEL is *not*

- Not a new pile of detectors — it's the unification of the ones you have, plus
  one.
- Not LLM-first — the fuzzy passes (drift, fact-check-coherence) stay their own
  explicit, cost-capped commands; SENTINEL orchestrates the deterministic core.
- Not a corrector — it flags, it never rewrites.

---

## Phases

The grounded, file-by-file plan is in
[`SENTINEL-1_IMPL.md`](SENTINEL-1_IMPL.md): **CT-P0** substrate (public
`edges_of_kind` + `ContinuityFinding`) → **P1** referenced-before-introduced →
**P2** the unification engine + `continuity check` → **P3** the `continuity:`
config → **P4** the review-pass line → **P5** the incremental/ambient watch →
**P6** the dashboard modal → **P7** invoke-the-slow-pass wiring → **P8** Bund
`ink.continuity.*` + docs → **P9** capstone. Value core = P1 (the new invariant)
+ P2 (unify) + P4 (review pass) + P5 (watch).
