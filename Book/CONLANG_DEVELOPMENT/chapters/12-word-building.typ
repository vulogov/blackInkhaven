#import "../design.typ": *

#chapter(number: 12, title: "Building new words")

Inflection (last chapter) changes a word's form without changing what it
fundamentally is — *stone* and *stones* are the same word. *Derivation* is
different: it makes a genuinely *new* word from an old one. *Build* becomes
*builder* (a person who builds); *king* becomes *kingdom* (the realm of a king).
This is one of the most productive ways a real lexicon grows, and your language
can do it too.

#term("Derivation")[
  Forming a new word (a new *lexeme*, with its own dictionary entry) from an
  existing one, usually by adding an affix and often changing its part of speech.
  *teach* (verb) → *teacher* (noun); *happy* (adjective) → *happiness* (noun).
  Contrast with *inflection*, which only changes a word's form, not its identity.
]

#section("Declaring a derivation rule")

You add derivation rules to the morphology block in the *Grammar* chapter, under
a `derivations` field. Each rule names the affix it adds, where it goes, which
part of speech it applies to (`from_pos`), what part of speech it produces
(`to_pos`), and a template for the new meaning:

```hjson
derivations: [
  { name: "agent", form: "ron", position: "suffix",
    from_pos: "verb", to_pos: "noun", gloss_template: "one who {}s" }
]
```

This is an *agent noun* rule: take a verb, add *-ron*, and you get a noun meaning
"one who does the verb". The `{}` in the template is filled with the root's
meaning — so a verb meaning "build" yields a noun glossed "one who builds".

#term("Agent noun")[
  A noun naming the doer of an action, derived from a verb — *teach* → *teacher*,
  *bake* → *baker*. The "*-er*" of English is an agent suffix. Agent nouns are
  one of the most common derivations across the world's languages.
]

#section("Coining derived words")

With a rule in place, ask Inkhaven to apply it to a root:

```sh
inkhaven language derive Eldar --root kata --gloss build --pos verb
```

Inkhaven runs every derivation rule whose `from_pos` matches "verb", applies the
affix (with allophony at the join), and prints the proposed new words — their
form, meaning, and part of speech. For *kata* "build" it might propose *kataron*,
"one who builds", a noun.

By default this only *proposes*. To actually add the derived words to your
dictionary — recording where each came from, as a small etymology — add `--yes`:

```sh
inkhaven language derive Eldar --root kata --gloss build --pos verb --yes
```

#term("Etymology")[
  The origin and history of a word — what it was derived or descended from. When
  Inkhaven coins a derived word for you, it records the etymology in the entry
  ("derived from *kata* via the agent rule"), so your dictionary remembers how
  each word came to be.
]

#callout(label: "Derivation versus generation")[
  Two ways to grow vocabulary, for two purposes. *Generation* (Chapter 10)
  invents brand-new roots on a theme — good for filling out basic vocabulary.
  *Derivation* grows words *from words you already have*, the way real languages
  spin families of related terms from a single root. Use generation for breadth,
  derivation for depth and internal consistency.
]

#recap((
  [*Derivation* makes a new word from an existing one, often changing its part of
   speech (*build* → *builder*).],
  [Declare rules under `derivations`: `form`, `position`, `from_pos`, `to_pos`,
   and a `gloss_template` with `{}` for the root's meaning.],
  [`derive --root … --gloss … --pos …` proposes derived words; `--yes` adds them
   with their *etymology* recorded.],
  [Use generation for new roots, derivation to grow word families from existing
   roots.],
))
