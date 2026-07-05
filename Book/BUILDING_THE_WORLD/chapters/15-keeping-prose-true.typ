#import "../design.typ": *

#chapter(number: 15, title: "Keeping Your Prose True")

The last chapter put the world at your cursor so it could hand you a season, a
distance, a people, before you wrote. This chapter turns the same world around to
face the other way — so that after you have written, it can read what you wrote
and tell you where you contradicted it. This is the quiet, unglamorous dividend
of all that building, and in truth it is the biggest one. A world you can consult
is useful. A world that *checks you* is transformative, because the mistakes that
wreck a setting are never the ones you look up — they are the ones you never
thought to doubt.

#insight[
  The real dividend of a built world is that it catches you contradicting it. You
  will not deliberately write snow into your tropics or send a cart three hundred
  miles in a morning; you will do it *by accident*, in a fast draft, and read
  straight past it a dozen times because it looked fine. The world argues back —
  it holds the facts you set and measures your prose against them — and that is a
  thing no binder has ever done.
]

#section("Reading your prose against the world")

The tool for this is the fact-checker. You hand it a passage — a sentence you are
unsure of, a paragraph you just wrote — and it reads that prose against the world
you compiled.

```
realworld fact-check --text "They rode from the capital to the far coast in a single afternoon."
```

You can check loose text with `--text`, as above, or check a paragraph already in
your manuscript by its identifier with `--paragraph <id>`. Either way the checker
looks for the kinds of claim a world can adjudicate: *travel times* against the
real distances and paces, *climate* against the biome a place sits in, *date
coherence* against the calendar, and more besides. The afternoon ride above will
come back flagged, because the world knows how far the far coast is and how long
an afternoon lasts, and the two do not meet.

#term("Fact-check (against the world)")[
  Reading a passage of prose against the compiled world to find claims that
  contradict it — a journey too fast for its distance, weather that fights the
  local climate, a date that does not fit the calendar. It is not a grammar or a
  style check; it takes no view on your sentences. It has exactly one question:
  *is this consistent with the world you built?*
]

#term("Continuity error")[
  A place where the story contradicts itself or its world — the crossing that took
  four days on page ten and one day on page ninety, the harvest festival held in
  the dead of the local winter, the river that flows the wrong way down its own
  valley. A continuity error is rarely a failure of imagination; it is a failure
  of *bookkeeping*, which is exactly the sort of failure a machine is good at
  catching.
]

#section("Two speeds of checking")

The fact-checker runs on two tracks, and it is worth knowing which is doing the
work. The first is a *fast, deterministic* track: it reads the structured claims
it can measure directly — a named journey, a stated season, a date — and settles
them against the world's own numbers, instantly and the same way every time.
This is the track that catches the afternoon ride, because distance and pace are
arithmetic.

The second is a slower *LLM track* that reads the prose more as a reader would,
catching the softer contradictions that are not stated as tidy claims — an
implied warmth in a scene the calendar places in deep winter, a detail that only
reads as wrong once you understand what the sentence is describing. The
deterministic track is your everyday check; the LLM track is there for the harder
reading, when you want a second pair of eyes over a passage rather than a
measurement of it.

#note[
  Because the two tracks differ in cost, they differ in when you reach for them.
  The deterministic track is cheap enough to run constantly, on a line you are
  unsure of, as often as you like. The LLM track is worth spending on a finished
  scene, a chapter you are about to call done — the moments where a careful,
  slower reading pays for itself.
]

#section("Why a checked world does not drown you")

There is an obvious fear here, and it is worth meeting head-on: a checker strict
enough to be useful sounds like a checker that will flag every deliberate wonder
in your book. If your world has a witch who summons frost in high summer, a
climate check that does not know about her will flag that frost every single
time, and a tool that cries wolf on your own magic is a tool you will switch off
inside a week.

This is exactly what the *magic ledger* is for. Back when you drew the line
between what your world computes and what you declare, the magic you set down was
recorded as a set of *declared exceptions* to physics — each rule saying what it
covers, and whom and where and when it applies. The fact-checker consults that
ledger before it flags anything, and *suppresses* any contradiction your ledger
already sanctions. The witch's summer frost, covered by a rule with `covers`
including `climate_anomaly` and an `applicable_to` that names her, is not a
continuity error; it is a fact of your world, and the checker knows it.

#insight[
  A one-time declaration buys you endless quiet. You wrote the magic rule once,
  deliberately, when you decided what your world's exceptions were. Because of
  that single act the checker can stay strict *everywhere else* without ever
  crying wolf on the one wonder you meant. This is the deep reason the ledger
  exists: not to permit magic, but to let the check around it stay honest.
]

#note[
  The magic ledger is precisely why a checked world does not bury you in false
  warnings. A checker with no notion of sanctioned exceptions must either be too
  loose to help or too shrill to keep on. The ledger gives the fact-checker a
  third option — strict about physics, silent about the magic you declared — and
  that is the only setting a working author can actually live with.
]

#pitfall[
  The commonest way to get a *true* flag you did not expect is to write weather
  that fights the biome — a snowfall in a place your climate layer made
  tropical, a drought in a rainforest, a hard freeze on a coast that never
  freezes. The checker will catch it. But note the condition buried in that
  sentence: it can only catch this *if you built the climate*. Skip the climate
  layer, or leave a place's biome undecided, and there is nothing for the weather
  to contradict — the check falls silent not because your prose is true but
  because the world has nothing to say. The check is only ever as good as the
  world beneath it.
]

#tryit[
  Run `realworld fact-check --text "They rode from the capital to the far coast in a single afternoon."` and read the flag it returns — the distance, the pace,
  the verdict. Then take a real paragraph from your draft, one set somewhere with
  a decided climate, and check it with `--paragraph <id>`. If it comes back clean,
  you have earned a small, specific confidence; if it does not, you have caught a
  contradiction while it was still cheap to fix.
]

#recap((
  [The *fact-checker* reads your prose against the compiled world —
   `realworld fact-check --text "…"` or `--paragraph <id>` — and flags travel
   times, climate, date coherence, and more.],
  [It runs on two tracks: a *fast deterministic* one for measurable claims, and a
   slower *LLM* one for the softer contradictions a reader would feel.],
  [The *magic ledger* suppresses declared exceptions, so a sanctioned wonder is
   never flagged — which is what lets the check stay strict without crying wolf.],
  [The checker only catches what the world can adjudicate: weather that fights the
   biome is flagged *only if you built the climate*.],
  [The genuine payoff of a built world is that it *argues back* — it catches the
   continuity errors you would otherwise read straight past.],
))
