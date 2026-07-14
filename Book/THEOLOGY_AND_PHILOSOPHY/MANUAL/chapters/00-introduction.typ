#import "../design.typ": *

#pagebreak(weak: true, to: "odd")

#hide(heading(level: 1, numbering: none, outlined: true, bookmarked: true, "Introduction"))

#v(2cm)
#align(left)[
  #text(font: sans_family, size: 9pt, tracking: 2pt, fill: ink_gray, upper("Introduction"))
  #v(4mm)
  #text(font: body_family, size: 32pt, weight: "regular", fill: ink_black, "One question, from blank project to finished essay")
]
#v(1cm)
#line(length: 100%, stroke: 0.5pt + ink_rule)
#v(10mm)

#dropcap("T")heology and philosophy argue about things measurement cannot
settle. You are not reporting a fact the reader can check against the world; you
are advancing a _position_ and asking that it be granted a hearing. That makes
the work's obligations unusual — your ground is internal coherence and the
tradition you answer to, not a dataset — and it makes the tooling unusual too. A
naïve fact-checker pointed at "the soul is immortal" will flag an unsupported
claim, which is not a defect in the argument but a category error in the reader.
This book is about doing the work with tools that have been recalibrated to the
standard the work is actually held to.

It is a _process_ book, and it takes an unusual vow: it does not talk about
theology-and-philosophy writing in the abstract. It picks one real question and
researches and writes it end to end, in front of you, using nothing but
Inkhaven. Every stage — framing the project, gathering the primary sources,
interrogating them for tension and agreement, drafting against what was found,
confronting the draft with the corpus, and producing a finished essay with a
bibliography and an index of cited passages — is shown on that one question. The
essay the process produces is not hypothetical either: it is the companion
volume that sits beside this one on the shelf.

#section("The question we will work")

The question is a genuine one in comparative philosophical theology, and it has
the shape this track rewards — a real family resemblance between two traditions,
running alongside a real disagreement:

#question[
  Kant's transcendental idealism and the Latter-day Saint doctrine of _eternal
  progression_ both refuse a single, instantaneous, static salvation — both
  make perfection a trajectory rather than a state. Where does that resemblance
  hold, and where does it break? Do the two ever say the same thing, or only
  rhyme?
]

We chose it deliberately. It reaches across a philosopher and two scriptural
traditions; it turns on what specific passages actually say, so provenance
matters; and its whole interest lies in a _graded_ relation — agreement here,
tension there, flat contradiction elsewhere — which is exactly what the tools in
Part III were built to surface. A question with no tension would waste them; a
question with only contradiction would be a polemic, not a study.

#section("The sources will be real, and public-domain")

Nothing in this book is invented data. Kant comes from Project Gutenberg; the
Bible and the Book of Mormon come through Inkhaven's scripture adapters, which
draw only on public-domain texts. This is a discipline, not a convenience: a
theological argument that recalls what a source says from memory is unreliable
and against the whole grounding ethos. We ingest the real texts, retrieve from
them with their provenance intact, and let the AI judge only passages actually
in front of it — never its own recollection.

#insight[
  The engine of this track is not a machine that _knows_ theology. It is a
  machine that retrieves what your sources actually say, tests your claims
  against those passages, and points you to where the sources agree, strain, or
  contradict. The judgement stays yours. The tools make the judgement
  _checkable_ — and on a track where correctness is off the table, checkable is
  the closest thing to credible there is.
]

#section("The loop this book walks")

The work is a loop, and this book is its stations in order:

#research_arc()

We *frame* the question and set the genre that stops the readers demanding proof
(Chapter 1). We *gather* the primary sources — Kant, the Bible, the Book of
Mormon — as a provenance-tagged corpus (Chapters 2–3). We *interrogate* that
corpus with SCHOLAR, the relation engine: where does a claim relate to the
sources, where do the sources contradict, where do they converge, and what
questions must the whole hold up under (Chapter 4)? We *read* the growing draft —
confronting each paragraph against the corpus and weighing it with the Inner
Theologian, the reader this track exists for (Chapter 5). We compose with *cited
loci* — `@bible[John 3:16]`, `@kant[A51/B75]` — and gather them into an Index
Locorum (Chapter 6). We *revise* the prose with the craft reader, the grammar
check, and the snapshot beneath a bold rewrite (Chapter 7). And we *produce* the
finished essay (Chapter 8).

#note[
  This book assumes you have met Inkhaven's desk before — the chapter tree, the
  editor, snapshots, export. If you have not, read _Developing a Story with
  Inkhaven_ first; its theology-and-philosophy chapter is the short overview this
  book is the long, worked expansion of. Where a command or a chord appears here,
  it is shown doing real work on the essay, not defined in the abstract.
]

The companion essay was written this way and no other. When you reach its Index
Locorum and see every cited passage gathered under its source, you will be
looking at the last station of the loop you are about to walk. Turn the page, and
let us frame the question.
