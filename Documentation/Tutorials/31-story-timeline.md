# 31 — Story timeline

Inkhaven 1.2.7 adds **events** — first-class story-time
metadata layered over the existing paragraph hierarchy. With
the timeline enabled you can:

- Assign every scene-worthy paragraph a calendar moment
  (sols, gregorian, or a custom calendar with named months).
- View the book as a horizontal swim lane with one row per
  POV / track.
- Drill into a subtree (chapter / subchapter) to see only
  events visible from that scope.
- Ask the AI to audit the timeline for travel-time conflicts,
  fuzzy-precision overlaps, orphan signals, and pacing
  outliers.
- Drive everything from `inkhaven event` on the CLI or
  `ink.event.*` from Bund scripts.

The feature ships across four phases — this tutorial walks
the whole surface end to end.

## Enabling the timeline

The feature is **off by default** so existing projects upgrade
without surprise. Add a `timeline:` block to `inkhaven.hjson`:

```hjson
timeline: {
  enabled: true
  default_track: "main"

  # Three preset shapes. Pick one, OR pick `custom` and
  # define the units array yourself.
  calendar: {
    preset: "gregorian"
  }
}
```

Three calendar presets ship in 1.2.7:

| Preset       | Use case                                  | Display format    |
|--------------|-------------------------------------------|--------------------|
| `gregorian`  | Real-world dates                          | `Y.M.D` (`2026.5.20`) |
| `sols`       | Single-unit "days since day zero"         | `Sol N` (`Sol 142`) |
| `custom`     | Anything else (fantasy / sci-fi calendars) | User-defined        |

### Custom calendar — Aerin Saga example

A custom block specifies the unit stack base-first. Months
can be **named or numeric**.

```hjson
timeline: {
  enabled: true
  default_track: "main"

  calendar: {
    preset: "custom"
    base_unit: "day"

    # Unit stack. First entry is the base (one tick = one
    # of these); each subsequent entry's per_parent is "how
    # many of the level below stack into one of me".
    units: [
      { name: "day",   names: [] }
      { name: "month", per_parent: 30,
        names: ["Frostmoon", "Snowfall", "Greenstart",
                "Bloomtide", "Highsun",  "Goldfall",
                "Mistwane",  "Stormrise","Coldgate",
                "Longnight", "Hearthlit","Yearfall"] }
      { name: "year",  per_parent: 12, names: [] }
    ]

    # Seasons (used by `Precision::Season` fuzz windows).
    seasons: [
      { name: "winter", start_month: 1,  span_months: 3 }
      { name: "spring", start_month: 4,  span_months: 3 }
      { name: "summer", start_month: 7,  span_months: 3 }
      { name: "autumn", start_month: 10, span_months: 3 }
    ]

    epoch_label:        "A"      # "1A.3.15" = First Age year 1
    epoch_before_label: "BA"     # negative years
    display_format:     "{year}{epoch_label}.{month}.{day}"
  }
}
```

**Empty `names` = numeric form**: the formatter falls back
to a bare number when no name is configured. So `month` with
no names renders as `1.3.15` instead of `1A.Greenstart.15`.

## Calendar arithmetic — one i64 under the hood

A `TimelinePoint` is a signed `i64` count of "ticks since
epoch". Negative for prequels. The calendar config converts
ticks ↔ human strings on display / parse.

Parser shapes (custom Aerin example):

| Input        | Parsed precision | Ticks |
|--------------|-------------------|-------|
| `1A`         | Year              | 0     |
| `1A.3`       | Month             | 60    |
| `1A.Frost`   | Month (prefix matches `Frostmoon`) | 0 |
| `1A.spring`  | Season            | 90    |
| `1A.3.15`    | Day               | 74    |
| `-1BA`       | Year              | -360  |
| `Founding`   | Day (alias)       | 0     |

Precision matters for the AI critique — `1A.spring` becomes
a 90-day window the critique can collide against other
season-precision events. `1A.3.15` is exact: a one-day
window.

## CLI — `inkhaven event …`

