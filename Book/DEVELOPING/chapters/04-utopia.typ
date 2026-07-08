#import "../design.typ": *

#chapter(number: 4, title: "The utopia track")

The utopian and dystopian track is fiction with a philosophical spine. You are
still telling a story in an invented world, so everything in the fiction chapter
applies — but the world here is not merely a setting. It is an _argument_: a claim
that a society organised _this way_ would produce _these_ consequences. The
discipline of the track is to make that argument hold, and Inkhaven adds one
instrument the other tracks rarely touch to help you.

#section("Frame — declare the premise, not just the place")

Set the genre to the utopian frame — `utopian`, `utopia`, `dystopian`, and
`dystopia` all resolve to the same reading:

#config("inkhaven.hjson", [```hjson
genre: "utopian"
```])

This does something specific to the readers: it tells them to treat the world as a
_designed system whose logic is on trial_, not as a neutral backdrop. Build the
project as you would for fiction — a manuscript, Characters, Places, and a World
book — but write your setting down as a set of *premises*: the founding rules of
the society. This world says property is abolished; that one says memory can be
edited; the other says no one may leave the city. These are the load-bearing
claims, and they are what the track will hold you to.

#term("Premise")[
  A declared founding rule of a designed society — the "what if" the book is built
  on. A premise is a claim with consequences: abolish money, and something must
  still allocate scarce things; edit memory, and something must decide whose. The
  utopia track reads your premises as a chain of such claims and looks for the link
  the prose quietly breaks.
]

#section("Gather — the world as fiction, plus its argument")

Grow the world exactly as the fiction track does — the world simulation for the
physical setting, Characters for the people who live under the premise, Threads for
the story that tests it. What you add is care in the World book: state the premises
plainly, and state what each is _supposed_ to produce. The richer that declaration,
the more the coherence checker has to reason about.

#section("Read — the coherence checker, the track's signature tool")

Here is what makes utopia its own track. Inkhaven can read your declared world as a
_chain of logical claims_ and look for the place where the society cheats — where a
premise is declared and then the prose relies on the very thing the premise
forbade.

```
inkhaven world utopia-check
```

It surfaces the tensions: the abolished currency that reappears as "favours" doing
exactly a currency's job; the perfect equality with an unexamined class of people
who do the unpleasant work; the total surveillance the hero somehow evades without
explanation. You are not obliged to resolve every one — a deliberate contradiction
can be the _point_ of a dystopia — so `world utopia-suppress` lets you mark a
tension as intended, and `utopia-refresh` re-runs the check as the manuscript
grows.

#note[
  The coherence findings feed the readers, too. The Inner Socrates roster includes
  a `utopian-architect` persona that opens each session grounded on your world's
  premises and its open tensions, so its questions press exactly where the argument
  is thinnest. With the genre set to `utopian`, even the general personas read the
  society as a case to be made rather than a place to be described.
]

#insight[
  A utopia fails not when it is implausible but when it is _incoherent_ — when it
  quietly assumes what it claims to have removed. The coherence checker exists
  because that failure is almost invisible from inside the draft: you declared the
  premise so long ago that you have stopped noticing you keep leaning on its
  opposite. The tool remembers what you declared and holds you to it.
]

#section("Draft, revise, produce")

The rest of the loop is fiction's. Draft the story that lives under the premise;
read it with the questioners; and when a stretch is finished, run `utopia-check`
over it to see whether the argument still holds where the new prose landed. Produce
with `export epub|pdf` as any novel would.

#pitfall[
  Don't let the argument eat the story. A utopia is still fiction: readers come for
  people, not for a treatise. Use the coherence checker to keep the world honest,
  but keep the inner readers (`Ctrl+B J`, `Ctrl+V o`) turned on the _prose_ — is the
  scene alive, does the character want something — with the same seriousness. The
  best books on this track are arguments you feel as stories.
]

#section("Hands-on: declare a premise and put it on trial")

+ Set up the world as any fiction (`inkhaven realworld new`, edit `world.hjson`, `inkhaven realworld compile --materialize`).
+ In the World book, write your founding rules as plain declarations — one premise per statement: _"No one owns land."_ _"Memory may be edited by the state."_ The clearer and more consequential each is, the more the checker has to reason about.
+ Run the coherence check: `inkhaven world utopia-check`. It reads your premises as a chain of claims and reports the tensions — the place where the abolished thing quietly returns.
+ Read each finding and decide. If a tension is a genuine slip, fix the prose. If it is deliberate — the crack that _is_ the story of a dystopia — mark it intended: `inkhaven world utopia-suppress <id>`.
+ As you draft new chapters, re-run the check over the grown manuscript: `inkhaven world utopia-refresh`. New prose can lean on an old premise's opposite without your noticing; this catches it.
+ Turn the architect's questions on the argument: `Ctrl+B J`, then choose the `utopian-architect` persona. It opens each session grounded on your premises and open tensions, and asks where the design is thinnest.

#recap((
  [Utopia is *fiction with an argument*: set `genre: "utopian"` and write your setting as a set of *premises* — the founding rules the book is built to test.],
  [Gather the world as any fiction would (world simulation, Characters, Threads), but state each premise and what it is meant to produce, plainly, in the World book.],
  [The track's signature tool is `world utopia-check` — it reads the premises as a chain of claims and finds where the society *cheats*; `utopia-suppress` marks an intended contradiction, `utopia-refresh` re-checks as you grow.],
  [The `utopian-architect` persona grounds its Socratic questions on your premises and open tensions — but keep the ordinary inner readers on the prose, so the argument stays a story.],
))
