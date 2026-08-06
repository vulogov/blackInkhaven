#import "../design.typ": *

#chapter(number: 15, title: "The Knowledge Graph")

Every chapter so far has added *nodes*. A paragraph is a node; so is a chapter,
a book, an image, a script, a fact, a character, a place, a source, a glossary
entry. They all live as one uniform kind of thing — a UUID in one database,
embedded for semantic search — and Inkhaven has quietly stored them that way
since your first project. What it did not store, until now, was the *lines
between them*: a first-class, persisted, typed way to say "this fact
contradicts that source", "this paragraph links to that one", "this word means
this concept". That layer of lines is the knowledge graph, and this chapter is
its complete tour — the shape of an edge, the three ways the graph gets filled,
the `graph` command that reads it, and the two ways you can *talk* to it.

The graph does not replace the manuscript tree or the vector index you met in
the search chapter. It *overlays* them, threading the nodes you already have
into one interrogable whole — so that a question the flat text cannot answer,
like "which of my claims about the harbour contradict each other?", becomes a
walk across the lines instead of a guess.

#callout(label: "You never lose data to the graph")[
  An edge is an annotation *over* your nodes, never a container of them.
  Deleting an edge never touches a paragraph; deleting a node cascades its edges
  away so none is left dangling. And most edges are *derived* — you can throw
  the entire graph away and rebuild it from your manuscript, your sidecars, and
  your installed dictionaries. The graph is powerful precisely because it is
  disposable.
]

#section("The edge — a typed line between two nodes")

Everything in the graph is built from one small object. An *edge* is a directed
or symmetric relation from a source endpoint to a destination endpoint, carrying
a kind and a little metadata about why it exists and how much to trust it.

#term("Edge")[
  A single line in the graph: `(src) --kind--> (dst)`, plus a handful of fields
  — a *kind* (the typed relation), a *weight* (confidence), a *reason* (the
  rationale, when there is one), an *origin* (how it came to exist), and
  kind-specific *attrs*. Symmetric kinds are undirected and are found from
  either end.
]

#screen(caption: "The anatomy of an edge")[```
  (src) --kind--> (dst)    + weight, reason, origin, attrs

  src, dst   the two endpoints (a node, or an extern ref)
  kind       the typed relation: links_to, contradicts, …
  directed   false for symmetric kinds (found either way)
  weight     confidence in [0,1]   (1.0 = asserted)
  reason     the human / LLM rationale, when there is one
  origin     how it arose — & whether rebuild recomputes it
  attrs      kind-specific extras (a locus key, a role, …)
```]

#subsection("Endpoints — nodes, and the things that aren't nodes")

Most endpoints are ordinary *nodes*: a paragraph, a fact, a character, addressed
by its UUID. But an edge sometimes needs to point at something that is *not* a
node and should never become one — you do not want ten thousand citations to
force ten thousand phantom nodes into being. So an endpoint can also be an
*extern reference*: an addressable non-node entity the graph can name without
materialising it.

#screen(caption: "The endpoint kinds an edge can reach")[```
  Node       any manuscript node, by UUID
  Source     a Sources-book entry / @cite key
  Work       an external work id (OpenAlex/arXiv/Wikidata…)
  Locus      a canonical primary-source locus (bible:Jn 3:16)
  Sense      a WordNet synset in a language (en: s-dog)
  Ili        an interlingual index id — cross-lingual pivot
  Grade      a fact-check verdict bucket (inaccurate)
  Evidence   a labelled confront / relate evidence item
  Declared   a declared world entity (character/symbol/…)
