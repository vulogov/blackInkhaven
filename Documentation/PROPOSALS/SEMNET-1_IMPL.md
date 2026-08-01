# SEMNET-1 — Implementation plan

> Companion to the RFC `Documentation/PROPOSALS/SEMNET-1_PLAN.md`. Grounds every phase
> against the actual code, with the exact files/functions to touch and the tests to write.
> Drafted 2026-07-31 on `1.10.2-dev`. Phases fold into 1.x releases; **P0–P8 must all land
> before the 2.0.0 cut** (P8 closes the `2.0_READINESS.md` gate).

## Reality check — the RFC's assumptions vs. the code

The substrate audit (2026-07-31) confirms the RFC is buildable as written; four points make
it *smaller* than a from-scratch graph, and two decisions get pinned here.

1. **A new store is a copy of `JsonStorage`.** The whole DuckDB layer is one file,
   `src/storage/engine.rs`. `StorageEngine::new(path, init_sql, pool_size)` (engine.rs:31)
   + a `const *_INIT_SQL` + a thin wrapper type (`JsonStorage` engine.rs:324 is the
   template) is the entire recipe for a new table. It already ships: `transaction()`
   (engine.rs:154, the atomic/rollback primitive), `checkpoint()` (:183),
   `integrity_check()` (:199), `ensure_schema_version()` (:346, the `_inkhaven_schema`
   anchor). **EdgeStore adds no new dependency and no new engine** — it's a third wrapper
   beside `JsonStorage`/`BlobStorage`.

2. **The wiring point is exact and singular.** Sub-stores are opened in
   `DocumentStorage::with_embedding(root, engine, pool)` (`src/storage/document.rs:44`)
   over paths from `Paths::from(root)` (document.rs:281). `root` is the project store root
   (`Store::open` passes `layout.store_root()`, store/mod.rs:196). Adding `edges.db` is:
   one field on `DocumentStorage` (document.rs:31), one line in `Paths` (`root.join("edges.db")`),
   one `EdgeStore::new(...)` in the constructor. That's the whole integration surface.

3. **The cascade-GC precedent already exists.** `delete_subtree` (store/mod.rs:1784)
   already calls `scrub_linked_paragraphs(&deleted)` (mod.rs:1805/1825) which reloads the
   hierarchy after a delete and prunes stale `linked_paragraphs` + `event.characters/places`
   refs from every surviving node. **This is both the migration *source* for P1 and the
   template for edge-GC** — a `scrub_edges(&deleted)` sits right beside it, same shape.

4. **`Edge` is a `Node`-shaped type.** `Node` (store/node.rs:69) is the serde model to
   mirror: `#[derive(Debug,Clone,Serialize,Deserialize)]`, `#[serde(default)]` on every
   non-core field (forward-compat with old on-disk rows), hand-rolled `to_json()`
   (node.rs:257, Uuid→string, DateTime→rfc3339) / `from_json(id, &value)` (node.rs:289,
   id is the row key, not in the doc, malformed UUIDs dropped defensively). Ids are minted
   in the storage layer via `Uuid::now_v7()` (document.rs:65), not by callers.

### Pinned decisions

