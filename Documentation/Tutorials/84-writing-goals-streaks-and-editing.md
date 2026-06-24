# Tutorial 84 — Writing goals: streaks, terminal report, and in-app editing

*Inkhaven 1.3.35–1.3.37*

Tutorial 17 introduced the writing-progress subsystem — the
`goals:` stanza, the status-bar widget, and the `Ctrl+V` `G`
overview modal. This tutorial does **not** repeat that; read
[`17-writing-goals.md`](17-writing-goals.md) first. Here we
cover only what 1.3.35–1.3.37 added on top of it:

- a read-only `inkhaven goals` terminal report,
- a **lifetime best** streak plus milestone hooks,
- an **in-app goals editor** (press `e`) that writes back to
  `inkhaven.hjson` without clobbering your comments,
- a new `day_boundary:` field that decides when the writing day
  rolls over.

## The `goals:` stanza, revisited

Tutorial 17 already documents `daily_words`,
`active_minutes_daily`, `streak_grace_per_week`, per-book
`books.<slug>.{target_words, deadline}`, and `status_ladder`.
1.3.37 adds one field — `day_boundary`:

```hjson
goals: {
  daily_words:           1500
  active_minutes_daily:  60

  // Missed days forgiven per rolling 7-day window before the
  // streak breaks. 0 = strict, 1 = one rest day allowed.
  streak_grace_per_week: 1

  books: {
    story: {
      target_words: 80000
      deadline:     "2026-12-31"
    }
  }

  status_ladder: {
    ready: 1
    final: 3
    third: 5
  }

  // NEW in 1.3.37 — when the writing day rolls over.
  //   utc   (default) — resets at 00:00 UTC.
  //   local           — resets at the writer's local midnight.
  // Governs the streak, daily word totals, AI-usage tallies,
  // and cost caps together — they all share one boundary.
  day_boundary: local
}
```

### Why `day_boundary` matters

The "today" everything is measured against — the streak, the
daily word total, AI-usage tallies, and cost caps — has to agree
on when one day ends and the next begins.

- `utc` (default) resets at `00:00 UTC`. Stable and machine-
  agnostic, but if you write in the evening far from UTC, that
  session can land on "tomorrow".
- `local` resets at the writer's local midnight, so an evening
  session is attributed to the day you experienced it.

Switch to `local` if your streak ever "breaks" after a late-
night session that you'd swear was the same day. The cost
dashboard (Tutorial 82) reads the *same* `day_boundary`, so the
daily cost cap rolls over in lockstep with the word count.

## `inkhaven goals` — the terminal report (1.3.35)

`inkhaven goals` is the terminal counterpart to the `Ctrl+V` `G`
progress modal. It is **read-only** — it prints and exits,
touching nothing. Run it from a project directory:

```
$ inkhaven goals
Writing goals — story-project

Today        1,247 / 1,500w  [██████████████░░░░] 83%
             active 45m  (goal 60m)
Streak       3d  · best 11d
             grace 0/1 used this week

Project      48,920w total

Books
  Story      12,300 / 80,000w   pace 165w/d · 42 day(s) left
  Essays      3,140w            (no deadline set)

Status ladder · last 7 days
  → ready    0/1
  → final    2/3
  → third    5/5  ✓

30d  ▁▂▃▅▆▇█▇▅▃▂▁▁▂▃▄▅▆▅▄▃▂▁▂▃▄▅▆▇█

Read-only — edit with `e` in Ctrl+V g, or in inkhaven.hjson.
```

What it shows, in order:

- **Project + per-book word totals.**
- **Today vs `goals.daily_words`** with a progress bar.
- **Active time** today (and the `active_minutes_daily` goal),
  plus this week in the modal.
- **Current streak AND lifetime best** (`3d · best 11d`).
- **Per-book required pace + days-to-deadline** for books that
  have both `target_words` and `deadline`.
- **This week's status-ladder promotions** against the targets.
- A **30-day sparkline** of daily words.

Nothing here is interactive; for editing, use the in-app editor
below or open `inkhaven.hjson` directly.

## Lifetime best streak + milestones (1.3.35)

The streak now knows its **all-time best** — the longest run in
the full history, computed with the same `streak_grace_per_week`
rule that drives the current streak. It shows as `· best Nd`
next to the current streak in both the `inkhaven goals` report
and the `Ctrl+V` `G` modal:

```
streak: 3d · best 11d (grace 0/1 per wk)
```

### The milestone hook

When the streak crosses a milestone **upward**, Inkhaven fires a
Bund hook once:

```bund
hook.on_streak_milestone ( milestone_days -- )
```

Details that matter:

- Milestones are **7 / 30 / 100 / 365** days.
- The hook fires once when the streak first crosses a milestone,
  passing the **highest newly reached** milestone — if a single
  catch-up jumps you past two at once, you get the higher one.
- Reopening the project when you're *already* past a milestone
  does **not** re-fire it. The hook is edge-triggered on the
  upward crossing, not level-triggered on "currently above".
- It is **informative, never blocking** — a no-op
  `hook.on_streak_milestone` is fine; define it only if you want
  the notification.

```bund
: hook.on_streak_milestone ( milestone_days -- )
  "🔥 streak milestone: " swap tostr cat " days!" cat notify
;
```

## In-app goals editor — press `e` (1.3.35, extended 1.3.36)

You no longer have to leave the editor to retune your goals.
Press **`e`** from either of two places:

- inside the `Ctrl+V` `G` progress modal, **or**
- inside the `Ctrl+B` `Shift+G` writing-streak heatmap.

(1.3.35 shipped the editor; 1.3.36 fixed it and added the
heatmap entry point.)

A small inline form appears over the three editable fields:

```
┌── Edit goals ────────────────────────────────────┐
│                                                   │
│   daily_words            › 1500                   │
│   active_minutes_daily     60                     │
│   streak_grace_per_week    1                      │
│                                                   │
│   ↑↓ field · digits edit · Enter save · Esc cancel│
└───────────────────────────────────────────────────┘
```

- `↑` / `↓` move between the three fields.
- Type digits to edit the highlighted field.
- `Enter` saves; `Esc` cancels and discards.

Only three keys are editable here: `daily_words`,
`active_minutes_daily`, and `streak_grace_per_week`. Per-book
targets, `status_ladder`, and `day_boundary` are still edited in
`inkhaven.hjson`.

### How the save protects your config

The commit does **not** rewrite the whole file. It performs a
comment-preserving **surgical splice**:

1. A versioned backup of `inkhaven.hjson` lands in
   `.config-backups/` first.
2. Only the changed keys are spliced in place. Every comment,
   every unrelated stanza, and all your formatting survive
   **byte-for-byte**.
3. The change applies **live** — no restart. The status-bar
   widget, the modal, and the streak math pick up the new values
   immediately.

So you can keep heavily commented `goals:` and `theme:` blocks
and trust that pressing `e` won't strip a single line of them.
If anything ever looks wrong, the pre-edit copy is sitting in
`.config-backups/`.

## See also

- [`17-writing-goals.md`](17-writing-goals.md) — the original
  goals tutorial: the full `goals:` stanza, word-counting rules,
  the streak-with-grace mechanics, the status-bar widget, active
  time, and per-paragraph goals. Start there.
- [`82-cost-dashboard.md`](82-cost-dashboard.md) — the AI cost
  dashboard, which shares the same `day_boundary` for its daily
  cost caps and usage tallies.
