# RESRCH-4 — Computational `/calc`: scientific words + World-book grounding (RFC)

| | |
|---|---|
| **Track** | RESRCH-4 (Computational research) |
| **Status** | R4-D + R4-B/A **shipped 1.5.4**; R4-C/E + reducers + route-2 open (formalizes [`RESRCH-4_TRACK.md`](RESRCH-4_TRACK.md)) |
| **Builds on** | RESRCH-3 / R3-C (`calc.*` words + `/calc`, shipped 1.5.2) · WORLD-4/5/6 (the **World** system book) |
| **New runtime crates** | **none** · **no network** — pure arithmetic over the in-tree Bund VM + the already-ambient `active_store()` reading already-on-disk World JSON |
| **Decisions to confirm** | (D1) World read route — **read the materialized book**, recompile fallback. (D2) reader surface — **one path accessor + a compact convenience set**. (D3) provenance — extend `computed` with a **`world:<path>`** citation, gate still bypassed. |

Turn `/calc` from a unit/constant calculator into a **computational research instrument**: a deeper
scientific word library (math, astronomy, planetology, climate, geography, economy) plus **reader words
that pull this project's own World-book facts into the computation**. Every result stays deterministic,
recorded `origin=computed`, fact-check gate bypassed — the computation is its own proof.

## Grounding (verified against the tree)

Nothing here invents infrastructure; the audited facts:

- **`calc.*` already exists** (`src/scripting/stdlib/calc.rs`, shipped 1.5.2): `kpush!` (push constant) /
  `kconv!` (pop→compute→push) macros, a `WORDS` table registered with short aliases, integer/float
  inputs coerced by `as_f64` (`cast_float().or_else(|_| cast_int().map(|i| i as f64))`), pure — no store,
  no network. New words extend this file/namespace with no structural change.
- **The store is ambient inside a `/calc` word.** `scripting::active_store() -> Option<&'static Store>`
  (`src/scripting/mod.rs:526`) is a process-global; `helpers::active_store(tag)` wraps it with a graceful
  "no project" error (`helpers.rs:62`). A `calc.*` word reads the store with **zero plumbing changes** —
  exactly how `world_timeline.rs` / `sources.rs` already do.
- **World facts are already on disk as JSON.** `src/world/materialize.rs` writes each compiled layer as
  `content_type = "hjson"` **paragraphs** under chapters — `World/Astronomy` (*System overview*,
  *Calendar*, *Celestial events*), `World/Geology`, `World/Climate`, `World/Hydrology`,
  `World/Demographics`, `World/Magic Ledger`. The pretty-printed JSON body is the on-disk source of truth.
- **The layer schemas are typed + round-trippable.** `AstronomyOutput` … `DemographicsOutput`
  (`src/world/types/`) derive `Deserialize` with documented units (e.g. `astronomy.rs`:
  `stellar_mass_solar`, `year_length_planet_days`, `axial_tilt_deg`, `insolation_bands[].annual_mean`,
  `moons[].synodic_period_planet_days`).
- **The reader precedent is `sources.rs`.** It finds a system book by tag → walks its subtree
  (`Hierarchy::collect_subtree`) → reads paragraph bodies from disk (`read_body`) → parses → pushes
  dicts. RESRCH-4's reader is the same shape against `SYSTEM_TAG_WORLD = "world"` (`src/store/mod.rs:99`;
  book predicate `kind==Book && system_tag==Some("world")`, `src/world/materialize.rs:366`).
- **The provenance `detail` field is already a free string.** `ProvMeta { origin, query, detail }`; the
  current `run_calc` records `origin="computed"`, `detail=String::new()`. A `world:<path>` citation needs
  **no schema change** — just populate `detail`.
- **The one real gap.** No code path deserializes the materialized `World/*` JSON back into the structs —
  every current consumer re-loads `world.hjson` and re-runs `compile_*` (`src/cli/realworld.rs`). R4-D
  supplies that reader, with recompilation as the fallback.

## D1 — World read route (the central decision)

