#import "../design.typ": *

#chapter(number: 22, title: "Poetry")

Every other reader in this book watches your prose. This one listens to your
verse — and it holds itself to a stricter discipline than any of them. Inkhaven
will scan a line for its metre, weigh a rhyme, count a haiku's syllables, and
tell you a sonnet is two lines short, but it will never write you a line, never
finish your couplet, never suggest a rhyme to close the stanza. It observes and
measures; it does not compose. That single refusal is the whole shape of the
feature, and it is worth stating before anything else, because it is what makes
the Inner Poet trustworthy: a tool that measures your verse is a mirror, and a
mirror that started painting would be worse than useless.

This chapter is the operator's tour — what verse *is* inside Inkhaven, how you
declare the form you are aiming at, how the Inner Poet reads a stanza against it,
and how the same machinery is reachable from a chord in the editor and from the
`inkhaven poetry` command in your shell. It is deliberately breadth-first. The
full treatment — the history of each form, the theory of scansion, the craft of
translating verse — lives in the companion volume, *Poetry with Inkhaven*, and
this chapter points you to it wherever the depth is more than an operator needs.

#callout(label: "The companion")[
  *Poetry with Inkhaven* is the deep dive: ten chapters on verse in the terminal,
  from the sound of a line to translating a stanza at the desk. This chapter
  tells you what the tools are and how to run them; the companion tells you what
  to do with what they show you.
]

#section("The iron rule — measure, never make")

Inkhaven's AI touches prose only under the strict advisory contract you met
earlier: it proposes, you confirm, nothing is written without your say-so. Verse
goes further still. The Inner Poet has *no write path at all* — not a confirmed
one, not a guarded one. It cannot be made to emit a line of poetry, because the
capability was never built. Its slow, LLM-driven track is prompted, in its own
system instruction, to observe and never rewrite; its fast track is pure
arithmetic over your syllables.

#term("The iron rule of verse")[
  The Inner Poet *observes and measures* — it never generates or rewrites a line
  of verse. You write the poem; it reads it. Where a prose tool might offer a
  rewrite for you to accept, the poetry tools offer only a reading: a scansion, a
  rhyme verdict, a completion count, an observation. The composing is yours.
]

The reason is not timidity but respect for the form. A metre is a decision made
across a whole poem; a rhyme is a bet on sound and sense at once; a substitution
in a pentameter line is often the point, not the error. A machine that "fixed"
these would flatten exactly the choices that make verse verse. So Inkhaven does
the one thing a machine does better than a poet — it *counts, exactly, without
tiring* — and leaves everything else to you.

#section("Verse is a paragraph family, not a project")

Poetry is not a separate kind of project, a special editor, or a branch of the
tree. It is a *family of structural paragraph subtypes*, each tagged `para:verse-*`,
exactly as code blocks are `para:code` and mathematics is `para:math`. A poem is
just a paragraph (or a run of them) that carries a verse tag. This has a quiet
but important consequence: a stanza can live *anywhere* — an epigraph atop a
chapter, a song a character sings, a recited fragment in the middle of a scene —
without changing the manuscript's structure, and a poetry-only project is simply
a book whose paragraphs happen to all be verse.

#term("The verse family")[
  Six `para:verse-*` subtypes, each with its own tree glyph: `verse-line` (‖),
  `verse-stanza` (♩), `verse-couplet` (‗), `verse-tercet` (⁚), `verse-quatrain`
  (⁛), and `verse-translation` (⇄). Any of them marks a paragraph as verse; the
  Inner Poet scopes on the whole family, and the prose readers skip it.
]

Because every verse tag begins `para:`, the prose companions — the Inner Editor,
the Inner Socrates, the narrative profiler — already know to leave it alone, and
its words are kept out of the prose word count. You do not wire any of that up;
tagging a paragraph as verse is enough to hand it to the Inner Poet and take it
away from everyone else. You add the next stanza of a poem without leaving the
writing flow: `Ctrl+B Shift+Y` creates a sibling verse paragraph of the same
subtype, immediately below and open for editing — structure only, never a
generated line.

