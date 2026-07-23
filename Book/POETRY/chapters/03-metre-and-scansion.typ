#import "../design.typ": *

#chapter(number: 3, title: "Metre and scansion")

Scansion is the act of marking a line's rhythm — showing which syllables are stressed and
which are not, and reading a pattern off the result. It is the oldest analytic tool in
poetry, older than the printed page, and it is where a line stops being words and becomes
*measure*. This chapter is about `poetry metre`, the command that scans a line, names the
foot it hears, and — if you have declared a form — tells you how well the line keeps the
promise the form made.

#section("The vocabulary of the foot")

A #emph[foot] is the small repeating unit of a metre: a fixed little pattern of stressed
and unstressed syllables that the line stacks end to end. English and German verse is
#emph[accentual-syllabic] — it counts both the syllables and the stresses — and its feet
have names two and a half thousand years old:

#table(
  columns: (auto, auto, 1fr),
  stroke: none,
  inset: (x: 6pt, y: 3pt),
  align: (left, left, left),
  table.header(
    text(weight: "bold", size: 9pt)[Foot],
    text(weight: "bold", size: 9pt)[Pattern],
    text(weight: "bold", size: 9pt)[Example],
  ),
  table.hline(stroke: 0.5pt + ink_rule),
  [iamb],    [unstressed–stressed],           [_be·*low*_ — the heartbeat of English verse],
  [trochee], [stressed–unstressed],           [_*gar*·den_ — falling, insistent],
  [anapaest],[unstressed–unstressed–stressed],[_in·ter·*vene*_ — galloping, the limerick's foot],
  [dactyl],  [stressed–unstressed–unstressed],[_*mer*·ri·ly_ — falling triple time],
)

#v(2mm)

Stack five iambs and you have #emph[iambic pentameter], the ten-syllable line of
Shakespeare and Milton — _penta_ (five) _meter_ (measure). Four of them is tetrameter,
the ballad and Pushkin's line; three, trimeter. The metre of a form is a foot and a
count: `iambic` × 5.

#term("Foot")[
  The repeating rhythmic unit of a metre — iamb, trochee, anapaest, dactyl — each a fixed
  pattern of stressed and unstressed syllables. The number of feet in a line gives the
  line its length-name: pentameter (5), tetrameter (4), trimeter (3).
]

#section("Scanning a line")

Hand `poetry metre` a clean line and it scans it. We start in Russian, with the opening of
#emph[Eugene Onegin] — because, as Chapter 2 warned, Russian is where the scanner is at its
sharpest:

```
$ inkhaven poetry metre --line "Мой дядя самых честных правил" --language ru
  Мой дядя самых честных правил
  · / × / × / × / ×   (9 syllables)
  → detected: iambic tetrameter (fit 1.00)
```

The middle row is the scan, and it uses *three* marks, not two:

#table(
  columns: (auto, auto, 1fr),
  stroke: none,
  inset: (x: 6pt, y: 3pt),
  align: (center, left, left),
  table.header(
    text(weight: "bold", size: 9pt)[Mark],
    text(weight: "bold", size: 9pt)[Name],
    text(weight: "bold", size: 9pt)[Meaning],
  ),
  table.hline(stroke: 0.5pt + ink_rule),
  [`/`], [stressed],  [a syllable the scanner is confident carries stress],
  [`×`], [unstressed], [a syllable it is confident does *not*],
  [`·`], [flexible],  [a monosyllable, or a syllable whose stress depends on context —
                       the scanner declines to fix it alone],
)

#v(2mm)

Read the Russian line: `Мой` is a monosyllable (`·`, flexible), and then the four
polysyllables — `дя́дя`, `са́мых`, `че́стных`, `пра́вил` — each lay down a clean strong–weak
pair (`/ ×`). The result is a textbook iambic tetrameter, and Inkhaven names it with a
*conformance* of 1.00.

#subsection("Conformance is the whole idea")

