#import "../design.typ": *

#chapter(number: 6, title: "Voices and Threads")

By now *The Ninth Lantern* has a world, a cast, and a secret. Mira Fenn goes
looking for a keeper who is gone; Sella Vale, the harbourmaster, holds the
town's official story; Toft the oil-merchant is the wrong suspect everyone
reaches for first; Bryn Crane drifts back into town knowing more than he says.
On the page, though, a cast is not five people until it *reads* as five people —
and a mystery is not a mystery until every promise it makes is one the book
remembers to keep. This chapter is about those two jobs: making the voices
distinct, and tracking the plot's promises from the moment they are made to the
moment they pay off.

Nothing here rewrites a word. The tools in this chapter *read* the draft and
report what they find — a pair of characters who blur together, a viewpoint that
slips, a line of speech no one can be sure is spoken, a promise the manuscript
has not yet redeemed. Every fix stays yours. They are advisory readers, and the
first of them holds the whole cast in mind at once.

#section("Do they sound like themselves?")

The reader we reach for first is *CHORUS* — the voice reader at book scale (the
manual's Chapter 18 is its full tour). It profiles each character's dialogue on
the same axes the narrator is measured on — sentence rhythm, lexical diversity,
hedging, interiority — and then does the thing every novelist is quietly afraid
of: it lines the voices up against one another and looks for two that read the
same. Run it on the draft:

#screen(caption: "inkhaven chorus voices — a signature card + the matrix")[```
Character voices — `The Ninth Lantern` [en]

  Mira            confidence ●●●●   Δ from cast mean
    rhythm      ▇▇▅▁   clipped          −0.2
    diversity   ▇▇▇▅   healthy          +0.3
    hedging     ▇▁▁▁   asserts          −0.4
    interiority ▇▇▇▁   high             +0.5

  Distinctiveness
    ⚠ Sella ↔ Toft  read alike (z-dist 0.34 < 0.50)
    ✓ all other pairs distinct

  Aldous          confidence ●○○○   too few lines to judge

  Drift
    ● Bryn  ch 7 drifts from his ch 3 voice
```]

Three things in that one screen are worth slowing down for. The headline is the
`⚠` line: *Sella and Toft read alike.* On reflection it is exactly the pair you
would expect to blur — both are older, transactional, town-official in register,
both speak in short declaratives about supply and duty — but you did not *notice*
it while writing them a chapter apart. CHORUS z-scores every voice against your
own book's spread, so the comparison is relative to *this* manuscript, not to
some external notion of "distinct"; a z-distance of `0.34`, under the `0.50`
floor, means their fingerprints sit almost on top of each other.

The second thing is Aldous. He is missing from the opening and barely speaks, so
he shows a `●○○○` confidence badge and *no* verdict. CHORUS profiles sparse
speakers but refuses to flag a voice it cannot measure — it will not tell you
Aldous blurs with anyone, because it does not have the lines to know. That
restraint is deliberate: a reader that cried wolf on every under-written walk-on
would be one you learned to ignore.

The third is Bryn, flagged for *drift* — his chapter-7 voice has wandered from
where it started in chapter 3. Drift is always measured against a character's
*first* chapter, which turns a vague worry ("does Bryn still sound like Bryn?")
into a specific place to look.

#callout(label: "The flag is a question, not a verdict")[
  CHORUS measures *consistency* and *distinctiveness*, never quality. "Sella and
  Toft read alike" is not "one of them is badly written" — it is "a reader may
  not be able to tell them apart on dialogue alone." If two voices are *meant* to
  echo — twins, a chorus of identical guards — you tell CHORUS once with
  `chorus.distinct_ignore_pairs` and it stops raising them. The judgement of
  whether the echo is a problem stays with you.
]

The fix for Sella and Toft is authorial and small: give one of them a verbal
tic the other lacks. Toft hedges and qualifies (he is, after all, covering for a
delivery he cannot account for); Sella never hedges — she *rules*. Sharpen that
contrast in a half-dozen lines and their fingerprints separate. You do not need
CHORUS to make the change; you need it to have told you the change was owed.

#subsection("The Inner Stylist — turning the numbers into judgement")

The signature cards are numbers. To hear them as advice, open the *Inner
Stylist* — the seventh member of Inkhaven's inner-reader family, the one whose
whole job is voice at scale. You reach it through the family hub: `Ctrl+B J`
opens the Inner Socrates overview, and `Y` from there opens the Stylist.

#screen(caption: "Ctrl+B J → Y — the Inner Stylist overview")[```
┌─ Inner Stylist · voice at scale ────────────────────┐
│ Praise                                              │
│   the cast reads distinct — 9 of 10 pairs separate  │
│ Note                                                │
│   Bryn's voice drifts plainer after ch 7            │
│ Concern                                             │
│   Sella ↔ Toft read alike — consider sharpening one │
│                                                     │
├─────────────────────────────────────────────────────┤
│ F synthesise → Output   E engage AI coach → Thoughts│
│ R report dashboard      Esc close                   │
└─────────────────────────────────────────────────────┘
```]

The Stylist reads all of CHORUS's measurements and offers a few grounded
observations in the inner family's own register — *Praise*, *Note*, *Concern*,
"I notice…", never a rewrite. `F` synthesises them to the Output pane, `R` opens
the full voice dashboard, and `E` engages an AI coach into the Thoughts pane for
grounded coaching (still never a rewrite). A judgement you have weighed and
rejected — say you decide Sella and Toft are *supposed* to sound like two faces
of the same officialdom — can be silenced for good with `chorus stylist
--suppress <key>`, so it never nags you again.

#two_track(
  [In a novel you want the cast to *diverge* — Mira must not sound like Sella. The
  `⚠` read-alike flag is a warning that two people have collapsed into one voice,
  and the fix is to sharpen a contrast until the reader can tell them apart in
  the dark.],
  [In a multi-author or multi-section non-fiction book the goal *inverts*: you
  want the chapters to *converge*. Run CHORUS across the sections and a voice that
  reads alike is good news — it means the seams don't show. Here you hunt the
  *outlier*: the co-author whose chapter sits apart, the appendix that drifts
  casual. Same instrument, opposite reading.],
)

#section("Whose eyes are we behind?")

Two viewpoint errors survive line-editing because they are structural, not local
— they live in the shape of a scene, not in any one sentence. The first is the
*head-hop*: a scene that belongs to one character suddenly showing you the inner
life of another. In a Mira scene, `Toft wondered whether…` is a leak — we are
not behind Toft's eyes, and for one clause the camera jumped heads.

CHORUS can only catch this if it knows whose scene it is, so you tell it. A
scene's viewpoint is declared with a paragraph tag — the same lightweight tag
mechanism the rest of Inkhaven uses — placed on the scene's opening paragraph:

#screen(caption: "Declaring a scene's viewpoint with a pov: tag")[```
  pov:Mira         single POV — anyone else's inner life
                   is a leak

  pov:first        first person — ANY named character's
                   interiority is a leak

  pov:omniscient   deliberately multi-POV — head-hop off
  pov:multi        an alias for pov:omniscient
```]

*The Ninth Lantern* is a close-third mystery: we ride with Mira and learn only
what she learns — which is the engine of the whole book, because the reader must
not know what Aldous knew before Mira uncovers it. So every scene of hers carries
`pov:Mira`. Where you forget the tag, CHORUS falls back to *inferring* the
viewpoint from who is mentioned most, and says so. Now run the discipline scan:

#screen(caption: "inkhaven chorus scan — POV, head-hop, tense, register")[```
Voice-discipline scan — `The Ninth Lantern` [en]

  POV / head-hop
    ⚠ ch 4 · Mira-POV — "Toft wondered" (interiority leak)
    ● ch 2 · POV inferred (Sella) — no pov: tag

  Tense
    ✓ past-tense throughout — no slips

  Register
    ● ch 7 drifts plainer vs ch 1 (Δ 0.10 > 0.08)
```]

The chapter-4 catch is the real prize. In the scene where Mira first suspects
Toft, you wrote a line from *inside* Toft's head — "Toft wondered if she could
smell the missing oil on him" — a small, natural slip that reads fine in
isolation and quietly breaks the book's contract with its reader. CHORUS flags it
because the scene is tagged `pov:Mira` and Toft is not Mira. The chapter-2 `●` is
softer: a Sella scene with no `pov:` tag, so CHORUS inferred her viewpoint and is
telling you it guessed. Tag it and the guess becomes a rule.

CHORUS is honest about its own limits here. It has no grammar parser, so it
catches interiority attributed to a *named* subject — "Toft wondered" — but not a
bare pronoun whose antecedent is someone other than the viewpoint. The tense
check, likewise, is a four-language heuristic: it confirms *The Ninth Lantern*
holds its past tense throughout, and it would flag a lapse into present — but it
covers English, German, French, and Spanish only.

#callout(label: "Russian is excluded from the tense check by design")[
  If you were writing this book in Russian, `chorus scan` would tell you plainly
  that it is *not* checking tense — because Russian narrative tense is governed by
  aspect, and the historical present and perfective/imperfective interleaving are
  legitimate devices, not slips. A naïve past-to-present flag would be *wrong*
  there, so CHORUS declines rather than false-flag. Character voice and head-hop
  still work in Russian; only the tense pillar bows out.
]

#section("Who is talking?")

Dialogue is the most demanding prose mode there is: a reader must always know who
is speaking, an over-decorated attribution verb pulls the eye, and an unbroken
run of talking heads floats the scene off its floor. The *DIALOGUE* reader
(DIALOG-1) measures speech as a mode of its own — deterministically, in the five
languages, with no model call. Run the scan:

#screen(caption: "inkhaven dialogue scan — a pre-submission gate")[```
Dialogue scan — `The Ninth Lantern` [en]
──────────────────────────────────────────────────────
  dialogue spans detected   28
  zero attribution           2
  said-bookism density       ch 5 above baseline
  talking-head sequences     1
──────────────────────────────────────────────────────
  [ch.5 · a1f2…] "…she said, he ejaculated" — non-neutral
                 tag verb above the book's own baseline
  [ch.6 · 9c3b…] unattributed speech — no tag or
                 character name within range
──────────────────────────────────────────────────────
```]

Two findings, both worth acting on. The chapter-5 flag is a *said-bookism*: in
the tense exchange on the quay you reached past `said` for `he ejaculated`, and
the density of these ornamental tags in that chapter has climbed above your own
book's baseline. DIALOGUE does not ban them — a single `whispered` in the right
place earns its keep — it tells you when a chapter has started leaning on them.
The chapter-6 flag is *zero-attribution*: a line of speech with no tag and no
nearby name, in a spot where the reader cannot be sure whether it is Bryn or
Mira talking. Note that DIALOGUE allows an established two-speaker exchange to
run untagged for a stretch before it fires — readers track a back-and-forth fine
— so this flag means the exchange genuinely lost its footing.

Because zero-attribution is the one dialogue fault a reader *cannot* recover
from, `dialogue scan` exits non-zero when it finds one, which lets you wire it
into a pre-submission check — `inkhaven dialogue scan --findings zero-attribution
|| echo "fix attribution first"`.

#subsection("The fingerprint view — Ctrl+V Shift+Q")

`inkhaven dialogue profile` builds a six-metric signature for each character from
the lines confidently attributed to them — terse or expansive, asking or
declaring, hedging or asserting:

#screen(caption: "inkhaven dialogue profile — per-character signatures")[```
Character    Utts  Avg words  MATTR  Quest.  Excl.  Hedge
Mira           41       9.4   0.72    0.29   0.05   0.014
Sella          33      10.1   0.68    0.10   0.18   0.031
Toft           22       9.8   0.66    0.12   0.16   0.028
Bryn           19      13.6   0.71    0.21   0.03   0.037
```]

Read across Sella's and Toft's rows and CHORUS's earlier warning turns concrete:
their numbers sit almost on top of each other — similar length, similar low
question rate, similar exclamatory heat. Mira, terse and questioning, and Bryn,
expansive and hedging, stand clearly apart. This is the same story the
distinctiveness matrix told, now in the dialogue's own figures.

In the editor, `Ctrl+V Shift+Q` opens the fingerprint view for the *nearest*
character — one named in the paragraph you are sitting in, else the
most-speaking — drawn as ASCII bars with a compare line for the next speaker. The
mnemonic is `Q` for *Quote*. It is built from confidently-attributed lines, so
run the review pass (`Ctrl+B Shift+C`) or a `dialogue scan` once to populate it.

#section("The promises a book makes")

A voice is a thing a book *has*; a thread is a promise a book *makes*. *The Ninth
Lantern* opens by making a big one — the ninth lantern is cold and its keeper is
gone — and the whole mystery is the slow discharge of that promise: suspicion
falls the wrong way onto Toft, Mira follows Aldous's trail onto the Long Mole,
and the reveal lands (Aldous put the lantern out *himself*, and why). A thread is
how you make that promise explicit so the book cannot forget to keep it.

#term("A plot thread")[
  A named narrative arc — the cold-lantern mystery, Bryn's family grudge, the
  town's buried bargain — tracked from its opening hook through its midpoint pivot
  to its payoff. In Inkhaven a thread is an HJSON-fronted paragraph under the
  *Threads* system book, carrying a status, a weight, its opening/midpoint/payoff
  sentences, and links to the characters, places, and artefacts it touches.
]

You create one from the shell. The lantern mystery is the spine of the book, so
it is a `major` thread, and it starts in `setup`:

#screen(caption: "inkhaven thread add — seeding the spine of the mystery")[```
$ inkhaven thread add "the-cold-lantern" \
      --status setup --weight major
added thread `the-cold-lantern` to Threads (setup · major)
  open Threads/the-cold-lantern in the editor to fill
  opening / midpoint / payoff
```]

That seeds a paragraph under the Threads book with a commented HJSON skeleton you
fill in the editor. Status runs from `setup` through `develop` to `payoff`, and
then to `resolved` or `abandoned`; weight is one of `major`, `subplot`, `runner`,
or `bridge`. For the cold-lantern thread you write its arc out in three sentences
and mark it `payoff`, because the reveal chapter is the one you are about to
draft:

#screen(caption: "Threads/the-cold-lantern — the arc, filled in")[```
{
  title:     "The cold lantern"
  status:    "payoff"
  weight:    "major"

  opening:   "The ninth lantern is found dark and its
              keeper gone — an accident, the town assumes."
  midpoint:  "Mira follows Aldous's trail out onto
              the Long Mole and into the fret."
  payoff:    "Aldous put the lantern out himself —
              and what the light was really holding back."

  characters: ["mira", "aldous", "sella"]
  places:     ["long-mole", "ninth-lantern"]
  tension:    9
}
```]

A thread on its own is just a note; it earns its keep by being *linked* to the
manuscript. As you draft the scenes that raise and develop the mystery, you
attach each to the thread with the ordinary paragraph-link chords — `Ctrl+V A`
adds an outgoing link, `Ctrl+V I` an incoming one — no new machinery. The links
are what let the thread reader see whether a promise is being kept in the actual
prose, or only in your intentions. Which is exactly what the doctor checks.

#subsection("The doctor — a promise not yet paid off (Ctrl+V Shift+D)")

`inkhaven thread doctor` (the chord is `Ctrl+V Shift+D`) reads every thread, tallies
how many manuscript paragraphs link to each, and reports the blind spots — the
promises the book has made but not yet visibly kept:

#screen(caption: "inkhaven thread doctor — the blind-spot report")[```
Thread doctor

  threads defined : 3
  avg tension     : 6.3

  status:
    payoff     1
    setup      1
    develop    1

  weight:
    major      1
    runner     1
    subplot    1

Blind spots
  PAYOFF UNFIRED — status `payoff` but no paragraph links:
    · the-cold-lantern
```]

There it is: *PAYOFF UNFIRED.* You marked the cold-lantern thread `payoff` — the
reveal is the whole point of the book — but not one manuscript paragraph links to
it yet, because you have not written the reveal chapter. The doctor is telling
you, in the plainest terms, that the book's central promise is *set up but not
yet delivered*. That is not a bug in your draft; at this stage it is a to-do list
of one, and precisely the one you most need to keep in view. When the reveal is
written and its paragraphs linked back to the thread, the flag clears itself.

The doctor names two other blind spots on longer books. *ZERO LINKS* catches a
thread whose status has moved past `setup` but which nothing in the manuscript
points at — a subplot you developed in your head but not on the page. *DORMANT*
catches a `develop` thread with only a link or two project-wide — a promise that
is technically alive but has gone quiet. Between them they answer the question a
long mystery lives or dies by: *have I paid off everything I set up?* And when
you want the whole ledger outside the terminal — for a co-writer, or a
submission packet — `inkhaven thread export --format markdown` (or `json`, or
`csv`) writes the inventory out.

#two_track(
  [In a novel the threads are plot promises — a cold lantern, a buried bargain, a
  Chekhov's gun on the mantel. *PAYOFF UNFIRED* is the reader's unmet expectation
  made mechanical: the doctor will not let the reveal you promised slip out of the
  finished book unpaid.],
  [In non-fiction the threads are *claims and undertakings* — "we will show that
  X," "this we return to in Chapter 9." Track each as a thread from where you
  promise it to where you discharge it, and the doctor's blind-spot report becomes
  a list of arguments you opened and never closed — the non-fiction equivalent of
  a dropped subplot.],
)

Between them, CHORUS and the Threads reader answer the two questions that turn a
pile of scenes into a book that holds: *do these people sound like themselves*,
and *does the book keep the promises it makes?* Neither reader touches your
prose. They hand you a sharper contrast to write and a promise to keep — and the
writing of both stays where it belongs, with you.

#recap((
  [*CHORUS* profiles the cast at book scale. `inkhaven chorus voices` builds the
  *distinctiveness matrix* and flagged that *Sella and Toft read alike*; sparse
  speakers like Aldous are profiled but never judged; *drift* is measured against a
  character's first chapter. The *Inner Stylist* (`Ctrl+B J → Y`) turns the numbers
  into Praise / Note / Concern.],
  [Declare a scene's viewpoint with a `pov:Mira` paragraph tag. `inkhaven chorus
  scan` then catches *head-hops* — "Toft wondered" in a Mira-POV scene — plus tense
  slips (English/German/French/Spanish; *Russian excluded by design*) and register
  drift.],
  [*DIALOGUE* reads speech as its own craft: `inkhaven dialogue scan` flags
  *said-bookisms* ("he ejaculated") and *zero-attribution*, and exits non-zero as a
  gate; `inkhaven dialogue profile` and `Ctrl+V Shift+Q` show per-character
  fingerprints.],
  [Track a plot promise as a *thread*: `inkhaven thread add "the-cold-lantern"
  --weight major`, fill its opening/midpoint/payoff under the Threads book, and
  link scenes to it with `Ctrl+V A` / `Ctrl+V I`. `inkhaven thread doctor`
  (`Ctrl+V Shift+D`) flagged the reveal as *PAYOFF UNFIRED* — set up but not yet
  paid off.],
  [Every reader here is *advisory*: it measures and reports, never edits. The fix —
  a sharper voice, a tagged viewpoint, a written reveal — always stays yours.],
))
