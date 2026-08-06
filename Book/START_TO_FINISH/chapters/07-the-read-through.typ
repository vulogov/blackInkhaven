#import "../design.typ": *

#chapter(number: 7, title: "The Read-Through")

The draft of *The Ninth Lantern* is done. Not finished — done in the sense that
there is now a first chapter and a last one and a whole book in between, cold
lantern to the choice on the Mole, with nothing left blank. Every reader so far
has read the book *small*: the Inner Editor a paragraph at a time, SENTINEL a
chapter break at a time, CHORUS a character at a time. None of them has read it
the way the one reader who matters will — cover to cover, in order, once,
knowing nothing of the ending until it arrives. That reader is who you become
the day the book ships, and you cannot be them, because you know too much. You
know Aldous put the lantern out himself. You cannot un-know it and feel the
middle drag the way a stranger will.

*LECTOR* is Inkhaven's model of that stranger (the manual's Chapter 18 is its
full tour). It reads the finished manuscript forward, once, carrying only what a
first reader would carry, and reports two different things you must keep apart in
your mind: the *shape* of the read — its structure and its pacing — and the
*experience* of the read — its clarity, its attention, whether the stakes are
legible and the promises paid. It rewrites nothing. It is a reading instrument,
and this chapter is where we finally point it at the whole book.

#section("The shape of the read")

The Shape half measures each chapter's dramatic *intensity* straight from the
prose — no tagging, no plan required. It reads the signals a reader feels as
rising tension (dialogue density, a stakes-and-conflict lexicon keyed to the
project language, sentence-rhythm acceleration), penalises passages that
*summarise* instead of *dramatise*, and rewards a chapter that ends on a turn.
That gives the *realised* curve — how the book actually rises and falls — which
it lays against the *intended* curve of a story framework.

Which framework? *The Ninth Lantern* is a quiet mystery whose engine is not a
fight but a *recognition*: the whole book pivots on the moment the cold lantern
stops being an accident and becomes a deliberate act. That is the twist-driven
shape the East-Asian *kishōtenketsu* structure describes exactly — its energy
peaks not at a conflict climax but at the *ten*, the recontextualising turn — and
because it is conflict-optional, a tension-light book is not scored as though it
had failed to be a thriller. So we set the framework by hand rather than let the
default `genre` guess pick the Seven-Point:

#screen(caption: "Pinning the intended shape in the config")[```
  [lector]
  framework = "kishotenketsu"     # ki · shō · ten · ketsu
```]

Now run the read-through. It prints the whole read in one shot — the curve, the
per-chapter beat, and the ranked findings:

#screen(caption: "inkhaven readthrough — the measured curve vs kishōtenketsu")[```
Read-through — 9 chapter(s) · Kishōtenketsu
  measured   ▃▄▃▂▁▄▆█▃
  expected   ▂▂▃▃▄▅█▆▃

  ch  1  ▃ ▶   The Cold Lantern
  ch  3  ▃ ◉   What Toft Owed
  ch  5  ▁ ◉   Old Debts
  ch  6  ▄ ▶   Onto the Mole
  ch  7  █ ▶   What the Light Held
  ch  9  ▃ ◉   Relit

⚠ [shape_sag] the Kishōtenketsu shape wants rising tension
             around ch. 5 (~55%) but the prose reads flat (~14%).

