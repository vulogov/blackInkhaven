#import "../design.typ": *

#chapter(number: 17, title: "A World, End to End")

Every chapter so far has taught one movement of the work in isolation — the sky
on its own, the rivers on their own, the history on its own. That is how a craft
is learned, but it is not how a world is built. When you sit down to make one,
the movements run together: you set a few numbers, watch the land fall out of
them, name a region because the land invited it, read the past that the cities
imply, and only then start writing scenes that lean on all of it at once. This
chapter is that single, continuous session. We will build one world, called
*Tharn*, from an empty folder to a scene you can fact-check — and everything you
learned in isolation will arrive, in order, exactly where a working writer would
reach for it.

#world_arc()

Follow along at your own keyboard if you can. Every command below is one you have
met; the point here is not any single one of them but the *shape they make
together* — define, grow, deepen, write, and loop back. Read Tharn as a worked
example, then build your own the same way.

#section("Define — the starter world")

Everything begins with an empty definition. In a project folder, ask Inkhaven to
scaffold one:

```
realworld new tharn
```

This writes a starter `world.hjson` — a small, readable file with Earth-like
numbers already filled in. Open it and you will find the shape you know: a `name`,
a `seed`, a `primary_language`, and an `astronomy` block holding a star, a
planet, an orbit, one moon, and a calendar. Nothing exotic yet; a familiar sky
you can trust while you decide what to change. Tharn's opening lines read:

```
name: "Tharn"
seed: 0x7a17
primary_language: "Tharnic"
astronomy: {
  star:   { class: "G2V", luminosity_solar: 1.0, age_gyr: 4.6 }
  planet: { mass_earth: 1.0, radius_earth: 1.0, axial_tilt_deg: 23.4,
            day_length_hours: 24.0, rotation_direction: "prograde" }
  orbit:  { semi_major_axis_au: 1.0, eccentricity: 0.017, year_length_days: 365.25 }
  moons:  [ { name: "Vael", mass_lunar: 1.0, period_days: 27.3 } ]
}
```

#note[
  `realworld new <name>` never overwrites an existing `world.hjson`. It gives you
  a clean, valid starting point every time — a real world you could compile as-is,
  not a stub full of blanks.
]

#section("Grow — a sky of our own, then validate")

A copy of Earth is a fine place to learn, but Tharn should feel like somewhere.
Two small edits to the astronomy will do it. First, a slightly larger axial tilt
— push `axial_tilt_deg` from `23.4` to `28.0`, for sharper seasons, a hotter
summer and a longer-shadowed winter. Second, a second moon — Tharn will have two,
so its nights and its tides have a layered rhythm:

```
axial_tilt_deg: 28.0
moons: [
  { name: "Vael",  mass_lunar: 1.0,  period_days: 27.3 }
  { name: "Corin", mass_lunar: 0.4,  period_days: 11.8 }
]
```

Before growing anything on top of this, make sure it holds together:

```
realworld validate
```

`validate` compiles every layer in turn and reports each one `ok` — it is your
proof that the definition is sound before you build on it. Change a number to
something impossible and this is what catches it. Tharn validates cleanly; the
calendar-consistency check is quiet, the two moons resolve to two synodic
periods. The sky is sound. Now let the rest of the world fall out of it.

#tryit[
  Before you move on, run `realworld compile --layer astronomy` and read the two
  moons' synodic periods and the four season markers. You are looking at the raw
  rhythm — months and tides and solstices — that every later layer will inherit.
]

#section("Grow — compile the world and read the layers")

One command now grows the whole physical world and writes it down where you can
read it:

```
realworld compile --materialize
```

The bare compile runs every layer in order; `--materialize` writes each one as a
chapter into your World system book. Open it with `Ctrl+B W` and read what
emerged, layer by layer — this is the moment the numbers become a place.

*Geology* came first, seeded from `0x7a17`: a handful of tectonic plates, two
continents parted by a strait, a long mountain range thrown up where two plates
met, minerals salted into the ranges, a sea level, a heightmap. None of it was
placed by hand; all of it followed from the seed.

