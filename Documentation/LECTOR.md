# The Read-Through (LECTOR)

*(2.3, RFC LECTOR-1 — see [`PROPOSALS/LECTOR-1_PLAN.md`](PROPOSALS/LECTOR-1_PLAN.md)
and [`PROPOSALS/LECTOR-1_IMPL.md`](PROPOSALS/LECTOR-1_IMPL.md))*

Every intelligence Inkhaven has built reads *small* — a paragraph (the inner
readers), a break (SENTINEL), the voice (CHORUS). Nothing reads the manuscript the
way the one reader who matters most does: the **first reader**, forward, once,
whole, not knowing the ending. LECTOR is that reader.

> **No feature reads your book end to end. LECTOR does — reporting both the *shape*
> of the read (structure & pacing, measured from the prose) and the *experience*
> of the read (clarity, attention, stakes, payoff).**

It is **advisory** (it reports; the Editorial Pass owns fixes), **deterministic and
free** at the core, and its one LLM pass — the synthetic first-read — is
**explicit and cost-capped**, never automatic.

---

## Shape — the structural read-through

LECTOR measures a chapter's dramatic intensity **from the prose itself** (dialogue
density, a per-language stakes/conflict lexicon, sentence-rhythm acceleration, a
summary penalty, a chapter-ending turn) — so the realized curve works on any
manuscript with **no tagging**. It compares that against the framework's *intended*
curve and flags where the shape wants a rise but the prose reads flat:

```
Read-through — 12 chapter(s) · Hero's Journey
  measured   ▂▃▄▂▁▁▃▅▄▆█▃
  expected   ▁▂▃▄▅▅▆▇▇██▂
⚠ [shape_sag] the Hero's Journey shape wants rising tension around ch. 5 (~55%)
              but the prose reads flat (~12%).
```

It also classifies each chapter on the **scene ⇄ sequel** axis (a scene is
goal→conflict→disaster, a sequel is reaction→dilemma→decision) and flags
*arrhythmia* — a run of all-scene reads breathless, a run of all-sequel sags.

The framework is `lector.framework` when set, else **suggested from your `genre`**
(fantasy → Hero's Journey, thriller → Save the Cat, mystery → Seven-Point,
slice-of-life → Kishōtenketsu, …), else Three-Act — the six built-in frameworks
including the four-movement, conflict-optional **Kishōtenketsu**.

---

## Audience — the reader's read-through

LECTOR walks the book **forward**, carrying the reader's accumulating state — which
entities they've met, which threads are open, how the energy is running — and
derives the reader-experience problems a first reader hits, **zero-AI**:

| Finding | What it catches |
| ------- | --------------- |
| `confusion` | an entity used before it's introduced ("who is this again?") |
| `info_dump` | too many new names to meet in one chapter |
| `attention_dip` | a flat, eventless chapter where attention drifts |
| `put_down_risk` | a run of flat chapters — a likely put-down point |
| `unpaid_setup` | a setup raised but never paid off |

The discipline is **forward-only**: every finding uses only the chapters read *so
far*, so a later payoff never cancels an earlier dip — that's what makes it a
reader rather than an analyst.

### The synthetic first-read

The one thing the deterministic walk can't judge — real confusion, illegible
stakes, flagging engagement — an LLM can. The synthetic first-read reacts to each
chapter **as a first reader who does not know the ending** (forward-only by
construction: each call sees only a recap of what's been read plus the current
chapter). It is **explicit and cost-capped**:

- `inkhaven readthrough --deep [--max-cost 8000] [--force]`
- `k` in the read-through dashboard.

Findings arrive tagged `source: reader`; the cost is previewed against your daily
cap before each chapter (cost *informs*, it never blocks).

---

## The command line

```
inkhaven readthrough [--deep [--max-cost 8000] [--force]] [--json]
```

Prints the measured-vs-expected shape curve, the per-chapter scene/sequel beat, and
the ranked reader findings; `--deep` folds in the synthetic first-read; `--json`
for tooling.

## In the editor

- **The review pass** (`Ctrl+B Shift+C`) includes a `read-through` line — the
  deterministic findings, each anchored, in the Output pane's `readthrough` category.
- **The dashboard** (`Ctrl+B Shift+A`) — a scrollable modal of the curve + beats +
  findings. `↑↓` scroll, **Enter** jumps to the chapter, **`k`** runs the synthetic
  first-read, Esc closes.

---

## Configuration

```hjson
lector: {
  enabled: true      // the read-through line in the review pass
  framework: null    // three_act | save_the_cat | story_circle | hero_journey |
                     // seven_point | kishotenketsu; null = suggest from `genre`
}
```

Turning `enabled` off silences the review-pass line; the standalone `inkhaven
readthrough` still runs (it's explicitly invoked).

---

## Bund

Read the deterministic read-through from a script or hook (read-only — the
synthetic first-read is not exposed):

```
ink.readthrough.report  ( -- list )  the ranked findings as dicts
                                      {kind, severity, chapter, source, message, entities}
ink.readthrough.curve   ( -- list )  per chapter {chapter, title, position, measured,
                                      expected, kind}
ink.readthrough.check   ( -- dict )  {chapters, findings, concerns, notices, info, by_kind}
```

---

## Multilingual

The intensity signals key off the project language exactly as the other detectors
do (dialogue conventions and the stakes/reflection lexicons ship for EN/RU/DE/FR/ES,
skipping cleanly elsewhere; sentence rhythm is language-agnostic). The forward walk
reuses SENTINEL's Unicode-aware mention matching, so the confusion / info-dump
findings work in every script. The synthetic first-read runs in the manuscript's
language.

---

## What it is not

- Not a rewriter — it reports the read; the Editorial Pass (`Ctrl+V Shift+R`) owns
  fixes.
- Not the Planning Board — it *measures* the shape the Board *declares*.
- Not a per-paragraph reader — it is whole-book, forward, once. The inner readers
  stay exactly what they are.
- Not an oracle of taste — it reports legible, grounded signals, not verdicts.
