# Tutorial 104 — The Knowledge Graph

*Inkhaven 2.0*

Inkhaven has always *been* a knowledge graph — every book, chapter, paragraph,
fact, character, place, and source is one uniform node. What was missing was the
**edges**: a first-class, persisted, typed way to say "this fact contradicts
that source", "this paragraph links to that one", "this word means this concept".
SEMNET is the edge layer. It overlays the nodes you already have and connects
them into one interrogable whole — and you never lose data to it (edges annotate
your nodes; most are derived and rebuildable).

## Build the graph

The graph starts empty. Derive the structural edges from your project:

```sh
inkhaven graph rebuild
inkhaven graph stats
```

```
Graph — "The Drowned City"
  nodes 1,204   edges 3,881
    links_to        212     sourced_from    140
    event_involves  96      graded_as       58
    cites_locus     44      declares        61
    contradicts     7       …
```

`rebuild` is idempotent — run it whenever the manuscript changes. It derives
paragraph links, timeline event involvements, fact provenance, `/factcheck`
verdicts, and `@key[locus]` citations. To bring in the WordNet lexical bridge
(if you've installed a dictionary — see Tutorial 109):

```sh
inkhaven graph lexical
```

## Walk a node's neighbourhood

Every node knows what it connects to. Grab a paragraph's id (the status bar shows
it, or `inkhaven search`) and look around it:

```sh
inkhaven graph neighbors <node-id>
```

```
◆ 003. Quiet hour
├─ contradicts (1)
│    ⇄ evidence fact: The lantern was lit at dusk — opposes §3
├─ links_to (2)
│    ← 002. The tide returns
│    → 004. A tally of names
├─ sourced_from (1)
│    → source: Aldous, "Harbour Records"
```

Direction arrows: `→` outgoing, `←` incoming, `⇄` symmetric. Other verbs:
`contradicting <node>` (just the stance clashes), `loci <node>` (the
primary-source loci it cites), and `paths <from> <to>` (a bounded
citation/link path between two nodes, ≤ 8 hops).

## In the editor — the graph hub

Press **`Ctrl+B z`** to open the graph hub for the paragraph you're editing:

- **`n`** — the neighbourhood view (the same tree, scrollable).
- **`i`** — the **edge inbox**: advisory (`judged`) stance edges awaiting your
  decision, where **`P`** promotes an edge (kept across rebuilds) and **`d`**
  rejects it.

Stance edges accumulate as you work: the editor's `Ctrl+V ?` confront and
`/relate` persist their judged stance, so a second confront doesn't repeat the
first. Promote the ones that stick; dismiss the rest.

## Durability — you never lose data

An edge's `origin` records how it came to exist and whether a rebuild recomputes
it. Your decisions (`authorial` / `promoted`), pending judgements (`judged`), and
imported reference data (WordNet, citation registries) all survive
`graph rebuild`; only the `structural` projection is recomputed. The graph lives
in its own `edges.db` beside your other stores — writes are atomic and survive a
`kill -9` mid-write.

---

**See also:** [GRAPH.md](../GRAPH.md) · Tutorial 105 (chat with your graph) ·
`inkhaven graph --help`.
