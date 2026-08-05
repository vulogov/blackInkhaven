#import "../design.typ": *

#chapter(number: 10, title: "The Inner Editor")

Where Inner Socrates asks, the Inner Editor *observes*. It is the reader who sits with
a single paragraph and notices what the prose is actually doing — the tautology you
did not hear, the sentence carrying more than its weight, the small excellence worth
keeping. It is a craft reader, not a grammar checker: it cares about literacy and
style, about whether the language earns its place, and it says so in a warm, specific
register that assumes you are a serious writer who wants to be read closely.

#term("The Inner Editor")[
  A single, configurable persona (unlike Socrates' roster) that reads the open
  paragraph — and a few before it — for craft. It emits *observations*, never edits;
  a rewrite is always yours to request, review as a diff, and confirm.
]

#section("Praise, Note, Concern")

The Editor speaks at three weights, and it is deliberate about the lightest one:

#screen(caption: "Ctrl+V O — the Inner Editor's observations")[```
Inner Editor · ch. 12 ¶ the-vigil
  ✎ Praise   the triple beat in "she waited, she counted, she
             forgot to breathe" earns its rhythm.
  ✎ Note     "the very fact that" carries no weight the sentence
             needs — the claim stands without it.
  ✎ Concern  three sentences in a row open on "It was" — the prose
             leans on the same hinge.
```]

*Praise* must be *earned* — it names a specific thing that works, and is hidden by
default so it never becomes flattery. *Note* is the bulk of the reading. *Concern*
flags a craft issue worth attention. Its categories run from literary richness and
tautology to style instability, dictionary richness, and the belief-stance a passage
takes — eight ways of looking at a paragraph, weighted the way you tune them.

#section("It never prescribes")

The Editor is *non-prescriptive* by rule. It will not say "you should" or "you must";
it names what it sees and trusts you to decide. When you *do* want a change made, that
is the Editorial Pass's job — the Editor's own observation can be handed to the
rewrite as the instruction, and even then it lands as a diff you accept, snapshot and
all. The reading and the editing are kept separate on purpose: the Editor's only job
is to help you see.

#section("Engaging it")

`Ctrl+V O` — *O for Observe* — opens the overview; `E` engages the reading, `A` toggles
an ambient reading on a paragraph pause, `F` lists the findings. It is language-model
work, so it is cost-aware and off the hot path; a first draft is not the place for a
close reader, and the Editor is content to wait for a finished scene. Everything it
says is recorded, so a Praise you dismissed today is still there when you come back to
harvest the good lines.

#two_track(
  [For fiction the Editor is a line editor with taste — the reader who catches the
  echo and the crutch, and tells you which sentence is quietly the best on the page.],
  [For non-fiction it reads for clarity and completeness as the craft: the undefined
  leap, the paragraph that says the same thing twice, the place the prose leans.],
)

#recap((
  [The *Inner Editor* observes a paragraph for craft — literacy and style — in a warm,
  *non-prescriptive* register; it never says "you should."],
  [It speaks at three weights: *Praise* (earned, hidden by default), *Note* (the bulk),
  *Concern* (a craft issue), across eight categories.],
  [`Ctrl+V O` engages it; a rewrite it suggests still lands as a diff you confirm —
  reading and editing kept apart on purpose.],
))
