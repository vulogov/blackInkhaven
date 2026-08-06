#import "../design.typ": *

#chapter(number: 20, title: "The Inner Family")

Every reader you have met so far in this part answers a question you can put
plainly: is this fact consistent, does this reference come before its
introduction, does the pacing sag on page one. The readers in this chapter are
different in kind. They do not answer; they *ask*. They read your prose the way
a careful editor, a sceptical friend, or a thoughtful stranger would — and hand
back not corrections but the questions those readers would raise. The choice of
what to do about each one stays entirely yours.

Inkhaven calls them the *Inner family*, and there are five: the *Inner Socrates*
(the dialectician who surfaces the assumptions and framings your prose treats as
given), the *Inner Editor* (who notices craft — what the words are doing to
themselves and to the reader), the *Inner Theologian* (who reads the moral and
theological weight of what you depict, through eleven traditions and belonging
to none), the *Inner Poet* (who measures verse against the form it declares),
and the *reasoning-rigor reader* (who scans arguments, deterministically, for
the classic weaknesses). This chapter is the operator's tour of all five: what
each one is for, how to reach it, and how to run it. When you want the full
treatment — the theory behind each, the worked case studies — the companion
book *Know Your Book* devotes its Part VI to exactly this family.

#callout(label: "The one promise")[
  Not one member of the Inner family ever edits your prose. Every finding is a
  question, an observation, or a flag — surfaced to the Output pane or printed
  to your terminal, and nothing more. There is no confirm-this-rewrite path
  anywhere in the family; that machinery belongs to REDLINE (Chapter 19), not
  here. The family reads; you write.
]

#section("What the whole family shares")

Before the five, the shape they hold in common. Learn it once and each member
is a variation on a form you already know.

They are *advisory*. A finding is a candidate for your attention, never a
verdict on your book. The word "should" is, by structural commitment, absent
from what they say — they report what a reader would notice or ask, and leave
the judgement to you.

They surface to the *Output pane*. Findings land as messages on the notice
board you met in Chapter 3, each carrying its own glyph so you can tell the
families apart at a glance: `◇` for a Socratic question, `✎` for an Editor
observation, `⚖` for a theological question, `⊬` for a reasoning-rigor flag.
Press `f` in the Output pane to filter to one source; press `o` on a row to
expand it to the evidence it grounds in; press `d` to dismiss one.

Most of them split into a *Fast track* and a *Slow track*. The Fast track is
deterministic — pattern-matching and the language detector, no model, no
network, instant and free. The Slow track calls your configured LLM for the
questions patterns cannot reach, and is always cost-capped: each call prints a
preflight, respects a per-call soft cap and a daily ceiling, and degrades to a
quiet notice when no provider is configured. The Fast track always works; the
Slow track is where you spend.

They are *multilingual*. Every deterministic detector keys its cue tables off
the paragraph's language across the five Inkhaven supports — English, Russian,
German, French, Spanish — and every LLM question is posed in the book's
language with an English fallback. The question a Russian paragraph raises is
raised in Russian.

And most of them honour a *shared intent ledger*. When a finding names
something you did on purpose — an ambiguity you are holding, a motif of
repetition, an unreliable narrator whose prose should not believe him — you
declare that choice once and the matching findings fall silent. Inner Socrates
and the Inner Editor write into the same ledger, so a declaration is a
first-class part of your examined-authorship record, not a per-tool mute.

#section("Inner Socrates — the dialectician")

The oldest member of the family, and its spine. Inner Socrates reads alongside
you and surfaces *questions* about your prose — the assumptions it treats as
given, the framings it presupposes, the tensions inside it, what each scene
does for the work. It makes exactly one claim, and never another: *you have
written something; here are the questions a careful reader would ask about it.*
If a surface would ever say "the prose should be X," it does not ship.

#screen(caption: "A first question, from the terminal")[```
$ inkhaven inner-socrates check --text \
    "The regent had to declare war; the council left no choice."

◇ Inquiry [Asserted Necessity]
  This passage treats an outcome as inevitable ("had to").
  What alternatives did you decide to leave out?

