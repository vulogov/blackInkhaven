# LECTOR-1 — The Read-Through: the book reads itself, end to end (RFC)

*The 2.3.0 flagship. Status: RFC. Nothing built.*

## Summary

Every intelligence Inkhaven has built reads *small*. The inner readers (Socrates,
Editor, Theologian, Poet, Rigor, Stylist) work a paragraph or a window. SENTINEL
watches *breaks*. CHORUS measures *voice*. The Planning Board declares a *shape*.
Nothing reads the manuscript the way the one reader who matters most does — the
**first reader**: forward, once, whole, not knowing the ending, holding the whole
book in their head as it accumulates.

LECTOR is that reader. Its thesis:

> **No feature reads your book end to end. LECTOR does — reporting both the *shape*
> of the read (structure & pacing, measured from the prose, not declared) and the
> *experience* of the read (clarity, attention, stakes, payoff) — the way a
> thoughtful first reader with a ruler would.**

It is the zoom-out complement to everything 2.x built: where SENTINEL asks "does
any fact contradict another?", LECTOR asks "does the **whole thing hold together
as a read** — does it rise, does it land, would someone keep turning pages?"

Two halves, one artifact:

- **SHAPE** — the *structural* read-through. Mostly deterministic. Measures the
  book's realized dramatic-intensity/pacing curve **from the prose itself** and
  compares it to the intended shape; adds the scene/sequel micro-structure axis.
