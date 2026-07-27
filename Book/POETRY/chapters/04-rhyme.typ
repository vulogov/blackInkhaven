#import "../design.typ": *

#chapter(number: 4, title: "Rhyme")

Rhyme looks simple — two words that end alike — and is not. Whether two words rhyme
depends on where their stress falls, on sounds the spelling hides, and on which
language's ear is listening: a pair that rhymes richly in Russian may barely chime in
English, and a pair the eye accepts the ear rejects. `poetry rhyme` is the tool that
takes two words and tells you not just *whether* they rhyme but *how* — its quality, its
type, and the tail they share.

#section("Asking whether two words rhyme")

```
$ inkhaven poetry rhyme day may
  day / may: perfect masculine rhyme on "-ay"
```

Three judgements again, as with syllables. The rhyme's *quality* is perfect; its *type*
is masculine; and the *shared tail* — the stretch of sound the two words hold in common —
is `-ay`. When two words do not rhyme, it says so plainly:

```
$ inkhaven poetry rhyme day tree
  day / tree: no rhyme
```

#section("The four qualities of rhyme")

Inkhaven grades a rhyme on a four-step scale, from a true chime down to a mere agreement
of spelling:

#table(
  columns: (auto, 1fr),
  stroke: none,
  inset: (x: 6pt, y: 3pt),
  align: (left, left),
  table.header(
    text(weight: "bold", size: 9pt)[Quality],
    text(weight: "bold", size: 9pt)[What it means],
  ),
  table.hline(stroke: 0.5pt + ink_rule),
  [*Perfect*], [the sounds from the stressed vowel onward agree completely — _day / may_,
                _light / night_. The rhyme the ear wants.],
  [*Near*],    [the tails are close but not identical — a slant or half rhyme, _bind / mind_,
                _shape / keep_. The workhorse of modern verse.],
  [*Eye*],     [the words *look* like they should rhyme but do not *sound* alike — _love /
                move_, _cough / bough_. A rhyme for the eye, betrayed by the ear.],
  [*None*],    [no meaningful agreement of sound or spelling.],
)

#v(2mm)

The distinction between *near* and *eye* is the subtle one, and it is where a tool reading
plain text reaches its limit. A near rhyme is close in *sound*; an eye rhyme is close only
in *spelling*. Telling them apart requires knowing how the words are pronounced — which,
in English, spelling does not reliably reveal. Watch the limit bite:

```
$ inkhaven poetry rhyme love move
  love / move: perfect masculine rhyme on “-ove”
```

The tool calls it *perfect* — because, reading the spelling, it sees the identical tail
`-ove` and has no way to know that \"love\" and \"move\" part company in the mouth. This is
a textbook eye rhyme, and Inkhaven has confidently mis-graded it. The failure is not
sloppiness; it is the honest consequence of judging sound from spelling in a language whose
spelling lies.

#callout(label: "The English eye-rhyme problem — and the fix")[
  In a language whose spelling matched its sound, eye rhyme would not exist as a separate
  category — if it looks alike it sounds alike. English is not that language. \"Love\" and
  \"move\" share four letters and rhyme for the eye alone; \"cough,\" \"bough,\" \"through,\"
  and \"though\" all end in `-ough` and no two of them rhyme. Read from spelling *alone*,
  Inkhaven cannot reliably tell a real rhyme from an eye rhyme — as the `love / move` verdict
  above shows, it stamps an eye rhyme \"perfect.\"

  Install a *pronouncing dictionary* and this closes: `inkhaven poetry phonemes import
  <cmudict>` gives the rhyme engine each word's actual phonemes, after which `love / move` is
  correctly reported as an *eye masculine rhyme* (\"looks alike, sounds different\") and a
  true pair like `day / may` stays perfect. It is English-specific — the other four languages'
  spelling already tracks their sound — and words outside the dictionary fall back to the
  orthographic reading, so it never breaks. Even so, where the argument is delicate, trust
  your own ear.
]

#section("Masculine, feminine, dactylic")

A rhyme's *type* is about where the stress sits relative to the rhyming sound — how many
syllables the chime spans:

- *Masculine* — the rhyme falls on the final, stressed syllable: _be*low* / a*glow*_.
  Single, hard, closing.
