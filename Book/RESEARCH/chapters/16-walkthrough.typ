#import "../design.typ": *

#chapter(number: 16, title: "One Fact, End to End")

You have met every tool in this book one at a time. This chapter runs them as one
workflow. We will follow a single subject — a Roman aqueduct — from a first
question to a grounded, cross-checked, cited fact, and on into a piece of writing.
Two authors make the journey side by side: a *novelist* who needs the aqueduct to
feel real in a scene, and a *historian* who needs a claim about it to be
defensible in an argument. Same tools, same order; two destinations.

#section("Stage 1 — Ask")

Open the Research Assistant, start a thread for the topic, and ask a plain
question.

```
How much water did a major Roman aqueduct deliver per day?
```

The answer arrives grounded on whatever you have already kept — which, on a new
project, is not much, so it leans on the model. It gives you a figure. That figure
is currently a guess: the bottom rung.

#two_track(
  [You need the *scale* to feel right in a crowd scene — enough to sense the city's
   thirst, not a number you will print.],
  [You need the *figure itself*, with evidence, because it anchors a paragraph of
   your argument.],
)

#section("Stage 2 — Climb to a real source")

Do not keep the guess. Go up the ladder. The aqueduct is a real thing, so try the
structured and scholarly rungs:

```
/wikidata Aqua Claudia
/openalex Roman aqueduct hydraulic capacity
```

Wikidata hands you the identified entity and its properties; OpenAlex hands you a
paper with a daily-capacity figure and a DOI. Now you have a claim that starts far
higher than the model's guess — and the OpenAlex result, when you keep from it,
will file its own citation into your Sources book.

#section("Stage 3 — Keep it")

Take the fact, and confirm it at the gate.

```
/fact Aqua Claudia delivered roughly 190,000 cubic metres of water per day
```

Because you grounded on a scholarly source, the fact lands cited — provenance
`openalex`, DOI attached — and its citation is now in Sources without another
keystroke.

#section("Stage 4 — Cross-check the load-bearing claim")

This figure is going to carry weight, so do not trust a single source. Triangulate
it:

```
/triangulate Aqua Claudia delivered about 190,000 cubic metres per day
```

The Assistant queries the structured and scholarly sources at once and reports the
agreement. Suppose it comes back with one clear `SUPPORTS` and no `CONTRADICTS` —
the claim is corroborated across independent references, and you keep it knowing it
was tested from more than one side.

#two_track(
  [For the novelist, this is where you stop. The number will never appear on the
   page; you needed to know the *order of magnitude* is real so the scene's sense
   of scale is honest. One check is plenty.],
  [For the historian, one supporting source is a beginning. You might also run
   `/upgrade` on any related guesses, and turn the refutation gate on so a
   determined sceptic pass runs before each new claim commits. The figure is going
   in a footnoted paragraph; it should survive attack as well as find support.],
)

#section("Stage 5 — Audit, before anyone else does")

Chapters later, your corpus has grown. Before a beta read or a submission, sweep
it:

```
/factcheck
```

The tree lights up with verdict glyphs. A ✗ appears on a travel-time fact you kept
early — it contradicts the aqueduct's dates. You select it and ask `/whatswrong`,
which explains the clash, and you fix the wording. The contradiction that a sharp
reader — or a referee — would have caught, you caught first.

#section("Stage 6 — Compose it back out")

Now use what you built. Ask the corpus to organise itself:

```
/synthesize the water supply of imperial Rome
/gaps the water supply of imperial Rome
```

`/synthesize` returns a cited overview, drawn only from your verified facts, honest
about where it is thin. `/gaps` lists what is still missing — and those questions
go into a file you hand to a headless `--batch` run overnight, so the holes fill
themselves while you sleep.

#two_track(
  [`/outline` the scene grounded in your facts, copy it into the manuscript, and
   write — the aqueduct's scale, the city's thirst, the engineering, all resting on
   ground you verified, none of it slowing the prose.],
  [`/outline` the section, each point already citing its source; write the
   argument; then `/bibliography` collects every citation you accrued into a
   references list — the deliverable assembling itself from the provenance you kept
   all along.],
)

#section("What just happened")

Trace the arc. You *acquired* a fact, climbing from a guess to a cited scholarly
claim. You *cross-checked* it against independent sources. You *maintained* the
corpus, catching a contradiction in an audit. And you *composed* it back out — a
synthesis, an outline, a bibliography — into the book you are writing. Every fact
you kept remembered where it came from; nothing was fact-checked that you invented;
your prose was never touched by the tool.

That is the whole discipline, and it is not really about commands. It is about a
single habit made effortless: *never let a borrowed claim rest on nothing, and
always remember where it came from.* Do that, and a reader can trust the facts you
borrowed — which is what lets them believe the world you invented.

The appendices that follow are for reference: every command in one place, the
provenance rungs and their glyphs, and a glossary of the terms this book defined.
Keep them at your elbow. The workflow you now have in your hands is small, honest,
and yours.

#recap((
  [The whole workflow in order: *ask → climb to a real source → keep → triangulate
   → audit → compose*.],
  [A novelist and a non-fiction author walk the same path to different ends — a
   world that *feels* solid, and an argument that *is* defensible.],
  [Grounding is not about commands; it is one habit made effortless: never let a
   borrowed claim rest on nothing, and always record where it came from.],
  [Because the borrowed facts can be trusted, the reader can believe the invented
   ones — which is the entire point.],
))
