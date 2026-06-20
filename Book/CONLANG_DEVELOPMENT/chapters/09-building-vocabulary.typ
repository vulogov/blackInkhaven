#import "../design.typ": *

#chapter(number: 9, title: "Building a vocabulary")

Your language can now make word-shapes; it is time to give them meanings. The
collection of words and their meanings is the *lexicon*, and each entry pairs a
word with what it means, what part of speech it is, and any extra notes you care
to keep. This chapter is about adding words by hand; the next adds two more
powerful ways to grow the lexicon.

#section("Adding a word")

The basic command is *add-word*. It needs the language, the word itself, its
*part of speech*, and its meaning (the *translation* into your everyday
language):

```sh
inkhaven language add-word Eldar makil --type noun --translation sword
```

This stores *makil* in Eldar's *Dictionary* chapter as a noun meaning "sword".
No reindex is needed — commands that write changes for you handle that
themselves. Add a few more:

```sh
inkhaven language add-word Eldar kira --type noun --translation bird
inkhaven language add-word Eldar nami --type verb --translation see
inkhaven language add-word Eldar mira --type adjective --translation bright
```

#term("Part of speech")[
  The grammatical category of a word — what *kind* of word it is. The common
  ones are *noun* (a thing: sword, bird), *verb* (an action: see, run),
  *adjective* (a description: bright, cold), and *adverb*. The category matters
  later, when grammar rules apply only to words of a certain kind.
]

#term("Translation (gloss)")[
  The meaning of a word given in your working language — for *makil*, the
  English "sword". A short meaning like this is also called a *gloss*. It is how
  you and Inkhaven know what the word means.
]

#section("Recording more than the basics")

A word often carries more than a bare meaning. Is it formal or vulgar? What
subject does it belong to — warfare, cooking, religion? Does it belong to an
older era of the language? You can record all of this, and search by it later.
A dictionary entry, stored as a small HJSON block, can hold these extra fields:

```hjson
{ word: "makil", type: "noun", translation: "sword",
  register: "formal", domain: ["weapon"], era: "third_age" }
```

#term("Register")[
  The level of formality or social setting a word belongs to — *formal*,
  *neutral*, *vulgar*, *sacred*, *archaic*. The same idea in your everyday
  language separates "purchase" (formal) from "buy" (neutral). Marking register
  lets a language have words that fit a king's court and others that fit a
  tavern.
]

#term("Domain")[
  The subject area a word belongs to — *weapon*, *kinship*, *weather*, *magic*.
  Domains help you build vocabulary by theme and find related words quickly.
]

#section("Finding words again")

As the lexicon grows you will want to search it. The *query* command filters by
any of the rich fields — part of speech, register, domain, era, or a substring of
the word or its meaning:

```sh
inkhaven language query Eldar --pos noun
inkhaven language query Eldar --domain weapon
inkhaven language query Eldar --text bright
```

Each prints the matching entries. With `--json` you get machine-readable output,
handy for scripts.

#section("Importing many words at once")

If you already have a word list — perhaps in a spreadsheet — you can bring it in
all at once. Export it as a CSV file (a simple table of word, part of speech,
translation, …) and import:

```sh
inkhaven language add-word Eldar --import words.csv
```

Every row becomes an entry. This is the fast path when you are moving an existing
list into Inkhaven.

#callout(label: "Removing a word")[
  Made a mistake, or retired a word? `inkhaven language remove-word Eldar makil`
  deletes it from the dictionary. Like `add-word`, it finds the entry by name and
  needs no reindex.
]

#section("Using your words while you write")

The reason to build a lexicon is, of course, to *use* it. When you write prose in
Inkhaven's editor, you can drop an invented word straight in: type a colon, your
language's name or code, and another colon — `:eldar:` — and a picker appears
listing your words; choose one and it is inserted. Inkhaven also lights up your
invented words where they appear in a manuscript, and can translate whole
paragraphs into or out of the language. These editor features are beyond the
scope of this command-focused book, but it is worth knowing the lexicon you are
building is meant to be lived in.

#recap((
  [The *lexicon* is your vocabulary; each entry pairs a word with a meaning.],
  [`add-word <lang> <word> --type <pos> --translation <meaning>` adds one.],
  [Record *part of speech*, *register*, *domain*, and *era*; search them with
   `query`.],
  [Import a CSV with `add-word --import`; remove with `remove-word`.],
  [In the editor, `:lang:` inserts a word from the lexicon.],
))