#section("Declaring a form — the poem: block")

The Inner Poet measures your lines against a *target*, and you set that target by
declaring a form. A declared form is a small `poem:` block — an HJSON object
naming the metre, the foot count, the rhyme scheme, and any structural rules —
that sits beside the stanza as a sidecar. Everything the Inner Poet says is said
*relative to* this declaration: without one it can only observe free-verse
tendencies; with one it can tell you a line runs long, a rhyme fell to a
near-rhyme, or a villanelle's refrain has drifted.

You rarely write a `poem:` block by hand. Inkhaven ships a catalogue of eighteen
canonical forms, and you scaffold any of them — tuned for your project's
language — with one command.

#screen(caption: "A sonnet's declared form, printed by the forms library")[```
$ inkhaven poetry forms --form sonnet --language en

poem: {
  // 14 lines of iambic pentameter (generic English scheme)
  form: sonnet
  metre: iambic
  feet: 5
  metre_tradition: accentual_syllabic
  rhyme_scheme: "ABAB CDCD EFEF GG"
  language: en
}
```]

Run `inkhaven poetry forms` with no arguments and it lists every form with a
one-line description; add `--form <name>` to print that one's block, and
`--language en|ru|fr|de|es` to tune it. The tuning is not cosmetic. French verse
is *syllabic* — you count syllables and elide the mute *e* — so a French form
switches its tradition and sets `elide_mute_e`; Russian accentual-syllabic verse
allows the pyrrhic substitution and expects a final stress, so a Russian form
sets `allow_pyrrhic` and `require_final_stress`. The same sonnet, asked for in
Russian, comes back with the rules Russian prosody actually plays by.

#term("The declared form")[
  A `poem:` block — the target the Inner Poet measures against. Its fields:
  `form` (the canonical name), `metre` (`iambic`, `trochaic`, `anapestic`, …),
  `feet` (per line), `metre_tradition` (`accentual_syllabic`, `syllabic`,
  `free`), `rhyme_scheme` (label string like `ABAB CDCD EFEF GG`), and structural
  counts (`stanzas`, `lines_per_stanza`). Localising it retunes the rules.
]

To attach a form inside the editor rather than the shell, open the Inner Poet on
a verse paragraph and press `D` — a picker that writes the language-localised
`poem:` sidecar for you. And if none of the eighteen fits, `inkhaven poetry forms
--new --name my-form` prints a `custom` scaffold you edit and keep in
`.inkhaven/custom-forms.hjson`.

#section("The Inner Poet — the reader for verse")

The Inner Poet is the member of Inkhaven's inner-reader family that scopes on
verse. Like its siblings it works on two tracks: a *fast* track that is
deterministic, offline, and free, and a *slow* track that engages the LLM for an
observation. You reach both from the editor at `Ctrl+B J → P`, and the fast track
from the shell as `inkhaven poetry scan`.

#subsection("The fast track — metre and rhyme, counted")

The fast track scans a stanza against its declared form and reports what it finds
as a list of *findings*, each carrying one of three weights. Unusually for an
Inkhaven reader, it does not only warn — it also *praises*: a line that scans
cleanly earns a Praise finding, because in verse a thing done right is worth
naming.

#screen(caption: "The fast-track scan of a sonnet from the shell")[```
$ inkhaven poetry scan --text "$(cat sonnet.txt)" \
                       --form shakespearean_sonnet
♪ Praise   [Metre]  Line 1 scans cleanly as iambic pentameter.
♪ Note     [Rhyme]  Lines 12↔14 (G–G): "gone" / "dawn" —
           near-rhyme. Intended?
♪ Concern  [Metre]  Line 9 has 13 syllables; declared iambic
           allows 10 (±1 for a feminine ending). Long by 2.
