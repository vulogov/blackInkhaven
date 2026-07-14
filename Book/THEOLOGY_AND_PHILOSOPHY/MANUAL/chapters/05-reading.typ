#import "../design.typ": *

#chapter(number: 5, title: "Reading the draft")

Everything in the last chapter happened in the Research Assistant — a separate room
from the one you write in. But a study is drafted _while_ the corpus is still
growing, and the readers that matter most are the ones you can turn on the
paragraph in front of you without leaving the editor. This track keeps two, and
they ask different questions of the same prose: one confronts the paragraph with
your _sources_, the other reads it for its _theological weight_. Between them they
are the second pair of eyes this kind of writing most needs.

#section("Confront the paragraph against the corpus")

The first reader is SCHOLAR's `/relate`, brought to the open paragraph. Put the
cursor in a paragraph, press `Ctrl+V` then `?`, and Inkhaven retrieves the nearest
facts and source passages, judges each against the prose with the graded relation
engine, and emits the findings — anchored to the paragraph — into the Output pane.
It is the manuscript-side twin of `/relate`: the same judge, pointed at what you
are writing.

Suppose we have just drafted the second-section paragraph, the one crediting Kant
with an endless approach that never arrives. We press `Ctrl+V ?`, and:

#transcript("Ctrl+V ?  — confront the open paragraph", [
  ⚔ *confront* · 2 relation(s) to sources in this ¶ — ^B Tab → Output \
  ⚔ *against* — `@book-of-mormon` (Moroni 10:32): "perfected in Christ" reads as a
  reachable completion; the paragraph's "never arrives" over-generalises across the
  traditions it describes. \
  ✓ *supporting* — `@kant-cpracr` (Ak. 5:122): the endless progress toward fitness
  of the will confirms the paragraph's reading of Kant specifically.
])

The finding marked `against` is the chord doing its job: our paragraph said "the
approach never arrives"; the corpus answers that this holds for Kant but that the
Book of Mormon, which the paragraph was gesturing at, says the opposite. The prose
reached one clause too far, and the source we had already gathered caught it — while
the cursor was still in the sentence. An `against` relation arrives as a warning, a
`supporting` one as citable information, a silent source is dropped, and each
finding re-badges its paragraph in the chapter tree so an unaddressed tension is
visible at a glance.

#pitfall[
  Do not confront a paragraph with no corpus behind it and read the silence as
  approval. If you have not yet gathered the sources a paragraph depends on, the
  chord has nothing to retrieve and honestly reports that nothing bears on the
  prose — which is not the same as the prose being sound. When confront finds
  nothing, ask whether that is because the claim is safe or because the source is
  missing.
]

#section("Read for theological weight — the Inner Theologian")

Confront tests a paragraph against what the sources _say_. It cannot tell you that
a sentence carries more moral freight than its prose admits, or that a claim about
deification has walked past a consequence it never depicted. That reading is the
*Inner Theologian's*, and on this track it is the signature reader — the one built
for exactly the kind of writing where the stakes are not factual but moral and
theological.

Put the cursor in a passage and press `Ctrl+B J`, then `T`. The Inner Theologian
reads the passage through the lens of the traditions in play — not against a fact,
but for its weight — and its reflection lands in the Thoughts pane, where a longer
response belongs. Turn it on the essay's hardest paragraph, the one asserting that
for Kant the gap between creature and God stays permanently open:

#transcript("Ctrl+B J → T  — the Inner Theologian reads the open paragraph", [
  _On "the creature never becomes holy; the gap remains permanently in place":_
  this sentence carries a heavier claim than it states. To fix the gap _permanently_
  is not only an epistemic modesty about Kant's postulates — it is a theological
  verdict on the worth of the creature, and the paragraph should own it. Consider
  what the sentence leaves undepicted: a striving with no term is either the
  creature's dignity (Kant's reading) or its tragedy (the tradition's objection),
  and the prose currently lets the reader supply whichever they brought. Name the
  stake. And weigh the word "never" against Matthew 5:48, which you cite two
  paragraphs earlier as a _command_ to perfection — a command implies a capacity
  the tradition takes seriously and Kant denies. Is your "never" Kant's, or yours?
])

Read what that did, because it is different from confront. Confront told us a
_source_ contradicted the paragraph; the Inner Theologian told us the paragraph was
_theologically underweight_ — that "never" carries a verdict the prose was smuggling
rather than arguing, and that the same word sits in tension with a command the essay
had already granted. That is not a fact-check. It is the reading a thoughtful editor
in the tradition would give, and it made the paragraph's central word the thing to
defend rather than assume.

#insight[
  The Inner Theologian is the reader this track exists for. Every other tool checks
  the argument's _joints_ — where a claim meets a source, where two claims collide.
  The Inner Theologian reads the argument's _weight_: the consequence left
  undepicted, the sentence that decides more than it defends, the moral freight a
  clause carries past what its prose admits. On a track where correctness is off the
  table, weight is much of what remains — and it is the thing no fact-checker, and no
  confront pass, can see. Editing a work of theology _is_, in large part, weighing
  it; this is the reader that weighs.
]

Two more ways to reach it. `inkhaven theologian scan` reads the whole manuscript
for moral and theological weight in one deterministic, zero-AI pass — the fast way
to find the paragraphs that deserve a deeper look — and `inkhaven theologian session`
opens that deeper look on a passage from the shell. Run the scan across a finished
draft and it hands you the list of passages carrying more than they say; open a
session on each, and the Inner Theologian reads it as it read the paragraph above.

#section("Press the argument — the Socratic personas")

Alongside the Theologian, the Inner Socrates roster carries two readers built to
grant your frame and press your reasoning. `Ctrl+B J`, then the `philosophical-reader`
or `theological-reader` persona, and the reader asks where the hidden premise is,
what the argument presupposes, which objection you have not met. Because the genre
is set (Chapter 1), even these read your prose as an argument to be tested, not a
set of facts to be verified.

Use them deliberately against yourself. The characteristic failure of this track is
the straw man — defeating a weak version of the view you oppose. Ask the
`philosophical-reader` to state the _strongest_ form of the objection you are
answering, and to find where your argument only defeats a weaker one. On our essay
that pressure produced the fourth-movement concession — that Kant's "trade" of
arrival for honesty is not obviously the wrong one — which is what keeps the study a
comparison rather than a polemic.

#note[
  Before you act on any of these readings — narrowing a claim confront flagged,
  rewriting a sentence the Theologian found underweight — take a snapshot (`F5`), so
  the draft you are about to change is always recoverable. The next chapter makes
  snapshots and the prose readers a discipline of their own; here it is enough to
  know that a reading worth acting on is worth protecting the draft before you do.
]

#recap((
  [This track turns two *manuscript readers* on the growing draft: `Ctrl+V ?` *confronts* the paragraph with your sources (the graded relation engine, anchored findings), and the *Inner Theologian* (`Ctrl+B J → T`) reads it for *theological weight*.],
  [Confront catches a claim that reaches past its evidence; the *Inner Theologian* catches the sentence that carries more moral freight than it admits — the consequence left undepicted, the word that decides more than it defends. It is the signature reader of the track.],
  [Reach the Theologian three ways: the `Ctrl+B J → T` chord in the editor, `inkhaven theologian scan` (a whole-manuscript, zero-AI weight pass), and `inkhaven theologian session` (a deep read on one passage).],
  [Press your own argument with the `philosophical-reader` / `theological-reader` Socratic personas — ask for the *strongest* objection, to defeat the best opposing case rather than a caricature — and *snapshot* (`F5`) before acting on any reading.],
))
