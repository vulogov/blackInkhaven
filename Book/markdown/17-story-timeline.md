# 17 — Story timeline

The timeline is inkhaven's first-class story-time metadata layer. Every scene-worthy paragraph can be tied to a calendar moment; the swim-lane view shows the book as horizontal tracks; AI critique audits the timeline for consistency. This is the headline feature of the 1.2.7 cycle.

The feature is opt-in: set `timeline.enabled: true` in HJSON to turn it on. Existing projects upgrade without surprise.

## Enabling the timeline

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

| Preset | Use case |
|--------|----------|
| `gregorian` | Real-world dates. `Y.M.D` format. |
| `sols` | Single-unit days since day zero. `Sol N` format. |
| `custom` | Anything else. Define units + seasons + format string yourself. |

A custom calendar block for an Aerin Saga (12 months × 30 days, named months):

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

Empty `names` falls back to numeric. Negative ticks (prequels) supported throughout.

## Parser shapes

| Input | Inferred precision | Meaning |
|-------|--------------------|---------|
| `1A` | Year | Year 1, day 1. |
| `1A.3` | Month | Year 1, month 3. |
| `1A.Frost` | Month | Year 1, Frostmoon (prefix-match). |
| `1A.spring` | Season | Year 1, spring (90-day window). |
| `1A.3.15` | Day | Year 1, month 3, day 15. |
| `-1BA` | Year | Year before epoch (prequel). |
| `Founding` | Day | Alias (configured in `parse_aliases`). |

Precision matters for AI critique: a season-precision event collides differently against other season-precision events than against a day-precision one.

## CLI

```
inkhaven event add "Storm" \
  --start "1A.2.3" --end "1A.2.5" \
  --track main --book-name "Aerin Saga"

inkhaven event list --book-name "Aerin Saga" --track main

inkhaven event show aerin-saga/timeline/storm
```

## Where events live

The first time you `event add` under a book, a Timeline chapter materialises inside that book:

```
Aerin Saga/
├── Chapter 1/
├── Chapter 2/
└── Timeline/           ← lazy; system_tag: "book_timeline"
    ├── Birth of Aerin  ← event paragraph (Node.event = Some(...))
    ├── Storm
    └── Marketplace scene
```

Each event is a paragraph carrying `EventData` in its metadata: `start_ticks`, optional `end_ticks`, `precision`, optional `track`, lists of `characters` / `places` / `linked_paragraphs`.

## Ctrl+V e — vertical picker

![figure: timeline-event-picker](images/timeline-event-picker.png) — Ctrl+V e: chronological event picker. Track filter via `t`. Enter opens the event paragraph.

| Chord | What it does |
|-------|--------------|
| ↑ / ↓ / Home / End | Move cursor. |
| t / T | Cycle track filter (None → t0 → t1 → … → None). |
| Enter | Open the event paragraph in the editor. |
| Esc | Close. |

## Ctrl+V t — swim-lane view

The headline UI. Opens at the current paragraph's nearest scope (Subchapter → Chapter → Book):

![figure: timeline-swim-lanes](images/timeline-swim-lanes.png) — Ctrl+V t: swim-lane view. Per-track rows. ● instant; ─ duration; ◌ orphan. Axis labels along the top.

| Chord | What it does |
|-------|--------------|
| ← / → | Scroll by ~10 cells. |
| PgUp / PgDn | Page by ~60 cells. |
| + / = | Zoom in (0.66× ticks/cell). |
| - / _ | Zoom out (1.5×). |
| 0 | Reset zoom to 1.00×. |
| Home / End | Jump to first / last event in the visible set. |
| Tab | Cycle highlighted track. |
| Enter | Open the event closest to the cursor. |
| n / N | Pop a title prompt; commit a new event at cursor tick. |

Zoom preserves the cursor's screen column — drilling in feels anchored to whatever event you were inspecting.

## Scope navigation

| Chord | What it does |
|-------|--------------|
| u / U | Up-scope (subchapter → chapter → book). |
| d / D | Open the inline descent picker — immediate children with event counts. |
| b / B | Jump straight to book scope. |
| p / P | Toggle project overlay (every user book; tracks prefixed with book slug). |

Event filter rule: at book scope all events show; at sub-book scope an event appears iff itself OR any of its `linked_paragraphs` is a descendant of the scope.

![figure: timeline-descent-picker](images/timeline-descent-picker.png) — Descent picker (`d`): immediate child scopes with their event counts. Enter descends; Esc returns.

## AI health critique

Three chords kick off an AI consistency audit:

| Chord | What it audits |
|-------|----------------|
| y | Current view scope, highlighted track only. |
| Y | Current view scope, all tracks. |
| Ctrl+Y | Book scope (widens regardless), all tracks. |

The payload includes one bullet line per event with calendar-formatted start, optional end, title, track, precision, and `[ORPHAN]` tag where applicable. Linked paragraphs resolve to slug-paths; characters / places resolve to titles. Trailing audit checklist instructs the model:

> **What the AI looks for:**
> - Travel-time / co-location conflicts.
> - Paragraph references that contradict event dates.
> - Fuzzy-precision overlaps (`season` events colliding).
> - Orphan signals (events that look like they want a scene).
> - Pacing — long gaps, rushed sequences.

Prompt resolves through the standard chain (Prompts book paragraph named `timeline-health` → `prompts.hjson` → embedded fallback). The `05-timeline-health-example.typ` seed lands in the Prompts book on `inkhaven init`.

## Bund — `ink.event.*`

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

Policy: reads under `store_read`; mutations under `store_write`.

## Bund hooks

| Hook | When |
|------|------|
| `hook.on_event_added(uuid)` | Every successful add — CLI, TUI `n` chord, Bund `ink.event.add`. |
| `hook.on_event_orphaned(uuid)` | When an event transitions linked → orphan. Catches deletes via the 1.2.6 AC scrub. |

## Orphans

An event with no linked_paragraphs AND no characters AND no places auto-tags `orphan`. The reconciler runs on every metadata write that touches an event. Orphans render with `◌` glyphs across every UI (CLI list, picker, swim lanes).

They're not errors — they're a soft signal that the event might want a scene attached.

## Recap

- Opt-in: `timeline.enabled: true` + a calendar preset.
- Calendars: sols, gregorian, or custom (configurable named months, seasons, fuzz windows).
- CLI: `inkhaven event add/list/show`.
- `Ctrl+V e` vertical picker, `Ctrl+V t` swim lanes.
- Scope nav: `u` up, `d` descent picker, `b` book, `p` project.
- AI critique: `y` track, `Y` scope, `Ctrl+Y` book-wide.
- Bund: 7 `ink.event.*` words + 2 hooks (`on_event_added`, `on_event_orphaned`).
- Orphans auto-tagged; surfaced via `◌` everywhere.
