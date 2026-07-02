#import "../design.typ": *

#chapter(number: 9, title: "Strengthening What You Keep")

Two chapters ago, triangulation left us with a promise: it asks *who agrees*, and
a claim can slip through simply because the sources are silent on it. This chapter
delivers the mirror image — a check that does not look for support but actively
tries to *knock the claim down* — and then turns from *vetting* new facts to
*maintaining* old ones: strengthening a guess into a cited fact, and flagging
facts that may have gone stale. This is the "maintain" quarter of the project arc.

#section("The mirror of triangulation: refutation")

Corroboration and refutation are two different questions. Triangulation asks
"which sources support this?" Refutation asks "can I *disprove* this?" — and the
difference matters, because a plausible, fluent, wrong claim is exactly the kind
that attracts no contradiction and simply sits there looking reasonable. To catch
it, you have to *attack* it.

#two_gates()

#term("Refutation")[
  *Refutation* is a check that actively tries to *disprove* a claim — hunting for
  factual errors, anachronisms, internal contradictions, things that conflict with
  well-established knowledge. Where triangulation looks for agreement, refutation
  looks for the flaw. A claim that survives a genuine attempt to break it has
  earned more trust than one that merely went unchallenged.
]

Turn on `research.refute_gate`, and a plain model- or document-derived `/fact` —
one not already handled by triangulation or the web check — gets a single
skeptic's pass before it commits. The Assistant tries to refute the claim and
reports a verdict: `SOUND` (it could not disprove it) lets the fact through;
`REFUTED` shows you the reasoning and asks you to confirm again. It is cheap, it
needs no external source, it works offline — and it is the natural check for the
speculative tier, where you most need a second, adversarial opinion. Like every
gate, it is advisory: you always keep the last word.

#callout(label: "Two questions, one habit")[
  You do not have to choose between triangulation and refutation — they answer
  different questions and compose. A load-bearing claim ideally survives both: the
  independent sources corroborate it, *and* a determined skeptic could not knock it
  down. When a fact has passed both, you are as sure as this workflow can make you.
]

#section("Climbing the ladder: `/upgrade`")

Back in Part I you kept a fact that was only a model's guess, and its provenance
said so. The honest thing to do with such a fact, eventually, is to *ground it on
something firmer* — and you should not have to delete it and start over to do that.
`/upgrade` does it in place.

Select a `model`-origin fact and run:

```
/upgrade
```

The Assistant takes the claim to the structured and scholarly sources — the same
triangulation engine from Chapter 7 — and asks whether any of them *corroborate*
it. If one does, and none contradicts, it *raises the fact's provenance to that
source*: the fact that used to say "just a guess" now says "corroborated by
Wikidata," and its rung on the ladder climbs accordingly.

#term("Tier upgrade")[
  A *tier upgrade* re-grounds an existing fact on a firmer source and raises its
  provenance to match — *without changing the fact's wording*. A guess becomes a
  cited fact over time. Only the provenance moves; your text is never touched.
]

This is the quiet engine of a corpus that improves with age. Early in a project
you keep quick guesses to keep moving; later, in a maintenance pass, you `/upgrade`
the ones your book leans on, and watch the low rungs of your Facts book climb —
each fact growing firmer without a word of it changing.

#two_track(
  [Draft fast on the model's memory to keep the story moving, then, before you
   hand the book over, `/upgrade` the handful of facts the plot actually depends
   on. The guesses that survive become cited; the ones that don't, you rewrite.],
  [`/upgrade` is how a working bibliography fills in. Each speculative claim you
   re-ground pulls a real citation up under it — turning a placeholder into a
   footnote you can defend, one fact at a time.],
)

#section("Facts grow old: `/stale`")

The last piece of maintenance is time itself. A fact grounded on a web page two
years ago may no longer be current; a figure that was right when you kept it may
have moved. `/stale` finds those:

```
/stale 90
```

It lists your `model`- and `web`-tier facts older than the number of days you
give — the ones whose grounding is softest and most likely to have drifted — so
you can re-verify or `/upgrade` them. Structured and computed facts, being firmer
and stable, are left out; there is little point re-checking a Q-id.

#term("Staleness")[
  A fact is *stale* when enough time has passed that its grounding may no longer
  hold — a moving figure, a page that changed, a claim overtaken by newer work.
  `/stale` surfaces the soft, aging facts for a second look; it is how a knowledge
  base stays honest across the months a book takes to write.
]

#section("The corpus that tends itself")

Put the last three chapters together and you have the whole "trust" half of the
work. You *triangulate* a claim against independent sources; you *refute* it to see
if it survives attack; you *audit* the whole corpus for internal contradictions;
you *upgrade* guesses into cited facts; and you flag the *stale* ones for renewal.
None of it edits your prose, all of it informs rather than dictates, and every step
leaves the provenance more honest than it found it.

A corpus maintained this way is not a dead pile of notes — it is checked, current,
and firm where it needs to be. Which means it is finally ready to be *used*. The
next parts turn from building and tending the knowledge base to *computing* new
facts from it, protecting the facts you *invented*, and composing the whole thing
back out into the book you are writing.

#recap((
  [*Refutation* is triangulation's mirror: it tries to *disprove* a claim rather
   than find support, catching plausible-but-wrong facts that attract no
   contradiction.],
  [`research.refute_gate` runs a cheap, offline skeptic pass on low-rung `/fact`s:
   `SOUND` passes, `REFUTED` asks again — advisory, never blocking.],
  [`/upgrade` re-grounds a `model` fact on a corroborating source and *raises its
   provenance tier in place* — the wording never changes, only the rung.],
  [`/stale [days]` surfaces aging `model`/`web` facts whose grounding may have
   drifted, for re-verification or upgrade.],
  [Together — triangulate, refute, audit, upgrade, refresh — they make a corpus
   that tends itself, honestly, without ever touching your prose.],
))