*Climate* came next, out of the sky and the land together. Tharn's sharper tilt
shows here — the temperate band runs hot in summer, and the lee of that long
mountain range is dry, a rain shadow where the wet winds have already spent
themselves climbing. The biomes aggregate into zones: taiga in the north,
temperate forest and grassland across the midlands, a hot desert behind the
mountains, savanna toward the equator.

*Hydrology* came third, water finding its way downhill across the heightmap.
Rivers gather off the mountains and run to the sea; two of them meet at a
confluence in the midland forest; a great river reaches the coast at a broad
mouth. And the layer marks its *settlement priors* — the river mouth, the
confluence, a fertile valley — the sites people would reach for first.

*Demographics* came last, and put people on exactly those sites. A city at the
great river's mouth, a town at the midland confluence, villages scattered where
the land supports them, all arranged into a rank-size hierarchy. Tharn now has a
capital-in-waiting — the mouth-city, largest and best-sited — without your having
chosen it. The land chose it.

#note[
  `Ctrl+B W` opens the World overview read-only — you are reading the compiled
  world, not editing it. Everything you see there was written by `--materialize`;
  to change it you change `world.hjson` and recompile, never the book directly.
]

#section("Declare — a region and an economy")

So far Tharn has grown entirely on its own. Now the author's hand enters. The
mouth-city and its river valley deserve a name and a character, so declare a
`geography` region over them, and give the world an `economy` to say how it makes
its living:

```
geography: {
  regions: [
    { name: "The Vale of Enst", kind: "river_valley",
      center_lat: 34.0, center_lon: -12.0 }
  ]
}
economy: {
  base: "agrarian"
  trade_goods: ["grain", "river-fish", "mountain-iron"]
}
```

Then recompile so the declaration and the physics are reconciled into one world:

```
realworld compile --materialize
```

Nothing you declared overrode the land — the Vale of Enst still sits where the
fertile valley was; the iron in the trade goods is the mineral the geology
already salted into the range. You did not fight the world; you named what it
offered. That is the whole discipline of declaring: the physics is the hand that
emerges, your names and rules are the hand you set, and the compiler holds both.

#section("Deepen — a past and a people")

The place is complete; now read the time and the peoples folded into it. Four
commands, in the order a working writer would run them.

```
realworld history
```

Tharn's history reads the founding order out of the settlements — the mouth-city
oldest, the marginal villages youngest — divides it into the *Founding Age*, the
*Age of Expansion*, and the *Present Age*, and dates every event in years before
the present. A realm rose in the Vale during the Age of Expansion; a smaller
inland realm fell two centuries before the present; a migration ran out of the
dry lands behind the mountains toward the river forest during a long drought.
None of it was decreed. It fell out of where the cities sit.

```
realworld polities
```

The settlements cluster into nations around their largest capitals. Tharn gets
two: *Enst*, the river realm centred on the mouth-city, populous and agrarian;
and *Kadur*, a lean upland realm along the mountains' dry flank. They are seeded
as rivals — the drought migration left a grievance — which is precisely the kind
of hook a plot reaches for.

```
realworld culture
```

Each polity gets one culture. Enst's ethos is drawn from its river-valley biome —
settled, patient, water-minded — with a belief about the two moons' tides;
Kadur's is harder, drawn from the desert's edge. Each culture carries a *language
profile* — for Enst, say, `SVO · fusional · non-tonal` — a sketch you can realise
in the ConLang suite with `inkhaven language`, plus a naming sample to write
against.

```
realworld ecology
```

Finally the living world: flora and fauna archetypes per biome, and a keystone
animal for each land biome — the river-forest's great heron, the desert's burrow
lizard. Tharn is no longer a map with cities on it. It has a yesterday, two
peoples with a quarrel between them, and animals a scene could put on the page.

#tryit[
  Run all four — `history`, `polities`, `culture`, `ecology` — and read them as
  one chronicle. Notice how they agree: the rival polities, the drought
  migration, the desert ethos, and the burrow lizard are all the *same fact* seen
  from four angles. That agreement is the world holding together.
]

#section("The author's hand — calendar and history events")

The world has proposed a calendar and a past. Adopt the parts you want, and only
those. First the calendar:

```
realworld calendar
```

