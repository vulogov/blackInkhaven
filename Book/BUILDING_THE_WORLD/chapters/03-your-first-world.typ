#import "../design.typ": *

#chapter(number: 3, title: "Your First World")

Enough principle. In this chapter you will make a world — a real one, compiled from
a single small file, with a sky and land and rivers and cities in it — in about
four commands. You will not understand every layer yet; Part II is for that. The
aim here is only to see the whole machine turn over once, from a definition to
pages you can read, so that everything after this has something concrete to stand
on.

Every command lives under one program, `realworld`, run from your project folder.

#section("Scaffolding a world")

You do not write `world.hjson` from a blank page. Ask `realworld` to lay down a
starter for you:

```
realworld new myworld
```

This writes a `world.hjson` describing a plain, Earth-like world — a single yellow
star, a planet much like our own, a familiar calendar — ready to compile as-is and
ready for you to edit.

#term("world.hjson")[
  The single file that defines your world. HJSON is a relaxed form of JSON that
  tolerates comments and unquoted keys, so the file stays readable by a human. It
  holds your world's `name`, its `seed`, its `primary_language`, and blocks of
  physical and declared detail — the *whole* definition of your world lives here,
  and nowhere else.
]

#note[
  `world.hjson` is written to your *project root* — the top folder of the project
  you have open in Inkhaven, alongside your manuscript. There is one per project.
  When any `realworld` command speaks of "the world," it means this file and what
  compiles from it.
]

Open it, and the shape is easy to read even before you know every field:

```
{
  name: "My World"
  seed: 0x5eed
  primary_language: "Common"

  astronomy: {
    star:   { class: "G", luminosity_solar: 1.0, age_gyr: 4.6 }
    planet: { mass_earth: 1.0, radius_earth: 1.0, axial_tilt_deg: 23.4,
              day_length_hours: 24, rotation_direction: "prograde" }
    orbit:  { semi_major_axis_au: 1.0, eccentricity: 0.017, year_length_days: 365 }
  }
}
```

Three things at the top name the world: a `name`, a `seed` — the number from the
introduction that fixes every "random" choice, here written in hexadecimal — and a
`primary_language` for the names it will generate. Then the `astronomy` block, the
one required layer, sets the physics the rest of the world emerges from: the star's
class and brightness, the planet's mass and tilt and day, the orbit that decides
the length of the year. Change nothing, and you have a workable Earth-like world.
Change the tilt, and you have set a different climate in motion — but that is
Part II's pleasure, not this chapter's.

#section("Checking it holds together")

Before compiling the whole thing, ask whether the definition is even valid:

```
realworld validate
```

This compiles every layer in turn and reports each one `ok`, or stops at the first
value it cannot make sense of — a negative year, a calendar whose months do not add
up. It is the quickest way to catch a broken edit, and worth running whenever you
change the file.

#section("Compiling the world")

Now grow it. With no arguments, `realworld compile` runs the entire layer chain —
astronomy through demographics — and holds the finished world in hand:

```
realworld compile
```

#term("Compile")[
  To run the world's layers and produce the finished world from `world.hjson`. A
  bare `realworld compile` (or `compile all`) computes the *whole* world; you can
  also compile a single layer by name while you experiment. Compiling is pure and
  deterministic: the same file always compiles to the same world.
]

Compiling gives you the world, but it does not yet write anything you can sit and
read. For that, materialise it:

```
realworld compile --materialize
```

#term("Materialize")[
  To write the compiled world into readable pages — chapter by chapter, layer by
  layer — inside Inkhaven's *World book*. Compiling produces the world in memory;
  materialising commits it to the page, so you and Inkhaven can read it, search it,
  and cite it from your manuscript.
]

#term("The World book")[
  The read-only *system book* that holds your materialised world: a chapter per
  layer — the sky, the land, the climate, the waters, the peoples. It sits
  alongside your manuscript like the Places and Timeline books, and is rewritten
  each time you materialise, so it is always a faithful picture of the current
  world rather than a set of notes that can drift.
]

Here is the whole path you have just walked, from definition to readable pages:

#compile_flow()

If you would rather glance at the definition than read the full book, `realworld
show` prints the world's current shape to your terminal — a fast way to confirm
what seed and star you are working with.

#tryit[
  Do the whole loop once, now. Run `realworld new myworld` to scaffold a world,
  then `realworld compile` to grow it, then `realworld compile --materialize` to
  write it into the World book. Finally, press `Ctrl+B W` in the editor to open the
  read-only *World overview* and page through the sky, land, climate, waters, and
  peoples the seed grew for you. You wrote a handful of lines; read how much world
  came back.
]

#section("What you now have")

Take a moment with what just happened. From a file you could read in a minute — a
star, a planet, a seed — a full physical world emerged: a calendar and seasons,
continents and mountains, climate bands and biomes, rivers and lakes, and cities
standing where the land supports them. You placed none of it by hand. You set the
conditions, and the system did the rest, the same way it will every time you ask.

That is the end of Part I. You now understand *why* a world is worth building, that
a world is a *system* whose detail emerges from a few choices, and — as of this
chapter — you have a real, compiled world of your own to open with `Ctrl+B W`. In
Part II you will go into that world one layer at a time, starting with the sky, and
learn to shape each layer on purpose. The machine has turned over once; now you
learn to drive it.

#recap((
  [`realworld new <name>` scaffolds a starter `world.hjson` in your project root —
   an Earth-like world with a `name`, `seed`, `primary_language`, and a required
   `astronomy` block.],
  [`realworld validate` compiles every layer and reports each `ok`, catching a
   broken value before you rely on it.],
  [`realworld compile` grows the *whole* world; `--materialize` writes it as
   readable, always-current chapters into the *World book*; `realworld show` prints
   the definition.],
  [`Ctrl+B W` opens the read-only *World overview*. You end Part I with a real
   compiled world of your own, ready to explore layer by layer.],
))