- **Storage shape: a dedicated `edges.db` with a *relational* `edges` table, not
  `json_docs`.** The RFC's edge is typed columns (`src_kind/src_ref/dst_kind/dst_ref/
  kind/directed/weight/reason/origin/attrs/created_at`) indexed **both ways** — that is not
  the `json_docs` shape (id+key+document blob). So P0 adds a new `EDGE_INIT_SQL` + a new
  wrapper `EdgeStore` (new file `src/storage/edge_store.rs`, mirroring `JsonStorage`), with
  `attrs` as a `JSON` column (DuckDB supports it — `json_docs.document` is `JSON`). Its own
  `_inkhaven_schema` version anchor, its own pool. Rationale: the reverse index (§RFC 5) is
  the feature; it needs real indexed columns, and a separate file keeps a crash isolated to
  the derived-ish edge layer (no cross-db txn with metadata — same doctrine as vectors).
- **Two-layer split, mirroring node.rs vs engine.rs.** Domain types + traversal API live in
  `src/store/graph.rs` (`Edge`, `EndpointRef`, `EdgeKind`, `EdgeOrigin`, the query methods);
  DuckDB persistence lives in `src/storage/edge_store.rs`. `Store` gets a `graph` submodule
  (`pub mod graph;` at store/mod.rs:2, sibling to `node`/`hierarchy`) and delegates.

## Cross-cutting invariants (hold in every phase)

- **1.2.15 bar:** durable edges written under `transaction()` (engine.rs:154); no swallowed
  `Result`s; poison recovery on the pool; `integrity_check()` wired into `inkhaven doctor`.
  Mirror the named tests: `data_survives_checkpoint_and_reopen` (engine.rs:611),
  `transaction_commits_and_rolls_back` (engine.rs:492), `checkpoint_drains_wal_after_write`
  (engine.rs:559).
- **Rebuildable ≠ durable.** `origin ∈ {Authorial, Structural, Promoted}` = source-of-truth,
  survives `kill -9`. `origin ∈ {Derived, Imported}` = cache, dropped+recomputed by
  `graph rebuild` (doctrine copied from `VectorEngine::sync_in_background`, vector.rs:201).
- **No orphaned edges.** Every node-endpoint delete cascades via `scrub_edges` beside
  `scrub_linked_paragraphs` (mod.rs:1825). Test: delete a node, assert its edges are gone
  and no dangling `Node(uuid)` endpoint remains.
- **Multilingual:** edge *kinds* are enums; any LLM edge-proposer keys its prompt off
  project language (the `feedback_multilingual` rule); acceptance test per phase runs the
  en/ru/fr/de/es fixture.
- **Advisory, never authorial:** edges annotate; prose changes stay user-initiated. `Judged`
  edges are suggestions until promoted.

---

## SEMNET-P0 — EdgeStore substrate  *(target 1.11.x)*

The foundation. No migration, no UI — a persisted, indexed, crash-safe edge table + the
domain types + CRUD + rebuild/stats, fully tested. Everything else builds on this.

**New files:**
- `src/store/graph.rs` — domain types. `Edge`, `EndpointRef` (`Node(Uuid)` |
  `Extern(ExternRef)`), `ExternRef` (`Source/Work/Locus/Sense/Ili`), `EdgeKind` (closed
  enum, RFC §4.3 table), `EdgeOrigin` (RFC §4.4). Each enum: `#[serde(rename_all="lowercase")]`
  + hand `as_str`/`from_str` (mirror `NodeKind` node.rs:8-55). `Edge::to_json`/`from_json`
  mirror `Node` (node.rs:257/289). `EndpointRef` serializes to the `(kind, ref)` string pair
  the table stores (`Node(u)`→`("node", uuid)`, `Extern(Source{book,key})`→`("source",
  "{book}:{key}")`, etc.) — one `fn as_columns(&self) -> (&str, String)` + `fn from_columns`.
- `src/storage/edge_store.rs` — `EdgeStore` wrapper over `StorageEngine`, `const
  EDGE_INIT_SQL` (the relational table + `idx_edges_src`/`idx_edges_dst`/`idx_edges_kind` +
  `_inkhaven_schema`). Methods: `new(path, pool)`, `insert(&Edge)` / `insert_batch(&[Edge])`
  (one `transaction()`), `by_id`, `outgoing(kind,ref,kinds)`, `incoming(kind,ref,kinds)`,
  `delete(id)`, `delete_endpoint(kind,ref)` (cascade), `delete_by_origin(EdgeOrigin)` (for
  rebuild), `all`, `count`, `checkpoint`, `integrity_check`. All parameterized
  (`execute_with`/`select_all_with`, engine.rs:96/139).

