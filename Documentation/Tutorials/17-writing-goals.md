# 17 — Writing goals + progress tracking

Inkhaven 1.2.3 adds a self-contained writing-progress subsystem:
an append-only event log of every save / status promotion, plus
configurable goals (daily words, streak grace, per-book targets
with deadlines, status-ladder promotion targets), surfaced through
a status-bar widget and a `Ctrl+V` `G` overview modal.

## What gets tracked

Every save records:

- `node_id` (which paragraph)
- `book_id` (which user book it belongs to — system books are
  excluded everywhere)
- `word_delta` = `count_words(new) − count_words(prev)`
- `total_words` snapshot at the time of save
- timestamp

Every status promotion records:

- the `from` and `to` status names
- `total_words` at the moment of promotion
- timestamp

Storage lives in `<project>/progress.db` — a self-contained
DuckDB file that reuses the same connection-pool primitive the
main store does. Drop it any time to reset history; the editor
will recreate it on next launch (today's baseline is captured
fresh).

## Word counting rules

The progress counter is Typst-aware:

- Whitespace-separated runs of non-whitespace are words.
- Heading markers (`=`+) are dropped — the heading **text** still
  counts.
- `#fn(...)` directives don't count (structure, not content).
- Lines starting with `//` (Typst comments) are skipped.

This is close to what Microsoft Word reports for plain prose.
Drift is small unless your paragraph is mostly markup.

## Configuration

`inkhaven.hjson` grows a `goals:` stanza:

```hjson
goals: {
  // Project-wide daily target. Status-bar shows
  //   today N/M words
  // when set. 0 disables the slash.
  daily_words: 1500

  // Missed days forgiven per rolling 7-day window before
  // the streak breaks. 0 = strict, 1 = one rest day allowed.
  streak_grace_per_week: 1

  // Per-book targets, keyed by book SLUG (case-insensitive).
  // target_words: 0 hides the per-book pace line.
  // deadline:    ISO date YYYY-MM-DD, empty disables pacing.
  books: {
    story: {
      target_words: 80000
      deadline:     "2026-12-31"
    }
  }

  // Trailing-7-days promotion targets keyed by status name
  // (lowercased). Modal shows `→ ready: N/M this week`.
  status_ladder: {
    ready: 1
    final: 3
    third: 5
  }
}
```

All fields are optional. Leaving the whole stanza out / empty /
zero disables that particular goal but still records events so
the modal has history when you fill them in later.

## "Today's words" — diff vs morning baseline

`today_words` is computed as **current total − today's baseline**,
where the baseline is captured on project open the first time
that UTC day. This means:

- Re-saving the same paragraph without changes adds zero.
- Deleting content drops `today_words` (it can go negative —
  surfaces as `today -200w` in the status bar).
- Closing and reopening the project mid-day doesn't reset the
  count.

Compared to "sum of every positive word delta", this is more
honest about what you actually wrote.

## Streak with grace

The streak is the trailing run of "writing days" (≥1 save with
`word_delta > 0`) ending today, allowing `streak_grace_per_week`
skipped days inside a rolling 7-day window.

| Setting | Example pattern (5d ago → today) | Streak |
| ------- | -------------------------------- | ------ |
| `0` | `W W W W W` | 5 |
| `0` | `W W _ W W` (one skip) | 2 |
| `1` | `W W _ W W` | 5 (skip forgiven) |
| `1` | `W _ _ W W` (two skips inside window) | 2 |
| `2` | `W _ _ W W W` | 6 |

The modal shows the streak's current grace usage so you know how
close you are to breaking.

## Status-bar widget

The right edge of the status bar shows a one-line summary,
refreshed on every save:

```
today 1,247/1500w · 45m · streak 3d · story 12,300/80,000w (pace 165w/d)
```

Components:

- `today X/Yw` — today's net words against `daily_words`. If no
  goal is set, just `today Xw`.
- `Nm` / `Hh Mm` — **active writing time** today (1.2.4+). See
  next section for the heuristic.
- `streak Nd` — only shown when N > 0.
- Per-book pace line — only shown when the open paragraph belongs
  to a book that has both `target_words` and `deadline` set.
  Format: `<book> <current>/<target>w (pace <required>w/d)`.

The widget is dimmed by default — it doesn't compete with the
status message on the left.

## Active writing time (1.2.4+)

Inkhaven tracks "time at the keyboard" without watching every
keystroke. The heuristic is dead simple:

- On every save, look at the gap to the previous save in the
  current day.
- Cap each gap at **5 minutes**. Anything longer is assumed to
  be AFK time (lunch, meetings, doom-scrolling…).
- Sum the (capped) gaps over the window — today, week, all-time.

What this means in practice:

- A 90-second pause between saves contributes 90 s of active
  time.
- A 4-minute pause contributes 4 min.
- A 2-hour pause (you left for lunch and came back) contributes
  5 min — the cap.
- The very first save of a day contributes 0 (no prior gap to
  measure against). A single isolated save = 0 active time.

The status-bar widget shows today's total; the Ctrl+V G modal
shows both today + this week:

```
 Today
   words: 1,247/1500 (83%)
   streak: 3d (grace 0/1 per week)
   active: 45m today · 4h 12m this week
```

This is intentionally **not** a "session timer" — there's no
START / STOP. Open inkhaven, write, save, repeat: the active
counter accumulates honestly. Leave for lunch and come back:
you lose a single 5-minute cap per absence, not the whole gap.

Active time isn't tied to a specific paragraph or book — it's
project-wide. Per-book / per-paragraph time tracking would
need keystroke buffering and is out of scope for the v1
heuristic.

## The progress modal: Ctrl+V G

Press `Ctrl+V` then `G` for the full overview. Layout:

```
┌── Writing progress ──────────────────────────────────────┐
│  Today                            ┌── 30d words/day ───┐ │
│    words: 1,247/1500 (83%)        │ ▁▂▃▅▆▇█▇▅▃▂▁    ▁▂│ │
│    streak: 3d (grace 0/1 per wk)  │                    │ │
│    active: 45m today · 4h 12m wk  │                    │ │
│                                   │                    │ │
│  Books                            │                    │ │
│    Story: 12,300w · target        │                    │ │
│       80,000w · pace 165w/d ·     │                    │ │
│       42 day(s)                   │                    │ │
│      today: 1,247w                │                    │ │
│                                   │                    │ │
│  Status ladder · last 7 days      │                    │ │
│    → ready: 0/1 this week         │                    │ │
│    → final: 2/3 this week         │                    │ │
│    → third: 5/5 this week         │                    │ │
│                                   └────────────────────┘ │
│ ↑↓ / PgUp/PgDn scroll · r refresh · Esc close            │
└──────────────────────────────────────────────────────────┘
```

The sparkline on the right is a 30-day daily-words chart
(`ratatui::Sparkline`). Days with no baseline (project wasn't
open) render as 0.

**`r`** refreshes the cache (re-walks the hierarchy + re-queries
the store). Useful if a long-running session has drifted.

## Per-book pace forecasting

When a book has both `target_words` and `deadline`, the modal +
status bar show the **required daily pace** to hit the target:

```
required_pace = ceil((target_words − current_total) / days_to_deadline)
```

Past-due deadlines (negative days remaining) collapse to "the
remaining gap, all at once" — pacing is moot at that point.
The pace number is honest about overshoot: if you're already at
or above target, it disappears.

## What doesn't count

- **System books** — Help, Scripts, Typst, Prompts, Places,
  Characters, Notes, Artefacts, Research. Editing them doesn't
  bump your daily count or streak. The book's word total
  doesn't feed `total_words`.
- **Empty saves** — if `count_words(body)` doesn't change, the
  event records `word_delta = 0` and the day isn't credited
  toward the streak.
- **Read-only Help paragraphs** — never get a `save` event in
  the first place (the editor blocks the write).

## Resetting history

The store is a single DuckDB file. To wipe:

```sh
rm <project>/progress.db
# Next launch recreates an empty store + new baseline for today.
```

Per-book and per-day deletion are not exposed through the CLI in
v1 — `duckdb` directly on `progress.db` if you really need it.

## Per-paragraph goals (1.2.4+)

Beyond project-wide + per-book targets, 1.2.4 adds a goal on
**individual paragraphs**.

Set it with `Ctrl+V T` while the paragraph is open:

```
┌── Per-paragraph goal — Ctrl+V T ─────────────────────────┐
│                                                          │
│  Paragraph word-count target:                            │
│   › 500                                                  │
│                                                          │
│   Enter sets · empty/0 clears · Esc cancels              │
└──────────────────────────────────────────────────────────┘
```

Two visual cues land when a goal is set:

**Tree pane — a compact "pip" glyph after the title.** Long
auto-derived paragraph titles can already crowd the tree pane,
so the pip is a single character whose shape tracks progress:

| Glyph | Progress | Colour      |
| ----- | -------- | ----------- |
| `○`   | 0%–24%   | red (dim)   |
| `◔`   | 25%–49%  | light red   |
| `◑`   | 50%–74%  | yellow      |
| `◕`   | 75%–99%  | light green |
| `●`   | ≥100%    | green bold  |

```
¶ N The morning  ◑
¶ F Lightning    ●
¶ R Storm at sea ●
```

**Editor pane — full gauge on the bottom border** for whichever
paragraph is open:

```
┌── Editor — The morning · [F] · L1 C0 · 300w · 1m 12s · edited 5m ago ─┐
│ = The morning                                                          │
│                                                                        │
│ The first light came through the eastern shutters…                     │
│ …                                                                      │
│ [██▒░] 60%  300/500 words  · goal: story/01-arrival/morning            │
└────────────────────────────────────────────────────────────────────────┘
```

Colour buckets: red <25%, light-red <50%, yellow <75%,
light-green <100%, green-bold ≥100%. The trailing slug-path
confirms which paragraph the gauge belongs to.

### Auto-promote on goal hit

When a save crosses a paragraph's target, inkhaven advances its
status one rung on the ladder (Napkin → First → Second → Third
→ Final → Ready). The promotion is **idempotent per
`(paragraph, status)`**: once promoted, repeated saves at the
same status won't re-fire. A manual `Ctrl+B R` cycle resets the
bookkeeping — cycle backwards then save again above target and
the auto-promote fires from the new (lower) status.

Disable in `inkhaven.hjson` if you'd rather promote manually:

```hjson
goals: {
  auto_promote_on_target: false
}
```

### Bund

Two new words for the scripting surface:

| Word                       | Stack                       | Category     |
| -------------------------- | --------------------------- | ------------ |
| `ink.paragraph.set_target` | `( path target -- )`        | `store_write`|
| `ink.paragraph.target`     | `( path -- int \| NODATA )` | `store_read` |

`target ≤ 0` clears the goal. `path` is the slug-path the rest
of the `ink.*` API uses.

```bund
"story/01-arrival/scene-one" 750 ink.paragraph.set_target
"story/01-arrival/scene-one"     ink.paragraph.target println
```

## See also

- [`14-document-status.md`](14-document-status.md) — the status
  ladder (Napkin → … → Ready) the `status_ladder:` goal counts
  ride on.
- [`../CONFIGURATION.md`](../CONFIGURATION.md) — every HJSON
  field including the `goals:` stanza.
