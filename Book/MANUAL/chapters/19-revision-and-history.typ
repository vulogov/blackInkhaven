#import "../design.typ": *

#chapter(number: 19, title: "Revision and History")

Every reader you have met so far in this part *diagnoses*. SENTINEL finds the
continuity break; LECTOR finds the saggy act and the point a reader would put
the book down; the Inner Editor finds the paragraph that tells where it should
show; CHORUS finds the two characters whose voices read alike. That is where
most writing tools stop — with a list of what is wrong. But a diagnosis is not
a revision. Between the finding and the fixed book lie two questions no
detector answers on its own: *what do I do about this one?* and, a draft later,
*did any of it actually help?*

This chapter is about the two intelligences that answer those questions.
*REDLINE* is the revision partner — it gathers every reader's findings into one
worklist and gives each the right kind of help, turning a diagnosis into an
author-confirmed change without ever touching your prose on its own. *CHRONICLE*
is the draft historian — it remembers what your book measured at each milestone
and trends it, so the question "did the revision work?" finally has a number
behind it. The two are a matched pair: REDLINE closes the loop, CHRONICLE
measures whether it stayed closed. Read this chapter as the operator's tour of
both; the companion book *Know Your Book* holds the fuller treatment of the
readers they draw on.

#section("REDLINE — the revision partner")

You have, by the time you reach a revision, a great deal of diagnosis scattered
across the tool. The `doctor` has its editorial classes. The Facts scan has
flagged a contradiction. Semantic drift has noticed a fact that changed shape.
The Planning Board's `plan check` has a structural note. The prose-style
detectors have underlined a bit of telling, a filter word, an anachronism.
SENTINEL has a co-location error, LECTOR a put-down risk, the Inner Editor a
craft observation, CHORUS a voice that has drifted. Acting on all of that by
hand means visiting a dozen surfaces and holding a dozen mental lists. REDLINE's
first job is to *end the scavenger hunt*.

#subsection("One worklist, every reader")

REDLINE gathers every reader's findings into a single ranked list — the same
unified `collect` the Editorial Pass and `inkhaven revise` both share. The
`doctor`'s editorial classes, the Facts contradictions, semantic drift, the
`plan` structure notes, the prose-style detectors, SENTINEL continuity, the
LECTOR read-through, the Inner Editor's craft notes, and CHORUS voice findings
all land in one queue, errors first, each carrying a note of *how it can be
acted on*. Nothing new is computed here — REDLINE reads what the readers have
already found — which is why the pass opens instantly and needs no live model to
show you the list.

#term("Worklist")[
  The single ranked list of *every* reader's findings, gathered by the shared
  `collect`. Each row is one finding — its severity, its category, where it
  lands in the book, and its *response kind*: the honest form of help for that
  particular problem. The Editorial Pass, `inkhaven revise`, and CHRONICLE's
  milestone capture all read this same list, so what you act on is exactly what
  gets measured.
]

#subsection("Three kinds of help")

The insight that makes REDLINE more than a to-do list is that different problems
want *different kinds* of help, and pretending otherwise is how automated
editors do damage. A repeated word wants a small, local rewrite. A fact
collision the machine cannot adjudicate wants *you* to decide which version is
true. A whole act that sags cannot honestly be fixed by rewriting one paragraph
at all — it wants advice, not a patch. So every finding carries a *response
kind*, shown as a glyph, and only one of the three ever touches prose.

#chord_table((
  chord_row("✎ Rewrite", "A diff-reviewed local prose fix — an honest single-paragraph change: de-echo, tighten pacing, show-don't-tell, cut a filter word, period-fit an anachronism, or carry out an Inner-Editor craft note."),
  chord_row("⇄ Decision", "A guided authorial choice. The AI cannot know which fact is right — which scene Mara is in, which value is canon. You state what is true; REDLINE reconciles the paragraph to your decision as a confirmed rewrite."),
  chord_row("✉ Brief", "A concrete revision brief, for a structural or book-level problem one paragraph cannot solve — a saggy act, a likely put-down point. The AI advises; it never rewrites. The brief lands in the Thoughts pane."),
))

The marquee case is the Inner Editor's own craft note. When it becomes a ✎
Rewrite, its observation — the specific thing it noticed about *this* paragraph
— is handed to the model as the instruction, so the fix addresses that note and
not some generic recipe. That is the difference between "make this better" and
"the tension slackens because the verb goes passive in the second clause; fix
*that*."

#subsection("The Editorial Pass — Ctrl+V Shift+R")

