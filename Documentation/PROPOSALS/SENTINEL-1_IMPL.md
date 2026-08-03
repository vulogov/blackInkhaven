# SENTINEL-1 — Implementation Plan (grounded, file-by-file)

*Companion to [`SENTINEL-1_PLAN.md`](SENTINEL-1_PLAN.md). Every anchor verified
against the tree at 2.2.0-dev (two-agent substrate audit). Nothing built.*

## Grounded anchors (the reuse map)

**The deterministic detectors to WRAP (never re-implement):**
- Co-location: `world::timeline_context::co_location_conflicts(&[TlEvent]) ->
  Vec<CoLocationConflict{character,event_a,event_b,place_a,place_b,…}>` —
  `src/world/timeline_context.rs:179`; feed it `gather_events(&hierarchy)` :91.
  (CLI precedent: `realworld co-location`, `src/cli/realworld.rs:463`, which also
  consults the magic-ledger `teleportation` rule — reuse that suppression.)
- Timeline critique: `timeline::critique::run(events, fuzz, min_sig, min_susp,
  cluster_min, staleness) -> CritiqueReport{orphans, overlaps}` —
  `src/timeline/critique/mod.rs:80`; `CritiqueEvent` `types.rs:43`.
- Numeric/direction: `continuity::detect_contradictions(&[Quantity],
  &ContradictionConfig) -> Vec<Contradiction>` — `src/continuity.rs:467`;
  `extract_quantities(&[String], &lex)` :368; `built_in_lexicon(lang)` :135.
- Character-fact drift: `continuity_bible::detect_drift(&bible, lang) ->
  Vec<Drift{character,attribute,conflicts:[(chapter,value)]}>` —
  `src/continuity_bible.rs:157`; `ContinuityBible::load(root)` :90 (reads
  `.inkhaven/continuity.json` — SENTINEL READS it; extraction stays `continuity
  extract`, `src/cli/continuity.rs`).

**Graph + hierarchy substrate (Pillar 2 + Pillar 3):**
- All edges of a kind: `store.raw().edges_of_kind(EdgeKind) -> Vec<Edge>`
  (`src/storage/document.rs:144`, crate-internal; `EdgeStore::by_kind`
  `edge_store.rs:568`). **CT-P0 adds a public `Store::edges_of_kind` wrapper**
  (mirroring `map_edge_err`) — there is none today.
- `Edge{src,dst:EndpointRef,kind,attrs,…}` `edge_store.rs:393`; `EndpointRef::
  {Node(Uuid),Extern(ExternRef)}` :315; `ExternRef::Declared{kind,label}` :311;
  `EventInvolves` role in `attrs["role"]` = "character"|"place"
  (`graph.rs:494–521`, read pattern `graph.rs:389`).
- Timeline `when` lives ONLY on `node.event` (`EventData{start_ticks:i64,
  end_ticks:Option<i64>,precision,characters:Vec<Uuid>,places:Vec<Uuid>,track}` —
  `src/store/node.rs:211`), NOT on the edge — read `node.event` via the
  hierarchy, don't rely on `EventInvolves` for ticks.
- Declared entities: `store.raw().edges_of_kind(Declares)` filtered to
  `src==Node(book)`, `dst==Extern::Declared{kind,label}` (kind ∈ character/
  symbol/motif/tension); or the sources directly — `CharStore::all_declarations`
  (`src/character/store.rs:264`), `MythStore::symbols/motifs`, `UtopiaStore::
  findings`. (Tension labels are positional `#N` — round-trip to UtopiaStore for
  text.)
- Reading order: `Hierarchy::flatten() -> Vec<(&Node,usize)>` preorder,
  order-sorted (`src/store/hierarchy.rs:140`) — the Vec index IS the reading
  position. `slug_path` :333, `children_of` :131, `get` :121.
- Mentions primitive: `crate::drift::mentions(haystack_lc, name_lc) -> bool`
  (`src/drift.rs:196`, Unicode word-boundary). (`Mentions` EDGES are ¶→WordNet
  sense, NOT ¶→entity — do NOT use them for entity mentions; recompute with
  `drift::mentions`.) Prose read from `node.file` (pattern `graph.rs:197`), strip
  heading `typst_prose::strip_leading_heading`.
