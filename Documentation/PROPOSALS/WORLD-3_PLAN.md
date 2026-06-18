# WORLD-3 — Drift depth + the world report (1.3.11)

_Two threads that round out the world-consistency pillar (WORLD-1 hard facts,
WORLD-2 soft drift):_

1. **Drift depth** — close WORLD-2's stated recall gap: a description in a
   paragraph that never *names* the entity (pure pronouns) is missed today,
   because the name anchor that kills topical false-positives also drops
   "She was tall and pale." A light, precision-favouring coreference pass
   recovers those.
2. **The world report** — `inkhaven world`: one consolidated consistency
   snapshot (facts conflicts + prose-vs-fact + drift + continuity coverage +
   anachronism count), with a health summary, for the terminal and CI — and
   a one-line health line at the top of the story bible.

Zero new dependencies throughout (the coref pass is pure Rust heuristics; the
report aggregates existing sidecars).

---

## P0 — Drift depth: coreference-lite

The WORLD-2 retriever keeps only paragraphs that contain the entity's name.
That's the right precision anchor, but it misses the extremely common
pattern:

```
Mara crossed the yard.            ← names Mara
She was taller than he remembered, ← describes Mara, but only "She"
  and her hair had gone grey.
```

**The heuristic (pure, precision-favouring).** Walk each chapter's paragraphs
in order, tracking the *most recent unambiguously-named* entity of each kind.
A paragraph that names **no** entity but **does** carry a kind-matching
pronoun is attributed to that anchor:

- one entity named in a paragraph → it becomes the anchor for its kind;
- a later name-less paragraph with a matching pronoun
  (character → he/she/they/him/her/them/his/hers/their; place → it/its/there;
  artefact → it/its) attributes to the current anchor;
- **two or more** entities named in a paragraph → the anchor for that kind is
  **cleared** (ambiguous — attribute nothing rather than guess wrong);
- attribution never crosses a chapter boundary.

`drift::attribute_continuations(chapter_paras, lexicon) -> HashMap<Uuid,
String>` (paragraph → entity name) is the pure, unit-tested core. The
retriever's name-filter becomes *"contains the name **or** is coref-attributed
to it"*: `assemble_descriptions` gains a `coref: &HashSet<Uuid>` param (empty
set = today's behaviour, so the existing tests pass with one extra arg).

**Honest limits (state them):** recency-based coref is a heuristic — it favours
precision (single-anchor, cleared on ambiguity) over recall, so it recovers
the common case without inventing attributions; the AI judge remains the
backstop. Cross-paragraph pronoun chains beyond the immediate anchor, and
two-same-kind-characters disambiguation, are out of scope.

**Deliverable:** drift retrieval picks up pronoun-only descriptions adjacent
to a named mention; `drift list` shows the recovered snippets. Pure tests for
the attribution rule (single anchor, ambiguity-clear, chapter reset, pronoun
kinds).

---

## P1 — The world report core

`src/world_report.rs` (pure aggregation over the existing sidecars):

```rust
pub struct WorldReport {
    pub facts_total: usize,             // Facts-book paragraphs
    pub facts_conflicts: Vec<FactConflict>,   // internal (facts check)
    pub facts_prose_findings: usize,    // prose-vs-fact (facts scan)
    pub drift_conflicts: Vec<DriftConflict>,
    pub continuity_attributes: usize,   // tracked character facts
    pub entities: (usize, usize, usize),// characters / places / artefacts
    pub anachronisms: usize,            // from a deterministic edit pass
}
```

- `WorldReport::gather(project) -> Result<Self>` reads `FactCheckReport`,
  `FactScanReport`, `DriftReport`, `ContinuityBible`, the entity books, and
  the anachronism count (the deterministic detector — no AI), tolerating any
  missing sidecar (counts as zero).
- `WorldReport::issue_count()` + a one-line `summary()` (`"World: 3 issue(s)
  — 1 fact conflict · 2 drift"`), the shared health string P2/P3 render.

Pure, unit-tested with hand-built sidecars in a tempdir.

---

## P2 — `inkhaven world`

```sh
inkhaven world            # the consistency dashboard
inkhaven world --json     # for a CI gate (issue_count → exit signal)
inkhaven world --deep     # refresh facts-check + drift + continuity first
```

The human view is a sectioned dashboard — a header health line, then Facts
(established / internal conflicts / prose contradictions), Drift (per-entity
contradictions), Continuity (tracked attributes), and a coverage line
(entities described vs total). It complements, not duplicates, `inkhaven
edit`: `edit` is a *walkable worklist* of everything; `world` is a
*consistency snapshot* of the world layer, grouped by entity/fact, CI-able.

**Deliverable:** a one-shot terminal + JSON consistency report; `--deep`
chains the three AI scans.

---

## P3 — World health in the story bible

The story bible (`Ctrl+V Shift+L`) already lists entities + drift. Add the
`WorldReport::summary()` as a **header line** at the very top (cyan, like the
section headers): `World: 3 issue(s) — 1 fact conflict · 2 drift` — so the
author sees the consistency state at a glance the moment they open the bible.
Reuses P1; no new computation beyond loading the (already-written) sidecars.

**Deliverable:** the bible opens with a one-line world-health banner.

---

## P4 — Docs + 1.3.11 release cut

- **Tutorial 71** — the world report: `inkhaven world`, the health summary,
  `--deep`; a note on drift's new coref recall + its honest limits.
- **KEYBINDING.md** / quick-help — the bible's new health line (no new chord).
- RELEASE_NOTES/1.3.11.md + index row; top README; version bump; signed tag
  `v1.3.11`; `cargo publish`; merge to main; open the next cycle.

---

## Out of scope (carryovers)

- **The Whole-Book AI Editor** (general retrieve-then-reason over any query) —
  the 1.4 headline; the RAG retrieval-scope step toward it stays parked.
- PDF N-up / booklet presets; CMYK-JPEG grayscale; ePub inline images + popup
  footnotes; sixth supported language; TUI `edit --deep` trigger.

## Phase order

P0 (drift depth) and P1 (report core) are independent; P2 + P3 consume P1.
Sequence: **P0 → P1 → P2 → P3 → P4**.
