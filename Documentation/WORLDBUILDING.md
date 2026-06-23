# Worldbuilding reference (`realworld`)

The world-simulation system (RFC WORLD-4), introduced in **1.3.25** and continuing
across 1.3.x. You declare the **physics** of a world in one `world.hjson`; a
deterministic compiler derives the rest — astronomy, geology, climate, hydrology,
demographics — and materializes it into a **World** system book. A fact-checker
then reads your prose and flags claims that contradict the world you built.

This is the reference. For guided walkthroughs see the tutorials:
[76 — Building a world](Tutorials/76-building-a-world-realworld.md) and
[77 — World maps and the fact-checker](Tutorials/77-world-maps-and-fact-checking.md).
The design rationale is in [`PROPOSALS/WORLD-4_PLAN.md`](PROPOSALS/WORLD-4_PLAN.md).

> **Authoring tip.** `world.hjson` is HJSON: an unquoted string runs to
> end-of-line, so quote inline enum values (`class: "G2V"`, `kind: "city"`). Use
> multi-line blocks; a definition is opt-in — the system only activates when a
> project has a `world.hjson` at its root.

## The two halves

- **Branch A — the compiler.** `inkhaven realworld` turns a definition into a
  coherent world: five layers, a proposal queue that becomes Place records, and a
  rendered map.
- **Branch B — the fact-checker.** As you write, Inkhaven checks each paragraph
  against the world (travel, climate, demographics, astronomy, economy), in your
  language, respecting a declared magic ledger.

Everything in Branch A is a pure function of `(definition, seed)` — same inputs,
same world, every run (an in-tree SplitMix64 keyed by the seed; never the `rand`
crate). Astronomy is closed-form physics, re-asserted every run.

---

## The world definition

One file at the project root: `world.hjson`. Scaffold with `inkhaven realworld
new <name>`. Only `name` and `astronomy` are required; every other block is
optional with sensible defaults.

| Block | Required | Drives |
|---|---|---|
| `name` / `seed` / `primary_language` | name only | identity; the seed drives every procedural layer; the language sets fact-check rendering |
| `astronomy` | **yes** | the closed-form astronomy layer (seasons, tides, calendar) |
| `geology` | no | the geology layer — `generated` (procedural) or `dem` (real heightmap) |
| `geography` | no | named regions + landmarks → Setting chapter + the gazetteer |
| `hydrology` | no | named waters + rainfall → Setting chapter |
| `economy` | no | tech/currency/trade/resources → Setting chapter + the checker's known goods |
| `magic` | no | declared exceptions to physics the fact-checker respects |

### Top level

```hjson
{
    name: "Aldoria"
    seed: 0x1A2B3C            // integer, or a "0x…" hex string; drives all layers
    primary_language: "en"    // default "en"; sets the fact-checker's language
    astronomy: { … }
    geology:   { … }          // optional
    geography: { … }          // optional
    hydrology: { … }          // optional
    economy:   { … }          // optional
    magic:     { … }          // optional
}
```

### `astronomy` (required)

Closed-form physics — Kepler's year length, daily insolation by latitude band,
lunar synodic periods, tides, and a calendar-divergence check.

```hjson
astronomy: {
    star:   { class: "G2V", age_gyr: 4.6, luminosity_solar: 1.0, mass_solar: 1.0 }
    planet: { mass_earth: 1.0, radius_earth: 1.0, axial_tilt_deg: 23.4,
              day_length_hours: 24.0, rotation_direction: "prograde" }
    orbit:  { semi_major_axis_au: 1.0, eccentricity: 0.017, year_length_days: 365 }
    moons:  [ { name: "Luna", mass_lunar: 1.0, period_days: 27.32, eccentricity: 0.055 } ]
    calendar: {
        months: 12
        month_length_days: 30
        weekdays: 7
        month_names: [ … ]        // optional
        day_names:   [ … ]        // optional
        new_year_aligns_to: "winter_solstice"   // optional
    }
}
```

| Field | Default | Notes |
|---|---|---|
| `star.class` / `age_gyr` / `mass_solar` | — / 0 / derived | descriptive + mass for the year-length law |
| `star.luminosity_solar` | — | sets the climate's stellar flux |
| `planet.axial_tilt_deg` | — | the seasons |
| `planet.day_length_hours` | — | planet-day → calendar units |
| `planet.rotation_direction` | `"prograde"` | `prograde` / `retrograde` |
| `orbit.eccentricity` | `0` | |
| `orbit.year_length_days` | computed | a declared value is *checked* against Kepler's; a >1-day gap warns |
| `moons[].mass_lunar` / `eccentricity` | `0` / `0` | mass drives the tide share |
| `calendar.weekdays` | `0` | |

### `geology` — `generated` or `dem`

