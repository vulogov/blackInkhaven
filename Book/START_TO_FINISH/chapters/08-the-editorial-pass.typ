#import "../design.typ": *

#chapter(number: 8, title: "The Editorial Pass")

Every reader in this part of the book has now had its say about *The Ninth
Lantern*, and each said its piece on its own surface. KEN, in Chapter 5, caught
Sella naming the reveal a chapter before she could know it. CHORUS, in Chapter
6, found that Sella and Toft read alike. LECTOR, in Chapter 7, felt the middle
sag and marked a place a first reader might lose the thread. And back in Chapter
4 the fact-checker noticed the lanterns burning *three hundred years* in one
place and *two hundred* in another. Four true findings — and four different
screens, four different commands, four different mental notes to hold while you
sit down to actually fix the book.

That scattering is the problem this chapter solves. A diagnosis is not a
revision, and a dozen diagnoses spread across a dozen surfaces is not a to-do
list — it is a scavenger hunt. *REDLINE* ends the hunt. It gathers every reader's
findings into one ranked worklist and, for each one, hands you the honest kind of
help — a rewrite you confirm, a decision it asks you to make, or a brief it can
only advise. Nothing new is computed; REDLINE reads what the readers already
found. And — the promise the whole chapter turns on — it never changes a word of
your prose without showing you the exact diff and taking a snapshot first. (The
manual's Chapter 19 is REDLINE's full reference; this is the pass run on our own
book.)

#section("One worklist, every reader")

In the editor, `Ctrl+V Shift+R` opens the *Editorial Pass* — the worklist as a
cockpit you can walk. It sits under the view prefix (`Ctrl+V` reaches a tool
rather than reshaping the book), and the mnemonic is simply *R* for *Revision*.
The pass opens instantly, because it is reading findings the readers have already
produced, not running them again. Here is *The Ninth Lantern*'s, the morning
after the read-through:

#screen(caption: "Ctrl+V Shift+R — one worklist, each row tagged how to act")[```
┌─ Editorial Pass · 6 findings (2 err · 2 warn · 2 info) ───────┐
│  ✗ ⇄ leaked_secret  ch. 5 Sella names the reveal before ch. 6 │
│  ✗ ⇄ fact           ch. 4 "200 years" but ch. 1 says 300      │
│  ⚠ ✎ editor         ch. 4 "Mira felt afraid" — line is inert  │
│  ⚠ ✉ distinctiveness  —   Sella and Toft read alike           │
│  · ✉ shape_sag      ch. 3 the middle sags — wants a rise      │
│  · ⇄ confusion      ch. 3 a reader may lose who suspects whom │
├───────────────────────────────────────────────────────────────┤
│ ↑↓ move · [ ] filter · ⏎ jump · f act · F fix-all ·           │
│ s skip · d defer · D clear · Esc close                         │
└───────────────────────────────────────────────────────────────┘
```]

Six findings from five different readers, errors first, each carrying one thing
no bare to-do list has: a glyph in the second column that tells you *what acting
on it will do* before you do it. That glyph is the whole idea behind REDLINE.

#term("Response kind")[
  Every finding carries a *response kind* — the honest form of help for that
  particular problem, shown as a glyph. *✎ Rewrite* is a diff-reviewed local prose
  fix; *⇄ Decision* is a choice only you can make, after which REDLINE reconciles
  the prose to your answer; *✉ Brief* is written advice for a structural problem no
  single paragraph can solve. Only one of the three — Rewrite — ever touches your
  words, and even then only through a diff you accept.
]

The kind is derived from what sort of problem the finding *is*, not guessed. A
repeated word or a flat line has an honest single-paragraph fix, so it is a
Rewrite. A fact stated two ways, or a secret spoken too early, cannot be
adjudicated by a machine — it needs *you* to say which way is true — so it is a
Decision. A whole act that sags cannot be honestly patched by rewriting one
paragraph at all, so it is a Brief. Read down our worklist and the glyphs sort
the morning's work for you: two Decisions to rule on, one line to rewrite, one
voice-note and one structural sag to take as advice, and one more Decision about
the thread the reader might lose.

Moving through it is quick. `↑ ↓` move the cursor and expand the selected
finding's full message in place; `[` and `]` cycle a category filter, so you can
walk all the Decisions, then all the Rewrites, one class at a time; `⏎` jumps the
editor straight to the paragraph so you can read the finding in its context. The
work itself happens on three keys — `f` acts on the selected finding, `F` walks
*only* the ✎ Rewrites one diff at a time, and `s` / `d` skip for the session or
defer until the prose changes. Let us walk ours.

#section("Walking the list")

#subsection("✎ Accepting a rewrite — the flat line")

Start with the cheapest win: the `editor` finding at Chapter 4. When Mira first
steps onto the Long Mole you wrote *"Mira felt afraid as she stepped onto the
Mole"* — and the Inner Editor, reading that paragraph, noticed the line goes
inert: it *names* the feeling instead of letting the reader feel it, and the fret
that should be closing round her never touches her at all. Press `f`, and this is
the marquee case — the Inner Editor's own observation is handed to the model *as
the instruction*, so the fix answers that note and not some generic "make it
better" recipe.

#screen(caption: "f on a ✎ Rewrite — the diff you accept, note-driven")[```
┌─ AI diff · ✎ editor · ch. 4 ─────────────────────────────────┐
│ the note handed to the model:                                │
│   "names the feeling instead of showing it — the verb is a   │
│    bare copula and the fret never touches her"               │
├──────────────────────────────────────────────────────────────┤
│ - Mira felt afraid as she stepped onto the Mole.             │
│ + The Mole narrowed to a grey thread ahead, and the cold     │
│ + fret closed over Mira's hands as she stepped out onto it.  │
├──────────────────────────────────────────────────────────────┤
│ a / e / ⏎  accept    r  reject    (snapshot on accept · F6)  │
└──────────────────────────────────────────────────────────────┘
```]

Nothing has changed yet. The model's rewrite has streamed into the AI pane and a
diff-review modal is showing you the exact before-and-after — no more, no less.
Now you choose. `a` accepts it as written; `e` lets you edit the suggestion first
and then accept; `⏎` accepts; `r` rejects and the paragraph stays exactly as it
was. Reject costs nothing — the flat line simply remains, and you can come back to
it or fix it by hand. Accept, and *before* the new prose lands, your old sentence
is snapshotted with a labelled entry, so the version you had is one `F6` away for
as long as the project lives. That order — snapshot first, then replace — is not
an incidental nicety; it is the safety contract, and the next section is about
why it holds without exception.

Because this is a plain single-line Rewrite, you could also have batched it: `F`
walks every ✎ Rewrite in the list in turn, each one its own diff to accept or
reject, `Esc` stopping the run wherever you are. On this list `F` would offer you
exactly one diff — the `editor` line — because it is the only Rewrite present. The
Decisions and the Briefs are never batched; they need you.

#subsection("⇄ Making a decision — which fact is true")

The two red rows at the top are Decisions, and they are red because they are the
findings a reader will feel first. Take the `fact` one. The fact-checker noticed
the manuscript states the lanterns' vigil two ways: *three hundred years* in the
opening of Chapter 1, *two hundred* in Sella's mouth on the quay in Chapter 4.
The machine cannot know which you meant — both are grammatical, both are clean
prose — so it does not touch either. It asks.

#screen(caption: "f on a ⇄ Decision — you rule, REDLINE reconciles")[```
┌─ ⇄ Decision · fact · ch. 1 ↔ ch. 4 ──────────────────────────┐
│ The manuscript states this two ways. Which is true?          │
│                                                              │
│   1  three hundred years    ch. 1 · the opening              │
│   2  two hundred years      ch. 4 · Sella on the quay        │
│                                                              │
│ ↑↓ choose · ⏎ confirm — REDLINE reconciles the other to your │
│ answer as a diff you accept + a snapshot · Esc leave it open │
└──────────────────────────────────────────────────────────────┘
```]

The bible says three hundred — the lanterns have burned three centuries — so you
choose 1. REDLINE does not simply record your ruling; it carries it into the
prose, rewriting the Chapter 4 line so Sella says *three hundred*, and it does so
through the *same* confirmed-diff-plus-snapshot path the line-rewrite took. A
Decision, once decided, becomes a reconcile that you still see as a diff and still
accept before it lands. You made the judgement the machine could not; the machine
did the retyping the judgement implied, and left the receipt.

The leaked-secret Decision above it works the same way, though its resolution is
usually not a rewrite at all: KEN caught Sella naming the reveal in Chapter 5,
before she learns it in Chapter 6, and the honest fixes — move the line past her
Chapter 6 grant, or grant her the knowledge earlier if she truly should have had
it — are structural choices you make in the manuscript, exactly as Chapter 5
described. REDLINE's job was to put that break in the same queue as the flat line,
so it is not forgotten, and to mark it a Decision so no tool ever "resolves" a
secret on your behalf.

#subsection("✉ Reading a brief — the saggy middle")

The last kind is the one an automated editor does the most damage pretending it
can fix. LECTOR's `shape_sag` says the middle of the book — Chapters 3 and 4 —
holds the same suspicion of Toft without deepening it, so the read goes slack
before the Mole. There is no paragraph to rewrite here; the problem lives in the
shape, across two chapters. So pressing `f` on it does not open a diff. It writes
a developmental *brief* to the Thoughts pane, and stops.

#screen(caption: "f on a ✉ Brief — advice to the Thoughts pane, no rewrite")[```
◆ Revision brief · ✉ shape_sag · ch. 3–4

