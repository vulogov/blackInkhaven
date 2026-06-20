#import "../design.typ": *

#chapter(number: 11, title: "Morphology: words that change")

Words are not frozen. *Cat* becomes *cats*; *walk* becomes *walked*. A word
changes its shape to express grammar — number, tense, who is doing what.
*Morphology* is the study of those changes, and it is the first half of grammar.
This chapter shows how to build the pieces words are made of and how to assemble
them.

#term("Morphology")[
  The part of grammar dealing with the internal structure of words — how they are
  built from smaller meaningful pieces, and how they change shape to express
  grammar. (The other half of grammar, *syntax*, deals with word order, covered
  in Chapter 13.)
]

#section("Morphemes and affixes")

The smallest meaningful pieces of a word are *morphemes*. *Cats* has two: *cat*
(the thing) and *-s* (meaning "more than one"). A morpheme like *-s* that attaches
to a word is an *affix*. Affixes that go on the front are *prefixes* (*re-* in
*rebuild*); on the end, *suffixes* (*-s*, *-ed*); inside, *infixes*.

#term("Morpheme")[
  The smallest unit of a word that carries meaning. *Unhappiness* has three:
  *un-* (not), *happy*, and *-ness* (the quality of). A *root* is the central
  morpheme; *affixes* attach to it.
]

#term("Affix")[
  A morpheme attached to a root to modify its meaning or grammar. A *prefix*
  attaches to the front (*re-do*), a *suffix* to the end (*cat-s*), an *infix* in
  the middle. Inkhaven supports prefixes and suffixes.
]

#section("Declaring your affixes")

You list a language's affixes in a *morphology block* in the *Grammar* chapter.
Each *morpheme* gives an `id` (a short name you choose), a `gloss` (a label for
what it means, conventionally in small capitals), a `form` (the actual sounds it
adds), and a `position` (`"prefix"` or `"suffix"`):

```hjson
{
  morphemes: [
    { id: "pl",  gloss: "PL",  form: "i",  position: "suffix" }
    { id: "dat", gloss: "DAT", form: "ti", position: "suffix" }
    { id: "def", gloss: "DEF", form: "na", position: "prefix" }
  ]
}
```

Here `pl` is a plural suffix *-i*, `dat` a dative suffix *-ti*, and `def` a
definite prefix *na-*. The glosses `PL`, `DAT`, `DEF` are the standard linguistic
abbreviations for "plural", "dative", and "definite".

#subsection("Ordering stacked affixes")

When two or more affixes pile onto the same side of a root, which comes first?
In many languages the order is fixed — a case ending might always sit closer to
the root than a number ending. You control this with an optional `precedence`
number on each morpheme:

```hjson
{ id: "pl",  gloss: "PL",  form: "i",  position: "suffix", precedence: 2 }
{ id: "dat", gloss: "DAT", form: "ti", position: "suffix", precedence: 1 }
```

#term("Affix ordering (precedence)")[
  A number saying how close an affix sits to the root when several stack: `0`
  (the default) means any position — the order you listed them in is kept; `1`
  means immediately next to the root; `2` the next slot out; and so on. A lower
  non-zero number is closer to the root. Above, the case suffix (precedence 1)
  always hugs the root and the number suffix (precedence 2) sits outside it, no
  matter which order a paradigm lists them.
]

#term("Gloss (grammatical)")[
  A short label for a grammatical piece, written in small capitals by convention:
  PL (plural), SG (singular), DAT (dative case), PST (past tense), and so on.
  Glosses let you write what a word *means* grammatically, piece by piece.
]

#term("Inflection")[
  Changing a word's form to express grammar *without* making it a new word —
  *cat* / *cats*, *walk* / *walked*. The set of all inflected forms of a word is
  its *paradigm*. (Contrast with *derivation*, Chapter 12, which makes a genuinely
  new word.)
]

#section("Paradigms: the forms of a word")

A *paradigm* is the full table of a word's inflected forms — for a noun, perhaps
singular and plural, in each grammatical case. You describe one as a list of
*cells*, each saying which features it expresses (number, case) and which
morphemes build it:

```hjson
paradigms: [ { name: "noun", cells: [
  { features: { number: "sg", case: "nom" }, morphemes: [] }
  { features: { number: "pl", case: "dat" }, morphemes: ["dat", "pl"] }
] } ]
```

Add this beside `morphemes` in the Grammar chapter. The first cell is the bare
root (no morphemes); the second stacks the dative and plural suffixes.

#term("Paradigm")[
  The organised set of all inflected forms of a word — for example a noun in
  singular and plural across every case. Each form is one *cell* of the paradigm,
  defined by its grammatical *features* and the morphemes that build it.
]

#section("Generating the forms")

Now Inkhaven can build the whole paradigm for any root, applying the morphemes
*and* your allophony rules at the joins:

```sh
inkhaven language paradigm Eldar --root kata --template noun --gloss stone
```

For the root *kata* ("stone") this prints each cell's surface form and its
gloss — for example *katati* for "stone-DAT". And here is the elegant part: if
your allophony rule says /t/ softens to /s/ before /i/, then *kata* plus the
dative *-ti* comes out as *katasi*, because the rule fires at the boundary,
exactly as it would in a real language. The sound machinery from Part II is
working for you automatically.

#section("Interlinear glossing")

When you write a sentence in the language, you will often want to show, word by
word, what each piece means — a *interlinear gloss*, the two-line format
linguists use. A dictionary entry can declare which paradigm it inflects by
(`paradigm: "noun"`), and then Inkhaven can gloss running text:

```sh
inkhaven language gloss Eldar --text "kata katai katat"
```

It prints each word above its Leipzig-style gloss — `kata` → `stone`, `katat` →
`stone-DAT` — and it recognises inflected *and* allophony-altered forms, because
it works out each entry's paradigm forward and matches what it finds.

#term("Interlinear gloss (Leipzig glossing)")[
  A way of showing a sentence in an unfamiliar language with a word-by-word
  breakdown lined up underneath, using grammatical glosses (PL, DAT, …). The name
  comes from the widely used *Leipzig Glossing Rules*. It is how grammars and
  textbooks make a foreign sentence transparent.
]

#recap((
  [*Morphology* is how words change shape; the pieces are *morphemes*, and
   attached ones are *affixes* (prefixes / suffixes).],
  [Declare affixes in a morphology block: each with `id`, `gloss`, `form`,
   `position`.],
  [*Inflection* changes a word's form; the full set is a *paradigm*, described as
   `cells` of features + morphemes.],
  [`paradigm` builds the forms (with allophony at the joins); `gloss` shows a
   sentence interlinear, word by word.],
))
