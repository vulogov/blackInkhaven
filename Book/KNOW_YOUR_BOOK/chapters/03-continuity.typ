#import "../design.typ": *

#chapter(number: 3, title: "Continuity — Where and When")

Continuity errors are the ones readers write letters about. A character in two
places on the same afternoon; a wound that heals and reopens; a sum of years that
does not add up; a name that changes spelling between chapters. None of them are
failures of imagination — they are failures of *bookkeeping*, and bookkeeping is
exactly what a tool should do so you do not have to. Inkhaven's continuity
intelligence is called SENTINEL: the book watching itself.

#section("One ledger, every detector")

SENTINEL unifies the deterministic continuity checks into a single ranked ledger.
It watches four kinds of slip, plus one invariant nobody usually catches:

#screen(caption: "inkhaven continuity check — the unified ledger")[```
Continuity ledger · 3 findings
co_location (1)
  ⊗ Mara is in the tower and the courtyard within one scene (ch. 3)
numeric (1)
  ⚠ "twelve years later" contradicts the dated events (ch. 8 → ch. 2)
introduce (1)
  ● "Cael" is named in ch. 2 before his first scene (ch. 5)
```]

- *co-location* — a character in two places at once, without the travel to connect them.
- *timeline* — events that orphan or overlap where they cannot.
- *numeric* — a span of years, a distance, a count that reverses itself.
- *character-fact drift* — a detail about someone that quietly changes.
- *referenced-before-introduced* — an entity named before its first scene (the
  invariant that KEN, next chapter, extends from *existence* to *knowledge*).

#section("The dashboard, and the jump")

`Ctrl+B Shift+I` opens the ledger in the editor. Scroll it, and press *Enter* on any
finding to jump straight to the paragraph where the slip happens — the whole point
is to get you to the fix in one keystroke, not to hand you a report you then have to
hunt through.

#term("Ambient watch")[
  With `continuity.ambient` on, SENTINEL re-checks *only what your edit touched* on
  every save — the paragraph's entities and its chapter — and shows the delta inline.
  It is the difference between a checkup you schedule and a nurse who never leaves the
  room.
]

#section("The subtle cases, on request")

The patterns above are deterministic and free. For the contradictions no pattern can
see — two paragraphs that disagree in meaning, not in a number — press `k` in the
ledger (or `inkhaven continuity check --coherence`) to run a cost-capped language
pass over the book. Explicit, budgeted, and off by default: the free checks earn
their keep first.

Continuity answers *where* and *when*. It is deliberately silent about one axis it
was never built to see — what a character *knows*. That axis is the next chapter, and
it is the reason this book exists.

#recap((
  [*SENTINEL* unifies the continuity checks — co-location, timeline, numeric,
  character-fact drift, and referenced-before-introduced — into one ranked ledger.],
  [`Ctrl+B Shift+I` opens it; *Enter* jumps to the slip. `continuity.ambient` re-checks
  what each save touched.],
  [The core is free; the LLM *coherence pass* (`k` / `--coherence`) is explicit and
  cost-capped for the contradictions patterns can't see.],
))
