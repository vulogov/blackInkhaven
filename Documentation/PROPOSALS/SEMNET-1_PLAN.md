# SEMNET-1 — The Semantic Net (the 2.0 flagship RFC)

> Status: **draft RFC**, 2026-07-31. Target: **2.0.0**.
> Gate: `Documentation/2.0_READINESS.md` (perf + stability) must pass alongside.
> This document is the design; the phase map (§11) is the plan. The grounded,
> file-by-file engineering plan is the companion `SEMNET-1_IMPL.md`.

## 1. Thesis

inkhaven already *is* a knowledge graph — it just doesn't know it yet.

Every book, chapter, paragraph, image, script, fact, character, place, source and
glossary entry is one uniform `Node` (UUIDv7, `src/store/node.rs:70`), stored in one
DuckDB `json_docs` table, embedded twice into one HNSW index (`{uuid}:meta`,
`{uuid}:content`), and reachable by semantic search. The **vertices of a knowledge graph
are already present, already embedded, already multilingual.**

What is missing is **edges as a first-class, persisted, bidirectional, typed thing.**
Today relations live in *five mutually-incompatible encodings* (§2), most of them either
forward-only, string-keyed, or thrown away the moment they're computed. The richest
relations we produce — "fact A *contradicts* source B, cross-source, because …",
"this claim is *in tension* with that evidence", a citation snowball neighbourhood — are
rendered to the screen and discarded.

**SEMNET-1 introduces one thing: a typed-edge layer over the nodes that already exist.**
`(src, kind, dst, attributes)`, persisted in DuckDB beside the metadata it annotates,
with a reverse index and a traversal API. Everything else in this RFC is *migration* —
promoting the five implicit encodings into that one explicit layer — and *surfacing* —
letting the editor, the Inner-family readers, and the research assistant query the graph
they've been building blind.

This is the 2.0 flagship because it is not a new feature bolted on; it is the connective
tissue that makes the features already shipped (facts, provenance, contradiction, stance,
timeline, worldbuilding, WordNet, the Inner family) into a single interrogable whole.

## 2. Motivation — the graph we already have, in five broken pieces

From the substrate audit (2026-07-31). Edges exist today as:

1. **Tree edges** — `parent_id` + `order`, first-class, indexed (`src/store/hierarchy.rs`).
   *These are fine and stay as they are.* The graph does not replace the hierarchy; it
   overlays it.
2. **Intra-node UUID lists** — `Node.linked_paragraphs` (¶→¶, `node.rs:165`),
   `EventData.characters` / `.places` (¶→entity, `node.rs:211`). Typed only by which
   field they occupy; **forward-only, no reverse index, no relation metadata** (no reason,
   no confidence, no provenance).
3. **Sidecar side-tables keyed by one node UUID** — provenance
   (`node → SourceRecord`, `src/research/provenance.rs`, `.inkhaven/fact-sources.json`),
   verdicts (`node → Verdict`, `src/research/verdicts.rs`,
   `.inkhaven/fact-verdicts.json`). One endpoint is a node; **the other endpoint (a
   source, a grade) is not a node at all** — it can't be traversed to.
4. **Ephemeral LLM-judged relations** — `Relation { stance: Stance, reason }` and
   `Clash { a, b, reason }` (`src/research/contradiction.rs`), `FactConflict`
   (`src/facts_scan.rs`), citation snowball neighbourhoods (`src/research/snowball.rs`
   over OpenAlex `referenced_works` / `cited_by`). These carry the **richest semantics we
   have** — Contradicts / Tension / Qualifies / Agrees / Silent, cross-source vs
   within-source — and are **computed on demand and discarded**, at most flattened into
   one Output-pane message anchored to a single paragraph (`emit_confront_findings`,
   `tui/app.rs`).
5. **A real typed graph, walled off** — WordNet: synset/sense vertices, typed `Rel`
   edges (`hypernym`/`hyponym`/`antonym`/…), cross-lingual by ILI
   (`src/wordnet/mod.rs`). A complete semantic net that lives in
   `<data_dir>/inkhaven/wordnet/<lang>.wn` and is **entirely disconnected from any
   manuscript node.**

The cost of this fragmentation:
- **No "what points at this?"** query exists. To find everything that cites, contradicts,
  or references a fact, we full-scan.
- **Judged relations don't accumulate.** Run `/relate` twice, get the work done twice; the
  editor never builds a memory of what's been established.
- **The Inner family re-derives context by hand.** `inner_grounding::build_grounding`
  (`src/inner_grounding.rs`) already joins characters + myth + world + timeline into a
  prose prefix — a hand-rolled, one-book, prose-only precursor to exactly the query the
  graph would answer generically.

