# The Revision Partner (REDLINE)

*(2.4, RFC REDLINE-1 — see [`PROPOSALS/REDLINE-1_PLAN.md`](PROPOSALS/REDLINE-1_PLAN.md)
and [`PROPOSALS/REDLINE-1_IMPL.md`](PROPOSALS/REDLINE-1_IMPL.md))*

Every reader Inkhaven has built **diagnoses**: SENTINEL finds the continuity break,
LECTOR finds the saggy act and the put-down point, the Inner Editor finds the
telling-not-showing, CHORUS finds the two voices that read alike. Diagnosis is
where most tools stop. REDLINE is the pass that helps you **act** — turning every
finding into an author-confirmed change, with the *right kind of help* for the
*kind of problem*.

> **REDLINE never edits your prose on its own.** Every change flows through the
> same confirmed-diff + snapshot contract the editor already uses: you see the
> exact diff, you accept or reject, and your old prose is snapshotted first
> (F6-restorable). There is no unconfirmed prose-write path anywhere in REDLINE.

---

## The unified worklist

REDLINE gathers **every reader's findings into one ranked list** — the same
`collect` the Editorial Pass and `inkhaven revise` share. Doctor's editorial
classes, Facts contradictions, semantic drift, `plan` structure, the prose-style
detectors, **SENTINEL** continuity, **LECTOR** read-through, the **Inner Editor**'s
craft notes, and **CHORUS** voice findings all land in the same queue, each tagged
with *how it can be acted on*.

---

## Three kinds of help

Each finding carries a **response kind** — the honest form of help for that
problem. Only a Rewrite ever touches prose.

| Kind | Glyph | What it does |
| ---- | :---: | ------------ |
| **Rewrite**  | ✎ | A diff-reviewed local prose fix — there's an honest single-paragraph change (de-echo, tighten pacing, show-don't-tell, de-filter, period-fit an anachronism, or an Inner-Editor craft note). |
| **Decision** | ⇄ | A guided authorial choice — the AI *can't* know which fact is right (which scene Mara is in, which value is canon). You state what's true; REDLINE reconciles the paragraph to your decision as a confirmed rewrite. |
| **Brief**    | ✉ | A concrete revision brief — for a structural or book-level problem a single paragraph can't solve (a saggy act, a likely put-down point). The AI *advises*; it never rewrites. The brief lands in the Thoughts pane. |

The **Inner Editor's** own observation is the marquee Rewrite: its craft note is
passed to the model as the instruction, so the fix addresses *that note* — not a
generic recipe.

---

## The editorial letter

Run `inkhaven revise` and REDLINE synthesises the **whole worklist into one
developmental letter** — the overview a writer opens a revision with: the big
picture first (what a reader feels first), then grouped by theme (continuity,
structure & pacing, voice & character, line & prose), most important first, each
with a brief *why* and *what to do*. It advises; it never rewrites.

```
inkhaven revise                 # the editorial letter over the whole project
inkhaven revise --book-name X   # restrict to one book (slug or title)
inkhaven revise --json          # the findings as JSON (category, severity,
                                # response, location, message) for tooling
```

---

## In the editor

Open the **Editorial Pass** with `Ctrl+V Shift+R`. Each row shows its response
glyph so you can see, at a glance, what acting on it will do:

```
⚠ ✎ echo         ch. 3   "about" repeats five times in two sentences
✗ ⇄ co_location  ch. 3   Mara is in the tower and the courtyard at once
· ✉ shape_sag    ch. 5   the Three-Act shape wants a rise here; the prose reads flat
⚠ ✎ editor       ch. 7   the verb tense wobbles mid-paragraph
```

| Key | Action |
| --- | ------ |
| `↑ ↓`     | move the cursor |
| `[ ]`     | filter by category |
| `⏎`       | jump to the paragraph |
| `f`       | **act** — ✎ opens the AI rewrite → diff; ⇄ asks your decision, then reconciles; ✉ writes a brief to the Thoughts pane |
| `F`       | fix-all — walk every ✎ Rewrite in turn, each diff-reviewed (Decisions, Briefs, and finding-aware editor notes are handled one at a time, never batched) |
| `s` / `d` | skip (this session) / defer (persist until the prose changes) |
| `Esc`     | close |

---

## The safety contract

Every REDLINE prose change — a single ✎ fix, a ⇄ decision-reconcile, an editor
note, or a step in the `F` batch — streams through **one path**:

1. the model's rewrite streams into the AI pane;
2. an **AI diff review** modal shows the exact change; you accept (`a` / `e` / `⏎`)
   or reject (`r`);
3. on accept, your pre-rewrite prose is **snapshotted** with a labelled entry
   *before* it is replaced — recover it any time with **F6**.

The batch queue is **Rewrite-only** by construction: a Decision or Brief can never
acquire a fix recipe, so it can never slip into the prose-write path. This is
locked by a guard test.

---

## Bund

Read the worklist from a script or hook (deterministic, read-only — the AI
editorial letter and every prose rewrite are not exposed to Bund):

```
ink.revise.findings  ( -- list )  the ranked findings as dicts
                                   {category, severity, response, location, message, source}
ink.revise.check     ( -- dict )  summary counts
                                   {findings, high, med, low, clean, by_response, by_category}
```

`clean` is a simple pass/fail gate — `true` when there are no high-severity
findings — for a revision-readiness script. `severity` uses the same
`high` / `med` / `low` vocabulary as `revise --json`.

---

## Multilingual

REDLINE inherits every source reader's language coverage and never claims more.
The editorial letter and every brief are written **in the manuscript's language**;
an Inner-Editor craft note is carried through in the language it was written in.
Every finding keeps its `source`, so *"does it work in Russian?"* answers itself
per detector.

---

## What it is not

- Not a new reader — it's the *actioning* layer over the readers you have.
- Not an autopilot — it never edits prose without a confirmed diff and a snapshot.
- Not a corrector for structure — a saggy act gets a brief, never a silent rewrite;
  moving the furniture stays yours.
