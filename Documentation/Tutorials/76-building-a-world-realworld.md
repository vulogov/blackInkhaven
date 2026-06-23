# Tutorial 76 — Building a world with `realworld`

*Inkhaven 1.3.25+ (the compiler); setting blocks + the Earth example are 1.3.27+*

Most worldbuilding tools ask you to draw a map and fill in a wiki. Inkhaven
takes the opposite approach: you declare the **physics** of your world — its
star, its tilt, its tectonics — and a deterministic compiler *derives* the rest,
layer by layer, the way the real one works. Astronomy sets the seasons; geology
raises the mountains; climate falls out of latitude and elevation; rivers follow
the slope; people settle where the land can feed them. You get a coherent,
internally-consistent world you can then *write against* — and a fact-checker
(Tutorial 77) that knows when your prose contradicts it.

This is RFC WORLD-4, Branch A. Everything here is a pure function of your
definition plus a seed: same inputs, same world, every time.

## The world definition

A world lives in one file at the project root, `world.hjson`. Scaffold a
starter:

```
$ inkhaven realworld new Aldoria
scaffolded world.hjson for world `Aldoria`
edit it, then `inkhaven realworld compile`
```

Open it. The only required block is `astronomy`; everything else is optional and
has sensible defaults. The smallest useful world is a star, a planet, an orbit,
and a calendar:

```hjson
{
    name: "Aldoria"
    seed: 0x1A2B3C
    primary_language: "en"

    astronomy: {
        star:   { class: "G2V", luminosity_solar: 1.0 }
        planet: { mass_earth: 1.0, radius_earth: 1.0, axial_tilt_deg: 23.4, day_length_hours: 24.0 }
        orbit:  { semi_major_axis_au: 1.0, eccentricity: 0.017, year_length_days: 365 }
        moons:  [ { name: "Luna", mass_lunar: 1.0, period_days: 27.32 } ]
        calendar: { months: 12, month_length_days: 30, weekdays: 7 }
    }
}
```

The **seed** drives every procedural layer. Change it and the continents,
climate, rivers, and settlements all regenerate; keep it and the world is
reproducible forever. It accepts a decimal integer or a `0x…` hex string.

Check that it parses and is internally consistent:

```
$ inkhaven realworld validate
ok — world `Aldoria`, seed 0x1a2b3c, primary language `en`
  astronomy: 1 moon(s), 12-month calendar
```

## The five layers

The compiler runs five layers in dependency order, each a pure function of the
ones before it. Compile any single layer with `--layer` to inspect it:

```
$ inkhaven realworld compile --layer demographics
demographics · Aldoria (160×120 grid)
  population: 6.1M · 65% of land habitable
  settlements: 20 (1 cities, 10 towns, 9 villages)
    city     ( 34, 71) · pop 27,010 · fertile_valley · mediterranean
    …
```

1. **Astronomy** — closed-form physics: Kepler's year length, daily insolation
   by latitude, lunar synodic periods, tides, and a calendar-divergence check.
   This is the one layer with no randomness; it's treated as fact.
2. **Geology** — the seed grows tectonic plates, which raise a procedural
   heightmap → continents, mountain ranges, and mineral hints. (Or import a real
   heightmap; see *DEM import* below.)
3. **Climate** — a zonal model: stellar flux sets the mean temperature, latitude
   and elevation shape it, rainfall belts and orographic rain shadows fall out,
   and every cell gets a Köppen-style biome.
4. **Hydrology** — D8 flow over the heightmap traces rivers (with Strahler
   order), pools lakes, delineates watersheds, and scores settlement sites.
5. **Demographics** — biome carrying-capacity feeds a rank-size hierarchy of
   cities, towns, and villages.

Pass `--json` to any layer to get the full structured output for scripting.

### Materializing into the World book

`--materialize` writes a layer into the **World** system book (a chapter per
layer) plus a `heightmap.png` asset — idempotent, so re-running updates in place:

```
$ inkhaven realworld compile --layer demographics --materialize
demographics · Aldoria …
  → World/Demographics: 2 paragraph(s) created, 0 updated
```

In the TUI, **`Ctrl+B W`** opens the World overview hub; **`C`** compiles *and*
materializes all five layers *and* seeds the proposal queue in one step.

### DEM import — compile a real place

To build the actual Earth (or any real region), give geology a grayscale
heightmap instead of generating one:

```hjson
geology: {
    dem: {
        path: "assets/earth_heightmap.png"
        scale_km_per_pixel: 50.0
        sea_level_pixel_value: 128
    }
}
```

