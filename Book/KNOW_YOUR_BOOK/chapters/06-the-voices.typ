#import "../design.typ": *

#chapter(number: 6, title: "The Voices")

Give three characters the same lines and a weak book is exposed: they all sound like
the author. A strong book gives each a voice you could recognise with the dialogue
tags stripped away — a rhythm, a vocabulary, a way of hedging or not. Knowing whether
your cast actually sounds like different people is hard from the inside; you hear the
voice you *meant*, not the one on the page. CHORUS listens for you.

#term("CHORUS")[
  *CHORUS* profiles the voices of your book at the scale of the whole cast. It
  fingerprints each character's speech, measures how *distinct* they are from one
  another, and watches the discipline of point of view and register — synthesised by
  a reader in the inner family, the *Inner Stylist*.
]

#section("A fingerprint, and a distinctiveness matrix")

CHORUS builds a statistical fingerprint of each character's dialogue — sentence
rhythm, vocabulary richness, how often they ask, exclaim, hedge — and then z-scores
the whole cast against itself to find the voices that read *alike*.

#screen(caption: "inkhaven chorus — the voices, compared")[```
Voice distinctiveness · 5 speakers
  most distinct  Grandmother  (archaic, long clauses)
  least distinct Joren ↔ Cael  (0.31) — they read the same
  ⚠ two flat voices: consider sharpening Cael's diction.
```]

Sparse voices — a character with three lines — are profiled but never flagged; there
is not enough to judge, and CHORUS says so rather than inventing a verdict.

#section("Whose head, and does the tense hold")

The other pillar is *discipline*. Declare a scene's point of view with a `pov:` tag,
and CHORUS flags a *head-hop* — a named non-viewpoint character whose interiority
leaks onto the page — and, in English, a *tense slip*.

#callout(label: "Honest about Russian")[
  Tense detection is *English-only, by design*. Russian tense is aspect, and the
  historical present is legitimate prose; nothing here models aspect, so CHORUS says
  so rather than false-flag it. Character voice and head-hop work in every supported
  language.
]

#section("The Inner Stylist")

All of this — voice, distinctiveness, drift, point of view, register — is gathered by
the *Inner Stylist*, the seventh reader in Inkhaven's inner family, into warm
observations you can act on (`Ctrl+B J → Y`, or the `Ctrl+B Shift+C` review pass). It
coaches; it never rewrites. A voice is yours to give; CHORUS only tells you whether
you gave it.

#two_track(
  [For fiction this is the difference between a cast and a crowd — the assurance that
  the reader can tell who is speaking with the tags removed.],
  [For non-fiction voice matters less, but *register drift* — a chapter that slides
  from plain into jargon and back — is exactly the kind of unevenness CHORUS catches.],
)

#recap((
  [*CHORUS* profiles the whole cast's voices: a per-character fingerprint plus a
  *distinctiveness matrix* that flags voices reading alike.],
  [It watches *discipline* too — head-hops against a `pov:` tag, and English tense
  slips (Russian excluded by design).],
  [The *Inner Stylist* synthesises it into coaching observations; it never rewrites.],
))