In the editor, `Ctrl+V Shift+R` opens the *Editorial Pass*: the worklist as a
cockpit you can walk. It sits under the view prefix — `Ctrl+V` reaches a tool
rather than acting on the book's structure — and the mnemonic is simply *R* for
*Revision*. Each row shows its response glyph, so you can see at a glance what
acting on it will do before you do it.

#screen(caption: "The Editorial Pass — one worklist, each row tagged how to act")[```
┌─ Editorial Pass · 14 findings (2 err · 5 warn · 7 info) ──────┐
│  ✗ ⇄ co_location  ch. 3   Mara is in the tower and the        │
│                           courtyard in the same breath        │
│  ⚠ ✎ echo         ch. 3   "about" repeats five times in two   │
│                           sentences — de-echo                 │
│  ⚠ ✎ editor       ch. 7   the verb tense wobbles mid-paragraph│
│  · ✉ shape_sag    ch. 5   the Three-Act shape wants a rise    │
│    ▌                      here; the prose reads flat          │
│  · ✎ filter       ch. 2   filter word: "very" — consider      │
│                           cutting                             │
├───────────────────────────────────────────────────────────────┤
│ ↑↓ move · [ ] filter · ⏎ jump · f act · F fix-all ·           │
│ s skip · d defer · D clear · Esc close                        │
└───────────────────────────────────────────────────────────────┘
```]

Moving through it is quick. `↑ ↓` move the cursor, and the selected finding's
full message and any hint expand in place below it. `[` and `]` cycle a category
filter, so you can walk all the echoes, then all the continuity errors, one
class at a time. `⏎` jumps the editor straight to the paragraph so you can read
the finding in its context. The three keys that follow are where the work
happens.

#chord_table((
  chord_row("f", "Act on the selection. ✎ streams an AI rewrite into a diff review; ⇄ asks what is true, then reconciles the paragraph to your answer; ✉ writes a developmental brief to the Thoughts pane."),
  chord_row("F", "Fix-all — walk every ✎ Rewrite in turn, each one diff-reviewed. Decisions, Briefs, and finding-aware editor notes are never batched; they are handled one at a time."),
  chord_row("s / d", "Skip for this session (s), or defer (d) — persisted, hidden until the prose changes so a finding you have judged and set aside stays gone."),
  chord_row("D / Esc", "Clear every deferral (D), or close the pass (Esc)."),
))

The distinction between `f` and `F` is worth internalising. A single `f`
acts on one finding of any kind. The `F` batch is deliberately narrower: it
walks *only* the ✎ Rewrites, one diff at a time, and `Esc` stops it wherever you
are. Anything that requires your judgement — a Decision you must answer, a Brief
that only advises, an editor note whose rewrite is finding-specific — is left for
you to take one at a time. The batch is a convenience for the mechanical fixes,
never an autopilot for the ones that need a human.

#subsection("The safety contract")

REDLINE will not edit your prose on its own. This is not a policy bolted on top;
it is the shape of the code. Every prose change REDLINE can make — a single ✎
fix, a ⇄ decision-reconcile, an editor note, or one step of the `F` batch — flows
through exactly one path, and there is no other.

#screen(caption: "Every REDLINE prose change takes the same three steps")[```
   the model's rewrite streams into the AI pane
                    │
                    ▼
   an AI diff-review modal shows the exact change
        accept (a / e / ⏎)   or   reject (r)
                    │  accept
                    ▼
   your pre-rewrite prose is SNAPSHOTTED first,
   with a labelled entry — then it is replaced
                    │
                    ▼
        recover it any time with  F6
```]

Read the three steps as promises. First, you always see the exact diff — the
change is shown, never applied blind. Second, nothing is written until you
accept it; reject and the paragraph is untouched. Third, on accept your old
prose is snapshotted with a labelled entry *before* the new text lands, so the
version you had is one `F6` away for as long as the project lives. There is no
code path in REDLINE that writes prose without all three.

#callout(label: "The batch is Rewrite-only by construction")[
  The `F` queue can hold nothing but ✎ Rewrites. A Decision or a Brief cannot
  acquire a fix recipe in the first place, so it can never slip into the
  prose-writing path — the type of the finding forbids it, and a guard test
  locks the invariant in place. When you press `F` you are never one wrong
  keystroke away from an unconfirmed edit; the queue *cannot* contain one.
]

#subsection("The editorial letter — inkhaven revise")

The Editorial Pass is the row-by-row cockpit. Sometimes, though, you want the
opposite view first — the *overview* a good editor opens a revision with, before
touching a single line. That is `inkhaven revise`. It runs the same worklist,
then synthesises the whole of it into one developmental letter: the big picture
first — the one or two things a reader will feel first — then grouped by theme
(continuity, structure and pacing, voice and character, line and prose), most
important first, each with a brief *why it matters* and *what to do*. It advises;
it never rewrites.

#screen(caption: "inkhaven revise — the letter, or the findings for tooling")[```
inkhaven revise                  # the editorial letter over the
                                 # whole project
