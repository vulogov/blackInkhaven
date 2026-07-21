#import "../design.typ": *

#chapter(number: 2, title: "Modelling Russian")

A study begins by making the object of study explicit. In Inkhaven that means
creating the language, declaring its sounds, and giving it enough vocabulary to work
with. The model you build here is the ground every later chapter stands on.

#section("Creating the language")

One command makes the language and its empty chapters — phonology, dictionary,
grammar, and the rest:

```sh
inkhaven language init Russian
```

Russian now exists as a language book in your project, alongside any others. It is
blank: no sounds, no words. We fill it in.

#section("The inventory: sounds, or letters?")

Here is the first real decision, and it is a linguistic one. Inkhaven's phoneme
inventory is a list of segments, each with an `ipa` field. A phonologist's instinct
is to fill it with the International Phonetic Alphabet — Russian's /p/, /pʲ/, /a/,
/ɨ/, and so on — because that is the level at which sound *contrasts* live, and the
distinctive-feature tools of the next chapter are built on IPA symbols.

But Russian is written in Cyrillic, and much of what we will do — glossing real
sentences, building a corpus from a real book — works over Cyrillic *text*. If our
inventory were in the IPA, the tools could not line the inventory up with the words.
So for this study we model Russian at the level of its *orthography*: each Cyrillic
letter is a segment. It is a simplification — Cyrillic spelling is not a perfect
phonemic transcription — but it keeps the model and the texts in the same alphabet,
and it is honest about what it is.

#term("Orthography vs. phonology")[
  *Orthography* is how a language is spelled; *phonology* is how it sounds. They are
  never identical — English *though*, *through*, *tough* share `ough` and share
  almost nothing in sound. Russian's fit is much closer than English's, but still
  imperfect: the letter `о` is pronounced *a* when unstressed. Modelling at the
  orthographic level trades some phonological precision for the ability to work
  directly with written Russian — a trade this book makes deliberately, and revisits
  in Chapter 3.
]

Open the Russian language's *Phonology* chapter in the editor and declare the
inventory as an HJSON block — the ten vowel letters, the twenty-one consonants, and
the two signs:

```hjson
{ phonemes: [
    { ipa: "а", kind: "vowel" }  { ipa: "е", kind: "vowel" }  { ipa: "ё", kind: "vowel" }
    { ipa: "и", kind: "vowel" }  { ipa: "о", kind: "vowel" }  { ipa: "у", kind: "vowel" }
    { ipa: "ы", kind: "vowel" }  { ipa: "э", kind: "vowel" }  { ipa: "ю", kind: "vowel" }
    { ipa: "я", kind: "vowel" }
    { ipa: "б", kind: "consonant" }  { ipa: "в", kind: "consonant" }  // …и так далее…
    { ipa: "ь", kind: "consonant" }  { ipa: "ъ", kind: "consonant" }
] }
```

`inkhaven language stats Russian` then confirms what you declared:

```
  inventory · 33 phonemes (23 C / 10 V)
```

Thirty-three segments — the Russian alphabet exactly. (The soft and hard signs, `ь`
and `ъ`, are letters rather than sounds; classing them as consonants lets words that
contain them segment cleanly, which is all the model needs of them.)

#section("A starter lexicon")

Analysis needs words. Add them one at a time, or import a list; each entry is a
headword, a part of speech, and a gloss:

```sh
inkhaven language add-word Russian мать  --type noun --translation mother
inkhaven language add-word Russian брат  --type noun --translation brother
inkhaven language add-word Russian дом   --type noun --translation house
inkhaven language add-word Russian окно  --type noun --translation window
```

A handful is enough to begin; the corpus chapter will pull far more vocabulary from
a real text. Everything is Cyrillic throughout — the headword, the bucket it files
under (`мать` lands under `М`), the segmentation — and it simply works.

#callout(label: "The model is yours to inspect")[
  Every result in this book is computed from the inventory and lexicon you just
  declared — nothing else. If a later number surprises you, the answer is always in
  the model: add the missing word, fix the inventory, and run the command again. That
  transparency is the whole point of modelling a language explicitly.
]

#recap((
  [`language init <name>` creates the language; its phoneme inventory is declared as
   an HJSON block in the Phonology chapter, and `stats` confirms it.],
  [We model Russian at the *orthographic* (Cyrillic-letter) level — a deliberate
   simplification that keeps the model and real Russian texts in one alphabet, at
   some cost in phonological precision (Chapter 3 revisits the cost).],
  [`add-word` builds the lexicon; Cyrillic headwords, buckets and segmentation all
   work directly.],
))