**Wiring (edits):**
- `src/storage/document.rs`: add `edges: EdgeStore` field (:31); open in `with_embedding`
  (:44) from a new `Paths.edges_db = root.join("edges.db")` (:281). Add pass-through methods
  (`add_edge`, `edges_out`, `edges_in`, `delete_edges_for`, `rebuild_derived_edges`,
  `edge_count`).
- `src/store/mod.rs`: `pub mod graph;` (:2). `impl Store` delegators: `add_edge`,
  `neighbors`, `incoming`, `outgoing`, `graph_rebuild`, `graph_stats`. Extend `checkpoint()`
  (:755) + `integrity_check()` (:765) + `sync()` (:739) to cover edges.
- `src/cli/mod.rs`: user-facing `Graph { #[command(subcommand)] cmd: GraphCommand }` (mirror
  `Wordnet` :581/:3006/:6284) with verbs `Stats`, `Rebuild` (P0 usable); hidden
  `#[command(hide=true, name="_bench-graph")] BenchGraph { edges: usize }` (mirror
  `_bench-render` :1463). New `src/cli/graph.rs` + `src/cli/bench_graph.rs`
  (`run(project: &Path, ...) -> Result<()>`).

**Tests** (in `edge_store.rs`, mirroring engine.rs conventions — `TempDir`, `Uuid::now_v7()`,
assert-on-reopen):
- `edge_survives_checkpoint_and_reopen` (← engine.rs:611).
- `insert_batch_commits_and_rolls_back` (← engine.rs:492): a batch with a bad row leaves no
  half-state.
- `reverse_index_finds_incoming` — insert `A→B`, assert `incoming(B)` yields it and
  `incoming(A)` doesn't.
- `kind_filter_and_symmetric_direction` — `directed=false` edge found from both endpoints.
- `graph_rebuild_is_deterministic` — build, drop `Derived`, rebuild, assert identical set.
- `from_json_drops_malformed_endpoint` (← node.rs defensive parse).

**Cut:** `inkhaven graph stats` prints node/edge/kind counts; `graph rebuild` is a no-op
(nothing derived yet) but exercises the path; `_bench-graph` inserts N edges + times a
neighbour query. Warning-free, tests green.

## SEMNET-P1 — Structural lift  *(target 1.11.x/1.12.x)*

Migrate the two intra-node UUID-list encodings into edges; add the cascade GC.

- **`linked_paragraphs` → `LinksTo`** (`origin=Structural`), **`event.characters/.places` →
  `EventInvolves`** (`Structural`). A migration pass `graph_rebuild` now *derives these from
  node fields* (they remain the write path this phase — no data moves, edges are a
  projection), so rebuild is lossless and idempotent. Source fields: `Node.linked_paragraphs`
  (node.rs:165), `Node.event` (node.rs:203, `EventData` :211).
- **Cascade GC:** add `scrub_edges(&self, deleted: &[Uuid])` beside `scrub_linked_paragraphs`
  (store/mod.rs:1825), call it in `delete_subtree` (mod.rs:1805). Deletes any edge with a
  `Node(uuid)` endpoint in `deleted`.
- First reverse-index product: `neighbors(node, [LinksTo], Incoming)` = "what links *to*
  this paragraph" (never existed before); `outgoing(char_id,[EventInvolves],Incoming)` =
  "every scene involving this character".

**Tests:** `structural_edges_match_node_fields` (derive == fields); `delete_cascades_edges`
(delete node → its edges gone, no dangling endpoint); `rebuild_idempotent_after_edit`.

## SEMNET-P2 — Provenance & verdicts  *(target 1.12.x)*

- **Provenance sidecar → `SourcedFrom`**, **verdict sidecar → `GradedAs`**. Sources:
  `research/provenance.rs` (`.inkhaven/fact-sources.json`, `SourceRecord{origin,…}`) and
  `research/verdicts.rs` (`.inkhaven/fact-verdicts.json`, `Verdict{level,reason}`). Endpoint:
  fact-node → `Extern::Source`/`Extern::Work` (from `SourceRecord.origin`), fact-node →
  verdict-value (`attrs.level` = Accurate/Dubious/Inaccurate). `origin=Structural` for
  imported sidecar rows, `Promoted` when the user accepts.
