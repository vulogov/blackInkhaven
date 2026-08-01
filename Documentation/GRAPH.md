# The Semantic Net (Knowledge Graph)

*(2.0, RFC SEMNET-1 — see [`PROPOSALS/SEMNET-1_PLAN.md`](PROPOSALS/SEMNET-1_PLAN.md)
and [`PROPOSALS/SEMNET-1_IMPL.md`](PROPOSALS/SEMNET-1_IMPL.md))*

Inkhaven has always been a knowledge graph — it just didn't know it. Every book,
chapter, paragraph, image, script, fact, character, place, source and glossary
entry is one uniform **node** (a UUIDv7), stored in one DuckDB table, embedded for
semantic search. What was missing was **edges**: a first-class, persisted,
bidirectional, *typed* way to say "this fact contradicts that source", "this
paragraph links to that one", "this word means this concept".

The **semantic net** is that edge layer. It doesn't replace the manuscript tree
or the vector index — it *overlays* them, connecting the nodes you already have
into one interrogable whole.

> **You never lose data to the graph.** Edges are an annotation over your nodes.
> Deleting an edge never touches a paragraph; deleting a node cascades its edges
> away. Most edges are *derived* — you can throw the whole graph away and rebuild
> it from your manuscript, sidecars, and installed dictionaries.

---

## The model

An **edge** is `(src) --kind--> (dst)`, plus metadata:

| Field | Meaning |
| ----- | ------- |
| `src`, `dst` | the two **endpoints** (see below) |
| `kind` | the typed relation (`links_to`, `contradicts`, `sourced_from`, …) |
| `directed` | `false` for symmetric kinds (found from either endpoint) |
| `weight` | confidence / strength in `[0,1]` (1.0 = asserted) |
| `reason` | the human/LLM rationale (the "why", when there is one) |
| `origin` | how the edge came to exist — and its durability (see below) |
| `attrs` | kind-specific extras (a locus key, a cross-source flag, a role) |

### Endpoints

Most endpoints are **nodes** (a paragraph, a fact, a character). Some point at an
**external** entity that isn't a node — so an edge can reference it without
forcing 10 000 citations to become 10 000 nodes:

| Endpoint | Example |
| -------- | ------- |
| **Node** | any manuscript node, by UUID |
| **Source** | a Sources-book entry / `@cite` key |
| **Work** | an external work id in a registry (OpenAlex / arXiv / Wikidata / GeoNames) |
| **Locus** | a canonical primary-source locus (`bible: John 3:16`) |
| **Sense** | a WordNet synset in a language (`en: s-dog`) |
| **Ili** | an interlingual index id (the cross-lingual pivot) |
| **Grade** | a fact-check verdict bucket (`inaccurate`) |
| **Evidence** | a labelled confront/relate evidence item |

### Edge kinds

| Kind | Between | Comes from |
| ---- | ------- | ---------- |
| `links_to` | ¶ → ¶ | the editor's paragraph links |
| `event_involves` | event → character / place | timeline event markers |
| `sourced_from` | fact → source / work | fact provenance |
| `graded_as` | fact → grade | `/factcheck` verdicts |
| `contradicts`, `in_tension`, `qualifies`, `agrees` | claim ↔ evidence / fact ↔ fact | confront / `/relate` / `/contradict` |
| `cites` | work → work | citation graphs (snowball / OpenAlex) |
| `cites_locus` | ¶ → locus | `@key[locus]` primary-source citations |
| `hypernym`, `hyponym`, `antonym` | sense → sense | WordNet taxonomy |
| `translates` | sense → ili | cross-lingual pivot |
| `mentions` | ¶ → sense | the manuscript↔lexicon bridge |
| `similar_to` | ¶ ↔ ¶ | embedding similarity (a live HNSW query; not materialised) |

### Origin & durability

An edge's `origin` records how it came to exist — **and whether a rebuild will
recompute it**:

| Origin | Meaning | Survives `graph rebuild`? |
| ------ | ------- | ------------------------- |
| `authorial` | you asserted it directly | **yes** (never GC'd) |
| `promoted` | a judgement you accepted | **yes** |
| `judged` | an LLM judgement, advisory until you promote it | **yes** (until dismissed) |
| `structural` | derived from your node fields / sidecars / citations | recomputed |
| `imported` | reference data (WordNet, external citations) | **yes** (rebuilt by its own command) |
| `derived` | recomputable from content (similarity) | recomputed |

`graph rebuild` clears and recomputes only the **structural** projection —
everything it can rebuild from your project's own durable data. Your decisions
(`authorial` / `promoted`), pending judgements (`judged`), and imported reference
data (`imported`) are preserved.

---

## Populating the graph

The graph starts empty. Three actions fill it:

1. **`inkhaven graph rebuild`** — derives the *structural* edges from your project:
   paragraph links, timeline event involvements, fact provenance, `/factcheck`
   verdicts, and `@key[locus]` citations. Idempotent — run it whenever the
   manuscript changes.
2. **`inkhaven graph lexical`** — imports the **WordNet lexical bridge** for the
   project language (if installed): links the words your manuscript uses to their
   senses, the local semantic net (hypernym / hyponym / antonym), and the
   cross-lingual ILI. Run `inkhaven wordnet fetch <lang>` first. Idempotent.
3. **Confront, live** — the editor's `Ctrl+V ?` confront (and `/relate`) persist
   their judged stance as `judged` edges as you work — accumulating, so a second
   confront doesn't repeat the first. Promote the ones that stick; dismiss the rest.

---

## The `graph` command

```
inkhaven graph <verb>
```

| Verb | What it does |
| ---- | ------------ |
| `stats` | node + edge counts and a per-kind breakdown |
| `rebuild` | (re)derive the structural edges (see above) |
| `lexical` | (re)build the WordNet lexical bridge for the project language |
| `neighbors <node>` | the node's one-hop neighbourhood as a tree — what it links to, contradicts, is sourced from, cites, and the senses it mentions |
| `contradicting <node>` | the recorded stance clashes touching a node (`contradicts` / `in_tension`, either direction) |
| `loci <node>` | the primary-source loci a node cites |
| `paths <from> <to>` | a bounded citation / link path between two nodes (≤ 8 hops) |
| `promote <edge>` | accept a `judged` stance edge → `promoted` (kept across rebuilds) |
| `dismiss <edge>` | delete a stance edge |

### `graph neighbors` — the neighbourhood view

```
◆ 003. Quiet hour
├─ contradicts (1)
│    ⇄ evidence fact: The lantern was lit at dusk — opposes §3
├─ links_to (2)
│    ← 002. The tide returns
│    → 004. A tally of names
├─ mentions (500)
│    → sense en:enoewn-01929162-a
│    …
│    … +492 more
```

Direction arrows: `→` outgoing, `←` incoming, `⇄` symmetric. Large groups
truncate; the neighbourhood is hard-capped so a hub node can't flood the view.

---

## Multilingual

The graph is multilingual by construction. Edge *kinds* are language-neutral — a
`contradicts` edge between a Russian fact and a French source is the same kind as
any other. Locus canonicalization collapses `John 3:16`, `Иоанна 3:16`, and
`Joh 3.16` to one endpoint. The lexical bridge is cross-lingual by design:
`translates` edges ride the interlingual index, so a Russian sense and a German
sense of one concept meet at the same ILI — "what's the German word for this
Russian concept?" is a reverse-index lookup on that ILI.

---

## Where it shows up

Beyond the `graph` command, the graph feeds the **Inner-family readers**: their
shared grounding now also reports what the *graph* has established — recurring
character pairings (who keeps appearing together across your scenes) and the count
of unresolved factual contradictions — relational context no single declared-data
store computes.

---

## Storage & safety

The graph lives in `edges.db` — its own DuckDB store beside `metadata.db`,
`blobs.db`, and `vectors/` under your project. Writes are atomic (a batch either
lands whole or rolls back); the store survives a `kill -9` mid-write and reopens
consistent. Deleting or moving a node cascades its edges away, so no edge is ever
left pointing at a node that's gone. The `derived` / `structural` / `imported`
edges are provably rebuildable, so a corrupted graph is never lost data — just run
`graph rebuild` (+ `graph lexical`).

---

## Not yet wired

The graph is complete as a data layer and a CLI surface. Still to come (net-new,
not regressions):

- an **in-editor neighbourhood chord** + promote/dismiss buttons on the confront
  Output messages (the graph is fully usable from the CLI today);
- the live **snowball → `cites`** persistence seam (the citation-chain `paths`
  query is ready for it);
- importing **character arcs / myth symbols / world tensions** as edges (which
  would let the Inner-family grounding derive *those* from the graph too).