```]

Under the hood the two endpoint columns are real, indexed columns — not a JSON
blob — because the *reverse* index is the whole point. Every edge is stored so
that "what points at this node?" is a single indexed query rather than a scan of
the table, which is what makes a one-hop neighbourhood instantaneous no matter
how large the graph grows.

#subsection("Edge kinds — the vocabulary of relations")

An edge's *kind* is the typed relation it encodes, and the vocabulary is fixed
and language-neutral. Each kind has a natural pair of endpoint types and a place
it comes from in the rest of the tool — the kinds are, in effect, the five
implicit ways Inkhaven already encoded relationships, unified into one table.

#screen(caption: "The edge kinds, and where each one comes from")[```
  links_to        ¶ → ¶          the editor's paragraph links
  event_involves  event→char/place   timeline event markers
  sourced_from    fact→source/work   fact provenance
  graded_as       fact → grade       /factcheck verdicts
  contradicts     claim ↔ evidence   confront / /contradict
  in_tension        fact ↔ fact      /relate
  qualifies, agrees      ""          confront / /relate
  cites           work → work        snowball / OpenAlex
  cites_locus     ¶ → locus          @key[locus] citations
  hypernym/hyponym/antonym  sense↔sense  WordNet taxonomy
  translates      sense → ili        cross-lingual pivot
  mentions        ¶ → sense          manuscript↔lexicon
  declares        book → entity      cast/symbols/motifs
  similar_to      ¶ ↔ ¶     embedding similarity (see below)
```]

The last one, `similar_to`, is special: it is deliberately *not* stored. Two
paragraphs are "similar" only in the sense that a live nearest-neighbour query
over the vector index says so this instant, and that answer changes every time
you edit. Materialising it would be a cache that is stale the moment it is
written, so embedding similarity stays a live HNSW query and never lands as an
edge. Everything else in the list is a real, persisted line.

#subsection("Origin and durability — will a rebuild keep it?")

The single most important field on an edge is its *origin*, because origin
answers the question that governs the whole graph's economics: *if I run
`graph rebuild`, does this edge survive?* Your decisions must survive; a
recomputable cache need not.

#screen(caption: "Origins, ordered from most durable to recomputed")[```
  authorial   you asserted it directly   kept, never GC'd
  promoted    a judgement you accepted   kept
  judged      an LLM judgement, advisory  kept until dismiss
  imported    reference data (WordNet…)   its own command
  structural  derived from your fields   RECOMPUTED
  derived     recomputable (similarity)  RECOMPUTED
```]

`graph rebuild` clears and recomputes only the *structural* projection — the
edges it can re-derive from your project's own durable data. Your assertions
(`authorial`), the judgements you have accepted (`promoted`), the LLM
judgements still awaiting your triage (`judged`), and imported reference data
(`imported`, rebuilt by its own command) are all preserved. This is why a
rebuild is safe to run at any time and as often as you like: it can only touch
the part of the graph that is, by definition, a function of your manuscript.

#section("Populating the graph")

A fresh project's graph is empty. Three actions fill it, and between them they
cover the structural spine, the lexical bridge, and the judgements you
accumulate while you work.

#subsection("Structural edges — graph rebuild")

#screen(caption: "Deriving the structural projection")[```
  inkhaven graph rebuild
```]

`graph rebuild` walks your project and derives every *structural* edge from it:
the paragraph links you drew in the editor, the timeline event involvements, the
provenance of each fact, the `/factcheck` verdicts, and the `@key[locus]`
primary-source citations. It is idempotent — it clears the structural projection
and rebuilds it whole — so the discipline is simply to run it whenever the
manuscript has changed enough that you want the graph current. It never touches
your authorial, promoted, judged, or imported edges.

#subsection("The lexical bridge — graph lexical")

#screen(caption: "Importing the WordNet lexical net")[```
  inkhaven wordnet fetch en      # once, per language
  inkhaven graph lexical
