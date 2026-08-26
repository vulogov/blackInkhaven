# Tutorial 114 — The Draft Chronicle

*Inkhaven 2.5 (CHRONICLE)*

Every reader Inkhaven gives you diagnoses the *current* draft. CHRONICLE remembers
what your book measured *last* draft, so you can see whether a week of revision
actually made it better — and, crucially, what it broke. It is pure measurement: it
never touches your prose.

## Mark a draft

Whenever a draft is worth remembering, stamp a milestone:

```sh
inkhaven chronicle mark "draft-1"
```

A milestone captures the whole diagnostic state at once — every reader's findings,
tallied and fingerprinted. Stamp another after you revise:

```sh
inkhaven chronicle mark "draft-2"
inkhaven chronicle list          # the milestones, newest first
```

## See whether it got better

Run `chronicle` with no arguments and it captures the live state and diffs it
against your last milestone:

```
Chronicle — since "draft-1" (2026-08-03) → now

  findings           9 →   6   ▼
  errors             2 →   0   ▼
  by category:
    echo               4 →   1   ▼
    co_location        2 →   0   ▼  cleared
    confusion          0 →   1   ▲  NEW

  ✓ 5 cleared    ▲ 1 introduced    · 5 unchanged

  introduced (new since the last mark):
    ⚠ confusion    ch. 7      an entity used before it's introduced
```

Every count is **fewer-is-better**: a ▼ is progress, a ▲ a regression. The
`✓ cleared / ▲ introduced` line is the point — it tells you exactly which findings
your revision resolved, and which new ones it created. `chronicle diff draft-1
draft-2` compares two saved milestones directly.

## Jump to what you broke

In the editor, **`Ctrl+B Shift+U`** opens the dashboard. `↑↓` scroll (`PgUp`/`PgDn`
page, `Home`/`End` to the ends; blank separators are skipped, so **Enter** always
lands on a real row), and **Enter**
on an introduced finding jumps straight to its paragraph — so the thing your last
edits broke is one keystroke away. Press **`m`** to mark the current draft without
leaving the editor (it's labelled by today's date; rename via the CLI if you like).

## Gate a submission

`chronicle check` (via Bund) is a simple pass/fail gate — clean when your latest
edits introduced no error-severity finding since the last mark:

```
ink.chronicle.check     ( -- dict )  { baseline, cleared, introduced, introduced_errors, clean }
ink.chronicle.trend     ( -- dict )  the live deltas vs the last mark
ink.chronicle.marks     ( -- list )  the milestones
```

Read-only — marking (which writes) stays a deliberate CLI/editor act.

---

CHRONICLE turns revision from a leap of faith into a measurement. The full reference
is [`CHRONICLE.md`](../CHRONICLE.md).