- System-book roster: Characters/Places entry titles via `SYSTEM_TAG_CHARACTERS`/
  `SYSTEM_TAG_PLACES` (lexicon build precedent `src/tui/lexicon_build.rs:56`).

**Surface plumbing (Pillar 4):**
- Output kinds: add `pub const CONTINUITY: &str = "continuity";` in
  `src/pane/output/types.rs`.
- Review pass: `run_unified_check` `src/tui/app.rs:15394` (+ the `run_*_check`
  pattern, e.g. `run_stylist_check`); CLI mirror `src/cli/check.rs:32`
  (`run_timeline_critique` :187 is the closest shape). Wire BOTH.
- Config: top-level `Config` `src/config.rs:9`; section pattern `ChorusConfig`
  (`config.rs`) / `TheologianConfig` :4585. Existing scattered knobs stay
  (`timeline.critique` `config.rs:5382`); `continuity:` is additive.
- Dashboard modal: clone `Modal::StyleReport` (CH-P8) scaffolding (rows+cursor,
  scrollable; `draw_style_report_modal`).
- Ambient/on-save: the poet/prose ambient pattern (`prose.ambient` /
  `poet_ambient` runtime toggle + cooldown, on-save hook).
- Bund: the BUND-2.2 pattern (`src/scripting/stdlib/chorus.rs` twin) + **classify
  every word in `src/scripting/policy.rs`** (the `every_registered_word_is_
  classified` test enforces it).

**LLM tier to INVOKE, not rebuild (Pillar 4/P7):** `world::fact_check_slow`
COHERENCE (`fact_check_slow.rs:31`, its prompt already names the whole break
taxonomy) + `cli::drift` AI judge. Explicit, cost-capped.

---

## Phase map