```]

#term("Praise · Note · Concern")[
  The three weights of a fast-track finding. *Concern* is a real departure — a
  line long past tolerance, a rhyme pair that does not rhyme. *Note* is a
  question worth asking — a near-rhyme, a line that scans a touch short. *Praise*
  marks a line that meets its declared metre cleanly. A stanza with nothing to
  say draws silence, which is itself a verdict.
]

In the editor the same scan runs on `F`, and its findings land in the Output pane
under the `inner-poet` category, each jumpable to its line. Turn on *ambient*
mode with `A` and the fast scan re-runs automatically every time you open a verse
paragraph — free, uncapped, always current, so the reading is simply *there* as
you move through a poem. Findings and your suppressions persist per project in
`inner_poet.db`, so a finding you have chosen to live with stays quiet.

#subsection("The slow track — an observation, never a rewrite")

Press `E` to engage the slow track. It sends the stanza to your LLM under a
system prompt that pins it to the family's register — "I notice…", "you might
consider…", never "should", never "must" — and asks for a few observations on
what the verse is *doing*: whether the line breaks fall at phrase boundaries or
cut across the syntax (enjambment), the sound texture (alliteration, assonance,
consonance), and where the breath-pause falls (caesura). For a sonnet it also
asks whether a real turn — a volta — is present at the expected place, after line
eight in a Petrarchan sonnet, after line twelve in a Shakespearean one, noting
whether the turn is substantive, partial, or absent, without prescribing what it
ought to be. The observation streams into the Thoughts pane, the quiet reading
surface, and never proposes a replacement word.

#section("Scansion — reading the beats")

Underneath the fast track is a scanner that turns a line into a row of beats. It
syllabifies each word, marks the stressed syllable, and renders the result in
three single-width glyphs.

#term("The scansion glyphs")[
  `/` — a *stressed* syllable. `×` — an *unstressed* syllable. `·` — a *flexible*
  beat: a monosyllable that will promote or demote to fit the metre (English is
  full of them). An acute accent in the text (`compáre`) fixes a stress and
  overrides the rule — the way Russian verse is annotated.
]

You see the glyph row from `inkhaven poetry metre`, which scans one line, counts
its syllables, and — when you name a form — checks it against that form's declared
foot pattern.

#screen(caption: "Scanning one line, then checking it against a form")[```
$ inkhaven poetry metre \
    --line "The curfew tolls the knell of parting day" \
    --form blank_verse
  The curfew tolls the knell of parting day
  · / × · · · · / × ·   (10 syllables)
  → detected: iambic pentameter (fit 0.90)
  → declared iambic (5 feet): 10 of 10 syllables, fit 1.00
```]

The two "→" lines are two different questions. *Detected* asks the line what it
is, with no target in mind, reusing the same scansion engine the conlang tools
use — it may answer "irregular / free" for a line that fits no clean pattern.
*Declared* checks the line against the foot you asked for and reports the fit as a
fraction of the *fixed* (non-flexible) beats that land where the metre wants them,
so a line of mostly monosyllables can still score a clean fit. The scanner knows
about the two ways a line legitimately runs off its count: a *feminine ending*
(one extra unstressed syllable past the last foot, normal in iambic verse) and
*catalexis* (one syllable short) — both tolerated within ±1, so the Inner Poet
does not cry wolf over a perfectly good line.

#subsection("Exact English scansion — the phoneme dictionary")

English spelling lies about stress. The crude fallback rule stresses the
penultimate syllable, which mis-scans every final-stressed word in the language —
*compare*, *begin*, *above*, *return* all come out backwards. The fix is to
install a pronouncing dictionary in CMUdict format, which gives the scanner each
word's *exact* stress and syllable count.

#screen(caption: "Installing the pronouncing dictionary makes English exact")[```
$ inkhaven poetry phonemes import cmudict.dict
✓ installed 135000 words → …/inkhaven/phonemes/en.dict
  English metre + rhyme now use it (other languages are
  near-phonemic already).

