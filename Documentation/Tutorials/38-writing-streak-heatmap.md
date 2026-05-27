# 38 — Writing-streak heatmap

`Ctrl+B Shift+G` opens a GitHub-style heatmap of the
last 91 days of project-wide word activity.  It
answers two questions at a glance: *am I writing
every day?* and *when did I last have a real
session?*

```
┌─ Writing streak — last 91 days ───────────────────┐
│                                                   │
│              Mar             Apr             May  │
│     M ░ ▒ ▓ █ ░ . ░ . ▒ ░ . ▓ ░ █ ▒                │
│     T ▒ ▓ ░ ░ . . ▒ . ░ ▒ . ▒ ▓ ▒ ▒                │
│     W . ▓ ▒ ░ . . . . ░ ▓ ▒ ▓ █ ▓ ▓                │
│     T █ . . . . . . . . . . ▒ ▓ ▒ ▓                │
│     F ▒ ▒ ░ ░ . . . . . . ░ ▒ ░ ▓ ▒                │
│     S . . . . ░ . . . . . . . . . ▒                │
│     S . . . . . . . . . . . . . . .                │
│                                                   │
│  current streak: 8 days · longest: 14 (91-day)    │
│  total: 19,432 words · 4.1 days/week average      │
│  ▒ ▓ █ darker = bigger session  · today on right  │
│                                                   │
│             any key closes                        │
└───────────────────────────────────────────────────┘
```

## What each cell means

One cell per calendar day, painted by daily word
delta:

| Cell glyph | Range            | Read as                  |
|------------|------------------|--------------------------|
| `·`        | 0 words          | no writing recorded      |
| `░`        | 1–249            | light session            |
| `▒`        | 250–499          | steady session           |
| `▓`        | 500–999          | productive session       |
| `█`        | 1000+            | heavy session            |

Buckets bracket common writing-session sizes — a
paragraph is roughly 250 words, a scene 500, a
chapter 1500.

Today's cell sits in the bottom-right and is
highlighted with a dark background even when its
delta is zero, so you can locate "right now" on the
grid.

## Streak math

Two numbers in the footer:

- **Current streak** — consecutive days ending at
  today (or yesterday, if today is still empty —
  the modal grants today a grace period).
- **Longest in window** — the longest run of
  consecutive >0 days inside the 91-day window.

Both reset to zero when a day records no writing
event.  The streak is intentionally project-wide —
writing in any paragraph in the project counts.
Project hopping doesn't break it.

## Where the data comes from

Daily totals come from the project's `progress.db`
(DuckDB), which records a `writing_events` row on
every paragraph save plus a daily baseline snapshot
on first save of each day.  See
[`17-writing-goals.md`](17-writing-goals.md) for the
event-model details.  No additional config is
required for the heatmap to work — the same data the
status-bar progress widget reads.

If the progress store hasn't been written to yet
(brand-new project), the modal won't open and the
status bar shows *"streak: no progress data yet —
write a paragraph then save"*.

## Closing

Any key dismisses the modal.  The heatmap is a
read-only viewer — nothing to interact with inside.

## Use cases

- **End-of-week check-in**.  Open Friday afternoon
  to see whether the week was a five-day streak or a
  three-day burst.
- **Setting a daily target**.  If the histogram is
  full of `░` (low) cells, dial down the daily-words
  goal until streaks become attainable, then ratchet
  up.
- **Spotting a slipping habit**.  A trailing run of
  `·` cells under today is a signal — open the modal
  on Monday morning and see whether the weekend went
  dark.

## See also

- [`17-writing-goals.md`](17-writing-goals.md) —
  status-bar progress widget + the `progress.db`
  event model.
