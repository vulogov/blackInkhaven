#import "../design.typ": *

#chapter(number: 18, title: "The Read-Through and the Voices")

The last chapter's readers watch a manuscript for what it must not do — a fact
contradicted, a secret spilled early, a name used before it is met. This chapter
is about a softer, harder question: not "is the book *wrong*" but "how does the
book *read*." Four intelligences answer it, and they divide the labour by
zoom. LECTOR reads the whole book forward, once, as the one reader who matters
most — the *first* reader, who does not know the ending. CHORUS holds the whole
cast's voices in mind at book scale and asks whether any two of them blur.
DIALOGUE reads speech as its own craft, tracking who is talking and whether the
page ever loses them. And the narrator's own voice — the person telling the
story — is profiled across the draft so you can see chapter forty drift away
from chapter one.

None of the four edits a word. They are *reading* instruments: they measure, they
report, and they hand any fix they imply to the Editorial Pass (`Ctrl+V Shift+R`,
Chapter 19). Almost all of what they do is deterministic and free — no model
call, no cost — and where a model *does* help, the call is explicit, cost-capped,
and never automatic. Read this chapter as the tour of how a long book *sounds*
and *feels* from the outside, and how Inkhaven lets you hear it without leaving
the terminal.

#term("The outer readers")[
  Where the *inner readers* (Chapter 20) read a paragraph at a time, these four
  read *out*: LECTOR the whole arc, CHORUS the whole cast, DIALOGUE every
  exchange, NARR-1 the whole narrator over time. They are advisory to the last
  one — every finding is a signal to consider, never a change to your prose.
]

#section("LECTOR — the read-through")

Every other intelligence in Inkhaven reads *small*: a paragraph, a chapter break,
a character's lines. Nothing read the manuscript the way a person actually reads
it — cover to cover, in order, once, carrying forward everything met so far and
knowing nothing of what is coming. That reader is the one whose experience the
book lives or dies by, and LECTOR is Inkhaven's model of them. It reports two
different things about the read, and it is worth keeping them apart in your mind:
the *shape* of the read (its structure and pacing) and the *experience* of the
read (its clarity, attention, stakes, and payoff).

#subsection("The Shape half — the intensity curve against a framework")

LECTOR measures each chapter's dramatic *intensity* from the prose itself — no
tagging, no plan required. It reads the signals a reader feels as rising tension:
dialogue density, a per-language stakes-and-conflict lexicon, sentence-rhythm
acceleration, a penalty for passages that summarise rather than dramatise, and a
bonus for a chapter that ends on a turn. That gives a realised curve — how the
book *actually* rises and falls — which it then lays against the *intended* curve
of a story framework, and it flags the places where the shape wants a rise but
the prose reads flat.

#screen(caption: "inkhaven readthrough — the measured curve vs the framework")[```
Read-through — 12 chapter(s) · Hero's Journey
  measured   ▂▃▄▂▁▁▃▅▄▆█▃
  expected   ▁▂▃▄▅▅▆▇▇██▂

  ch  1  ▃ ▶   Arrival
  ch  2  ▄ ▶   The City
  ch  5  ▁ ◉   Old Debts
  ch 11  █ ▶   The Breakwater
  ch 12  ▃ ◉   After

⚠ [shape_sag] the Hero's Journey shape wants rising tension
             around ch. 5 (~55%) but the prose reads flat (~12%).

