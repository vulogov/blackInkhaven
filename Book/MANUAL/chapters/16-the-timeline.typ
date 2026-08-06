#import "../design.typ": *

#chapter(number: 16, title: "The Timeline")

A long book keeps two clocks. One is the order in which the reader meets the
scenes — the order of the pages. The other is the order in which the events
actually happened in the world of the story, which a book with flashbacks,
parallel storylines, and a prologue set three ages earlier will scramble on
purpose. Most of the mistakes a novel makes about time are mistakes about the
gap between those two clocks: a messenger who arrives before he could have
ridden the distance, a character standing in two cities on the same afternoon,
a midsummer feast three chapters after the first snow. Inkhaven's *timeline* is
the machinery for keeping the second clock — a record of what happened *when*,
in a calendar you invent, that the reading intelligences can hold up against
the prose.

This is an opt-in layer. A project has no timeline until you ask for one, and
nothing you have read so far changes when you do — the timeline sits over the
paragraph hierarchy you already have, adding a thin skin of *event* metadata to
selected paragraphs rather than a new place to store anything. This chapter
covers the whole of it: what an event is and how you record one, how to define
a calendar of your own devising and how Inkhaven reads dates written in it, the
critique that catches the timeline's internal slips, and — the reason the
feature earns its keep — how the events you record feed the continuity watch
and the knowledge tracker so that *when* becomes a fact those systems can check.

#section("Turning the timeline on")

The feature is off by default so that existing projects upgrade without
surprise, and so that a book that has no need of a calendar is never nagged
about one. You turn it on with a `timeline` block in the project's
`inkhaven.hjson`, and the smallest possible form is three lines and a preset.

#screen(caption: "The minimal timeline block in inkhaven.hjson")[```
timeline: {
  enabled: true
  default_track: "main"
  calendar: { preset: "gregorian" }
}
```]

Two settings do the everyday work. `enabled` is the gate: with it `false` (or
absent) every timeline command refuses politely rather than seeding events into
a project that never opted in — `inkhaven event add` errors with a one-line
reminder to set the flag. `default_track` names the storyline an event belongs
to when you do not say otherwise; think of a track as a swim lane — a POV
character, a parallel plot, a "flashback" line — that lets two events at the
same moment sit on different rows instead of colliding. The `calendar` block is
the substance, and the rest of this chapter's first half is about filling it in.

#callout(label: "Why opt-in")[
  A timeline only pays for itself once you have events in it, and events are
  work to record. Inkhaven would rather ask you to turn the feature on
  deliberately than have every project carry an empty calendar it never uses.
  A non-fiction manuscript, a book of poems, a reference manual — none of these
  wants a story clock, and none of them is asked to.
]

#section("What an event is")

An event is not a new kind of object with its own file and its own database
table. It is a *paragraph* — an ordinary node in your book's tree — that carries
an extra parcel of metadata marking it as something that happened at a moment in
story-time. That parcel is small and entirely legible.

#term("Event")[
  An *event* is a paragraph tagged with a start on the calendar and, optionally,
  an end, a precision, a track, and links to the characters and places it
  involves. Under the hood it is a block of `EventData` hung on a normal node:
  a start tick, an optional end tick, a *precision* (how exact the date is), a
  track label, a list of character ids, and a list of place ids. Everything
  else about the paragraph — its title, its prose, its position in the tree — is
  unchanged.
]

The heart of the parcel is a single number. Every moment on the timeline is
stored as one signed 64-bit integer — a count of *ticks* since the calendar's
epoch, negative for anything before it, so a prologue set in a prior age is just
a negative tick. All the arithmetic the timeline ever does is integer
arithmetic on that number; every scrap of calendar complexity — months of
uneven length, named moons, seasons that wrap the year — lives in the *display*
layer that converts a tick to a human string and back. An event that is an
instant carries a start tick and no end; an event with duration carries both,
and the span it occupies is the closed interval between them.

#subsection("Where events live — the Timeline chapter")

The first time you add an event under a given book, Inkhaven creates a chapter
called *Timeline* inside that book to hold it, and every later event under the
same book goes into the same chapter. It is created lazily — a book you never
add an event to never grows one — and it is marked internally with a system tag
rather than by its title, so you can rename it with `F2` and the timeline
machinery still finds it. Different books each get their own.

#screen(caption: "The Timeline chapter, created on the first event add")[```
Aerin Saga/
├── Chapter 1/
├── Chapter 2/
└── Timeline/            ← lazily created on first event add
    ├── Birth of Aerin   ← an event paragraph (carries EventData)
    ├── Storm of Year 1
    └── Marketplace scene