inkhaven revise --book-name X    # restrict to one book (slug or
                                 # title)
inkhaven revise --json           # the findings as JSON: category,
                                 # severity (high/med/low),
                                 # response, location, message
```]

The letter is written in the manuscript's language — REDLINE inherits every
source reader's language coverage and claims no more than they do — and each
finding keeps its `source`, so the perennial question *"does it work in
Russian?"* answers itself per detector rather than as one blanket promise. The
`--json` form is the machine face of the same worklist: the AI letter and every
prose rewrite stay out of it, but the ranked findings come through in full, ready
for a script or a hook.

#subsection("REDLINE from a script")

Two Bund words expose the worklist read-only, for a revision-readiness check you
can wire into a hook. The editorial letter and every rewrite are deliberately
*not* scriptable — advice and prose changes stay author-initiated — but the
deterministic findings are fair game.

#screen(caption: "The REDLINE Bund surface — read-only")[```
ink.revise.findings  ( -- list )
    the ranked findings as dicts: { category, severity,
    response, location, message, source }

ink.revise.check     ( -- dict )
    summary counts: { findings, high, med, low, clean,
    by_response, by_category }
```]

`check.clean` is a plain pass/fail gate — `true` when there are no high-severity
findings — the thing a "is this draft ready?" script asks. The `high` / `med` /
`low` vocabulary matches `revise --json`, so a report and a gate speak the same
words.

#section("CHRONICLE — the draft history")

REDLINE helps you act. But once you have acted — cleared a dozen findings, moved
a scene, rewritten a flat chapter — a harder question arrives, and until
CHRONICLE nothing in the tool could answer it: *did the book actually get
better?* Every reader diagnoses the draft *in front of it*. None of them
remembered what the last draft measured. So the single question a reviser most
wants answered had no answer at all. CHRONICLE is the memory that closes that
gap — and it is *pure measurement*: there is no prose-write path anywhere in it.

#subsection("A milestone — chronicle mark")

A *milestone* is an explicit capture of the current draft — the whole diagnostic
state frozen in one shot. You stamp one whenever a draft is worth remembering:
after a big pass, before you send it to a reader, at the turn of a version.

#screen(caption: "Stamping and listing milestones")[```
inkhaven chronicle mark "draft-2"
inkhaven chronicle mark "beta-1" --ref v0.9   # record a git ref
inkhaven chronicle list                        # newest first
inkhaven chronicle list --json                 # for tooling
```]

Marking runs the unified worklist once — the same `collect` REDLINE uses — and
captures the total finding count, the tallies by severity, category,
response-kind and source, and, crucially, the *fingerprint of every finding*, so
that a later diff can name exactly which ones cleared and which appeared. The
`--ref` string is stored verbatim for your own bookkeeping; CHRONICLE never
resolves, creates, or enumerates git refs — a milestone is a decision you make,
not a tag it invents.

#term("Milestone")[
  One named, timestamped capture of the draft's whole diagnostic state: the
  metric vector (counts by severity, category, response, and source) plus the
  fingerprint of every finding. Milestones are deliberate — you stamp them with
  `chronicle mark` — and they are the fixed points every trend and diff measures
  between. They live in the project's own `chronicle.db`, separate from your
  prose and your snapshots.
]

#subsection("The trend — every count fewer is better")

Run `inkhaven chronicle` bare and CHRONICLE captures the *live* state of the book
right now and diffs it against your most recent milestone — the "did it get
better since I last looked?" view. Every number it trends is a count of findings
the readers raised, which means every one is *fewer-is-better*: a fall is an
improvement, a rise a regression, and the regressions sort to the top of the
category list so the worst news never hides.

#screen(caption: "inkhaven chronicle — the trend since your last mark")[```
Chronicle — since "draft-1" (2026-08-03) → now

  findings           1 →   2   ▲
  warnings           0 →   1   ▲  NEW
  infos              1 →   1   ·

  by category:
    put_down_risk      0 →   1   ▲  NEW
    shape_sag          0 →   1   ▲  NEW
    attention_dip      1 →   0   ▼  cleared

  ✓ 1 cleared    ▲ 2 introduced    · 0 unchanged

  introduced (new since the last mark):
    ⚠ put_down_risk  ch. 3   ch. 1-3 run flat and eventless…
    · shape_sag      ch. 2   the shape wants rising tension…