Either grow a world from the seed, **or** compile a real heightmap. Provide one
of the two sub-blocks.

```hjson
geology: {
    generated: {
        plates: 7                 // default 7
        continents: 7             // default 4
        mountain_orogeny: "active"   // "active" | "quiet" | "ancient"; default "active"
        sea_level: 0.40           // 0.0 (no oceans) … 1.0 (drowned); default 0.4
        // descriptive — materialize into the Setting chapter:
        volcanism: "moderate"        // "quiet" | "moderate" | "active"
        mineral_richness: "rich"     // "sparse" | "normal" | "rich"
        notable_minerals: [ "iron", "copper", "tin", "gold" ]  // join the economy check
    }
}
```

```hjson
geology: {
    dem: {
        path: "assets/earth_heightmap.png"   // grayscale; brighter = higher
        scale_km_per_pixel: 50.0             // default 5.0
        sea_level_pixel_value: 128           // pixels ≤ this are ocean
    }
}
```

`sea_level` is a **threshold on the generated heightmap, not a literal ocean
fraction** — the distribution isn't uniform, so ~0.40 gives an Earth-like
ocean-dominant world with continents large enough to be useful; raise it for a
wetter world, lower it for more land.

### `geography` — named regions + landmarks

```hjson
geography: {
    regions: [
        { name: "The Sundered Coast", biome: "mediterranean", climate: "temperate",
          description: "Warm-summer headlands above a drowned valley." }
    ]
    landmarks: [
        { name: "Caer Dath", kind: "city", climate_zone: "tundra",
          population: 27000, description: "The northern seat, walled against the ice." }
    ]
}
```

A **landmark with a `climate_zone`** becomes a gazetteer entry the fact-checker
resolves *by name* — even before you compile and accept procedural Places.
`kind` is free-form (`city` / `port` / `mountain` / …). Both blocks materialize
into the **Setting** chapter.

### `hydrology` — named waters

```hjson
hydrology: {
    rainfall: "temperate"        // "arid" | "temperate" | "wet" (descriptive)
    rivers: [ { name: "The Long Vail", description: "Runs the central valley." } ]
    lakes:  [ { name: "Mirrormere" } ]
    seas:   [ { name: "The Sundered Sea", description: "The enclosed inland water." } ]
}
```

Descriptive — the procedural hydrology layer still traces rivers from the
heightmap; this names the real ones for the Setting chapter.

### `economy`

```hjson
economy: {
    tech_level: "medieval"       // free-form: bronze | iron | medieval | industrial | …
    currency: "silver mark"
    trade_goods: [ "wool", "salt", "amber" ]
    resources:   [ "iron", "copper", "tin", "silver" ]   // join the economy check
}
```

`resources` (like `geology.notable_minerals`) extend the fact-checker's known
goods, so trading your world's materials isn't flagged.

### `magic` — declared exceptions

When your world breaks physics on purpose, declare it so the checker respects the
exception instead of flagging it forever.

```hjson
magic: {
    enabled: true
    rules: [
        {
            kind: "messenger_birds"
            covers: ["travel_time"]      // categories this rule excuses
            description: "Royal pelicans fly day and night with relays"
            applicable_to: { roles: ["royal_messenger"] }   // roles / regions / seasons
        }
    ]
}
```

Each rule names a `kind`, the categories it `covers`, an optional
`applicable_to` scope (`roles` / `regions` / `seasons`), and any extra parameters
(kept verbatim). The checker consults the ledger **lazily** — only after a
candidate warning — and a covered, applicable rule suppresses the finding with a
note rather than hiding it. `inkhaven realworld magic` lists it; it materializes
into `World / Magic Ledger`.

---

## The compiler

Five layers run in dependency order, each a pure function of the ones before it.

| Layer | Derives |
|---|---|
| **Astronomy** | Kepler year, daily insolation by latitude, lunar synodic periods, tides, calendar-divergence flag |
| **Geology** | seed → tectonic plates → procedural heightmap → continents, mountain ranges, mineral hints (or a DEM import) |
| **Climate** | zonal model: stellar-flux mean, latitude profile, elevation lapse, rainfall belts + orographic rain shadows → Köppen biomes + winds |
| **Hydrology** | D8 flow over the heightmap → rivers with Strahler order → lakes → watersheds → settlement priors |
| **Demographics** | biome carrying-capacity → a rank-size hierarchy of cities / towns / villages |

```
$ inkhaven realworld compile --layer demographics        # a human summary
$ inkhaven realworld compile --layer climate --json       # full structured output
$ inkhaven realworld compile --layer geology --materialize # write into the World book
```

`--layer` is one of `astronomy` (default) / `geology` / `climate` / `hydrology` /
`demographics`. In the TUI, **`Ctrl+B W` → `C`** compiles and materializes all
five **and** seeds the proposal queue in one step.

