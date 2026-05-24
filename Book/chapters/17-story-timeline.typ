#import "../design.typ": *

#chapter(number: 17, part: "Part V — The Timeline",
  title: "Story timeline")

#dropcap("T")he timeline is inkhaven's first-class story-time
metadata layer. Every scene-worthy paragraph can be tied to
a calendar moment; the swim-lane view shows the book as
horizontal tracks; AI critique audits the timeline for
consistency. This is the headline feature of the 1.2.6
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

#section("Where events live")

The first time you add an event under a book — from the CLI
or the TUI — a Timeline chapter materialises inside that
book:

```
Aerin Saga/
├── Chapter 1/
├── Chapter 2/
└── Timeline/                ← lazy; system_tag: "book_timeline"
    ├── ◆ Birth of Aerin     ← event paragraph (Node.event = Some(...))
    ├── ◆ Storm
    └── ◆ Marketplace scene
```

Each event is a paragraph carrying `EventData` in its
metadata: `start_ticks`, optional `end_ticks`, `precision`,
optional `track`, lists of `characters` / `places` /
`linked_paragraphs`.

The Timeline chapter is auto-managed and #emph[excluded from
every export] — `inkhaven export pdf|markdown|tex|epub|typst`,
the TUI's Ctrl+B B build, and the in-process render all
skip both the Timeline chapter and any individual event
paragraph (anything with `node.event.is_some()`). Your
manuscript prose never gets timeline noise glued onto it.

#section("Tree + editor visual cues")

#chord_table((
  chord_row("`◆ `", "Tree-pane glyph for an event paragraph (replaces the prose `¶`)."),
  chord_row("`◆ <start>[ → <end>] · <prec> · <track>`", "Editor title bar shows full timing while editing an event."),
  chord_row("`[ORPHAN]`", "Red chip in the editor title when an event has no linked paragraphs / characters / places."),
  chord_row("`◆ linked from N event(s)`", "Editor title when a #emph[manuscript] paragraph is referenced by N events — same paragraph can anchor any number of events."),
))

The visible timing in the editor means you can edit the
event's body prose AND see at a glance which calendar
moment it covers — no flipping to the swim-lane view.

#section("Adding events — three paths")

#chord_table((
  chord_row("CLI", "`inkhaven event add \"Storm\" --start \"1A.2.3\" --end \"1A.2.5\" --track main --book-name \"Aerin Saga\"`"),
  chord_row("TUI · `Ctrl+V Shift+E`", "From any pane: opens the timeline AND immediately fires the title prompt. Works on a fresh project — no chicken-and-egg with the CLI."),
  chord_row("TUI · `n` inside Ctrl+V Shift+T", "Position the timeline cursor first, then `n` for a title prompt at that tick + the highlighted track."),
))

For the TUI paths, the title prompt commits with day-precision
at the cursor's current tick. To change the start, add an end,
or rename the track, use the edit-timing chord (next section).

#figure_slot(
  id: "timeline-new-event-prompt",
  caption: "Ctrl+V Shift+E — title prompt for a brand-new event. The status bar shows the calendar-formatted tick the event will land on.",
  height: 35mm,
)

#section("Editing event timing (Ctrl+V Shift+I)")

When the open paragraph is an event, `Ctrl+V Shift+I` pops
a one-line edit prompt for #emph[start | end | track],
pipe-separated:

```
› Sol 13 | Sol 14 | main
```

Pre-filled with the event's current values. Conventions:

#chord_table((
  chord_row("`Sol 13 | Sol 14 | main`", "Start, end, track."),
  chord_row("`Sol 13 |  | main`", "No end (instant event)."),
  chord_row("`Sol 13 | Sol 14 |`", "Drop the track (falls back to `timeline.default_track`)."),
  chord_row("`Sol 13`", "Start only — no end, no track."),
))

Enter commits all three at once. Precision is re-derived from
the start string on each commit, so `Sol 13` is
day-precision, `1A.3` is month-precision, etc. Bad parse
fails to the status bar; the modal stays so you can fix it.

#figure_slot(
  id: "timeline-edit-event-prompt",
  caption: "Ctrl+V Shift+I — edit prompt for the open event's timing. Pipe-separated start | end | track.",
  height: 35mm,
)

#section("Linking events to manuscript paragraphs")

An event is far more useful when anchored to the scene
where it happens. To link from an open event paragraph:

#chord_table((
  chord_row("`Ctrl+V A`", "Enter link-pick mode: focus moves to the tree; navigate to the manuscript paragraph; `Enter` commits."),
  chord_row("Bund", "`event-uuid \"aerin-saga/ch4/marketplace\" ink.event.link_paragraph`"),
))

The link drops the `orphan` tag atomically with the link
write — no separate "reconcile" step required. The same
chord from the inverse direction (`Ctrl+V I` from the
manuscript paragraph, picking the event in the tree) does
the same write.