The middle sags: the suspicion of Toft is raised, then held
at the same pitch for two chapters, so the read goes slack
before the turn onto the Mole. Three ways to give it a rise:

  · Give ch. 3 a smaller reversal of its own — a detail that
    half-clears Toft — so the suspicion has somewhere to move
    rather than simply persisting.
  · Bring one Mole thread forward: let Mira find the trail-
    head early, so the middle pulls toward the reveal instead
    of marking time until it.
  · Merge the two quay scenes; the second restates the first
    without raising the stake, and the join would tighten both.

(advice only — nothing was written to your prose)
```]

That is the whole of what a Brief does: name the problem in a line, give two or
three concrete, craft-grounded suggestions tied to *this* sag, and stop. It
advises; it never rewrites. The fix — cutting a scene, planting a reversal — is
work only you can do, and REDLINE does not pretend a paragraph-sized patch could
stand in for it. The brief lands where your other thinking lands, in the Thoughts
pane, for you to act on when you next open the middle.

#two_track(
  [The three kinds map onto the three problems every draft has: a *line* that
  reads flat (✎ rewrite it), a *fact* the book states two ways (⇄ decide which is
  true), and a *stretch* that sags across chapters (✉ take the brief). Our
  worklist has one of each, plus the mystery-specific leaked secret.],
  [An argument has the same three. A sentence that clears its throat instead of
  making its point is a ✎ Rewrite. A claim one of your own sources contradicts is
  a ⇄ Decision — you rule which the book will stand on. A section that drags
  before it earns its conclusion is a ✉ Brief. Same worklist, same three kinds of
  help.],
)

#section("The safety contract")

Everything above rests on one guarantee, and it is worth stating flatly: *REDLINE
will not edit your prose on its own.* This is not a setting or a policy layered on
top — it is the shape of the code. Every prose change REDLINE can make — a ✎ fix,
a ⇄ decision's reconcile, one step of the `F` batch — flows through exactly one
path, and there is no other.

#screen(caption: "Every REDLINE prose change takes the same three steps")[```
   the model's rewrite streams into the AI pane
                     │
                     ▼
   a diff-review modal shows the exact change
        accept (a / e / ⏎)   or   reject (r)
                     │  accept
                     ▼
   your old prose is SNAPSHOTTED first, labelled,
   and only THEN replaced
                     │
                     ▼
        recover it any time with  F6
```]

Read the three steps as promises. You always see the exact diff — the change is
shown, never applied blind. Nothing is written until you accept — reject, and the
paragraph is untouched. And on accept your old prose is snapshotted *before* the
new text lands, so the version you had survives one keystroke away. There is no
code path in REDLINE that writes prose without all three.

#callout(label: "The F batch is Rewrite-only by construction")[
  When you press `F` you are never one keystroke from an unconfirmed edit. The
  batch queue can hold *nothing but* ✎ Rewrites: a Decision or a Brief has no fix
  recipe to run in the first place, so it cannot enter the queue — the *type* of
  the finding forbids it, and a guard test locks the invariant. The batch is a
  convenience for the mechanical fixes, never an autopilot for the ones that need
  a human.
]

#section("The editorial letter — inkhaven revise")

The Editorial Pass is the row-by-row cockpit. Sometimes, though, you want the
opposite view first — the *overview* a good editor opens a revision with, before
touching a single line. That is `inkhaven revise`. It runs the very same worklist,
then synthesises the whole of it into one developmental letter.

#screen(caption: "inkhaven revise — the letter over the whole worklist")[```
$ inkhaven revise
revise: synthesising the editorial letter over 6 finding(s)…

