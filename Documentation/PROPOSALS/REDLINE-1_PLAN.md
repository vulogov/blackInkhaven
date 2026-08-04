# REDLINE-1 — The Revision Partner: from finding to confirmed fix (RFC)

*The 2.4.0 flagship (the "REVISE" pass). Status: RFC. Nothing built.*

## Summary

Five releases have *diagnosed* your manuscript. CHORUS measures the voice, SENTINEL
watches the continuity, LECTOR reads it as a first reader, the Inner family
interrogates it paragraph by paragraph. Every one of them **reports**. None of them
helps you **act** — except the Editorial Pass, which today can only rewrite four
mechanical problems (echo, pacing, show-don't-tell, filter words). Everything a
*reader* found — a character in two places, two voices that read alike, a saggy
Act 2, a setup never paid off — is a finding you jump to and fix by hand.

REDLINE is the layer that closes the loop. Its thesis:

> **You've been told what's wrong for five releases. REDLINE helps you fix it — as a
> partner you supervise, never a rewriter you unleash. Every reader's finding
> becomes an author-confirmed change: the right kind of help for each kind of
> problem.**

The crucial design fact — and the reason this is safe by construction — is that the
**confirmed-diff, snapshot-safe contract already exists** (`start_editorial_rewrite`
→ diff review → accept-snapshots-then-replaces). The buffer is *never* mutated
except in the accept keypress, which snapshots the pre-edit prose first. REDLINE
reuses that contract verbatim; it adds **no way to edit prose unconfirmed**. It adds
only: the readers' findings brought into the queue, more scoped fixes, and two new
kinds of help for the problems a single rewrite can't honestly solve.

It is **advisory at the boundary** (the AI proposes; you decide; nothing is written
without your keypress and a snapshot), **cost-capped**, and adds **no new runtime
crates**.

## What already exists (so REDLINE extends, it does not rebuild)

This is the honest core: the machinery REDLINE stands on is already in the tree.

- **The aggregation** — `cli::editorial::collect` gathers findings from doctor
  scans, facts, drift, `plan check`, and the deterministic prose-style detectors
  into one ranked `EditorialReport` of `EditorialFinding { category, severity,
  location{paragraph, char_range, …}, message, source }`.
- **The rewrite loop** — `start_editorial_rewrite(category, span)` looks up a
  `FixSpec` (prompt + `FixScope::Paragraph|Span`), streams the LLM, and routes the
  result through the **diff-review modal**. The prose buffer is mutated *only* in
  the accept arm, which calls `snapshot_open_paragraph_with_annotation` **before**
  replacing. Batch (`F`), defer-by-fingerprint (`d`), and reversible snapshots
  (`F6`, whose restores also snapshot first) all exist.
- **The surface** — `Modal::EditorialPass` (`Ctrl+V Shift+R`): `↑↓` · `⏎` jump ·
  `f` fix · `F` fix-all · `s` skip · `d` defer.
- **The gate** — `rewritable() = location.paragraph.is_some() && fix_spec(category)
  .is_some()`, and `fix_spec` covers exactly **four** categories today. **Widening
  this — safely — is the whole flagship.**

**The genuinely empty slots:** the *judgment* readers (Inner Stylist, SENTINEL
continuity, LECTOR, Inner Editor, tension, drift) are surfaced but not actionable —
no converter brings them into the queue with their anchor, and no help exists for
the findings a single-paragraph rewrite can't honestly solve.

---

## The model

REDLINE routes every finding to one of **three kinds of help** — and only the first
touches prose, always through the existing confirmed-diff contract:

- **Rewrite** — a diff-reviewed local prose fix, where the finding has a paragraph
  (or span) locus *and* an honest single-locus fix. The four mechanical fixes today,
  plus the localizable judgment findings: an Inner Editor craft observation, a drift
  description to reconcile, a flat character-voice paragraph, a single-paragraph
  numeric contradiction. Flows through `start_editorial_rewrite` unchanged — confirm
  → snapshot → replace.
- **Decision** — a guided authorial choice, where the fix requires *you* to decide
  which way is right, and then REDLINE executes the chosen resolution as a
  diff-reviewed rewrite. *"Mara is in two places in ch. 3 and ch. 7 — which scene is
  right? → then I'll reconcile the other."* *"'The sealed letter' is never opened —
  resolve it here, or cut the setup?"* *"Aldous is named in ch. 2 but introduced in
  ch. 5 — introduce him here, or move the introduction earlier?"* The **decision is
  yours**; the rewrite is confirmed as always.
- **Brief** — a concrete revision suggestion with **no rewrite**, where the problem
  is structural or book-level and there is no honest single-paragraph fix. *"Act 2
  sags (ch. 5–7): the Hero's Journey wants the ordeal here — consider raising the
  stakes in the ch. 6 confrontation."* *"Mara and Joren read alike — the two voices
  need a distinguishing habit."* REDLINE writes a specific, actionable brief and
  leaves the writing to you. Structure is yours to move.

And the entry point that makes it a *pass*, not a pile:

- **The editorial letter** — one AI synthesis over *all* the findings: a prioritized,
  thematically-grouped developmental letter (the big picture, then continuity →
  structure → voice → line), the way a real editor opens. It organizes five releases
  of diagnosis into one revision plan.

Everything is **advisory at the boundary**: the deterministic classification is
free; the rewrite/decision LLM calls are the existing cost-capped path; the brief +
letter are explicit background passes. No prose is ever written without your keypress
and a snapshot.

---

## Pillar 1 — The unified revision queue

Bring every reader into `collect`. New `from_*` converters map each source's finding
— with its anchor into `location.paragraph` — into an `EditorialFinding`, tagged with
its **response kind** (`Rewrite` | `Decision` | `Brief`). The Editorial Pass becomes
the one place the whole manuscript's diagnosis lives, each item showing how it can be
acted on.

## Pillar 2 — Finding-aware fixes

- **Rewrite** widens `fix_spec` with new slugs + prompts for the localizable judgment
  categories (Inner Editor, drift-reconcile, character-voice, numeric) — each a
  `Prompts`-book-overridable prompt, each flowing through the untouched confirmed-diff
  contract.
- **Decision** adds a small `Modal::RevisionDecision` that presents the choice, then
  hands the chosen resolution to `start_editorial_rewrite` as a targeted prompt
  ("make this paragraph consistent with *this* fact").
- **Brief** adds a background pass (`start_bg_job` + the cost-capped `slow_llm_call`)
  that writes a grounded revision brief to the Thoughts pane — never the buffer.

## Pillar 3 — The safety contract (reused, not rebuilt)

REDLINE's whole prose-editing surface routes through the existing
`start_editorial_rewrite` → `open_ai_diff_review*` → accept-snapshots-then-replaces
path. The guarantee is *structural*: there is no code path that writes the prose
buffer except the accept keypress, and every editorial fix carries a
`post_accept_snapshot`. REDLINE adds new *slugs*, *converters*, and the *decision/
brief* affordances — **not** a new way to touch prose. Restores stay reversible
(`F6`), and every accepted change is a discoverable snapshot.

## Pillar 4 — The editorial letter

One synthesis pass over the ranked findings → a prioritized, grouped developmental
letter. It is the overview a writer opens a revision with, and the CLI's default
output (`inkhaven revise`).

---

## Multilingual

Every rewrite/decision prompt is resolved through the existing `resolve_prompt`
(book-`Prompts` → cross-language → embedded), so fixes come out in the manuscript's
language and are author-overridable. The brief + letter run in the project language.
The findings themselves inherit each reader's coverage.

## Principles

- **The AI proposes; the author disposes.** No prose is written without an explicit
  per-change keypress and a pre-edit snapshot — guaranteed by the reused diff-review
  interposition, not by convention.
- **The right help for each problem.** A rewrite where one is honest; a decision
  where you must choose; a brief where structure is yours. REDLINE never pretends to
  auto-fix a saggy middle.
- **Extend, don't duplicate.** The queue, the rewrite loop, the diff modal, the
  snapshot store, the prompt-override + cost-cap plumbing are all reused.
- **No new crates; warning-free; the 1.2.15 bar.**

## What REDLINE is *not*

- Not an auto-rewriter — every change is confirmed and reversible; nothing is applied
  in bulk without per-item review.
- Not a ghostwriter — briefs suggest, they don't write; structure stays yours.
- Not a new reader — it acts on the findings the five prior flagships already produce.
- Not a corrector of taste — it offers help, it never overrides your judgment.

---

## Phases

The grounded, file-by-file plan is in [`REDLINE-1_IMPL.md`](REDLINE-1_IMPL.md):
**RD-P0** the response-kind substrate → **P1** bring the judgment readers into the
queue → **P2** more rewrites (widen `fix_spec`) → **P3** the decision flow → **P4**
the brief flow → **P5** the editorial letter (`inkhaven revise`) → **P6** the revision-
pass surface → **P7** batch + reversibility polish → **P8** Bund + config + docs →
**P9** capstone. Value core = **P1 (the unified queue) + P2 (more rewrites) + P3
(decisions) + P5 (the letter)**; P4 (briefs) is the honest handling of what a rewrite
can't solve.
