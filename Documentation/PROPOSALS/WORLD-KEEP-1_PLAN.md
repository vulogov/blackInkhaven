# WORLD-KEEP-1 — "A World That Keeps Its Word"

*Status: IN PROGRESS — ships in **3.15.0** alongside
[`CANON-UI-2`](CANON-UI-2_PLAN.md) (the author's call, 2026-10-01).*

## Why

The two 3.14.0 correctness passes fixed what was *wrong* in the worldbuilder and
`realworld`. Their audits also left a list of things that are not wrong so much
as **unkept**: fields the schema accepts and nothing reads, two layers that
measure the same grid with different rulers, a guided interview that only speaks
English in a tool whose hard rule is five languages, and compiles that freeze the
screen. None of it panics. All of it makes the world say one thing and do
another. This track makes every declared value mean what the docs say it means.
Deterministic, no new store, no new dependency.

Every item below was verified in the 3.14.0 tree.

## Phases (each independently shippable; value core = P1 + P4)

- **WK-P1 — one ruler.** Distances and areas come from a single cell-size
  function.
  - `demographics_layer` hard-codes `CELL_KM2 = 625` (25 km cells) while
    `travel::cell_km` derives roughly 250 × 167 km cells from the planet radius
    and grid. Population density and travel time therefore disagree about how big
    the world is. Demographics takes its cell area from `cell_km`.
  - `geology.dem.scale_km_per_pixel` is parsed, documented ("default 5.0") and
    read nowhere: a DEM world is always treated as the whole globe. When a DEM
    declares a scale, travel/trade distance and cell area use it (resampled to
    the model grid); latitude span follows from the map's north–south extent.
    Pre-flight: decide the latitude anchor for a regional map (centre on a
    declared `lat`, default equator-centred) — a small schema addition
    (`dem.center_lat`, optional, defaulted).
  - Re-baseline the affected tests; note the population change in the release
    notes (numbers move — a determinism break for existing worlds, called out).

- **WK-P2 — the week exists.** `calendar.weekdays` and `calendar.day_names` are
  parsed and unused. `realworld calendar` emits them into the adopted Timeline
  calendar (the timeline already knows a `week` precision), `weather` / `scene`
  name the weekday of a day-of-year, and `lint_definition` already checks the
  name count. Pre-flight: confirm how the Timeline `CalendarConfig` represents a
  week that does not divide the month.

- **WK-P3 — the worldbuilder sees the heightmap.** `plausibility::compile_layers`
  (the live ★ score, `/compile`, `/validate`, the Map pane) always generates
  terrain, so a DEM world is scored and drawn as its procedural twin while the
  CLI compiles the real one. It takes the project root and uses the shared
  `compile_geology_at` introduced in 3.14.0. `/terrain` output then round-trips
  into the pane it was sculpted in.

- **WK-P4 — the world speaks the project's language.** A standing violation of
  the multilingual rule (en/ru/fr/de/es):
  - the interview script (prompts, stage labels, the opening and closing turns)
    is hard-coded English → a per-language script keyed on the project language,
    with the answer words (`orange` / `оранжевая` / …, `yes` / `да` / …)
    accepted in each;
  - *Deferred — larger than a template pass:* the rationales and committed prose
    of `propose`, `propose-rulers`, `propose-language`, `propose-myth` and the
    `critique` Note titles are hard-coded English, but their slots are filled
    from the compile layers' own English vocabulary (settlement class, siting
    basis, biome, the generated ethos and belief phrases). Translating only the
    sentence frame would commit mixed-language prose into the author's books;
    doing it properly means five-language tables for every one of those
    vocabularies. Scoped as its own item;
  - the `coherence` and slow fact-check system prompts never name the project
    language → they do.
  - Decision to make here: `primary_language` is only *displayed* today, though
    the docs say it "sets the fact-checker's language". Recommendation: the
    **project** language drives all generated prose (one rule, like every other
    reader); `primary_language` stays the *world's* common tongue, used by the
    language proposals — and the docs say exactly that.

- **WK-P5 — compiles leave the UI thread.** Worldbuilder `/roll` (up to eight
  full compiles), `/compile`, `/export --pdf`, and every map placement (two
  compiles each); the main TUI's `Ctrl+B W` map and compile (DEM decode + eleven
  materialize steps + the map renderer) and the idle fact-check's uncached
  recompile. A single-flight background job with a spinner and a cached
  `CompiledLayers` keyed on the definition hash, panic-contained like the other
  background work. The world is re-read once per change, not once per query.

- **WK-P6 — small promises.**
  - `Perlin::new(seed as u32)` truncates: seeds differing only in the high 32
    bits share terrain noise → fold the full seed.
  - *Deferred (needs a `world.db` migration decision):* place-proposal
    signatures ignore the seed, so accept/reject decisions leak across an adopted
    seed — but changing the signature would re-propose every Place an existing
    project already resolved; and `realworld map` ingest overwrites coordinates
    set with `set-coords` — but the place-link table records no provenance, so
    "author-set wins" needs a new column. Both wait for that decision.
  - A landmark declared outside the grid is clamped to the border silently, and a
    landmark on a capital's cell (with every road to it) is dropped silently →
    both warn in `validate`.
  - `weather` reports "deep winter" on a planet with no axial tilt → the season
    wording follows the tilt.
  - `/terrain` leaves its PNG behind on `/undo` / `/reset` → cleaned up with the
    delta that referenced it.

- **WK-P7 — docs + the book.** WORLDBUILDING.md, appendix C and the affected
  chapters of *Building the World* (scale, weeks, languages), release notes.

## The ~1-user gate

The author is using the worldbuilder now — the interview bug was found by using
it, in a project whose language rule is not English-only. P4 fixes the surface
that was just exercised; P1 fixes numbers the fact-checker judges prose against.
Accretion is low: nothing here adds a feature, every phase removes a gap between
what the schema accepts and what the pipeline does.

## Constraints kept

Deterministic and free throughout (no new LLM path); multilingual by construction
(P4 is the rule being applied); atomic writes; background work single-flight and
panic-contained; no external-binary dependency added.

## Non-goals

Replacing `plakat` with an in-crate renderer (the one external binary, against the
project rule — worth its own sizing, not folded in here); new world layers; any
change to what the fact-checker *checks*.