```]

The arrows carry the whole polarity at a glance: `▼` a count that fell (good),
`▲` one that rose (a regression), `·` one that held. A category that went from
some findings to none is tagged `cleared`; one that went from none to some is
tagged `NEW`. You are never left to work out whether a number moving is welcome
— the direction is scored for you, and the layout puts the regressions where you
will see them first.

#subsection("Cleared versus introduced — the signature")

The move that makes CHRONICLE worth having is the *cleared / introduced* split.
Because every finding carries a stable fingerprint, CHRONICLE can do a simple set
difference between two milestones' finding sets and sort the result into three
piles.

#chord_table((
  chord_row("✓ cleared", "Findings that were there last milestone and are gone now — the ones your revision actually resolved. The proof that the work landed."),
  chord_row("▲ introduced", "Findings new since the last milestone — the ones your edits, or the ripple around them, created. The early warning on collateral damage, before it ships."),
  chord_row("· unchanged", "Findings present in both — still standing, still waiting for you."),
))

This is what closes REDLINE's loop. When you spend an afternoon in the Editorial
Pass clearing echoes and reconciling a continuity error, the *cleared* list is
the receipt: those exact findings are gone. And the *introduced* list is the
thing no amount of careful rewriting can see from inside a single paragraph — the
new confusion your cut created three chapters downstream, the sag that opened up
when you tightened the scene before it. The introduced findings are itemised, not
just counted, so you can act on them rather than merely worry about them.

#subsection("Diffing two named milestones — chronicle diff")

The bare trend measures *now* against the last mark. To compare two fixed points
in your history — beta-1 against the release draft, say — name them both.

#screen(caption: "chronicle diff — two milestones head to head")[```
inkhaven chronicle diff draft-1 draft-3
inkhaven chronicle diff beta-1 rc-1 --json   # deltas + the three
                                             # finding lists
```]

`chronicle diff <from> <to>` runs the same trend machinery between two stored
milestones rather than against the live book, and `--json` on either the trend or
a diff emits the deltas plus the cleared, introduced, and persisted lists for a
script to read.

#subsection("The dashboard — Ctrl+B Shift+U")

Inside the editor, `Ctrl+B Shift+U` opens the CHRONICLE dashboard — the trend and
the cleared/introduced split, live, without leaving the window. It sits under the
meta prefix because it is a whole-book intelligence, and it reads the same
numbers the CLI prints.

#screen(caption: "The CHRONICLE dashboard — Ctrl+B Shift+U")[```
┌─ Chronicle ───────────────────────────────────────────────────┐
│ ◆ Chronicle — since "draft-1"                                  │
│                                                                │
│   findings           1 →   2   ▲                              │
│   warnings           0 →   1   ▲  NEW                          │
│   infos              1 →   1   ·                               │
│                                                                │
│ by category                                                    │
│   put_down_risk      0 →   1   ▲  NEW                          │
│   attention_dip      1 →   0   ▼  cleared                      │
│                                                                │
│ ✓ 1 cleared   ▲ 2 introduced   · 0 unchanged                  │
│                                                                │
│ introduced                                                     │
│   ⚠ put_down_risk  ch. 3   ch. 1-3 run flat and eventless…    │
│   · shape_sag      ch. 2   the shape wants rising tension…    │
├────────────────────────────────────────────────────────────────┤
│ ↑↓ scroll · Enter jump to an introduced finding · m mark · Esc │
└────────────────────────────────────────────────────────────────┘
```]

`↑ ↓` scroll the panel. `⏎` on an introduced finding jumps the editor straight
to its paragraph — the same jump the read-through and continuity ledgers give
you, so a regression is never more than one keystroke from the prose that
carries it. `m` marks the current draft on the spot, labelled by today's date
(rename it later from the CLI if you want a real name), and `Esc` closes.
Marking from the dashboard is the one place the tool writes a milestone
interactively; everything else the dashboard does is read-only.

#chord_table((
  chord_row("↑ ↓", "Scroll the dashboard."),
  chord_row("⏎", "Jump to an introduced finding's paragraph."),
  chord_row("m", "Mark the current draft now — labelled by today's date, renamed via the CLI."),
  chord_row("Esc", "Close the dashboard."),
))

#subsection("CHRONICLE from a script")

Three Bund words expose the history, all read-only. Marking is *not* scriptable —
stamping a milestone is a deliberate act, the same shape as taking a snapshot —
so scripts read the history, they do not write it.

#screen(caption: "The CHRONICLE Bund surface — read-only")[```
ink.chronicle.marks  ( -- list )
    { label, ts, book, findings, errors, warnings, infos }