1 question(s) · persona: Inner Socrates
```]

That is the whole spirit: a question a careful reader would ask, in a persona's
voice, about a choice the prose made. There is no fix offered — only the
question, and the space it opens.

#subsection("The two tracks")

The *Fast track* is deterministic and instant, needs no provider, and runs in
all five languages. It has seven categories, each a pattern a careful reader
would catch on a first pass:

#screen(caption: "The seven Fast-track categories")[```
  Asserted Necessity  an outcome treated as inevitable
  Hedging             authorial hedging
  Pattern             a run of same-opening / same-length lines
  Speaker             dialogue running on with no speaker tag
  Length              a very long sentence
  Tense Shift         a slip between past and present (EN)
  Reference           a pronoun with two possible antecedents (EN)
```]

The *Slow track* reaches the questions patterns cannot. Over a paragraph it adds
five LLM categories — Hidden Assumption, Internal Tension, Framing,
Significance, Echo — and over a project *timeline* it adds three more
(Dramatization Gap, Implication, Temporal Density), reading the prose against
the events the timeline declares. You reach the Slow track with `--slow` on the
CLI or with the `E` sub-key in the hub; it is cost-capped and, in the editor,
runs in the background so it never blocks your typing.

#subsection("Notice, Inquiry, Probe")

Every Socratic finding carries one of three severities, and they are worth
knowing because they set what you see by default.

#term("Notice · Inquiry · Probe")[
  A *Notice* is a surface observation — hidden by default. An *Inquiry* is a
  question that invites reflection — the bulk of the output, and visible. A
  *Probe* is a rare, structural question about the work as a whole — always
  visible. The default visible threshold is *Inquiry*, so Inner Socrates is
  quiet unless it has a real question to raise.
]

#subsection("The persona roster")

The same paragraph reads differently to different readers, so Inner Socrates
reads *as* a chosen persona — a distinct careful-reader perspective whose
per-category emphasis weights scale what it notices (a weight of `0.0` mutes a
category outright). Fourteen ship bundled, in four groups.

Five read for *fiction*: Inner Socrates itself (the default), the Careful
Editor, the Skeptical Reader, the First-Time Reader, and the Slow Reader. Four
(added in the AUDIENCE work) read for *nonfiction* — the Skeptical Practitioner
for technical writing, the Domain Newcomer for general nonfiction, the Expert
Reviewer for academic prose, the End User for documentation; these mute the
narrative-only categories and lean instead on what a step assumes the reader
already knows. Three read *ideas-driven* work: the Dialectician for philosophy
(the unstated premise, the equivocation, the unanswered objection), the
Theological Reader for theology (deliberately non-empiricist — it probes
coherence and scope, not proof), and the Utopian Architect for utopia and
dystopia (a hybrid that reads the narrative as fiction while pressing on what
the imagined society assumes and what it costs).

#subsection("The two adversaries")

The last two break the family's spine on purpose. Inner Socrates is otherwise
*always* a neutral questioner — it never praises, never charges, only asks. The
two *verdict personas* are one-sided by design:

#two_track(
  [The *Defender* `⚖` is counsel for the defence. It states *only praise* — what
  works in the passage and why it is worth protecting. The steelman.],
  [The *Prosecutor* `⚖` is the prosecution. It states *only concern* — the
  charge, what fails, stated as an indictment. The devil's advocate.],
)

This is governed by a persona `stance`: `question` (the neutral default that
every other persona keeps), `praise` (the Defender), or `concern` (the
Prosecutor). Two things follow. The neutral interrogator is untouched — the
verdict prompt is reached only for a one-sided stance, so the twelve other
personas ask exactly the question they always did. And a verdict is *LLM-only*:
the deterministic Fast track can neither praise nor charge, so with a verdict
persona active the Fast chord simply points you to the Slow track (`E`). You can
give your *own* persona a stance by adding `stance: praise` (or `concern`) to
its HJSON file — the way to author a one-sided reader of your own.

#callout(label: "Authoring a persona")[
  A persona is a small HJSON file — an id, a name, a voice summary and notes, an
  optional `stance`, and a block of per-category `emphasis` weights. Drop it in
  `~/.config/inkhaven/personas/` to share it across projects or in the project's
  `books/intent/01-personas/` to keep it local; project wins over user wins over
  bundled. `Ctrl+B J → N` runs an AI wizard that scaffolds one for you.
]

#subsection("The intent ledger")

Some of what Inner Socrates flags is deliberate, and the *intent ledger* is how
you say so. It is the prose counterpart of the world simulator's magic ledger,
and it shares its whole vocabulary — an Entry with a Kind, a Coverage (which
categories it may suppress), and a Scope (project, chapter, a paragraph range, a
character, a scene, or a timeline range). A matching entry *suppresses* a
would-be finding with a note instead of nagging you again.

Entries accumulate two ways. You declare them by hand, or the *promotion
mechanism* offers one after you have dismissed the same kind of finding
repeatedly — `inner-socrates suggestions list` shows the patterns, and
`suggestions promote <category>` turns one into an entry. Carry a series' worth
of declared intentions into the next book with the `.isl` bundle
(`inner-socrates bundle export` / `bundle import`).

#subsection("The hub — Ctrl+B J")

In the editor the whole Socratic family lives on one chord. `Ctrl+B J` opens the
*Inner Socrates overview* — the active persona, the recent questions, and a way
into the ledger — and from there a single sub-key does each thing.

#screen(caption: "Ctrl+B J — the Inner Socrates overview")[```
┌─ Inner Socrates ◇ ─────────────────────────────────────┐
│ Persona: Inner Socrates (fiction) · stance: question   │
│ Ambient auto-check: off   (A toggles)                  │
│ Today: 4 / 150 slow calls  (cap informs, never blocks) │
│ Recent questions                                       │
│   ◇ Inquiry [Framing]   ch03 · the-quay                │
│   ◇ Inquiry [Hedging]   ch03 · the-inn                 │
├────────────────────────────────────────────────────────┤
│ F fast ¶ · E slow · S persona · C converse · N new     │
│ L ledger · A ambient · T theolog · P poet · R rigor    │
└────────────────────────────────────────────────────────┘
```]

#chord_table((
  chord_row("Ctrl+B J", "Open the Inner Socrates overview (persona, recent questions, ledger)."),
  chord_row("J → F", "Fast-check the open paragraph — deterministic, instant, free → Output."),
  chord_row("J → E", "Engage the Slow pass — the LLM deep questions or verdict, run in the background."),
  chord_row("J → S", "Cycle the active persona through the fourteen."),
  chord_row("J → C", "Open a conversation with the persona in the AI pane (it discusses, never rewrites)."),
  chord_row("J → R", "Run the deterministic reasoning-rigor reader over the open paragraph."),
  chord_row("J → N", "AI wizard to author a new persona."),
  chord_row("J → L", "View the intent ledger."),
  chord_row("J → A", "Toggle the ambient auto-check (off by default; runs on a writing pause)."),
))

Two of those sub-keys are the branch points to other family members: `J → T`
opens the Inner Theologian and `J → P` the Inner Poet, both covered below.
(`J → Y` opens the Inner Stylist, the voice-at-scale coach — that one belongs to
CHORUS and Chapter 18.) The hub letter is `J` because `Ctrl+B I` was already
book-info; think of `J` as the door to the readers who question.

#subsection("The command line")

Everything the hub does, and more, is scriptable under `inkhaven
inner-socrates`.

#screen(caption: "The Inner Socrates CLI")[```
inner-socrates check [--text "…" | --paragraph <uuid>
                      | --path <slug-path>] [--slow]
                     [--max-cost <n>] [--force]