3 reader finding(s): 1 concern(s).
```]

The framework is not something you must configure. If you set `lector.framework`
it is used; otherwise LECTOR *suggests* one from your project `genre` — fantasy
and epic and quest lean to the Hero's Journey, thriller and romance to Save the
Cat, mystery and crime to the Seven-Point, character and coming-of-age to the
Story Circle, slice-of-life and literary and vignette to Kishōtenketsu — and if
it can infer nothing it falls back to Three-Act, so the overlay works on any
manuscript with zero setup.

#term("The six frameworks")[
  LECTOR ships six intended-shape curves: *Three-Act*, *Save the Cat*, *Story
  Circle*, *Hero's Journey*, *Seven-Point*, and *Kishōtenketsu*. The last is the
  odd and important one: the four-movement East-Asian structure
  (ki-shō-ten-ketsu) whose energy peaks at the *ten* — the recontextualising
  twist — rather than at a conflict climax. It is *conflict-optional*, so a
  quiet, tension-light book is not scored as if it failed to be a thriller.
]

Alongside the curve, LECTOR classifies every chapter on the *scene ⇄ sequel*
axis. A *scene* is the forward, external unit — goal, then conflict, then
disaster; a *sequel* is the reflective, internal one — reaction, then dilemma,
then decision. In the report a scene chapter carries `▶` (forward), a sequel
`◉` (reflective), and a chapter that is neither clearly `·`. The value is in the
*rhythm*: a long run of pure scene reads breathless with no room to feel the
cost, and a long run of pure sequel sags, so LECTOR flags the *arrhythmia*
rather than any single chapter's kind.

#callout(label: "Measured, not declared")[
  The Shape half *measures* the curve the Planning Board (Chapter 16) *declares*.
  If you never touched the Board, LECTOR still works — the intended curve comes
  from the framework, the realised curve from your prose. The two are independent
  tools that happen to speak the same language of beats.
]

#subsection("The Audience half — the forward walk")

The second half is the reader's *experience*, and its defining discipline is that
it is *forward-only*. LECTOR walks the book from the first chapter, carrying the
state a first reader would carry — which characters and places they have met,
which threads are hanging open, how the energy has been running — and every
finding it raises uses only the chapters read *so far*. A payoff in chapter thirty
can never reach back and cancel a dip it noticed in chapter four, because the
reader in chapter four had not read chapter thirty. That single rule is what makes
this a *reader* rather than an *analyst*.

Five deterministic findings come out of the walk, and all of them are zero-AI:

#screen(caption: "The five forward-walk findings")[```
  confusion       an entity used before it is introduced —
                  "who is this again?"

  info_dump       too many new names to meet in one chapter

  attention_dip   a flat, eventless chapter where attention
                  drifts

  put_down_risk   a RUN of flat chapters — a likely place a
                  reader sets the book down

  unpaid_setup    a setup raised early and never paid off
```]

The `confusion` and `info_dump` findings lean on the same Unicode-aware mention
matcher SENTINEL uses (Chapter 17), so they work in every script Inkhaven
supports, not only English. The intensity signals behind the Shape half key off
the project language exactly as the other detectors do — the stakes and reflection
lexicons ship for English, Russian, German, French, and Spanish and skip cleanly
elsewhere, while sentence rhythm is language-agnostic and always contributes.

#subsection("The synthetic first-read — the one, explicit LLM pass")

Some things the deterministic walk simply cannot judge: whether a passage is
*genuinely* confusing to a human, whether the stakes are *legible* on the page,
whether engagement is *really* flagging. For those, a model helps — and LECTOR's
one LLM feature, the *synthetic first-read*, reacts to each chapter as a first
reader who does not know the ending. It is forward-only by construction: each call
sees only a recap of what has been read plus the current chapter, never the
whole book at once.

It is never automatic. You ask for it — `inkhaven readthrough --deep`, or the `k`
key in the dashboard — and before each chapter the estimated cost is previewed
against your daily cap. Per Inkhaven's permissive principle the cost *informs*; it
never blocks. Its findings arrive tagged `source: reader` and land alongside the
deterministic ones, marked so you always know which came from a model.

#screen(caption: "The synthetic first-read, cost-capped and explicit")[```
  inkhaven readthrough --deep [--max-cost 8000] [--force]

  ch  4  synthetic first-read  · est. ~1,240 tokens  (cap ok)
  → the reader can follow the heist, but the guard's
    betrayal reads as a surprise the setup never earned.

  ch  5  synthetic first-read  · est. ~1,180 tokens  (cap ok)
  → the funeral lands, but two new cousins arrive unnamed
    and the reader loses the thread of who inherits.
