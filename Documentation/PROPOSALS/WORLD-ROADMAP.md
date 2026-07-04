# WORLD — Roadmap (forward backlog for the World Simulation)

| | |
|---|---|
| **Status** | Shipped in 1.6.0 — WORLD-8 (History, incl. migrations/polity arcs), WORLD-9 (Polities + Culture), Ecology, and WORLD-10 (Weather, Travel, **Scene world-context**) all landed. The three dimensions — Time, Peoples, the Desk — are now real. Remaining: nearest-features proximity in the scene brief; time-evolution ("run the clock") as a stretch. |
| **Builds on** | WORLD-4 (layered sim), WORLD-6 (utopia coherence), WORLD-7 (unify / surface / bridge / cleanup) |
| **Organizing insight** | The sim today builds a **place** — a present-day physical + demographic snapshot. It does not yet have a **time** (history), a **people** (culture / polities), or a **presence at the desk** (integration into the act of writing). Those three dimensions are where the remaining value is. |

## Where the sim stands (grounding)

After WORLD-7 the subsystem is coherent end-to-end and the surface is complete for what it models:

- **Procedural physics (emergent), five layers:** `astronomy → geology → climate → hydrology → demographics`. The author fully controls **astronomy** (star / planet / orbit / moons / calendar) and **geology** (seed *or* an external DEM); **climate, hydrology, demographics emerge** from the physics + seed. Each compiles + materialises into the World book and is tested.
- **Author-declared world (descriptive), all wired to the gazetteer + fact-checker + materialisation:** `geography` (regions + landmarks), `hydrology` (named rivers / lakes / seas), `economy` (tech level / currency / trade / resources), `magic` (a `MagicLedger` of declared exceptions to physics).
- **Workflow:** `realworld new → edit world.hjson → validate → compile --materialize → proposals/accept (settlements → Places) → map (plakat) → calendar (→ Timeline) → fact-check`. Plus `Ctrl+B W` overview (read-only), WORLD-6 coherence, co-location, drift, world_report.

Everything below is **new capability**, not repair — no landmines remain (WORLD-7 retired the `dead_code` blanket).

---

## Dimension 1 — TIME (the biggest gap)

The sim is a static present-day snapshot; it has no past.

| Track | What it adds | Builds on | Value / risk |
|---|---|---|---|
| **WORLD-8 · History & Chronology** | A deterministic 6th layer that generates the world's past from `(demographics + geography + seed)`: settlement founding order, migrations, the rise and fall of polities, major events — **materialised as Timeline events**. | demographics settlements, the WORLD-7 (W7-P3) calendar bridge | **High** / Med — the single most impactful missing piece for fiction. The "founding-date events" deferred in W7-P3 are a subset of this. **W8-P1+P2 shipped**: `compile_history` (founding chronology + epochs) + `realworld history [--json] [--materialize]` — prints it, emits an adoptable `inkhaven event add` block, and writes a **History chapter** into the World book (also folded into the whole-world `compile --materialize`). **Open**: migrations / polity rise-and-fall as richer generated events. |
| **Time evolution ("run the clock")** | Advance the world N years: population growth, climate drift, tech advance. Snapshot → dynamic. | all layers | High ambition / **High** — a stretch; not near-term. |

## Dimension 2 — PEOPLES (ties to ConLang)

The sim produces settlements + biomes but stops short of *who lives there*.

| Track | What it adds | Builds on | Value / risk |
|---|---|---|---|
| **WORLD-9 · Culture & Society** | Derive cultures / religions / naming conventions per region from demographics + biomes, and **assign each culture a language** — wiring the World sim to the **ConLang** flagship (region X speaks conlang Y). | demographics, climate biomes, the ConLang suite | **✅ Shipped** — `realworld culture` / `compile_culture`: one culture per polity (biome-derived ethos, seeded belief, a conlang **language profile** to realise, a naming sample). The profile is proposed; the author realises it with `inkhaven language`. |
| **Polities layer** | Aggregate settlements into nations with borders, capitals, and relations → feeds maps + conflict / tension. | demographics, map coordinates | **✅ Shipped** — `realworld polities` / `compile_polities`: settlements cluster around the largest capitals into named realms with populations + seeded relations. |
| **Ecology / flora-fauna** | Populate each biome with plausible species. | climate biomes | **✅ Shipped** — `realworld ecology` / `compile_ecology`: flora/fauna archetypes + a keystone per land biome, seed-rotated. |