inner-socrates timeline [--max-cost <n>] [--force]
inner-socrates ledger
inner-socrates persona list | show <id> | activate <id>
inner-socrates suggestions list | promote <cat> | dismiss <cat>
inner-socrates bundle export [--scope-level series|project|all]
inner-socrates bundle import <path> [--conflict skip|override]
```]

Target a paragraph three ways: `--text "…"` for literal prose, `--paragraph
<uuid>` for an explicit id, or `--path <slug-path>` for the convenient path
printed in the bracket by `inkhaven list` (for example
`manuscript/03-rain/01-opening`, order-prefixes tolerated). `check --slow` and
`timeline` are the two that call the provider; both are cost-capped and both
degrade to the Fast track — or to nothing — when no provider is set.

#section("Inner Editor — the reader of craft")

Where Inner Socrates asks about your *choices*, the Inner Editor observes your
*craft*. It reads the prose the way a thoughtful editor reading over your
shoulder would — a row of repeated words, a sentence whose rhythm earns its
weight, a register that shifts halfway through, a paragraph that insists on a
feeling its texture does not quite support — and it tells you what it *sees*,
not what to fix. It is LLM-only, works at paragraph scope, and it never rewrites
your prose.

#subsection("Praise, Note, Concern")

The Editor grades every observation on its own three-level scale — `Praise <
Note < Concern` — distinct from the Socratic Notice/Inquiry/Probe. The visible
threshold is *Note*, which means *Praise is hidden by default*. That is
deliberate: praise is a first-class output, but only when it is *earned* —
generic encouragement is forbidden by design, so a paragraph that genuinely does
something well earns a specific note you can reveal (`inner-editor findings list
--severity praise`), and everything else stays quiet.

It observes across eight categories in three modes — literary (Richness,
Tautology, Style, Style-instability), vocabulary (Dictionary richness), and a
few that cross the grain: the Belief stance (the subtle "does the prose believe
its own message?" reading), Craft praise, and Editorial suggestions. Every
observation grounds in actual textual evidence, and the Editor speaks in
observations and qualified suggestions — *I notice*, *you might consider*, *if
intentional* — never *should* or *must*.

#screen(caption: "Editor observations in the Output pane")[```
┌─ Output · inner-editor ────────────────────────────────┐
│ ✎ Note   ch01 · p007   Tautology                       │
│   "Restates one idea four ways — 'quiet hush', 'noise- │
│    less and without sound'. If the chant is meant, it  │
│    works; if not, you might thin it."                  │
│ ✎ Note   ch01 · p007   Belief                          │
│   "The prose protests the silence so strenuously the   │
│    texture argues against it. Ironic, or played        │
│    straight? If straight, is that intended?"           │
├────────────────────────────────────────────────────────┤
│ ↑↓ select · o expand · i intent · d dismiss            │
└────────────────────────────────────────────────────────┘
```]

#subsection("Reaching it — Ctrl+V O")

The Inner Editor lives under the *view* prefix, not the meta one — it is a way
of *looking* at your prose rather than acting on your book. `Ctrl+V O` (the `O`
is for *Observe*) opens its overview; from there four sub-keys do the work.

#chord_table((
  chord_row("Ctrl+V O", "Open the Inner Editor overview — status, tuning, today's tally."),
  chord_row("O → E", "Engage the open paragraph — one LLM pass → Output (Praise/Note/Concern)."),
  chord_row("O → C", "Open an Editor conversation about the paragraph (also F9 → Editor scope)."),
  chord_row("O → A", "Toggle the ambient auto-engage — reads on a writing pause (opt-in; LLM cost)."),
  chord_row("O → F", "Jump to the Editor's findings in the Output pane."),
))

The ambient auto-engage (`A`) is the Editor reading as you write: a pause on a
paragraph runs one engagement in the background. It is off by default because it
spends LLM calls, it has a same-paragraph cooldown, and any edit re-arms the
timer — so it never interrupts you mid-sentence. When an observation deserves
more than a glance, `C` turns the AI pane into the Editor itself, opening with
what it noticed and ready to discuss in the same non-prescriptive voice; it
helps you think, and will not draft the scene for you.

#subsection("The command line, and living together")

The CLI surface is `inkhaven inner-editor engage / findings / config / usage`,
plus `intent` (declare a category deliberate — it writes into the *same* ledger
Inner Socrates consults) and `suggestions` (the same dismissal-driven promotion
mechanism). Each engagement is one LLM call, tallied under the `inner_editor`
budget in `inkhaven cost` and `Ctrl+B $`; as everywhere in Inkhaven, the caps
inform and never block.

#callout(label: "Socrates and the Editor, side by side")[
  Both run on the same paragraph and render distinctly in Output — *purple* `◇`
  Socratic questions, *warm-earth* `✎` Editor observations. Filter Output to
  one with `f`, disable either independently, or run both. Socrates examines
  your *choices*; the Editor examines your *craft*. Together they are the
  texture half of Inkhaven's examined-authorship system.
]

#section("Inner Theologian — the moral reader")

Fiction carries moral weight whether or not the author chose it: a novel enacts
a theodicy, a view of what suffering means and what transformation costs, even
when no one sat down to pick one. The Inner Theologian is the family's third
axis — moral and theological *seriousness*, asking whether the work engages
honestly with the weight of what it depicts.

It is a *tradition-neutral comparativist*. It reads any manuscript through the
lenses of eleven moral and theological traditions — Catholic, Protestant,
Orthodox, Gnostic, LDS, Islam, Judaism, Hinduism, Buddhism, Confucianism, and
secular moral philosophy — *not to judge the work by any of them, but to ask
what each of them sees*. It belongs to no tradition, advocates none, and never
delivers a verdict. Everything it produces is a question, and it never edits
your prose. Crucially, it is *silent without a moral frame*: a passage with no
moral weight to read draws nothing, so it does not manufacture significance
where there is none.

#subsection("The two tracks")

The *Fast track* is deterministic and zero-AI: three `info` signals in the
Output `theologian` category (the `⚖` glyph), folded into the `Ctrl+B Shift+C`
review pass. *Moral invisibility* — a named character harms another with no
acknowledgment in the paragraphs that follow. *Consequence gap* — lethal or
severe violence with no depicted consequence. *Sacred levity* — sacred or ritual
vocabulary sharing a paragraph with comic markers (the most cautious of the
three, because comedy with sacred content is a legitimate style; it only flags
for attention). All five languages, tradition-neutral word lists, and none ever
rises above `info`.

The *Slow track* is the LLM session, and the reason to reach for the feature.
`Ctrl+B J → T` engages the open paragraph: the reader poses two or three
moral or theological questions through the lenses most illuminating for that
passage, in the book's language, always naming which tradition raises which —
and always inviting you to say a lens is irrelevant, because that too is useful
information. The questions land in the Output pane (and its longer reflective
text in the Thoughts pane).

#screen(caption: "A slow-track theological session")[```
⚖ theologian   ch07 · the-vigil
  Buddhist lens — the scene frames the loss as pure
  affliction from without. Does the narrative allow that
  the clinging is itself part of the suffering?