Dear author,

The Ninth Lantern works because it guards one secret, so the
first thing to mend is the one place it slips: in Chapter 5
Sella already knows what she should not learn until Chapter 6.
Fix that before anything else — it is the whole difference
between a mystery and a misprint.

Continuity & knowledge
  That leaked reveal is the only hard break. A smaller snag
  sits beside it: the lanterns burn "three hundred years" in
  Chapter 1 and "two hundred" in Chapter 4 — one ruling settles it.

Structure & pacing
  The middle (ch. 3–4) holds its suspicion without deepening
  it; give it a reversal so it pulls toward the Mole.

Voice & character
  Sella and Toft still read alike — sharpen one and the quay
  scene stops sounding like one official talking to a mirror.

Line & prose
  A few lines name a feeling instead of earning it ("Mira felt
  afraid"); those are quick, local fixes.

Fix the leak, make the factual ruling, then walk the rest in
the Editorial Pass.
```]

The letter opens with the big picture — the one or two things a reader will feel
first — then groups the rest by theme, most important first, each with a brief
*why it matters* and *what to do*. It advises; it never rewrites — the writing
stays in the Editorial Pass's confirmed-diff loop. It is written in the
manuscript's language, and each finding keeps the reader it came from, so the
perennial *"does it work in Russian?"* answers itself per detector rather than as
one blanket promise. When you want the worklist for a script instead of a person,
the same command has a machine face:

#screen(caption: "The scriptable faces of the same worklist")[```
inkhaven revise                  # the editorial letter, whole
                                 # project
inkhaven revise --book-name X    # restrict to one book (slug or
                                 # title)
inkhaven revise --json           # the findings as JSON: category,
                                 # severity (high/med/low),
                                 # response, location, message
```]

The `--json` form emits the ranked findings — the AI letter and every prose
rewrite deliberately left out — ready for a hook or a report; and the read-only
`ink.revise.check` Bund word gives a plain *is this draft ready?* gate that is
`true` when nothing high-severity remains. Advice and prose changes stay
author-initiated; only the deterministic findings are scriptable.

That is the pass. You arrived with four readers' findings on four screens and
left with a flat line rewritten, a fact ruled on, a brief in hand for the middle,
and a secret marked for moving — every prose change a diff you accepted, every
old version a keystroke away. What you have not yet is proof that any of it
*helped*. That is the next chapter's question.

#recap((
  [*REDLINE* gathers every reader's findings — KEN's leaked secret, the
  fact-checker's contradiction, CHORUS's read-alike voices, LECTOR's sag, the
  Inner Editor's flat line — into *one ranked worklist*, errors first. The
  Editorial Pass (`Ctrl+V Shift+R`), `inkhaven revise`, and CHRONICLE all read the
  same list.],
  [Each finding carries a *response kind*, derived from the problem not guessed:
  *✎ Rewrite* (a diff-reviewed local fix), *⇄ Decision* (you rule which way is
  true, REDLINE reconciles the prose to your answer), or *✉ Brief* (advice for a
  structural problem, written to the Thoughts pane, never a rewrite). Only Rewrite
  touches prose.],
  [Walk it with `f` (act on one), `F` (batch *only* the ✎ Rewrites, one diff
  each), `[ ]` (filter), `⏎` (jump), `s` / `d` (skip / defer). A Decision's
  reconcile goes through the same confirmed-diff path a Rewrite does.],
  [The *safety contract*: every prose change streams a diff you see, is written
  only on your accept (`a` / `e` / `⏎`; `r` rejects), and *snapshots your old prose
  first* — one `F6` from recovery. There is no other path, and the `F` batch is
  Rewrite-only by construction.],
  [`inkhaven revise` synthesises the same worklist into one *editorial letter* —
  big picture, then grouped by theme — in the manuscript's language; `--json` and
  `ink.revise.check` expose the findings read-only for a script. It advises; it
  never rewrites.],
))