$ inkhaven poetry phonemes status
♪ pronouncing dictionary: 135000 words installed
```]

Once installed, English metre *and* rhyme consult it: `compare` scans as the iamb
it is, and — crucially for rhyme — `love` / `move` is correctly told apart as an
*eye* rhyme rather than the false "perfect" that spelling alone would report. The
other project languages have regular enough orthographies that they are
near-phonemic already; Russian scansion, in particular, is exact without any
dictionary. An out-of-vocabulary English word falls back to the heuristic
unchanged, so the dictionary only ever helps.

#section("The forms and their metres")

The bundled catalogue holds eighteen forms, spanning the fixed forms, the open
forms, and the syllabic traditions. Each declares the metre and rhyme the Inner
Poet will hold a poem to.

#screen(caption: "The eighteen built-in forms")[```
  sonnet / petrarchan_sonnet / shakespearean_sonnet
      14 lines, iambic pentameter, the schemes differ
  haiku / senryuu / tanka        syllabic — 5-7-5, 5-7-7-…
  villanelle                     19 lines, two refrains
  pantoum                        interlocking quatrains
  terza_rima / ottava_rima       chained / 8-line stanzas
  ghazal                         couplets + a signature
  blank_verse / heroic_couplet   unrhymed / rhymed iambic
  limerick                       5 anapestic lines, AABBA
  ode / elegy                    elevated / lament, open
  free_verse / prose_poem        no metre — tendencies only
```]

Foot counts read out in the familiar length-names — *monometer*, *dimeter*,
*trimeter*, *tetrameter*, *pentameter*, *hexameter* — and the metre itself is one
of the standard feet: iamb (weak-strong), trochee (strong-weak), dactyl, anapest,
amphibrach, spondee. A syllabic form such as the haiku is counted, not scanned
for stress; a free form is only *observed* — the Inner Poet reports a free-verse
passage's line-length tendencies and flags a rhythm gone monotone, but passes no
metrical verdict where none was declared.

#section("Rhyme — quality and kind")

The rhyme engine classifies a pair of words on two axes at once. Its *quality* is
how true the rhyme is, and its *type* is where the stress falls in it.

#term("Rhyme quality and type")[
  *Quality*: `perfect` (a true rhyme), `near` (a slant rhyme — close but not
  exact), `eye` (matched spelling, mismatched sound — only detectable with the
  phoneme dictionary), `none`. *Type*: `masculine` (stress on the final
  syllable), `feminine` (penultimate), `dactylic` (antepenultimate).
]

#screen(caption: "Classifying a rhyme from the shell")[```
$ inkhaven poetry rhyme flower power
  flower / power: perfect feminine rhyme on "-ower"

$ inkhaven poetry rhyme love move
  love / move: eye-rhyme only
  (matched spelling, different vowel — install the phoneme
   dictionary to tell these apart)
```]

The engine is multilingual by construction. It works on the *rhyme tail* — from
the stressed vowel onward — with per-language normalisation: German final
consonant devoicing (so *Hund* rhymes with *bunt*), French mute-*e* elision,
light Russian vowel reduction, and Spanish *assonance*, where a vowel-only match
counts. This is exact for the regular orthographies — Russian above all — and for
English it climbs to exact the moment the pronouncing dictionary is installed. In
a fast-track scan, the engine walks your declared rhyme scheme, pairs up the lines
that share a label, and reports any pair that falls to a near-rhyme (a Note) or
fails to rhyme at all (a Concern), naming the two lines and the two words.

#section("Completion — the form still owed")

A fixed form is a promise about the *whole* poem, and the completion checker
tracks how much of that promise the draft has kept: a line ratio, plus any
structural component the form requires that the text is missing or breaking.

The checks are specific to each form's architecture. A *sonnet* owes fourteen
lines. A *villanelle* owes its two refrains on a strict schedule — line one
returns at lines six, twelve, and eighteen; line three at nine, fifteen, and
nineteen — and the checker names any slot where the refrain has drifted. A
*pantoum* interlocks: lines two and four of each quatrain must reappear as lines
one and three of the next. A *ghazal* traditionally signs its closing couplet —
the *maqta* — with the poet's name, declared as the form's `signature_word`, and
the checker looks for it. What the checker never does is fill the gap; it tells
you what the form still owes and leaves the paying of it to you.

You see this live while you write. The Outline shows a completion *chip* on each
poem — `8/14` in progress, `14/14 ✓` in green when a bounded form is exactly
complete (an over-long `16/14` is *not* ticked, because too many lines is its own
problem). And whenever a verse paragraph is open, the status bar carries a live
readout of the current line's syllable count and its position in the stanza.

#screen(caption: "The live verse readout in the status bar")[```
 2 Sonnets ▸ when-i-have-fears   ● 118w   ♩ 8 syl · l2/4
                                          └ this line ┘ └ line 2 of 4
