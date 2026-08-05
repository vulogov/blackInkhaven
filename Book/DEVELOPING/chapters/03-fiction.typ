#import "../design.typ": *

#chapter(number: 3, title: "The fiction track")

Fiction is the track Inkhaven was first built to serve, and the one that uses the
widest span of its tools. You are inventing a world and a cast and a story, and the
whole discipline of the track is a single promise: _nothing you invent will
quietly contradict anything else you invented._ This chapter walks the working
loop — frame, gather, draft, read — as a novelist runs it.

#section("Frame — set the genre, lay the first bones")

Start a project with the novel template and declare your genre, so the readers that
will later question your prose are tuned to the kind of story you are telling:

```
inkhaven init "the-salt-road" --template novel
```

#config("inkhaven.hjson", [```hjson
genre: "fantasy"
```])

The genre can be any of the fiction keys — `literary`, `fantasy`, `mystery`,
`historical`, `romance`, `horror`, `ya`, `comedy` — and it changes how the Inner
Socrates and Inner Editor read you later. With the project open (`inkhaven tui`),
lay down the coarse structure: a few chapters, a scene or two you already hear.
Don't over-plan; the point of the loop is that structure and world grow together.

#section("Gather — build the world the story stands on")

This is where fiction earns its reputation as the heaviest track. Your ground is a
*world*, and Inkhaven can grow you a consistent one from a tiny definition rather
than a binder of notes that drift.

#subsection("The world simulation")

In the World book, a small `world.hjson` describes a star, a planet, and a few
choices; `inkhaven realworld compile` grows it into a full world — climate, rivers,
biomes, where cities would stand — and `--materialize` writes that world into
readable chapters beside your manuscript. Because the world is _computed_, the sun
rises in the same place every time and a river never runs uphill.

```
inkhaven realworld compile --materialize
```

The full craft of this lives in the companion volume, _Building the World with
Inkhaven_. For fiction, the point is that the world becomes _present at your desk_:
`inkhaven realworld scene` gives you the season, weather, and nearest settlement
for a scene at a given place and day, so you write into a real climate rather than
a guessed one.

#subsection("Characters, threads, and myth")

The Characters book holds your cast; `inkhaven character arc` and `character check`
help you declare where a character is meant to go and ask whether the pages get
them there. The Threads book (`Ctrl+V Shift+H`) tracks each plot thread from setup
through payoff, so a promise made in chapter two is not forgotten in chapter
twenty. And if your story leans on recurring images — a symbol that gathers weight
— the Mythology book lets you _declare_ them, so a later reader can check the prose
keeps faith with them.

#note[
  A world is optional. Plenty of fiction needs only Places and Characters, not a
  compiled planet. Reach for the full world simulation when the setting is doing
  real work — when distance, season, and geography matter to the plot. A quiet
  domestic novel may never open the World book at all, and that is a correct use of
  the track.
]

#subsection("A tongue, if the world needs one")

If your peoples should speak something, the ConLang hub (`Ctrl+B X`) builds a real
constructed language — phonology, lexicon, grammar, even a script — that you can
translate prose into and out of. The world simulation can even propose a language
per culture. This is a deep instrument with its own companion book, _Constructed
Language Development_; most fiction never needs it, and the fiction that does needs
it badly.

#section("Draft — write against the world")

Now the loop turns to prose. The advantage the gathering bought you is that you no
longer write into a void: the world is a room you can consult. Keep a scene brief
open as you write a location; drop into `Ctrl+/` to find the paragraph where you
first described a character; use `F4` split-edit to try a bolder version of a line
without losing the safe one.

#insight[
  The discipline of fiction on this track is _write first, accept later_. The world
  proposes — a settlement, a name, a calendar date — and you decide what enters the
  manuscript. Nothing the simulation generates is written into your book without
  your accepting it. The author always has the last word; the tools only make the
  first draft of the offer.
]

#section("Read — turn the questioners on the draft")

When a scene is down, fiction's second heavy investment pays off: a family of AI
readers that question rather than approve. None of them rewrites your prose; each
asks something a good editor would.

#subsection("The inner readers")

Press `Ctrl+B J` for *Inner Socrates* — a classical interrogator that asks what a
passage presupposes, what alternatives it closed off, what it leaves unsaid. It
comes with a roster of personas you can choose among: the `first-time-reader` who
notices what confuses, the `skeptical-reader` who doubts, the `careful-editor` who
watches craft, the `myth-reader` who asks whether a symbol earned its weight, and
the `inner-historian` who checks the page against your world's own chronology.
Because you set the genre, every one of them reads _as a reader of your kind of
book_.

Press `Ctrl+V o` for the *Inner Editor* — a craft reader attentive to clarity,
rhythm, and momentum, tuned by genre and by your own tuning knobs.

#subsection("The checks")

Where the readers ask, the checks _measure_. `Ctrl+B Shift+X` fact-checks the
current paragraph against the compiled world — is a journey plausible for its
distance and mode, is the weather right for the date, does a claimed span of time
hold? `inkhaven drift scan` watches for style and continuity drift across the book.
`inkhaven continuity check` unifies the deterministic continuity checks into one
ranked ledger — co-location, timeline, numeric contradictions, character-fact
drift, and an entity _referenced before it is introduced_ — surfaced together in
the `Ctrl+B Shift+I` dashboard (Enter jumps to the slip; `k` runs the LLM
coherence pass for the contradictions the patterns can't see).

There is one more axis of continuity, and it is the one a mystery lives or dies on:
not where a character is, but what they _know_. `inkhaven knowledge` watches it. You
declare the stakes with tags — `secret:the betrayal`, `know:the betrayal@Mara`,
`reveals:` on the event that lets it slip — and KEN gets the rest for free from your
timeline (anyone present at an event knows it). Then it walks the book and flags the
moment a character speaks of, or acts on, something they could not know yet — a
premature reference, a leaked secret, a reveal you set up and never spent. It is the
_referenced-before-introduced_ rule, moved from existence to knowledge; the
`Ctrl+B Shift+Z` dashboard jumps you straight to the slip. Deterministic and free —
it costs nothing until you ask for the subtle, unnamed cases with `--deep`.

A unified review pass (`Ctrl+B Shift+C`) folds the readers and all of these checks
into one sweep over a finished stretch; with `continuity.ambient` on, the continuity
slice re-checks itself on every save.

#subsection("From finding to fix — the revision partner")

Reading is only half of revision; the other half is _acting_, and Inkhaven treats
that as its own discipline. `Ctrl+V Shift+R` opens the *Editorial Pass* — the one
worklist that unifies every reader (the prose checks, continuity, the read-through,
the Inner Editor, voice) — and beside each finding it shows _how_ it can be acted
on. A `✎` finding has an honest local fix: press `f` and Inkhaven streams a rewrite
into a diff you accept or reject. A `⇄` finding is a judgement only you can make —
which scene a character is really in, which fact is canon; it asks you, then
reconciles the paragraph to your answer. A `✉` finding is structural — a saggy act,
a likely put-down point — and gets a written brief in the Thoughts pane, advice
rather than a rewrite, because moving the furniture stays yours.

For the overview a writer opens a revision with, `inkhaven revise` synthesises the
same worklist into one editorial letter: the big picture first, then grouped by
theme, most important first.

#subsection("Did it get better?")

Revision is a leap of faith unless you can measure it. Before a serious pass, stamp
a milestone — `inkhaven chronicle mark "draft-2"` — and Inkhaven records what every
reader found. After the pass, `inkhaven chronicle` (or `Ctrl+B Shift+U` in the
editor) captures the book again and trends it against that mark: fewer findings and
fewer sags read as `▼`, new problems as `▲`. Its most useful line is the split — how
many findings your revision _cleared_, and how many it _introduced_ — because good
revision often trades one problem for another, and this is how you catch the trade
before a reader does. Press `Enter` on an introduced finding to jump straight to the
paragraph your last edits broke. Chronicle only ever measures; it never touches a
word.

#insight[
  Every prose change the revision partner makes passes through the same contract:
  a rewrite you see as a diff and confirm, and a snapshot of your old prose taken
  _before_ the replace — recover it any time with `F6`. Inkhaven never edits your
  sentences on its own. The reader points; the author decides; the old words are
  always one keystroke back.
]

#pitfall[
  Don't run the heavy AI readers on every keystroke. They cost money and attention,
  and a first draft is not the place for a skeptic. Draft a scene to its end, _then_
  read it — the deterministic checks (fact-check, co-location, drift) as often as
  you like, the LLM readers on a finished scene or chapter where a careful second
  pass pays for itself.
]

#section("Produce — the book leaves the desk")

Mark scenes with a status as they harden (`Ctrl+B r`), and when a draft is ready,
`inkhaven export epub` or `inkhaven export pdf` renders it — scoped with
`--status ready` if you want only the finished parts. If the book is going out on
submission, the Submissions tools (`Ctrl+V u`) track agents and generate a synopsis
and comparables from the manuscript itself.

#tryit[
  Run one small loop end to end. Write a single scene set in a specific place. Open
  its scene brief (`inkhaven realworld scene`) and correct one detail to match the
  weather. Then read the scene with the `first-time-reader` persona (`Ctrl+B J`) and
  fact-check it (`Ctrl+B Shift+X`). You have just done, in miniature, everything the
  fiction track is.
]

#section("Hands-on: three procedures")

Here is the fiction loop as concrete keystrokes. Do these once and the abstractions
above become muscle memory.

#subsection("Grow a world and bring a settlement into your book")

+ From the project root, create a world definition: `inkhaven realworld new "Aeloria"`. This writes a small `world.hjson` into the World book.
+ Open `world.hjson` (press `Ctrl+B W`, or edit the file) and set the star, planet, and seed to taste. Check it is well-formed: `inkhaven realworld validate`.
+ Grow the world and write it into readable chapters: `inkhaven realworld compile --materialize`. The World book now holds pages for climate, rivers, settlements, and more.
+ See what the world offers your manuscript: `inkhaven realworld proposals`. It lists settlements, a calendar, and history events waiting for your decision.
+ Accept the ones you want — `inkhaven realworld proposals accept <id>` on the command line, or `Ctrl+B W` then `P` in the editor. Each accepted settlement becomes a Place in your Places book. Nothing enters without this step.
+ Give a hand-authored Place a spot on the map so it grounds its neighbours: `inkhaven realworld set-coords "Harbor Town" --lat 34 --lon -12`.

#subsection("Write a scene against the world")

+ Ask the world what a scene's setting is like: `inkhaven realworld scene --place "Harbor Town" --day 200`. It returns the season, the weather at that latitude, the biome, and the nearest named feature.
+ Draft the scene in the editor, matching what the brief told you. Use `F4` to split-edit a line you want to try two ways, and `Ctrl+F4` to accept the version you keep.
+ Check the scene holds up: `Ctrl+B Shift+X` fact-checks the paragraph against the world — travel time, weather for the date, elapsed time — and flags anything that cannot be true.

#subsection("Read a finished scene")

+ Open the Socratic reading: `Ctrl+B J`. Pick a persona for what you need — `first-time-reader` to find confusion, `careful-editor` for craft, `inner-historian` to test the scene against your world's chronology.
+ Open the craft reader: `Ctrl+V o` for the Inner Editor's attention to clarity and rhythm.
+ Sweep the book for style and continuity slips: `inkhaven drift scan`. Run this as often as you like — it is deterministic and cheap.

#recap((
  [*Frame* with `init --template novel` and a fiction `genre`, which tunes every AI reader to your kind of story.],
  [*Gather* a world you can trust: `realworld compile --materialize` grows a consistent planet, Characters and Threads hold the cast and promises, Mythology declares your symbols, and the ConLang hub builds a tongue when the world needs one.],
  [*Draft against the world* — scene briefs, semantic search, and split-edit let you write into a real setting rather than a void; the world proposes, you accept.],
  [*Read* with the inner readers (`Ctrl+B J`, `Ctrl+V o`) for the questions and the checks (`Ctrl+B Shift+X`, `drift`, `co-location`) for the measurements — after a scene is drafted, not during.],
  [*Produce* with `export epub|pdf` scoped by status, and the Submissions tools when the book goes out.],
))