`calc.world.*` resolves a fact in this order; it **never fabricates** a number:

1. **Read the materialized book** (primary — *"extract source data from World"*).
   `active_store()` → find the `world` book → resolve the path's chapter slug → iterate that chapter's
   `content_type=="hjson"` paragraph bodies → `serde_json` the one containing the requested key → index
   the remaining path → push the float. The book is the source of truth; no `world.hjson` required.
2. **Recompile fallback.** If the book isn't materialized (no `world` chapters yet) but `world.hjson`
   exists: `load(world.hjson)` + the relevant `compile_*` in memory (the path every current consumer
   uses), then index. Same numbers; covers the freshly-defined project.
3. **`NODATA`.** Neither available, or the path doesn't resolve → push `Value::nodata()` (the established
   sentinel) and say so on stdout. The author sees "no such World fact", never an invented value.

> Route 1 is what the user asked for; route 2 keeps the word useful before a `compile --materialize`;
> route 3 preserves the no-fabrication invariant.

## D2 — Reader surface: "predictable, compact, transparent"

A **path grammar** that mirrors the book's own layout, so what you type is where the fact lives:

```
<Chapter>/<field>[/<index-or-key>...]      paths are slug-insensitive on the chapter head
```

Resolution: the chapter head selects the `World/<Chapter>` subtree; `<field>` is matched against the
keys of that chapter's layer JSON (scanning its paragraphs — a chapter may split its JSON across
*System overview* / *Calendar* / … paragraphs); trailing segments index into lists/objects
(`zones/0/annual_mean_c`, `moons/Selene/synodic_period_planet_days`).

| Word | Stack | Behaviour |
|---|---|---|
| `calc.world.get` | `( path -- float \| NODATA )` | the generic accessor (route 1→2→3); echoes `world: <path> = <value>` on stdout |
| `calc.world.dict` | `( chapter -- dict \| NODATA )` | push a whole layer as a Bund **dict** (e.g. all of `Astronomy/System overview`) for multi-field formulas — mirrors `world_timeline.rs`'s `event_dict` |
| `calc.world.has` | `( path -- bool )` | true iff the path resolves (lets a script branch without erroring) |
| `calc.world.{year, tilt, day_hours, star_mass, star_lum, au, planet_mass, planet_radius, …}` | `( -- float )` | **convenience constants** — each is `calc.world.get "<fixed path>"`, registered like today's `kpush!` constants (e.g. `world.year` → `"Astronomy/year_length_planet_days"`, `world.star_mass` → `"Astronomy/stellar_mass_solar"`). A small, fixed set for the figures the formula words consume constantly; *not* a parallel API |

- **Predictable:** same path → same leaf, always; the path is the book's structure.
- **Compact:** one accessor + `dict`/`has` + a handful of convenience words — not a word per field.
- **Transparent:** every read echoes its source on `out.stdout` (which `/calc`'s `run_calc` already
  prints above the `=` line), and a derived `/fact` carries the citation (D3).

## D3 — Provenance: self-citing deterministic facts

A `/fact` taken from a World-grounded `/calc` records `origin="computed"` with
`detail = "world:<path>"` (multiple reads → comma-joined paths). The **gate stays bypassed** — the value
is deterministic and traceable to the book — but `/sources` now shows *which World facts* it came from.
This extends the RESRCH-3 trust ladder's deterministic tier into a **self-citing** one, and is the hook
for R4-E coherence checks. No `ProvMeta`/sidecar schema change (string `detail`).

## Phases

Each phase is independently shippable; R4-D is the keystone and the user's explicit ask.