- **Sidecars become projections *of* the graph** (dual-write this phase, then the sidecar is
  a read-through cache; cutover deferred to avoid a flag day). `/factcheck` + trust-ladder UI
  read edges.

**Tests:** `provenance_roundtrips_to_edges` (sidecar→edges→sidecar lossless, incl. the full
`origin` vocab); `verdict_edge_carries_level_and_reason`; multilingual fact fixture.

## SEMNET-P3 — Stance persistence (headline)  *(target 1.12.x/1.13.x)*

The win: judged relations stop being thrown away.

- **`Relation`/`Clash`/`FactConflict` → stance edges** (`Contradicts`/`InTension`/`Qualifies`/
  `Agrees`, `origin=Judged`). Sources: `research/contradiction.rs` (`Stance` enum :258,
  `Relation` :303, `Clash` :51), `facts_scan.rs` (`FactConflict`). `Contradicts`/`InTension`/
  `Agrees` stored `directed=false`; `Qualifies` directed. `reason` + the cross-source flag
  ride the edge (`attrs.cross_source`, from `Clash::is_cross_source` contradiction.rs:60).
- **Write seam:** `emit_confront_findings` (tui/app.rs:7264) already turns each non-`Silent`
  `Relation` into an Output message; it now *also* writes the edge. `/relate`, `Ctrl+V ?`
  confront, and the facts scan all persist. Promote/dismiss actions on the Output message
  flip `Judged`→`Promoted` or delete.
- **`contradicting(fact)`** query (both directions over `is_against` kinds) + Output-pane
  "show the N other facts this clashes with" traverse action.
- **Judged-noise control** (RFC §12 risk): default views show
  `Authorial/Structural/Promoted` only; `Judged` on request; a cooldown mirroring
  `editor_cooldown_state` (inner_editor/storage.rs).

**Tests:** `confront_persists_stance_edge`; `stance_symmetry` (Contradicts found both ways);
`promote_and_dismiss_transitions`; `cross_source_flag_preserved`; ru/fr fixture.

## SEMNET-P4 — Bibliographic  *(target 1.13.x)*

- **`index_locorum` → `CitesLocus`** (`Structural`): `@key[locus]` citations
  (`sources::extract_cite_loci`) → node→`Extern::Locus{scheme,canonical}` edges; the Index
  Locorum export (`src/index_locorum.rs`) becomes a `group_by_endpoint(CitesLocus)` query.
- **Snowball/OpenAlex → `Cites`** (`Imported`): `research/snowball.rs` neighbourhoods
  (`referenced_works`/`cited_by`) → `Work`→`Work` edges, durable. `paths(a,b,[Cites],max)` =
  citation chains.

**Tests:** `locus_edges_regroup_to_index` (edges reproduce the current Index Locorum output);
`citation_path_bounded` (max-hops respected); scripture-canonicalization multilingual
(`Jn 3.16`/`иоанна 3:16` collapse — reuse `research::scripture::canonical_bible_book`).

## SEMNET-P5 — Lexical bridge  *(target 1.13.x/1.14.x)*

Connect the walled-off WordNet net.

- **WordNet `Rel` → lexical edges** between `Sense` endpoints (`Hypernym`/`Hyponym`/
  `Antonym`/`Synonym`, `origin=Imported`). Source: `wordnet/mod.rs` (`Rel{rel_type,target}`
  :71, `Synset`/`Sense`).
- **Manuscript bridge:** a paragraph's salient lemmas → `Sense` endpoints; **lazy import**
  (RFC §12) — only senses reachable from manuscript lemmas + one hop, never the whole
  `<lang>.wn`.
- **Cross-lingual `Translates`** via ILI (`Sense →Ili→ Sense`), generalising
  `lookup_with_pivot` (wordnet/mod.rs:148).

