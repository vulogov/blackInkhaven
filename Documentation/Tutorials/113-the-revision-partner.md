# Tutorial 113 — The Revision Partner

*Inkhaven 2.4 (REDLINE)*

Every reader Inkhaven has given you **diagnoses** — SENTINEL finds the continuity
break, LECTOR the saggy act, the Inner Editor the telling-not-showing, CHORUS the
two voices that read alike. Diagnosis is where most tools stop. REDLINE is the pass
that helps you **act**: it turns every finding into an author-confirmed change, with
the right kind of help for the kind of problem — and it never edits your prose
without a confirmed diff and a snapshot first.

## The editorial letter

Start with the overview a writer opens a revision with:

```sh
inkhaven revise
```

REDLINE gathers **every reader's findings into one worklist** and synthesises them
into a developmental letter — the big picture first (what a reader feels first),
then grouped by theme (continuity, structure & pacing, voice & character, line &
prose), each with a brief *why* and *what to do*. It advises; it never rewrites.

```sh
inkhaven revise --book-name X   # restrict to one book
inkhaven revise --json          # the findings as data (for tooling)
```

## Three kinds of help

Open the same worklist in the editor with **`Ctrl+V Shift+R`**. Each row shows a
**response glyph** — how that finding can be acted on:

| Glyph | Kind | What pressing `f` does |
| :---: | ---- | ---------------------- |
| `✎` | **Rewrite** | streams a diff-reviewed local prose fix — de-echo, tighten pacing, show-don't-tell, de-filter, period-fit an anachronism, or apply an Inner-Editor craft note |
| `⇄` | **Decision** | asks you *what's true / how to resolve* (the AI can't know which fact is right), then reconciles the paragraph to your decision as a confirmed rewrite |
| `✉` | **Brief** | writes a concrete developmental brief to the Thoughts pane for a structural problem a single paragraph can't solve — advice, never a rewrite |

```
⚠ ✎ echo         ch. 3   "about" repeats five times in two sentences
✗ ⇄ co_location  ch. 3   Mara is in the tower and the courtyard at once
· ✉ shape_sag    ch. 5   the shape wants a rise here; the prose reads flat
⚠ ✎ editor       ch. 7   the verb tense wobbles mid-paragraph
```

The **Inner Editor's** own observation is the marquee rewrite: its craft note is
handed to the model as the instruction, so the fix addresses *that note* — not a
generic recipe.

## Acting on a finding

`↑↓` to move, `[`/`]` to filter by category, **`Enter`** to jump to the paragraph,
**`f`** to act:

- **`✎`** streams the rewrite into the AI pane, then pops the **diff review** —
  press `a` (or `e` / `Enter`) to accept, `r` to reject. On accept, your old prose
  is **snapshotted first** (recover it any time with **F6**).
- **`⇄`** opens a one-line prompt: *what's true, or how should this resolve?* You
  type the resolution; REDLINE reconciles the anchored paragraph to it as a
  confirmed rewrite through the same diff.
- **`✉`** writes the brief to the Thoughts pane, in your manuscript's language.

**`F`** walks every `✎` Rewrite in the current filter through that same review
(`Esc` in the diff stops it). Decisions, Briefs, and finding-aware editor notes are
reviewed one at a time, never batched.

`s` skips a finding for the session; `d` defers it (persisted — hidden until the
prose changes); `D` clears all deferrals; `Esc` closes.

## The one safety rule

Every prose change REDLINE makes — a single `✎`, a `⇄` reconcile, an editor note, a
step in the `F` batch — goes through **one path**: the model's rewrite → an AI diff
you accept or reject → a labelled snapshot taken *before* the replace. There is no
unconfirmed prose-write path anywhere in REDLINE. The batch is Rewrite-only by
construction: a Decision or Brief can never slip into it.

## From a script

```
ink.revise.findings  ( -- list )  { category, severity, response, location, message, source }
ink.revise.check     ( -- dict )  { findings, high, med, low, clean, by_response, by_category }
```

Read-only — the AI editorial letter and every prose rewrite are not exposed to
Bund. `clean` is `true` when there are no high-severity findings — a simple gate for
a revision-readiness script.

---

REDLINE doesn't replace your judgement; it makes acting on your readers' findings as
fast and safe as reading them. The full reference is [`REDLINE.md`](../REDLINE.md).
