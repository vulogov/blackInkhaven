#import "../design.typ": *

#chapter(number: 7, title: "Did It Get Better?")

Every intelligence in this book diagnoses the draft *in front of you*. But revision
is a leap of faith unless you can see it working, and the honest question at the end
of a hard week — *did I actually make it better, or did I just move the furniture?* —
has no answer if nothing remembers what the book measured last time. CHRONICLE
remembers. It is the book's memory of its own drafts.

#term("Milestone")[
  A *milestone* is a snapshot of every reader's verdict at one moment — taken with
  `inkhaven chronicle mark "draft-2"`. It captures the whole diagnostic state (every
  finding, tallied and fingerprinted) so a later draft can be measured against it.
]

#section("Mark, then trend")

Stamp a milestone before a serious pass. Afterward, `inkhaven chronicle` captures the
book again and *trends* it against that mark — every count read the honest way, where
*fewer is better*, and regressions sorted to the top so collateral damage cannot hide:

#screen(caption: "inkhaven chronicle — since the last milestone")[```
Chronicle — since "draft-2" (2026-08-03) → now
  findings          31 → 27   ▼  4 fewer
  errors             4 →  2   ▼  cleared 2
  shape sag          3 →  1   ▼  improved
  confusion (ch.7)   0 →  1   ▲  NEW — introduced by your edits
```]

#section("Cleared, and introduced")

The line that matters most is the split. Because every finding carries a stable
identity, CHRONICLE knows exactly which ones your revision *cleared* and which *new*
ones it *introduced* — proof the work landed, and an early warning on the problem
you created reaching for the fix:

#screen(caption: "The revision, weighed")[```
  ✓ 6 cleared     ▲ 1 introduced     · 20 unchanged

  introduced (new since the last mark):
    ⚠ confusion   ch. 7   an entity used before it's introduced
```]

Press *Enter* on an introduced finding in the `Ctrl+B Shift+U` dashboard and you jump
straight to the paragraph your last edits broke. Good revision often trades one
problem for another; this is how you catch the trade before a reader does.

#callout(label: "Pure measurement")[
  CHRONICLE has *no prose-write path anywhere*. It only ever measures. And because it
  reads findings the other intelligences already computed, it costs nothing to keep a
  running record of your book's whole health, draft to draft.
]

CHRONICLE closes the loop the rest of this book opens: the readers diagnose, you
revise, and CHRONICLE tells you whether it worked.

#recap((
  [A *milestone* (`chronicle mark`) snapshots every reader's verdict; `inkhaven
  chronicle` trends the live book against it, every count *fewer-is-better*.],
  [The signature move is *cleared vs. introduced* — which findings your revision
  resolved, and which new ones it created.],
  [`Ctrl+B Shift+U` opens the dashboard; *Enter* jumps to an introduced finding. Pure
  measurement — it never edits, and it costs nothing.],
))
