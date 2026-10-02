# WORLD-KEEP-2 — "The Rest of the Word" (3.16.0)

*Status: IN PROGRESS on `3.16.0-dev` (WK2-P1–P4 done), sequenced after
[`CANON-READER-1`](CANON-READER-1_PLAN.md). The four items
[`WORLD-KEEP-1`](WORLD-KEEP-1_PLAN.md) deferred, each with the reason it was
deferred turned into its first step.*

## Phases

- **WK2-P1 — a migration path for `world.db`.** *(Done.)* Correction to this
  plan's first draft: the store was not unversioned — every feature store has
  carried a `_inkhaven_schema` anchor since the 3.0.0 freeze, stamped `1`, so a
  newer file is refused by an older binary. What was missing was the *step*: the
  "older than current" arm did nothing, so no store could ever change shape.
  `StorageEngine::new_migrating` adds it — ordered, forward-only, idempotent
  statements (`ADD COLUMN IF NOT EXISTS`), then the stamp. `world.db` is now v2.
  Verified against a store written by the published 3.15.0 binary.
  - **Author-set coordinates survive map ingest.** `world_place_links` gains
    `coords_source` (`compiled` | `author` | `map`). `set-coords` marks `author`;
    a map render refines only rows that are not. Existing rows read as `compiled`.
  - **Seed-scoped place proposals.** `world_proposals` gains `seed`. A *place*
    signature is a map cell, so a place decision now applies only under the seed
    it was made under; rows from before the migration carry no seed and keep
    applying under every seed. Name-keyed kinds (rulers, languages, myths) stay
    global — their signature already identifies the thing.
  - One-way: once migrated, a 3.15.0 binary refuses the store ("upgrade
    inkhaven"), by the existing guard.

- **WK2-P2 — a regional map has a latitude.** *(Done.)* `geology.dem` gains
  optional **`center_lat`** and `center_lon`. With a regional scale and a
  `center_lat`, `world::latmap::LatMap` is the band of latitude the map's height
  really covers around that centre; otherwise it is the globe, with the exact
  arithmetic the layers always used. Climate, the editor's scene brief, the
  CLI's scene / `set-coords` and every landmark-in-degrees placement read
  latitude through it (carried on `GeologyOutput`). Strictly opt-in — verified:
  five seeds × four layers compile byte-identically to the published 3.15.0.
  The 3.15.0 region warning now names the key and goes quiet once it is set.
  - The census of "eighteen call sites" in the first draft was mostly tests: the
    real readers were climate, the two scene paths, `set-coords`, and the
    landmark grid (nine callers).
  - One pre-existing inconsistency removed: the editor's scene brief used a
    `y/(h−1)` edge convention where the climate and the CLI use cell centres —
    up to half a cell (~0.75°) apart. It now uses the shared mapping.

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

  *(Done.)* The inventory came out small: 3 classes, 3 siting bases, 12 biomes,
  15 ethos phrases + `settled`, 7 beliefs, 7 language-profile terms, 8 myth
  glosses/motif names — `src/world/i18n.rs`, with a test that compiles a real
  world and fails if a layer ever emits a term the tables lack. Frames are
  written to need no grammatical agreement (locative siting phrases; `Народ: …`
  / `Ein Volk: …` lists). The generators keep their signatures and their English
  output byte-for-byte; `localize_all` rewrites a batch after generation, and
  the four commit bodies take the language from the project config. Two things
  the plan did not foresee: (1) a Mythology symbol's **vocabulary** is scanner
  input, so it has to be in the prose language and — `myth scan` being
  exact-match — carry case forms for Russian and German; (2) `belief_vocabulary`
  split on ASCII, so a belief declared in Russian produced no words at all —
  now Unicode-aware. Left alone: author-declared ethos/belief (already the
  author's words), names, and the ConLang chapter titles the brief points at
  (they are scaffolded in English). The translations are the implementer's; a
  native read of ru/fr/de/es would be worth having.

- **WK2-P4 — materialize off the UI thread.** `Ctrl+B W` → compile writes eleven
  World-book chapters and re-embeds them on the UI thread. Move it to the shared
  background job (single-flight, panic-contained, progress per layer). Deferred
  in 3.15.0 because it creates and rewrites store nodes from a worker under a
  live editor and could not be verified. It can be now: the tmux-driven smoke
  method used for the 3.15.0 re-cut exercises exactly this — compile in the
  background while saving a paragraph in the foreground. That live check is the
  gate; without it the phase does not ship.

  *(Done — the gate passed.)* The compile moved out of the TUI method into
  `world::compile_job::compile_and_materialize`, which borrows nothing from the
  app: a cloned `Store` (pooled DuckDB; embedder, vector store and canon ledger
  each behind their own lock; Bund hooks behind the VM's), per-layer progress,
  a cancel flag checked between layers. `BgJobKind::WorldCompile`; the
  completion handler and the inline fallback share `finish_world_compile`.
  Live check, tmux, throwaway project, debug build: 24 typed tokens and 8
  `Ctrl+S` saves across a compile (two saves landing mid-compile) — no
  keystroke lost, every save succeeded, file and store agree, no error in the
  log; an overview left open re-rendered and kept the compile's report on the
  status line; `C` during a compile cancelled between layers; `C` while the
  overview job held the slot compiled inline; quitting mid-compile left a store
  that reopens and recompiles. Not covered by a unit test — a `Store` needs the
  embedding model, and there is no in-test store helper. Known limits, by
  design: a World-book leaf edited *during* a compile can lose to the compiler
  (it owns those leaves); a leaf already open shows its old text until
  re-opened, as before; quitting mid-compile is silent. Also corrected: the
  status line said "5 layers materialized" — it is eleven.

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
