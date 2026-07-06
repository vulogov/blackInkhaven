#import "../design.typ": *

#chapter(number: 8, title: "Giving the World a Past")

Until now you have built a *place*: a sky, a coastline, weather that behaves,
rivers that run downhill, cities that stand where people would build them. It is
a fine place. But if a reader walked into it today, they would feel something
was missing, even if they could not name it. The cities have no ruins under
them. The oldest port has no story about why it is the oldest. Nothing has ever
*happened* here. A place with no yesterday is a stage set — convincing from the
front row, hollow the moment anyone opens a door at the back.

This chapter opens the *time dimension* of your world. It is the first of the
parts that turn a map into a history: not more geography, but a past that
predates page one — an age of founding, an age of growth, a rise and fall of
realms, people moving from one land to another and taking their quarrels with
them. And you will not invent it from a blank page. Your world already contains
its own history, folded into where the cities sit and how large they grew. You
only have to read it out.

#three_dimensions()

#section("The past is already implied")

You have not written a single date, yet your world already tells you which of
its settlements is oldest. Think about how the demographic layer worked: cities
grew where the land supported them — river mouths, fertile valleys, the safe
confluence of two waters. The very best sites were claimed first, because they
are the sites people reach for first. So the largest, best-sited city is almost
certainly the *oldest* one; the modest town on the marginal river arrived later,
when the prime ground was already taken.

That is a chronology hiding in plain sight. The `realworld history` command
reads it. It sorts your settlements by how good their site is and how large they
grew, infers a *founding order* from that ranking, and lays those foundings out
along a single axis of time — measured backward from now.

#note[
  The past is *generated* first: `realworld history` reads it out of the world you
  have already compiled, taking no input but the settlements themselves. To change
  what it infers, change what it grows from — a different `seed`, or a land that
  settles its cities differently, tells a different story. But you may also
  *declare* events of your own, in a `history:` block: they merge into the
  generated chronology, sort into their epoch by year, and — where they name a
  Place you have accepted — adopt onto the story Timeline alongside the inferred
  ones. The world still takes the generated past as its base; your declarations
  are added to it, not swapped in for it. (The one date you *do* always set is the
  world's calendar, back in the `astronomy` block — that is what the years below
  are counted in.)
]

#term("Chronology")[
  The ordering of events along time — what came before what, and how long ago.
  Your world's chronology is inferred from its settlements: the better a city's
  site and the larger it grew, the earlier it was likely founded. The command
  turns that ranking into dated events on the world's own calendar, the one you
  derived from its astronomy back in the calendar chapter.
]

#insight[
  A world without a past is a stage set; give it one and the present acquires
  weight. The same tower is a different thing when it is nine hundred years old,
  when it was raised on the ashes of the realm before it, when it is the last
  building of a city three others have since surpassed. History is not backstory
  filed away — it is the mass that makes the present feel like it *cost*
  something to arrive at.
]

#section("Naming the ages")

A list of foundings dated in raw years is a spreadsheet, not a history. What
makes a past feel like a past is that it comes in *eras* — stretches of time
with a character of their own, so that "the Founding Age" already tells you
something before you have named a single event in it. So `history` divides your
world's inferred past into three epochs.

#term("Epoch")[
  A named stretch of a world's history with its own character. This command
  produces three, in order from oldest to most recent: the *Founding Age*, when
  the first and best-sited cities were established; the *Age of Expansion*, when
  people spread to the lesser sites and realms grew to their present reach; and
  the *Present Age*, the recent past that leads up to now. Every generated event
  falls into one of the three.
]

#term("The present (year 0)")[
  The instant your story's "now" sits at. All of the world's history is dated in
  years *before the present* — the founding of the oldest city might land at
  year 900, a realm's fall at year 210 — so that year 0 is always "when the
  reader arrives." Anchoring on the present, rather than an absolute epoch,
  keeps the history readable: a number is immediately a distance into the past.
]

#section("What happened, not just when")

Foundings alone are a skeleton. Onto it, `history` hangs *events* — the things a
historian would actually recount. Two kinds fall out naturally from what your
world already contains. First, the *rise and fall of realms*: where clusters of
cities imply a polity (the next chapter's subject), the command dates its rise
in one epoch and, for some, its fall in a later one — the older the world, the
more it has buried. Second, *migrations between biomes*: a people leaving the
cold grassland for the temperate forest, the desert's edge for the river valley,
each dated and given a direction. Movements like these are the engine of real
history — a drought empties one region and fills another — and they leave
exactly the kind of hook a story reaches for.

#question[
  What happened before page one? Something did. Which realm fell, and is its
  fall still resented? Who arrived from somewhere else, and are they still called
  newcomers by the people who were already here? Name the one event, before your
  story opens, whose consequences your protagonist lives inside — then see
  whether the generated history offers you something close to it.
]

