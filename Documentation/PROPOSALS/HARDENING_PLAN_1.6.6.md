# Hardening plan — 1.6.6

Adversarial audit of the WORLD-track code (1.6.0–1.6.5) + cross-cutting robustness
(panics, determinism, DoS, resource limits). Verified findings, ranked. Prior
fixes (BUG-1..16, 1.6.1/1.6.4) re-checked and confirmed holding.

## P1 — reachable panic

- **H1 · UTF-8 boundary panic in `realworld critique`** — `src/world/critique.rs`
  `build_critique_prompt` byte-slices `&hjson[..MAX_HJSON]`. A `world.hjson` over
  8000 bytes whose byte 8000 lands mid-multibyte-char panics ("not a char
  boundary"). Reachable from `realworld critique` (default LLM mode) on any large
  world with non-ASCII names. **Fix:** cut on a char boundary.

## P2 — DoS + determinism

- **H2 · Uncapped `geology.generated.plates` → CPU hang / OOM-abort** —
  `src/world/compile/geology_layer.rs`. `plates: u32` from `world.hjson` has only a
  lower bound (`.max(2)`), no ceiling. `plates: 7000` (typo for `7`) freezes the
  compile for minutes; a huge value aborts the process — now also from the async
  World-overview thread, taking the TUI down. **Fix:** `clamp(2, 64)`; warn in
  `validate` when the declared value was clamped.
- **H3 · Nondeterministic climate-zone order on an `area_pct` tie** —
  `src/world/compile/climate_layer.rs` `aggregate_zones`. Zones built by iterating a
  `HashMap` then sorted on `area_pct` only; equal areas → nondeterministic output
  order, propagating into ecology, plakat regions, and the materialized book —
  breaking the "pure function of (world, seed)" contract. **Fix:** stable secondary
  sort key (`biome`).

## P3 — robustness

- **H4 · Async World-overview worker ignores its cancel flag** — `src/tui/app.rs`
  the `BgJobKind::WorldOverview` worker drops `_cancel`; `cancel_bg_job` sets a flag
  nothing polls, and the single-job harness wedges all bg jobs until a runaway
  compile (H2) finishes. **Fix:** thread the cancel `AtomicBool` into
  `compute_world_overview_rows` and check it between layers.
- **H5 · DEM import has no image size cap** — `src/world/compile/geology_layer.rs`
  `image::open(dem_path)` decodes the whole image before resampling; a huge /
  decompression-bomb DEM OOMs (the BUG-15 class on the DEM path). **Fix:** set
  `image::Limits` before decode.

## Additional findings (recent-WORLD-code pass)

- **H6 · Stale World-overview after accept/reject a proposal** — `src/tui/app.rs`.
  `world_overview_cache` was invalidated only on compile; accepting proposals
  mutates the books but leaves `world.hjson` mtime unchanged → `Ctrl+B W` returns
  pre-accept rows. **Fixed:** invalidate in `refresh_hierarchy_after_world_write`
  (the shared hook for every world write).
- **H7 · Dangling hub references past the landmark cap** — `src/world/plakat.rs`.
  A world with >48 realm capitals drops some `hub_<i>` landmarks but roads still
  referenced them → plakat rejects the spec. **Fixed:** `road_features` now filters
  to hubs actually emitted.
- **H8 · Empty-slug signature collisions** — `myth/ruler/language_proposals.rs` +
  `plakat.rs`. An all-non-ASCII belief/nation/landmark name slugged to `""`, so two
  distinct such names collided (silently dropping one on re-propose; duplicate
  `decl_` ids). **Fixed:** one shared `proposals::stable_slug(name, sep)` with an
  FNV-1a hash fallback when the slug is empty.
- **H9 · Unchecked `flow_dir[ni]` in `trace_source`** — `src/world/plakat.rs`.
  Defensive (invariant holds). **Fixed:** `.get()` + range-guard `0..8` (also
  guards the `DX[nd]` index).
- **H10 · Non-atomic Place commit** — `src/world/commit.rs`. A failed
  `insert_place_link` after `create_place` left the proposal un-accepted → a retry
  duplicated the Place. **Fixed:** the link write is now best-effort (warns,
  doesn't fail the accept).

## Status

All P1/P2 + the actionable P3 fixed with regression tests (H1 multibyte-truncate,
H2 plate cap, H8 slug uniqueness) — suite 2313→2316. **H4** (overview worker
ignores its cancel flag) is defused by **H2**: with the plate count capped the
compile can no longer run away for minutes, so the un-cancellable worker no longer
wedges background jobs; threading a cancel token through the whole compile is
deferred as low-value.

## Round 2 — conlang + research/sources + shared plumbing (all FIXED, +3 tests, suite 2316→2319)

Two more adversarial agents over the subsystems round 1 didn't touch. No new P1 in
research/plumbing (round-1 UTF-8 discipline held); the conlang pass found one.

- **C1 (P1) · UTF-8 panic in the markdown→Typst heading converter** —
  `src/conlang/output.rs`. `trimmed[level..]` panics when an LLM-emitted heading is
  led by a multibyte space (U+00A0 etc.), crashing `grammar-book`/`tutorial`.
  **Fixed:** slice after `trim_start()` (also cures stray `#`s on ASCII-indented
  headings).
- **C2 (P2) · `--meter` unbounded/overflow** — `src/cli/language.rs`. **Fixed:**
  clamp each entry to `1..=64`.
- **C3 (P2) · unbounded `--count`** — `generate/word.rs`, `creative.rs`. **Fixed:**
  `MAX_GENERATE_BATCH = 100_000` cap (lexicon path flows through it).
- **C4 (P2) · exponential rewrite growth** — `phonology/rewrite.rs`. Chained
  empty-context insertion rules blow up every surface derivation. **Fixed:** cap the
  working sequence at `len·64`.
- **C7/C8 (P3) · jaccard >1.0 on repeated tokens** (`translate/memory.rs`, dedup
  with sets) and **semantic_filter index panic** (`generate/lexicon.rs`, `.get()`).
- **R1 (P2) · `resolve_path` mis-resolved a real numeric-hyphen slug** —
  `scripting/stdlib/helpers.rs`. `2024-review` was stripped to `review`, so a script
  `ink.tree.delete` could hit the wrong sibling (data loss). **Fixed:** exact-slug
  match first, prefix-strip only as fallback.
- **R2 (P2) · BibTeX export didn't escape braces** — `sources/mod.rs`. An unbalanced
  `}` corrupted every following entry / broke the build. **Fixed:** escape `{`/`}`.
- **R4 (P2) · swallowed import on-disk write** — `cli/sources.rs`. Reported success
  while the citation was absent from disk (what `check`/`export`/build read).
  **Fixed:** propagate.
- **R6 (P2) · swallowed provenance write** — `research/provenance.rs` + 4 callers.
  A failed sidecar write left a fact committed without provenance under "✓ Inserted".
  **Fixed:** `record` returns `Result`; every caller surfaces the failure.
- **R7 (P2) · script `ink.tree.delete/rename` ignored `protected`** — `scripting/
  stdlib/ink.rs`. A `store_write` script could erase a protected system book.
  **Fixed:** refuse a protected node (or a subtree containing one).
- **R5 (P2) · adapters had no HTTP timeout** — `research/{web,gutenberg,geonames,
  scholarly,wikidata}.rs`. A stalled remote hung the command forever. **Fixed:**
  30s (60s for Gutenberg) timeout on every client.

### Round-2 deferred (documented, lower value)
- **R3** cross-chunk verdict numbering in `/factcheck`/`/undisputed` (needs a
  per-chunk parse restructure); **R8** unbounded conversation replay (cap last-N);
  and the P3 long tail (extract.rs over-capture, verdict-line format drift, corrupt-
  sidecar reset, O(N²) import reload, bibtex `@type\n{` gap, image-node-to-root).
  Tracked for a follow-up pass.

## Deferred / not fixing now

- **silent `record_llm_call` write error** (`realworld.rs`) — cost telemetry only.
- **BUG-15 (EPUB image cap)** — separate import path; out of the WORLD scope of this
  release (though the DEM path of the same class is fixed here, H5).
