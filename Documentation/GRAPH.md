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
| **Declared** | a declared world entity — a character / symbol / motif / tension |

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
| `declares` | book → declared entity | the book's cast / symbols / motifs / tensions |
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
| `pending` | the advisory (`judged`) edges awaiting triage — the edge inbox |
| `link <node>` | propose stance edges from a fact to its related facts (needs an LLM); triage with `pending` |
| `ask <question>` | answer a question by **walking** the graph — search → query neighbours / contradictions / loci / paths → grounded answer (needs an LLM) |

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

### In the editor

- **`Ctrl+B z`** opens the **graph hub** — press **`n`** for the neighbourhood view
  of the paragraph you're editing (the same tree, scrollable `↑↓`, `Esc` to close),
  **`i`** for the **edge inbox**: the advisory (`judged`) stance edges awaiting
  triage (from confront, `graph link`, and deep research), where **`P`** promotes
  the selected edge and **`d`** rejects it, or **`w`** to **walk the graph** to
  answer the question in the AI prompt (see "Walking the graph" below). Populate
  the graph first with `graph rebuild` / `graph lexical`.
- On a **confront finding** in the Output pane (from `Ctrl+V ?`), **`P`** promotes
  its stance edge to a kept decision (survives `graph rebuild`), and **`d`**
  (dismiss) rejects the finding and deletes its edge.

---

## Chat with your graph

*(GRAPHMIND, 2.x — see [`PROPOSALS/GRAPHMIND-1_PLAN.md`](PROPOSALS/GRAPHMIND-1_PLAN.md))*

The graph is not only queried by verb — you can **converse** with it. Two AI
surfaces ground a language model in the graph's *relations*, not just the prose,
so you can ask how your book connects — what contradicts what, what grounds a
claim, how a scene is sourced — questions the flat manuscript can't answer.

### The **Graph** AI scope (in the editor)

Cycle the AI-pane scope with **F9** to **Graph** (it sits last, after Editor). A
prompt in Graph scope retrieves the passages relevant to your question — the same
semantic retrieval Book scope uses — and, beneath each, folds in the graph edges
touching it (`contradicts` / `sourced_from` / `links_to` / `cites` / …). The
answer is grounded in both the prose *and* those relations, with the same
citation contract as Book scope (each claim cites a passage's `[location/path]`;
invented labels are flagged). It's a **sticky** conversation scope, like Facts:
it retrieves once per chat and re-grounds when you clear history. Press **`p`** in
the AI pane to expand the "Retrieved passages + graph relations" transparency
section and see the subgraph the answer stands on.

### `graph ask` (the traversal loop, on the CLI)

```
inkhaven graph ask "which of my claims about the harbour contradict each other?"
```

Where the Graph scope reads one hop, `graph ask` lets the model **walk** the
graph: it searches for seed nodes, then issues read-only graph queries
(neighbours, contradictions, loci, paths) turn by turn until it can answer,
grounding the answer in what it observed. The exploration transcript prints to
stderr (so you can see the path it took); the answer to stdout (so you can pipe
it). Honest by construction: when the relations don't record what you asked, it
says so rather than inventing a connection — the graph is only as complete as
`graph rebuild` / confront / `graph link` have made it.

The cost of a question is bounded and tunable (the permissive principle — these
inform and cap, they never block):

```hjson
graph: {
  ask_max_steps: 8      // max LLM turns before a forced answer
  ask_search_width: 6   // seed nodes per search
}
```

### Walking the graph in the editor

The same traversal runs **inside the editor**, streamed: type a question in the AI
prompt, then **`Ctrl+B z → w`**. The AI pane shows the walk unfold live — each
step (`🔍 search…`, `🔗 neighbours…`, `⚖ contradicting…`) as the model takes it —
then streams the grounded prose answer, which lands as a normal chat turn. The
status bar shows `graph walk · turn k/N`; **`Esc`** stops the whole walk at any
time. Same `ask_max_steps` / `ask_search_width` bounds as the CLI. It's an
explicit action (a walk is several model calls, unlike the one-hop **Graph**
scope), so the depth — and its cost — is something you opt into per question.

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

## From a script

The graph is scriptable through Bund words (deterministic; the LLM `graph ask`
is not a sync word):

```
ink.graph.stats         ( -- dict )            node/edge counts + per-kind
ink.graph.neighbors     ( node -- list )       a node's one-hop edges
ink.graph.contradicting ( node -- list )       stance clashes touching it
ink.graph.loci          ( node -- list )       the primary-source loci it cites
ink.graph.paths         ( from to -- list|nil ) a bounded citation/link path
ink.graph.pending       ( -- list )            the judged edge inbox
ink.graph.rebuild       ( -- dict )            re-derive structural edges
ink.graph.promote       ( edge -- bool )       judged → promoted
ink.graph.dismiss       ( edge -- )            delete a stance edge
```

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

The graph is a first-class data layer, a CLI surface (`graph` verbs + `graph
ask`), an in-editor surface (the `Ctrl+B z` hub — neighbourhood + edge inbox + the
streamed graph walk — and the confront-finding `P`/`d` keys), and an AI surface
(the **Graph** scope + `graph ask`, on the CLI *and* streamed in-editor via the
hub `w` walk). A `similar_to` materialisation is deliberately *not* done
(embedding similarity stays a live HNSW query). The declared world is imported
(`declares` edges); the Inner-family grounding still reads those live for
freshness rather than off the graph.
