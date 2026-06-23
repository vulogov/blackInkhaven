# RFC WORLD-4 — World Simulation

| | |
|---|---|
| **RFC** | WORLD-4 *(authored as "WORLD-1"; renumbered — WORLD-1/2/3 are the shipped consistency pillar, 1.3.8/1.3.10/1.3.11)* |
| **Title** | World Simulation (`inkhaven realworld` + real-time fact-checker) |
| **Status** | **In progress — building incrementally in the 1.3.x cycle** |
| **Created** | 2026-06-25 |
| **Author** | Vladimir Ulogov |
| **Target version** | author wrote 1.5.0; **pulled forward into the 1.3.x tree** (user-directed 2026-06-25) |
| **Depends on** | PANE-1 (Output pane) — **COMPLETE (1.3.24, finished 1.3.25)** |
| **Soft-depends on** | LANG-1/2/3 (ConLang Suite) — **all shipped** (Languages cross-references) |
| **External dependency** | plakat (optional, subprocess; map generation) |

---

> ## Status banner (1.3.x incremental build)
>
> Author targeted 1.5.0; per Vladimir's direction we build incrementally in the
> **1.3.x** tree, one phase per signed increment (like LANG-1/2/3 + PANE-1).
> **ZERO new deps until a phase genuinely needs one** (then only the pure-Rust
> crates named in §11). The new module is **`src/world/`** (distinct from the
> shipped `src/world_report.rs`).
>
> **Identifier:** authored as "WORLD-1" but that number (and WORLD-2/WORLD-3) is
> shipped — the *consistency* pillar (facts/anachronism/bible 1.3.8, drift 1.3.10,
> drift-depth + world report 1.3.11). This *simulation* RFC is **WORLD-4**
> (user-decided 2026-06-25). Same naming-collision resolution as LANG-2→LANG-3.
>
> **PANE-1 dependency satisfied.** The Output envelope (extensible string `kind`,
> `ActionId::{Primary,Promote,Dismiss,Pin,AskAi,Snooze,Expand}`, severities,
> lifetimes, persistence, ask-AI bridge, the `lexicon_proposal`→`Enter`-promote
> pattern) covers every WORLD-4 message kind with no new pane mechanics.
>
> **Phasing (per §12), MVP = P0–P4:**
> - **P0** — Compiler foundation + **Astronomy** layer (types, validate,
>   astronomy compute, storage, materialize, `realworld new/validate/compile
>   --layer astronomy`, `Ctrl+B W` chord scaffold). *NOTE: `Ctrl+B W` is
>   currently `ToggleTypewriter` — resolve the chord collision when the TUI
>   surface lands (two-level chord or re-home typewriter).*
> - **P1** — Geology + Climate + Hydrology (+ DEM import). *First new deps:
>   `image`, `noise`, `delaunator`.*
> - **P2** — Demographics + Proposal queue + Cross-references + Facts↔World.
> - **P3** — Magic ledger + Plakat integration (`geojson`). Plakat optional.
> - **P4** — Fast-track fact-checker (UD parser asset, patterns, gazetteer, 5
>   category predicates, magic consult, debounced trigger, `fact_check_warning`).
> - **P5** — Slow-track fact-checker (LLM extraction + coherence, cost caps, seams).
> - **P6** — Multilingual (per-paragraph language, prompt/pattern/warning
>   localization, gazetteer variants, fallback chain).
> - **P7** — Polish (tutorial, `WORLDBUILDING.md`, tutorial chapter, example world).
>
> **Authority discipline (the spine):** author always wins. Compiler *proposes*
> (proposal queue; nothing commits without acceptance). Fact-checker *warns*
> (never blocks). The **magic ledger** declares exceptions to physics the
> fact-checker respects. **World** = physical-model derivations (compiler-owned,
> regenerable); **Facts** = specific commitments (author-owned, `derived_from`
> provenance on AI-proposed entries).
>
> ### Progress log
>
> - **P0.1 — astronomy layer (UNRELEASED, 1.3.25-dev).** New `src/world/` module
>   (`mod world` in main.rs): `WorldError`/`Result`; `types::world`
>   (`WorldDefinition::from_hjson` mirroring `conlang::*::from_hjson`, with a
>   `SeedValue` that accepts a decimal int or a `0x…` hex string — HJSON renders
>   unquoted hex as a quoteless string; the astronomy block modelled in full,
>   other blocks parse-and-ignore via serde); `types::astronomy`
>   (`AstronomyOutput` + sub-structs); `compile::astronomy_layer::compile_astronomy`
>   — **closed-form physics, deterministic, NO proposals, ZERO new deps**:
>   stellar mass from the mass–luminosity relation, Kepler-III year length in
>   planet-days, declared-vs-computed divergence flag, season markers anchored by
>   `new_year_aligns_to`, daily-insolation per 10° latitude band (the standard
>   hour-angle integral), lunar synodic periods + lunations, tidal forcing
>   (∝ mass/distance³, distance ∝ Kepler period — so a closer light moon out-tides
>   a farther heavy one), Earth-calibrated solar/lunar tide ratio (~0.46), calendar
>   consistency. 6 tests (Earth ≈365d + 0.46 solar tide; equator>poles insolation;
>   Velmaron ≈324.6 planet-days flags its 348-day calendar; winter-solstice anchor;
>   determinism; HJSON+hex-seed parse→compile). tests 1585→1590.
> - **P0.2 — `realworld` CLI (UNRELEASED, 1.3.25-dev).** `Command::Realworld` +
>   `src/cli/realworld.rs`: `new <name> [--force]` (scaffolds a valid starter
>   `world.hjson` at the project root), `validate`, `show [--json]`,
>   `compile [--layer <l>] [--json]`. Reads `<project>/world.hjson`, parses, runs
>   `compile_astronomy`, prints a human summary (year in planet-days + Earth-days,
>   declared-vs-computed divergence with ⚠, seasons by day-of-year, per-moon
>   synodic period + lunations, dominant moon + solar tide ratio, calendar
>   consistency) or `--json`. `--layer geology/…` errors "not implemented yet"
>   (only astronomy has landed). Smoke-validated: Earth-like template →
>   synodic month 29.5 d (matches reality), sun 0.46× lunar, 360-day calendar
>   flagged vs the 365.3-day year. tests 1591. ZERO new deps.
> - **P0.3 — World system book + astronomy materialization (UNRELEASED,
>   1.3.25-dev).** Registered the **World** system book: `SYSTEM_TAG_WORLD` +
>   `("world","World")` in `SYSTEM_BOOKS` immediately after Artefacts (the seeder's
>   bump logic slots it into existing projects; the worldbuilding cluster
>   Places/Characters/Artefacts/World now sits together). `src/world/materialize.rs`
>   — `materialize_astronomy(store, cfg, &AstronomyOutput)` writes three structured
>   (JSON, `content_type=hjson`) paragraphs under `World / Astronomy /` — *System
>   overview* (year/divergence/insolation), *Calendar* (seasons + calendar check),
>   *Celestial events* (moons/eclipses/tide). **Idempotent** (create-once, then
>   update-in-place — no duplicates). Astronomy is compiler-owned and overwrites
>   (it's closed-form fact, not a proposal). GOTCHA: `update_paragraph_content`
>   syncs the DB but NOT the on-disk `.typ` (the source of truth) — mirror
>   `create_chapter_paragraph`: set `content_type`, `std::fs::write` the body to
>   `node.file`, THEN `update_paragraph_content`. CLI: `realworld compile
>   --materialize` opens the store + writes the book. Smoke-validated end-to-end
>   (3 created → idempotent 0-created/3-updated; JSON body lands on disk). +1
>   system-book registration test. tests 1591→1592. ZERO new deps.
> - **P0.4 — `Ctrl+B W` World overview (UNRELEASED, 1.3.25-dev) → P0 COMPLETE.**
>   Chord collision resolved (user-decided): **ToggleTypewriter moved `Ctrl+B W` →
>   `Ctrl+B Shift+W`** (Ctrl+V W was unavailable — it's the story-graph views),
>   freeing `Ctrl+B W` for the World family. `Action::OpenWorldOverview` →
>   `Modal::WorldOverview{rows,cursor}` (read-only scrollable list, mirrors the
>   ConLang hub Ctrl+B X): reads `world.hjson`, compiles astronomy, shows the
>   definition + astronomy summary (year/divergence/tilt/moons/tide/calendar) +
>   per-layer status (geology/climate/hydrology/demographics "not yet
>   implemented") + materialization status (is it in World/Astronomy?). `↑↓`
>   scroll, `Esc` close. KEYBINDING.md updated. +1 chord regression test
>   (Ctrl+B W→overview, Ctrl+B Shift+W→typewriter). tests 1592→1593. ZERO new deps.
>   *Sub-chords (W L/N/C/M/P/F…) deferred — the CLI covers compile/validate; the
>   sub-family lands with P2's proposal queue (the first thing that needs a TUI).*
>
> **★★ WORLD-4 P0 COMPLETE (astronomy layer end-to-end): types + closed-form
> physics + materialization into the World book + `realworld` CLI + `Ctrl+B W`
> overview. 14 tests, ZERO new deps, all signed.**
> - **P1.1 — geology layer + first new deps (UNRELEASED, 1.3.25-dev).** Added
>   `noise` + `delaunator` (pure-Rust, MIT/Apache, RFC §11; `image` was already
>   in-tree, covers DEM). Determinism uses an **in-tree SplitMix64** seeded by the
>   world seed, not `rand`. `types::{world::{GeologyDef,GeneratedGeology,
>   DemGeology},geology::GeologyOutput}` + `compile::geology_layer::compile_geology`
>   (generated path): seed → plate seeds → **Delaunay adjacency** (delaunator) →
>   boundary classification (convergent/divergent/transform from relative plate
>   motion) → **4-octave Perlin heightmap** (noise) biased by continental plates +
>   convergent-boundary uplift (orogeny active/quiet/ancient) → continents
>   (flood-fill ≥0.5% land), sea coverage, mountain ranges, mineral hints,
>   elevation stats. Heightmap kept in-memory (`#[serde(skip)]` — a PNG asset
>   later, not JSON). CLI `realworld compile --layer geology` (+`--json`). 5 tests;
>   smoke-validated (Velmaron → 4▲1▽8↔, 2 continents, 36% ocean, 3 ranges along
>   convergent boundaries, copper/gold/iron/coal). tests 1593→1598.
> - **P1.2 — geology materialization (UNRELEASED).** `materialize_geology` → 3
>   JSON paragraphs under `World / Geology /` (Continents&plates, Mountains&ranges,
>   Mineral distribution) + the normalised heightmap rendered as
>   `assets/world/heightmap.png` (8-bit grayscale via `image`; the summary points
>   at it, not a JSON wall). CLI `--layer geology --materialize`. tests 1598.
> - **P1.3 — climate layer (UNRELEASED).** `compile_climate(def, astro, geo)` — a
>   zonal model: global mean from stellar flux (Earth-calibrated), latitude profile
>   from the astronomy insolation (renormalised, tilt-aware), elevation lapse from
>   geology; precipitation = zonal belts × continentality (BFS dist-to-ocean) ×
>   orographic (windward/leeward of mountains); Köppen-style biomes; aggregated
>   zones + Hadley/Ferrel/Polar wind bands (flip on retrograde). Materializes
>   `World/Climate/{Climate zones, Prevailing winds}`. 5 tests. tests 1598→1603.
> - **P1.4 — hydrology layer (UNRELEASED) → P1 terrain trio COMPLETE.**
>   `compile_hydrology(geo, climate)` — textbook D8: flow direction → rainfall-
>   weighted accumulation → rivers (Strahler-style order, counted at ocean mouths)
>   → lakes (interior pits) → watersheds (memoised terminal trace) → settlement
>   priors (river_mouth/confluence/fertile_valley, ranked, for Layer 5).
>   Materializes `World/Hydrology/{River systems, Watersheds and settlement
>   priors}`. 3 tests. tests 1603→1606. *Next: DEM-import path (image; author
>   supplies a heightmap → skip generated geology) finishes P1; then **P2** —
>   demographics + the proposal queue + cross-refs + the `Ctrl+B W` sub-chords.*
>
> **DEFERRED DELIVERABLE (user-requested 2026-06-25):** once WORLD-4 is *fully*
> implemented (all five layers + magic), generate `./examples/realworld/Earth.hjson`
> — a complete real-Earth definition (every parameter) for authors starting in an
> Earth setting. Tracked here so it isn't lost; do it at the P7 polish phase.

---

## 1. Summary

Two complementary branches on a shared world model.

**Branch A — `inkhaven realworld` compiler.** Deterministic, layered, on-demand:
compiles a structured world definition (HJSON declarative + Bund procedural) into
populated system books (Facts, Places, Notes, Research, Characters, and a new
**World** system book). Five MVP layers: astronomy, geology, climate, hydrology,
demographics. Each layer reproducible from `(inputs, seed)`; upper layers depend
on lower. Optional plakat maps (emit MapSpec → plakat renders → ingest GeoJSON
coordinates back into Places).

**Branch B — real-time fact-checker.** Reads prose for world-assertions,
verifies against the simulation. **Fast track**: continuous background (debounced
5 s), patterns + multilingual UD parser, < 500 ms/paragraph, warnings to Output.
**Slow track**: on-demand (idle/close/chord), LLM extraction of subtle
assertions. Both check five categories (travel time, season/climate, astronomy,
demographics, economy) and consult a **magic ledger**.

**Multilingual day one** (EN/RU/ES/FR/DE) via existing stemmers + embeddings +
`whatlang` + a lazy ~280 MB UD parser. **Plakat optional** (data-coupled
subprocess). **Authority discipline** as above.

## 2–6 (motivation / goals / non-goals / constraints / audience)

Invariants carried into implementation:

- **Single binary; pure Rust** in-repo; UD parser via existing candle stack;
  plakat is a subprocess; parser model lazy at `~/.inkhaven/assets/world/parser/`.
- **PANE-1 hard dependency** — emits exclusively through the Output pane.
- **Existing system books unmodified except additive optional fields.**
- **No new persistence layer** — new tables join the existing DuckDB schema; same
  backup/restore/snapshot/reindex.
- **Bund sandbox respect** — `ink.world.*` obey existing categories; no new ones.
- **LLM provider neutrality**; **plakat 1.5+** with graceful degradation.
- **5 MVP layers (not 9)** — ecology/economy/society/history deferred (need
  WALS-style catalogues or are multi-month). **5 baseline languages** = what
  Inkhaven already supports.

## 7. Design overview

### 7.2 Module layout (`src/world/`)

```
mod.rs                  -- public API, error types
storage/{mod,repository,proposals,cross_refs}.rs
types/{mod,world,astronomy,geology,climate,hydrology,demographics,magic,seed}.rs
compile/{mod,validate,astronomy_layer,geology_layer,climate_layer,
         hydrology_layer,demographics_layer,materialize,incremental,proposals}.rs
plakat/{mod,detect,mapspec_emit,invoke,ingest}.rs
fact_check/{mod, common/{assertion,extraction,evaluation,magic_consult,warning},
            fast/{patterns,parser_ud,gazetteer,trigger},
            slow/{extraction_llm,coherence_llm,scheduler,cost,seams},
            categories/{travel_time,climate_season,astronomy,demographics,economy}}
multilingual/{mod,detect,patterns/<lang>,prompts/template_loader,
              gazetteer_variants,warning_localize,fallback}
book/{mod,structure,renderers/<layer>}
io/{mod,export_bundle,import_bundle,dem_import}
cli.rs ; tui.rs ; bund.rs
```

### 7.4 World system book (10th, after Artefacts)

```
World/
  00-overview                 (author prose)
  01-astronomy/{01-system-overview,02-calendar,03-celestial-events}
  02-geology/{01-continents-and-plates,02-mountains-and-ranges,03-mineral-distribution}
  03-climate/{01-climate-zones,02-prevailing-winds,03-ocean-currents}
  04-hydrology/{01-river-systems,02-watersheds,03-lakes-and-seas}
  05-demographics/{01-settlement-overview,02-population-distribution,03-major-cities-list}
  06-magic-ledger/{01-rules,02-promoted-dismissals}
  99-compiler-state/{01-version,02-staleness-report}
  (reserved: 07-ecology,08-economy,09-society,10-history)
```

Each leaf HJSON (structured) or Typst (prose). Compiler writes structured leaves;
author prose in `00-overview`/chosen subchapters. Re-compile regenerates only
compiler-owned content; hand-edits preserved + flagged.

### 7.5 World vs Facts

**World** = the physical model (regenerable, deterministic). **Facts** = specific
commitments (author-curated). Derived Facts carry `derived_from` (source World
paragraph + version); staleness = stored vs current version.

### 7.6 Two-tier config

`world.hjson` (declarative) + `world.bund` (procedural lifecycle hooks). Mirrors
LANG-1.

## 8. Detailed design (implementation reference)

### 8.1 `world.hjson` — six MVP blocks

Top-level `name`/`seed`/`primary_language` + `astronomy`/`geology`/`technology`/
`magic`/`compiler`. `astronomy={star,planet,orbit,moons[],calendar}`;
`geology={generated{plates,continents,mountain_orogeny,sea_level}}` OR
`{dem{path,scale_km_per_pixel,sea_level_pixel_value,coordinate_origin}}`;
`technology={baseline,baseline_extensions[],exceptions[]}`;
`magic={enabled,rules[]}`; `compiler={layers_enabled[],ai_elaboration{}}`.
(Velmaron example: Appendix C of source RFC.)

### 8.2 Magic ledger

`MagicLedger{enabled, rules:Vec<MagicRule>}`;
`MagicRule{id, kind:RuleKind, covers:Vec<CheckCategory>, description,
applicability:Applicability, parameters:Value}`;
`RuleKind = ExtendedLifespan|WeatherControl|ExtendedTravelSpeed|MessengerBirds|
Teleportation|Resurrection|PreternaturalStrength|AcceleratedAging|
DivineIntervention|CustomBund(String)`;
`Applicability{roles?,regions?,seasons?,frequency_per_year?,world_year_range?}`.
**Lazy consult**: generate candidate finding, *then* match ledger by
`covers`+`applicability` → suppress-with-note or emit.

### 8.3 Pipeline

`validate → astronomy → geology(|DEM) → climate → hydrology → demographics →
materialize → (mapspec emit)`. **Incremental** via `world_layer_state`
(`inputs_hash`/`outputs_hash`); topo walk skips unchanged, cascades changes;
`compile --force` = full.

### 8.4 Astronomy (Layer 1) — deterministic closed-form, NO proposals

**Inputs**: star class+luminosity, planet mass/radius/tilt/day_length, orbit a+e,
moons[], calendar. **Outputs**: year length in planet-days (Kepler III, stellar
mass adjusted); insolation by 10° latitude band × season; solstice/equinox
fractions; per-moon synodic period + lunar-months/year; eclipse sample (first 10
yr); tides by coastline (lunar+solar, dominant moon); calendar conversions.
Astronomy = *fact not opinion*; re-asserted every run unless overridden.

### 8.5 Geology (Layer 2)

Generated: seed→Voronoi plates (`delaunator`)→continents→mountains→mineral
hints→noise heightmap (`noise`, ~5 km/px). DEM: read PNG/TIFF (`image`), skip
generation. Proposals: continent/range names.

### 8.6 Climate (Layer 3)

Zonal (Hadley/Ferrel/Polar) → winds → rainfall belts → continentality →
orographic rain shadows → Köppen ~16 biomes → zone polygons. Proposals:
biome/zone names.

### 8.7 Hydrology (Layer 4)

D8 flow → accumulation (rainfall-weighted) → rivers + Strahler order → lakes →
watersheds → settlement priors. Proposals: river/lake names.

### 8.8 Demographics (Layer 5) — deterministic + AI elaboration

Carrying capacity (biome×water×tech) → population (seeded noise) → size hierarchy
(Rank-Size/Christaller) → city placement → plausible role types (AI). Cities→
Places, role types→Characters archetypes.

### 8.9 Materialization (proposals)

`Proposal{id,world_id,compilation_version,target_book:SystemBook,
target_path:Option<TreePath>,action:ProposalAction,payload:Value,
derivation:ProposalDerivation,rationale,status:ProposalStatus}`;
`ProposalAction=Create|Update{target_id}|MarkStale{target_id}`;
`ProposalStatus=Pending|Accepted|Rejected|Edited|Stale`. Each → Output as
`world_compiler_proposal` (actions accept/edit/reject/ask_ai/expand/dismiss,
until_acted_on). Batched + PANE-1 auto-grouping; reject recorded so re-compile
won't re-propose unchanged.

### 8.11 Plakat

Detect (`plakat --version`) → emit MapSpec (each feature `inkhaven_id`) → invoke
`plakat map` (progress→Output) → ingest GeoJSON → back-populate Place anchor/
coordinates (unless author-edited) → cache `world_plakat_renders`. **Manual only**
(`Ctrl+B W M`); `world_map_stale` notice.

### 8.13–8.22 Fact-checker

**Fast** (5 s debounce, <500 ms): language (whatlang, cached) → patterns → UD
parser → gazetteer → typed Assertions → 5 category predicates → magic consult →
`fact_check_warning`. **Slow** (5-min idle/close/`Ctrl+B W F`, <60 s/chapter):
preflight cost → LLM extraction → coherence → magic consult → seams → emit.
**Categories**: travel_time (`pace` vs `baseline_pace(tech,terrain,season)` table;
ratio>2.5 Contradiction, >1.5/<0.5 Warning), climate_season (4-level → severity),
astronomy (sun/moon/star vs claims), demographics (pop vs carrying capacity;
role-age), economy (resource availability + trade routes). **Magic consult**
uniform. **Seams**: Slow excludes Fast findings; Fast skips Slow-checked;
low-confidence Fast → Slow priority.

### 8.23 Multilingual

Per-paragraph `detected_language` (whatlang at save); gazetteer grammatical
variants (AI, author-confirmed once, cached); per-language prompt templates in
`books/Prompts/world/`; warning `body`+`body_en`; fallback pattern→parser→LLM→skip.

### 8.24 Output message kinds

`fact_check_warning` (until_paragraph_edit), `world_compiler_proposal`,
`world_compiler_progress`, `world_compiler_complete`, `world_plakat_progress`,
`world_plakat_complete`, `world_map_stale`, `slow_track_preflight`,
`slow_track_progress`, `slow_track_partial`, `slow_track_complete`,
`magic_ledger_promotion_suggestion`.

### 8.25 LLM cost protection

`CostBudget{max_calls_per_run:50, max_tokens_per_run:100k, confirm_above_calls:20,
max_calls_per_day:200, max_calls_per_month:5000}`; preflight →
`slow_track_preflight`; `world_llm_usage(date,provider,calls,in_tok,out_tok)`;
backoff 3×~2min → `slow_track_partial`.

## 9. Bund stdlib (`ink.world.*`, 40 words) + hooks

list/get/create/delete/compile[.layer/.status/.invalidate]/proposals.{list,accept,
reject,edit}/magic.{list,add,remove}/layer.{astronomy,geology,climate,hydrology,
demographics}/facts.{query,derived}/places.{from_world,coordinates}/dem.import/
plakat.{available,render,mapspec.emit,ingest}/fact_check[.fast,.slow,.dismissals
[.promote],.usage]/gazetteer.{list,rebuild,variants.generate}/bundle.{export,
import}. Hooks: on_world_compile_layer, on_world_proposal_emit/accepted,
on_fact_check_warning/dismissal, on_world_mapspec_emit, on_world_plakat_complete,
on_magic_ledger_change.

## 10. Surfaces

**CLI**: `inkhaven realworld {new,list,show,delete,validate,compile[--layer/
--force],compile-status,proposals …,magic …,layer …,dem import,map[--style/
--output],mapspec emit,bundle {export,import}}` + `inkhaven fact-check {<scope>,
--fast-only,--slow-only,--paragraph,--chapter,--book,--recent,dismissals …,usage,
gazetteer …,parser …}`.

**TUI `Ctrl+B W` family** *(W collides with ToggleTypewriter — resolve at TUI
phase)*: `W`=overview, `W L`=list, `W N`=new, `W C`=compile (`C C`=layer, `C F`=
force), `W M`=map (`M S`=mapspec), `W V`=cycle layer views, `W A/G/I/H/D`=
astronomy/geology/climate(I)/hydrology/demographics, `W X`=magic, `W P`=proposals,
`W F`=fact-check chapter (`F W`book,`F P`paragraph,`F R`recent), `W E`=export iwb.

**Book-take**: adds `world_map`, `world_bundle` formats to `Ctrl+B O`.

## 11. Dependencies

New (pure-Rust, added only when their phase lands): `geojson` (P3), `image` (P1),
`noise` (P1), `delaunator` (P1). Reused: `whatlang`, `fastembed`, `aho-corasick`,
`unicode-segmentation`, `rust-stemmers`, `serde-hjson`, `duckdb`, `regex`,
`tokio`. Lazy assets: UD parser (~280 MB GGUF via candle, P4); patterns + travel
baselines in-binary. External: `plakat` (optional). NOT: PyTorch/transformers,
GDAL/proj, ggml.

## 12. Phases — MVP = P0–P4 (~20 wk). P0–P3 (~16 wk) = compiler + plakat, no
fact-checker.

## 13. Testing — unit (astronomy formulas, D8, climate classification, gazetteer,
patterns, magic consult); property (determinism from `(definition,seed)`,
invalidation cascade, monotonic proposals); golden worlds (Velmaron+2); fact-check
golden; multilingual round-trip; magic suppression; plakat; seams; cost/backoff;
DEM; incremental; cross-ref integrity; parser quality; e2e book-take; perf.

## 14. Risks — see source RFC §14 (typologically-wrong outputs → known-good
models + magic ledger; false positives → severity tiers + lazy consult +
promotion; parser-on-prose → benchmark + LLM fallback + disable; plakat absent →
status; .iwb portability → separate exports; LLM limits → backoff + ceilings;
non-deterministic Bund → seeded RNG; massive worlds → resolution knob; conlang
cross-project refs → bundle carries declarations + warn).

## 15. Open questions — multilingual parser vs per-language; society WALS;
climate fidelity vs time; plakat MapSpec versioning; magic Bund extensibility;
cost cap per-provider; per-paragraph language override; multi-world switch
(`Ctrl+B W Q`, mirror LANG-1); release-to-compiler UX; conlang dangling refs.

## 16. Appendix A — DuckDB schema (~35 tables)

`world_definitions`, layer tables (`world_astronomy`/`_geology`/`_climate`/
`_hydrology`/`_demographics`, each `output_json`+`inputs_hash`+`outputs_hash`+
`compiled_at`), `world_magic_rules`, `world_layer_state`, `world_proposals`,
`world_{place,character,artefact}_links`, `world_facts_derivation`,
`fact_check_paragraph_state`, `fact_check_dismissals`,
`fact_check_promotion_candidates`, `world_llm_usage`, `world_gazetteer`
(+`idx_gazetteer_surface`), `world_plakat_renders`. **Implementation note:** the
in-tree `StorageEngine` (src/storage/engine.rs) uses unix-secs (not TIMESTAMP)
and scopes by DB file (no project_id column) — follow the `progress.db`/
`output.db` precedent (per-table store on `StorageEngine::new(path, INIT_SQL,
pool)`), adapting the RFC's canonical SQL to that house style.

Appendices B–F (full HJSON config, Velmaron sample, TUI overlays, end-to-end
workflow) live in the source RFC; reproduce into module docs as each phase lands.