| Phase | Content | Crates |
|---|---|---|
| **R4-D** | **World reader (keystone).** `world::calc_read` helper: book-by-tag → chapter → paragraph JSON → path index (route 1), `compile_*` fallback (route 2), `NODATA` (route 3). Bund words `calc.world.get` / `dict` / `has` + the convenience constant set, registered table+alias like `calc.rs`. stdout source echo. `run_calc` sets `detail="world:<path>"` on the resulting turn → provenance. Tests: path resolution (flat/indexed/missing), route-2 fallback, NODATA, provenance detail. **✅ Built 1.5.4** — `src/world/calc_read.rs` (route 1 + NODATA), `calc.world.{get,has,dict,year,declared_year,tilt,star_mass,orbit_days,divergence}`, source echo + `computed · world:<path>` provenance. **Route 2 shipped 1.5.6** — when the book isn't materialized, `recompile_chapter` loads `world.hjson` + re-runs the pure layer compilers (`compile_astronomy` … chaining upstream layers) and serializes the output, so the readers work pre-materialization. |
| **R4-B** | **Astronomy & planetology words** (pure, consume R4-D facts): `kepler_period` (a, M→T), `surface_gravity` (M,R→g), `escape_velocity`, `insolation` (L,d→flux, inverse-square), `synodic_period`, `angular_size`, `hill_sphere`, `roche_limit`, `tidal_accel`. Each documented with inputs/units; tested against known values (Earth/Sun) **and** against the project's own `Astronomy/*` (recompute vs declared). **✅ Built 1.5.4-dev** — all nine words in `calc.rs`, tested against Earth/Sun known values. | none |
| **R4-A** | **Scientific math substrate** — registered under `calc.<name>` (short alias best-effort, so the prefixed form never collides with bundcore): `sqrt cbrt pow exp ln log10 log2`, trig + inverse + `atan2`, `hypot floor ceil round abs`. **✅ Built 1.5.4** + `trunc sign factorial gcd lcm` (1.5.6) + **list reducers `sum mean min max` (1.5.6)** over a Bund list literal `[ 1 2 3 ] sum`. | none |
| **R4-C** | **Climate / geography / economy words.** Climate: `lapse_rate`, `dewpoint`, `insolation_at_lat`, `heat_index`. Geography: `haversine` (two lat/lon→great-circle km), `bearing`, `destination_point`, `slope`. Economy: `compound`, `cagr`, `inflation_adjust`, `annuity`, population `malthus` / `logistic`. **✅ Built 1.5.6-dev** — all in `calc.rs`, tested vs known values (London→NYC haversine, compound interest, logistic at t=0); completes R4-A scalar math (`trunc sign factorial gcd lcm`). *List reducers `sum mean min max` deferred (need Bund list-literal support); `area_quad` dropped.* | none |
| **R4-E** | **Cross-checking & coherence.** `calc.world.check <path> <computed>` → push the delta between a declared World fact and a `/calc`-recomputed value (e.g. `kepler_period` vs `Astronomy/year_length_planet_days`), surfacing the same divergence WORLD-4's `year_length_divergence_pct` reports — interactively, on any relationship. **✅ Built 1.5.6-dev** — `( path computed -- delta )`, echoes the declared value + `Δ`, NODATA when unresolved. | none |

## Worked examples

```
/calc "Astronomy/year_length_planet_days" world.get        → world: Astronomy/year_length_planet_days = 412.3
                                                              = 412.3
/calc world.star_mass world.au kepler_period               → = 411.8        (recompute this system's year)
/calc world.star_mass world.au kepler_period  world.year  - abs   → = 0.5    (computed vs declared divergence)
/calc 51.5 -0.13   40.7 -74.0   haversine                  → = 5570.2       (London→New York, km)
/calc "Demographics/settlements/0/population" world.get  0.012  100  logistic   → projected population
```
A `/fact` on any of these inserts `origin=computed`, `detail="world:…"`, gate bypassed.

## Formula reference (the A side, specified)

Bund is a stack language, so each word pops its arguments **in source order** (leftmost typed = deepest
on the stack) and pushes one result. Stack columns read `( deepest … top -- result )`. All inputs/outputs
are floats (integers coerced via `as_f64`); constants reuse the existing `calc.*` set (`calc.c`,
`calc.grav`, `calc.au`, `calc.gee`, …).

### R4-A — math substrate
Mostly thin wrappers over `f64` methods; register only those bundcore's stdlib lacks.

