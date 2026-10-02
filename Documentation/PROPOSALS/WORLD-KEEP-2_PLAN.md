# WORLD-KEEP-2 — "The Rest of the Word" (3.16.0)

*Status: PLAN. On `3.16.0-dev`, sequenced after
[`CANON-READER-1`](CANON-READER-1_PLAN.md). The four items
[`WORLD-KEEP-1`](WORLD-KEEP-1_PLAN.md) deferred, each with the reason it was
deferred turned into its first step.*

## Phases

- **WK2-P1 — a migration path for `world.db`.** Two fixes waited on this. The
  store has no schema version: tables are `CREATE TABLE IF NOT EXISTS`, so a
  column cannot be added to an existing project. Add a `world_meta` key/value
  table carrying `schema_version`, and an idempotent, forward-only migration
  step run at open (add-column-if-missing; never destructive; a newer file than
  the binary understands opens read-compatible). Tested against a `world.db`
  created by the 3.15.0 schema.
  - **Author-set coordinates survive map ingest.** `world_place_links` gains
    `coords_source` (`compiled` | `author` | `map`). `set-coords` marks `author`;
    `realworld map` ingest refines only rows that are not `author`. Existing rows
    default to `compiled` (today's behaviour for them).
  - **Seed-scoped proposals.** `world_proposals` gains `seed`. A place accepted
    or rejected under one seed no longer suppresses a settlement at the same cell
    of a different, adopted seed.

- **WK2-P2 — a regional map has a latitude.** A heightmap with a declared scale
  is measured as a region but still weathered pole to pole. `geology.dem` gains
  an optional **`center_lat`**: with it, the rows of the image span the
  latitudes the map's height actually covers around that centre, and climate,
  weather, the scene brief, `set-coords` and the landmark grid all read latitude
  from **one** shared mapping (eighteen call sites today compute it inline).
  Strictly opt-in: a world that does not declare `center_lat` compiles exactly as
  it does now, so no existing world's biomes move. The 3.15.0 warning then names
  the key that resolves it.

- **WK2-P3 — proposals in the project's language.** `propose`,
  `propose-rulers`, `propose-language`, `propose-myth` and the `critique` Notes
  write English sentences whose slots are filled from English vocabularies
  (settlement class, siting basis, the twelve biomes, the culture layer's
  generated ethos and belief phrases). Five-language tables for each of those
  vocabularies, then localised sentence frames over them, keyed on the project
  language. The stored *payload* keeps its canonical English keys (signatures,
  dedup and the fact-checker are unaffected); only what the author reads and what
  is committed to their books is localised. Pre-flight: inventory the culture
  layer's generators — the size of that vocabulary sets the size of this phase.

- **WK2-P4 — materialize off the UI thread.** `Ctrl+B W` → compile writes eleven
  World-book chapters and re-embeds them on the UI thread. Move it to the shared
  background job (single-flight, panic-contained, progress per layer). Deferred
  in 3.15.0 because it creates and rewrites store nodes from a worker under a
  live editor and could not be verified. It can be now: the tmux-driven smoke
  method used for the 3.15.0 re-cut exercises exactly this — compile in the
  background while saving a paragraph in the foreground. That live check is the
  gate; without it the phase does not ship.

- **WK2-P5 — docs.** WORLDBUILDING.md, the book (appendix C, the land and map
  chapters), release notes.

## Decisions to make

1. **Old proposal decisions under a new seed (WK2-P1).** Rows resolved before the
   migration carry no seed. Recommendation: they keep applying to *every* seed
   (the decisions you already made stay made); only decisions made from 3.16.0 on
   are seed-scoped.
2. **Latitude is opt-in (WK2-P2).** Recommendation as written: nothing changes
   for a world without `center_lat`. The alternative — inferring a latitude for
   every scaled heightmap — would rewrite the biomes of existing worlds.

## The ~1-user gate

The worldbuilder is in active use and the project language is not English-only:
P3 finishes what the 3.15.0 interview translation started, and P1's two fixes
remove ways the tool silently overrode the author. Accretion is low — no new
feature, four closed gaps.

## Constraints kept

Deterministic; multilingual (P3 is the rule applied); forward-only,
non-destructive migration; atomic writes; no external-binary dependency added.

## Non-goals

Replacing `plakat` with an in-crate renderer (still unsized, still its own
item); new world layers.