```]

`graph lexical` imports the *WordNet lexical bridge* for the project language,
if you have installed one. It links the words your manuscript actually uses to
their senses (the `mentions` edges), lays in the local semantic net between
those senses — `hypernym`, `hyponym`, `antonym` — and rides the interlingual
index with `translates` edges so a concept in one language meets its counterpart
in another. Run `inkhaven wordnet fetch <lang>` first to install the dictionary;
`graph lexical` is idempotent and imported, so it survives a structural rebuild
and is refreshed only by re-running it.

#subsection("Judged edges, live — confront and graph link")

The third source fills the graph *as you work*. The editor's confront
(`Ctrl+V ?`) and `/relate` persist their judged stance as `judged` edges — a
`contradicts`, an `in_tension`, a `qualifies`, an `agrees` — accumulating rather
than repeating, so a second confront of the same material does not re-assert the
first. And `graph link`, below, asks an LLM to propose stance edges from a fact
to its related facts. All of these land as *advisory* `judged` edges: the graph
records them, but they wait for your verdict. You *promote* the ones that stick
and *dismiss* the rest, and the place they wait is the edge inbox.

#term("The edge inbox")[
  The set of advisory `judged` edges awaiting triage — the stance edges proposed
  by confront, by `graph link`, and by deep research, none of them yet accepted.
  You read it with `graph pending` or the `Ctrl+B z` hub's `i` view, *promote*
  what you agree with (it becomes `promoted` and survives rebuilds), and
  *dismiss* the rest (the edge is deleted).
]

#section("The graph command")

The read surface is one command with a verb, mirroring the shape of the rest of
the CLI. Every verb but `link` and `ask` is deterministic and free; those two
need a language model.

#screen(caption: "The graph verbs")[```
  inkhaven graph <verb>

  stats                node + edge counts, per-kind breakdown
  rebuild              (re)derive the structural edges
  lexical              (re)build the WordNet lexical bridge
  neighbors <node>     one-hop neighbourhood as a tree
  contradicting <node> stance clashes touching a node
  loci <node>          the primary-source loci it cites
  paths <from> <to>    a bounded path between two nodes (≤8)
  pending              the judged edge inbox (advisory)
  promote <edge>       judged → promoted (kept across rebuild)
  dismiss <edge>       delete a stance edge
  link <node>          propose stance edges (needs an LLM)
  ask <question>       answer by WALKING the graph (LLM)
```]

#subsection("stats — the shape of the graph at a glance")

`graph stats` is the first thing to run after a rebuild: it reports the node and
edge counts and a per-kind breakdown, so you can see at once how much of the
graph is structure, how much is lexical, and how many judged edges are waiting.
It is the fastest way to answer "did the rebuild actually find anything?"

#subsection("neighbors — the one-hop neighbourhood")

`graph neighbors <node>` is the workhorse read. It renders a node's immediate
neighbourhood as a tree, grouped by kind, with an arrow on each line telling you
its direction — `→` outgoing, `←` incoming, `⇄` symmetric. Large groups
truncate, and the neighbourhood is hard-capped so a hub node with five hundred
`mentions` edges cannot flood the view.

#screen(caption: "graph neighbors — a node's one hop, grouped by kind")[```
  ◆ 003. Quiet hour
  ├─ contradicts (1)
  │    ⇄ evidence fact: The lantern was lit at dusk — §3
  ├─ links_to (2)
  │    ← 002. The tide returns
  │    → 004. A tally of names
  ├─ mentions (500)
  │    → sense en:enoewn-01929162-a
  │    …
  │    … +492 more