| Word | Stack | Definition |
|---|---|---|
| `sqrt` `cbrt` | `( x -- y )` | `x.sqrt()`, `x.cbrt()` |
| `pow` | `( base exp -- y )` | `base.powf(exp)` |
| `exp` `ln` `log10` `log2` | `( x -- y )` | natural/base exp & logs |
| `sin cos tan` | `( rad -- y )` | radians in (pair with `calc.deg2rad`) |
| `asin acos atan` | `( x -- rad )` | inverse, radians out |
| `atan2` | `( y x -- rad )` | `y.atan2(x)` (quadrant-correct) |
| `hypot` | `( a b -- c )` | `a.hypot(b)` = √(a²+b²) |
| `floor ceil round trunc` `abs sign` | `( x -- y )` | as named; `sign` → −1/0/+1 |
| `factorial` | `( n -- n! )` | Γ-free integer product |
| `gcd lcm` | `( a b -- y )` | Euclid; `lcm = a·b/gcd` |
| `sum mean min max` | `( list -- y )` | reduce a Bund list to a scalar |

### R4-B — astronomy & planetology
G = `calc.grav` = 6.674e-11; conveniences in solar/AU/Earth units to avoid huge numbers.

| Word | Stack | Formula | Units |
|---|---|---|---|
| `kepler_period` | `( a M -- T )` | SI: `T = 2π·√(a³/(G·M))` · solar/AU/yr: `T = √(a³/M)` | a [AU], M [M☉] → T [yr] |
| `surface_gravity` | `( M R -- g )` | `g = G·M/R²` · Earth units: `g = (M/M⊕)/(R/R⊕)²·g₀` | → g [m/s²] |
| `escape_velocity` | `( M R -- v )` | `v = √(2·G·M/R)` | → v [m/s] |
| `insolation` | `( L d -- S )` | `S = L/(4π·d²)` · solar/AU: `S = (L/L☉)/(d/AU)²·1361` | → S [W/m²] |
| `synodic_period` | `( T1 T2 -- Tsyn )` | `1/Tsyn = |1/T1 − 1/T2|` | same time unit |
| `angular_size` | `( size dist -- θ )` | `θ = 2·atan(size/(2·dist))` | → θ [rad] (pair `calc.rad2deg`) |
| `hill_sphere` | `( a e m M -- rH )` | `rH ≈ a·(1−e)·∛(m/(3M))` | a [any length] → rH [same] |
| `roche_limit` | `( R ρM ρm -- d )` | fluid: `d = 2.44·R·∛(ρM/ρm)` | R [length] → d [same] |
| `tidal_accel` | `( M r d -- a )` | `a = 2·G·M·r/d³` | → a [m/s²] |

### R4-C — climate, geography, economy

| Word | Stack | Formula | Notes |
|---|---|---|---|
| `lapse_rate` | `( Δh -- ΔT )` | `ΔT = −6.5·Δh/1000` | environmental Γ=6.5 K/km; Δh [m] |
| `dewpoint` | `( T RH -- Td )` | Magnus: `α = 17.625·T/(243.04+T)+ln(RH/100)`; `Td = 243.04·α/(17.625−α)` | T,Td [°C], RH [%] |
| `insolation_at_lat` | `( φ δ -- H )` | `h0 = acos(−tanφ·tanδ)`; `H = (1361/π)·(h0·sinφ·sinδ + cosφ·cosδ·sin h0)` | φ,δ [deg→rad]; δ from tilt·sin(year-fraction) |
| `heat_index` | `( T RH -- HI )` | NWS Rothfusz polynomial | T [°F], RH [%] |
| `haversine` | `( φ1 λ1 φ2 λ2 -- d )` | `a = sin²(Δφ/2)+cosφ1·cosφ2·sin²(Δλ/2)`; `d = 2R·atan2(√a,√(1−a))` | deg in; R=6371 → d [km] |
| `bearing` | `( φ1 λ1 φ2 λ2 -- θ )` | `θ = atan2(sinΔλ·cosφ2, cosφ1·sinφ2 − sinφ1·cosφ2·cosΔλ)` | → θ [deg, 0–360] |
| `destination_point` | `( φ1 λ1 θ d -- φ2 λ2 )` | `δ=d/R`; `φ2 = asin(sinφ1·cosδ+cosφ1·sinδ·cosθ)`; `λ2 = λ1+atan2(sinθ·sinδ·cosφ1, cosδ−sinφ1·sinφ2)` | **pushes two** values |
| `slope` | `( rise run -- grade )` | `grade = rise/run` (and `atan` for the angle) | unitless / rad |
| `compound` | `( P r n t -- A )` | `A = P·(1+r/n)^(n·t)` | r annual, n periods/yr, t yr |
| `cagr` | `( begin end yrs -- g )` | `g = (end/begin)^(1/yrs) − 1` | |
| `inflation_adjust` | `( nominal i yrs -- real )` | `real = nominal/(1+i)^yrs` | |
| `annuity` | `( PMT r n -- PV )` | `PV = PMT·(1−(1+r)^−n)/r` | |
| `malthus` | `( N0 r t -- N )` | `N = N0·e^(r·t)` | exponential growth |
| `logistic` | `( N0 K r t -- N )` | `N = K/(1 + ((K−N0)/N0)·e^(−r·t))` | carrying-capacity K |