Three subcommands on the CLI:

```bash
# Create an event.
inkhaven event add "Storm" \
  --start "1A.2.3" --end "1A.2.5" \
  --track main --book-name "Aerin Saga"

# Optional --precision overrides the parser's inference.
inkhaven event add "Spring of Rain" \
  --start "1A.spring" --precision season \
  --book-name "Aerin Saga"

# Chronological listing across the project.
inkhaven event list
inkhaven event list --book-name "Aerin Saga" --track main

# Print one event by slug-path.
inkhaven event show aerin-saga/timeline/storm
```

Sample output:

```
           1A.1.1 ◌  Birth of Aerin                          track=main  path=aerin-saga/timeline/birth-of-aerin
       1A.2.3–2.5 ─  Storm                                   track=main  path=aerin-saga/timeline/storm
           1A.2.8 ●  Marketplace scene                       track=main  path=aerin-saga/timeline/marketplace-scene
```

| Glyph | Meaning |
|-------|---------|
| `●`   | Instant event (no `end_ticks`). |
| `─`   | Duration event (`end_ticks` set). |
| `◌`   | Orphan — no `linked_paragraphs`, no characters, no places. |

## The Timeline chapter — lazy + per-book

The first time you add an event under a book, a new
`Timeline` chapter materialises inside that book with
`system_tag: "book_timeline"`:

```
Aerin Saga/
├── Chapter 1/
├── Chapter 2/
└── Timeline/                ← lazily created on first event add
    ├── Birth of Aerin       ← event paragraph (Node.event = Some(...))
    ├── Storm
    └── Marketplace scene
```

The chapter survives F2-rename — the system tag is the
identifier the timeline pipeline looks up, not the title.
Subsequent events under the same book go into the same chapter.
Different books get their own.

## TUI — Ctrl+V e (event picker)

`Ctrl+V e` opens a vertical chronological picker:

```
┌── Events · 23 events · track filter: all ───────────────────┐
│  1A.1.1     ●  Birth of Aerin                main           │
│  1A.1.3     ─  Flight from Highkeep         Aerin POV       │
│  1A.2.5–2.7 ─  Storm of Year 1              main            │
│  1A.2.8     ●  Marketplace scene            main            │
│  1A.4.spr   ◌  Lost map encounter (orphan)  main            │
│  …                                                           │
│   ↑↓ select · Enter opens · t cycles tracks · Esc closes    │
└──────────────────────────────────────────────────────────────┘
```

| Chord | Effect |
|-------|--------|
| `↑` / `↓` / `Home` / `End` | Move cursor. |
| `t` / `T`           | Cycle the track filter (None → first track → … → None). |
| `Enter`             | Load the event paragraph in the editor. |
| `Esc`               | Close. |

The picker is a chronological snapshot built at open time —
re-open to refresh.

## TUI — Ctrl+V Shift+T (swim-lane view)

The headline UI. `Ctrl+V Shift+T` opens at the **current paragraph's
nearest scope** (Subchapter → Chapter → Book) and renders one
row per track:

```
┌── Timeline · Aerin Saga ▸ Chapter 4 ▸ The Marketplace · 7 events · zoom 1.00× ──┐
│              1A.2                            1A.3                                │
│        J F M A M J J A S O N D | J F M A M J J A S O N D                        │
│ main:                ●─────●         ●                                           │
│                      Storm           Meet                                         │
│ Aerin POV:                ●                       ●─●                            │
│                           Flight                  Trial                          │
│ orphan:                                  ◌                                       │
└──────────────────────────────────────────────────────────────────────────────────┘
  ←/→ scroll · +/- zoom · 0 reset · Home/End jump · u/d/b/p scope ·
  Tab track · Enter open · n new · y/Y/Ctrl+Y critique · Esc close
```

### Scroll + zoom

