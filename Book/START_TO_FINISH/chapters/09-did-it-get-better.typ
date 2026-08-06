#import "../design.typ": *

#chapter(number: 9, title: "Did It Get Better?")

In the last chapter you spent an afternoon in the Editorial Pass. You reconciled
the leaked secret — moved Sella's give-away line so she no longer knows a chapter
early. You took the read-through's brief on the sag in the middle and tightened
the scenes where the mystery goes slack. You cleared a handful of echoes and a
filter word or two. It *feels* better. But feeling is not knowing, and every
reviser has had the afternoon that felt productive and left the book no better —
a dozen findings closed, a dozen new ones opened three chapters downstream, the
net gain a rounding error. So the honest question, the one no reader you have met
so far can answer, is the plainest one there is: *did the book actually get
better, or did you just move the problems around?*

This chapter is about the one intelligence built to answer it — *CHRONICLE*, the
draft historian (the manual's Chapter 19 gives its full tour). It does exactly
one thing, and it does nothing else: it *measures*. There is no prose-write path
anywhere in it. It cannot rewrite a line, and it will not try. It remembers what
your book measured at a milestone, compares that to the book in front of it now,
and tells you — in the book's own numbers — whether the revision was a revision.

#section("The milestone you already had")

CHRONICLE can only answer the question because, one keystroke before the
revision, you gave it something to measure against. Before the first `✎` Rewrite
of the last chapter — while the draft still held every fault the readers had
found — you stamped a *milestone*:

#screen(caption: "inkhaven chronicle mark — freezing the draft's verdict")[```
$ inkhaven chronicle mark "first-draft"
✓ marked "first-draft" — 12 finding(s) (1 error · 4 warn · 7 info)
```]

That one command runs the same unified worklist REDLINE walks — the shared
`collect` behind the whole of Part IV — and freezes the result: the total count,
the tallies by severity and category, and, the part that makes everything later
possible, the *fingerprint of every single finding*. It is a photograph of the
draft's diagnostic state, taken at the moment you decided this draft was worth
remembering.

#term("A milestone")[
  One named, timestamped capture of the draft's whole diagnostic state — the
  counts by severity and category *plus* the fingerprint of every finding.
  Milestones are deliberate: you stamp one with `chronicle mark <label>` before a
  big pass, before a reader sees it, at the turn of a version. They are the fixed
  points every trend and diff measures between, and they live in the project's
  own `chronicle.db`, beside your prose but never mixed into it.
]

Milestones are never taken behind your back. CHRONICLE is not a daemon and not a
git tool — it will not stamp a draft because you saved, and it will not invent a
tag. A draft is a decision, so a milestone is a keystroke. You can list the ones
you have, newest first:

#screen(caption: "inkhaven chronicle list — the marks you have stamped")[```
$ inkhaven chronicle list
2026-08-04  first-draft        12 finding(s)  1✗ 4⚠ 7·
```]

One mark so far. That single row is the baseline the rest of this chapter
measures against — and the whole reason the afternoon of revision can now be
graded rather than merely felt.

#section("The trend — every count fewer is better")

Now do the revision — you already did, in the last chapter — and ask the
question. Run `chronicle` with no arguments. Bare, it captures the *live* state
of the book right now and diffs it against your most recent mark: the "did it get
better since I last looked?" view.

#screen(caption: "inkhaven chronicle — the live book against your last mark")[```
$ inkhaven chronicle
Chronicle — since "first-draft" (2026-08-04) → now

  findings          12 →  11   ▼
  errors             1 →   0   ▼  cleared
  warnings           4 →   5   ▲
  infos              7 →   6   ▼

  by category:
    echo               0 →   1   ▲  NEW
    leaked_secret      1 →   0   ▼  cleared
    shape_sag          1 →   0   ▼  cleared

  ✓ 2 cleared    ▲ 1 introduced    · 10 unchanged

  introduced (new since the last mark):
    ⚠ echo         ch. 4      "fret" repeats four times — de-echo
```]

Read the top block first, because its polarity is the whole point. Every number
CHRONICLE trends is a count of findings the readers *raised* — which means every
one is *fewer-is-better*. A falling count is an improvement, a rising count a
regression, and CHRONICLE scores the direction for you so you never have to work
out whether a number moving is welcome. The arrow carries it at a glance: `▼` a
count that fell (good), `▲` one that rose (a regression), `·` one that held. A
category that dropped to nothing is tagged `cleared`; one that appeared from
nothing is tagged `NEW`. The one error you had — the leaked secret — is gone, so
`errors` reads `1 → 0 ▼ cleared`. That is the headline you came for.

But look at `warnings`: `4 → 5 ▲`. It went the wrong way. You *added* a warning
somewhere while you were fixing everything else, and CHRONICLE will not let that
hide behind the good news above it. The regressions sort to the top of the
category list precisely so the worst thing you did is the first thing you read —
here, `echo 0 → 1 ▲ NEW`, sitting above the two categories you cleared.

#callout(label: "Direction is scored, not left to you")[
  This is the difference between a dashboard and a diff. A pile of raw counts
  would leave you squinting at whether `4 → 5` is progress. CHRONICLE knows that
  every finding is a fault, so it knows `4 → 5` is a regression and says so, in
  the arrow and in the ordering. You are never asked to remember which direction
  is good; the tool remembers for you, and puts the bad news on top.
]

#section("Cleared versus introduced — the signature")

The counts tell you *how much* moved. The line below them tells you *exactly
what* — and it is the move that makes CHRONICLE worth having.

#screen(caption: "The signature line — a set difference over fingerprints")[```
  ✓ 2 cleared    ▲ 1 introduced    · 10 unchanged
```]

Because every finding froze with a stable fingerprint at the mark, CHRONICLE can
do a plain set difference between the old finding set and the live one, and sort
the result into three piles. Nothing here is estimated; it is arithmetic on
identities.

#chord_table((
  chord_row("✓ cleared", "There at the mark, gone now — the findings your revision actually resolved. The receipt that the work landed. Here: the leaked_secret and the shape_sag."),
  chord_row("▲ introduced", "New since the mark — the findings your edits, or the ripple around them, created. The early warning on collateral damage. Here: one echo, in ch. 4."),
  chord_row("· unchanged", "Present in both — still standing, still waiting for you. Here the ten you never touched, including Bryn's dropped_reveal from the KEN chapter."),
))

This is what closes the loop the Editorial Pass opened. The *cleared* list is the
proof of the work you meant to do: the leaked secret you reconciled and the sag
you tightened are named, by category, gone. You did not imagine the improvement —
here are its two receipts.

The *introduced* list is the thing no amount of careful rewriting can see from
inside a single paragraph. When you tightened the slack scene in Chapter 4 to
lift the sag, one of those hurried rewrites reached for the word `fret` four
times in two sentences — a fresh echo, in the very paragraph you were fixing. You
could not have caught it by re-reading the fix, because it reads fine in
isolation; the sentence you wrote to solve one problem quietly made another. The
introduced list is *itemised*, not merely counted, so it is a to-do and not a
worry: severity, category, the chapter, and the head of the finding, ready to act
on.

#callout(label: "The honest reading of this trend")[
  You cleared an error and lifted a structural sag, and you introduced one
  warning-level echo doing it. That is a good afternoon — a real net improvement
  with one small piece of collateral to sweep up. Without CHRONICLE you would have
  shipped the echo, because it lives three chapters from the win and looks
  innocent up close. *That* is the value: not the celebration of what cleared, but
  the catch on what crept in.
]

#section("From the dashboard — Ctrl+B Shift+U")

You do not have to leave the editor to see any of this. `Ctrl+B Shift+U` opens
the CHRONICLE dashboard — the same trend and the same cleared/introduced split,
live, in a panel. It sits under the meta prefix (`Ctrl+B`) because it is a
whole-book intelligence, and it reads the very numbers the CLI prints.

#screen(caption: "The CHRONICLE dashboard — Ctrl+B Shift+U")[```
┌─ Chronicle ───────────────────────────────────────────────────┐
│ ◆ Chronicle — since "first-draft"                              │
│                                                                │
│   findings          12 →  11   ▼                               │
│   errors             1 →   0   ▼  cleared                      │
│   warnings           4 →   5   ▲                               │
│   infos              7 →   6   ▼                               │
│                                                                │
│ by category                                                    │
│   echo               0 →   1   ▲  NEW                          │
│   leaked_secret      1 →   0   ▼  cleared                      │
│   shape_sag          1 →   0   ▼  cleared                      │
│                                                                │
│ ✓ 2 cleared   ▲ 1 introduced   · 10 unchanged                 │
│                                                                │
│ introduced                                                     │
│   ⚠ echo         ch. 4   "fret" repeats four times in two…    │
├────────────────────────────────────────────────────────────────┤
│ ↑↓ scroll · Enter jump to an introduced finding · m mark · Esc │
└────────────────────────────────────────────────────────────────┘
```]

The dashboard is where measurement turns back into work. Put the cursor on the
introduced echo and press `⏎`: the editor jumps straight to the Chapter 4
paragraph carrying it — the same jump the continuity ledger and the read-through
give you, so a regression is never more than a keystroke from the prose that
holds it. Cut two of the four `fret`s, and the echo is gone. Now the book is
genuinely, measurably better than the mark — and you say so with one more
keystroke: `m` stamps a fresh milestone on the spot, labelled by today's date.

#chord_table((
  chord_row("↑ ↓", "Scroll the dashboard."),
  chord_row("⏎", "Jump to an introduced finding's paragraph — go fix the collateral where it lives."),
  chord_row("m", "Mark the current draft now, labelled by today's date. Rename it later from the CLI."),
  chord_row("Esc", "Close the dashboard."),
))

Marking from the dashboard is the *one* place CHRONICLE writes anything
interactively, and even then it writes only a milestone — a row of numbers in
`chronicle.db`, never a word of your prose. Everything else the panel does is
read-only. Rename the date-stamped mark from the shell when you want a real name
for it:

#screen(caption: "inkhaven chronicle mark — naming the revised draft")[```
$ inkhaven chronicle mark "revised-1"
✓ marked "revised-1" — 10 finding(s) (0 error · 4 warn · 6 info)
```]

Ten findings now, no errors, and the echo you introduced already swept back up.
Two marks in the history, and the afternoon has a settled record.

#section("Two fixed points — chronicle diff")

The bare trend measures *now* against the last mark — it moves as you edit. To
compare two *fixed* points in your history, name them both. `chronicle diff
<from> <to>` runs the same machinery between two stored milestones rather than
against the live book:

#screen(caption: "inkhaven chronicle diff — first-draft against revised-1")[```
$ inkhaven chronicle diff first-draft revised-1
Chronicle — "first-draft" → "revised-1"

  findings          12 →  10   ▼
  errors             1 →   0   ▼  cleared
  warnings           4 →   4   ·
  infos              7 →   6   ▼

  by category:
    leaked_secret      1 →   0   ▼  cleared
    shape_sag          1 →   0   ▼  cleared

  ✓ 2 cleared    ▲ 0 introduced    · 10 unchanged
```]

This is the same revision, told as a settled fact. Notice what changed between
the live trend and this diff: `▲ 1 introduced` became `▲ 0 introduced`, and the
echo has vanished from the category list entirely. It was born and buried
*between* the two marks — introduced by the sag fix, cleared before you stamped
`revised-1` — so a diff of the two endpoints never sees it. The live trend caught
the echo the instant it existed; the milestone-to-milestone diff is the clean
record after you dealt with it. Both are true; they answer different questions.
*"What is wrong right now?"* is the trend. *"What did this revision, on balance,
do?"* is the diff — two cleared, nothing introduced, the sag and the leaked
secret gone for good.

#callout(label: "Pure measurement, kept apart from the prose")[
  Everything in this chapter reads and reports; none of it writes a word of the
  book. CHRONICLE keeps its milestones in the project's own `chronicle.db`,
  separate from your manuscript and separate from your `F6` snapshots — two
  databases, two jobs. It measures whether the revision worked; it never performs
  the revision. That division is the whole reason you can trust the number: the
  thing grading the draft has no stake in the grade.
]

#two_track(
  [You marked `first-draft` while the mystery still gave itself away and sagged in
  the middle. The diff proves the reveal now lands where it should and the slack
  scene tightened — not by your say-so, but by the readers' own count falling. And
  it caught the echo your fix bred, so the win did not quietly cost you a fault
  elsewhere.],
  [The same instrument grades an argument. Mark the draft before you restructure a
  chapter; revise; run `chronicle`. The *cleared* list is the objections you
  answered and the loose claims you closed; the *introduced* list is the new
  ambiguity your cut opened two sections down. Proof that you *tightened* the
  argument rather than merely *churned* it — a number, not a feeling.],
)

The book is better, and now you can say so without flinching. Not because it
feels better — because the readers raised eleven findings, then ten, and the two
you set out to fix are named in the cleared pile while the one you bred was caught
in the introduced pile before it could ship. That is what CHRONICLE is for: it
turns *"I think the revision helped"* into *"here is the ledger."* With the draft
graded and the collateral swept up, `The Ninth Lantern` is ready to leave the
workshop — which is where the last part of this book takes it.

#recap((
  [*CHRONICLE* answers the one question no diagnosing reader can: *did the revision
  work?* It is *pure measurement* — no prose-write path anywhere — kept in the
  project's own `chronicle.db`, apart from your manuscript and your snapshots.],
  [A *milestone* freezes the draft's whole verdict: `chronicle mark "first-draft"`
  runs the shared `collect` once and captures the counts *plus every finding's
  fingerprint*. Milestones are deliberate — a keystroke, never a daemon; `chronicle
  list` shows them newest first.],
  [Bare `inkhaven chronicle` trends the *live* book against your last mark. Every
  count is *fewer-is-better* — `▼` good, `▲` a regression, `·` held — and the
  regressions sort to the top, so `warnings 4 → 5 ▲` never hides behind the wins.],
  [The signature is the *cleared vs introduced* split — a set difference over stable
  fingerprints. *Cleared* named the leaked_secret and the shape_sag you resolved
  (the receipt); *introduced* named the echo a hasty sag-fix bred in ch. 4 (the
  collateral, itemised and jumpable).],
  [`Ctrl+B Shift+U` opens the dashboard: `⏎` jumps to an introduced finding to go
  fix it, `m` marks the draft (date-labelled, renamed via the CLI). It is the one
  place CHRONICLE writes — and it writes only a milestone, never prose.],
  [`chronicle diff <from> <to>` compares two *fixed* marks: the settled record. The
  echo born-and-buried between the marks never appears in it — the live trend
  catches regressions as they happen, the diff reports the revision on balance.],
))
