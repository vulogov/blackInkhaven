#import "../design.typ": *

#chapter(number: 4, title: "Who Knows What, When")

The last chapter watched *where* your characters are. This one watches something no
tool watched before it: *what they know*. It is the axis a mystery lives or dies on,
and the most common invisible plot-hole in the whole craft — a character acting on
something they have not learned yet. Bob mentions the murder in chapter four; he is
not told of it until chapter six. A reader feels the wrongness and cannot place it.
You, three hundred pages deep, cannot hold every character's knowledge in your head.

KEN can. Its name is an old word for the range of what a person knows — *beyond his
ken* — and that is exactly what it tracks.

#term("KEN")[
  *KEN* extends SENTINEL's referenced-before-*introduced* rule to referenced-before-
  *known*. Where continuity flags an entity named before it exists in the story, KEN
  flags a character acting on a fact before they could know it. Same forward walk, a
  new axis: not existence, but knowledge.
]

#section("The grant — when could they know it?")

KEN never *guesses* what a character knows. It derives the *grant* — the earliest
moment they could know something — two deterministic ways. The first is free, from
the timeline you already keep: anyone present at an event knows that event from the
moment it happens. The second is a tag you place, telling KEN what matters:

#screen(caption: "The tags that declare knowledge")[```
  secret:the betrayal          — this topic is a secret
  know:the betrayal@Mara       — Mara learns it here
  know:the betrayal            — the scene's pov: character learns it
  reveals:the heir's true name — this event lets that fact slip
```]

#section("The use — and the break")

With grants in hand, KEN walks the book forward. Wherever a character *references* a
topic — speaks of it in attributed dialogue, or dwells on it in their own point-of-
view scene — it asks the one question: *could they know this yet?* If the reference
comes before the earliest grant, that is the break.

#screen(caption: "inkhaven knowledge — the epistemic check")[```
Knowledge check · 2 findings
  ⊗ premature_knowledge  Bob speaks of "the murder" in ch. 4 —
                         before learning it in ch. 6
  ⊗ leaked_secret        Sella references "the heir's true name" in
                         ch. 3 — never established to know it
```]

Three kinds of break: *premature_knowledge* (a reference before the grant),
*leaked_secret* (a `secret:` topic used by someone never told it), and
*dropped_reveal* (a knowledge you set up and then never spent — the epistemic version
of a gun on the mantel that never fires). KEN stays *silent* wherever it cannot
ground a break; it never invents one.

#section("Where it lives")

`inkhaven knowledge` runs the check from the command line and exits non-zero on a
hard break, so you can gate a draft before it goes out. In the editor,
`Ctrl+B Shift+Z` opens the findings grouped by kind — *Enter* jumps to the paragraph where
the character knew too much. And the same findings ride the revision worklist
(`Ctrl+V Shift+R`), where you *decide* the fix: cut the reference, move the reveal
earlier, or add the grant you forgot.

#callout(label: "Cost")[
  The whole check is a forward walk plus name-matching — *no model, and it costs
  nothing at any book size*; it scales with your tags, not your page count, and does
  nothing at all until you place a tag. Only the opt-in `--deep` pass — for a
  character who acts knowing *without naming* the fact — asks a language model, and it
  is cost-capped like every other language pass in this book.
]

#two_track(
  [For fiction this is the crown jewel — mystery, thriller, betrayal, any story with
  a secret. KEN is the reader's instinct given a memory: a secret stays secret until
  you choose to spend it.],
  [For non-fiction the axis rarely bites — an argument has no secrets to keep — so KEN
  stays quiet, which is itself the right answer.],
)

#recap((
  [*KEN* tracks who knows what, when — the referenced-before-introduced invariant
  moved from existence to *knowledge*.],
  [The *grant* (when they could know it) comes from timeline presence + `secret:` /
  `know:` / `reveals:` tags; a *use* before the grant is the break.],
  [Findings: *premature_knowledge*, *leaked_secret*, *dropped_reveal* — surfaced in
  `inkhaven knowledge`, the `Ctrl+B Shift+Z` dashboard, and the `Ctrl+V Shift+R`
  worklist. Deterministic and free; `--deep` is the only, opt-in, model touch.],
))
