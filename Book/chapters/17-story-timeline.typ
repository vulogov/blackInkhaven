#import "../design.typ": *

#chapter(number: 17, part: "Part V — The Timeline",
  title: "Story timeline")

#dropcap("T")he timeline is inkhaven's first-class story-time
metadata layer. Every scene-worthy paragraph can be tied to
a calendar moment; the swim-lane view shows the book as
horizontal tracks; AI critique audits the timeline for
consistency. This is the headline feature of the 1.2.7
cycle.

The feature is opt-in: set `timeline.enabled: true` in HJSON
to turn it on. Existing projects upgrade without surprise.

#section("Enabling the timeline")

```hjson
timeline: {
  enabled: true
  default_track: "main"

  calendar: {
    preset: "gregorian"
  }
}
```

Three calendar presets:

#chord_table((
  chord_row("`gregorian`", "Real-world dates. `Y.M.D` format."),
  chord_row("`sols`", "Single-unit days since day zero. `Sol N` format."),
  chord_row("`custom`", "Anything else. Define units + seasons + format string yourself."),
))

A custom calendar block for an Aerin Saga (12 months × 30
days, named months):

```hjson
calendar: {
  preset: "custom"
  base_unit: "day"
  units: [
    { name: "day", names: [] }
    { name: "month", per_parent: 30,
      names: ["Frostmoon", "Snowfall", "Greenstart",
              "Bloomtide", "Highsun", "Goldfall",
              "Mistwane", "Stormrise", "Coldgate",
              "Longnight", "Hearthlit", "Yearfall"] }
    { name: "year", per_parent: 12, names: [] }
  ]
  seasons: [
    { name: "winter", start_month: 1, span_months: 3 }
    { name: "spring", start_month: 4, span_months: 3 }
    { name: "summer", start_month: 7, span_months: 3 }
    { name: "autumn", start_month: 10, span_months: 3 }
  ]
  epoch_label: "A"
  epoch_before_label: "BA"
  display_format: "{year}{epoch_label}.{month}.{day}"
}
```

Empty `names` falls back to numeric. Negative ticks (prequels)
supported throughout.

#section("Parser shapes")

#chord_table((
  chord_row("`1A`", "Year — Precision::Year."),
  chord_row("`1A.3`", "Month — Precision::Month."),
  chord_row("`1A.Frost`", "Month-name prefix — also Precision::Month."),
  chord_row("`1A.spring`", "Season — Precision::Season (90-day window)."),
  chord_row("`1A.3.15`", "Day — Precision::Day."),
  chord_row("`-1BA`", "Year before epoch (prequel) — Precision::Year."),
  chord_row("`Founding`", "Alias (defined in `parse_aliases`)."),
))

Precision matters for AI critique: a season-precision event
collides differently against other season-precision events
than against a day-precision one.

#section("CLI")

```
inkhaven event add "Storm" \
  --start "1A.2.3" --end "1A.2.5" \
  --track main --book-name "Aerin Saga"

inkhaven event list --book-name "Aerin Saga" --track main

inkhaven event show aerin-saga/timeline/storm
```

#section("Where events live")

The first time you `event add` under a book, a Timeline
chapter materialises inside that book:

```
Aerin Saga/
├── Chapter 1/
├── Chapter 2/
└── Timeline/           ← lazy; system_tag: "book_timeline"
    ├── Birth of Aerin  ← event paragraph (Node.event = Some(...))
    ├── Storm
    └── Marketplace scene
```

Each event is a paragraph carrying `EventData` in its
metadata: `start_ticks`, optional `end_ticks`, `precision`,
optional `track`, lists of `characters` / `places` /
`linked_paragraphs`.

#section("Ctrl+V e — vertical picker")

#figure_slot(
  id: "timeline-event-picker",
  caption: "Ctrl+V e — chronological event picker. Track filter via `t`. Enter opens the event paragraph.",
  height: 50mm,
)

#chord_table((
  chord_row("↑ / ↓ / Home / End", "Move cursor."),
  chord_row("t / T", "Cycle track filter (None → t0 → t1 → … → None)."),
  chord_row("Enter", "Open the event paragraph in the editor."),
  chord_row("Esc", "Close."),
))

#section("Ctrl+V t — swim-lane view")

The headline UI. Opens at the current paragraph's nearest
scope (Subchapter → Chapter → Book):

#figure_slot(
  id: "timeline-swim-lanes",
  caption: "Ctrl+V t — swim-lane view. Per-track rows. ● instant; ─ duration; ◌ orphan. Axis labels along the top.",
  height: 70mm,
)