## Dimension 3 — THE DESK (the writing payoff)

The sim is compiled and materialised, but it does not yet *show up while you write*. This is where an author feels it.

| Track | What it adds | Builds on | Value / risk |
|---|---|---|---|
| **WORLD-10 · Scene world-context** | For the open scene's place + Timeline date, surface the relevant world facts (season, weather, local culture) as ambient writing context. | place/gazetteer, calendar bridge, climate / astronomy | **✅ Shipped** — `realworld scene` (the composition) + the in-editor surface: an ambient **footer chip** (self-gating, `tick_scene_context`) and a **"This scene"** header in the `Ctrl+B W` overview. Place/date resolve from a place-linked Timeline event, then a paragraph link to a Place (no wiki-links). See [WORLD-10-SCENE_PLAN.md](WORLD-10-SCENE_PLAN.md). |
| **Travel / distance continuity checker** | The map carries coordinates + terrain; check prose travel claims ("rode A→B in a day") against real distance / terrain. | map `MapSpec` coords, geology / hydrology | **✅ Shipped** — `realworld travel` / `src/world/travel.rs`: real distance from planet + grid vs. the mode's pace; consults the magic ledger's `travel_time`. (Coord inputs; place-name resolution is a follow-up.) |
| **Weather / season at a date** | Given a scene's date + latitude, compute the season / insolation (astronomy already models this). | astronomy seasons, climate | **✅ Shipped** — `realworld weather` / `src/world/weather.rs`: hemisphere-corrected season + interpolated insolation for a day-of-year + latitude. |

---

## Depth & polish (smaller, bounded)

- **W7-P4 · Magic compile / validation pass** — the one place the model reads "declared but not generated": validate the `MagicLedger` for internal consistency and derive physics implications. *(Flagged as strengthening a "Building the World" book — the current model reads cleanly everywhere else.)*
- **World bible / gazetteer export** — auto-generate a consolidated world-reference appendix (all places, rivers, regions, calendar) for the manuscript. Low effort, high author value.
- **In-TUI world editor** — structured editing of `world.hjson` (the `Ctrl+B W` overview is read-only today).
- **Richer maps** — labelled / political / climate map overlays, exported as book figures. (Today `map --spec-only` always emits the `MapSpec` JSON; the rendered PNG needs the external `plakat` tool.)
- **World variants** — generate N candidate worlds from different seeds, pick one.

---

## Recommended sequence

1. **W7-P4 magic pass + gazetteer export** — cheap, and they round out the *current* surface. Best done **before** a "Building the World" book so the model reads "generated" everywhere and the book can point at a one-command reference export.
2. **WORLD-8 · History** — the highest-impact missing dimension (time); directly extends the calendar / Timeline work already shipped.
3. **WORLD-10 · Scene context + travel checker** — the writing payoff; makes the whole sim felt at the desk.
4. **WORLD-9 · Peoples** — the flagship-connecting track (World × ConLang), once history + context exist.

The three big tracks map cleanly to **"give the world a past, a people, and a presence while you write."**

## Relationship to a "Building the World" book

The subsystem is already book-ready on its current surface (the honest scope notes — magic is declared-not-generated, climate/hydrology/demographics emerge rather than being authored, World→Timeline is calendar-only, no in-TUI editor — are framing, not gaps). The **W7-P4 magic pass** and the **gazetteer export** are the two items that would most improve such a book; the three big tracks (8/9/10) are future editions, not prerequisites.
