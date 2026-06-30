# RESRCH-4 — Computational `/calc`: scientific words + World-book grounding (track proposal)

| | |
|---|---|
| **Status** | Proposed (track) |
| **Builds on** | RESRCH-3 / R3-C (`/calc` + the `calc.*` Bund words, shipped 1.5.2) · WORLD-4/5/6 (the **World** system book) |
| **Theme** | Turn `/calc` from a *unit/constant calculator* into a *computational research instrument*: (1) a deeper scientific word library (advanced math, astronomy, planetology, climate, geography, economy) and (2) **reader words that pull a project's own World-book facts into the computation** — so a `/calc` is grounded in *this* world, not generic constants. Every result stays deterministic, `origin=computed`, gate-bypassed. |

## Why this is the right next `/calc` step

1.5.2 shipped `calc.*` as **pure** words — constants + unit conversions, no store, no network (`src/scripting/stdlib/calc.rs`). That proved the model: `kpush!`/`kconv!` macros, a `WORDS` table with short aliases, integer/float-interchangeable inputs via `as_f64`, and a `/fact` from a `/calc` recorded as `origin=computed` with the **gate bypassed** (the computation *is* its proof). RESRCH-4 keeps that ethos and extends it along two independent axes that compose:

- **Axis A — richer math** the author actually needs for worldbuilding figures (orbital periods, surface gravity, great-circle distances, population growth, compound economics).
- **Axis B — ground the math in the World book.** The project already *has* a simulated world; `/calc` should be able to read its facts (year length, axial tilt, stellar mass, climate bands, settlement populations) and compute *on them*, transparently.