### What materializes

`--materialize` writes idempotently into the **World** system book (one chapter
per layer) plus assets:

| Chapter | Source |
|---|---|
| `Astronomy` / `Geology` / `Climate` / `Hydrology` / `Demographics` | the five layers |
| `Magic Ledger` | the `magic:` block |
| `Setting` | author-declared `geography` / `hydrology` / `economy` + expanded-geology notes |
| `assets/world/heightmap.png` | the normalized heightmap (8-bit grayscale) |

---

## Proposals & Places

The compiler never edits your manuscript silently. Its settlements become
**proposals** you accept or reject; only an accepted proposal becomes a real
**Place** record, with a **Place ↔ World cross-reference** recorded.

```
$ inkhaven realworld propose                 # seed the queue from demographics
$ inkhaven realworld proposals list
$ inkhaven realworld proposals accept-all    # or accept/reject <id>
$ inkhaven realworld places                  # the accepted cross-references
```

Re-running `propose` never re-offers a resolved site. These accepted Places are
what the fact-checker resolves place names against — closing the loop: **compile
→ accept cities → write → check.** TUI: **`Ctrl+B W` → `P`** (Enter accept, `r`
reject).

---

## Maps

`inkhaven realworld map` emits a **MapSpec v2** from the compiled layers and hands
it to [**plakat**](https://crates.io/crates/plakat), an external cartographer
(`cargo install plakat`). plakat loads the spec and skips its own AI, so the map
stays a pure function of the world + seed.

```
$ inkhaven realworld map
  features: assets/maps/world.features.png    # the rendered map
  geojson:  assets/maps/world.geojson         # coast / rivers / roads / landmarks
  spec:     assets/maps/world.mapspec.json    # the emitted MapSpec
```

Mountains come from clustering the heightfield's high cells; rivers run their real
D8 watercourse; landmarks are your largest settlements (coastal cities → ports).
plakat's resolved landmark positions are read back to **refine each accepted
Place's coordinates**.

| Flag / chord | Effect |
|---|---|
| `--spec-only` | write the MapSpec without invoking plakat |
| `--no-ingest` | render without refining Place coordinates |
| `Ctrl+B W` → `M` | the same, from the TUI |

plakat is **optional** — a missing binary degrades to a notice, never a failure.

---

## The fact-checker

When a project has a `world.hjson`, the **fast track** runs automatically: pause a
few seconds on a paragraph and any findings appear in the **Output pane** — no
chord, no focus stolen. A re-check replaces that paragraph's prior findings.

### Categories (fast track)

| Category | Flags |
|---|---|
| **travel_time** | a distance + duration in a sentence implying an impossible pace (pure prose) |
| **climate** | weather at a known place contradicting its climate zone |
| **demographics** | a population diverging sharply from the modeled figure |
| **astronomy** | a moon count disagreeing with the world's sky |
| **economy** | a metal mined/worked that the geology doesn't yield (extraction context only) |

Climate, demographics, and economy resolve names through the **gazetteer** — your
accepted Places plus any `geography.landmarks` you declared.

```
$ inkhaven fact-check --text "Snow fell on Cairo for three days."
⚠ [climate] Implausible: freezing weather at Cairo, whose climate zone is hot desert.
```

`--paragraph <id>` checks a stored paragraph instead of literal text.

### Scope (TUI)

**`Ctrl+B W` → `F`** arms a scope picker: **`P`** open paragraph · **`B`**
enclosing book · **`R`** the 12 most recently edited paragraphs.

### Languages & graceful degradation

Works in **English, Russian, Spanish, French, German** — detecting the
paragraph's language and rendering warnings in it. Place names resolve in their
**grammatical cases** (Russian `в Москве` matches `Москва`; German genitive).

Detection is a built-in heuristic that needs no model. When it isn't confident, it
**degrades** — rendering in English rather than guessing wrong — and never panics.
The fact-check footer names the active backend; `INKHAVEN_LANG_MODEL` points at an
optional enhanced parser, but nothing ever *requires* one.

### Slow track

`fact-check --slow` adds an LLM pass for the subtle contradictions patterns miss.
It's opt-in and cost-controlled:

- a **cost preflight** prints the estimated tokens and the day's call tally;
- a per-call **soft cap** (`--max-cost <tokens>`, default 6000; `--force`
  overrides) refuses an oversized call;
- a daily ceiling holds, and transient errors retry with backoff;
- a missing provider or a reached cap degrades to a notice.

An opt-in **idle auto** variant runs it in the background after ~45 s of quiet —
toggle with **`Ctrl+B W` → `S`** (off by default; it spends tokens).

### Coherence

`inkhaven realworld coherence <node>` checks a *run* of paragraphs **against each
other** — a character in two places without the travel between, a fact asserted
then reversed, a timeline that can't follow. Give it a book or chapter node id; it
gathers the paragraphs in document order and runs one cost-capped call, citing the
`¶` numbers. Honours the same `--max-cost` / `--force` and daily ceiling.

### Timeline-aware checking (WORLD-5)

When your project has a [timeline](Tutorials/31-story-timeline.md), the fast
checker learns *when* a paragraph happens — its events give the checker ground
truth instead of prose inference. A paragraph linked to a timeline event (or near
one in world-time) gains:

- **Calendar-grounded season** — weather that contradicts the dated season is a
  **contradiction** (snow in a paragraph the timeline places in summer), not a
  guess.
- **Event-derived travel time** — a prose "three days" that contradicts the gap
  between the traveller's events (say 35 days) is flagged.
- **`date_coherence`** — a seasonal date-hint in the prose (a *midsummer feast*, a
  *harvest*, a solstice) that contradicts the dated season.
- **`co_location`** — a character whose events place them in two *different* places
  at overlapping times. Check the whole timeline at once with `inkhaven realworld
  co-location`.

These run automatically (the same `Ctrl+B W → F` and the ambient check), in five
languages, and respect the **magic ledger** exactly like the world checks — a
`weather_control` or `teleportation` rule covering the category suppresses the
finding with a note. Timeline-derived findings carry a **📅** marker in the Output
pane. The CLI gates them with `fact-check --timeline-aware auto|on|off` (default
`auto`) and `--timeline-only`. Projects without a timeline are unaffected.

Bund scripts can query the same data read-only: `ink.world.fact_check.timeline.`
`events_near` / `events_for_character` / `events_for_place` / `season_for` /
`effective_date`.

> **Two timeline systems, for now.** Inkhaven's older timeline AI critique (1.2.6+,
> invoked from the swim-lane view) still runs unchanged; during this interim it and
> the timeline-aware fact-checker may both flag the same things. To avoid the
> overlap, rely on the fact-checker and simply don't invoke the legacy critique
> (it requires an explicit chord). A later RFC formally prunes the duplication.