| Chord            | Effect |
|------------------|--------|
| `←` / `→`        | Scroll by ~10 cells. |
| `PgUp` / `PgDn`  | Page by ~60 cells. |
| `+` / `=`        | Zoom in (0.66× ticks per cell). |
| `-` / `_`        | Zoom out (1.5× ticks per cell). |
| `0`              | Reset zoom to 1.00×. |
| `Home` / `End`   | Jump to first / last event in the visible set. |

Zoom preserves the cursor's screen column — zooming in feels
like drilling into a specific event rather than jumping to
the start of the row.

### Scope navigation

| Chord            | Scope change |
|------------------|--------------|
| `u` / `U`        | Up-scope (subchapter → chapter → book). |
| `d` / `D`        | Open a small descent picker listing immediate child scopes with event counts. |
| `b` / `B`        | Jump straight to book scope. |
| `p` / `P`        | Toggle project overlay — every user book in the current pane. Track labels prefix with book slug. |

The descent picker shows the per-child event count so you
can see where the events are dense before drilling in:

```
┌── Descend into … ────────────────────────────────────────────┐
│ →    ●  Chapter 1                                4 events    │
│         Chapter 2                                7 events    │
│      ◌  Chapter 3                                0 events    │
│      ●  Chapter 4                               12 events    │
│      ●  Timeline (system)                       23 events    │
│   ↑↓ select · Enter descends · Esc returns to same scope     │
└──────────────────────────────────────────────────────────────┘
```

**Event filter rule**: at book scope every event shows; at
sub-book scope an event appears iff itself OR any of its
`linked_paragraphs` is a descendant of the scope. So a Storm
event linked to a scene in Chapter 4 appears when you scope
into Chapter 4 — even though the event itself lives in the
book's Timeline chapter.

### Interactions

| Chord    | Effect |
|----------|--------|
| `Tab`    | Cycle the highlighted track. None → first → … → None. |
| `Enter`  | Open the event closest to the cursor (prefers the highlighted track). Closes the modal, lands you on the event paragraph in the editor. |
| `n` / `N`| Pop a one-line title prompt; on Enter creates a new event at the cursor's tick with the current track highlight. |

## TUI — AI health critique

Three chords inside the swim-lane view kick off an AI
consistency audit. They differ in **scope**:

| Chord            | Scope of events sent     | Tracks sent          |
|------------------|---------------------------|-----------------------|
| `y`              | Current view scope        | Highlighted track only |
| `Y`              | Current view scope        | All tracks           |
| `Ctrl+Y`         | **Book scope** (widens)   | All tracks           |

Each builds a normalised text payload — one event per bullet
line with start (calendar-formatted), optional `→ end`,
title, track, precision, and `[ORPHAN]` tag where applicable.
Linked paragraphs resolve to slug-paths; characters / places
resolve to titles. A trailing audit checklist instructs the
model on what to look for:

> Audit checklist:
> - Travel-time / co-location conflicts: a character at two
>   events whose start-to-start gap is shorter than the world
>   makes plausible.
> - Paragraph mismatches: a manuscript paragraph referencing
>   an event by name but the event's date contradicts the
>   paragraph's setting.
> - Fuzzy overlaps: two events with `season` / `month`
>   precision whose fuzz windows overlap suspiciously.
> - Orphan signals: an event tagged ORPHAN that looks like
>   it should attach to a paragraph mentioned above.
> - Pacing: long unexplained gaps or rushed sequences.
>   Comment only on outliers.

Prompt resolves through the standard chain (Prompts book
paragraph named `timeline-health` → `prompts.hjson` →
embedded fallback). The seed file
`05-timeline-health-example.typ` lands in your Prompts book
on `inkhaven init`; rename to drop `.example` to take effect.

The response streams into the AI pane like any one-shot
inference. The modal closes so the AI pane is visible.

## Bund stdlib — `ink.event.*`

Seven words (1.2.7+):

```bund
                              ink.event.list             ( -- list )
                              ink.event.list_orphans     ( -- list )
"Aerin Saga" "Storm" "1A.2.3" ink.event.add              ( book title spec -- uuid )
event-id "1A.2.5"             ink.event.set_end          ( uuid spec -- )
event-id "season"             ink.event.set_precision    ( uuid prec -- )
event-id "main"               ink.event.set_track        ( uuid track -- )
event-id "aerin-saga/ch4/storm-scene" ink.event.link_paragraph ( uuid path -- )
```

