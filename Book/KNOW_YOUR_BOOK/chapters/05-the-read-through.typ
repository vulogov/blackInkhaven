#import "../design.typ": *

#chapter(number: 5, title: "The Read-Through")

Every intelligence so far has read your book *small* — a paragraph, a break, a fact.
But the reader who matters most does not read small. They read *forward, once,
whole*, not knowing the ending, and their experience of the whole is the thing you
most want to know and can least see, because you already know how it turns out. KEN's
sibling, LECTOR, is that first reader — the book reading itself, end to end.

#term("LECTOR")[
  *LECTOR* reads the manuscript the way a first-time reader does: forward, once,
  whole. It reports two things — the *shape* of the read (its dramatic rise and fall)
  and the *experience* of it (where a reader gets lost, bored, or set the book down).
]

#section("Shape — measured from the prose")

LECTOR measures each chapter's dramatic intensity *from the prose itself* — the
density of dialogue, a stakes-and-conflict vocabulary, the acceleration of the
sentences, the turn at a chapter's end — with no tagging required. Then it reads that
realized curve against the shape your chosen framework intends, and marks the
*saggy middle* where the story means to rise but the prose reads flat.

#screen(caption: "inkhaven readthrough — the shape of the read")[```
Read-through · 12 chapters · Hero's Journey
  measured   ▂▃▄▂▁▁▃▅▄▆█▃
  expected   ▁▂▃▄▅▅▆▇▇██▂
  ⚠ [shape_sag] the shape wants a rise around ch. 5 (~55%) but the
                prose reads flat (~12%).
```]

Six frameworks are built in — including the four-movement, conflict-optional
*kishōtenketsu* — and the right one is suggested from your declared genre unless you
name it yourself.

#section("Audience — what a first reader trips on")

The other half walks the book forward carrying the *reader's* state — who they have
met, what is still open, how the energy runs — and flags the problems a first reader
actually hits, forward-only, so a later payoff never excuses an earlier dip:

#screen(caption: "The reader-experience findings")[```
  confusion       an entity used before it's introduced
  info_dump       too many new names in one chapter
  attention_dip   a flat, eventless chapter
  put_down_risk   a run of flat chapters — a likely place to stop
  unpaid_setup    a promise raised but never paid off
```]

#section("A reader's eyes, on request")

The forward walk cannot judge whether the stakes *land*. For that,
`readthrough --deep` (or `k` in the `Ctrl+B Shift+A` dashboard) runs a single, cost-capped
*synthetic first read* — a language model reacting chapter by chapter as a reader who
does not know the ending, seeing only a recap of what came before. It is the one
place the book gets a stranger's opinion, and it is always something you ask for.

LECTOR does not replace your first reader. It catches what you would wince at before
you hand them the pages.

#recap((
  [*LECTOR* reads the book forward, once, whole — reporting its *shape* (intensity vs.
  the framework's intended curve) and the *audience* experience.],
  [Shape is measured from the prose (no tagging); audience flags confusion, info-dump,
  attention-dip, put-down risk, and unpaid setup, forward-only.],
  [`inkhaven readthrough` and `Ctrl+B Shift+A`; the cost-capped *synthetic first read*
  (`--deep` / `k`) is the only model touch.],
))
