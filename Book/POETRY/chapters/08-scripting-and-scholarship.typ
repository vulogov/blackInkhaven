#import "../design.typ": *

#chapter(number: 8, title: "Scripting and scholarship")

Every chapter so far measured one poem at a time. But a scholar rarely has one poem — she
has an anthology, a poet's collected works, a corpus of a hundred sonnets she wants to
characterise all at once. And a working poet assembling a manuscript wants a gate: a check
that every poem in the book is finished before it goes to the printer. Both needs are the
same need — the measurements of this book, applied in bulk and read by a machine. This
final chapter is about the two doors Inkhaven opens for that: structured output on the
command line, and the poetry engines exposed to Bund, Inkhaven's scripting language.

#section("Machine-readable output: --json")

Every reporting command in this book has a `--json` flag, and it turns a human readout into
data a program can consume. Where `poetry scan` prints `♪` marks for a person to read,
`poetry scan --json` prints a structure for a script to parse:

```
$ inkhaven poetry scan --form heroic_couplet --language en --json \
    --text "The trees are green in summer and in spring
            The autumn leaves will fall upon the ground"
{
  "form": "heroic_couplet",
  "language": "en",
  "concerns": 1,
  "findings": [
    { "severity": "praise",  "kind": "Metre", "line": 1,
      "message": "Line 1 scans cleanly as iambic pentameter." },
    { "severity": "praise",  "kind": "Metre", "line": 2,
      "message": "Line 2 scans cleanly as iambic pentameter." },
    { "severity": "concern", "kind": "Rhyme", "line": 2,
      "message": "Lines 1↔2 (A–A): “spring” / “ground” — do not rhyme." }
  ]
}
```

The same findings you saw in Chapter 5, now with a `concerns` count at the top and every
finding a record with `severity`, `kind`, `line`, and `message`. `poetry status --json` and
`poetry rhyme --json` do the same for their reports. Pipe any of them through a JSON tool
and you can tabulate the metres of an anthology, count the rhyme types across a poet's
career, or list every poem in a manuscript that is still `drafting` — the close reading of
Chapters 3 and 4, run over a library instead of a line.

#subsection("A gate for the manuscript: --fail-on-concern")

One flag turns measurement into enforcement. `poetry scan --fail-on-concern` exits with a
non-zero status the moment a stanza raises a Concern:

```
$ inkhaven poetry scan --form heroic_couplet --fail-on-concern \
    --text "The trees are green in summer and in spring
            The autumn leaves will fall upon the ground"
♪ Praise   [Metre]  Line 1 scans cleanly as iambic pentameter.
♪ Praise   [Metre]  Line 2 scans cleanly as iambic pentameter.
♪ Concern  [Rhyme]  Lines 1↔2 (A–A): “spring” / “ground” — do not rhyme.
$ echo $?
1
```

That exit code is all a continuous-integration job needs. Put `poetry scan
--fail-on-concern` in the build of a book of formal verse, and the build fails if any poem
has drifted from its declared form — a rhyme that stopped rhyming after an edit, a line that
lost its metre in revision. The poet's promise, checked automatically, on every commit.
Note the restraint in the threshold: only a *Concern* fails the build, never a mere *Note*.
A line two syllables short is your business; a declared rhyme that does not rhyme is a
broken promise worth stopping for. The gate enforces only what the poet plainly meant.

#callout(label: "The permissive principle, one last time")[
  A build gate could have been a nag — failing on every Note, forbidding an off-metre line.
  It is not. `--fail-on-concern` is *opt-in* (the default scan never sets an exit code), and
  even when set it stops only for Concerns. Inkhaven measures freely and blocks rarely; the
  cost of a rough line is information, not obstruction. You choose to install the gate, and
  you choose how high to hang it.
]

#section("The engines in Bund")

Inkhaven's scripting language, Bund, is a concatenative stack language for driving the
editor from code, and the poetry engines are exposed to it as a family of `ink.poem.*`
words (each also answering to the shorter `poem.*`). Where `--json` gives you the
*commands'* output, Bund gives you the *engines* directly, to compose into analyses the
fixed commands do not offer. Try them with `inkhaven bund`:

```
$ inkhaven bund '"extensive" "en" ink.poem.syllable_count'
3

$ inkhaven bund '"day" "may" "en" ink.poem.rhyme'
{ "shared": "ay", "quality": "perfect", "type": "masculine" }

$ inkhaven bund '"Мой дядя самых честных правил" "ru" poem.scan_line'
{ "pattern": "·/×/×/×/×", "syllables": 9, "metre": "iambic tetrameter" }
```

Four words cover the layer's measurements: `syllable_count` (a word and its language →
a count), `scan_line` (a line and its language → its scansion), `rhyme` (two words and a
language → a rhyme analysis), and `status` (a text, a form, and a language → a completion
report). Each takes its arguments off the stack and pushes back a plain value or a
dictionary, ready to feed the next word. A Bund script can walk every paragraph of a book,
scan the verse ones, and accumulate a table of metres — the whole anthology characterised
in a loop, using the identical engines this book has described a command at a time.

#subsection("Read-only by default, and by category")

Every `ink.poem.*` word is classified `store_read` in Inkhaven's scripting policy — the
permission category for words that *look at* the project without changing it. This is not a
footnote; it is a guarantee. The poetry engines *cannot* alter your manuscript, because the
category system will not let a read-word write. A script that mis-scans a thousand poems has
done nothing but compute; your text is untouched, by construction, at the level of the
permission model. It is the observe-never-write rule of the whole poetry layer, enforced not
by good intentions but by the same machinery that guards every other read-only word in the
language.

#callout(label: "Does it work in Russian? All the way down")[
  The multilingual promise reaches even here. Every Bund word takes a language argument, and
  every one keys its behaviour off it — `poem.scan_line` reads a Russian line by Russian
  prosody, `poem.rhyme` grades a German pair by German devoicing. The scripting layer is not
  an English convenience with the other languages bolted on; it is the same five-language
  engine, addressable from code. A scholar of Russian verse scripts against it exactly as a
  scholar of English does.
]

#section("Where the workbench ends")

We have come the whole way — from a single syllable, through the scanned line and the graded
rhyme, to the Inner Poet's reading, the completeness of a form, the trilemma of translation,
and now a whole corpus measured in a loop. At every step the tool did the same thing and
refused the same thing: it counted, classified, and reported; it never wrote a word of your
poem, never told you a line was good or bad, never crossed from measurement into
composition. That refusal is not a missing feature. It is the feature — the reason the
workbench can be trusted with your verse and with your argument about someone else's. The
ruler does not hold the pen. You do.

#recap((
  [Every reporting command takes `--json` — structured output (findings with severity /
   kind / line / message, a `concerns` count, completion reports) for tabulating an
   anthology or auditing a manuscript in bulk.],
  [`poetry scan --fail-on-concern` gives a non-zero exit on any Concern — a CI gate that
   fails a build when a poem drifts from its declared form. Opt-in, and it stops only for
   Concerns, never Notes.],
  [The engines are exposed to Bund as `ink.poem.*` (short: `poem.*`) — `syllable_count`,
   `scan_line`, `rhyme`, `status` — composable words for analyses the fixed commands do not
   offer, over a whole book in a loop.],
  [Every poem word is `store_read` in the scripting policy: the permission model *forbids*
   it from altering your text — the observe-never-write rule enforced by machinery, not
   intention. And every word takes a language, so the scripting layer is fully multilingual.],
  [Across the whole workbench the discipline never broke: measure and report, never compose.
   The ruler does not hold the pen.],
))