That number is the soul of the poetry layer, so look at it closely. Conformance is *how
closely the line matches the pattern* — 1.00 is a perfect fit; lower numbers mean the
rhythm departs from the ideal. And departure is not error. The best lines are rarely the
ones that score 1.00; a great poet leans on the metre rather than marching to it, and a
lower fit may be exactly the felicity that makes the line live.

Inkhaven reports the number and says *nothing* about whether it is good. A 0.75 might be a
masterstroke of variation or a line that has lost its footing; the tool cannot tell, and
does not try. It measures the distance from the ideal; judging that distance is the poet's
and the critic's work, never the machine's.

#callout(label: "The metre engine is shared, and battle-tested")[
  The scansion here is not new code written for poetry. It is the same metre-detection
  engine Inkhaven's ConLang suite uses to analyse the prosody of invented languages,
  pointed now at real verse. Rhythm is rhythm whether the language was born or built —
  reusing the engine means the poetry layer inherited a mature, well-exercised scanner the
  day it shipped, rather than a fresh one full of fresh bugs.
]

#section("Why English fights back — and the accent mark")

Now the honest part. Give the scanner the most famous line of iambic pentameter in
English, unmarked, and watch it stumble:

```
$ inkhaven poetry metre --line "Shall I compare thee to a summer's day"
  Shall I compare thee to a summer's day
  · · / × · · · / × ·   (10 syllables)
  → detected: amphibrachic trimeter (fit 0.75)
```

Ten syllables, correctly counted — but a row full of `·`, and a nonsense verdict of
\"amphibrachic trimeter.\" What went wrong? Nothing, in fact: the scanner is being
*principled*. Six of the line's eight words are monosyllables — #emph[Shall], #emph[I],
#emph[thee], #emph[to], #emph[a], #emph[day] — and a monosyllable's stress is not a
property of the word but of the sentence around it. The scanner refuses to invent stresses
it cannot justify, so it marks those syllables flexible and finds no firm pattern to lock
onto. This is the same limitation Chapter 2 named: read from spelling alone, with no
pronouncing dictionary and no syntax, English rhythm is genuinely underdetermined.

And here is the recourse, the same one Chapter 2 offered: *mark the stresses you mean.*
Put an acute accent on the syllables that carry the beat, and the flexible marks resolve:

```
$ inkhaven poetry metre --line "Shall I compáre thee to a súmmers day"
  Shall I compáre thee to a súmmers day
  · · × / · · · / × ·   (10 syllables)
  → detected: iambic pentameter (fit 1.00)
```

Two accents — on #emph[compáre] and #emph[súmmers] — were enough to give the scanner its
anchors, and it now hears the pentameter perfectly, fit 1.00. The lesson is the book's
recurring one: in Russian the machinery just works; in English you sometimes have to tell
the machine what your ear already knows, and the accent mark is how you say it.

#section("Scanning against a declared form")

Detection is what the critic wants — hand it an unknown line, hear its metre named.
Checking is what the poet wants — *I said this would be iambic pentameter; is it?* Add
`--form` and `poetry metre` measures the line against the form's declared metre rather than
against whatever pattern it happens to resemble:

```
$ inkhaven poetry metre --line "Shall I compáre thee to a súmmers day" --form sonnet
  Shall I compáre thee to a súmmers day
  · · × / · · · / × ·   (10 syllables)
  → detected: iambic pentameter (fit 1.00)
  → declared iambic (5 feet): 10 of 10 syllables, fit 1.00
```

The last line is the judgement against the *promise*: ten syllables of the ten the form
asks for, a fit of 1.00 against the declared iamb. Feed the same command the *unmarked*
line and that final fit drops to 0.50 — not because the poetry changed but because the
evidence did. If you write an eleven-syllable line, Inkhaven reports it too — and may tag
it, which brings us to the two most useful marks the scanner adds.

#subsection("Feminine endings and catalexis")