ink.chronicle.trend  ( -- dict )
    { marked, since, headline, categories, cleared,
      introduced, persisted }

ink.chronicle.check  ( -- dict )
    { baseline, cleared, introduced, introduced_errors,
      clean }
```]

`check.clean` is `true` when your latest edits introduced *no* error-severity
finding since the last mark — a pre-commit or pre-submit gate that says "you did
not break anything new." With no milestone yet, `check` is vacuously clean and
`trend` reports `{marked: false}`, so a script that runs before you have ever
marked a draft degrades gracefully rather than erroring.

#callout(label: "Two databases, two jobs")[
  CHRONICLE keeps its milestones in the project's own `chronicle.db`, beside
  your prose but never mixed into it. It is deliberately not a git tool and not
  an auto-capture daemon: it will not create tags, and it will not stamp a
  milestone behind your back. A draft is a decision, so a milestone is a
  keystroke — `chronicle mark` on the CLI, or `m` in the dashboard.
]

#section("Closing the loop")

The two intelligences in this chapter are one workflow read from two ends. Set
them side by side and the shape is plain.

REDLINE *acts*. It gathers every reader's diagnosis into one worklist, hands each
finding the honest kind of help — a diff-reviewed rewrite, a decision it asks you
to make, or a brief it can only advise — and moves every prose change through the
one safe path: shown as a diff, applied only on your accept, snapshotted before
it lands. When you finish a pass in the Editorial Pass, you have turned a pile of
findings into a set of confirmed changes, and not one word changed without your
say-so.

CHRONICLE *measures*. Mark a milestone before the pass and another after, and the
cleared list is the receipt for the findings REDLINE helped you resolve, while
the introduced list is the warning about the ones your edits created three
chapters away. The counts trend fewer-is-better; the split names names. Neither
touches your prose — CHRONICLE only ever reads.

That is the loop. REDLINE closes findings; CHRONICLE proves they stayed closed
and catches what opened up in their place. Do the work in the pass, stamp a
milestone, and the next time you open the dashboard the book tells you, in its
own numbers, whether the revision was a revision — or merely a rearrangement.

#recap((
  [*REDLINE* gathers *every reader's findings* into one ranked worklist — the
  shared `collect` behind the Editorial Pass, `inkhaven revise`, and CHRONICLE's
  capture — errors first, each tagged with how it can be acted on.],
  [Each finding carries a *response kind*: *✎ Rewrite* (a diff-reviewed local
  fix), *⇄ Decision* (you state what is true, REDLINE reconciles), or *✉ Brief*
  (advice for a structural problem, never a rewrite). Only a Rewrite touches
  prose.],
  [The Editorial Pass is `Ctrl+V Shift+R`: `f` acts on one finding, `F` walks
  *only* the ✎ Rewrites one diff at a time, `s`/`d` skip or defer. Every prose
  change is a *confirmed diff plus a pre-change snapshot*, `F6`-restorable — there
  is no unconfirmed write, and the `F` batch is Rewrite-only by construction.],
  [`inkhaven revise` synthesises the same worklist into one *editorial letter*
  (big picture, then grouped by theme); `--json` and `ink.revise.*` expose the
  findings read-only. It advises; it never rewrites.],
  [*CHRONICLE* snapshots every reader's verdict per milestone. `chronicle mark
  <label>` stamps one; bare `inkhaven chronicle` trends the live book against the
  last mark, every count *fewer-is-better* (▼ good, ▲ a regression); `chronicle
  diff` compares two named marks.],
  [The signature is the *cleared vs introduced* split — set difference over stable
  fingerprints: what your revision *resolved* and what it *created*. This is how
  CHRONICLE closes the loop REDLINE opened.],
  [`Ctrl+B Shift+U` opens the dashboard (`⏎` jumps to an introduced finding, `m`
  marks the draft); `ink.chronicle.*` and its own `chronicle.db` keep the history.
  CHRONICLE is *pure measurement* — no prose-write path anywhere.],
))