```]

#subsection("Running it — the command, the dashboard, the Bund")

The command line prints the whole read in one shot: the measured-vs-expected
curve, the per-chapter scene/sequel beat, and the ranked reader findings. `--deep`
folds in the synthetic first-read; `--json` gives you the structured form for
tooling. In the editor the read-through has its own dashboard.

#screen(caption: "Ctrl+B Shift+A — the read-through dashboard")[```
┌─ Read-through · Hero's Journey · 12 ch ─────────────┐
│  measured   ▂▃▄▂▁▁▃▅▄▆█▃                            │
│  expected   ▁▂▃▄▅▅▆▇▇██▂                            │
│                                                     │
│ ⊗ put_down_risk   ch 5–6 run flat — likely put-down │
│ ⚠ shape_sag       ch 5 wants a rise, reads flat     │
│ ⚠ unpaid_setup    "the sealed letter" (ch 2) unpaid │
│ ● info_dump       ch 3 introduces 6 new names       │
│                                                     │
├─────────────────────────────────────────────────────┤
│ ↑↓ scroll   Enter jump to chapter   k synthetic-read│
│ Esc close                                           │
└─────────────────────────────────────────────────────┘
```]

`Ctrl+B Shift+A` opens that scrollable modal — the curve, the beats, and the
ranked findings together. Arrow keys scroll it, `Enter` jumps straight to the
flagged chapter in the editor, `k` runs the cost-capped synthetic first-read (its
results post to the Output pane's `readthrough` category), and `Esc` closes. The
deterministic findings also ride the unified review pass: `Ctrl+B Shift+C` adds a
`read-through` line, each finding anchored to its chapter, in the Output pane.

#screen(caption: "The LECTOR surface at a glance")[```
  inkhaven readthrough                 the full report
  inkhaven readthrough --deep          + synthetic read
  inkhaven readthrough --json          structured output

  Ctrl+B Shift+A    the dashboard (k = synthetic read)
  Ctrl+B Shift+C    read-through line in the review pass

  ink.readthrough.report   the ranked findings
  ink.readthrough.curve    per-chapter measured/expected
  ink.readthrough.check    counts by kind + severity
```]

The three `ink.readthrough.*` Bund words expose only the deterministic read — the
synthetic first-read is deliberately not scriptable, because a cost-bearing model
call has no place firing silently from a hook.

#callout(label: "What LECTOR is not")[
  It is not a rewriter — it reports the read and hands fixes to the Editorial
  Pass. It is not a per-paragraph reader — it is whole-book, forward, once. And it
  is not an oracle of taste: it reports legible, grounded signals — a flat stretch,
  an unmet name, an unpaid setup — never a verdict on whether your book is *good*.
]

#section("CHORUS — the voices at book scale")

A novel is not narrated in one voice but in many: the narrator's, and every
character who speaks. Inkhaven already profiles the narrator (NARR-1, below). What
makes a *cast* hold together — do the characters sound like distinct people, does
each one stay themselves, does the book keep the point-of-view rules it set, does
its register hold — is CHORUS. Three measurement pillars feed a seventh inner
reader, the Inner Stylist, and like everything here CHORUS *measures and reports*;
it never touches your prose.

#subsection("Pillar A — character voice and the distinctiveness matrix")

CHORUS profiles each character's dialogue through the *same* metric engine that
profiles the narrator — sentence rhythm, lexical diversity, hedging, interiority —
so a character's voice is measured on the same axes as the book's. From those
fingerprints it builds the *distinctiveness matrix*: it z-scores every voice
across the cast (the baseline is your own book's spread, so the comparison is
genre-relative) and looks for pairs that read alike. The headline finding is the
one every novelist fears — *"Mara and Joren read identically."*

#screen(caption: "inkhaven chorus voices — a signature card + the matrix")[```
Character voices — `The Sunken Throne` [en]

  Mara            confidence ●●●○   Δ from cast mean
    rhythm      ▇▇▇▅   varied           +0.4
    diversity   ▇▇▇▇   high             +0.6
    hedging     ▇▁▁▁   asserts          −0.3
    interiority ▇▇▁▁   moderate          0.0

  Distinctiveness
    ⚠ Mara ↔ Joren  read alike (z-dist 0.31 < 0.50)
    ✓ all other pairs distinct

  Drift
    ● Aldric  ch 9 drifts from his ch 1 voice