#callout(label: "Same paragraph, many events")[
  The data model is many-to-one: any number of events can
  carry the same manuscript paragraph in their
  `linked_paragraphs` list. Useful when a single scene
  resolves multiple plot lines, or when the same setting
  recurs across POV tracks. The editor title bar surfaces
  the count (`◆ linked from N event(s)`) so the user knows
  a scene is timeline-anchored before opening Ctrl+V K.
  Removing a link reconciles the orphan tag on every
  affected event.
]

#section("CLI")

```
inkhaven event add "Storm" \
  --start "1A.2.3" --end "1A.2.5" \
  --track main --book-name "Aerin Saga"

inkhaven event list --book-name "Aerin Saga" --track main

inkhaven event show aerin-saga/timeline/storm
```

#section("Ctrl+V e — vertical event picker")

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

#section("Ctrl+V Shift+T — swim-lane view")

The headline UI. Opens at the current paragraph's nearest
scope (Subchapter → Chapter → Book), auto-fitting the full
event span (earliest start → latest end) inside the visible
pane so you can see the whole timeline before drilling in:

#figure_slot(
  id: "timeline-swim-lanes",
  caption: "Ctrl+V Shift+T — swim-lane view, auto-fitted on open. Per-track rows. ● instant; ─ duration; ◌ orphan. Axis labels along the top.",
  height: 70mm,
)

Four distinct navigation modes — fine scroll, event-hop,
page-scroll, range-extremes:

#chord_table((
  chord_row("← / →", "Smooth scroll (~10 cells per press)."),
  chord_row("↑ / ↓", "(1.2.6+) Hop the cursor to the previous / next event, chronologically. Viewport pans automatically to keep the new cursor on screen."),
  chord_row("PgUp / PgDn", "Page-scroll (~60 cells per press)."),
  chord_row("Home / End", "Jump to first / last event."),
  chord_row("+ / =", "Zoom in (0.66× ticks/cell — anchored to cursor)."),
  chord_row("- / _", "Zoom out (1.5× — anchored to cursor)."),
  chord_row("0", "Reset zoom to 1.00×."),
  chord_row("Tab", "Cycle highlighted track."),
  chord_row("Enter", "Open the event closest to the cursor."),
  chord_row("n / N", "Pop a title prompt; commit a new event at cursor tick."),
))

The auto-fit on open means you almost never need `0` (reset)
manually — the initial frame already shows everything. Zoom
preserves the cursor's screen column through `+` / `-`, so
drilling in feels like inspecting, not jumping.

When the book has #emph[zero] events, the swim-lane view
still opens — just empty, with a status hint pointing at
`n` so the first event is one key away:

#figure_slot(
  id: "timeline-empty",
  caption: "Ctrl+V Shift+T on a fresh book with no events. The status bar tells you `press n to add the first event`.",
  height: 50mm,
)

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
  chord_row("hook.on_event_orphaned(uuid)", "When an event transitions linked → orphan. Catches deletes via the AC scrub."),
))

#section("Orphans")

An event with no `linked_paragraphs` AND no characters AND
no places auto-tags `orphan`. The reconciler runs on every
metadata write that touches an event — including link
adds/removes via `Ctrl+V A` from the TUI, `Ctrl+V Shift+I`
edits, and the Bund / CLI write paths. Orphans render with
`◌` across every UI (CLI list, picker, swim lanes) and the
red `[ORPHAN]` chip in the editor title bar.

They're not errors — they're a soft signal that the event
might want a scene attached.

To clear an orphan from the TUI: open the event paragraph
(the editor title's hint reminds you), press `Ctrl+V A`,
pick the manuscript paragraph in the tree, `Enter`. The
write that adds the link removes the `orphan` tag in the
same metadata update.

#recap((
  [Opt-in: `timeline.enabled: true` + a calendar preset.],
  [Calendars: sols, gregorian, or custom (configurable named months, seasons, fuzz windows).],
  [Three add paths: CLI `event add`, TUI `Ctrl+V Shift+E` from anywhere, `n` inside the timeline.],
  [Edit timing: TUI `Ctrl+V Shift+I` on an event ¶ — pipe-separated `start | end | track`.],
  [Tree shows events with `◆`; editor title shows `◆ <timing>` for events and `◆ linked from N event(s)` for anchor paragraphs.],
  [Multi-link: any number of events can target the same manuscript paragraph; orphan tag reconciles atomically with every link write.],
  [`Ctrl+V Shift+T` swim lanes auto-fit on open. Four nav modes: ↑↓ event-hop, ←→ scroll, PgUp/PgDn page, Home/End extremes.],
  [Scope nav: `u` up, `d` descent picker, `b` book, `p` project.],
  [AI critique: `y` track, `Y` scope, `Ctrl+Y` book-wide.],
  [Bund: 7 `ink.event.*` words + 2 hooks (`on_event_added`, `on_event_orphaned`).],
  [Timeline data is auto-excluded from every export format — your manuscript never picks up timeline noise.],
))
