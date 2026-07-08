#import "../design.typ": *

#chapter(number: 10, title: "The theology & philosophy track")

Theology and philosophy argue about things measurement cannot settle. You are not
claiming a fact the reader can check against the world; you are advancing a
_position_ and asking that it be granted a hearing. The track's obligations are
therefore unusual: your ground is _internal coherence_ and the _tradition you
answer to_, and your risks are not being wrong about a fact but being incoherent —
or arguing against a straw version of the view you oppose. This chapter is the loop
for the essay, the treatise, the sermon, the philosophical argument.

#section("Frame — the genre that stops demanding proof")

Set the genre, and this time the setting does something the other tracks never
need:

#config("inkhaven.hjson", [```hjson
genre: "philosophy"
```])

`theology`, `theological`, and `religious` share a nearby frame. What this genre
does is _tell the AI readers to stop asking for empirical proof_. Point a naive
fact-checker at "the soul is immortal" and it will flag an unsupported claim — which
is not a defect in your argument but a category error in the reader. Declaring the
genre recalibrates the interrogators: they stop asking "is this true?" and start
asking "does this _hold together_, and have you met the strongest form of the
objection?" — which are the only questions this kind of writing can be held to.

#insight[
  Every track tunes its readers, but this is the one where the tuning is
  load-bearing. Without it, the tools apply the wrong standard and generate noise;
  with it, they apply the standard the work was actually written to. On this track,
  setting the genre is not a nicety — it is the difference between a useful reader
  and a confused one.
]

#section("Gather — the tradition you answer to")

Philosophy and theology are conversations centuries deep; a position that ignores
the tradition it stands in is weaker for it. So the gathering here is _the reading_:
use the Research Assistant to build a corpus of the thinkers and texts you engage,
each kept with its provenance, so a claim about what a source argued can be checked
against the source. And where your argument turns on recurring symbols or a named
tradition — a theological reading that invokes a particular lineage — the Mythology
book lets you _declare_ the traditions you are working within, so the readers honour
your frame rather than importing another.

#section("Read — the moral reader and the coherent argument")

This track has a reader of its own. The *Inner Theologian* (`Ctrl+B J`, then `T`)
reads a passage for its moral and theological weight — the consequence left
undepicted, the claim that carries more freight than the prose admits — through the
lens of the tradition rather than against a fact. It is the questioner built for
exactly this kind of writing.

Alongside it, the Inner Socrates roster offers the `philosophical-reader` and
`theological-reader` personas, which grant your frame and press your reasoning:
where is the hidden premise, what does this argument presuppose, which objection
have you not met? Because the genre is set, even the general interrogator reads your
prose as an argument to be tested rather than a set of facts to be verified.

#subsection("Coherence over correctness")

The deterministic check that fits this track is not the fact-checker but its cousin
for _internal_ consistency. Declare your axioms — the positions you take as given —
and `/undisputed` checks them for coherence with one another: not "is this true in
the world?" but "do these commitments contradict each other?" It is the mechanical
half of the discipline the whole track is about — an argument that holds together on
its own terms.

#term("Internal coherence")[
  Consistency judged _within_ a frame rather than against the external world — the
  standard by which an argument about the unmeasurable is fairly assessed. A
  theology may be coherent or incoherent regardless of whether its claims are
  empirically testable; on this track, coherence is the thing the tools check and
  the thing a fair opponent presses, precisely because correctness is not on the
  table.
]

#pitfall[
  The characteristic failure of the track is the straw man — defeating a weak
  version of the view you oppose and mistaking it for a victory. Use the Socratic
  readers deliberately against yourself here: ask them to state the _strongest_ form
  of the objection you are answering, and to find where your argument only defeats a
  weaker one. An argument that beats the best opposing case is worth something; one
  that beats a caricature persuades no one who matters.
]

#section("Produce")

`export pdf|epub|docx` renders the essay or the treatise; if it cites — and serious
philosophy and theology usually do — the Sources machinery of the scientific track
is yours as well. The work leaves the desk as an argument that has already been
pressed, in your own study, by the readers built to press it.

#section("Hands-on: two procedures")

#subsection("Declare your axioms and check they cohere")

+ In the Facts book, record the positions you take as given — the commitments your argument rests on.
+ Mark each as authorial rather than checkable: select it in the Facts tree and press `u` (the ※ glyph). An undisputed fact sits outside the trust ladder — true within your work by decree, with no external truth to test it against.
+ Check they hold together: `/undisputed` (or `inkhaven fact-check` on the undisputed set) asks not "is this true in the world?" but "do these commitments contradict each other?" — the mechanical half of coherence.

#subsection("Read morally, and press your own argument")

+ Scan the manuscript for moral and theological weight: `inkhaven theologian scan`. It reads each passage through the lens of the tradition rather than against a fact.
+ Open a deeper session on a passage: `inkhaven theologian session`, or `Ctrl+B J` then `T` in the editor.
+ Grant your frame and press the reasoning: `Ctrl+B J`, then the `philosophical-reader` or `theological-reader` persona. Ask it, deliberately, to state the _strongest_ form of the objection you are answering — and to find where your argument only defeats a weaker one.

#recap((
  [Theology and philosophy argue the *unmeasurable*: your ground is *internal coherence* and the *tradition* you answer to; your risks are incoherence and the straw man.],
  [Setting `genre: "philosophy"` (or `theology`) is *load-bearing* — it recalibrates the readers to stop demanding empirical proof and start pressing whether the argument holds.],
  [*Gather* the tradition as a research corpus, and *declare* the traditions you work within (Mythology) so the readers honour your frame.],
  [*Read* with the *Inner Theologian* (`Ctrl+B J` → `T`) and the `philosophical-reader` / `theological-reader` personas; check *coherence* with `/undisputed`; and turn the questioners against your own argument to defeat the strongest objection, not a caricature.],
))