Two departures are so common, and so meaningful, that the scanner names them:

- A *feminine ending* is an extra unstressed syllable hanging off the end of a line — an
  eleventh syllable on a pentameter line. \"To be or not to be, that is the question\"
  ends on the unstressed `-tion`: a feminine ending. It is not a mistake; in Russian verse
  it is structural, alternating with masculine endings by rule. Inkhaven tags it
  `· feminine ending` rather than docking the line for being long.

- A *catalectic* line is one syllable *short* — a foot with its last syllable clipped off,
  a favourite trochaic trick for ending a line on a strong beat. Inkhaven tags it
  `· catalectic (one short)`.

Both tags are the tool distinguishing *deliberate metrical shapes* from *ragged counting*.
An eleven-syllable line is not simply \"wrong by one\"; it may be a feminine ending doing
exactly its job. Naming the shape is more useful — and more honest — than a bare error.

#term("Feminine ending")[
  A line ending on an extra unstressed syllable, beyond the metre's nominal count (e.g. an
  eleventh syllable on an iambic-pentameter line). Its opposite, ending on a stress, is a
  #emph[masculine ending]. In Russian verse the two alternate by convention; in English
  they are a source of variation.
]

#section("The three traditions of measure")

Not every poetry counts the way English does, and a tool that only knew accentual-syllabic
verse would mismeasure most of the world's poetry. The `metre_tradition` field in a poem
block tells Inkhaven which system to apply:

- *Accentual-syllabic* (English, German) — count both syllables and stresses. The default,
  and everything above.
- *Syllabic* (French, and the Japanese forms) — count *syllables only*; stress is not
  metrical. A French alexandrine is twelve syllables, full stop; a haiku is 5–7–5 syllables
  and cares nothing for which are stressed. Inkhaven measures the count and ignores the
  stress, as the tradition demands.
- *Accentual* (Old English, much folk and rap verse) — count *stresses only*; the number
  of unstressed syllables between them is free. A four-beat line is four strong beats
  however many weak ones it carries.

Declaring the tradition is not a formality — it changes what \"keeping the metre\" *means*.
Score a French alexandrine as if it were accentual-syllabic and you will find fault where
there is none, because you are measuring a thing the tradition never promised. The
multilingual promise of the workbench runs all the way down to this: each language's verse
is measured by its own tradition's rules, not English's.

#callout(label: "Free verse is measured too, not excused")[
  Verse with no regular metre is not beyond measurement — it is measured *differently*.
  Inkhaven can profile free verse by its rhythm of line lengths and stress density, the way
  a critic describes the cadence of Whitman or a psalm without scanning feet. Absence of
  metre is a positive style, and the workbench reports its shape rather than shrugging.
]

#recap((
  [A #emph[foot] is a metre's repeating unit — iamb, trochee, anapaest, dactyl; its count
   names the line — pentameter, tetrameter, trimeter.],
  [`poetry metre` scans a line into three marks — `/` stressed, `×` unstressed, `·`
   flexible (a monosyllable or context-dependent syllable) — names the metre, and reports
   a #emph[conformance], the distance from the metrical ideal. Departure is variation, not
   error; the tool measures it and judges it not.],
  [In Russian the scan is sharp (Onegin's line reads iambic tetrameter, fit 1.00,
   unmarked). English monosyllabic lines read \"irregular\" because the scanner *principledly*
   won't invent stresses — mark the beats with an acute accent and the pattern resolves.],
  [Add `--form` to check a line against a *declared* metre rather than a guessed one — the
   poet's question, not the critic's.],
  [The scanner names #emph[feminine endings] (an extra unstressed syllable) and
   #emph[catalectic] lines (one short), distinguishing deliberate metrical shapes from
   ragged counting.],
  [Three traditions — accentual-syllabic, syllabic, accentual — measure different things;
   the `metre_tradition` field picks the right one, so French and Japanese verse are
   measured by their own rules. Even free verse gets a profile.],
))
