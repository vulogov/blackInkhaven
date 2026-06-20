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

#section("Beyond prefixes and suffixes")

Not every language marks grammar by gluing pieces to the ends of words. Inkhaven
supports the other common strategies too; you choose them on a morpheme with
`position` or `process`.

#term("Infix")[
  An affix inserted *inside* the root rather than at an edge. Tagalog forms an
  actor from *sulat* ("write") as *s-um-ulat* — the *-um-* sits after the first
  consonant. Declare `position: "infix"`; the `anchor` is `before_first_vowel`
  (the default) or `after_first_vowel`.
]

#term("Circumfix")[
  A single affix in two pieces that wrap around the root — German makes a past
  participle with *ge-…-t* (*ge-sag-t*). Declare `position: "circumfix"` and
  write the `form` with a `_` marking where the stem goes: `ge_t` → *ge* + stem
  + *t*.
]

#term("Ablaut")[
  Marking grammar by *changing a sound inside* the root instead of adding
  anything — English *sing / sang / sung*. Declare `process: "ablaut"` and give
  the change as an SPE `rules` list (Chapter 7), e.g. `i > a`. The vowel swaps in
  place.
]

#term("Reduplication")[
  Marking grammar by *repeating* part (or all) of the root — Malay *buku* "book"
  → *buku-buku* "books". Declare `process: "reduplication"` with a `reduplicate`
  mode: `full` (the whole stem doubled), `initial_cv` (the first consonant +
  vowel copied to the front), `initial_syllable`, or `final_syllable`.
]

All four are written as ordinary morphemes and used in paradigm cells exactly
like prefixes and suffixes — and allophony still applies across the new seams.
For example, a morpheme `{ id: "ag", gloss: "AG", form: "um", position:
"infix" }` turns the root *tanik* into *t-um-anik*.

#section("Agreement")

Often one word must echo the grammar of another: an adjective takes its noun's
number and case, a verb takes its subject's person and number. This matching is
called *agreement* (or *concord*), and it is what makes "these tall trees" work
where "this tall tree" also does.

#term("Agreement (concord)")[
  The requirement that a *dependent* word copy certain grammatical features from
  the *head* it modifies — an adjective agreeing with its noun, a verb with its
  subject. The shared features (number, case, gender, person) must match.
]

You declare agreement as rules in the morphology block: which part of speech
agrees with which, on which features, and through which paradigm:

```hjson
agreement: [
  { dependent: "adjective", head: "noun", features: ["number", "case"], paradigm: "adj" }
]
```

Then Inkhaven can inflect a dependent to agree with a given head. Suppose a noun
is plural; to get the matching form of the adjective *mira* ("bright"):

```sh
inkhaven language agree Eldar --word mira --pos adjective \
    --gloss bright --features "number=pl"
```

It finds the agreement rule for adjectives, picks the `adj` paradigm cell that
matches `number=pl`, and prints the agreeing form — *mirai*, glossed
`bright-PL`. The `--features` you pass are the head's; only the ones the rule
lists as agreement features are copied.

#recap((
  [*Morphology* is how words change shape; the pieces are *morphemes*, and
   attached ones are *affixes* (prefix / suffix / *infix* / *circumfix*).],
  [Beyond gluing affixes, a morpheme can be a *process*: *ablaut* (an internal
   sound change) or *reduplication* (repeating part of the root).],
  [*Inflection* changes a word's form; the full set is a *paradigm*, described as
   `cells` of features + morphemes.],
  [`paradigm` builds the forms (allophony applies at every seam); `gloss` shows a
   sentence interlinear, word by word.],
  [*Agreement* makes a dependent copy a head's features; `agree` inflects it to
   match.],
))
