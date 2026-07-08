#import "../design.typ": *

#chapter(number: 1, title: "The document tracks")

Every book Inkhaven helps you write shares the same desk. What changes from one
kind of book to the next is not the desk but the _work_ — what you must invent,
what you must not get wrong, and who you imagine reading over your shoulder. This
chapter lays the eight tracks side by side so you can see, before you commit a
single word, which one your book belongs to and what it will ask of you.

A track has three defining marks. First, its *ground* — the thing the book must
stay true to: an invented world, a body of verified fact, a specification, a
philosophical position. Second, its *risk* — the particular way this kind of book
tends to fail: a novel contradicts its own map; a manual drifts from the software;
a paper cites a source that doesn't say what it claims. Third, its *reader* — the
skeptic Inkhaven imagines when it reads your draft back to you, which you name
with a single setting.

#section("The one setting that colours everything")

Before the tracks themselves, meet the switch that tells Inkhaven which one you are
on. In your project's `inkhaven.hjson`, a single line declares the genre:

#config("inkhaven.hjson", [```hjson
genre: "fantasy"
```])

That word is not decoration. It changes how the *AI readers* frame your prose —
the Inner Socrates interrogator, the Inner Editor — so their questions are
calibrated to your kind of book rather than to a generic one. Declare `theology`
and the readers stop demanding empirical proof of a claim that was never meant to
be empirical; declare `technical` and they read for precision and the reader's
task rather than for imagery. It also seeds the right continuity categories when
you set up a Facts book, and travels into the metadata of anything you export.

#note[
  The genre is free-form, and forgiving. It normalizes aliases — `sci_fi`,
  `science_fiction`, and `scifi` are one thing; `utopia` and `dystopian` share a
  frame — and an unfamiliar value simply falls back to a neutral reading rather
  than erroring. Leaving it unset is a valid choice: Inkhaven then judges the prose
  "on its own terms." Setting it is how you get a reader tuned to your track.
]

#insight[
  A track is a lens, not a cage. The genre line changes framing and defaults; it
  never removes a tool. Everything Inkhaven can do is available on every track. The
  point of choosing is focus — knowing which of the many tools are _yours_.
]

#section("Fiction — the invented world made consistent")

The fiction track (genre `literary`, `fantasy`, `mystery`, `historical`,
`romance`, `horror`, and their kin) is defined by *invention held to account*. You
make everything up — and the moment you do, you take on a debt: the made-up world
must not contradict itself. Its ground is the world you build; its risk is drift
(the tower two different heights in chapters three and twelve, the rider crossing
a continent in an afternoon); its reader is the attentive fan who remembers what
you wrote four hundred pages ago.

Fiction leans hardest on *worldbuilding* and on *reading*. It is the track for
which the world simulation, the character and myth layers, and the questioning
inner readers were most directly built.

#section("Utopia — the society as an argument")

The utopian and dystopian track (genre `utopian`) is a special case of fiction
with a philosophical spine. Its ground is not a landscape but a _premise_ — "what
if a society were organised like this?" — and its risk is incoherence: a designed
world that quietly cheats, that declares an ideal and then relies on the very
thing the ideal forbade. Its reader is the architect who asks whether the system
actually holds together.

Utopia borrows fiction's whole toolkit and adds one instrument the others rarely
touch: a *coherence checker* that reads the world you declared as a chain of
claims and looks for the link that breaks.

#section("Science fiction — invention that must obey a rule")

The science-fiction track (genre `scifi`) is fiction with a second ground: not
only must the world be self-consistent, its invented technology and physics must
be _rule-bound_. The risk is the same drift as fiction plus a new one — a
capability that appears when the plot needs it and vanishes when it would solve
the plot too early. Its reader is the one who accepts your one impossible thing
and holds you rigidly to its consequences.

Science fiction uses the world simulation like fiction, and adds the discipline of
the *magic ledger* (here, a technology ledger) and a research habit for the real
science it bends.

#section("Nonfiction — the claim that must be true")

The nonfiction track (genre `nonfiction`, `memoir`, `business`) inverts fiction's
relationship to truth. Here you invent nothing; your ground is _the world as it
is_, and your risk is the confident sentence that turns out to be wrong. Its
reader is the skeptical practitioner or the newcomer who will act on what you say.

Nonfiction lives on *grounding* and *fact-checking* — the research assistant, the
Facts book, the citation manager — far more than on invention.

#section("Scenarios — the world others will play in")

The scenarios track (game modules, interactive fiction, RPG sourcebooks — set up
with the `rpg-sourcebook` template, genre usually `fantasy` or `scifi`) writes not
a story but a _space for stories_: a place, its people, its situations, and the
branches a reader-player can take. Its ground is a world that must be _usable_ at
speed — a game-master flipping to a location mid-session. Its risk is a gap: a
place named but not described, a thread with a setup and no payoff. Its reader is
the referee who needs the answer _now_.

Scenarios use the world simulation for the setting, the Places and Threads books
hardest of all, and reference-grade structure so nothing is unreachable.

#section("Technical documentation — the text that must match the system")

The technical track (genre `technical`, `documentation`) documents something that
_exists and changes_ — software, an API, a machine. Its ground is the system
itself, and its risk is staleness: prose that was true of last release and is a
lie about this one. Its reader is the practitioner trying to do a task, and the
newcomer who has none of your context.

Technical writing leans on *structure* and *terminology governance* — reusable
blocks, a controlled glossary, reference-grade organisation — and on the AUDIENCE
readers that imagine an end user rather than a fan.

#section("Scientific writing — the argument that must be sourced")

The scientific track (genre `science`, `academic`) is nonfiction at its most
demanding: every load-bearing claim must trace to evidence, and the argument must
survive a hostile reading. Its ground is _the literature_ and _the data_; its risk
is the unsupported claim and the citation that doesn't resolve. Its reader is the
expert reviewer looking for the hole.

Scientific writing lives on the *research assistant*, the *Sources* book and its
bibliography, and the adversarial *verdict* readers — the prosecutor who tries to
break your claim before a referee does.

#section("Theology and philosophy — the position argued in good faith")

The theology-and-philosophy track (genre `theology`, `philosophy`) argues about
things that are not settled by measurement. Its ground is _internal coherence_ and
_the tradition it answers to_; its risk is not being wrong about a fact but being
incoherent, or arguing past the strongest form of the opposing view. Its reader is
the theological or philosophical reader who grants the frame and presses the
reasoning.

This track uses the readers most subtly of all: an Inner Theologian tuned to moral
and theological weight, a Socratic interrogator that — told the genre — stops
asking for proof and starts asking whether the argument holds.

#section("How the tracks differ, at a glance")

Read down this list to place your own book. The pattern is simple: as you move from
fiction toward science, the balance shifts from _invention held consistent_ toward
_claims held true_ — and the tools shift with it.

#chord_table((
  chord_row("Fiction", "Ground: an invented world · Risk: self-contradiction · Reader: the attentive fan · Leans on worldbuilding + inner readers."),
  chord_row("Utopia", "Ground: a social premise · Risk: incoherence · Reader: the system architect · Adds the coherence checker."),
  chord_row("Science fiction", "Ground: world + a rule-bound novum · Risk: convenient powers · Reader: the rigorous fan · Adds a technology ledger + real-science research."),
  chord_row("Nonfiction", "Ground: the world as it is · Risk: the wrong claim · Reader: the practitioner · Leans on research + fact-check."),
  chord_row("Scenarios", "Ground: a usable world · Risk: an unreachable gap · Reader: the referee mid-session · Leans on Places, Threads, reference structure."),
  chord_row("Technical", "Ground: a changing system · Risk: staleness · Reader: the task-driven user · Leans on structure + terminology + reuse."),
  chord_row("Scientific", "Ground: the literature + data · Risk: the unsourced claim · Reader: the hostile referee · Leans on sources + adversarial readers."),
  chord_row("Theology / philosophy", "Ground: internal coherence + a tradition · Risk: incoherence, strawmanning · Reader: the good-faith opponent · Leans on the moral + Socratic readers."),
))

#question[
  Which sentence, if a reader caught it being false, would most embarrass your book
  — "the moon was full two nights running", or "the study found the opposite of
  what you claimed", or "this flag was removed three releases ago"? Your answer
  names your risk, and your risk names your track.
]

#recap((
  [A *track* is a kind of book plus its working process, defined by three marks: its *ground* (what it must stay true to), its *risk* (how it tends to fail), and its *reader* (the skeptic Inkhaven imagines).],
  [The `genre` line in `inkhaven.hjson` is the one setting that tunes the AI readers, seeds continuity categories, and travels into exports — set it to get a reader calibrated to your track.],
  [The eight tracks run from *invention held consistent* (fiction, utopia, science fiction, scenarios) to *claims held true* (nonfiction, technical, scientific, theology/philosophy), and the tools shift along that line.],
  [A track is a lens, not a cage: choosing one focuses your attention on the tools that pay off, without removing any others. Most books lean on one track and borrow from a second.],
))
