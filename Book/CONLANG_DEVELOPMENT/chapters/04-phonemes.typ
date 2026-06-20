#import "../design.typ": *

#chapter(number: 4, title: "Phonemes: the sounds")

A language begins with its sounds. Before a single word can exist, you must
decide what your language is *made of* — the small set of distinct sounds that
its words are built from. These are its *phonemes*.

#term("Phoneme")[
  A single distinct sound that can change the meaning of a word in a given
  language. In English, /p/ and /b/ are separate phonemes, because *pat* and
  *bat* mean different things. The slashes are the usual way to write a phoneme,
  to distinguish it from a letter. A language typically has somewhere between a
  dozen and a few dozen phonemes.
]

#section("Consonants and vowels")

Phonemes come in two broad families. *Vowels* are the open, sung sounds your
voice glides through — the /a/ in *father*, the /i/ in *machine*. *Consonants*
are the sounds made by closing or narrowing the mouth somewhere — /p/, /t/, /k/,
/s/, /m/. Every word is a weave of the two.

#term("Vowel and consonant")[
  A *vowel* is a sound made with open, unobstructed airflow (a, e, i, o, u and
  their many cousins). A *consonant* is a sound made by obstructing the airflow
  with the lips, tongue, or throat (p, t, k, s, m, n, l, r, and so on). Most
  syllables are one or more consonants around a vowel.
]

#section("A note on the IPA")

You will see phonemes written with the letters you know, but also with a few
unusual symbols like /ʃ/ (the *sh* sound) or /ŋ/ (the *ng* at the end of
*sing*). These come from the *International Phonetic Alphabet*, a worldwide
system where one symbol always means exactly one sound. You do not need to learn
it; for an a-priori language you may simply use ordinary letters for your sounds.
But the IPA is handy when a sound has no obvious letter, and Inkhaven happily
accepts either.

#term("International Phonetic Alphabet (IPA)")[
  A standard set of symbols in which each symbol represents one and only one
  speech sound, used by linguists worldwide. Useful for sounds that ordinary
  spelling writes ambiguously (English *th* is two different sounds; IPA writes
  them /θ/ and /ð/). Inkhaven lets you give each phoneme an IPA symbol and a
  plain-letter spelling.
]

#section("Building your inventory")

Your full set of phonemes is your *inventory*. You declare it as a `phonemes`
list inside a phonology block, placed in the *Phonology* chapter (Chapter 3
explained how to add a block). Each phoneme gives three things: its `ipa` symbol
(the sound), an optional `romanize` spelling (how you will write it with ordinary
letters), and its `kind` — `"consonant"` or `"vowel"`.

Here is a small starter inventory for Eldar — eight consonants and three vowels:

```hjson
{
  phonemes: [
    { ipa: "p", romanize: "p", kind: "consonant" }
    { ipa: "t", romanize: "t", kind: "consonant" }
    { ipa: "k", romanize: "k", kind: "consonant" }
    { ipa: "s", romanize: "s", kind: "consonant" }
    { ipa: "m", romanize: "m", kind: "consonant" }
    { ipa: "n", romanize: "n", kind: "consonant" }
    { ipa: "l", romanize: "l", kind: "consonant" }
    { ipa: "r", romanize: "r", kind: "consonant" }
    { ipa: "a", romanize: "a", kind: "vowel" }
    { ipa: "i", romanize: "i", kind: "vowel" }
    { ipa: "u", romanize: "u", kind: "vowel" }
  ]
}
```

Save this as a file in your language's `04-phonology` folder, then run `inkhaven
reindex --adopt`. Eldar now has a sound system.

#callout(label: "How big should the inventory be?")[
  Real languages range from about eleven phonemes to over a hundred, but most
  sit between twenty and forty. A small, balanced set like the eleven above is an
  excellent place to start — it is easy to pronounce, easy to spell, and gives
  plenty of room for words. You can always add more later. A rough rule of thumb
  for a natural feel: more consonants than vowels, and at least three vowels.
]

#section("Grouping sounds into classes")

Many rules you will write later apply not to one sound but to a whole *family* of
sounds — "all consonants", "all vowels", "the stop consonants". To name such a
family, you declare a *class*: a label and the list of phonemes in it. The two
most useful are a class of all consonants and a class of all vowels, usually
called `C` and `V`:

```hjson
classes: {
  C: ["p", "t", "k", "s", "m", "n", "l", "r"]
  V: ["a", "i", "u"]
}
```

Add this `classes` field inside the same phonology block, beside `phonemes`. We
will use `C` and `V` in the next chapter to describe the shapes words can take.

#term("Phoneme class")[
  A named group of phonemes that behave alike for some rule — for example all
  consonants, all vowels, or all *front* vowels. Classes let you write a rule
  once for a whole family instead of repeating it for each sound.
]

#section("Checking your work")

You can confirm Inkhaven has read your inventory with the *stats* command, which
prints a profile of the language:

```sh
inkhaven language stats Eldar
```

Early on it will simply report your inventory size — eight consonants and three
vowels. As you add words later, the same command grows into a rich picture of
how your sounds are actually used.

#recap((
  [A *phoneme* is a meaning-distinguishing sound; the full set is your
   *inventory*.],
  [Phonemes are *consonants* or *vowels*; you may write them with ordinary
   letters or *IPA* symbols.],
  [Declare the inventory as a `phonemes` list in the Phonology chapter; give each
   an `ipa`, a `romanize` spelling, and a `kind`.],
  [Group sounds into *classes* (like `C` and `V`) to write rules over whole
   families.],
  [`inkhaven language stats` confirms what Inkhaven read.],
))