---

## CLI reference

```
inkhaven realworld new <name> [--force]
inkhaven realworld validate
inkhaven realworld show [--json]
inkhaven realworld compile [--layer <layer>] [--json] [--materialize]
inkhaven realworld propose
inkhaven realworld proposals list [--status <s>]
inkhaven realworld proposals accept <id> | reject <id> | accept-all | clear
inkhaven realworld places
inkhaven realworld magic [--materialize]
inkhaven realworld map [--spec-only] [--no-ingest]
inkhaven realworld coherence <node> [--max-cost <n>] [--force]
inkhaven realworld co-location                       # WORLD-5: a character in two places at once
inkhaven fact-check (--text "…" | --paragraph <id>) [--slow] [--max-cost <n>] [--force]
                                                     # WORLD-5: [--timeline-aware auto|on|off] [--timeline-only]
```

## TUI reference — the World hub

| Chord | Action |
|---|---|
| `Ctrl+B W` | open the World overview |
| → `C` | compile + materialize all layers, seed the proposal queue |
| → `P` | open the Place proposal queue |
| → `F` → `P`/`B`/`R` | fact-check the paragraph / book / recent edits |
| → `M` | render the world map with plakat |
| → `S` | toggle the idle auto slow-check |

---

## Determinism & reproducibility

Every procedural layer is a pure function of the definition and the seed, via an
in-tree SplitMix64 (never the `rand` crate). Change the seed and the continents,
climate, rivers, and settlements all regenerate; keep it and the world — and its
map — are reproducible forever. The astronomy layer has no randomness at all.

## Start from Earth

The repository ships a complete, heavily-commented real-Earth definition at
[`examples/realworld/Earth.hjson`](../examples/realworld/Earth.hjson): the exact
Sun/Earth/Moon, the Gregorian calendar, Earth-tuned procedural geology, and
populated geography / hydrology / economy. Copy it in as `world.hjson` and
compile to start in a recognisable world, then edit toward your own.

## See also

- Tutorials [76 — Building a world](Tutorials/76-building-a-world-realworld.md),
  [77 — World maps and the fact-checker](Tutorials/77-world-maps-and-fact-checking.md)
- Design: [`PROPOSALS/WORLD-4_PLAN.md`](PROPOSALS/WORLD-4_PLAN.md)
- World *consistency* (the older Facts / drift / anachronism pillar, distinct from
  this simulator): [`Tutorials/69-world-consistency.md`](Tutorials/69-world-consistency.md)
- Example: [`examples/realworld/Earth.hjson`](../examples/realworld/Earth.hjson)