This derives a story-Timeline calendar from Tharn's astronomy — its months,
weekdays, the new year aligned to the spring equinox — and prints lines you adopt
into `timeline.calendar`. Now your scenes' dates and your characters' seasons
share one root and cannot drift apart.

Then a few history events. `realworld history` printed `inkhaven event add …`
lines, one per event. Copy in only the ones your story needs — for Tharn, the
founding of the mouth-city and the fall of the inland realm — and leave the rest
on the cutting-room floor.

#note[
  The world proposes; you adopt. `calendar` and `history` hand you ready-made
  command lines and stop. Nothing reaches your Timeline until you run the lines
  you chose. The author always has the last word.
]

#section("The author's hand — proposing Places")

Settlements become writable Places the same deliberate way:

```
realworld propose
```

The world proposes its settlements as Place entries; `realworld proposals` lists
them; you accept or reject each. Accept two for Tharn — *Enstmouth*, the capital
at the great river's mouth, and *Karrow*, the midland confluence town — and reject
the villages you will never set a scene in. Only the two you accepted become
entries in the Places book, taggable in your prose.

#section("At the desk — writing against the world")

Now the payoff. Write a scene set in Enstmouth in early autumn, and put the world
to work beneath it. Ask what the desk should know:

```
realworld scene --place Enstmouth --day 250
```

The scene brief answers in one breath: the season and weather at Enstmouth's
latitude on day 250, its river-valley biome and climate, and the culture of the
nearest realm — Enst, water-minded and patient. You are writing with the season,
the land, and the people already in front of you.

```
realworld weather --day 250 --lat 34
```

The weather call sharpens it: the local season and the day's weather at latitude
34, so the light and the air in your paragraph are the world's, not a guess.

```
realworld travel --from Karrow --to Enstmouth --days 3 --mode horse
```

And when your courier rides from Karrow to Enstmouth in three days, `travel`
checks the real distance against a horse's pace — and consults the magic ledger's
`travel_time` rules, if you declared any — and tells you whether the journey is
plausible. For Tharn's midland distances, three days on horseback is comfortable;
the world confirms it.

#section("The loop closes — fact-check")

Last, let the world read your prose back. Write a deliberately wrong sentence and
check it:

```
realworld fact-check --text "In deep winter the courier crossed the whole of Tharn on foot in a single day."
```

Two things are wrong, and the world flags both: no traveller crosses a continent
on foot in a day — the distance and a walker's pace do not agree — and "deep
winter" collides with the scene's dated early autumn. The fact-checker measures
your sentence against the same compiled world every other command drew from. Fix
the sentence, run it again, and it comes back clean.

#note[
  The magic ledger is how you silence a *deliberate* exception. If Tharn's
  couriers really do cross the world in a day by some declared road-magic, add a
  `travel_time` rule to the `magic` block; the fact-checker then suppresses that
  one warning and stops crying wolf. An exception you declared is not a mistake —
  and the world learns to tell the difference.
]

#insight[
  This is the whole loop, and it is a loop on purpose. You *defined* a sky, *grew*
  a world out of it, *deepened* it with a past and a people, and *wrote* against
  it — and the fact-check at the end sent you back to the prose, or back to the
  definition, to reconcile them. A built world is never finished and filed away;
  it is a thing you return to, tightening the fit between the world and the page
  each time the story asks a new question of it. That returning — not any single
  command — is what it means to write inside a world that holds together.
]

#recap((
  [Building a world is one continuous loop — *define, grow, deepen, write* — not a
   set of isolated commands; this chapter ran it end to end on the world *Tharn*.],
  [You *grow* with `realworld new`, edits to the astronomy, `validate`, and
   `compile --materialize`, then read the emergent layers in `Ctrl+B W`.],
  [You *deepen* by reading the past and peoples the land implies — `history`,
   `polities`, `culture`, `ecology` — and by *declaring* regions and an economy
   that name what the physics already offered.],
  [The author's hand is always deliberate: `calendar`, history events, and
   `propose`/accept adopt only what you choose into the Timeline and Places.],
  [At the desk, `scene`, `weather`, `travel`, and `fact-check` write and check
   your prose against the same compiled world — and the check loops you back to
   refine it.],
))
