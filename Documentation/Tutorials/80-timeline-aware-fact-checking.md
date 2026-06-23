# Tutorial 80 — Timeline-aware fact-checking

*Inkhaven 1.3.30+ (RFC WORLD-5)*

[Tutorial 77](77-world-maps-and-fact-checking.md) showed the world fact-checker —
it knows your world's geography, climate, and demographics, and flags prose that
contradicts them. This tutorial adds the missing axis: **when**. If your project
has a [timeline](31-story-timeline.md), the checker now reads each scene against
the events you've declared — so it can catch the messenger who outran the calendar
and the character who's in two cities at once.

Nothing changes for projects without a timeline; this is purely additive. And as
always, the checker respects your **magic ledger** — a declared exception
suppresses a finding with a note instead of nagging.

## What the timeline adds

The world fact-checker infers time from prose ("a few weeks"); the timeline gives
it **ground truth**. A paragraph linked to a timeline event — or near one in
world-time — gains four kinds of question:

- **Calendar-grounded season.** Snow in a paragraph the timeline places in summer
  is no longer ambiguous; it's a contradiction.
- **Event-derived travel time.** The prose says a journey took "three days"; the
  timeline shows 35 days between the traveller's departure and arrival.
- **Date coherence.** A *midsummer feast* mentioned in a winter-dated scene.
- **Co-location.** A character whose events place them in Velmaril and Korthun at
  the same time.

## Setting it up

You need two things you probably already have: a **world** (`world.hjson`, even a
minimal one for the calendar) and a **timeline** with some events linked to
paragraphs. Link an event to the paragraph it depicts in the swim-lane view (see
Tutorial 31). The richer your event/character/place links, the more the checker
can see.

## As you write

Open a paragraph that depicts a linked event and fact-check it — **`Ctrl+B W →
F`**, or just let the ambient check run. Timeline-aware findings appear in the
**Output pane** alongside the world ones, marked with a **📅**:

```
⊗ fact_check_warning 📅   ch07-p042
   Tropical heat described here, but the timeline places this in winter.

⚠ fact_check_warning 📅   ch07-p042
   The prose suggests about 3 day(s), but the timeline places about 35 day(s)
   between "Departure from Velmaril" and "Arrival in Korthun".
```

The 📅 tells you the timeline was consulted. These run in all five languages — a
Russian paragraph gets Russian findings.

## Catching a character in two places

Co-location is a whole-timeline check (it compares events, not prose), so it has
its own command:

```
$ inkhaven realworld co-location
⊗ [co_location] Mara is in Velmaril ("The reunion") and Korthun ("The siege")
  at overlapping times.

1 co-location conflict(s).
```

If your world allows it — a teleporting mage, an astral projection — declare it in
the `magic:` ledger with a rule that `covers: ["co_location"]`, and the conflict is
suppressed with a note instead.

## On the command line

The same checks run headlessly. `fact-check` is timeline-aware by default when a
paragraph is identified and the project has events:

```
$ inkhaven fact-check --paragraph ch07-p042
$ inkhaven fact-check --paragraph ch07-p042 --timeline-only   # just the timeline checks
$ inkhaven fact-check --paragraph ch07-p042 --timeline-aware off   # world checks only
```

## A note on the older timeline critique

Inkhaven has had a separate timeline AI critique since 1.2.6 (you invoke it from
the swim-lane view). It still works, unchanged. During this interim the two may
flag the same things — to avoid the overlap, lean on the fact-checker and simply
don't invoke the legacy critique. A later release will prune the duplication.

## What you learned

- With a timeline configured, the world fact-checker reads each scene against your
  declared events — **calendar-grounded season**, **event-derived travel time**,
  **date coherence**, and **co-location**.
- Timeline-aware findings carry a **📅** in the Output pane and run automatically
  (`Ctrl+B W → F` or the ambient check), in five languages.
- `inkhaven realworld co-location` checks the whole timeline for a character in two
  places at once; the **magic ledger** excuses the deliberate exceptions.
- `fact-check --timeline-aware on|off|auto` / `--timeline-only` control it on the
  CLI; projects without a timeline are unaffected.

See also: [Tutorial 77 — World maps and the fact-checker](77-world-maps-and-fact-checking.md)
and [Tutorial 31 — The story timeline](31-story-timeline.md). Full reference:
[`../WORLDBUILDING.md`](../WORLDBUILDING.md).
