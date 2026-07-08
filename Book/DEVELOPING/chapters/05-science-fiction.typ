#import "../design.typ": *

#chapter(number: 5, title: "The science-fiction track")

Science fiction is fiction that has signed a contract. You are granted one
impossible thing — a faster-than-light drive, a machine that reads minds, a world
without death — and in exchange you agree to obey its consequences with a
straight face. The track's discipline is _rule-bound invention_: the novum works
the same way on page four hundred as on page four, and the story never gets a
capability it did not establish. This chapter adds to the fiction loop the two
things that keep that contract: a ledger of your declared rules, and a research
habit for the real science you are bending.

#section("Frame — set the genre, name the novum")

Set the genre so the readers hold you to consequence rather than to imagery:

#config("inkhaven.hjson", [```hjson
genre: "scifi"
```])

`sci_fi` and `science_fiction` resolve to the same frame. Then, before you draft,
name your novum in a sentence and, crucially, name its _rule_: not "there is
faster-than-light travel" but "travel is instantaneous but only between beacons
that took a decade to build." The rule is what makes the story science fiction
rather than fantasy in a spacesuit — and it is what the tools will help you keep.

#section("Gather — a world, a ledger, and real science")

Grow the world as the fiction track does. Two additions are specific to this
track.

#subsection("The technology ledger")

Your novum bends physics, and the world's fact-checker knows physics — so left
alone it would flag your FTL drive as an impossible journey on every page. The
World book's *magic block* (here, read it as a _technology ledger_) is where you
_declare the exception_: a named rule that says what it covers and whom it applies
to. Once declared, the fact-checker honours it and stops warning — while still
catching the journey that breaks even your _own_ rule.

#term("Technology ledger")[
  The declared list of a story's departures from real physics — each a named rule
  stating what it overrides and where it applies. Declaring an exception is not
  cheating; it is the opposite. It fixes the rule in one place so the fact-checker
  can hold every later page to it, and so a capability can never quietly appear
  where you never granted it.
]

#subsection("Research for the science you bend")

Hard SF earns its authority from the real science under the invention. The
*Research Assistant* (`inkhaven research`) grounds that science: it can pull
structured facts, scholarly papers, and web sources into a Research book, and keep
each with its provenance, so the orbital mechanics your plot leans on are right
where they can be and clearly invented where they can't. When you set up a Facts
book with `facts init --genre scifi`, Inkhaven seeds the continuity categories a
science-fiction manuscript most often contradicts itself on — technology, physics
and travel, the polity — so the book watches the right things.

#note[
  The two grounds pull in opposite directions, and that is the craft of the track.
  The technology ledger says "here is where I leave reality, on purpose"; the
  research habit says "everywhere else, I stayed." Keeping the line between them
  sharp — declared where you invent, sourced where you don't — is what a rigorous
  reader is really checking.
]

#section("Draft and read — held to your own rule")

Draft against the world as any novelist would. When you read the draft back, the
science-fiction difference is in what the checks enforce. `Ctrl+B Shift+X`
fact-checks a passage against the world _and_ the ledger: a journey that fits your
declared drive passes silently; one that exceeds even your own rule is flagged. The
inner readers, told the genre, ask the science-fiction questions — the
`inner-historian` persona in particular presses whether an event's timing fits the
technology you established, and whether a capability used here was ever granted.

#pitfall[
  The classic failure of the track is the _convenient power_: the drive that is
  slow all book until the escape needs it fast, the AI that can't do the thing
  until the plot requires it. Declare the rule fully in the ledger _before_ you need
  to break it, and let the fact-checker catch you leaning on the exception. A power
  the reader can predict is a power they can be surprised by honestly; one that
  bends to the plot is a cheat they can feel.
]

#section("Produce")

Nothing special: `export epub|pdf`, scoped by status, and the Submissions tools if
the book is going out. The rigour was all upstream — in the ledger you kept and the
sources you grounded — so the finished book carries its authority quietly.

#section("Hands-on: two procedures")

#subsection("Declare a technology exception and hold the story to it")

+ In `world.hjson`, add a `magic` block (read it as your technology ledger) that names the exception and its limit — for example a `travel_time` rule stating that beacon-to-beacon jumps are instantaneous but nothing else is.
+ Recompile so the world knows the rule: `inkhaven realworld compile`.
+ Write a passage where a ship jumps between two beacons, then fact-check it: `Ctrl+B Shift+X`. Because the exception is declared, the impossible speed passes silently.
+ Now write a passage where the ship crosses open space faster than your own rule allows, and fact-check again. This time it is flagged — the ledger honours your exception but still catches a page that breaks your _own_ law.

#subsection("Ground the real science")

+ Open the Research Assistant: `inkhaven research`.
+ Pull the real work behind your invention: `/openalex orbital mechanics of Lagrange points` (or `/arxiv …` for a preprint). The assistant files the paper's citation to your Sources book automatically.
+ Keep the fact you need: `/fact A Lagrange point is a position of gravitational equilibrium.` — it crosses the confirmation gate and lands in your Facts book with its provenance.
+ Seed the continuity categories a science-fiction manuscript most often trips on: `inkhaven facts init --genre scifi`, then `inkhaven facts scan` to check your prose against them.

#recap((
  [Science fiction is *rule-bound fiction*: set `genre: "scifi"` and, before drafting, name your novum _and its rule_ — the constraint that makes it SF rather than fantasy.],
  [Declare each departure from physics in the *technology ledger* (the World book's magic block) so the fact-checker honours your exception while still catching a page that breaks your own rule.],
  [Ground the real science with the *Research Assistant* and seed the right continuity categories with `facts init --genre scifi` — declared where you invent, sourced where you don't.],
  [Read with the genre-tuned inner readers (the `inner-historian` presses capability and timing) and fact-check against world *and* ledger; guard hardest against the convenient power.],
))
