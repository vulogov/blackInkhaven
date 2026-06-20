#import "../design.typ": *

#chapter(number: 14, title: "Idioms and metaphors")

A language is more than literal meanings. "It's raining cats and dogs" has
nothing to do with animals; "time is money" treats an abstraction as a substance.
These *idioms* and *conceptual metaphors* are part of what makes a language feel
lived-in and human. Recording a few gives your conlang texture — and helps
Inkhaven's translator render meaning rather than word-for-word nonsense.

#section("Idioms")

An *idiom* is a fixed phrase whose meaning is not the sum of its words. You record
one with its literal word-by-word reading *and* its real meaning, so both are
preserved:

```sh
inkhaven language idiom-add Eldar --form "kala men" \
    --literal "cold heart" --meaning "unforgiving" --register formal
```

Here the Eldar phrase *kala men* literally says "cold heart" but means
"unforgiving". The optional `--register` marks it as formal speech.

#term("Idiom")[
  A fixed expression whose meaning cannot be worked out from its individual words
  — English "kick the bucket" means "to die", not anything about buckets.
  Recording the literal reading and the real meaning separately lets a translator
  (human or AI) avoid translating it word for word into nonsense.
]

#section("Conceptual metaphors")

A *conceptual metaphor* is a deeper pattern: a whole way of thinking that maps one
idea onto another, surfacing in many expressions. English speakers routinely talk
about *time* as if it were *money* — you *spend* time, *save* time, find it
*wasted*. Declaring such a mapping tells the translator how your speakers think:

```sh
inkhaven language metaphor-add Eldar --source JOURNEY --target LIFE \
    --example "she reached a crossroads"
```

This records that your speakers understand *life* (the target) in terms of a
*journey* (the source), with an example expression.

#term("Conceptual metaphor")[
  A systematic way one domain of experience is understood in terms of another,
  underlying many everyday expressions — LIFE IS A JOURNEY, ARGUMENT IS WAR
  ("he *attacked* my point", "I *defended* my claim"). It is written with the
  *source* (the concrete domain, JOURNEY) mapped onto the *target* (the abstract
  one, LIFE). Choosing your language's metaphors shapes how its speakers see the
  world.
]

#section("Reviewing what you have")

List the idioms and metaphors you have recorded with:

```sh
inkhaven language idioms Eldar
```

Both are stored in the *Grammar* chapter alongside your typology answers. When you
translate a paragraph into the language, Inkhaven consults them so the result is
idiomatic — reaching for *kala men* where the meaning is "unforgiving", rather
than translating "unforgiving" literally.

#callout(label: "A little goes a long way")[
  You do not need many. A handful of idioms and one or two governing metaphors
  give a language a recognisable way of speaking — its own turns of phrase, its
  own way of carving up experience. They are also a delight to invent: each one
  is a tiny window into how your speakers think.
]

#recap((
  [An *idiom* is a fixed phrase whose meaning is not its literal words; record
   both with `idiom-add`.],
  [A *conceptual metaphor* maps a concrete *source* domain onto an abstract
   *target* (LIFE IS A JOURNEY); record it with `metaphor-add`.],
  [`idioms` lists what you have; both are stored in the Grammar chapter.],
  [The translator uses them to render meaning idiomatically rather than literally.],
))