```]

This is worth a moment's thought, because it explains a distinction you will
lean on. The event paragraph in the Timeline chapter is the *record* that the
event happened; it is not usually the same paragraph as the *scene* that depicts
it, which lives out in Chapter 4 where the reader meets it. The two are joined
by a *link* — the event's list of linked paragraphs — and that link is what lets
a scene inherit its date from the event, and what lets the swim-lane view show a
Storm event under Chapter 4 even though the event record sits in the Timeline
chapter. An event with no such link, and no characters or places either, is an
*orphan*: a moment recorded but attached to nothing, which the tooling marks so
you can decide whether it wants a scene or was a stray.

#section("Recording an event")

There are three ways to record an event, and they differ only in where you are
standing when you do it. The most explicit is the command line, which is also
the clearest way to learn the shape of the thing.

#screen(caption: "Adding events from the command line")[```
# An instant event — precision inferred from the date shape.
inkhaven event add "Marketplace scene" \
  --start "1A.2.8" --book-name "Aerin Saga"

# A duration event — a start and an end.
inkhaven event add "The Storm" \
  --start "1A.2.3" --end "1A.2.5" \
  --track main --book-name "Aerin Saga"

# --precision overrides what the parser would infer.
inkhaven event add "Spring of Rain" \
  --start "1A.spring" --precision season \
  --book-name "Aerin Saga"
