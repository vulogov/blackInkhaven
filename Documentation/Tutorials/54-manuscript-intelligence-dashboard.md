# Tutorial 54 — Manuscript intelligence dashboard

*Inkhaven 1.2.16+*

`Ctrl+V Shift+J` opens a synthesis pane that
unifies every metric Inkhaven has been
collecting since 1.2.5 — word count, structure,
pacing, threads, comments — into one view.  J
for *Journal*.

Use it at the start of a writing session to see
where the manuscript stands, what's stalled,
what's overdue.  Use it at the end to see what
moved.

## Opening the dashboard

In any pane, press `Ctrl+V`, then `Shift+J`.

The modal appears centred over the workspace.
Snapshot is computed once at open time —
re-open for fresh numbers.

```text
 ┌ Journal — manuscript intelligence (2026-06-01 14:23 UTC) ─┐
 │                                                            │
 │  Word count                                                │
 │    today: 1247 · total: 78912 · streak: 7d                 │
 │    goal: 80000 · remaining: 1088 · target: 2026-12-31      │
 │    active: 43m today · 312m this week                      │
 │                                                            │
 │  Structure                                                 │
 │    books: 1 · chapters: 12 · paragraphs: 142               │
 │    mean chapter: 6580 words ± 1240 (CV 19%)                │
 │    pacing: steady                                          │
 │                                                            │
 │  Threads                                                   │
 │    total: 6 · active: 5 · dormant (>30d): 1                │
 │                                                            │
 │  Comments                                                  │
 │    open: 4 · resolved this week: 7 · resolved total: 23    │
 │                                                            │
 │ ↑↓ scroll · e export to journal-<ts>.md · Esc closes       │
 └────────────────────────────────────────────────────────────┘
```

## What each section means

### Word count

* **today / total** — words written today + the
  full project word count across every counted
  user book (Characters / Places / Help and
  other system books are excluded).
* **streak** — consecutive days you've moved the
  word count forward.  Same number the writing-
  progress modal (`Ctrl+V G`) and the streak
  heatmap (`Ctrl+B Shift+G`) show.
* **goal / remaining / target** — only shown
  when `project.word_count_goal` is set in
  `inkhaven.hjson`.  Remaining = goal − total.
  Target date is verbatim from
  `project.target_date`.
* **active today / week** — sum of save→save
  gaps capped at 5 min each, so AFK time
  doesn't inflate the number.  Honest "minutes
  at the keyboard" without keystroke tracking.

### Structure

* **books / chapters / paragraphs** — counts
  across user books only.
* **mean chapter ± stdev (CV %)** — the
  coefficient of variation drives the pacing
  verdict.  Three bands:
  * `cv < 20%`  → *steady*  (consistent chapter
    lengths)
  * `20 ≤ cv < 50%` → *varied*  (intentional
    range — usually a good sign)
  * `cv ≥ 50%` → *choppy*  (one chapter is
    pulling far from the mean; worth a look at
    pacing-collapse findings from `Ctrl+B
    Shift+0` doctor scan)

### Threads

Only meaningful if you use the Threads system
book (1.2.14+).

* **total** — number of thread chapters.
* **active** — newest waypoint mtime within
  the last 30 days.
* **dormant** — newest waypoint older than 30
  days (or no waypoints at all).  Same
  threshold the 1.2.14 thread doctor +
  1.2.16's `stalled-thread` doctor-scan class
  use.

### Comments

* **open** — comments where `resolved = false`.
* **resolved this week** — closed in the last
  7 days, project-wide.
* **resolved total** — lifetime count.  Lets
  you see review momentum over the cycle.

## Scrolling + export

* `↑↓` scrolls one line; `PgUp` / `PgDn` jumps
  10 lines; `Home` returns to top.
* `e` exports the snapshot to `<project>/
  journal-<UTC>.md` — atomic write via
  `crate::io_atomic`.  Status bar names the
  path on success.

The exported markdown has the same section
shape as the modal, suitable for pasting into
a weekly check-in or sharing with an editor.

```markdown
# Manuscript Journal — 2026-06-01 14:23 UTC

## Word count

- today: 1247
- total: 78912
- goal: 80000 (remaining: 1088)
- target date: 2026-12-31
- streak: 7 day(s)
- active today: 43 min
- active this week: 312 min

## Structure
...
```

## When the data is sparse

* **No progress cache yet** (very early in the
  TUI's startup): word-count section shows
  zeros.  Wait a tick + re-open.
* **No Threads book** (pre-1.2.14 project):
  Threads section reads `total: 0 · active: 0
  · dormant: 0`.
* **Goal not set in HJSON**: the goal /
  remaining / target lines are omitted (no
  "set a goal" exhortation — Inkhaven is not
  preachy about this).

## What's not yet in the dashboard (A.2.b)

The 1.2.16 cycle shipped the first wave of
sections.  Planned for 1.2.17 (A.2.b):

* **POV chip** — most-mentioned character in
  the active book (heuristic from
  `pov_tracker`).
* **Rhythm gauge** — sentence-rhythm CV across
  the project, paired with the per-paragraph
  `Ctrl+B Shift+H` modal.
* **Filter-words flagged today** — count of
  style-warning hits per the existing overlay
  toggled by `Ctrl+B Shift+F`.
* **Recent AI critique** — first ~120 chars of
  the most recent AI history entry, so the
  dashboard reminds you of the last critique
  you got.

Each adds one row to the existing pane; no new
infrastructure needed.

## See also

* `Ctrl+V G` — writing-progress modal (today /
  pace / 30-day sparkline / status ladder /
  per-book bar chart).  The dashboard is the
  synthesis; this is the specific pace view.
* `Ctrl+V Shift+G` — project word-count goal
  modal with finish-date projection.
* `Ctrl+B Shift+0` — project doctor (1.2.15+,
  extended in 1.2.16 with narrative-audit
  classes).  Pairs with the dashboard:
  dashboard for the *what*; doctor for the
  *what's broken*.
* `Documentation/RELEASE_NOTES/1.2.16.md` —
  Phase A.2 implementation log.