### A × B — formulas that consume World facts (the integration, concretely)
The reader words feed the formula words directly; this is what "both A and B" buys:

```
year       = /calc world.star_mass world.au kepler_period      # R4-B ← R4-D (Astronomy)
gravity    = /calc world.planet_mass world.planet_radius surface_gravity
flux       = /calc world.star_lum world.au insolation          # → habitability check
pop_2100   = /calc "Demographics/settlements/0/population" world.get  K  r  t  logistic   # R4-C ← R4-D
divergence = /calc world.star_mass world.au kepler_period  world.year  -  abs            # R4-E
```
Each line is deterministic, records `origin=computed` + `detail="world:…"`, and bypasses the gate.

## Notes & limits
- **Read-only.** World words only read the book (the `store_read` posture of `ink.*`/`world_timeline.*`)
  — no materialize, no proposal queue, no writes.
- **No project / no World book** → `active_store()` is `None` or the book is empty → `NODATA` + a clear
  stdout note; `/calc` pure-math words keep working (they never touch the store).
- **Path stability.** Paths track the materialized chapter/field names; if WORLD-4 renames a layer field,
  the convenience-word fixed paths move with it (a single table in `calc.rs`). The generic `get` is
  always available as the escape hatch.
- **Integer/float interchangeable** for every new numeric word (the 1.5.2 `as_f64` rule).
- **Language-agnostic** — pure computation carries no prose, so no per-language prompt surface.

## Out of scope (later)
- Writing/feeding values *back* into the World book (RESRCH-4 is read-only).
- Symbolic algebra / units-as-types (Bund quantities carry a unit tag — `dt=CALL` with `q` — but
  unit-checked arithmetic is a separate, larger effort).
- Reading the magic ledger's rule semantics (only its numeric fields are in scope).

---

## Implementation path across RESRCH-2 / 3 / 4

RESRCH-4 doesn't live alone — three tracks are part-shipped and share one corpus + one provenance/trust
model. Current status (audited):

| Track | Shipped | Open |
|---|---|---|
| **RESRCH-2** (Grounded Research) | R2-A provenance, R2-B import, R2-C web, R2-D `/promote` (1.5.1–1.5.2) | **R2-E** trust/hygiene (cost table, chunked `/factcheck`, streamed extraction/factcheck, tab-completion — dedup already shipped) · **R2-F** batch/headless |
| **RESRCH-3** (Authoritative Sources) | R3-C `/calc`, R3-D folder/vault import (1.5.2) | **R3-A** `/wikidata` · **R3-B** `/openalex` `/arxiv` + SOURCES-1 auto-cite · R3-C `/world` (display) · R3-D Zotero/BibTeX + folder-watch · **R3-E** triangulation |
| **RESRCH-4** (Computational `/calc`) | — | **R4-A..E** (this RFC) |