```]

Two safeguards keep it honest. Sparse speakers — a character with only a handful
of lines — are profiled but never *flagged*: CHORUS reports a confidence badge and
refuses to judge a voice it cannot measure. And deliberate look-alikes — twins, a
uniform chorus of guards — are silenced with `chorus.distinct_ignore_pairs`, so
you tell it once that Mara and Joren are *meant* to echo and it stops raising them.
Per-character drift is measured against a character's *first* chapter, answering
"does Aldric still sound like Aldric in Act III?"

#subsection("Pillar B — POV and tense discipline")

Two classic errors survive line editing because they are *structural*, not local.
The first is *head-hopping*: a named character other than the scene's viewpoint
shown accessing their own inner life — `Joren wondered…` in a scene that belongs
to Mara. A scene's POV is either *declared* with a paragraph tag or *inferred* from
who is mentioned most.

#screen(caption: "Declaring a scene's POV with a paragraph tag")[```
  pov:Mara         single POV — anyone else's interiority
                   is a leak

  pov:first        first person — ANY named character's
                   interiority is a leak

  pov:omniscient   deliberately multi-POV — head-hop off
  pov:multi        (an alias for pov:omniscient)
```]

CHORUS is honest that this is a heuristic: there is no parser in the paragraph
tree, so it catches interiority attributed to a *named* subject, not pronoun
antecedents — `she thought` where "she" is not the POV would need antecedent
resolution CHORUS deliberately does not attempt. The second error is a *tense
slip*: a manuscript that lapses out of the tense it established. Each narration
sentence is classified past or present from its copula and auxiliary anchors, the
scene's dominant tense is the majority, and the sentences that break it are
flagged.

#callout(label: "The tense gate is English-gated — Russian is excluded by design")[
  Tense-slip detection covers *English, German, French, and Spanish* — languages
  that share the "hold one narrative tense" convention. *Russian is excluded on
  purpose.* Russian narrative tense is governed by *aspect*: the historical
  present and the perfective/imperfective interleaving are legitimate devices, not
  slips, and nothing in the tree models aspect — so a past-to-present heuristic
  would be *wrong* for Russian. `chorus scan` says so plainly rather than
  false-flagging. Character voice and head-hop *do* work in Russian.
]

#subsection("Pillar C — register and diction")

The third pillar tracks the narrator's *register* — formal or plain, contracted
or measured, plain or archaic — per chapter, so *drift* becomes visible: "the
prose gets casual in Act III." It bundles a contraction rate, an archaism density,
a formality balance, and (for English) a latinate-diction proxy; chapters that
drift from the opening past `chorus.register_drift_threshold` are flagged. It is
solid for English and Russian and degrades gracefully — to whatever its word
lists cover — for the other languages rather than guessing.

#screen(caption: "inkhaven chorus scan — POV, head-hop, tense, register")[```
Voice-discipline scan — `The Sunken Throne` [en]

  POV / head-hop
    ⚠ ch 4 · Mara-POV — "Joren wondered" (interiority leak)
    ● ch 7 · POV inferred (Seren) — no pov: tag

  Tense
    ⚠ ch 6 · 3 present-tense slips in a past-tense scene

  Register
    ● ch 10 drifts casual vs ch 1 (Δ 0.11 > 0.08)