```]

The `--start` string is a date written in your calendar, and its *shape* decides
the event's precision unless `--precision` overrides it — a point we return to
below. `--end` makes the event a duration; leave it off and the event is an
instant. `--track` places the event on a storyline, defaulting to your
`default_track`. `--book-name` names the book (by slug or title,
case-insensitively) and is required only when the project holds more than one.
Inkhaven refuses an end that falls before its start — events do not run
backwards — and prints back what it recorded, the date rendered in your
calendar's own display form.

Inside the editor the everyday way is a chord. `Ctrl+V Shift+E` records an event
from wherever you are: with text selected in the Editor, the selection becomes
the event's title; inside the timeline view it drops the event at the cursor's
tick.
This is the flow that fits actual writing — you are drafting a scene, you select
the sentence that anchors it in time, you strike the chord, you type the date,
and the event is on the timeline without your hands leaving the keyboard. Inside
the swim-lane view the bare `n` key does the same at the cursor's position.

#two_track(
  [A novelist records events as the plot demands them: the coronation, the
  siege, the two-week ride. The track label is where parallel POV lines and
  flashbacks earn their keep — give each its own track and the swim lanes stop
  colliding.],
  [A non-fiction writer rarely needs a story clock at all. Where a work is
  genuinely chronological — a history, a biography, a project retrospective —
  the same events + calendar apparatus will hold real dates on the `gregorian`
  preset, and the critique still catches the orphan and the impossible overlap.],
)

#subsection("Linking, characters, and places")

A freshly added event is bare — it has a date and nothing else — which is why it
starts life as an orphan. You give it substance by linking it to the scene that
depicts it: with the event's paragraph open, press `Ctrl+V A` and pick the
manuscript paragraph. The link is recorded and the orphan mark clears. From the
swim-lane view, `Enter` on an event does *not* create a link — it *navigates* to
the scene an event is already linked to (opening the one paragraph, or a picker
when several point at it). The link is also scriptable, through the
`ink.event.link_paragraph` word covered at the end of this chapter.

#callout(label: "Orphans are a nudge, not an error")[
  An orphan event — one with no linked scene — is drawn
  with a hollow `◌` glyph everywhere it appears, and opening one lands a hint on
  the status bar naming the one chord (`Ctrl+V A`) that will link it. It is a soft
  signal that a recorded moment might want a scene, not a mistake to be
  corrected. Deliberate backstory that will never get a scene is free to stay
  orphaned forever.
]

#section("A calendar of your own devising")

The calendar is the part of the timeline you actually design. Its job is to
convert between the single tick number the timeline stores and the dates a
person writes and reads, and Inkhaven gives you three ways to specify one:
two presets for the common shapes and a `custom` form for a world of your own.

#screen(caption: "The three calendar flavours")[```
preset: "gregorian"   real-world dates    →  1917.11.7 · 2026.5.20
preset: "sols"        days since day zero →  Sol 1 · Sol 142
preset: "custom"      anything you invent →  1A.Highsun.15
```]

The `gregorian` preset is the real world: years of twelve named months, months
of thirty days (a deliberate simplification — the timeline measures spans, not
leap-years), and the four seasons already wired up, so a scene dated to month
seven reads as summer without any further declaration. The `sols` preset is the
opposite extreme — a single unit, "days since day zero", displayed as `Sol N` —
which suits a survival log, a voyage, or any story that counts days from a
fixed beginning and needs nothing finer.

#subsection("Building a custom calendar")

A custom calendar is a *stack of units*, declared base-first. The first unit is
the base — one tick is one of these — and each unit above it declares how many
of the level below make one of itself. A day, then a month of thirty days, then
a year of twelve months, is the familiar shape; but the counts are yours, and so
are the names.

#screen(caption: "A custom calendar — the Aerin Saga's reckoning")[```
calendar: {
  preset: "custom"
  base_unit: "day"
  units: [
    { name: "day",   names: [] }
    { name: "month", per_parent: 30,
      names: ["Frostmoon","Snowfall","Greenstart",
              "Bloomtide","Highsun","Goldfall",
              "Mistwane","Stormrise","Coldgate",
              "Longnight","Hearthlit","Yearfall"] }
    { name: "year",  per_parent: 12, names: [] }
  ]
  seasons: [
    { name: "winter", start_month: 1,  span_months: 3 }
    { name: "spring", start_month: 4,  span_months: 3 }
    { name: "summer", start_month: 7,  span_months: 3 }
    { name: "autumn", start_month: 10, span_months: 3 }
  ]
  epoch_label:        "A"    # 1A.3.15 = First Age, year 1
  epoch_before_label: "BA"   # negative years, e.g. -1BA
  display_format:     "{year}{epoch_label}.{month}.{day}"
}
```]

Four pieces here repay attention. A unit's `names` list gives its values display
names — the twelve moons above turn month 5 into *Highsun* — and an empty list
falls back to a bare number, so a numeric month renders `1.5.15` while a named
one renders `1A.Highsun.15`. The `seasons` list names spans of months and is
what gives a date its season; each season declares the month it starts on and
how many months it covers, and a season may wrap the year boundary (winter over
the turn) without confusing the arithmetic. The two `epoch_label` fields are the
suffixes for years at or after the epoch and before it — leaving
`epoch_before_label` empty makes the calendar reject negative years outright,
which is the right choice for a world with no prehistory. And `display_format`
is the template every date is rendered through, with `{year}`, `{month}`,
`{month-name}`, `{day}`, and the epoch tokens as its placeholders. A
`parse_aliases` list, not shown, lets you name landmark dates — `Founding`,
`Day Zero` — that resolve to a fixed tick.

#subsection("How a date is read — precision from shape")

Here is the idea that makes the calendar more than decoration. When you write a
date, Inkhaven infers not only *which* moment you meant but *how exact* you were
being, and it reads that exactness from the *shape* of what you typed. A date
that stops at the year is a year-precise date; one that names a month is
month-precise; one that runs down to the day is day-precise. That inferred
precision — `Tick`, `Hour`, `Day`, `Week`, `Month`, `Season`, or `Year` — is
stored with the event and decides how wide a *window* the event occupies when
the critique asks whether two events could overlap.

#term("Precision")[
  A date's *precision* is how exact you were when you wrote it, inferred from its
  shape. A day-precise date is a point — a one-day window. A season-precise date
  is a whole season — a window ninety days wide, in a thirty-day-month calendar.
  Precision is why "sometime that spring" and "the fifteenth of Highsun" behave
  differently: the first can collide with other vague dates; the second is a
  pin. `--precision` overrides the inference when you want to be explicit.
]

The parser walks the dotted segments of a date under the year — month, then day,
then hour — and the last segment you supply sets the precision. A season *name*
in the month slot is read as season-precision; a landmark alias resolves to its
fixed tick at day-precision. The table below reads the Aerin calendar above.

#screen(caption: "Parsing the Aerin calendar — shape sets precision")[```
   you type       parsed as     precision   ticks
   ----------     -----------   ---------   -----
   1A             year 1        Year          0
   1A.3           year 1 mo 3   Month        60
   1A.Frost       Frostmoon     Month         0   (prefix match)
   1A.spring      spring        Season       90
   1A.3.15        day 15        Day          74
   -1BA           year -1       Year       -360
   Founding       alias         Day           0
