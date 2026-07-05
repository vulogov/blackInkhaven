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
  There is no `history:` block in `world.hjson`, and you will not find one. A past
  is not something you *declare* — it is something the world already implies, so
  `realworld history` takes no input but the world you have already compiled.
  To change the history, change what it grows from: a different `seed`, or a land
  that settles its cities differently, tells a different story. (The one date you
  *do* set is the world's calendar, back in the `astronomy` block — that is what
  the years below are counted in.)
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
