#import "../design.typ": *

#chapter(number: 11, title: "The poetry track")

Poetry is the track where the line stops being a container for prose and becomes the unit
of measure. Everything the other tracks treat as invisible machinery — the syllable, the
stress, the sound of a word against the next — is here the substance. And so the track's
tools are unlike any other's: they do not check your facts or grow your world, they *count*.
They scan a line and name its metre, weigh two endings for rhyme, hold a stanza to the form
you declared. This chapter is the loop for the sonnet, the villanelle, the free-verse
sequence — and, because Inkhaven serves the critic as readily as the poet, for writing
*about* verse as much as writing it.

#note[
  This is the one track with a companion volume of its own. #emph[Poetry with Inkhaven]
  covers the whole toolset in depth — scansion, rhyme across five languages, the translation
  trilemma, the editor workflow screen by screen. This chapter is the map; that book is the
  territory. Reach for it when you want more than the loop.
]

#section("Frame — declare the genre, then the form")

Poetry frames twice, and both matter.

First the genre, as every track does:

#config("inkhaven.hjson", [```hjson
genre: "poetry"
```])

`poetry`, `verse`, and `poem` share a frame, and setting it does something specific: it
tells the *prose* readers to stand down. Point a naive style checker at a line of verse and
it will flag the fragment, the inversion, the line break mid-clause as errors — which they
are, in an essay, and are the medium in a poem. Declaring the genre recalibrates the general
readers so they read for image and rhythm rather than grammar, and leaves metre and rhyme to
the reader built for them.

Then the deeper frame, unique to this track: the *form*. A stanza is only lines until you
declare what it is meant to be — a sonnet in iambic pentameter, a haiku in 5–7–5. You make
that declaration at the desk: open the Inner Poet (`Ctrl+B J`, then `P`) and press `D` to
pick a form; Inkhaven writes its `poem:` block, localised to your project's language, beside
the stanza. Only once a form is declared can anything be *measured against* it — the
declaration is what turns the passive ruler into an active second reader.

#insight[
  Every track tunes its readers; poetry is the one that also declares a *target*. \"Is this
  line the right length?\" has no answer until you have said what length is right. The form
  block is where you say it — and you write it, deliberately. Inkhaven never guesses your
  intent and never imposes a form. It measures only against the promise you chose to make.
]

#section("Gather — the ear and the tradition")

The gathering here is not a corpus of facts; it is the sound in your head and the forms you
answer to. Three tools serve it. The *forms library* (`inkhaven poetry forms`) offers
eighteen shapes — the sonnets, the villanelle, terza and ottava rima, the Japanese syllabics,
the ghazal, and open forms for when you want the tools without the corset — each ready to
declare and localised to all five languages. The *WordNet thesaurus* (`Ctrl+V Shift+Y` on a
word) reaches for the exact synonym, antonym, or broader term when the word you have is
nearly but not quite right — the poet's oldest need, met in place. And your own *ear*: read
the draft aloud, or let Inkhaven read it to you (`Ctrl+B S`), because a metrical fault you
cannot see you will always hear.

#section("Read — the Inner Poet")

This track has a reader of its own, and it is the heart of the loop. The *Inner Poet*
(`Ctrl+B J`, then `P`) reads a stanza and reports what it is doing prosodically — never
prescribing, never rewriting, only observing and measuring. It works two ways.

The *fast track* (`F`) is deterministic and instant: it scans every line's metre and rhyme
against the declared form and posts *findings* to the Output pane, each graded *Praise* (a
line that keeps its promise), *Note* (a departure worth seeing), or *Concern* (a promise
plainly broken — a declared rhyme that does not rhyme). The harshest word it will use is
Concern, and even that is a flag, never a fix. Turn on *ambient* (`A`) and it runs on every
stanza the moment you open it, free of charge, so a manuscript of poems each greets you with
its own reading. A finding you are breaking on purpose can be *suppressed* — silenced across
sessions, so the reader never nags you about a rule you have chosen to break.

The *slow track* (`E`) is reflective: it hands the stanza to a language model under a strict
observer's brief and returns prose to weigh — where an image strains, whether a sonnet's
argument turns at its volta — in the poem's own language. It never returns a rewrite.

#subsection("The counts that never sleep")

Two readings run without your asking. While a verse paragraph is open, the status bar shows
the *live syllable count* of the line your cursor is on (`♩ 8 syl · l2/4`) — is this line at
its ten, is this haiku line at its five — updated as you type. And the *Outline*
(`Ctrl+2`) shows every poem's *completion* as a chip: `8/14` while a sonnet drafts, `14/14 ✓`
when it is whole. You see the whole book's progress off one screen.

#pitfall[
  The honest limit, carried from the companion book: read from spelling alone, with no
  pronouncing dictionary, *English metre and rhyme are approximate*. A line packed with
  monosyllables scans as \"irregular\" because the engine will not invent stresses it cannot
  justify — mark the stressed syllables with an acute accent (`compáre`) and the scan
  resolves. Russian, whose spelling wears its sounds openly, is measured far more sharply. If
  a scansion looks wrong in English, it may be the language, not your line — trust your ear,
  and mark the stress.
]

#section("Produce")

`export pdf|epub|docx` renders the collection. Verse paragraphs preserve every line break you
typed — Inkhaven never reflows a poem the way it reflows prose — and the `poem:` blocks travel
with their stanzas as invisible sidecars, so the manuscript stays measurable at every later
pass. A book of translations built from `para:verse-translation` paragraphs is a parallel text
by construction, source and rendering kept together.

#tryit[
  Run one small loop end to end. In the Tree, make a paragraph and cycle its type to
  #emph[verse stanza] (`t`). Declare a form on it — `Ctrl+B J` → `P` → `D`, pick #emph[haiku].
  Type three lines, watching the `♩ N syl` chip reach 5, 7, 5. Then press `F` and read what
  the Inner Poet says. You have just done, in miniature, everything the poetry track is.
]

#section("Hands-on: three procedures")

#subsection("Write a poem at the desk")

+ In the Tree, add a paragraph and give it a verse type: press `i` and choose #emph[verse stanza], or cycle an existing paragraph with `t` until the `♩` glyph appears.
+ Declare its form: `Ctrl+B J`, then `P`, then `D`; pick a form and Enter. Inkhaven writes the localised `poem:` sidecar beside the stanza.
+ Write the lines, watching the status bar's live syllable count. Grow the poem a stanza at a time with `Ctrl+B Shift+Y` — it makes the next verse sibling and opens it.
+ Read it back: `Ctrl+B J` → `P` → `F` for the instant metre/rhyme findings, or `E` for an AI reading. Turn on ambient (`A`) to have every stanza scanned as you move.

#subsection("Scan and analyse from the terminal")

+ Scan a single line and name its metre: `inkhaven poetry metre --line "…" --language en`. Add `--form sonnet` to check it against a declared metre.
+ Weigh a rhyme: `inkhaven poetry rhyme дом том --language ru` — quality, type, and the shared tail, normalised by the language's own rules.
+ Check a whole stanza and gate a manuscript in CI: `inkhaven poetry scan --form villanelle --text "…" --fail-on-concern` exits non-zero if a poem has drifted from its form.

#subsection("Review a translation")

+ In a `para:verse-translation` paragraph, put the source above a `---` line and the translation below it.
+ Open the two-column view: `Ctrl+B J`, then `P`, then `T`. Source and translation sit side by side, with the Form/Sound trilemma measured beneath.
+ For the Meaning axis — which no ruler measures — press `E` to engage the Inner Poet on the pair.

#recap((
  [Poetry frames *twice*: `genre: "poetry"` stands the prose readers down (line breaks and
   fragments are the medium, not errors), and the declared *form* (`Ctrl+B J → P → D`) gives
   the tools a target to measure against.],
  [*Gather* the ear and the tradition — the forms library, the WordNet thesaurus for the exact
   word, and reading aloud (`Ctrl+B S`).],
  [*Read* with the *Inner Poet* (`Ctrl+B J → P`): `F` for instant metre/rhyme findings
   (Praise / Note / Concern), `E` for an AI reading, `A` for ambient scanning; suppress a rule
   you break on purpose. The live syllable chip and the Outline's completion chips count without
   being asked.],
  [English scansion is approximate (mark stress with an acute accent); Russian is measured
   sharply — trust your ear at the seams.],
  [The dedicated companion volume, #emph[Poetry with Inkhaven], covers all of it in depth.],
))