4 reader finding(s): 1 concern, 3 notice(s).
```]

Read the two sparklines together and the book's problem is drawn in eight
characters. The *expected* line climbs steadily through the *shō* — the
development movement — toward the *ten* at chapter 7, where Mira learns what the
light was really holding back. The *measured* line does the opposite in the
middle: it *dips* at chapters 4 and 5, exactly where the book most needs to be
tightening. That is the saggy middle every mystery is prone to — the stretch
after the wrong suspect (Toft) is set aside and before the trail onto the Mole
picks up, where Mira searches and re-searches and the prose is *reporting* her
search rather than dramatising it. LECTOR names it `shape_sag` and gives you the
coordinates: chapter 5, about 55% of the way in, where the framework wants a rise
and the prose reads flat.

#callout(label: "The flag measures the read, it does not prescribe the fix")[
  `shape_sag` does not say "add a fight in chapter 5." It says a first reader's
  attention will slacken there against the shape you intended. The remedy is
  authorial and it is yours: cut the second search scene, or give the middle its
  own small turn — Mira finds the circled date, say, and misreads it — so the
  *shō* actually develops instead of marking time. LECTOR told you *where*; the
  page is still your job.
]

Alongside the curve, each chapter carries a *beat* on the scene ⇄ sequel axis. A
*scene* is the forward, external unit — goal, conflict, disaster — and shows `▶`;
a *sequel* is the reflective, internal one — reaction, dilemma, decision — and
shows `◉`; a chapter that is neither reads `·`. The value is in the *rhythm*, not
any single chapter: a long run of pure scene reads breathless with no room to
feel the cost, and a long run of pure sequel *sags*. Look back at the beat
column and the sag has a second signature — chapters 3 and 5 are both `◉`,
reflective back to back, with the search chapter between them carrying almost no
forward motion. The intensity dip and the sequel run are the same weakness seen
two ways.

#section("The experience of the read")

The second half is the reader's *experience*, and its defining discipline is that
it is *forward-only*. LECTOR walks the book from chapter one carrying the state a
first reader would carry — which characters and places have been met, which
threads hang open, how the energy has been running — and every finding uses only
the chapters read *so far*. A payoff in chapter 8 can never reach back and cancel
a confusion the reader felt in chapter 3, because the reader in chapter 3 had not
read chapter 8. That single rule is what makes this a *reader* and not an
*analyst*, and it is why its findings are the ones worth trusting. Five come out
of the walk, all deterministic and free:

#screen(caption: "The five forward-walk findings")[```
  confusion       an entity used before it is introduced —
                  "who is this again?"

  info_dump       too many new names to meet in one chapter

  attention_dip   a flat, eventless chapter where attention
                  drifts

  put_down_risk   a RUN of flat chapters — a likely place the
                  reader sets the book down

  unpaid_setup    a detail planted early and never paid off
```]

Two of them fired on *The Ninth Lantern*, and both are the kind of fault you
cannot see from inside your own book:

#screen(caption: "inkhaven readthrough — the ranked reader findings")[```
Reader findings — `The Ninth Lantern`

  ⊗ put_down_risk   ch 4–5 run flat — a likely put-down point
  ⚠ shape_sag       ch 5 wants a rise, the prose reads flat
  ⚠ confusion       "Bryn" used in ch 3, first met in ch 5
  ⚠ unpaid_setup    "the circled date" (ch 2) — never paid off