⚖ theologian   ch07 · the-vigil
  Secular moral philosophy — a duty is asserted ("he owed
  them this"). To whom is it owed, and what grounds it?
```]

#subsection("From the terminal")

The CLI is `inkhaven theologian scan / session / suppress`. `scan` runs the
fast-track signals (exit 1 if any, so it drops into a check pass); `session`
runs a slow-track session, defaulting to Category 6 — the work's implicit
theology — over the whole book. Narrow it with `--chapter`, pick a question
family with `--category 1-6` (moral weight, theodicy, redemption, the sacred,
duty, implicit theology), or restrict to one tradition with `--lens <code>`.

Before a session the reader grounds itself in what is already known — world
findings flagged with a theological dimension, character arcs that involve a
change of belief, any open fast-track signals in scope — and opens there;
each source degrades cleanly, none is required. Findings live in
`.inkhaven/inner_theologian.db`, and the `theologian:` config block tunes the
windows, the sub-budget, the disabled lenses, and the language (`enabled: false`
turns the whole feature off).

#section("Inner Poet — the reader of verse")

The Inner Poet is the family's specialist for verse. It *observes and measures*
a poem against its declared form — metre, rhyme, syllable counts, structural
completion — and, on the slow track, offers one LLM observation. It follows the
cardinal rule harder than any other member: *it never generates or rewrites a
line of verse*. You write the poem; it reads it. This section is the brief tour;
Chapter 22 gives poetry its full treatment.

Verse lives in the `para:verse-*` structural family, and a poem is measured
against a *declared form* — a `poem:` block beside the stanza. Open a verse
paragraph and press `Ctrl+B J → P` for the Inner Poet, with its own sub-keys:

#chord_table((
  chord_row("Ctrl+B J → P", "Open the Inner Poet on the open verse paragraph."),
  chord_row("P → F", "Fast-scan metre + rhyme against the declared form → Output. Deterministic, free."),
  chord_row("P → E", "Engage the LLM slow track — an observation on sound, caesura, the turn. Never a rewrite."),
  chord_row("P → D", "Declare a form — a picker that writes the language-localised poem: block."),
  chord_row("P → T", "The two-column translation view (source ∥ translation) + the Form/Sound trilemma."),
  chord_row("P → A", "Ambient — auto fast-scan each verse paragraph as you open it. Free."),
))

The Fast scan grades against the form in the same Praise / Note / Concern
vocabulary as the Editor, with the scansion drawn in glyphs — `/` stressed, `×`
unstressed, `·` flexible. While a verse paragraph is open the status bar shows a
live syllable readout (`♩ 8 syl · l2/4`) and the outline shows completion chips
(`8/14`, `14/14 ✓`). The CLI surface is `inkhaven poetry forms / syllabify /
metre / rhyme / scan / status / trilemma`; the whole of it, and the trilemma of
verse translation, is Chapter 22's subject.

#section("The reasoning-rigor reader")

The fifth member is the argument-side of the family. Where the Inner Socrates'
Dialectician asks the hard logical question by hand, and one at a time, the
reasoning-rigor reader (RIGOR) asks — deterministically, and at book scale —
whether the *arguments themselves hold*. It scans manuscript prose for
argument-rigor signals via language-keyed cue markers and surfaces each as an
advisory finding with the glyph `⊬`.

It is the most austere member: *deterministic and free* (no LLM, no network, no
persistence), *advisory* (a cue is a candidate weakness to weigh, never a
verdict), *multilingual*, and *stateless* — findings are recomputed each pass
from the manuscript files and emitted straight out. It reads every prose
paragraph under a user book in reading order, strips the Typst to plain text,
and matches against the language's cue tables, at most one finding per category
per paragraph so a dense passage never floods the pane.

#subsection("The six signals")

Each category is keyed to a conservative table of strong, rarely-innocent cues —
a false positive costs a glance, but noise erodes trust, so the lists stay lean.

#screen(caption: "The six reasoning-rigor categories")[```
false-dichotomy      a forced binary — "either … or", "the
                     only alternative", "one or the other"
question-begging     asserted as self-evident — "obviously",
                     "of course", "needless to say"
straw-man            a view dismissed — "so-called", "would
                     have us believe", "simplistic"
overgeneralization   an unqualified universal — "always",
                     "never", "without exception"
non-sequitur         a conclusion connective ("therefore",
                     "thus") with NO warrant ("because",
                     "since") anywhere in the paragraph
equivocation         a Glossary term with ≥2 senses used ≥2
                     times in a paragraph, sense unpinned
```]

Two of those deserve a note. `overgeneralization` deliberately fires only on the
strong absolutes — *always*, *never*, *without exception* — and *not* on the
innocent *all* or *every*, which do honest work far too often to flag.
`equivocation` is the one that needs more than prose: it projects from the
scholarly Glossary, watching multi-sense terms that carry `watch_equivocation`,
and with no such Glossary entries it is simply inert. Each finding carries a
localized advisory sentence that names the matched cue and poses the question
you should answer.

#subsection("The command line — and the CI gate")

RIGOR is reachable in the editor at `Ctrl+B J → R` — the deterministic
reasoning-rigor reader over the open paragraph. It can also join the
review-pass ambient surface (set `fast_track: true` in the `rigor:` config
block), but its fuller home is the command line.

#screen(caption: "Scanning for rigor, with the strict gate")[```
$ inkhaven rigor scan --strict

⊬ [ch.2 · overgeneralization]
  A universal claim ("never") — would a single counter-
  example break it? If so, qualify it.

⊬ [ch.5 · non-sequitur]
  "therefore" draws a conclusion, but no warrant ("because",
  "since") appears in the paragraph. What licenses the step?

2 signal(s) · exit 1   (--strict)
```]

`rigor scan` runs across one user book (`--book NAME` to pick, default the
resolved user book), `--signal CODE` filters to one category, and `--json` emits
a machine-readable array. By default the command *exits 0* — it is advisory, and
means only to inform. But `--strict` makes *any* signal a non-zero exit, which
turns the reader into a pre-submission gate you can wire into CI: a build that
fails when an unqualified universal or an unwarranted conclusion slips into the
manuscript. It is the same conservative reader either way; the flag only decides
whether it whispers or blocks the pipe.

#callout(label: "First-class in five languages")[
  RIGOR ships its own cue tables *and* its own advisory sentences in English,
  Russian, German, French, and Spanish — a Russian paragraph draws findings that
  name the matched Cyrillic cue. Matching is Unicode-aware and whole-word (so
  "art" never fires inside "start"). Spanish declares no clean *"either … or"*
  correlative, so false-dichotomy there rests on the forced-binary phrases
  alone — the reader never claims coverage it does not have.
]

#section("The family, at a glance")

Five readers, one promise. Here is where each one lives and what it costs to
run.

#screen(caption: "The Inner family — reach and track")[```
  Reader          Reach              Track     Glyph
  ─────────────── ────────────────── ───────── ─────
  Inner Socrates  Ctrl+B J           fast+slow  ◇
  Inner Editor    Ctrl+V O           slow only  ✎
  Inner Theologian Ctrl+B J → T      fast+slow  ⚖
  Inner Poet      Ctrl+B J → P       fast+slow  (P/N/C)
  Rigor reader    Ctrl+B J → R       fast only  ⊬
```]

Reach for the Fast tracks and RIGOR freely — they are deterministic, instant,
and cost nothing, and they run in all five languages. Spend on the Slow tracks
when you want the question a pattern cannot reach — a hidden assumption, a
theological weight, an observation on the turn of a sonnet — knowing every call
is capped, background, and degrades quietly when no provider is set. And whatever
you run, remember the one promise that binds the whole family: they read your
book and hand back questions. Not one of them writes a word of it.

#recap((
  [The *Inner family* is five readers that *question rather than answer* — Inner
  Socrates, Inner Editor, Inner Theologian, Inner Poet, and the reasoning-rigor
  reader — and every one is *advisory*: they surface findings, never edit prose.],
  [*Inner Socrates* asks about your choices — a deterministic *Fast track* (seven
  categories, five languages) and a cost-capped LLM *Slow track*; fourteen
  *personas* (fiction / nonfiction / ideas, plus the one-sided *Defender* and
  *Prosecutor*); the shared *intent ledger* suppresses what you chose on purpose.
  Hub: `Ctrl+B J` (`F` fast, `E` slow, `S` persona); CLI `inkhaven inner-socrates`.],
  [*Inner Editor* observes *craft* as *Praise / Note / Concern* — Praise hidden
  until earned — non-prescriptively, LLM-only. Reach: `Ctrl+V O` (`E` engage,
  `A` ambient, `F` findings); CLI `inkhaven inner-editor`.],
  [*Inner Theologian* reads moral weight through *eleven tradition lenses*,
  belonging to none and *silent without a moral frame*; `Ctrl+B J → T`, CLI
  `inkhaven theologian`. *Inner Poet* measures verse against a declared form and
  *never generates*; `Ctrl+B J → P`, CLI `inkhaven poetry` (Chapter 22).],
  [The *reasoning-rigor reader* deterministically flags six argument weaknesses
  (false dichotomy, question-begging, straw-man, overgeneralization,
  non-sequitur, equivocation) in five languages; `inkhaven rigor scan`, with
  `--strict` as a CI gate.],
))