```]

#subsection("The Inner Stylist — the coach (Ctrl+B J → Y)")

The three pillars produce numbers; the *Inner Stylist* turns them into judgement.
It is the seventh member of the inner-reader family — alongside the Editor,
Socrates, the Theologian, the Poet, Rigor, and Grounding — and it does not
measure, it *synthesises*: it reads all three pillars and offers a few grounded
*Praise / Note / Concern* observations, in the inner-family's own voice
(*"I notice…"*, never a rewrite).

You reach it from the family hub. `Ctrl+B J` opens the Inner Socrates overview,
and `Y` from there opens the Inner Stylist. Inside, `F` synthesises the pillars to
the Output pane's `stylist` category, `E` engages the AI coach into the Thoughts
pane (grounded coaching, never a rewrite), and `R` opens the scrollable voice
report. It also rides the `Ctrl+B Shift+C` review pass.

#screen(caption: "Ctrl+B J → Y — the Inner Stylist overview")[```
┌─ Inner Stylist · voice at scale ────────────────────┐
│ Praise                                              │
│   the cast reads distinct — 9 of 10 pairs separate  │
│ Note                                                │
│   Aldric's voice drifts formal after ch 9           │
│ Concern                                             │
│   Mara ↔ Joren read alike — consider sharpening one │
│                                                     │
├─────────────────────────────────────────────────────┤
│ F synthesise → Output   E engage AI coach → Thoughts│
│ R report dashboard      Esc close                   │
└─────────────────────────────────────────────────────┘
```]

Each finding carries a stable *key*, and `--suppress <key>` silences it for good —
persisted in `inner_stylist.db`, this reader's own store — so a judgement you have
considered and rejected does not nag you again.

#subsection("The CHORUS command line and Bund")

Four subcommands cover the surface. `chorus voices` prints the signature cards and
the distinctiveness summary; `chorus scan` runs the POV, head-hop, tense, and
register checks; `chorus stylist` synthesises the Praise/Note/Concern (with
`--coach` for grounded LLM coaching and `--suppress`/`--unsuppress` to manage
findings); and `chorus report` is the one-screen dashboard that folds the narrator
profile, the cast voices, and the Stylist's synthesis together.

#screen(caption: "The CHORUS surface")[```
  inkhaven chorus voices  [--character NAME] [--json]
  inkhaven chorus scan    [--json]
  inkhaven chorus stylist [--coach] [--suppress KEY]
  inkhaven chorus report  [--json]

  ink.chorus.voices     per-character fingerprints
  ink.chorus.distinct   the distinctiveness matrix
  ink.chorus.drift      per-character voice drift
  ink.chorus.headhops   POV / head-hop findings
  ink.chorus.tense      tense summary (or the honest reason)
  ink.chorus.register   per-chapter register + drifts
  ink.stylist.findings  the synthesised Praise/Note/Concern
```]

Every `ink.chorus.*` word is deterministic and returns a list or dict; the LLM
coaching is not a Bund word (a cost-bearing call again stays off the script path)
— use `chorus stylist --coach` for it.

#callout(label: "What CHORUS is not")[
  It is not a style *corrector* — it flags, it never rewrites. It is not a grammar
  checker — there is no parser; the tense check is an honest, four-language
  heuristic. And it is not a "good writing" score — it measures *consistency* and
  *distinctiveness*, not quality. Statistical voice is not literary voice, and
  every surface states its own limits.
]

#section("DIALOGUE — speech as its own craft")

Dialogue is the most technically demanding prose mode: a reader must always know
who is speaking, said-bookisms (`he ejaculated`) degrade the page, and an unbroken
run of talking heads loses the physical scene. The Inner Socrates flags
*unattributed* speech in passing, but DIALOG-1 measures dialogue as a *mode* with
its own properties — deterministically, with no AI and no parser, in the five
languages.

`inkhaven dialogue scan` detects every dialogue span and raises three findings:
*zero-attribution* (a speech span with no tag and no nearby character name — though
an established two-speaker exchange is allowed to alternate untagged for a run,
default eight turns, before it fires, because readers track that fine),
*said-bookism density* (a chapter whose non-neutral tag verbs — `whispered`,
`growled` — run above the book's own baseline), and *talking heads* (a run of
dialogue-only paragraphs with no action beat to ground the scene).

#screen(caption: "inkhaven dialogue scan — a pre-submission gate")[```
Dialogue scan — `The Sunken Throne` [en]
──────────────────────────────────────────────────────
  dialogue spans detected   34
  zero attribution           3
  talking-head sequences     1