```]

The `confusion` catch is the one that stings, because it is invisible to the
author by construction. In chapter 3, writing Mira alone in Aldous's cottage, you
had her think *"not since Bryn left"* — a line that means everything to you,
because you know Bryn is Aldous's estranged nephew and you know he is about to
walk back into the story in chapter 5. But the *first reader* has never heard the
name. To them "Bryn" is a stranger dropped into a sentence as though they should
recognise him — *who is this again?* — two chapters before he is introduced.
LECTOR flags it because the walk met the token "Bryn" in chapter 3 and did not
meet the *character* Bryn until chapter 5, and it reports the gap. The fix is one
clause: make chapter 3's mention introduce him — *"not since Aldous's nephew Bryn
left"* — or move it later. Either way, a slip you could not have felt is now a
line you can see. This detector shares the Unicode-aware mention matcher SENTINEL
uses, so it works in every script Inkhaven supports, not English alone.

The `unpaid_setup` catch is the mystery writer's other recurring sin: a detail
raised with weight and then forgotten. In chapter 2, searching Aldous's post,
Mira notices his tide-chart pinned over the cot with *one date circled in ink*.
It reads like a clue — you meant it as atmosphere — and the reader files it as a
promise. But the book never returns to it: the circled date is never explained,
never paid. LECTOR walked the whole book forward, saw the setup opened and never
closed, and raised it. Now you have a clean choice, and it is a *good* choice to
have: pay it off (the date is the anniversary of the town's buried bargain — the
thing the lanterns hold back) and the detail becomes a thread; or cut it, and the
reader stops waiting for a bill that never comes. Left alone, it is exactly the
loose end a review will name.

#two_track(
  [In a novel these are plot faults: a name used before the reader can place it, a
  Chekhov's gun that never fires. The forward walk is the beta reader who does not
  yet know your ending and cannot fill your gaps with what you know.],
  [In non-fiction the same two flags serve an *argument's* flow. `confusion` is a
  term used before it is defined — a piece of jargon the reader meets three pages
  before the definition. `unpaid_setup` is a claim you promise to support ("we
  return to this in Chapter 9") and never do. Same instrument, reading the logic
  of an exposition instead of the shape of a plot.],
)

#section("The synthetic first-read — the one explicit cost")

Some things the deterministic walk simply cannot judge: whether a passage is
*genuinely* confusing to a human, whether the stakes are *legible* on the page,
whether engagement is *really* flagging where the intensity numbers merely dip.
For those, a model helps. LECTOR's one LLM feature — the *synthetic first-read* —
reacts to each chapter as a first reader who does not know the ending. It is
forward-only by construction: each call sees only a recap of what has been read
plus the current chapter, never the whole book at once.

It is never automatic. You ask for it — `inkhaven readthrough --deep`, or the `k`
key in the dashboard — and before each chapter the estimated cost is previewed
against your daily cap. Per Inkhaven's permissive principle the cost *informs*;
it never blocks. On our saggy middle, the synthetic reader says out loud what the
`shape_sag` number could only imply:

#screen(caption: "inkhaven readthrough --deep — the synthetic first-read")[```
  inkhaven readthrough --deep [--max-cost 8000] [--force]

  ch  5  synthetic first-read  · est. ~1,150 tokens (cap ok)
  → I keep waiting for something to happen. Mira searches
    the cottage a second time and I already know the room.
    Who is Bryn? — the name lands like I've missed a page.

  ch  7  synthetic first-read  · est. ~1,290 tokens (cap ok)
  → Oh — he put it out himself. That reframes the whole
    search; I want to go back and reread chapter 1.
```]

That chapter-7 reaction is the sound of a working *ten*: the reader recontextual-
ises everything behind them. The chapter-5 reaction is the sag felt from the
inside — boredom and the same stray "who is Bryn?" the deterministic walk caught,
now voiced. The two passes agree, which is the point: the free walk finds the
*where*, and the paid read confirms the *feel*, and you pay for the model only
when you choose to. Its findings arrive tagged `source: reader` and land beside
the deterministic ones, always marked so you know which came from a model.

#section("The dashboard — Ctrl+B Shift+A")

The command line is the batch view; inside the editor the read-through has its
own dashboard. `Ctrl+B Shift+A` opens a scrollable modal with the curve, the
beats, and the ranked findings together:

#screen(caption: "Ctrl+B Shift+A — the read-through dashboard")[```
┌─ Read-through · Kishōtenketsu · 9 ch ───────────────┐
│  measured   ▃▄▃▂▁▄▆█▃                               │
│  expected   ▂▂▃▃▄▅█▆▃                               │
│                                                     │
│ ⊗ put_down_risk   ch 4–5 run flat — likely put-down │
│ ⚠ shape_sag       ch 5 wants a rise, reads flat     │
│ ⚠ confusion       "Bryn" used ch 3, met ch 5        │
│ ⚠ unpaid_setup    "the circled date" (ch 2) unpaid  │
│                                                     │
├─────────────────────────────────────────────────────┤
│ ↑↓ scroll   Enter jump to chapter   k synthetic-read│
│ Esc close                                           │
└─────────────────────────────────────────────────────┘
```]

Arrow keys scroll it; `Enter` jumps straight to the flagged chapter in the editor
— select the `confusion` line and you land in chapter 3 on the stray "Bryn", ready
to fix it; `k` runs the cost-capped synthetic first-read, whose reactions post to
the Output pane's `readthrough` category; `Esc` closes. The deterministic findings
also ride the unified review pass, `Ctrl+B Shift+C`, which adds a `read-through`
line — each finding anchored to its chapter — alongside everything else the book
noticed about itself.

#screen(caption: "The LECTOR surface at a glance")[```
  inkhaven readthrough            the full report
  inkhaven readthrough --deep     + synthetic first-read
  inkhaven readthrough --json     structured output

  Ctrl+B Shift+A    the dashboard (k = synthetic read)
  Ctrl+B Shift+C    read-through line in the review pass

  ink.readthrough.report   the ranked findings
  ink.readthrough.curve    per-chapter measured/expected
  ink.readthrough.check    counts by kind + severity