## 3. Principles & non-goals

Binding project constraints this RFC inherits and must not violate:

- **No new storage engine, no external-binary dep.** The edge store is a DuckDB table in
  the *existing* per-project store, built on `src/storage/engine.rs`. (Aligns with the
  standing "crate deps OK, NO external-binary deps" rule.)
- **Derived-and-rebuildable where possible; durable where not.** Like the HNSW index, any
  edge that can be recomputed from node content (embedding-similarity edges, tree edges)
  is a cache. Edges that encode a *decision* (a judged stance, a promoted provenance, an
  authorial link) are durable source-of-truth and must survive `kill -9`
  (atomic writes via `crate::io_atomic`, the 1.2.15 bar).
- **Advisory, never authorial.** The graph annotates; it never edits prose. Consistent
  with the AI-advisory rule — findings/edges write to stores and the Output pane, prose
  changes stay user-initiated with confirmation.
- **Multilingual by construction.** Endpoints are nodes whose content is any of
  en/ru/fr/de/es (and beyond); edge *kinds* are language-neutral enums; any LLM that
  proposes edges keys its prompt off project language; WordNet bridging uses ILI, the
  interlingual join. The graph must answer "what contradicts this?" identically whether
  the fact is written in Russian or French.
- **Permissive.** Cost caps on edge-proposing LLM passes inform, never block. No feature
  gate beyond security.
- **Warning-free, no panic surfaces.** Standard bar.

**Non-goals for 2.0:**
- Not a general-purpose triplestore / SPARQL engine. The query surface is the handful of
  traversals the product actually needs (§7), not an open query language.
- Not automatic prose rewriting from graph inferences.
- Not a distributed / multi-user graph. Single-writer, single-project, same as today's
  store.
- Not replacing the tree, the hierarchy API, or the HNSW index. It overlays them.

## 4. The model — `EdgeStore`

### 4.1 The edge

```
Edge {
    id:        Uuid,          // UUIDv7, stable identity for the edge itself
    src:       EndpointRef,   // where the relation starts
    dst:       EndpointRef,   // where it points
    kind:      EdgeKind,      // typed, language-neutral (see 4.3)
    directed:  bool,          // false for symmetric kinds (Agrees, SiblingSense)
    weight:    f32,           // confidence / strength in [0,1]; 1.0 = asserted
    reason:    Option<String>,// human/LLM rationale (the `Relation.reason` we throw away)
    origin:    EdgeOrigin,    // how this edge came to exist (see 4.4)
    created_at: i64,
    // small typed attribute bag for kind-specific data (locus string, ILI, stance
    // sub-label, cross_source flag) — JSON, mirrors how Node carries `tags`/`event`.
    attrs:     serde_json::Value,
}
```

### 4.2 Endpoint identity — `EndpointRef`

The central design problem (§2 items 3–5) is that not every endpoint is a node. Sources,
external works, loci, and WordNet senses are referenced by strings today. SEMNET gives
each a *stable reference* without forcing a full `Node` into existence:

```
enum EndpointRef {
    Node(Uuid),               // the common case — any manuscript node
    Extern(ExternRef),        // an addressable non-node entity
}

enum ExternRef {
    Source { book_node: Uuid, key: String },   // a Sources-book entry / @cite key
    Work   { registry: Registry, id: String }, // OpenAlex / arXiv / Wikidata id
    Locus  { scheme: String, canonical: String },// canonical primary-source locus
    Sense  { lang: String, synset: String },   // a WordNet synset/sense
    Ili    { id: String },                     // interlingual index (cross-lingual pivot)
}
```

`Extern` endpoints are **not** stored as nodes — they're value-typed addresses. This keeps
the node table clean (a bibliography of 10 000 OpenAlex works does not become 10 000
manuscript nodes) while still letting an edge *point at* them and letting a query
*group by* them. Where an extern entity already has a node (a Sources-book paragraph),
prefer `Node(uuid)`; `Extern::Source` carries the book-node + key so the two can be
reconciled.

### 4.3 `EdgeKind` — the taxonomy

Language-neutral, closed enum (open string escape hatch only via `attrs` sub-labels).
Grouped by the encoding they replace:

| Group | Kind | Directed | Replaces |
|---|---|---|---|
| **Authorial link** | `LinksTo` | ✓ | `Node.linked_paragraphs` |
| **Timeline** | `EventInvolves` (→char/place) | ✓ | `EventData.characters`/`.places` |
| **Provenance** | `SourcedFrom` (fact→source/work) | ✓ | `provenance.rs` sidecar |
| **Assessment** | `GradedAs` (fact→verdict-value) | ✓ | `verdicts.rs` sidecar |
| **Stance** | `Contradicts`, `InTension`, `Qualifies`, `Agrees` | see below | `Relation`/`Clash`/`FactConflict` |
| **Citation** | `Cites` (work→work) | ✓ | snowball / OpenAlex |
| **Bibliographic** | `CitesLocus` (node→locus) | ✓ | `index_locorum.rs` |
| **Lexical** | `Hypernym`, `Hyponym`, `Antonym`, `Synonym`, `Translates` (ILI) | mixed | WordNet `Rel` bridge |
| **Semantic (derived)** | `SimilarTo` (embedding cosine) | ✗ | ad-hoc `search_text` |

Stance directionality mirrors `contradiction.rs`: `Contradicts` / `InTension` are
symmetric (store `directed=false`); `Qualifies` is directed (A qualifies B); `Agrees` is
symmetric. The existing `Stance` enum maps 1:1 (`Silent` is simply "no edge"). The
`is_against` / `is_support` helpers become queries over kind sets.

### 4.4 `EdgeOrigin` — how an edge exists (trust, not just history)

```
enum EdgeOrigin {
    Authorial,     // the user asserted it (a manual link) — highest trust, never GC'd
    Structural,    // derived from node fields on migration (linked_paragraphs, event) — durable
    Promoted,      // a judged LLM relation the user accepted (a former ephemeral Relation)
    Judged,        // an LLM relation not yet accepted — advisory, lower weight
    Derived,       // recomputable (SimilarTo, tree) — a rebuildable cache, may be GC'd
    Imported,      // WordNet / OpenAlex bridge — reference data
}
```

`origin` is what lets the graph hold both durable decisions and cheap advisory suggestions
in one table without conflating them: `Derived` edges are a cache (droppable, rebuildable,
like HNSW); `Authorial`/`Structural`/`Promoted` are source-of-truth (atomic, survive
crash). A `Judged` edge is the durable form of what we throw away today — it persists a
`/relate` result as advisory until the user promotes or dismisses it.

## 5. Storage

One new DuckDB table in the per-project store (`src/storage/`), beside `metadata.db` /
`blobs.db` / `vectors/`:

```sql
edges (
    id            TEXT PRIMARY KEY,
    src_kind      TEXT,   -- 'node' | 'source' | 'work' | 'locus' | 'sense' | 'ili'
    src_ref       TEXT,   -- uuid or the extern address
    dst_kind      TEXT,
    dst_ref       TEXT,
    kind          TEXT,   -- EdgeKind
    directed      BOOLEAN,
    weight        REAL,
    reason        TEXT,
    origin        TEXT,   -- EdgeOrigin
    attrs         JSON,
    created_at    BIGINT
)
```

Indexed both ways: `(src_kind, src_ref)` and `(dst_kind, dst_ref)` — **the reverse index
is the whole point** (§2: "what points at this?"). A `_inkhaven_schema`-style version
anchor, matching the existing engine convention.

**Consistency & the 1.2.15 bar:**
- Writes atomic (`io_atomic` / DuckDB txn), no swallowed `Result`s, poison recovery on the
  lock — same standard the vector-sync fix (`storage/vector.rs`) just re-asserted.
- `origin=Derived` edges are provably rebuildable: a `graph rebuild` verb re-derives
  SimilarTo (from HNSW) and Structural (from node fields), so the durable edges are a
  strict subset that a corruption can't silently lose.
- Deleting/moving a node cascades: edges with a dangling `Node` endpoint are GC'd on the
  next graph pass (no orphaned edges — the "no orphaned nodes after delete/move" gate item
  extends to edges).

## 6. Migration — promoting the five encodings

Each is a *one-way lift* into the edge table; the original stays as the write path until
its phase cuts over, so nothing breaks mid-migration.

1. **`linked_paragraphs` → `LinksTo`** (`origin=Structural`). Reverse index gives, for the
   first time, "what links *to* this paragraph".
2. **`event.characters` / `.places` → `EventInvolves`** (`Structural`). Enables "every
   scene this character appears in" as a traversal, not a scan — and feeds the timeline.
3. **Provenance sidecar → `SourcedFrom`** (`Structural`/`Promoted`). `SourceRecord.origin`
   (model/web/wikidata/openalex/arxiv/…) becomes the edge's `attrs.source_origin`. The
   sidecar `.inkhaven/fact-sources.json` becomes a projection *of* the graph.