```]

You reach the same completion report from the shell with `inkhaven poetry status
--text "…" --form villanelle`, which prints the ratio, the drafting/complete
state, and any structural issue as a list.

#section("The translator's trilemma")

Translating verse forces a choice no translation can escape, among three things a
poem is at once: its *Form* (the metre and rhyme), its *Meaning* (the sense), and
its *Sound* (the texture — the alliteration and assonance). You cannot keep all
three; every verse translation sacrifices something, and the interesting question
is *what*. Inkhaven's answer is not to resolve the trilemma — it has no resolution
— but to *make the trade visible* by scoring the two dimensions that are
measurable and prompting the LLM for the one that is not.

#term("The trilemma")[
  *Form* and *Sound* are scored deterministically: Form by scanning both texts for
  metre and rhyme preservation, Sound by comparing their alliterative density.
  *Meaning* is the AI's axis — engaged in the editor, it names which dimension a
  translation most preserved and which it most sacrificed. Nothing is judged; the
  trade is only shown.
]

A translation lives in its own subtype, `para:verse-translation`, whose body
holds the source above a delimiter line and the translation below it — either the
thematic `⇄` glyph or a plain rule of three-or-more dashes (`---`) you can
actually type. Press `T` on such a paragraph for the two-column view: source and
translation side by side, with the Form and Sound bars beneath.

#screen(caption: "The trilemma scored — Form and Sound measured, Meaning to the AI")[```
$ inkhaven poetry trilemma --source "$(cat src.txt)" \
    --translation "$(cat en.txt)" --form sonnet \
    --language ru --to-language en

Translation trilemma (ru → en):

  Form     ███████░░░   70%   3/4 lines keep the foot ·
                              2/3 source rhymes kept
  Meaning  ░░░░░░░░░░         (the AI axis — engage the
                              Inner Poet in the editor)
  Sound    ████████░░   84%   alliteration density
                              0.31 → 0.24
