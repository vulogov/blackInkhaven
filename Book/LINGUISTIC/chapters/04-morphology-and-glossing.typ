#import "../design.typ": *

#chapter(number: 4, title: "Morphology and glossing")

Russian is a *fusional* language: a single ending can carry case, number and gender
at once, and a noun or verb runs through a dozen forms. This is where a morphological
model earns its keep — you declare the endings once, and the workbench can then both
*generate* the forms and *parse* them back, and gloss whole sentences of real text.

#section("A fragment of the noun declension")

Russian nouns decline for six cases. We will not model all of it here — the point is
the method, not a complete grammar — so take one masculine noun, `дом` "house", and a
couple of its endings. In the *Morphology* chapter declare the affixes and a paradigm
that applies them:

```hjson
{ morphemes: [
    { id: "gen", gloss: "GEN", form: "а", position: "suffix" }   // дом-а
    { id: "ins", gloss: "INS", form: "ом", position: "suffix" }  // дом-ом
  ]
  paradigms: [ { name: "masc-noun", cells: [
    { features: { case: "nom" }, morphemes: [] }
    { features: { case: "gen" }, morphemes: ["gen"] }
    { features: { case: "ins" }, morphemes: ["ins"] }
  ] } ]
}
```

`paradigm` then generates the forms from the root:

```sh
inkhaven language paradigm Russian --root дом --template masc-noun --gloss house
```

```
  дом      house         (case=nom)
  дома     house-GEN     (case=gen)
  домом    house-INS     (case=ins)
```

#term("Fusional morphology")[
  A morphology in which one affix bundles several grammatical meanings at once, and
  the boundaries between them are blurred — Russian `-ом` is *instrumental* and
  *singular* and *masculine* together, indivisibly. Contrast *agglutinative*
  languages like Turkish, where each meaning has its own separable suffix. Russian
  sits firmly on the fusional side, which is why its endings are few but each does a
  lot.
]

#section("Reading a form backwards")

The interesting direction is the reverse. Hand the parser an inflected form and it
recovers the root and the ending:

```sh
inkhaven language parse Russian --word домом
```

```
  дом ‘house’ + INS
```

This is analysis, not lookup: the parser strips the endings your morphology declares
until what remains is a dictionary word. Point it at a page of Russian and it will
tell you, form by form, what case and number each word is in — the first thing a
learner (or a parser) must work out.

#section("Interlinear glossing")

The linguist's way of presenting a sentence is *interlinear glossed text*: the
sentence, a gloss under each word, and a translation. `igt` builds it:

```sh
inkhaven language igt Russian --text "окно дома"
```

```
окно     дом-а
window   house-GEN
'the window of the house'
```

Every recognised word is glossed and, where it is inflected, segmented into its
morphemes and their tags — `дома` shown as `дом-а`, `house-GEN`. The literal third
line is a scaffold you replace with a real translation; words the model does not yet
know pass through untouched, telling you exactly which vocabulary to add next.

#term("Interlinear glossed text (IGT)")[
  The standard three-line format for quoting an example from any language: the
  morpheme-segmented sentence, a morpheme-by-morpheme gloss in standard abbreviations
  (`GEN`, `INS`, `PL`…), and a free translation. It lets a reader who does not know
  the language see precisely how its meaning is assembled — which is why every
  grammar and every paper is built from it.
]

You will store and reuse these glosses in Chapter 6, where a handful of them, plus a
real text, become a corpus.

#section("Agreement")

Russian adjectives agree with their nouns in gender, number and case — *новый дом*
"new house", *новая книга* "new book", *новое окно* "new window". Once you have
declared the adjective's agreement paradigm, the Oracle can check that a form agrees:

```sh
inkhaven language check-agreement Russian --dependent adjective \
  --form новый --root нов --head-features "gender=masc,case=nom"
```

It regenerates the ending the adjective *should* take for those features and flags
the form if it differs — a plural or feminine ending on a masculine-nominative noun
is caught, and the expected form named. Agreement is much of what makes Russian
sentences hang together, and the check makes the rule you declared testable against
any form you write.

#recap((
  [Russian is fusional — one ending carries case, number and gender together;
   `paradigm` generates a root's forms from the endings you declare, and `parse`
   reverses them, recovering root and ending from an inflected word.],
  [`igt` glosses a real sentence as interlinear text, segmenting each inflected word
   (`дом-а` / `house-GEN`) and passing unknown words through so you see what to add.],
  [`check-agreement` tests adjective–noun (and any head–dependent) agreement in
   gender, number and case against the rule you declared.],
))