**Tests:** `wordnet_rel_becomes_edge`; `lazy_import_is_bounded` (unreferenced synsets absent);
`cross_lingual_two_hop` (ru sense → de sense via ILI).

## SEMNET-P6 — Surfacing  *(target 1.14.x)*

- **Neighbourhood view:** `subgraph(current, radius=1..2)` rendered with the terminal-native
  `screen()` monospace helper (**not** bob-draw — the standing design decision, keeps it
  warning-free). Shows sources, contradictions, in-links, timeline neighbours of the current
  node.
- **`graph` CLI verbs:** `neighbors <node>`, `contradicting`, plus `stats`/`rebuild` from P0
  (extend `GraphCommand`). Editor chord to open the view.

**Tests:** `subgraph_radius_bounded`; snapshot the `screen()` render for a small fixture.

## SEMNET-P7 — Inner-family rewrite  *(target 1.14.x)*

- **`inner_grounding::build_grounding` (src/inner_grounding.rs) becomes a graph query.** Its
  hand-rolled join of characters+myth+world+timeline → `subgraph(book, kinds=[EventInvolves,
  …])`. Every reader (editor/socrates/theologian/rigor/poet) gains "what does the graph
  already know about this paragraph?" instead of re-deriving context.
- **Proof-of-value:** this phase *deletes* hand-written join code. Keep the old function's
  output shape (a prose prefix) so readers don't change; only the derivation moves to the
  graph. Regression: golden-output test that the graph-derived grounding matches the old
  hand-rolled one on the fixture.

**Tests:** `grounding_from_graph_matches_legacy` (golden); reader consumes edges without
prose edits.

## SEMNET-P8 — Graph bench + gate  *(target 1.14.x → the 2.0 cut)*

- **`benches/graph.rs`** (+ `[[bench]]` in Cargo.toml, `harness=false`) driving hidden
  `_bench-graph`: edge-insert throughput + neighbour-query latency at 10k/50k-node fixtures.
  Add it to the gated set in `.github/workflows/bench.yml` (deterministic + offline, like
  render/scale/export).
- **Stability audit additions** (`2.0_READINESS.md` checklist): graph crash/`kill -9`
  mid-write → opens consistent (edges.db WAL/atomic); `graph rebuild` provably reconstructs
  the `Derived`/`Imported` subset; multilingual "what contradicts this?" returns equivalent
  structure across en/ru/fr/de/es; no orphaned edges after delete/move soak.
- **This phase closes the 2.0 gate** — with P0–P7 shipped and the graph bench + audit green,
  `2.0_READINESS.md` is fillable end to end.

## SEMNET-P9 — Capstone  *(2.0.0)*

Companion-book chapter (the graph story), `Documentation/` reference (`GRAPH.md` +
KEYBINDING/cheat-sheet entries for the `graph` verbs + chord), `/graph` in the CLI docs.
Ships with the 2.0.0 release ritual.

---

## Dependency order & parallelism

P0 blocks everything. P1 (structural + GC) and P2 (provenance/verdicts) are independent and
can interleave. P3 depends on P0 only (its sources are self-contained) but reads best after
P2 (shared fact-node endpoints). P4/P5 are independent leaf migrations. P6 needs P1–P3 to
have something worth viewing. P7 needs P1 (+ ideally P3). P8 needs everything. Each phase is
**cut with its own tests and folds into a 1.x release** — the graph is usable and correct at
every step, never half-migrated in a shipped build (the dual-write-then-cutover discipline in
P1/P2 guarantees this).

## What stays out (non-goals, from RFC §3)

No triplestore/SPARQL; no auto prose rewriting; no multi-user graph; the tree/hierarchy/HNSW
are overlaid, not replaced; `SimilarTo` stays a live HNSW query (materialisation deferred —
RFC §12). No new external-binary dep; no new engine (EdgeStore is a `StorageEngine` wrapper).