```]

Read the bars as a portrait of the compromise, not a grade. A translation that
scores high on Form and low on Sound kept the metre and let the music go; the
opposite kept the music and bent the shape. The Meaning bar is deliberately empty
in the shell — that axis is the LLM's, and it fills in the editor when you engage
the Inner Poet on the paired paragraph.

#section("At the desk — the chords")

Everything above is reachable without leaving the editor, through one branch of
the inner-reader menu. Open a verse paragraph, press `Ctrl+B J` for the inner
readers, then `P` for the Inner Poet, and the sub-keys fan out.

#chord_table((
  chord_row("Ctrl+B J → P → F", "Fast-scan metre + rhyme against the declared form → Output (Praise / Note / Concern). Deterministic, free."),
  chord_row("Ctrl+B J → P → E", "Engage the LLM slow track — an observation on enjambment, sound, caesura, and a sonnet's turn. Never a rewrite."),
  chord_row("Ctrl+B J → P → D", "Declare a form — a picker that writes the language-localised poem: sidecar."),
  chord_row("Ctrl+B J → P → T", "The two-column translation view: source ∥ translation, with the Form / Sound trilemma."),
  chord_row("Ctrl+B J → P → A", "Ambient — auto fast-scan each verse paragraph as you open it. Free, no cost cap."),
  chord_row("Ctrl+B Shift+Y", "Next stanza — create a sibling verse paragraph of the same subtype, open for editing. Structure only."),
))

The menu is pane-aware, so `P` opens the Inner Poet only when the open paragraph
is verse; on prose it tells you there is no verse here. The `J` branch is shared
with the Inner Poet's siblings — `T` for the Inner Theologian, `Y` for the Inner
Stylist — which is why the poetry key is `P`, one level in.

#section("From the shell — inkhaven poetry")

The same toolset is a command, for scripting, for a continuous-integration gate,
or simply for a quick reading without opening the project. Every subcommand takes
`--language en|ru|fr|de|es`, and several take `--json` for machine-readable output.

#chord_table((
  chord_row("poetry forms", "List the forms; --form prints one's block, --new scaffolds a custom one."),
  chord_row("poetry syllabify", "Show a word's or a --line's syllable boundaries and stress."),
  chord_row("poetry metre", "Scan a --line's beats, detect its metre, and check it against a --form."),
  chord_row("poetry rhyme", "Classify the rhyme between two words — quality and type."),
  chord_row("poetry scan", "The fast-track stanza scan against a --form. --fail-on-concern is the CI gate."),
  chord_row("poetry status", "A poem's completion against its --form — ratio and missing components."),
  chord_row("poetry trilemma", "Score a --source against its --translation on Form and Sound."),
  chord_row("poetry phonemes", "import / lookup / status — the English pronouncing dictionary."),
))

The `--fail-on-concern` flag on `poetry scan` is the one built for automation: it
exits non-zero the moment any Concern-level finding is present, so a poetry
manuscript can be gated in CI the same way a codebase is gated by its tests — the
build fails if a sonnet's metre has slipped. Nothing here writes to your
manuscript; the shell tools, like the editor's, only ever read.

#callout(label: "Where to go deeper")[
  This was the operator's tour. *Poetry with Inkhaven* takes each thread further:
  the sound of a line, the theory behind the scansion, the forms in their
  history, the craft of translating verse, and a closing chapter on writing a
  whole poem at the desk with the Inner Poet at your side. Reach for it when the
  question stops being "how do I run this" and becomes "what should I do with
  what it shows me".
]

#recap((
  [The *iron rule*: the Inner Poet *observes and measures* verse — metre, rhyme,
  syllables, completion — and *never* generates or rewrites a line. The composing
  is always yours.],
  [Verse is a *paragraph family* (`para:verse-*`), so a poem lives anywhere in a
  book; the prose readers skip it and it stays out of the word count.],
  [You *declare a form* in a `poem:` block — from `inkhaven poetry forms` or the
  editor's `D` picker — and everything the Inner Poet says is measured against it,
  retuned for your language.],
  [The *fast track* (`F`, `poetry scan`) counts metre and rhyme deterministically
  and reports *Praise / Note / Concern*; the *slow track* (`E`) offers a
  non-prescriptive LLM observation in the Thoughts pane.],
  [Scansion renders beats as `/` stressed, `×` unstressed, `·` flexible; a CMUdict
  *phoneme dictionary* makes English metre and rhyme exact (`love` / `move` as an
  eye-rhyme), and Russian is exact already.],
  [The *translator's trilemma* — Form vs Sound vs Meaning — is *scored, not
  resolved*: Form and Sound measured deterministically, Meaning left to the AI, in
  a two-column view (`T`) or `poetry trilemma`.],
  [Reach it all at `Ctrl+B J → P` in the editor or `inkhaven poetry` in the shell;
  the full craft is the *Poetry with Inkhaven* companion.],
))