- **AUDIENCE** — the *reader's* read-through. LLM-forward but graph-grounded. Reads
  the book chapter by chapter, **forward**, carrying state (who's been introduced,
  what's open, what stakes are live), and reports where a first reader gets
  confused, bored, lost, or unrewarded — and where they'd put it down.

The synthesis is **the read-through report**: one editorial-letter-shaped artifact
that opens *"here is how your book reads,"* the shape curve and the reader beats
side by side, per chapter.

LECTOR is **advisory** (it reports; it never edits — the rewrite loop is the
Editorial Pass's job), **deterministic where it can be** (the whole SHAPE half and
the structural reader-signals are zero-AI), and its one LLM pass (the synthetic
first-read) is **explicit and cost-capped**, never automatic.

---

## What already exists (so LECTOR unifies + completes, it does not rebuild)

This is the honest core of the RFC — a great deal of the *structural* half is
already in the Planning Board, and the *reader* half has real scaffolding.

| Capability | Already in-tree | Kind |
| ---------- | --------------- | ---- |
| Story frameworks + beats + intended curve | `planning.rs` — 5 `Framework`s, `BeatSpec{target_position, expected_tension}` | declared |
| Beat → chapter mapping | `plan analyze` (over the book digest) | deterministic |
| Structure findings (gap / midpoint drift / act imbalance / pacing) | `plan check` | deterministic |
| **Expected-vs-actual-vs-AI tension curve + sparkline** | `planning.rs::tension_curve` + `intensity_sparkline` + `plan tension rate` (LLM) | deterministic + LLM |
| Unresolved setups ("the gun that never fires") | `tension.rs` (introduce/resolve threads) | LLM-tag + deterministic match |
| Referenced-before-introduced ("who is this again?") | `continuity_intel/introduce.rs` (SENTINEL CT-P1) | deterministic, graph-grounded |
| Character arcs + agency over time | `character/` (CHAR-1) | deterministic + LLM |
| Reader personas + genre framing + grounding prefix | `inner_socrates/personas.rs` (AUDIENCE-1) + `inner_grounding.rs` | LLM |
| Dialogue density / attribution | `dialogue/` (DIALOG-1) | deterministic |
| Sentence rhythm / readability / reading time | NARR-1 + `tui/readability.rs` / `reading_time.rs` | deterministic |
| Cost-capped whole-book LLM pass pattern | `world::fact_check_slow` + `slow_llm_call` (reused by SENTINEL CT-P7) | LLM |

**The two genuinely empty slots:**

1. **The realized curve is *declared*, not *measured*.** `tension_curve`'s "actual"
   line is built from author-tagged tensions/threads (`has_actual` is false without
   them). Nothing measures dramatic intensity **from the prose** — so the shape
   analysis is blind on an untagged draft, which is exactly the draft that needs it.
2. **Nobody reads the book forward as a first reader.** The personas interrogate a
   window; they don't carry reading state across the whole book to catch "you
   introduced four names in three pages," "this setup is still open in ch. 9," "the
   momentum dies after the climax — I'd stop here."

LECTOR fills both, and unifies the existing structural pieces into one report.

---

## The model

LECTOR speaks one vocabulary over a **forward walk** of the manuscript:

- A **`ChapterRead`** is what one chapter looks like on the read-through:
  `{ chapter, measured_intensity, new_entities, opened_threads, resolved_threads,
  live_stakes, findings }`. The walk is **stateful and ordered** — each chapter's
  `ChapterRead` is computed with the accumulated state of every chapter before it
  (which entities are known, which threads are open), and **never** with knowledge
  of what comes after. That forward-only discipline is what makes it a *reader*.
- A **shape signal** is the deterministic dramatic-intensity measured from a
  chapter's prose (see SHAPE). The sequence of them is the realized curve.
- A **reader finding** is a first-reader problem: a *confusion* (an entity used
  before it's introduced — LECTOR reuses SENTINEL's `introduce`), an *attention
  dip* (measured intensity low and little new information), an *info dump* (many
  new entities at once), an *unpaid setup* (a thread open too long / open at the
  end — reuses `tension.rs`), a *stakes gap*, a *put-down risk* (a sustained
  low-intensity, low-progress run).
- The **read-through report** is the deduped, ordered set of chapter reads + the
  shape curve + the ranked findings — surfaced as the dashboard, the review-pass
  line, and `inkhaven readthrough`.

Everything is **advisory**. The deterministic half is free; the one LLM pass (the
synthetic first-read) is explicit and cost-capped.

---

## Pillar 1 — SHAPE (the structural read-through)

Make the Planning Board's shape analysis **empirical and tagging-free**.

- **Prose-measured intensity.** A deterministic per-chapter dramatic-intensity
  signal built from signals already computable: dialogue density (DIALOG-1),
  conflict/stakes vocabulary (a per-language lexicon, the `built_in_lexicon`
  pattern), sentence-rhythm acceleration (NARR-1 — short punchy sentences read as
  high intensity), scene-vs-summary ratio, a cliffhanger/turn at the chapter end,
  plus the existing tension-thread activity when tags exist. This becomes the
  "actual" curve `tension_curve` already knows how to plot — but now on **any**
  manuscript, no tagging required.
- **Scene/sequel rhythm.** Classify each scene as a **scene** (goal → conflict →
  disaster) or a **sequel** (reaction → dilemma → decision) from prose signals +
  the goal/conflict/disaster fields `planning::parse` already extracts; report the
  alternation and flag arrhythmia — an all-scene stretch reads breathless, an
  all-sequel stretch sags. The Swain/Bickham axis nobody has.
- **Genre-aware shape.** Suggest the framework from `cfg.genre`; add
  **kishōtenketsu** (the four-part, conflict-optional structure the current five
  frameworks can't express).

## Pillar 2 — AUDIENCE (the reader's read-through)

Read the book **forward, as a first reader**.

- **The stateful walk (deterministic).** Carry, chapter by chapter: the set of
  introduced entities (from SENTINEL `introduce` — first scene vs first mention),
  the open threads (from `tension.rs`), the live stakes and arcs. From that state
  alone, zero-AI, derive the structural reader findings: confusion, info dump,
  attention dip, unpaid setup, put-down risk.
- **The synthetic first-read (LLM, explicit, cost-capped).** A forward pass where,
  for each chapter, the model is given the *running state* + the chapter prose and
  asked to react **as a first-time reader who does not know the ending** — is it
  clear who's who, are the stakes legible, is it engaging, would you turn the page?
  The prompt is forward-only by construction (it never sees later chapters).
  Grounded via `inner_grounding` + a reader-persona stance; run as a background
  job, cost previewed against the daily cap. Never automatic.

## Pillar 3 — The read-through report (the synthesis)

One artifact, the shape of an editorial letter's opening:

- **`inkhaven readthrough`** — the CLI report: the realized-vs-intended shape curve
  (sparkline), the per-chapter reader beats, the ranked findings, `--json`, and
  `--deep` to include the LLM synthetic read.
- **The story-map dashboard** — a scrollable modal (the SENTINEL ledger pattern):
  the curve, act balance, beat placement, and the reader findings, **jump-to-chapter
  on Enter**, with a key to run the deep synthetic read on demand.
- **The review pass** — the deterministic structural + reader findings join
  `Ctrl+B Shift+C` (a `lector` Output category) and `inkhaven check`.
- **`ink.readthrough.*` Bund words** — read the report from a script or hook.

---

## Multilingual

The deterministic intensity signals key off the project language exactly as the
existing detectors do (dialogue conventions across EN/RU/DE/FR/ES; conflict/stakes
lexicons per language, skipping cleanly where none ships; sentence rhythm is
language-agnostic). The SENTINEL `introduce` reuse is Unicode-safe everywhere. The
synthetic first-read runs in the manuscript's language via the grounding + persona
plumbing. Every finding carries its source, so coverage is honest per signal.

---

## Principles

- **Advisory, forward-only, honest.** LECTOR reads; it never rewrites (that's the
  Editorial Pass). The reader walk never peeks ahead — a reader problem is only real
  if a first reader would hit it *there*.
- **Deterministic-first.** The whole SHAPE half and the structural reader findings
  are zero-AI and free; the synthetic first-read is the one LLM pass, explicit and
  cost-capped.
- **Unify, don't duplicate.** Reuse the Planning Board's curve, SENTINEL's
  `introduce`, `tension.rs`, the arcs, the dialogue/rhythm metrics, the grounding
  prefix, the review-pass rails, the ledger-modal + slow-pass-as-bg-job patterns.
- **No new runtime crates; warning-free; the 1.2.15 bar.**

## What LECTOR is *not*

- Not a rewriter — it diagnoses the read; the Editorial Pass (`Ctrl+V Shift+R`)
  owns fixes.
- Not the Planning Board — it *measures* the shape the Board *declares*.
- Not a per-paragraph reader — it is whole-book, forward, once. The inner readers
  stay exactly what they are.
- Not an oracle of taste — it reports legible, grounded signals (this stretch is
  low-intensity; this entity is used before it's introduced), not verdicts on
  quality.

---

## Phases

The grounded, file-by-file plan is in [`LECTOR-1_IMPL.md`](LECTOR-1_IMPL.md):
**LR-P0** the read-through substrate (the stateful forward walk + `ChapterRead`) →
**P1** prose-measured intensity (SHAPE core) → **P2** scene/sequel → **P3** the
deterministic reader-state findings (AUDIENCE core) → **P4** the LLM synthetic
first-read → **P5** the read-through report + dashboard → **P6** the review-pass
rails → **P7** genre-aware frameworks + kishōtenketsu → **P8** Bund + config +
docs → **P9** capstone. Value core = **P1 + P3 + P4 + P5** (measure the shape,
read forward deterministically, add the synthetic read, and unify them into the
report). This is a larger arc than SENTINEL — it folds two flagships — so the two
halves ship value independently: P1–P2 alone complete the Planning Board; P3–P4
alone deliver the synthetic beta read.