#chord_table((
  chord_row("← / →", "Scroll by ~10 cells."),
  chord_row("PgUp / PgDn", "Page by ~60 cells."),
  chord_row("+ / =", "Zoom in (0.66× ticks/cell)."),
  chord_row("- / _", "Zoom out (1.5×)."),
  chord_row("0", "Reset zoom to 1.00×."),
  chord_row("Home / End", "Jump to first / last event in the visible set."),
  chord_row("Tab", "Cycle highlighted track."),
  chord_row("Enter", "Open the event closest to the cursor."),
  chord_row("n / N", "Pop a title prompt; commit a new event at cursor tick."),
))

Zoom preserves the cursor's screen column — drilling in feels
anchored to whatever event you were inspecting.

#section("Scope navigation")

#chord_table((
  chord_row("u / U", "Up-scope (subchapter → chapter → book)."),
  chord_row("d / D", "Open the inline descent picker — immediate children with event counts."),
  chord_row("b / B", "Jump straight to book scope."),
  chord_row("p / P", "Toggle project overlay (every user book; tracks prefixed with book slug)."),
))

Event filter rule: at book scope all events show; at sub-book
scope an event appears iff itself OR any of its
`linked_paragraphs` is a descendant of the scope.

#figure_slot(
  id: "timeline-descent-picker",
  caption: "Descent picker (`d`) — immediate child scopes with their event counts. Enter descends; Esc returns.",
  height: 45mm,
)

#section("AI health critique")

Three chords kick off an AI consistency audit:

#chord_table((
  chord_row("y", "Current view scope, highlighted track only."),
  chord_row("Y", "Current view scope, all tracks."),
  chord_row("Ctrl+Y", "Book scope (widens regardless), all tracks."),
))

The payload includes one bullet line per event with
calendar-formatted start, optional end, title, track,
precision, and `[ORPHAN]` tag where applicable. Linked
paragraphs resolve to slug-paths; characters / places resolve
to titles. Trailing audit checklist instructs the model:

#callout(label: "What the AI looks for")[
  - Travel-time / co-location conflicts.
  - Paragraph references that contradict event dates.
  - Fuzzy-precision overlaps (`season` events colliding).
  - Orphan signals (events that look like they want a scene).
  - Pacing — long gaps, rushed sequences.
]

Prompt resolves through the standard chain (Prompts book
paragraph named `timeline-health` → `prompts.hjson` →
embedded fallback). The `05-timeline-health-example.typ`
seed lands in the Prompts book on `inkhaven init`.

#section("Bund — `ink.event.*`")

Seven stdlib words:

```bund
                          ink.event.list             ( -- list )
                          ink.event.list_orphans     ( -- list )
"Aerin Saga" "Storm" "1A.2.3"   ink.event.add        ( book title spec -- uuid )
event-id "1A.2.5"         ink.event.set_end          ( uuid spec -- )
event-id "season"         ink.event.set_precision    ( uuid prec -- )
event-id "main"           ink.event.set_track        ( uuid track -- )
event-id "aerin-saga/ch4/scene"  ink.event.link_paragraph
                                                      ( uuid path -- )
```

Policy: reads under `store_read`; mutations under
`store_write`.

#section("Bund hooks")

#chord_table((
  chord_row("hook.on_event_added(uuid)", "Every successful add — CLI, TUI `n` chord, Bund `ink.event.add`."),
  chord_row("hook.on_event_orphaned(uuid)", "When an event transitions linked → orphan. Catches deletes via the 1.2.6 AC scrub."),
))

#section("Orphans")

An event with no linked_paragraphs AND no characters AND no
places auto-tags `orphan`. The reconciler runs on every
metadata write that touches an event. Orphans render with
`◌` glyphs across every UI (CLI list, picker, swim lanes).

They're not errors — they're a soft signal that the event
might want a scene attached.

#recap((
  [Opt-in: `timeline.enabled: true` + a calendar preset.],
  [Calendars: sols, gregorian, or custom (configurable named months, seasons, fuzz windows).],
  [CLI: `inkhaven event add/list/show`.],
  [`Ctrl+V e` vertical picker, `Ctrl+V t` swim lanes.],
  [Scope nav: `u` up, `d` descent picker, `b` book, `p` project.],
  [AI critique: `y` track, `Y` scope, `Ctrl+Y` book-wide.],
  [Bund: 7 `ink.event.*` words + 2 hooks (`on_event_added`, `on_event_orphaned`).],
  [Orphans auto-tagged; surfaced via `◌` everywhere.],
))