The two axes are the difference between `/calc 1.0 0.5 kepler_period` (generic) and `/calc world.au kepler_period` (this world's orbit). Axis B is what the user's request — *"extract source data from 'World' for /calc in a predictable, compact and transparent way"* — is about.

## The grounded reality (what already exists — audited)

This proposal touches no fabricated infrastructure. The relevant pieces already in-tree:

- **The World system book** — `SYSTEM_TAG_WORLD = "world"` (`src/store/mod.rs:99`), identified by the predicate `kind == NodeKind::Book && system_tag == Some("world")` (`src/world/materialize.rs:366`). Seeded on every project open by `ensure_system_books` (`src/store/mod.rs:536`).
- **Its facts are already on disk as JSON.** `src/world/materialize.rs` writes each compiled layer as `content_type = "hjson"` **paragraphs** under chapters: `World/Astronomy` (*System overview*, *Calendar*, *Celestial events*), `World/Geology`, `World/Climate`, `World/Hydrology`, `World/Demographics`, `World/Magic Ledger`. The pretty-printed JSON body is the on-disk source of truth.
- **The schemas are typed and round-trippable.** `AstronomyOutput`, `ClimateOutput`, `GeologyOutput`, `HydrologyOutput`, `DemographicsOutput` (in `src/world/types/`) all derive `Deserialize`, with documented units (e.g. `astronomy.rs`: `stellar_mass_solar`, `orbital_period_days_earth`, `year_length_planet_days`, `axial_tilt_deg`, `insolation_bands[].annual_mean`, `moons[].synodic_period_planet_days`).
- **The store is ambient inside a `/calc` word.** `scripting::active_store() -> Option<&'static Store>` is a process-global (`src/scripting/mod.rs:526`); `helpers::active_store(tag)` wraps it with a graceful "no project" error. A `calc.*` word can read the World book with **zero plumbing changes** — exactly how `world_timeline.rs` and `sources.rs` already work.
- **The exact reader precedent already exists.** `src/scripting/stdlib/sources.rs` reads a *system book by tag* → walks its subtree (`Hierarchy::collect_subtree`) → reads paragraph bodies from disk (`read_body`) → parses → pushes dicts. RESRCH-4's World reader is the same shape against `SYSTEM_TAG_WORLD`.

**The one real gap:** no code path deserializes the materialized `World/*` JSON paragraphs back into the `*Output` structs — every current consumer instead re-loads `world.hjson` and re-runs `compile_*` (`src/cli/realworld.rs`). RESRCH-4 supplies that reader (and can fall back to recompilation when the book isn't materialized).

## The model — one namespace, two word families

Everything lands under the existing `calc.*` namespace (short aliases as today), so `/calc` is the single surface:

```
calc.<scientific>     pure compute words   (Axis A) — stack in, stack out, no store
calc.world.<reader>   World-book accessors  (Axis B) — read the World book, push a number/dict
```

### Reader design — "predictable, compact, transparent" (the user's three asks)

- **Predictable** — one **path-addressed** accessor whose path mirrors the World book's own layout, so what you type is exactly where the fact lives:
  ```
  /calc "Astronomy/year_length_planet_days" world.get     → = 412.3
  /calc "Climate/zones/0/annual_mean_c"     world.get     → = 18.7
  ```
  The path is `Chapter/field` (dotted/sliced into the layer JSON). No hidden lookups; the same path always resolves to the same leaf.
- **Compact** — a **small** vocabulary, not hundreds of words: the generic `calc.world.get` (string path → float) plus a handful of convenience constant-words for the figures used constantly, registered exactly like today's `kpush!` constants:
  ```
  calc.world.year      calc.world.tilt      calc.world.gravity
  calc.world.star_mass calc.world.au        calc.world.day_hours
  ```
  Each is `calc.world.get "<their fixed path>"` under the hood — convenience, not a parallel API.
- **Transparent** — every read **echoes its source** on `out.stdout` (which `/calc` already surfaces above the `=` line): `world: Astronomy/year_length_planet_days = 412.3`. A `/fact` taken from a World-grounded `/calc` records `origin=computed` **with a `world:<path>` detail** in its provenance, so the corpus shows the figure was *computed from this project's World book*, not invented. This extends the existing `computed` posture (gate bypassed) with a citation — the deterministic tier of the RESRCH-3 trust ladder, now self-citing.

### Read route (the key decision)

- **Primary: read the materialized World book** (route 1) — `active_store()` → find the `world` book → walk the chapter whose slug matches the path head → read the paragraph JSON body → `serde` into the layer struct (or index the path) → push the field. This is *literally* "extract source data from World", uses the book as the source of truth, and needs no `world.hjson` on disk.
- **Fallback: recompile** (route 2) — if the book isn't materialized yet, `load(world.hjson)` + `compile_*` in memory (the path every current consumer uses). Same numbers; covers the not-yet-materialized project. A `world.get` on a missing book with no `world.hjson` pushes `NODATA` (the established sentinel) and the word's stdout says so — never a fabricated value.

## Phases

### R4-A — Scientific math words (Axis A, pure; zero new crates, no network)

Extend `calc.*` with the functions worldbuilding figures need, as `kconv!`-style words (and a few binary/list variants). **Check bundcore's vanilla stdlib first** — register only what core lacks, to avoid shadowing (`mod.rs` already documents ordering hazards).
- **Math**: `sqrt cbrt pow exp ln log10 log2`, trig + inverse (`sin cos tan asin acos atan atan2`), hyperbolic, `hypot`, `floor ceil round trunc`, `abs sign`, `factorial gcd lcm`, and list reducers `sum mean min max` (read a Bund list, push a scalar).
- These are the substrate the domain words below build on.

### R4-B — Astronomy & planetology words (Axis A; consume World facts via Axis B)

Deterministic formulas, each documented with its inputs/units:
- `kepler_period` (semi-major axis + total mass → orbital period), `surface_gravity` (mass, radius → g), `escape_velocity`, `insolation` (luminosity, distance → flux, inverse-square), `synodic_period`, `angular_size`, `hill_sphere`, `roche_limit`, `tidal_accel`.
- Pairs naturally with R4-D: `/calc world.star_mass world.au kepler_period` computes *this* system's year from *its* World facts and can be cross-checked against `Astronomy/year_length_planet_days`.

### R4-C — Climate, geography & economy words (Axis A)

- **Climate**: `lapse_rate` (altitude → ΔT), `dewpoint`, `insolation_at_lat` (latitude, tilt → seasonal insolation — mirrors `InsolationBand`), `heat_index`.
- **Geography**: `haversine` (two lat/lon → great-circle km — the headline "how far is it" word), `bearing`, `destination_point`, `area_quad`, `slope` (rise/run → grade & angle).
- **Economy**: `compound` (principal, rate, periods), `cagr`, `inflation_adjust`, `annuity`, and population models `malthus` (exponential) / `logistic` (carrying-capacity) — the last two consume `World/Demographics` populations via R4-D.

### R4-D — World-book reader words (Axis B; zero new crates, no network) — **the keystone**

The reader described above:
- `calc.world.get <path>` — generic path-addressed float accessor (route 1 → route 2 fallback → `NODATA`).
- A compact convenience set (`calc.world.{year, tilt, gravity, star_mass, au, day_hours, …}`) registered as fixed-path wrappers.
- `calc.world.dict <chapter>` — push a whole layer as a Bund **dict** (e.g. all of `Astronomy/System overview`) for words that want several fields, mirroring `world_timeline.rs`'s `event_dict` shape — so a single read can feed a multi-input formula.
- Source echo on stdout + `world:<path>` provenance detail on a derived `/fact`.

### R4-E — Cross-checking & coherence (folds into WORLD-4)

Because R4-B can *recompute* what WORLD-4 already materialized, `/calc` becomes a spot-checker: `/calc world.star_mass world.au kepler_period  world.year  -  abs` surfaces the divergence between the computed and declared year. This is the same divergence WORLD-4's `year_length_divergence_pct` reports — `/calc` lets the author probe any such relationship interactively. (Optional: a `calc.world.check` word that returns the delta directly.)

## Dependency & safety posture

- **Zero new crates, zero network** across the whole track — pure arithmetic (Axis A) + the already-ambient `active_store()` reading the already-on-disk World JSON (Axis B). Backed entirely by the in-tree Bund VM (`rust_multistackvm`) reused from 1.5.2.
- **Read-only** — World reader words only read the book (the `store_read` posture of `ink.*`/`world_timeline.*`); no materialize, no proposal queue, no writes.
- **No fabrication** — a missing/unmaterialized World fact pushes `NODATA`, never a guessed number; the source is always echoed; provenance stays `computed` + `world:<path>`.
- **Integer/float interchangeable** everywhere (the 1.5.2 `as_f64` rule extends to every new numeric word).
- **Language-agnostic** — pure computation carries no prose, so no per-language prompts are involved (unlike the LLM features).

## Recommended order
1. **R4-D World reader** — the keystone and the user's explicit ask; small, high-leverage, makes every later word world-aware. Build the `sources.rs`-shaped reader + `calc.world.get` + the convenience set first.
2. **R4-B astronomy/planetology** — the most compelling demo (`/calc world.star_mass world.au kepler_period`) and the natural consumer of R4-D.
3. **R4-A math substrate** — fill in as R4-B/C need it (sqrt/pow/trig land with the first formula that uses them).
4. **R4-C climate/geography/economy** — `haversine` and the population models are the headline author-facing wins.
5. **R4-E cross-checking** — once recomputation exists, expose the coherence delta.

## Relationship to the other tracks
- **Extends RESRCH-3 / R3-C.** R3-C proposed `/world <query>` to *surface* the simulation as a read-only fact source; RESRCH-4 is the deeper form — *compute on* World facts, not just display them. The two share `origin=computed`/`simulation`, gate-bypassed.
- **Reuses WORLD-4's materialized book** without touching the simulation; supplies the missing typed reader of the materialized JSON that WORLD-4 itself never needed.
- **RESRCH-2's hygiene pass (R2-E)** remains the recommended next step *within RESRCH-2* before broad new surface; RESRCH-4 is a self-contained, zero-dependency extension that can slot in whenever `/calc` is the focus.
