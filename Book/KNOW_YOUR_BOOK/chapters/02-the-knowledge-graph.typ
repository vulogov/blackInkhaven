#import "../design.typ": *

#chapter(number: 2, title: "The Knowledge Graph")

A book is not a list; it is a *web*. Mara is Joren's sister and the war's orphan and
the coronation's reluctant guest; the war connects to the treaty connects to the
famine connects back to Mara. You hold these connections loosely in your head, and
they fray. Inkhaven holds them exactly, in a *graph* — a map of how everything in
your book relates to everything else — and lets you ask it questions.

#term("The graph")[
  Inkhaven's *knowledge graph* is a layer of typed connections over the things in
  your book — paragraphs, facts, events, characters. An edge says *how* two things
  relate: this cites that, this event involves her, this claim contradicts that one.
  You never build it by hand; the tool derives it from what you have already written.
]

#section("What the edges know")

The graph is not decoration — every edge is one the tool can walk. A fact
*contradicts* another; a paragraph *mentions* a character; a claim is *sourced from*
a passage; an event *involves* the people who were there. That last edge is the one
KEN will lean on two chapters from now: the graph already knows who was present when
something happened.

#screen(caption: "inkhaven graph stats — the web, counted")[```
Graph · 1,204 nodes · 3,881 edges
  Mentions ........ 2,140   (paragraph → character / place)
  EventInvolves ...   612   (event → who was there)
  Contradicts .....    18   (fact ⇄ fact)
  Cites ...........   744   (claim → source)
  Declares ........   167   (world.hjson → the fact it fixes)
```]

#section("Ask your book a question")

The graph would be a curiosity if you could only count it. What makes it an
intelligence is that you can *talk to it*. `inkhaven graph ask` takes a plain
question and *walks the graph* to answer it — searching, following neighbours,
chasing contradictions and paths — and it is honest about what it does not record.

#screen(caption: "graph ask — a question, answered by traversal")[```
> inkhaven graph ask "who has reason to want the treaty to fail?"

  Walking: search 'treaty' → neighbours → contradictions …
  ─ Sella loses her claim if the treaty holds (ch. 6, ch. 11).
  ─ The northern lords are bound by it against their oath (ch. 9).
  ─ I have no recorded motive for Joren — the graph is silent there.
```]

Inside the editor the same power is the *Graph* AI scope (F9) and the graph hub
(`Ctrl+B z`): ask a question and the answer is grounded in the passages *and* the
edges touching them, with the evidence one keypress away.

#section("The graph builds itself")

You do not curate this web. Deep research proposes *contradicts* edges when two
findings clash; `inkhaven graph link` suggests connections from a fact to its
nearest kin; and an *edge inbox* (`graph pending`, or `i` in the hub) collects these
proposals for you to keep or reject. The graph grows the way your book does — a
little at a time, mostly on its own.

#two_track(
  [For fiction, the graph is where "how does everyone connect to the murder" stops
  being a note card wall and becomes a question you can ask out loud.],
  [For non-fiction, the graph is the shape of your argument — which claim rests on
  which source, and where two of your own findings quietly disagree.],
)

#recap((
  [The *knowledge graph* is a layer of typed edges over your book — mentions, event
  participants, contradictions, citations — derived from what you wrote.],
  [`inkhaven graph ask` (and the F9 Graph scope / `Ctrl+B z` hub) *walks* the graph
  to answer a plain question, grounded and honest about gaps.],
  [The graph *builds itself* — contradiction and link proposals collect in an edge
  inbox you keep or reject.],
))
