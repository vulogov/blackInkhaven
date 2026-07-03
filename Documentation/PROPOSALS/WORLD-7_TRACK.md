# WORLD-7 — Finishing the World Simulation (track proposal)

| | |
|---|---|
| **Status** | Proposed (track) · headline of the 1.6.0 "Living World" cycle |
| **Builds on** | WORLD-4 (the layered simulation), WORLD-6 (utopia coherence), RESRCH-3/4 (`/world`, `/calc` world reads) |
| **Theme** | The World Simulation is **built but not finished-off**. All five physical layers compile, all materialize, the map and Places bridge exist — but there is no single "compile my world" command, the docs lie about what's wired, and the non-astronomy layers are invisible in the TUI. WORLD-7 unifies, surfaces, and documents the last mile, then deepens the world→prose bridge. |

## Grounding (verified against the code, 1.6.0-dev)

The earlier assumption that "only astronomy is wired" is a **stale comment**, not reality:

- **All five layer compile functions exist and are tested** (`src/world/compile/`):
  `compile_astronomy(&def.astronomy)`, `compile_geology(&def)` / `compile_geology_dem`,
  `compile_climate(&def, &astro, &geo)`, `compile_hydrology(&geo, &climate)`,
  `compile_demographics(&climate, &hydro)` — each with passing `#[cfg(test)]` tests.
- **All materialize functions exist** (`src/world/materialize.rs`):
  `materialize_{astronomy, geology, climate, hydrology, demographics, magic, setting}`.
- **On-demand reads already run the full chain** — `calc_read::recompile_chapter` compiles
  any layer from `world.hjson` for `/world` and `/calc` (route-2 fallback).
- **Adjacent pieces are built**: the plakat **Map** (`realworld map`, compiles every layer →
  `MapSpec`), the **Places** bridge (`realworld places`), **Magic** (`realworld magic`, from the
  author's `MagicLedger`), **Coherence** (WORLD-6), **CoLocation** (timeline), **Proposals**
  (author-accept flow), and the fast/slow **fact-checker**.

**The actual gaps:**

1. `realworld compile` takes `--layer` (default `astronomy`); a *specific* layer materializes
   only its own chapter. There is **no single command that compiles + materializes the whole
   world** (all five chapters + the author-declared Setting) in dependency order.
2. `starter_template` (`src/cli/realworld.rs:1242`) and the `src/world/mod.rs` P0 doc comment
   still say "geology / climate / hydrology / demographics / magic … accepted-and-ignored" — **false**.
3. The **`Ctrl+B W` World overview** (TUI) shows the astronomy layer only; the compiled
   geology/climate/hydrology/demographics are not surfaced there.
4. The world→prose bridge (`realworld places`) materializes settlements to Places, but is a
   one-shot CLI; it is not refreshed/surfaced interactively, and demographics→Timeline is unbuilt.

## Phases

| Phase | Content |
|---|---|
| **W7-P1 — One-command full-world compile** | `realworld compile` with no `--layer` (or `--layer all`) runs the full chain once and **materializes every chapter** — Astronomy → Geology → Climate → Hydrology → Demographics, plus the author-declared **Setting** — in dependency order, reporting per-chapter paragraph counts. Reuses the existing `compile_*` + `materialize_*` fns (pure orchestration). Fix the stale starter-template + `mod.rs` comments. **This is the headline UX win: "compile my world" becomes one command.** |
| **W7-P2 — Surface every layer** | The `Ctrl+B W` World overview shows the compiled layers that exist in the World book (not just astronomy): a compact summary per materialized chapter, with a "not yet compiled — run `realworld compile`" hint for absent ones. Optionally a `/world` completeness line in the Research Assistant. **✅ Shipped** — the overview now compiles the chain live and shows a summary line per layer (geology plates/continents/ocean%, climate temp/precip/biomes, hydrology rivers/lakes/watersheds, demographics population/settlements), each with a `✓ in World book` / `· press C to compile` mark. |
| **W7-P3 — Deepen the world→prose bridge** | Make `realworld places` idempotent/refreshable and surface it (materialize demographics settlements → the Places book with provenance), and add a demographics/astronomy → **Timeline** seed (founding dates, seasons) so the simulation actually feeds the manuscript's places and calendar. **✅ Shipped (calendar bridge)** — the Places bridge already exists (the idempotent `propose`→accept flow, authority-respecting). Added the **astronomy → story-Timeline calendar** bridge: `build_timeline_calendar` (pure, tested) derives a `timeline.calendar` `CalendarConfig` (day→month→year units with author month names + the four season markers) from the world; `realworld calendar` prints it to adopt (the sim proposes, the author pastes it into `inkhaven.hjson`), and the `Ctrl+B W` overview surfaces the story-calendar line. *(Deferred: settlement founding-date events — demographics carries no dates; would be synthetic.)* |
| **W7-P4 *(stretch)*** | Validation of every definition block (not just astronomy); a small **magic/rules** compile pass over the `MagicLedger` beyond materialization; plakat map polish. Retire the module-level `#![allow(dead_code)]` on `src/world/` as the surface lands. |

## Dependency posture
- **No new runtime crates.** Every layer, materializer, and reader already exists; WORLD-7 is
  orchestration, surfacing, and documentation.
- **Authority discipline preserved** — the compiler *proposes*; the author accepts (the WORLD-4
  proposal flow is unchanged). Astronomy stays closed-form fact; the stochastic layers keep their
  seed.
- **Deterministic** — same `(world.hjson, seed)` yields the same world; nothing calls a model on
  the physical layers.

## Recommended first cut
**W7-P1** — the one-command full-world compile + the doc fix. It turns "run five commands for a
partial world" into "run one command for a fully-materialized world," it is pure orchestration of
tested building blocks, and it makes the rest of the track (surfacing, the prose bridge) meaningful.
