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

- **WK-P1 — one ruler.** *(Done; calibrated rather than naively unified.)*
  - Demographics hard-coded a 625 km² cell (a 4000×3000 km region) while climate,
    travel and trade treat the grid as the whole planet (≈250 × 167 km cells at
    the equator). The climate layer now carries each row's true cell area — the
    same `cell_km` travel uses, × cos latitude on a planet — and demographics
    measures with it.
  - Unifying the area alone put **2.4 billion** people on the starter world: the
    biome capacities are densities of *worked* land, and the old undersized cell
    had been hiding that. A `SETTLED_FRACTION` (1.57 % of habitable land under
    cultivation) is calibrated so an Earth-sized, Earth-like world keeps its
    bronze-age ~38 M — the starter world's population and settlements are
    unchanged. What changes: population now scales with the planet's radius
    (twice the radius, about four times the people) and with where the land sits
    (polar cells count for less), so other seeds and non-Earth-sized worlds move.
  - `geology.dem.scale_km_per_pixel` is now optional. Declared, the map is a
    region of that size: travel and trade distances, the scene's nearest-feature
    distance and population density all use it. Omitted, the image is the whole
    planet (the previous behaviour for everything).
  - *Not done:* a regional map's **latitude**. Its climate still runs pole to
    pole down the image; the row→latitude mapping is shared by climate, weather,
    the scene brief, `set-coords` and the landmark grid, and re-anchoring it
    (`dem.center_lat`) changes every biome of an existing DEM world. The mismatch
    is reported as a low-severity warning instead.

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

- **WK-P5 — compile once, not once per consumer.** *(Re-scoped by measurement.)*
  The plan was to move compiles off the UI thread. Measured first: a whole
  `realworld validate` — process start, every layer, every lint — takes ~0.26 s
  in a **debug** build, so one compile of a generated world is not what freezes
  anything. What cost time was *repetition*: the worldbuilder compiled the world
  for the ★ score and again for the map on every change (and WK-P3 made each of
  those a heightmap decode for a DEM world), and the main TUI's idle fact-check
  recompiled geology on every run just to list minerals.
  - `run_fast_with` lints layers the caller already compiled; the worldbuilder
    compiles once per world change and hands the layers from the scorer to the
    map (keyed on the definition).
  - The idle fact-check keeps its world context (moons, minerals) until
    `world.hjson` or its heightmap changes on disk.
  - *Not done:* moving `Ctrl+B W` → compile/materialize (eleven World-book
    writes + embeddings) and `/export --pdf` to a background job. Materialize
    creates and rewrites store nodes; doing that from a worker while the editor
    is live is a concurrency change that needs a real session to verify, and
    since 3.14.0 a re-compile already skips unchanged leaves. Left on the UI
    thread deliberately.

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