```]

Two conveniences fall out of this. Month names match on a unique prefix, so
`1A.Frost` finds *Frostmoon* without your typing it out; and a date is symmetric
through parse and format, so a tick rendered to a string and read back lands on
the same tick. The one value the calendar forbids is a year of zero — there is
no year 0 between `1A` and `-1BA`, the epoch being year 1 — and Inkhaven says so
plainly rather than guessing.

#section("Seeing the timeline")

Two views show you the events you have recorded, and both are reached with the
view prefix. This manual keeps them brief — the companion book *Building the
World* walks every chord — but you should know they exist and what each is for.

`Ctrl+V e` opens a *chronological picker*: a vertical, time-ordered list of every
event, filterable by track, from which `Enter` loads an event's paragraph into
the Editor. It is the fast way to jump to a known event. `Ctrl+V Shift+T` opens
the *swim-lane view*, the headline UI: a horizontal timeline with one row per
track, opened at your current paragraph's nearest scope, which you scroll, zoom,
and drill into by chapter. In both, the same three glyphs read the same way — a
filled dot for an instant, a bar for a duration, a hollow ring for an orphan.

#screen(caption: "The swim-lane view — one row per track")[```
┌─ Timeline · Aerin Saga ▸ Chapter 4 · zoom 1.00× ──────┐
│           1A.2                    1A.3                 │
│      J F M A M J J A S O N D | J F M A M J J A S O    │
│ main:            ●─────●        ●                      │
│                  Storm          Meet                   │
│ Aerin POV:            ●                    ●─●         │
│                       Flight               Trial       │
│ orphan:                            ◌                   │
├───────────────────────────────────────────────────────┤
│ ←/→ scroll · +/- zoom · u/d/b/p scope · Tab track ·   │
│ Enter open · n new · y/Y/Ctrl+Y critique · Esc close  │
└───────────────────────────────────────────────────────┘
```]

The one rule worth carrying out of these views is how *scope* filters events. At
book scope every event shows. At a narrower scope — a chapter, a subchapter — an
event appears if it *or any paragraph it links to* lives inside that scope. So a
Storm event whose record sits in the Timeline chapter still shows when you scope
into Chapter 4, because the scene it links to is there. That is the link doing
its second job: not only dating the scene, but placing the event where the scene
is read.

#section("The timeline critique")

The critique is the timeline watching *itself* — the checks that depend on the
timeline's own semantics and belong to no other system. It once did more; over
several releases, four of its original concerns moved to systems that do them
better (we come to those next), leaving two genuinely timeline-internal checks.
Both are pattern-based, deterministic, and free — no model is called — and both
emit their findings to the Output pane alongside the fact-checker's and the
continuity watch's, with the same severity glyphs and the same `Enter`-to-jump.

#subsection("Orphan events")

The first check finds *orphans* — events linked to nothing, no paragraph, no
character, no place — and grades how much the orphan matters. A trivial stub
recorded five minutes ago is barely a note; a four-word, day-precise event that
has sat unconnected for three months is a likely authorial slip. The grade
combines *significance* — read from how concrete the date is, how rich the
title, and how active the event's track — with *staleness*, how long the orphan
has gone unlinked past a grace window you can set. High significance is a
contradiction-level finding at any age; a middling orphan warns once it goes
stale; a low one stays a passing note.

#subsection("Fuzzy-precision overlap")

The second check is the one the precision system was built for. Two events with
coarse dates — a season here, a month there — each occupy a *window* rather than
a point, and when two such windows collide *suspiciously* the prose probably
cannot place both events consistently. Suspicion is not mere overlap; it is
overlap plus a reason to worry — the two events share a track, or share a
character or place. A same-track, shared-character collision is a warning; a
loose cross-track brush is filtered out below the threshold. When three or more
fuzzy events mutually overlap on a common window, Inkhaven reports the *cluster*
once instead of drowning you in pairwise noise.

#screen(caption: "Critique findings in the Output pane")[```
┌─ Output · 3/3 · timeline-critique ──────────────────┐
│ ⊗ orphan_event                                      │
│   "The Lost Coronation Rite" — day-precise, 4 words,│
│   orphaned 92 days. Link a scene or retire it.      │
│ ⚠ fuzzy_overlap                                     │
│   "Training" and "Journey" (season-precise, main,   │
│   both with Mara) place in the same window.         │
│ ● overlap_cluster                                   │
│   3 season events share a window at Velmaril.       │
├─────────────────────────────────────────────────────┤
│ ↑↓ select · Enter jump · a ask-AI · d dismiss       │
└─────────────────────────────────────────────────────┘
```]

You run the critique over a *scope*, and a few chords set how wide. Inside the
swim-lane view, `y` runs it over the current view for the highlighted track
only, `Y` over the current view for all tracks, and `Ctrl+Y` widens to the whole
book; `F12` is a global shortcut for that same whole-book pass, reachable from
any pane. There is also a
unified pass: `Ctrl+B Shift+C` runs every fast, deterministic checker at once —
the fact-checker and the Socratic reader over the open paragraph, and the
timeline critique over the project — and drops all their findings into the
Output pane together. On the command line, `inkhaven event critique` prints the
same two checks, scoped with `--track` or `--book-name`, and its findings are
localized to the project's working language.

#callout(label: "What moved out of the critique")[
  Four checks that once lived here now live where they belong, and knowing where
  saves you looking in the wrong place. *Travel-time* (a journey the calendar
  says was too fast) and *co-location* (a character in two places at once) are
  the world fact-checker's, run by `inkhaven realworld fact-check
  --timeline-aware` and `inkhaven realworld co-location`. *Date coherence* (a
  midsummer feast in a winter scene) is the fact-checker's too. *Pacing* — how
  densely events pack the page — is the Inner Socratic reader's. The critique
  keeps only what is irreducibly about the timeline's own structure.
]

#section("How the timeline feeds the other intelligences")

The timeline earns its place not by what it shows you but by what it lets the
rest of the tool *know*. Once events carry dates, participants, and places, three
of Inkhaven's reading intelligences can ask questions of the prose that no
generic reader could — because the answers depend on a story clock that only you
have declared.

#subsection("The fact-checker reads when, not just what")

The world fact-checker knows your world's geography and climate; with a timeline
it also knows *when* each scene happens. A paragraph linked to an event, or near
one in world-time, gains an *effective date* — the date of the event it links
to — and from that date, a *season*.

#term("Effective date")[
  A paragraph's *effective date* is the world-time the checker reads it at: the
  start of the earliest event linked to it. From the effective date the checker
  derives the *season* per your calendar, and against that it can judge whether
  the weather, the light, and the festivals a scene describes belong to the time
  the timeline places it. A paragraph with no linked event has no effective date,
  and the timeline simply stays quiet about it.
]

So snow in a scene the timeline dates to summer stops being ambiguous and
becomes a contradiction the checker can name; a three-day ride the calendar says
took thirty-five is a warning it can raise. These timeline-aware findings carry
a small calendar mark in the Output pane to tell you the clock was consulted,
and they run in every project language.

#subsection("SENTINEL continuity uses event placement")

The continuity watch — the system that watches a book for the things a long
manuscript forgets — folds the timeline's placement into its deterministic
findings. *Co-location* is the clearest case: from nothing but the events'
participant and place lists, the watch finds a character whose overlapping
events put them in two different places at once, and reports it without reading a
word of prose. The same event placement backs the date-coherence and travel-time
findings the continuity ledger surfaces. When your world genuinely allows the
impossible — a teleporting mage, an astral projection — a rule in the *magic
ledger* excuses the conflict with a note instead of nagging.

#subsection("KEN uses event participation for who was present")

The knowledge tracker — Inkhaven's watch over *who knows what, and when* — draws
one of its two grant sources straight from the timeline. Its principle is that a
character *present at an event knows what happened there*, from the moment of the
scene that depicts it onward. So every character in an event's participant list
is granted knowledge of that event's subject, dated to the event's first linked
paragraph. That grant is what lets the tracker catch a character who refers to
the coronation before any scene could have told them of it — *premature
knowledge* — the referenced-before-introduced invariant extended from things to
*knowledge*. The event's title is the topic by default; a `reveals:` tag on the
linked scene can name the topic more precisely when the title is terse. This is
the timeline's deepest contribution: presence at an event, which only your
timeline records, becomes the ground truth for what a character could plausibly
know.

#section("The CLI and the Bund word")

Everything the timeline does interactively is also scriptable and headless, which
is what lets a timeline live under version control and take part in a batch run.

The `inkhaven event` subcommand is the command-line surface: `add` records an
event (with `--start`, `--end`, `--precision`, `--track`, `--book-name`), `list`
prints every event in chronological order (filterable by `--book-name` and
`--track`), `show` prints one event's full detail by its slug-path, and
`critique` runs the two retained checks. Every one of them refuses unless
`timeline.enabled` is `true`, so you cannot seed events into a project that has
not opted in.

#screen(caption: "inkhaven event list — the chronological view")[```
$ inkhaven event list --book-name "Aerin Saga"
       1A.1.1 ◌  Birth of Aerin        track=main  …/birth
   1A.2.3–2.5 ─  The Storm             track=main  …/storm
       1A.2.8 ●  Marketplace scene     track=main  …/market