- *Feminine* — a stressed syllable followed by an unstressed one, both rhyming:
  _*moth*·er / *broth*·er_. Softer, a falling close.
- *Dactylic* — the rhyme spans three syllables, stressed–unstressed–unstressed:
  _*quiv*·er·ing / *shiv*·er·ing_. Rare and virtuosic in English; ordinary in Russian.

The type is not decoration — it is structural. Russian verse alternates masculine and
feminine rhymes by strict convention, and a form's rhyme scheme often implies its rhyme
types. Naming the type lets the completion tools of Chapter 6 check that a poem's rhymes
fall in the pattern its tradition demands.

#term("Rhyme tail")[
  The stretch of a word from its stressed vowel to its end — the part that must agree for
  a rhyme to hold. \"Delight\" and \"tonight\" share the tail `-ight`; the rhyme is judged
  on the tail, not the whole word.
]

#section("Every language rhymes differently")

This is the chapter where the multilingual promise does its most interesting work, because
what counts as a rhyme is genuinely different in each language. Before comparing two tails,
Inkhaven *normalises* them by the target language's own rules of sound:

- *German* devoices final consonants — a `d` at the end of a word is pronounced `t`, `b`
  becomes `p`, `g` becomes `k`. So German _Rad_ and _Rat_ rhyme perfectly to the ear
  though they end in different letters, and Inkhaven, devoicing before it compares, hears
  the rhyme the German ear hears.
- *French* drops the mute final `e`. _Aimée_ and _aimer_ agree once the silent ending is
  stripped. French also lets the eye rule more than most traditions — a legacy of its
  spelling — and Inkhaven's normalisation follows the convention.
- *Russian* reduces unstressed `о` toward `а` (the reduction called _akanye_), so words
  that differ on paper can chime in the mouth. Normalising for it lets Inkhaven grade
  Russian rhyme the way a Russian ear grades it — and Russian, with its rich inflectional
  endings, rhymes so easily that the tradition prizes the *unexpected* rhyme over the
  merely correct one.

```
$ inkhaven poetry rhyme Rad Rat --language de
  Rad / Rat: perfect masculine rhyme on “-at”
```

Two differently-spelled words, and Inkhaven calls them a perfect rhyme — because it
devoiced the final `d` to `t` before comparing, hearing the rhyme the German ear hears
rather than the difference the eye sees. The normalisation is silent here, but it is doing
real work: without it, `Rad` and `Rat` would look like a near rhyme at best. And in Russian
the same machinery, running the akanye reduction, grades a clean feminine rhyme:

```
$ inkhaven poetry rhyme ночи очи --language ru
  ночи / очи: perfect feminine rhyme on “-очи”
```

#callout(label: "Does it work in Russian? Here, best of all")[
  Rhyme is the feature where Russian most outshines English. Russian spelling tracks sound
  closely, its stress (once marked) is unambiguous, and its rhyme tradition is precise and
  well-codified. Inkhaven's Russian rhyme judgements are correspondingly sharp — the akanye
  normalisation, feminine/masculine alternation, and rich inflectional rhymes all fall out
  cleanly. One caution carried over from Chapter 2: Russian's mobile stress still needs
  marking on longer words for the tool to place the tail correctly, so mark your stresses
  and the rhyme engine rewards you. If you want to *see* it at its most accurate, give it a
  marked line of Pushkin.
]

#recap((
  [`poetry rhyme` grades two words on *quality* (perfect / near / eye / none), *type*
   (masculine / feminine / dactylic), and the *shared tail* they chime on.],
  [*Near* rhyme is close in sound; *eye* rhyme is close only in spelling. In English,
   read from spelling alone, the two cannot always be told apart — the rhyme engine's
   hardest limit, and an English-specific one.],
  [Rhyme *type* is structural, not cosmetic: Russian verse alternates masculine and
   feminine rhymes by rule, and the completion tools check for it.],
  [Each language is *normalised* by its own rules before comparison — German final-consonant
   devoicing, French mute-e, Russian akanye — and the tool reports the normalisation so you
   can inspect its reasoning.],
  [Russian is where the rhyme engine is sharpest, because its spelling tracks its sound;
   English is where it is roughest.],
))
