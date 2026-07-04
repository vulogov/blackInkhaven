#import "../design.typ": *

#chapter(number: 14, title: "Writing Against the World")

Everything so far has been building. You raised a sky, grew a climate, ran the
rivers down the mountains, settled people where the land would hold them, gave
the whole thing a past and a people. It is a great deal of work to have done for
a setting — and it is worth precisely nothing if, when you sit down to write the
scene where your courier rides out of the capital in the first hard frost of the
year, none of it is anywhere near your hand. A world kept in another window is a
binder by another name. This part of the book is about the payoff: the world *at
the desk*, present in the moment you are actually writing, answering the small
questions a scene keeps asking.

#insight[
  A world earns its keep only when it is *at your cursor*. However thorough the
  climate, however careful the history, a setting you have to stop and go look up
  is a setting you will stop looking up — and then quietly start contradicting.
  The whole point of building the world in the same tool you write in is that it
  can come to you, in the scene, without your leaving the page.
]

#section("What season is it, where my character is standing?")

This is the question a scene asks first, and the one a binder answers worst. You
know the date — it is on the scene, on the Timeline — but the date alone does not
tell you what your character *feels* when she steps outside. That depends on
where she is standing on the planet, and your world already knows the answer,
because it computed the insolation for every latitude when you grew the sky.

```
realworld weather --day 300 --lat 55
```

Give it a day of the year and a latitude, and it reports the local season and the
weather you would expect there — and it is *hemisphere-correct*, which is the
detail a hand-kept note almost always gets wrong. Day 300 at fifty-five degrees
north is deep into the descent toward winter; the very same day at fifty-five
degrees *south* is climbing toward high summer, because the tilt that leans one
hemisphere toward the star leans the other away. The command knows this because
it reads it off the astronomy layer you already built, not off a season table
someone typed by hand.

#term("Scene context")[
  The small bundle of world facts that a particular scene sits inside: *where* it
  takes place, *when* on the world's calendar, its *season and weather*, its
  *biome and climate*, and the *people* whose land it is. Scene context is the
  world narrowed to one moment and one spot — the answer to "what is true, right
  here, right now, for this character?"
]

#section("Could they actually get there?")

The second question a scene asks is about distance, and it is the one that most
often catches a careful author out, because prose has no sense of scale. A
sentence can carry a rider from the capital to the far coast in an afternoon
without the slightest strain, and the reader who has been paying attention will
feel the whole world shrink around it. Your world knows the *real* distance,
because it knows how big the planet is and how the map grid maps onto it.

```
realworld travel --from Rivenmouth --to Caldwatch --days 4 --mode horse
```

Name a starting place and a destination, say how many days you are giving the
journey, and pick a mode — `foot`, `horse`, `cart`, or `ship` — and Inkhaven
measures the true distance between the two points and checks it against that
mode's honest pace. Four days on horseback covers only so much ground; if your
draft asked for more, the command says so before your reader does. And crucially,
it does not stop at physics: it consults the *magic ledger* first, so that if
your world has declared some sanctioned way of crossing distance — a road that
folds, a courier's spell, a season when the rivers run fast — the journey is
judged against the rules you actually set, not against a bare horse.

#term("Continuity")[
  The property of a story whose facts stay consistent with one another and with
  its world from one page to the next — the same journey taking the same time,
  the same city in the same place, winter falling when the calendar says it
  should. Continuity is what a built world exists to protect: not to make the
  writing fancier, but to keep it from quietly contradicting itself.
]

#note[
  If you know the map coordinates of a spot that is not a named place — a battle
  in an empty valley, a camp between towns — you can give `travel` a
  `--from-x` and `--from-y` instead of a named `--from`. The check is about
  distance on the real planet, so any point on the grid will do.
]

#section("The whole scene, in one brief")

Often you do not want three separate answers; you want the setting of a scene
handed to you in one piece, the moment before you write it. That is what the
scene brief is for.

```
realworld scene --place Rivenmouth --day 300
```

For a named place on a given day, Inkhaven assembles the *scene context* in a
single reading: the season and weather at that place's latitude, the biome and
climate it sits in, and the culture of the nearest realm — the people whose land
this is, the ethos and beliefs you gave them back when you grew the cultures. It
is the difference between remembering that Rivenmouth is "up north somewhere" and
being told, before the first sentence, that it is a cold-coast town in the last
grey week before the frost, in the lands of a people who hold the sea sacred. You
write a truer scene when the brief comes first.

#section("The world while you type")

The commands are there when you want to ask. But the deeper idea of writing
against a world is that you should not always have to ask — that the setting
should be quietly present as you work. Inkhaven does this two ways in the editor.

As you write a scene that is linked to a place and a date, an ambient *footer
chip* sits at the bottom of the editor showing that scene's context in miniature:
its place, its season, its people. You do not summon it; it is simply there, the
way the word count is there, so that the season your character is standing in is
in the corner of your eye while you describe what she sees.

And when you want the fuller picture, `Ctrl+B W` opens the read-only World
overview — every layer of the world laid out to read — and, when your cursor is
inside a scene, it leads with a *This scene* header: the same context the brief
would give you, pinned to the top, because that is the part of the whole world
you need right now.

#note[
  The footer chip is *self-gating*. It appears only for a scene that is actually
  linked to a place and a date — the two facts it needs to compute a context. An
  unplaced, undated scene shows no chip, and rightly so: there is nothing true to
  say about a where and a when you have not set yet. The chip is a reward for
  having placed the scene, not a nag to do it.
]

#insight[
  Notice what has quietly happened. The season your character feels, the distance
  her journey covers, the people whose land she crosses — none of these are things
  you now have to *remember*. They are things the world *tells you*, in the scene,
  as you write it. That is the whole arc of this book arriving at its point: you
  built the world once so that from here on it can carry the small facts for you.
]

#tryit[
  Run `realworld weather --day 300 --lat 55`, note the season it reports, then run
  it again with `--lat -55` and watch the season flip to its opposite — the
  hemisphere correction, made visible. Then take a journey from your own draft
  that has always felt a little too easy, and put it to `realworld travel
  --from … --to … --days … --mode …`. If the world argues, believe it.
]

#recap((
  [A built world pays off only when it is *at your cursor* — present in the scene
   you are writing, not filed in another window.],
  [`realworld weather --day <N> --lat <deg>` gives the local season and weather
   for a day and latitude, *hemisphere-correct* from the astronomy you built.],
  [`realworld travel --from … --to … --days … --mode <foot|horse|cart|ship>`
   checks a journey against the *real* distance and the mode's pace — and consults
   the magic ledger's `travel_time` rules first.],
  [`realworld scene --place … --day …` hands you the whole *scene context* in one
   brief: season, weather, biome, and the nearest realm's culture.],
  [In the editor an ambient *footer chip* shows the scene's place, season, and
   people, and `Ctrl+B W` opens the World overview with a *This scene* header —
   the chip appearing only for scenes linked to a place and a date.],
))
