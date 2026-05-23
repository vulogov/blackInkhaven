#import "../design.typ": *

#chapter(number: 9, part: "Part II — The Editor",
  title: "Status and writing goals")

#dropcap("E")very paragraph carries two pieces of metadata that
make a manuscript feel like a working project rather than a
pile of text: its status on a workflow ladder and an
optional word-count target. Both are visible at a glance
in the tree pane.

#section("The status ladder")

Seven rungs, from least to most complete:

#chord_table((
  chord_row("None", "Default — unstarted or unclaimed."),
  chord_row("Napkin", "A sketch or rough outline. The bones of an idea."),
  chord_row("First", "First draft — words on the page."),
  chord_row("Second", "Second pass — structure intact, prose polished."),
  chord_row("Third", "Third pass — pacing, voice, line-level fix."),
  chord_row("Final", "Final edit — ready for proof."),
  chord_row("Ready", "Ready to submit / publish."),
))

The tree pane shows the status as a single letter (`N`, `F`,
`S`, …) coloured by rung. Books / chapters / subchapters
roll up the deepest-rung paragraph beneath them.

#section("Cycling status")

`Ctrl+B R` (Editor scope) cycles the open paragraph's status
forward through the ladder. Shift+`Ctrl+B R` cycles
backward. The tree pane updates immediately.

The same cycle works on the tree-cursor's paragraph from
tree focus (`Ctrl+B R` in tree scope).

#section("Status-filter modal")

`Ctrl+B 1` through `Ctrl+B 7` open a project-wide filter modal
keyed to one rung:

#figure_slot(
  id: "status-filter",
  caption: "Ctrl+B 4 — every paragraph at Status:Second across the project. Enter opens; n/N walk in tree order.",
  height: 50mm,
)

#chord_table((
  chord_row("Ctrl+B 1", "Filter to Napkin."),
  chord_row("Ctrl+B 2", "First."),
  chord_row("Ctrl+B 3", "Second."),
  chord_row("Ctrl+B 4", "Third."),
  chord_row("Ctrl+B 5", "Final."),
  chord_row("Ctrl+B 6", "Ready."),
  chord_row("Ctrl+B 7", "None (unstatused)."),
  chord_row("Enter", "Open the cursor paragraph in the editor."),
  chord_row("Esc", "Close."),
))

#section("Writing goals — the goals stanza")

In `inkhaven.hjson`:

```hjson
goals: {
  daily_words: 800              # baseline target
  morning_baseline: true        # measure from your first
                                 # save of the day
  streak_grace: 1               # how many "rest days" the
                                 # streak survives
  per_book_deadline: {
    "my-first-book": "2026-08-31"
  }
}
```

#figure_slot(
  id: "ctrl-v-g-progress",
  caption: "Ctrl+V G — progress modal. Today's words, current streak (with grace), per-book burn-down to deadline.",
  height: 55mm,
)

`Ctrl+V G` opens the progress modal. Per-day word counts,
streak status, deadline burn-down, and a per-book rollup.

#section("Per-paragraph targets")

A target is a word count you want the paragraph to hit:

#chord_table((
  chord_row("Ctrl+V G then T", "Set the open paragraph's target (in the progress modal)."),
  chord_row("Ctrl+V T", "Same shortcut from anywhere — opens a small number prompt."),
))

The target shows up in the tree pane as a tiny progress pip
(`○ ◔ ◑ ◕ ●`) coloured by completion (red at 0% → green at
100%+).

#section("Status hooks (Bund)")

Two Bund hooks fire on status events:

```bund
"hook.on_status_promoted" {
  // ( uuid new_status -- )
  drop drop
  // Side-effects when a paragraph is promoted up the ladder.
} register
```

```bund
"hook.on_goal_hit" {
  // ( word_count_today -- )
  drop
  // Maybe play a sound, push a notification, …
} register
```

Useful for ergonomic touches — a typewriter ding when you hit
the daily goal, an auto-tag when a paragraph promotes to
Ready, an auto-snapshot when a paragraph promotes to Final.

#recap((
  [Seven-rung status ladder: None → Napkin → First → Second → Third → Final → Ready.],
  [`Ctrl+B R` cycles forward; Shift+R cycles back.],
  [`Ctrl+B 1..7` filters the project to one rung.],
  [`goals:` stanza controls daily target + streak + deadlines.],
  [Per-paragraph word targets render as a tree-pane progress pip.],
  [`hook.on_status_promoted` and `hook.on_goal_hit` enable rituals.],
))
