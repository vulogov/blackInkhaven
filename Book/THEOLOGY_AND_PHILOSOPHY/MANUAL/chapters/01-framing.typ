#import "../design.typ": *

#chapter(number: 1, title: "Framing the question")

Every track begins by telling Inkhaven what kind of work this is, but on this
track the telling is load-bearing. Set the genre wrong and the readers apply the
wrong standard and generate noise; set it right and they apply the standard the
essay was written to. So we begin not with prose but with one line of config, and
with the discipline of stating the question sharply enough that the tools can
help us answer it.

#section("Set the genre that stops demanding proof")

Create the project and set its genre. `philosophy`, `theology`, `theological`,
and `religious` share a nearby frame; all of them tell the AI readers to stop
asking "is this true?" and start asking "does this _hold together_, and have you
met the strongest form of the objection?":

#config("inkhaven.hjson", [```hjson
genre: "philosophy"
language: "english"
```])

The language line matters more than it looks. It drives which public-domain
scripture translations the adapters reach for in the next chapter — English pulls
the World English Bible; a Russian project would pull the Synodal text and the
Kuliev Qur'an without another word from you — and it sets the tongue the finished
apparatus is labelled in. State it once, here, and the whole pipeline honours it.

#insight[
  Declaring the genre recalibrates the interrogators. Point a naïve fact-checker
  at "the noumenal self is uncreated" and it flags an unsupported empirical
  claim — a category error, not a finding. With `genre: "philosophy"` set, the
  same readers grant the frame and press the reasoning instead: where is the
  hidden premise, what does this presuppose, which objection have you not met?
  On this track that recalibration is not a nicety. It is the difference between
  a useful reader and a confused one.
]

#section("State the question as a thesis you could be wrong about")

A research question that cannot fail teaches you nothing. Ours has two live
halves — a resemblance and a rupture — and we write it down that way, because the
tools in Part III reward a claim with genuine tension in it:

#config("the working thesis", [
  Kant's transcendental idealism and Latter-day Saint eternal progression both
  refuse instantaneous, static salvation and make perfection a _trajectory_.
  The resemblance is real at two joints — the asymptotic moral self of the second
  _Critique_, and the uncreated self that grounds freedom. It breaks at three —
  epistemic posture (postulate versus revelation), asymptote versus arrival
  (endless approach versus real deification), and the kind of God at the end
  (a moral postulate versus an embodied, continuous being).
])

That is not yet an argument; it is a map of where the argument will have to do
work. We will spend the rest of the book testing each joint against what the
sources actually say — and, more than once, moving a joint when the sources
refuse to hold it where we first put it.

#question[
  Before you gather a single source, can you name the place your thesis is most
  likely to be wrong? For us it is "asymptote versus arrival" — Kant may not
  permit even the endless _approach_ we are crediting him with. Write your
  weakest joint down now. The confront pass in Chapter 5 will go looking for it,
  and it helps to have named it first.
]

#section("Sharpen it with the Dialectician")

Inkhaven's Research Assistant carries a reader whose whole job is to press a
question before you commit to it. `/socrates` points the Inner Socrates
Dialectician — the `philosophical-reader` persona — at your corpus and returns
the questions your position must answer to stand. Run it early, on the bare
thesis, and it will tell you what you have not yet earned:

#transcript("/socrates eternal progression and Kant's postulates", [
  _The Dialectician asks:_ (1) You call both views a "trajectory" — but does
  Kant's endless approach _to_ holiness and the Saints' progression _into_
  godhood share a direction, or only a shape? (2) "Refuse instantaneous
  salvation" — refused by whom, and on what authority: a limit of reason, or a
  revealed fact? (3) If the gap between creature and God never closes for Kant
  and does close for the Saints, is your "resemblance" a similarity of doctrine,
  or only of grammar?
])

Every one of those questions became a section of the finished essay. That is the
point of asking them now: the Dialectician is not answering your question, it is
telling you which questions are load-bearing — so you gather to answer _those_
and not waste a single source on the parts that were never in doubt.

#pitfall[
  The characteristic failure of this track is the straw man — defeating a weak
  version of the view you oppose and mistaking it for a victory. Notice that the
  Dialectician's first question already guards against it: it refuses to let
  "trajectory" mean the same thing for both traditions without proof. Keep the
  readers pointed at your own argument, not a caricature of the other side. An
  argument that beats the best opposing case is worth something; one that beats a
  cartoon persuades no one who matters.
]

#recap((
  [Set `genre: "philosophy"` (or `theology`) *first* — it recalibrates the readers to stop demanding empirical proof and start pressing coherence, and it is load-bearing on this track, not cosmetic.],
  [The `language` line drives which public-domain scripture translations the adapters reach for and the tongue the finished apparatus is labelled in — state it once, here.],
  [Write the thesis as a *resemblance and a rupture* — a claim with genuine tension — and name the joint most likely to be wrong before you gather anything.],
  [Run `/socrates` on the bare thesis to surface the load-bearing questions; gather to answer *those*, and keep the Dialectician pointed at your own argument to forestall the straw man.],
))