#tryit[
  Run `realworld history`. Read it top to bottom: the three epochs, the founding
  dates of your cities, the realms that rose and the ones that fell, the
  migrations and their directions. Do not adopt anything yet — just read it as a
  chronicle of the world you have been building, and notice which single line
  makes you want to write a scene.
]

#section("Reading it, keeping it, adopting it")

`realworld history --json` prints the whole chronology as structured data, for
tools and scripts. `realworld history --materialize` writes it down as a History
chapter in your World book, so the epochs and events live beside the rest of the
compiled world, readable and searchable. Both of those record the world's
*proposal* — they do not touch your manuscript.

Adoption is the separate, deliberate step. Alongside its chronicle, `history`
prints ready-made command lines — `inkhaven event add …`, one per event — that
place a chosen event onto your story's *Timeline*, the system book your scenes
are dated against. You copy the lines for the events you want, and only those.
The fall of the old realm, if it matters to your plot; the great migration, if
your people are its descendants; nothing you do not need.

#note[
  The world proposes; you adopt. `history` never writes an event onto your
  Timeline on its own — it hands you the `inkhaven event add …` lines and stops.
  You decide which foundings, falls, and migrations are real for *your* story,
  and paste in only those. The chronicle is a generous draft of the past; the
  author edits it down to the history the book actually needs.
]

A last point of craft: the history is inferred, not decreed. If your plot needs
the youngest city to be the ancient one — a colony that outgrew its motherland,
a capital deliberately founded on empty ground — override it. Rename an epoch,
redate a fall, drop a migration that does not fit. The generated past is a
strong first draft precisely because it is consistent with the geography; but,
as always, the author has the last word over which yesterday the world remembers.

#section("Declaring your own events")

Inference is generous, but it cannot know the one event your book is built on —
the treaty, the landing, the betrayal that has no settlement to imply it. So you
may write events straight into `world.hjson`, in a `history:` block, and the
world will fold them into the chronology it generates:

#hjson[
```
history: {
  events: [
    {
      year: -1200
      title: "The First Landing"
      epoch: "Founding Age"
      places: ["Karthage"]
      description: "The seafarers make landfall."
    }
  ]
}
```
]

Each field earns its place. The `year` is dated the same way the generated events
are — years *before the present*, so `-1200` is twelve centuries ago (the sign is
simply the direction into the past). The `epoch` is optional: name it and the
event files under that age; leave it out and the world infers the epoch from the
year, dropping the event into whichever of the three ages the date falls in. The
`places` list links the event to Places you have already accepted — that link is
what lets a declared event adopt onto the Timeline, exactly as a generated one
does; an event with no place stays in the chronicle but has nowhere to sit on the
story's dated axis. The `title` and `description` are yours to phrase as a
historian would.

#note[
  A declaration is checked for plausibility, not obeyed blindly. The world warns
  if a `year` lands *after* the present — an event that has not happened yet — or
  so far back that it predates recorded history, and it warns if you name an
  `epoch` that does not contain the `year` you gave (an event you filed under the
  Founding Age but dated to last week). The warning does not delete your event;
  it tells you the past you wrote does not line up with the past the world knows,
  and leaves the reconciling to you.
]

#section("The past as a rising tide")

`realworld history` gives you the events — the foundings, the rises and falls, the
migrations — dated on the world's own calendar. But sometimes what you want is not
the events but the *shape* of the past: how big the world was as it grew, how many
realms stood at once, when the tide of settlement was rising and when it turned.
For that there is `realworld chronicle`:

```
inkhaven realworld chronicle
```

It reads the same compiled history, but instead of a list of events it reports, at
the close of each epoch, *how far the world had grown by then* — how many
settlements of each kind, how many people they held, how many realms were
standing. You watch the Founding Age hold a handful of towns, the Age of Expansion
swell with cities, and the Present Age settle into the world your story opens in,
a realm or two having risen and waned along the way.

#note[
  The chronicle invents nothing. It is the *same* deterministic history, read as a
  running total rather than a timeline — a presentation, not a simulation. The
  world does not model growth year by year; it shows you the state its own
  chronology already implies at each turning of the age. When you need a felt sense
  of scale over time — for a prologue, a founding myth, a fallen empire in the
  backstory — the chronicle is where you read it.
]

#recap((
  [A world with no past is a stage set; a *chronology* — inferred from where the
   cities sit and how large they grew — gives the present its weight.],
  [`realworld history` reads the founding order out of your settlements, divides
   the past into three *epochs* (Founding Age, Age of Expansion, Present Age),
   and dates everything in years *before the present (year 0)*.],
  [It generates events a historian would recount — the *rise and fall of realms*
   and *migrations between biomes* — on the world's own calendar.],
  [`--json` and `--materialize` record the proposal; the printed `inkhaven event add …` lines let you *adopt* chosen events onto the story Timeline — you pick
   which yesterdays are real.],
))