Brighter pixels are higher ground; everything at or below `sea_level_pixel_value`
is ocean. The remaining four layers run unchanged on top of it.

## From settlements to Places: the proposal queue

The compiler never edits your manuscript silently. Its settlements become
**proposals** you accept or reject — and only an accepted proposal becomes a real
**Place** record, cross-referenced back to the world.

```
$ inkhaven realworld propose
proposed 20 Place(s) into the queue (0 already resolved, skipped)

$ inkhaven realworld proposals list
$ inkhaven realworld proposals accept-all      # or accept/reject <id> one by one
accepted 20 proposal(s)

$ inkhaven realworld places
  Tharliar   (108, 27) · tundra        · confluence     · pop 27,010
  …
```

Re-running `propose` never re-offers a site you already resolved. In the TUI,
**`Ctrl+B W` → `P`** opens the same queue with `Enter` to accept, `r` to reject.

These accepted Places are what the fact-checker resolves place names against
(Tutorial 77) — so the loop closes: **compile a world → accept its cities → write
about them → the checker knows what they are.**

## Declaring more: geography, hydrology, economy *(1.3.27+)*

The procedural layers give you a plausible world, but you'll want to name the
real things in *your* story. Four optional blocks let you assert them — they
materialize into a **Setting** chapter of the World book, and (for geography and
economy) they feed the fact-checker directly.

```hjson
geography: {
    regions: [
        { name: "The Sundered Coast", climate: "temperate", biome: "mediterranean",
          description: "Warm-summer headlands above a drowned valley." }
    ]
    landmarks: [
        { name: "Caer Dpath", kind: "city", climate_zone: "tundra",
          population: 27000, description: "The northern seat, walled against the ice." }
    ]
}

hydrology: {
    rainfall: "temperate"
    rivers: [ { name: "The Long Vail", description: "Runs the length of the central valley." } ]
    seas:   [ { name: "The Sundered Sea", description: "The enclosed inland water." } ]
}

economy: {
    tech_level: "medieval"
    currency: "silver mark"
    trade_goods: [ "wool", "salt", "iron", "amber" ]
    resources:   [ "iron", "copper", "tin", "silver" ]
}
```

Two of these do real work beyond documentation:

- A **landmark** with a `climate_zone` becomes a gazetteer entry the fact-checker
  resolves *by name* — so "snow fell on Caer Dath" can be flagged even before you
  compile and accept procedural Places.
- `economy.resources` and `geology.notable_minerals` join the checker's known
  goods, so a story that trades your world's ores isn't flagged as an economy
  contradiction.

You can also expand geology with descriptive knobs that materialize as setting
notes:

```hjson
geology: {
    generated: {
        plates: 7, continents: 7, mountain_orogeny: "active", sea_level: 0.40
        volcanism: "moderate"
        mineral_richness: "rich"
        notable_minerals: [ "iron", "copper", "tin", "gold", "silver", "coal" ]
    }
}
```

## Magic — the declared exceptions

If your world breaks physics on purpose, declare it in a `magic:` block so the
fact-checker respects the exception instead of flagging it forever. Each rule
names a `kind`, the check categories it `covers`, and who/where it applies. See
Tutorial 77 for the full treatment; `inkhaven realworld magic` lists the ledger.

## Start from Earth

The repository ships a complete, heavily-commented real-Earth definition at
[`examples/realworld/Earth.hjson`](../../examples/realworld/Earth.hjson): the
exact Sun/Earth/Moon, the Gregorian calendar, Earth-tuned procedural geology, and
populated geography/hydrology/economy blocks (the Sahara, Cairo, the Nile, an
industrial economy, Earth's ores). Copy it in as your `world.hjson` and compile
to start in a recognisable world, then edit toward your own.

## What you learned

- A world is one `world.hjson`: physics in, a coherent world out, reproducible
  from a seed.
- Five deterministic layers — astronomy → geology → climate → hydrology →
  demographics — each derived from the last; `compile --layer … --materialize`
  writes them into the World book.
- The compiler *proposes*; you accept settlements into real Place records, which
  close the loop with the fact-checker.
- Optional `geography` / `hydrology` / `economy` / expanded `geology` blocks let
  you name your world's real features — and feed the checker.
- `examples/realworld/Earth.hjson` is a ready-made starting point.

Next: **[Tutorial 77 — World maps and the fact-checker](77-world-maps-and-fact-checking.md)**.
Field-by-field reference: **[`../WORLDBUILDING.md`](../WORLDBUILDING.md)**.
