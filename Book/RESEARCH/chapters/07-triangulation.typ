#import "../design.typ": *

#chapter(number: 7, title: "Cross-Checking a Claim")

A single source can be wrong. A reputable web page can repeat a myth; a paper can
be superseded; even a structured database can carry a stale value. The most
reliable thing you can do with a load-bearing claim is not to find a *better*
single source — it is to ask *several independent sources at once* and see whether
they agree. That is triangulation, and it is the heart of this part.

#section("The idea: agreement, not authority")

When three surveyors fix a point from three directions, no single line locates it
— the *intersection* does. A claim works the same way. A fact that Wikidata, a
scholarly paper, and a preprint all support — with none of them contradicting it —
is far firmer than any one of them alone, because for it to be wrong, several
independent records would all have to be wrong in the same way.

#triangulate_diagram()

#term("Triangulation")[
  *Triangulation* is testing a claim by gathering evidence from several
  *independent* sources at once and judging whether they agree. The verdict is not
  "a source says so" but "the sources *concur*" — which is a much stronger thing
  to be able to say.
]

#section("The `/triangulate` command")

Point `/triangulate` at a claim — or run it bare to test the last answer in the
conversation:

```
/triangulate the aqueduct carried about a million cubic metres of water per day
```

The Assistant queries Wikidata and the two scholarly indexes — OpenAlex and arXiv
— in one background pass, gathers what each one says, and then judges the evidence
— reporting, per source, whether it *supports*, *contradicts*, or is *silent* on
the claim, and a bottom line of how many concur.

#screen(caption: "/triangulate — three sources, one verdict")[```
> /triangulate the aqueduct carried about a million
  cubic metres of water per day

Wikidata: SILENT — no capacity statement for this entity
OpenAlex: SUPPORTS — the cited work gives a daily figure
arXiv:    SILENT — the preprint models flow, not capacity
Agreement: 1/3 support
```]

The important discipline: the model here is judging *external evidence*, not
grading its own earlier answer. That is what makes the verdict mean something.
"The sources I can reach don't corroborate this" is a genuinely useful thing to
learn *before* you commit a claim your argument will rest on.

#callout(label: "Silence is information")[
  A `SILENT` source has not failed — it simply has nothing to say about this
  particular claim. Triangulation only counts *support* and *contradiction*; a
  claim that no source can speak to is one you have learned you cannot yet ground
  by cross-checking, which is worth knowing on its own.
]

#section("Triangulation as the gate")

You can run `/triangulate` by hand whenever a claim feels shaky. But you can also
make it *automatic*. Turn on the setting `research.triangulate_gate`, and from
then on every `/fact` drawn from a low rung — a model guess, a web page, an
imported document — is triangulated *before it commits*. Cross-source agreement
becomes the gate.

At the confirmation, a claim that the sources *support with no contradiction*
inserts normally. A claim that is *contradicted* or that *no source corroborates*
shows you the agreement summary and asks you to confirm again — the same
inform-never-block principle you met with the web fact-check, now backed by
multiple independent references instead of one. And because the structured and
scholarly facts (`wikidata`, `geonames`, `openalex`, `arxiv`) are already
authoritative, they skip the gate entirely — there is nothing to cross-check about
a datum that arrived cited.

#two_track(
  [Reserve triangulation for the *load-bearing* facts — the date the reader could
   catch, the distance the plot depends on. You don't need three sources for the
   colour of a market awning; you very much want them for the thing a sharp reader
   will test.],
  [Make triangulation the default. Turn the gate on and let every borrowed claim
   face cross-source agreement before it enters your argument. A claim that
   survives triangulation is one you can footnote with confidence.],
)

#section("What triangulation cannot do")

Triangulation asks *who agrees*. It is a corroboration test — it looks for
support. That is powerful, but it has a blind spot: a plausible-sounding false
claim that the sources happen to be *silent* on will pass through uncorroborated
rather than caught, and a claim can be *agreed upon and still wrong* if the
sources share a common error.

So the next chapter turns from checking one claim to auditing the *whole* Facts
book — looking for internal contradictions across everything you have kept — and
the chapter after that adds the mirror image of triangulation: a check that does
not ask who agrees, but actively tries to *disprove* the claim. Corroboration and
refutation are two different questions, and a fact you can lean on has answered
both.

#recap((
  [*Triangulation* tests a claim against several *independent* sources at once and
   judges *agreement*, not mere authority.],
  [`/triangulate` queries Wikidata and the two scholarly indexes (OpenAlex, arXiv)
   in one pass and reports each as SUPPORTS / CONTRADICTS / SILENT, with a tally.],
  [The model judges *external evidence*, not its own answer — which is what makes
   the verdict trustworthy; `SILENT` is information, not failure.],
  [`research.triangulate_gate` makes cross-source agreement the automatic gate for
   low-rung `/fact`s; structured/scholarly facts skip it.],
  [Triangulation asks *who agrees* — it can't catch an agreed-upon error or a
   plausible claim the sources are silent on; refutation (Chapter 9) is its
   mirror.],
))