### The through-line: the trust ladder is the spine

Every open item slots onto the same ladder (RESRCH-3's organizing idea), which decides each source's
factcheck-gate posture:

```
deterministic / structured   →  scholarly (DOI/ID)  →  web prose  →  model
  R4 (computed, World)            R3-B (OpenAlex/arXiv)   R2-C (web)    base
  R3-A (Wikidata triples)         gate relaxed            gate          gate
  gate bypassed / citation-only
```

So the natural build order is **highest-verifiability-first**, because each new deterministic/structured
source strengthens R3-E triangulation (the cross-check that finally replaces model self-grading), and
because R2-E's streamed-display + real cost model are debt that *every* later source would otherwise
inherit. Concretely:

### Recommended cut sequence

1. **1.5.3 — R2-E hygiene (foundation).** *No new crates.* Real per-model cost table, **chunked
   `/factcheck`**, **streamed extraction/factcheck** (reuse the chat stream path), tab-completion on
   `/goto` + `→ path`. Pay the debt before widening surface; every later source displays/streams/cost-
   reports correctly for free. (RESRCH-2's own recommended next.)
2. **1.5.4 — RESRCH-4 first cut: R4-D + R4-B.** *No new crates, no network.* The World reader keystone +
   astronomy words — the zero-dependency, un-fabricatable, ethos-fit win that needs only what's already
   in-tree. Lands while the cost/stream work is fresh. (Math substrate R4-A rides along as R4-B needs it.)
3. **1.5.5 — R3-A `/wikidata`.** *No new crates* (reuse `reqwest`, keyless). Top of the trust ladder, the
   first *external* structured source; gate bypassed by Q-ID. Now the corpus has **two** deterministic
   sources (World + Wikidata) → R3-E triangulation becomes possible.
4. **1.5.6 — R3-B `/openalex` + `/arxiv` + SOURCES-1 auto-cite.** *No new crates* (keyless). Scholarly
   tier; a `/fact` from a paper auto-creates a `BibEntry` (`src/sources/`) so facts and bibliography land
   together. Relaxed gate.
5. **1.5.7 — RESRCH-4 R4-C + R4-E.** Climate/geography/economy words + the World coherence check — now
   that World reading is mature, the author-facing breadth (`haversine`, population models) and the
   `/calc`-as-spot-checker.
6. **1.5.8 — R3-E triangulation + R3-D Zotero/BibTeX + folder-watch.** With ≥2 structured sources live,
   fold multi-source agreement into the WC-P3 confirmation gate ("3/3 sources agree" replaces the model
   grading itself); ingest the author's curated library; folder-watch re-imports on change.
7. **Later — R2-F batch/headless.** Seed a corpus from a question list non-interactively (auto-confirm
   only under explicit flag + threshold) — naturally last, once every source + the trust gate exist.

### Why this order (the brainstorm rationale)
- **Debt before breadth (1).** Streamed display + a real cost model are load-bearing for *every* source;
  building them first means R3-A/B and R4 inherit them, not the reverse.
- **Zero-dependency wins early (2).** R4-D/B add the most capability per unit risk — no crates, no
  network, no fabrication possible — so they're the safest large step and a strong demo.
- **Structured-before-prose (3→4).** Wikidata/OpenAlex/arXiv are higher on the ladder than web prose;
  building them next maximizes how often the gate can be *skipped or relaxed*, and seeds triangulation.
- **Triangulation needs a quorum (6).** R3-E only pays off once multiple independent structured sources
  exist (World + Wikidata + OpenAlex) — so it lands after them, turning the gate from self-grading into
  cross-source agreement: the strongest trust posture the whole program can reach.
- **Batch last (7).** Headless auto-confirm is only safe once the gate it relies on is at its strongest.

This keeps each release **one coherent step**, mostly crate-free, with the dependency/network surface
(only `reqwest`, already present) introduced exactly where the ladder demands it — and it routes the
whole program toward the same destination: facts that are **cited, cross-checked, and computed**, not
asserted.