──────────────────────────────────────────────────────
  [ch.12 · 7f1a…] unattributed speech — no tag or
                  character name within range
──────────────────────────────────────────────────────
```]

`scan` *exits non-zero* if any zero-attribution span is found, so it doubles as a
CI gate — `inkhaven dialogue scan --findings zero-attribution || echo "fix
attribution first"`. Detection is deliberately *not* uniform across languages,
because quotation convention is not: English and German use paired marks
(`"…"`, `„…"`, `»…«`), French and Russian use guillemets and em-dash openers
(with the French inline *incise*, `, dit-il,`, stripped before measuring), and
Spanish detects all three additively.

#subsection("Fingerprints and the fingerprint view (Ctrl+V Shift+Q)")

`inkhaven dialogue profile` builds a six-metric voice signature for each character
from the lines confidently attributed to them — are they terse or expansive, do
they ask or declare, do they hedge or assert?

#screen(caption: "inkhaven dialogue profile — per-character signatures")[```
Character    Utts  Avg words  MATTR  Quest.  Excl.  Hedge
Mara           47      11.3   0.74    0.31   0.08   0.019
Aldric         83      11.8   0.69    0.12   0.22   0.041
Seren          31       7.1   0.61    0.44   0.04   0.009
```]

In the editor, `Ctrl+V Shift+Q` opens the fingerprint view for the *nearest*
character — one named in the paragraph you are in, else the most-speaking — as
ASCII bars with a compare line for the next speakers. It is built from
confidently-attributed dialogue, so run the `Ctrl+B Shift+C` review pass (or
`inkhaven dialogue scan`) once to populate it. The mnemonic is `Q` for *Quote*;
`Ctrl+V D` was already taken.

#callout(label: "The theatergoer persona")[
  DIALOG-1 also added an Inner Socrates persona, *theatergoer* — cycle to it with
  `Ctrl+B J → S`. It reads a dialogue scene as if the narrator's private glosses
  were invisible and asks whether the subtext is legible from the *words and
  visible action alone* — the test a stage or screen adaptation would impose.
]

The dialogue *findings* ride the `Ctrl+B Shift+C` review pass into the Output pane,
navigable to the flagged paragraph; the check is zero-AI and hash-lazy, so only the
chapter you just edited is re-detected. From Bund, `ink.dialogue.stats`,
`ink.dialogue.fingerprint`, `ink.dialogue.violations`, `ink.dialogue.spans`, and
`ink.dialogue.refresh` expose it all read-only. The `dialogue:` config block tunes
the windows and — importantly for SF and fantasy — marks genre speech verbs as
neutral with `extra_neutral_verbs` so telepathy's `transmitted` is not counted as
a bookism.

#section("NARR-1 — the narrator's voice over time")

The fourth reader is the oldest and the quietest: `inkhaven prose` profiles the
*narrator's* voice as a statistical property of the whole book over time — sentence
rhythm, lexical diversity, epistemic hedging, interiority, sensory balance, passive
ratio — per chapter, deterministically, with no model, no parser, and no external
dependency. It answers a question nothing else in Inkhaven does: *does chapter
forty read like the person who wrote chapter one?* It measures where the voice
moved; it never says the move was wrong.

The always-on tier is language-agnostic rhythm — the sentence-length distribution,
the coefficient of variation (a falling CV across chapters is the primary signal of
a voice *narrowing*), a bounded burstiness index, and MATTR, a length-corrected
lexical diversity whose late drop can mean vocabulary fatigue. Two language-sensitive
metrics ride on top — modal (hedging) density and an interiority ratio — with
complete curated word lists for all five first-class languages. A deep pass
(`--deep`) adds sensory-channel balance and a per-language active/passive ratio.

#screen(caption: "inkhaven prose — the narrator profiler")[```
  inkhaven prose profile  [--deep] [--json] [--language de]
  inkhaven prose refresh                 recompute, summary
  inkhaven prose drift    [--mode baseline|rolling]
  inkhaven prose suggest                 how to read them