```]

From Bund, the `ink.event` family gives scripts the same operations. Seven words
cover the lifecycle — listing, adding, and refining an event — and each mutation
fires a hook so other scripts can react.

#screen(caption: "The ink.event.* Bund words")[```
                 ink.event.list          ( -- list )
                 ink.event.list_orphans  ( -- list )
"Saga" "Storm" "1A.2.3"
                 ink.event.add           ( book title spec -- uuid )
id "1A.2.5"      ink.event.set_end        ( uuid spec -- )
id "season"      ink.event.set_precision  ( uuid prec -- )
id "main"        ink.event.set_track      ( uuid track -- )
id "saga/ch4/storm"
                 ink.event.link_paragraph ( uuid path -- )
```]

`ink.event.add` takes a book name, a title, and a start spec, creates the event
under that book's Timeline chapter (materialising the chapter if need be),
infers precision from the spec's shape exactly as the CLI does, and returns the
new event's id — which the `set_*` and `link_paragraph` words then refine. The
`list` words return dictionaries carrying every field of an event: its id,
title, slug, path, start and end ticks, precision, track, orphan flag, and its
linked paragraphs, characters, and places. Reads are default-allowed; the
mutating words sit behind Bund's `store_write` category, opt-in like every other
write. A companion set of `ink.event.critique.*` words exposes the two checks to
scripts, and two hooks — `hook.on_event_added` and `hook.on_event_orphaned` —
fire on the lifecycle so a script can, say, tag every new event or flag one that
has just lost its last link.

#recap((
  [The *timeline* is an opt-in layer (`timeline.enabled: true`) that records
  what happened *when*, as *events* — paragraphs carrying a start on a calendar,
  an optional end, a precision, a track, and links to characters and places.],
  [Every moment is one signed integer *tick*; a *calendar* — a `gregorian` or
  `sols` preset, or a *custom* stack of named units with seasons and epoch
  labels — converts ticks to dates and back, and a date's *shape* sets its
  *precision*.],
  [Events live in a lazily-created *Timeline chapter* per book; an event linked
  to nothing is an *orphan* (`◌`), a nudge rather than an error. Record events
  with `inkhaven event add`, `Ctrl+V Shift+E`, or `n` in the swim-lane view.],
  [The *critique* keeps two timeline-internal checks — *orphan events* graded by
  significance and staleness, and *fuzzy-precision overlaps* that collide
  suspiciously — run by `y`/`Y`/`Ctrl+Y`/`F12` or `inkhaven event critique`;
  travel-time, co-location, date coherence, and pacing moved to other systems.],
  [The timeline *feeds the intelligences*: the fact-checker reads each scene's
  *effective date* and season, SENTINEL uses event placement for co-location and
  date coherence, and KEN turns *presence at an event* into who could know what.],
  [Everything is scriptable — the `inkhaven event` subcommands and the
  `ink.event.*` Bund words (with `hook.on_event_added` /
  `hook.on_event_orphaned`) — so a timeline works headless and under version
  control.],
))
