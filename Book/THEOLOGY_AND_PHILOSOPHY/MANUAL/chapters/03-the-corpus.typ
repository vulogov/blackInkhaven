#import "../design.typ": *

#chapter(number: 3, title: "The corpus and its provenance")

Ingesting the sources gave us texts the assistant can retrieve; it did not yet
give us _claims_. A study advances on assertions — "Kant grounds freedom outside
the causal order", "the Book of Mormon asserts real deification" — and each of
those must be traceable to the passage that warrants it. This short chapter turns
the raw sources into a workbench of provenance-tagged facts: the ground the
argument will stand on, and the ground the tools in the next chapter will test.

#section("Facts and notes — two shelves, two confidences")

Inkhaven keeps two system books for what you learn. The *Facts* book is for
claims you are prepared to build on; the *Notes* book is for the speculative, the
half-formed, the "check this later." The Research Assistant files to either with a
mandatory confirmation step — nothing enters your ground truth without your
seeing it first — and each entry records _where it came from_:

#transcript("/fact \"Kant postulates immortality as a demand of practical reason, not a theoretical proof\" → kant/immortality", [
  Extracted from the last grounded answer. \
  Source: `@kant-cpr` (Critique of Practical Reason, retrieved passage). \
  Confirm insert to *Facts › kant › immortality*? *[y]* — Inserted.
])

The arrow routes the fact to a place in the Facts tree, so the corpus grows an
organisation as it grows — a `kant` branch, a `bible` branch, a
`book-of-mormon` branch. That organisation is not cosmetic: the contradiction and
convergence passes in the next chapter cluster their findings by exactly these
branches, so a tree that mirrors your argument gives you a report that mirrors it
too.

#term("Provenance")[
  The record, kept per fact, of where a claim came from — which source, retrieved
  from which passage, at what confidence tier. Provenance is what lets a later
  reader (human or machine) ask not "is this asserted?" but "on whose authority,
  and can I go read it?" On this track it is the difference between a claim you can
  defend before a critic and one you merely remember being persuaded of.
]

#section("Declare your axioms")

Some of what an argument rests on is not a finding but a _commitment_ — a position
you take as given and ask the reader to grant for the sake of the argument. "We
read Kant's postulates as regulative, not constitutive" is that kind of claim:
not true or false in the world, but a stance the essay adopts. Record these in the
Facts book and mark each _undisputed_ — select it in the Facts tree and press
`u`, and it takes the ※ glyph. An undisputed fact sits outside the trust ladder:
true within your work by decree, with no external truth to test it against.

Marking axioms is not bookkeeping. It changes what the tools do. The
`/undisputed` check asks of your declared commitments not "are these true?" but
"do these contradict _each other_?" — the mechanical half of the discipline this
whole track is about. An argument may be built on any axioms it likes, so long as
they cohere; `/undisputed` is the reader that holds you to the "so long as."

#insight[
  The distinction between a _fact_ (grounded in a source, checkable) and an
  _axiom_ (declared, coherent-or-not) is the distinction the whole track runs on.
  Facts answer to their provenance; axioms answer only to one another. Keep them
  on separate footing — grounded claims cite a passage, commitments wear the ※ —
  and every later tool knows which standard to apply. Blur them, and the
  fact-checker will demand proof of a commitment while the coherence check ignores
  a claim that needed a source.
]

#section("See where everything came from")

At any point, `/sources` lists each fact beside its recorded provenance — the
whole corpus as a ledger of who-said-what. It is worth running before the
interrogation begins, because a contradiction between two facts means something
very different depending on whether they came from _one_ source contradicting
itself or _two_ sources disagreeing — and that distinction, cross-source versus
within-source, is one the next chapter's engine reads straight off the provenance
you are looking at here.

#recap((
  [Turn sources into *claims*: `/fact` files a grounded, provenance-tagged assertion to the *Facts* book (confirmation required); `/note` holds the speculative in the *Notes* book.],
  [Route facts into a tree that *mirrors your argument* (`→ kant/immortality`) — the contradiction and convergence passes cluster their findings by those same branches.],
  [Separate *facts* (grounded, checkable) from *axioms* (declared commitments): mark axioms undisputed with `u` (the ※), and `/undisputed` checks them for coherence with one another, not truth.],
  [`/sources` shows the whole corpus as a provenance ledger — and cross-source versus within-source, which the next chapter's engine reads off that provenance, is the difference between two sources disagreeing and one contradicting itself.],
))