Voice profile — `The Sunken Throne` [en] · 12 ch
  sentence CV     0.42   varied
  MATTR           0.71   healthy
  modal density   0.018
  interiority     0.22

Drift vs ch 1
  ⚠ ch 9  CV 0.42 → 0.28   the voice is narrowing
```]

In the editor, `Ctrl+V V` ("Voice") runs the profiler in the *background* —
deterministic, content-hash lazy, so only edited chapters recompute and there is no
cost. Any chapter metric that drifted past its threshold versus the baseline chapter
is emitted to the Output pane as an informational `prose` finding that navigates to
the chapter. `Ctrl+V Shift+V` toggles *ambient* mode (off by default): the check
re-runs after an editing pause, gated by a cooldown floor. From Bund,
`ink.prose.{profile, drift, violations}` read the stored profiles and
`ink.prose.refresh` recomputes; language-sensitive metrics return `null`, not zero,
on an unsupported-language book.

#callout(label: "NARR-1 and CHORUS are one engine")[
  NARR-1 profiles the *narrator*; CHORUS reuses the very same metric engine per
  *character* to build the dialogue fingerprints and the distinctiveness matrix.
  When you read a character's rhythm/diversity/hedging/interiority card, you are
  reading the narrator's own instrument turned on that character's lines.
]

#section("Where the four readers meet")

These four instruments overlap on purpose, and they converge in two places. The
first is the review pass, `Ctrl+B Shift+C`: one keystroke runs the fast checkers
together, and the LECTOR read-through line, the DIALOGUE findings, the Inner
Stylist's synthesis, and the prose-drift notices all land in the Output pane
alongside the fact and continuity checks — a single board of everything the book
noticed about itself. The second is the *Editorial Pass*, `Ctrl+V Shift+R`
(Chapter 19): every reader here is advisory, and the Pass is where their findings
become a ranked worklist you can *act* on — a diff-reviewed rewrite, a guided
decision, or a written brief — under the confirmed-diff, snapshot-first contract
that means no reader ever changes your prose without your explicit say-so.

#recap((
  [*LECTOR* reads the whole book forward, once, as a first reader: the *Shape*
  half measures a prose-derived intensity curve against one of six frameworks
  (including conflict-optional *Kishōtenketsu*) and flags `shape_sag` plus
  scene/sequel arrhythmia; the *Audience* half walks forward-only for `confusion`,
  `info_dump`, `attention_dip`, `put_down_risk`, and `unpaid_setup`. Free at the
  core; the *synthetic first-read* is the one explicit, cost-capped LLM pass
  (`inkhaven readthrough --deep`, or `k` in `Ctrl+B Shift+A`).],
  [*CHORUS* profiles the cast at book scale: character voice fingerprints + the
  *distinctiveness matrix* ("Mara and Joren read identically"), POV/head-hop and a
  tense check (English/German/French/Spanish — *Russian excluded by design*), and
  register drift. The *Inner Stylist* (`Ctrl+B J → Y`) synthesises it into
  Praise / Note / Concern. Declare a scene's POV with `pov:<name>`.],
  [*DIALOGUE* (DIALOG-1) reads speech as a mode — attribution, per-character
  fingerprints, said-bookism density, talking-head runs — deterministically in five
  languages. `inkhaven dialogue scan` doubles as a CI gate; `Ctrl+V Shift+Q` opens
  the fingerprint view.],
  [*NARR-1* (`inkhaven prose`) profiles the *narrator's* voice over time — rhythm,
  diversity, hedging, interiority — to catch a voice narrowing or drifting.
  `Ctrl+V V` runs it in the background; `Ctrl+V Shift+V` makes it ambient. It shares
  its metric engine with CHORUS.],
  [All four are *advisory* — they measure and report, never edit. They converge in
  the `Ctrl+B Shift+C` review pass, and their findings become actionable only in the
  Editorial Pass (`Ctrl+V Shift+R`), under a snapshot-first, confirmed-diff
  contract.],
))