```]

#subsection("contradicting, loci, paths — the targeted reads")

Three verbs answer sharper questions. `graph contradicting <node>` pulls only
the recorded stance clashes touching a node — its `contradicts` and `in_tension`
edges, in either direction — which is the query you want when you are hunting
for a fact that fights another. `graph loci <node>` lists the canonical
primary-source loci a node cites, the scholarly companion to `neighbors`. And
`graph paths <from> <to>` finds a bounded citation-or-link path between two nodes
— up to eight hops — and prints it, or tells you plainly when no path exists
within the bound. A path is how you answer "is this claim connected to that
source at all, and by what chain?"

#subsection("promote, dismiss, pending — triaging judgements")

`graph pending` prints the edge inbox — the advisory `judged` stance edges
awaiting your verdict. For each, `graph promote <edge>` accepts it: the edge's
origin becomes `promoted`, and it is thereafter kept across every rebuild.
`graph dismiss <edge>` rejects it, deleting the stance edge outright. These three
are the CLI face of the same triage you can do from the editor's hub, and they
are the only way a machine judgement ever becomes a durable part of your graph —
nothing is promoted without your say-so.

#subsection("link — proposing stance edges from a fact")

`graph link <node>` asks a language model to look at a fact and propose stance
edges from it to its related facts — the contradictions, tensions, and
agreements a human might notice reading the two side by side. It writes them as
`judged` edges into the inbox; you then triage them with `pending` /
`promote` / `dismiss` exactly as with any other judgement. It is the one write
verb that reaches for the LLM, and it never asserts anything directly — its whole
output is advisory by construction.

#subsection("ask — answering by walking the graph")

`graph ask` is the verb that made the graph worth building, and it gets its own
section below, because it is not a read of the graph so much as a *conversation*
with it.

#section("Chatting with your graph")

The graph is not only queried by verb. You can *converse* with it — ground a
language model in the graph's *relations*, not just the prose — so that you can
ask how your book connects: what contradicts what, what grounds a claim, how a
scene is sourced. There are two surfaces for this, and they differ in how far
they let the model travel: one reads a single hop, the other walks.

#subsection("The Graph AI scope — one hop, in the editor")

You met the AI pane's *scopes* in the assistant chapters — the setting, cycled
with `F9`, that decides what the model is grounded in before it answers. The
last scope on the cycle, after Editor, is *Graph*.

#term("The Graph scope")[
  An AI-pane scope (`F9`) that grounds the model in the graph. It retrieves the
  passages relevant to your question — the same semantic retrieval Book scope
  uses — and, beneath each passage, folds in the graph edges touching it
  (`contradicts`, `sourced_from`, `links_to`, `cites`, …). The answer stands on
  both the prose *and* those relations, under the same citation contract as Book
  scope. A *sticky* scope: it retrieves once per chat and re-grounds when you
  clear history.
]

A prompt in Graph scope answers from one hop out — the passages it retrieved and
the edges directly on them. It carries the same discipline as Book scope: every
claim cites a passage's `[location/path]`, and an invented label is flagged
rather than trusted. Press `p` in the AI pane to expand the "Retrieved passages
+ graph relations" transparency section and read the exact subgraph the answer
was built on — the graph never asks you to take its grounding on faith.

#subsection("graph ask — the traversal loop on the CLI")

#screen(caption: "Asking a question the flat text cannot answer")[```
  inkhaven graph ask \
    "which of my claims about the harbour contradict?"
```]

Where the Graph scope reads one hop, `graph ask` lets the model *walk*. It
searches for seed nodes matching your question, then issues read-only graph
queries — neighbours, contradictions, loci, paths — turn by turn, following the
lines wherever they lead, until it has seen enough to answer. The exploration
transcript prints to *stderr* so you can watch the path it took; the grounded
answer goes to *stdout* so you can pipe it. It is honest by construction: when
the relations simply do not record what you asked, it says so rather than
inventing a connection. The graph is only ever as complete as `graph rebuild`,
confront, and `graph link` have made it, and `ask` will not paper over a gap.

The cost of a question is bounded and tunable — this is the permissive
principle, where the caps inform and never block. Two settings govern the walk.

#screen(caption: "Bounding the walk (hjson config)")[```
  graph: {
    ask_max_steps: 8      // max LLM turns before answering
    ask_search_width: 6   // seed nodes per search
  }