```]

The three `ink.readthrough.*` Bund words expose only the *deterministic* read —
the synthetic first-read is deliberately not scriptable, because a cost-bearing
model call has no business firing silently from a hook. That is the same line
Inkhaven draws everywhere: the free, deterministic measurement is always
available to your scripts; the model call stays an explicit, deliberate act.

#callout(label: "What the read-through is not")[
  It is not a rewriter — it reports the read and hands every fix to the Editorial
  Pass (Chapter 8), where a finding becomes an action under the confirmed-diff,
  snapshot-first contract. It is not a per-paragraph reader — it is whole-book,
  forward, once. And it is not an oracle of taste: `shape_sag`, `confusion`,
  `unpaid_setup` are legible, grounded signals — a flat stretch, an unmet name, a
  loose end — never a verdict on whether *The Ninth Lantern* is *good*. That
  verdict is still yours to earn.
]

The read-through is the first time the book has been looked at *whole*, the way a
reader will look at it, and it did what only that vantage can: it found the
middle that drags, the name that arrives too early, the clue that never pays. None
of these was visible from inside a paragraph, and none of them is a change LECTOR
will make for you. What it hands you is a short, honest list of the places a
stranger will stumble — and, in the next chapter, a disciplined way to act on it.

#recap((
  [*LECTOR* (`inkhaven readthrough`) reads the finished book forward, once, as a
  first reader, and reports two things: the *shape* of the read and the
  *experience* of it. On *The Ninth Lantern* we pinned `framework =
  "kishotenketsu"` because the book peaks on a recognition (the *ten*), not a
  fight.],
  [The *Shape* half measured the intensity curve from the prose and laid it
  against the framework, flagging `shape_sag` at chapter 5 — the search-heavy
  middle that dips where the shape wants a rise — reinforced by a back-to-back
  `◉` sequel run on the scene ⇄ sequel axis.],
  [The *Audience* half walks *forward-only*: it flagged a `confusion` ("Bryn" used
  in ch 3, two chapters before he is introduced) and an `unpaid_setup` (the
  circled date on Aldous's tide-chart, planted in ch 2 and never paid) — faults
  invisible from inside the draft.],
  [The *synthetic first-read* (`inkhaven readthrough --deep`, or `k` in the
  dashboard) is the one explicit, cost-capped LLM pass — a chapter-by-chapter
  reaction that voiced the same sag and confusion the free walk found. Cost
  informs, never blocks; findings are tagged `source: reader`.],
  [`Ctrl+B Shift+A` opens the dashboard (curve · beats · findings; `Enter` jumps
  to a chapter, `k` runs the synthetic read); `Ctrl+B Shift+C` folds the
  read-through into the review pass. Everything is *advisory* — the fix is handed
  to the Editorial Pass, never made for you. The non-fiction reading is the same:
  `confusion` is a term used before it is defined, `unpaid_setup` a claim promised
  and never supported.],
))
