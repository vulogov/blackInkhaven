#import "../design.typ": *

#chapter(number: 8, title: "What the Book Rests On")

CHRONICLE remembers what the *readers* found, draft to draft. But underneath the
findings sits a second kind of memory the last chapter did not touch — the memory
of what you *decided*. The city floats on the back of a sleeping leviathan. The
only way out is the sea gate in its flank. The harbour-master has known for years
and told no one. None of those is a fact you can look up or a finding a reader can
raise; each is a *choice* you made, and the whole book leans on it. Git remembers
your words. Nothing, until CANON, remembered your commitments — and so the one
question every revision asks had no answer: *if I cut this, what breaks?*

#term("Decision")[
  A load-bearing choice behind the story — a *world-fact*, *character trait*,
  *plot point*, *reveal*, or *setup* — recorded with its *grounds* (the decisions
  it rests on), a *commitment* level (how settled it is), and a link to the
  paragraph it came from. It is *canonicity*, not truth: not "is this correct?"
  but "how firmly has the author committed to it?"
]

#section("Where decisions come from")

You do not keep a story bible by hand. Decisions reach the ledger two ways, and
the second is the important promise: a model can *propose*, but nothing enters
without your word.

A `rel:` relationship tag you already write — `rel:mentor:Vasa:Iri` on a
paragraph — becomes a character-trait decision the moment you save, for free, with
no model. And when you want more, `inkhaven canon harvest` asks a model to read a
scene and *propose* decisions in your project's language. They do not enter the
ledger. They are *staged*, and one command — `canon accept` — is the only thing
in the whole feature that writes.

#screen(caption: "The model proposes; you decide")[```
inkhaven canon harvest ch-1     # a model reads, and proposes …
inkhaven canon staged           # … you review the staged list …
    4 staged proposal(s) — not yet in the ledger:
      [world-fact]  The city of Vell floats on a sleeping leviathan.
      [reveal]      Mara never told anyone the tremors are the beast.
inkhaven canon accept           # … and only this commits them.
```]

#section("The one question — what breaks if I cut this?")

This is why the ledger is worth keeping. Because each decision records what it
rests on, CANON can walk the chain and name the *blast radius* of a cut — before
you make it, not three chapters later when the book quietly stops adding up.

#screen(caption: "canon impact — the cost of a cut, in advance")[```
inkhaven canon impact b3:lypx
    If cut:  b3:lypx  [world-fact]  The city floats on a leviathan.
      2 decisions would dangle:
        b3:x3k7  [plot-point]  The escape uses the sea gate in its flank.
        b3:q0m2  [reveal]      The tremors are the beast waking.
```]

Cut the leviathan and the ledger tells you, at once, that the escape route and
the central reveal both come loose. Its mirror, `canon why b3:x3k7`, runs the
chain the other way — the grounds a decision stands on — so you can see not just
what depends on a choice but what the choice itself depends on.

#section("How settled is it — and am I building on sand?")

A decision carries a commitment level, from *floated* through *drafted*,
*committed*, and *canonical*, to *retconned*. Marking them lets CANON catch the
quiet danger of a scene built on a premise you never really settled:

#screen(caption: "canon check — a canonical scene on a floated footing")[```
inkhaven canon commit b3:x3k7 --level canonical
inkhaven canon check
    ⚠ b3:x3k7  [plot-point · canonical]  The escape uses the sea gate
       rests on  b3:lypx  [world-fact · floated]  the leviathan
       — canonical, but its ground is only floated.
```]

You have committed hard to the escape while the world-fact it needs is still a
maybe. Nothing is wrong with the prose; the *foundation* is soft, and only a
ledger that tracks both the decisions and their footings can see it.

#two_track(
  [For the *novelist*, the decisions are the secrets and the scaffolding — the
  reveal you must not spend early, the premise a whole act leans on. `impact`
  turns "can I cut this scene?" from a guess into a list.],
  [For the *non-fiction* writer, a decision is a load-bearing claim, and its
  grounds are the argument beneath it. Cut a premise and `impact` names the
  conclusions that now dangle unsupported — the same question, one field over.],
)

#section("In the editor, and when two of you disagree")

The reader hub (`Ctrl+B *` → *Canon*) opens the ledger as a scrollable dashboard —
every decision with its kind and `«commitment»` — and `⏎` jumps to the paragraph a
decision came from, the same jump the other ledgers give you. And CANON meets the
Editorial Pass: if you ever `canon merge` a second ledger — a co-writer's, another
device's — and the two disagree on whether something is canonical, that
*commitment fork* joins the worklist (`Ctrl+V Shift+R`) as a brief. There is no
line to rewrite; you reconcile the *ledger*, not the prose. A ledger you keep
alone never forks, so the day it appears is the day it matters.

#callout(label: "Derived, and safe to lose")[
  The ledger lives in the project's own `canon.cbor`, beside your prose but never
  mixed into it — and it is *derived*: rebuildable by re-harvesting the
  manuscript. Lose it and you lose a re-harvest, never a word of the book. The
  queries cost nothing and run in milliseconds; the one model path, harvest, is
  opt-in and confirmed. It is *advisory*, like everything in this book.
]

#recap((
  [*CANON* is the memory of your *decisions* — world-facts, plot points, reveals —
  each recorded with its *grounds* and a *commitment* level (floated →
  canonical). It tracks canonicity, not truth.],
  [Decisions enter from the `rel:` tags you save (free, deterministic) or from an
  opt-in `canon harvest` that *stages* proposals; `canon accept` is the only thing
  that writes — the model proposes, you decide.],
  [The headline is `canon impact` — *what breaks if I cut this?* — the blast radius
  of a decision before the cut; `canon why` runs the grounds the other way;
  `canon commit` / `canon check` flag a firm choice resting on a soft one.],
  [The reader hub (`Ctrl+B *` → *Canon*) is the dashboard; `canon forks` (after a
  `merge`) reaches the Editorial Pass as a brief. The ledger is *derived* and safe
  to lose — a re-harvest, never prose.],
))