```]

#subsection("Walking the graph in the editor — Ctrl+B z")

The same traversal runs *inside* the editor, streamed, through the graph hub.
`Ctrl+B z` opens a small menu onto the graph with three entries, and it is the
one place all of the graph's in-editor surfaces gather.

#term("The graph hub")[
  The `Ctrl+B z` overlay — a three-way menu onto the knowledge graph. Press
  `n` for the *neighbourhood* of the paragraph you are editing (the same tree as
  `graph neighbors`, scrollable, `Esc` to close); `i` for the *edge inbox* (the
  advisory `judged` edges, where `P` promotes and `d` rejects the selected one);
  or `w` to *walk the graph* and answer the question typed in the AI prompt.
  Populate the graph first with `graph rebuild` / `graph lexical`.
]

The `w` walk is the streamed twin of `graph ask`. Type a question into the AI
prompt, then `Ctrl+B z` and `w`. The AI pane shows the walk unfold live — each
step as the model takes it, `🔍 search`, `🔗 neighbours`, `⚖ contradicting` —
and then streams the grounded prose answer, which lands as a normal chat turn.
The status bar reads `graph walk · turn k/N` as it goes, and `Esc` stops the
whole walk at any moment. The bounds are the same `ask_max_steps` and
`ask_search_width` as the CLI. It is deliberately an *explicit* action — a walk
is several model calls, not the single hop of the Graph scope — so the depth,
and its cost, is something you opt into per question.

#subsection("Triaging findings without opening the hub")

One more pair of keys lives outside the hub. When a *confront finding* is
sitting in the Output pane — a stance the fact-checker or continuity watch has
proposed — you can act on its edge in place: `P` promotes the finding's stance
edge to a kept decision that survives a rebuild, and `d` dismisses the finding
and deletes its edge. It is the same promote/dismiss verdict as the inbox, on
the finding where you first meet it.

#chord_table((
  chord_row("F9", "Cycle the AI scope; Graph sits last, after Editor."),
  chord_row("Ctrl+B z", "Open the graph hub (then n / i / w)."),
  chord_row("z then n", "Neighbourhood of the open paragraph (tree, ↑↓, Esc)."),
  chord_row("z then i", "Edge inbox — P promotes, d rejects the selection."),
  chord_row("z then w", "Walk the graph to answer the AI-prompt question."),
  chord_row("P / d", "On an Output-pane confront finding: promote / dismiss its edge."),
  chord_row("p", "In the AI pane (Graph scope): expand the grounding subgraph."),
))

#section("From a script — the ink.graph words")

The graph is scriptable from the embedded Bund language, which you will meet in
full in the scripting part. The read and triage verbs are exposed as
deterministic words; the LLM walk (`graph ask`) is deliberately *not* a sync
word, because a word that fans out to a model is not something a script should
call as if it were pure.

#screen(caption: "The ink.graph Bund surface")[```
  ink.graph.stats         ( -- dict )   counts + per-kind
  ink.graph.neighbors     ( node -- list )   one-hop edges
  ink.graph.contradicting ( node -- list )   stance clashes
  ink.graph.loci          ( node -- list )   cited loci
  ink.graph.paths         ( from to -- list|nil )  a path
  ink.graph.pending       ( -- list )   the judged inbox
  ink.graph.rebuild       ( -- dict )   re-derive structure
  ink.graph.promote       ( edge -- bool )  judged→promoted
  ink.graph.dismiss       ( edge -- )   delete a stance edge
