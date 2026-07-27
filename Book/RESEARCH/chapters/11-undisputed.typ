#import "../design.typ": *

#chapter(number: 11, title: "The Facts You Invented")

This whole book has been about grounding — about never letting a claim rest on
nothing. But a work of fiction is built on claims that rest on nothing *by design*.
That the kingdom has three moons, that the drug wears off in an hour, that your
detective was born in Lyon — these are not facts you got wrong or right. You
*decreed* them. There is no external world to check them against, and a tool that
tried to fact-check them would be making a category error.

So Inkhaven, for all its insistence on grounding, makes deliberate room for the
facts you invented. It calls them *undisputed*.

#section("Two kinds of fact, checked two ways")

Back in Chapter 1 you learned to tell *borrowed* claims (checkable against the
world) from *invented* ones (yours to command). The trust ladder, the sources, the
cross-checking — all of Parts II and III — apply only to the borrowed kind. The
invented kind needs the opposite treatment: protection *from* fact-checking, not
by it.

#authorial_split()

#term("Undisputed fact")[
  An *undisputed* (or authorial) fact is one you have marked as your own creative
  invention — true within your work by decree, with no external truth to check it
  against. It sits *outside* the trust ladder entirely: it is not a low-rung fact
  waiting to be grounded, it is an *axiom*. Marking it undisputed tells the tool,
  "do not question whether this is real — I made it real."
]

#section("Marking a fact undisputed")

In the Facts tree, select an invented fact and press a single key: `u`. It gains a
distinct glyph — ※ — and from that moment it is treated as an authorial axiom.

The immediate effect is that `/factcheck` *leaves it alone*. When you audit your
corpus (Chapter 8), undisputed facts are excluded from the truth pass — there is
nothing to verify — though the audit does *report how many* it skipped, so you
always know how much of your world is invented axiom versus borrowed fact. Your
three moons will never be flagged as "unsupported," because support was never the
point.

#two_track(
  [This is the feature that makes the Research Assistant safe for a novelist. Mark
   your world's invented facts undisputed, and the fact-checker stops treating your
   magic system as a factual error. Your borrowed facts still get the full
   scrutiny; your invented ones are left to be exactly as you decreed.],
  [Non-fiction needs this rarely, but it needs it. A definition you *stipulate*
   ("in this book, 'early modern' means 1500–1700"), the premise of a thought
   experiment, an axiom your argument builds on — these are undisputed too: true by
   authorial declaration, not to be fact-checked.],
)

#section("Coherent, even if invented")

An invented fact cannot be checked against the world — but it can still be checked
against *itself*. A magic system can contradict its own rules; an invented history
can put two decreed events in an impossible order. So there is a separate,
optional check made just for authorial facts, asking a completely different
question.

```
/undisputed
```

`/undisputed` runs a *common-sense* pass over your undisputed facts — not "is this
real?" (it never is; that is the point) but "does this make sense *within its own
frame* — is it self-consistent, free of obvious internal contradiction?" Each fact
comes back `PLAUSIBLE`, `ODD`, or `INCOHERENT`, and the tree colours its ※ glyph
accordingly, so you can see at a glance which of your invented facts hang together
and which want a second look.

#screen(caption: "/undisputed — a coherence pass over authorial facts")[```
> /undisputed
1. PLAUSIBLE — consistent with the guild's charter
2. ODD — the festival date shifts between chapters
3. INCOHERENT — the heir is born after the regency
   it supposedly justified

undisputed check complete · 3 · ✓1 ?1 ✗1
```]

#term("Internal coherence")[
  *Internal coherence* is consistency *within* an invented frame, judged without
  reference to the real world. A dragon is not "wrong," but a dragon that is
  described as both cold-blooded and a source of its own heat may be internally
  *incoherent*. `/undisputed` checks for that kind of self-contradiction — and,
  like every check in this book, it only reports; it never rewrites your invention.
]

#callout(label: "It checks your world; it never edits it")[
  `/undisputed` will never change a word of an invented fact — it has no standing
  to. Your creative decisions are yours. It simply holds a mirror up: "within the
  rules you set, does this hang together?" A `PLAUSIBLE` verdict is reassurance; an
  `INCOHERENT` one is an invitation to look, not an order to change. And it works
  in your project's language, so a world written in German or Russian is judged in
  its own tongue.
]

#section("The whole picture")

Step back and the design resolves into something clean. Your book contains two
kinds of fact, and the Research Assistant serves both without confusing them.
*Borrowed* facts climb the trust ladder, get cross-checked, and remember their
sources — because a reader could look them up. *Invented* facts sit outside the
ladder as undisputed axioms, protected from fact-checking, checked only for
internal coherence — because you made them, and the only truth they answer to is
your own.

A novelist gets a research tool that never mistakes imagination for error. A
non-fiction author gets one that respects a stipulated definition as readily as it
scrutinises a borrowed claim. Both get the same promise: the tool grounds what
should be grounded, protects what you invented, and never edits either.

Which means your corpus is now complete — borrowed facts grounded and checked,
invented facts marked and coherent. It is finally time to *use* it: to compose out
of everything you gathered, straight into the book you are writing. That is the
last working part.

#recap((
  [Fiction rests on *invented* facts that must be protected *from* fact-checking —
   there is no external truth to check them against.],
  [Press `u` in the Facts tree to mark a fact *undisputed* (glyph ※): an authorial
   axiom that sits *outside* the trust ladder.],
  [`/factcheck` *excludes* undisputed facts from the truth pass (and reports how
   many it skipped) — your invented world is never flagged as "unsupported."],
  [`/undisputed` checks invented facts for *internal coherence* — self-consistency
   within their own frame (`PLAUSIBLE`/`ODD`/`INCOHERENT`) — in the project
   language, and never rewrites them.],
  [The design resolves cleanly: borrowed facts are grounded and checked; invented
   facts are marked and kept coherent; the tool never confuses the two, and never
   edits either.],
))