### CT-P0 — Substrate (pure)
- `src/store/graph.rs`: `pub fn edges_of_kind(&self, kind: EdgeKind) ->
  Result<Vec<Edge>>` wrapping `self.raw().edges_of_kind(kind)` + `map_edge_err`
  (the audit's gap #1). +1 test.
- New `src/continuity_intel/mod.rs`: `ContinuityFinding{kind:&'static str,
  severity, chapter:u32, anchor:Option<Uuid>, entities:Vec<String>,
  message:String, source:&'static str, dedup_key:String}` + `Severity`
  (reuse the Output severity or a local Info/Warning/Contradiction). Pure; tested
  for `dedup_key` equality.

### CT-P1 — Referenced-before-introduced (the new invariant)
- `src/continuity_intel/introduce.rs`: pure
  `referenced_before_introduced(entities: &[(name, intro_pos)], mentions_by_pos:
  &[(pos, chapter, text)]) -> Vec<ContinuityFinding>` — first-mention position <
  intro position (beyond a tolerance) → finding. Impure `scan(store, layout, h,
  book)` gathers the roster (Characters/Places titles), computes intro positions
  (entry / first scene), walks `flatten()` prose with `drift::mentions`.
- Tests: a name mentioned in ch.2 but introduced in ch.5 flags; introduced-then-
  mentioned does not; Russian names match.

### CT-P2 — The unification engine + `continuity check`
- `src/continuity_intel/engine.rs`: `run(store, cfg, layout, book) ->
  Vec<ContinuityFinding>` = fan out to the adapters (`adapters/{co_location,
  timeline, numeric, char_facts, introduce}.rs`), each a thin map from the native
  finding → `ContinuityFinding`; then `dedupe` (by `dedup_key`) + `rank`.
- CLI: extend `Command::Continuity` (`ContinuityCommand::Check{book, only, skip,
  json}`) — `src/cli/continuity.rs`. Nonzero exit on any Contradiction.
- Tests: the engine dedupes a co-location + travel-time complaint about the same
  pair; ranking orders Contradiction first.

### CT-P3 — The `continuity:` config namespace
- `ContinuityConfig{enabled, ambient, per-detector toggles
  (co_location/timeline/numeric/char_facts/introduce), introduce_tolerance,
  ambient_cooldown_secs}` (`config.rs`, ChorusConfig pattern) on `Config`. Existing
  `timeline.critique`/`editor.echo_*` untouched (the engine reads both). +1 test.

### CT-P4 — The review-pass line
- `kinds::CONTINUITY` (`pane/output/types.rs`).
- `run_continuity_check(store, cfg, layout) -> Result<usize>` (`src/tui/app.rs`,
  the `run_stylist_check` shape): run the engine, clear+emit `kinds::CONTINUITY`
  (Contradiction→Warning, else Info), anchored to `finding.anchor`. Fold into
  `run_unified_check` (+`ct` in the total/status) AND `cli/check.rs`.

### CT-P5 — Watch (incremental / ambient)
- `dirty_scope(store, paragraph_id) -> DirtyScope{characters,places,chapter}` from
  the paragraph's `EventInvolves` neighbourhood + recomputed mentions.
- `run_scoped(engine, scope)` — only the detectors touching the scope
  (co-location for those characters' events; introduce for those entities;
  numeric locally). On-save hook + a `continuity.ambient` runtime toggle +
  cooldown (poet/prose ambient pattern). Emits the delta to Output.
- Tests: `dirty_scope` returns the paragraph's involved characters; scoped run is
  a subset of the full run.

### CT-P6 — The continuity dashboard
- `Modal::ContinuityLedger{rows, cursor}` (clone `Modal::StyleReport`): the ranked
  findings grouped by kind, jump-to-paragraph on Enter (reuse the
  neighbourhood-modal jump). A `Ctrl+B` chord opens it (pick a free meta letter).
  `k` = run the slow coherence pass on the current scope (P7).

### CT-P7 — Invoke the slow passes (reuse, no rebuild)
- From the dashboard / a CLI flag, invoke `fact_check_slow` COHERENCE over a
  scope's paragraph run + the `cli::drift` AI judge — explicit, cost-capped,
  background job (`BgJobKind`), results merged into the ledger as
  `source:"coherence"`/`"drift"` findings. Never automatic.

### CT-P8 — Bund + docs
- `src/scripting/stdlib/continuity.rs`: `ink.continuity.findings` ( -- list ),
  `ink.continuity.check` ( -- dict, summary counts ). Register + **classify in
  `policy.rs`** (STORE_READ). Docs: new `Documentation/CONTINUITY.md`.

### CT-P9 — Capstone
- Tutorial (the unified continuity workflow), KEYBINDING (the dashboard chord),
  README index, CONFIGURATION (`continuity:` block), the multilingual note (each
  detector's coverage + the new invariant everywhere). Verify the "watches
  itself" loop end-to-end on a fixture.

---

## Cross-cutting
- **Advisory / deterministic core / cost informs.** The always-on sweep is
  zero-AI; the slow passes (P7) are explicit + capped.
- **Unify, don't duplicate** — adapters call the existing detectors; SENTINEL owns
  normalisation, dedup, incrementality, and the surface, nothing else.
- **No new crates; warning-free; 1.2.15.**
- **Value core = P1 + P2 + P4 + P5**; P3/P6/P7/P8/P9 are config/surface/docs.

## Open decisions (resolve during CT-P0/P2)
1. **`ContinuityFinding` severity** — reuse Output `Severity` vs a local enum
   (local, mapped at emit, keeps the engine decoupled — lean local).
2. **`dedup_key` shape** — how aggressively to fold co-location vs fact-check
   travel-time about the same pair; start conservative (kind+entities+chapter),
   widen if noisy.
3. **The dashboard chord** — a free `Ctrl+B` meta letter (audit the keymap; the
   graph hub took `z`, stylist rides `J→Y`).
4. **Extract vs read** — SENTINEL *reads* `.inkhaven/continuity.json`; does it
   nudge the author to run `continuity extract` when it's stale/absent (like the
   Book-RAG staleness nudge)? Recommend yes, non-blocking.