```]

The read words are classified `STORE_READ` and the three that change the graph
— `rebuild`, `promote`, `dismiss` — are `STORE_WRITE`, so the sandbox policy
knows exactly what a script may touch. This is the surface you reach for when you
want to bake a graph check into a headless pipeline: rebuild, read `pending`,
and act on what it returns without opening the editor at all.

#section("Multilingual by construction")

The graph is multilingual because its *kinds* are language-neutral. A
`contradicts` edge between a Russian fact and a French source is the very same
kind of edge as any other — the relation does not know or care what language its
endpoints are written in. Locus canonicalisation collapses `John 3:16`,
`Иоанна 3:16`, and `Joh 3.16` to a single endpoint, so a citation in any
language lands on the same node. And the lexical bridge is cross-lingual by
design: because `translates` edges ride the interlingual index, a Russian sense
and a German sense of one concept meet at the same ILI, and "what is the German
word for this Russian concept?" becomes a reverse-index lookup rather than a
translation call.

#two_track(
  [For a novel with an invented world, the graph's value is the stance edges —
  the `contradicts` and `in_tension` lines confront lays down — and the
  neighbourhood view that shows a scene's links and mentions at a glance.],
  [For non-fiction, the spine is `sourced_from`, `graded_as`, `cites`, and
  `cites_locus`: `graph paths` and `graph ask` let you trace a claim back to the
  source that grounds it, across the whole corpus.],
)

The graph also feeds the *Inner-family readers* you met in the intelligences
part. Their shared grounding now reports what the graph has established that no
single declared-data store computes on its own — recurring character pairings
(who keeps appearing together across your scenes) and the running count of
unresolved factual contradictions. Relational context, in other words, surfaced
where the readers can use it.

#section("Storage and crash-safety")

The graph lives in its own DuckDB store, `edges.db`, sitting beside
`metadata.db`, `blobs.db`, and the `vectors/` directory under your project.
Keeping it separate is deliberate: the graph is a distinct data layer with its
own durability rules, and it can be rebuilt without touching anything else.

#callout(label: "It survives a kill -9")[
  Writes to `edges.db` are atomic — a batch of edges either lands whole or rolls
  back — so the store survives a `kill -9` mid-write and reopens consistent, to
  the 1.2.15 stability bar. Deleting or moving a node cascades its edges away, so
  no edge is ever left pointing at a node that is gone. And the `structural`,
  `derived`, and `imported` edges are provably rebuildable — so even a corrupted
  graph is never *lost data*. Run `graph rebuild` (and `graph lexical`) and it
  comes back.
]

That disposability is the quiet theme of the whole chapter. The graph is
powerful — it holds relations the flat manuscript cannot express, and it lets a
model walk them to answer questions the prose alone cannot — and yet the only
part of it you can truly *lose* is the part you authored: your assertions and the
judgements you chose to promote. Everything else is a function of your book, and
a function can always be recomputed. Build the graph freely; it will never hold
your book hostage.

#recap((
  [The knowledge graph is a *typed-edge* layer over the UUID nodes you already
  have — `(src) --kind--> (dst)` with a weight, a reason, an *origin*, and
  attrs — stored in its own `edges.db` and indexed on *both* endpoints so a
  neighbourhood is a query, not a scan.],
  [An edge's *origin* decides its durability: `authorial`, `promoted`,
  `judged`, and `imported` survive a rebuild; only the `structural` and
  `derived` projections are recomputed — so `graph rebuild` is always safe.],
  [Three actions fill the graph: `graph rebuild` (structural edges), `graph
  lexical` (the WordNet bridge), and *judged* edges accumulated live from
  confront / `/relate` / `graph link` into the *edge inbox*.],
  [The `graph` command reads it — `stats`, `neighbors`, `contradicting`, `loci`,
  `paths`, `pending` — and triages it — `promote`, `dismiss` — while `link` and
  `ask` reach for an LLM.],
  [You can *chat* with the graph: the *Graph* scope (`F9`) grounds an answer in
  one hop of relations, and `graph ask` — on the CLI, or streamed in-editor via
  `Ctrl+B z → w` — lets the model *walk* the graph, honestly saying so when the
  relations do not record what you asked.],
  [`ink.graph.*` scripts the read and triage verbs; the graph is multilingual by
  construction; and `edges.db` is atomic, cascade-safe, and — for everything but
  your own assertions — rebuildable, so it is never lost data.],
))
