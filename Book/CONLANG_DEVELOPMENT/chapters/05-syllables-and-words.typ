#import "../design.typ": *

#chapter(number: 5, title: "Syllables and word shapes")

You have sounds. Now you decide how they fit together into *syllables*, and from
those, into possible words. This is where a language starts to have a
characteristic feel — the difference between a flowing tongue of open syllables
(like `ta-ki-na`) and a crunchy one full of consonant clusters (like `strkth`).

#section("What a syllable is")

A *syllable* is one beat of speech, built around a vowel. The word *banana* has
three: *ba-na-na*. Each syllable has up to three parts.

#term("Syllable")[
  A unit of pronunciation containing one vowel sound, optionally wrapped in
  consonants. It has three parts: the *onset* (consonants before the vowel), the
  *nucleus* (the vowel itself, the core), and the *coda* (consonants after the
  vowel). In *cat*, the onset is /k/, the nucleus is /a/, and the coda is /t/.
]

#term("Onset, nucleus, coda")[
  The three slots in a syllable. The *nucleus* is the required centre (a vowel).
  The *onset* is the consonant(s) before it, and the *coda* the consonant(s)
  after it; both are usually optional. A syllable with no coda (like *ta*) is
  called *open*; one with a coda (like *tan*) is *closed*.
]

#section("Describing word shapes with templates")

How do you tell Inkhaven what a possible Eldar word looks like? With a
*template*: a small pattern written with your class names, where `C` means "a
consonant", `V` means "a vowel", and parentheses mean "optional". The pattern
`C V (C)` reads: a consonant, then a vowel, then optionally another consonant —
that is, the syllables *ta* or *tan*, but not *atra*.

#term("Syllable template")[
  A pattern describing the allowed shape of a syllable, written with phoneme
  class names. `C V` is consonant-plus-vowel; `C V C` adds a final consonant;
  `(C)` marks an optional slot. A language usually allows a handful of templates.
]

You declare templates in the phonology block, under a `templates` field. Each
template can carry a `weight` — a number saying how common that shape should be
when Inkhaven generates words (higher means more frequent). Here we let Eldar
have open `CV` syllables and closed `CVC` ones, with open ones twice as common:

```hjson
templates: {
  root: [
    { pattern: "C V",   weight: 2.0 }
    { pattern: "C V C", weight: 1.0 }
  ]
}
```

The name `root` groups these as the shapes used for word *roots* (the core of a
word). Add this field beside `phonemes` and `classes`, and reindex.

#section("Generating words")

With an inventory and templates, Inkhaven can invent word-shapes for you that
obey your rules. This is the fastest way to discover the *sound* of your language
and to find candidate words to give meanings to later.

```sh
inkhaven language generate-word Eldar --role root --count 10
```

This prints ten random roots that fit your templates — perhaps *taki*, *mun*,
*sala*, *kira*. None of them mean anything yet; they are raw material. When one
sounds right, you will turn it into a real word in Part III.

#callout(label: "Listen to what comes out")[
  Generating a batch of words is the best way to *audition* your phonology. If
  the results feel too harsh, too samey, or hard to say, adjust your inventory
  or template weights and generate again. This loop — tweak, generate, listen —
  is the heart of designing a sound system. It costs nothing and uses no AI.
]

#section("Inspecting a single word")

To see how Inkhaven breaks a word into syllables, use the *syllabify* inspector:

```sh
inkhaven language syllabify Eldar --word takina
```

It prints the syllable division — `ta.ki.na` — showing the onsets, nuclei, and
codas it found. This is useful for checking that your templates do what you
expect, and it is the same machinery that later figures out where stress falls.

#term("Word root")[
  The core form of a word, carrying its basic meaning, before any endings are
  added. *build* is a root; *builder* and *rebuilding* are formed from it. In
  Part IV you will add endings to roots; here you are just shaping the roots
  themselves.
]

#recap((
  [A *syllable* is a beat of speech: an optional *onset*, a vowel *nucleus*, and
   an optional *coda*.],
  [*Templates* like `C V (C)` describe allowed syllable shapes; `weight` controls
   how often each appears.],
  [`generate-word` invents word-shapes that obey your rules — raw material for
   real words.],
  [`syllabify --word` shows how a word divides, for checking your templates.],
  [Designing phonology is a loop: tweak, generate, listen, repeat.],
))
