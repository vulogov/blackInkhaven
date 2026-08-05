# The Draft Chronicle (CHRONICLE)

*(2.5, RFC CHRONICLE-1 — see [`PROPOSALS/CHRONICLE-1_PLAN.md`](PROPOSALS/CHRONICLE-1_PLAN.md)
and [`PROPOSALS/CHRONICLE-1_IMPL.md`](PROPOSALS/CHRONICLE-1_IMPL.md))*

Every reader Inkhaven has built *diagnoses* the current draft — SENTINEL the
continuity break, LECTOR the saggy act, CHORUS the voices that read alike, the Inner
Editor the telling-not-showing — and REDLINE (2.4) helped you *act* on all of it.
But nothing **remembered what your book measured last draft.** So the one question a
reviser most wants answered had no answer.

> **CHRONICLE snapshots the readers' collective verdict at each draft milestone and
> trends it. It measures whether revision is working — did the sag lift, did the
> voices separate, did the continuity breaks close, did confusion drop — and, most
> usefully, which findings your revision *cleared* versus which *new* ones it
> *introduced*.**

It is **pure measurement**: there is no prose-write path anywhere in CHRONICLE. It
is **deterministic and free** (it persists and diffs numbers the readers already
compute), **advisory**, **multilingual by inheritance**, and adds **no new runtime
crates**.

---

## A milestone

A milestone is an explicit capture of the current draft — the whole diagnostic
state in one shot. Stamp one whenever a draft is worth remembering:

```
inkhaven chronicle mark "draft-2"
inkhaven chronicle mark "beta-1" --ref v0.9   # record a git ref (verbatim)
inkhaven chronicle list                        # the milestones, newest first
```

A milestone captures, from a single headless run of the unified worklist (the same
`collect` the Editorial Pass and `inkhaven revise` use): the total finding count, the
tallies by severity / category / response-kind / source, and the **fingerprint of
every finding** (so the cleared/introduced diff is exact). CHRONICLE never resolves
or enumerates git refs — `--ref` is a string stored for your own bookkeeping.

---

## The trend

Run `inkhaven chronicle` (bare) and CHRONICLE captures the live state and diffs it
against your last milestone:

```
Chronicle — since "draft-1" (2026-08-03) → now

  findings           1 →   2   ▲
  warnings           0 →   1   ▲  NEW
  infos              1 →   1   ·

  by category:
    put_down_risk      0 →   1   ▲  NEW
    shape_sag          0 →   1   ▲  NEW
    attention_dip      1 →   0   ▼  cleared

  ✓ 1 cleared    ▲ 2 introduced    · 0 unchanged

  introduced (new since the last mark):
    ⚠ put_down_risk ch. 3      ch. 1–3 run flat and eventless (3 chapters) — a likely…
    · shape_sag    ch. 2      the Three-Act shape wants rising tension around ch. 2…
```

Every count is **"fewer is better"** — a fall (▼) is an improvement, a rise (▲) a
regression, and regressions sort to the top of the category list. `inkhaven
chronicle diff <from> <to>` diffs two named milestones head-to-head; `--json` on
either emits the deltas plus the three finding lists.

---

## The REDLINE hook — cleared vs introduced

The signature move. Because every finding carries a stable identity, CHRONICLE
splits the change into three sets by simple set difference:

- **cleared** — findings that were there last milestone and are gone now (your
  revision resolved them);
- **introduced** — findings new since the last milestone (your edits, or the ripple
  around them, created them);
- **unchanged** — still standing.

This is what closes REDLINE's loop: proof the work landed, and an early warning on
collateral damage *before* it ships. The introduced list is itemised — press
**Enter** on one in the dashboard to jump straight to its paragraph.

---

## In the editor

**`Ctrl+B Shift+U`** opens the CHRONICLE dashboard: the trend since your last
milestone and the cleared/introduced split.

| Key | Action |
| --- | ------ |
| `↑ ↓`  | scroll |
| `⏎`    | jump to an introduced finding's paragraph |
| `m`    | mark this draft now (labelled by today's date — rename via the CLI) |
| `Esc`  | close |

---

## From a script

```
ink.chronicle.marks  ( -- list )  { label, ts, book, findings, errors, warnings, infos }
ink.chronicle.trend  ( -- dict )  { marked, since, headline, categories, cleared, introduced, persisted }
ink.chronicle.check  ( -- dict )  { baseline, cleared, introduced, introduced_errors, clean }
```

Read-only — **marking is not exposed** (it writes a milestone; scripts read the
history, they don't stamp it). `check.clean` is `true` when your latest edits
introduced **no error-severity** finding since the last mark — a pre-submit /
pre-commit gate. With no milestone yet, `check` is vacuously clean and `trend` is
`{marked: false}`.

---

## Multilingual

Inherited. The metrics and finding messages already key off the project language;
the trend itself is language-agnostic counts, and every finding keeps its `source`.

---

## What it is not

- Not a new reader — it snapshots the readers you already have.
- Not a git tool — it never creates or lists tags; a milestone is an explicit
  `chronicle mark`, the same shape as a paragraph snapshot.
- Not an auto-capture daemon — marks are deliberate (a draft is a decision).
- Not a corrector — the introduced list *points*; the `Ctrl+V Shift+R` Editorial
  Pass is where you act.