Each `list` / `list_orphans` returns a list of hashes with
`id`, `title`, `slug`, `path`, `start_ticks`, `end_ticks`
(NODATA when instant), `precision`, `track` (NODATA when
default), `is_orphan`, `linked_paragraphs`, `characters`,
`places`.

Policy: reads under `store_read` (default-allowed). All
mutations under `store_write` (default-denied — opt in via
`scripting.enabled_categories`).

## Bund hooks

Two timeline-specific hooks fire automatically:

| Hook                                | When |
|-------------------------------------|------|
| `hook.on_event_added ( uuid -- )`   | Every successful add — CLI, TUI `n` chord, and `ink.event.add`. |
| `hook.on_event_orphaned ( uuid -- )` | When an event transitions linked → orphan. Catches deletes (via the 1.2.6 AC scrub) automatically. |

Example — when an event lands, auto-add a status tag:

```hjson
scripting: {
  bootstrap: '''
    "hook.on_event_added" {
      // ( uuid -- )
      // Need to convert UUID → path for ink.tag.add.
      dup ink.node.get "path" get
      "new-event"
      ink.tag.add
    } register
  '''
}
```

Hook stdout (print/println) is drained at fire time and
surfaced via `tracing::info!(target: "inkhaven::hook::out")` —
so it lands in the project's `.inkhaven.log` from the TUI and
on stderr from the CLI.

## Orphans

An event with NO linked_paragraphs AND no characters AND no
places is auto-tagged `orphan` by the reconciler. The tag
sync runs on every metadata write that touches an event:

- Add an event → no links yet → tagged orphan.
- Link a paragraph via `Ctrl+V A` (`view.add_link` — opens
  link-pick mode in the tree; pick the manuscript paragraph
  where the event happens, Enter to confirm) or
  `ink.event.link_paragraph` → tag removed.
- Delete the only linked paragraph → orphan tag returns;
  `hook.on_event_orphaned` fires.

Opening an orphan event in the editor (Enter on it from the
timeline / picker / tree) lands a one-line hint on the status
bar:

```
orphan event — Ctrl+V A to link a manuscript paragraph (target).
Saving the link drops [ORPHAN].
```

So the chord-to-fix is always one keystroke away from where the
event opened.

Orphans render with the `◌` glyph everywhere (CLI list, picker,
swim lanes). They're not errors — they're a soft signal that
the event might want a scene attached.

## Configuration reference

```hjson
timeline: {
  enabled: true
  default_track: "main"

  calendar: {
    # Pick one of the three flavours:
    preset: "gregorian"   # or "sols" or "custom"

    # When preset = "custom" the rest is required.
    base_unit: "day"
    units: [ ... ]
    seasons: [ ... ]
    epoch_label: "..."
    epoch_before_label: "..."
    display_format: "..."
    parse_aliases: [ { match: "Founding", ticks: 0 } ]
  }

  display: {
    show_orphans: true            # orphan row at the bottom of swim lanes
    swim_lane_max_rows: 12        # truncate beyond this
    default_zoom: 1.0
  }
}
```

## Recap

- **HJSON gate**: `timeline.enabled: true` + a calendar preset.
- **CLI**: `inkhaven event add/list/show`.
- **TUI**: `Ctrl+V e` (picker) · `Ctrl+V Shift+T` (swim lanes).
- **Swim lanes**: scroll / zoom / scope nav (u/d/b/p) / Tab
  track / Enter open / `n` new.
- **AI critique**: `y` track · `Y` scope · `Ctrl+Y` book-wide.
- **Bund**: 7 `ink.event.*` words + `hook.on_event_added` +
  `hook.on_event_orphaned`.
- **Orphans**: auto-tagged when an event has no outbound
  links; surfaced via `◌` glyphs across every UI.