4. **Verdict sidecar → `GradedAs`** (`Judged`, promotable). `Level` (Accurate/Dubious/
   Inaccurate) + `reason` ride the edge. `/factcheck` writes edges; the trust-ladder UI
   reads them.
5. **`Relation`/`Clash`/`FactConflict` → stance edges** (`Judged`). This is the headline
   win: `/relate`, `Ctrl+V ?` confront, and the facts scan **persist** their judgements as
   `Contradicts`/`InTension`/`Qualifies`/`Agrees` edges with `reason` and the
   cross-source flag in `attrs`. `emit_confront_findings` still posts the Output message,
   but now also writes the edge, so the assessment accumulates.
6. **Snowball / OpenAlex → `Cites`** (`Imported`). A citation neighbourhood, currently
   rendered-and-discarded, is stored as `Work`→`Work` edges — the latent citation graph
   becomes durable and queryable.
7. **`index_locorum` → `CitesLocus`** (`Structural`). `@key[locus]` citations become
   node→`Locus` edges; the Index Locorum export becomes a graph query.
8. **WordNet bridge → lexical edges** (`Imported`). Two levels: (a) intra-WordNet `Rel`s
   become `Hypernym`/`Antonym`/… edges between `Sense` endpoints; (b) the bridge to the
   manuscript — a paragraph's salient lemmas link to their `Sense`, and cross-lingual
   `Translates` edges ride the ILI. This is what finally connects the walled-off semantic
   net to the manuscript.

## 7. Query & traversal API

Deliberately small — the traversals the product needs, not a query language:

```
neighbors(ep, kinds, dir) -> Vec<Edge>          // one hop, filtered by kind + direction
incoming(ep, kinds) / outgoing(ep, kinds)       // the reverse-index queries
paths(a, b, kinds, max_hops) -> Vec<Path>       // bounded BFS (e.g. citation chains)
group_by_endpoint(kind) -> Map<EndpointRef,…>   // "all facts sourced from work X"
contradicting(fact) -> Vec<(Edge, EndpointRef)> // is_against kinds, both directions
subgraph(seed, radius, kinds) -> Graph          // for the graph view + Inner grounding
```

`contradicting`, `sourced_from`, `cites`, `involves` are thin named wrappers — the same
ergonomics as `book_rag::retrieval` today. Traversals are bounded (max-hops, node-budget)
so a pathological graph can't hang the UI, mirroring the token-budgeting in RAG.

## 8. Surfacing — where the graph shows up

The graph is invisible until it's surfaced. Four consumers, all reusing existing UI spines:

1. **Output pane** (`src/pane/output/`) — already the universal node-anchored advisory
   channel (`source_paragraph_id`). Stance/verdict/provenance edges post here as they do
   now; the new capability is **actions on the message that traverse** ("show the 3 other
   facts this contradicts").
2. **A graph/neighbourhood view** — a focused subgraph around the current node
   (`subgraph(current, radius=1..2)`): its sources, what it contradicts, what links to it,
   its timeline neighbours. Rendered with the existing monospace `screen()` helper (the
   design decision from the POETRY book — no `bob-draw`, terminal-native), so it stays
   warning-free and portable.
3. **CLI + chords** — `graph neighbors <node>`, `graph contradicting`, `graph rebuild`,
   `graph stats`; an in-editor chord to open the neighbourhood view.
4. **The Inner family** — `inner_grounding::build_grounding` is **rewritten as a graph
   query.** Its hand-rolled join of characters+myth+world+timeline becomes
   `subgraph(book, kinds=[EventInvolves, …])`, and every reader (editor, socrates,
   theologian, rigor, poet) can ask "what does the graph already know about this
   paragraph?" instead of re-deriving it. This is the clearest proof the graph earns its
   keep: it deletes hand-written join code.

## 9. Multilingual

- Edge *kinds* are enums, language-independent. A `Contradicts` edge between a Russian fact
  and a French source is the same kind as any other.
- LLM edge-proposers (stance, fact-conflict) inherit the existing per-language prompt
  discipline (`feedback_multilingual`): the prompt and any word-lists key off project
  language; Unicode-aware throughout.
- The lexical layer is cross-lingual *by design*: `Translates` edges ride ILI, so
  "what's the German synset for this Russian sense" is a two-hop `Sense →Ili→ Sense`
  traversal — the WordNet `lookup_with_pivot` path (`wordnet/mod.rs:148`) generalised into
  the graph.
- Acceptance test (from the readiness gate): the same "what contradicts this?" query
  returns equivalent structure for the en/ru/fr/de/es variants of a fixture fact.

## 10. Stability & performance

- **1.2.15 bar:** durable edges atomic + crash-safe; derived edges rebuildable; poison
  recovery; no swallowed persist `Result`s; no orphaned edges after node delete/move.
- **Perf:** the edge table is small (edges ≪ words). The reverse index makes the queries
  that are full-scans today O(log n). A new **`graph` bench** joins the harness
  (`Documentation/2.0_READINESS.md`): edge-insert throughput + neighbour-query latency at
  10k/50k-node fixtures, gated like the others. `subgraph`/`paths` are hop- and
  budget-bounded so the UI can't hang.
- **Rebuild cost:** `graph rebuild` re-derives the `Derived` subset; measured and
  documented, must be linear in nodes.

## 11. Phase map

Each phase ships behind the feature working end-to-end; no half-migrations left in a
release. Version numbers are indicative (1.x runway → 2.0 cut).

- **SEMNET-P0 — EdgeStore substrate.** The `edges` table + `Edge`/`EndpointRef`/`EdgeKind`/
  `EdgeOrigin` types + atomic CRUD + reverse index + `graph rebuild`/`graph stats`. No
  migration yet; pure foundation + tests (crash-safety, rebuild determinism).
- **SEMNET-P1 — Structural lift.** Migrate `linked_paragraphs` + `event.characters/places`
  → `LinksTo`/`EventInvolves`. First reverse-index queries. Cutover the write path.
- **SEMNET-P2 — Provenance & verdicts.** `SourcedFrom` + `GradedAs`; sidecars become graph
  projections. `/factcheck` + trust-ladder read/write edges.
- **SEMNET-P3 — Stance persistence (headline).** `/relate`, confront, facts-scan persist
  `Contradicts`/`InTension`/`Qualifies`/`Agrees` edges (`Judged`), with promote/dismiss.
  `contradicting()` query + Output-pane traverse actions.
- **SEMNET-P4 — Bibliographic.** `CitesLocus` (index_locorum) + `Cites` (snowball/OpenAlex)
  durable. Citation-chain `paths()`.
- **SEMNET-P5 — Lexical bridge.** WordNet `Rel`s + manuscript-lemma→`Sense` + cross-lingual
  `Translates` via ILI. The walled-off net joins.
- **SEMNET-P6 — Surfacing.** The neighbourhood view (`screen()` render), `graph` CLI verbs,
  editor chord.
- **SEMNET-P7 — Inner-family rewrite.** `inner_grounding` becomes a graph query; readers
  consume the graph. Delete the hand-rolled join.
- **SEMNET-P8 — Graph bench + gate.** `graph` bench in the harness; multilingual
  round-trip + crash/rebuild in the stability audit. **This closes the 2.0 gate.**
- **SEMNET-P9 — Capstone.** Companion-book chapter + `Documentation/` docs; `/graph`
  cheat-sheet entry.

## 12. Risks & open questions

- **Judged-edge noise.** Persisting every LLM `/relate` judgement could bury the durable
  graph in low-confidence advisory edges. Mitigation: `origin=Judged` + `weight`; the
  default views show `Authorial`/`Structural`/`Promoted` only, Judged on request; a GC/
  cooldown like the Inner-editor's `editor_cooldown_state`.
- **Extern-endpoint reconciliation.** When does an `Extern::Source` become a `Node`?
  Proposal: it stays extern until the user promotes the source into the Sources book; then
  a one-time rewrite repoints edges. Needs a stable identity map — open.
- **WordNet scale.** Importing full synset graphs per language is large. Decision: import
  **lazily** — only senses reachable from manuscript lemmas + their one-hop relations, not
  the whole `<lang>.wn`. Keeps the per-project graph bounded.
- **Do derived `SimilarTo` edges belong in the store at all,** or should similarity stay a
  live HNSW query? Leaning: *don't* persist SimilarTo except as an optional materialised
  cache — the HNSW index already answers it. Keep `SimilarTo` in the taxonomy but P-defer
  its materialisation.
- **Symmetric-edge storage.** Store one row with `directed=false` and query both ways, vs
  two directed rows. Leaning one-row + reverse index handles both directions.

## 13. What 2.0 becomes

With SEMNET the standalone features collapse into one interrogable fabric: a fact carries
its sources, its grade, and every claim it agrees or clashes with, in *any* project
language; a character is the set of scenes that involve it; a citation is a walkable chain;
a word is a node in a cross-lingual sense net bridged to the prose that uses it; and the
Inner family reasons over the graph instead of re-deriving it. The vertices were always
there. 2.0 is the edges.
