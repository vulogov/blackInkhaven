#import "../design.typ": *

#chapter(number: 24, title: "Translating with your language")

By now your language has sounds, words, and a grammar. It can do something with
all three: *translate*. Inkhaven ships a rule-based translation engine that runs
entirely on what you built — it looks up your words, inflects them with your
paradigms, and orders them by your typology — plus a growing *memory* of the
sentences you approve, so the more you use it, the better it fits your language.

This is not a large language model guessing at Elvish. It is a deterministic
machine reading your dictionary and your grammar. Where it has the words and the
rules, it is right by construction; where it does not, it tells you what it is
missing rather than inventing.

#term("Rule-based translation")[
  Translation done by explicit rules and a lexicon rather than by a statistical
  model — the engine looks each word up, applies your inflection and agreement
  rules, and arranges the result by your declared word order. Because it uses
  *your* language's rules, its output is a direct consequence of the language you
  built, not an outside guess. Its limits are exactly your lexicon's limits: a word
  you have not coined cannot be translated, and the engine says so.
]

#section("Translating into, and out of")

The everyday command turns an English sentence into your language, glossed so you
can check it:

```
inkhaven language translate Eldar --text "The king drew his sword."
```

You get back the surface sentence in your language, an interlinear gloss, and a
literal back-rendering — three views of the same translation, so you can see not
just *what* it said but *how* it said it, and catch a wrong case or a missed
agreement at a glance. To go the other way, from your language back into English:

```
inkhaven language reverse Eldar --text "aran makil mapta"
```

And if you have built more than one language, you can translate directly between
two of your own, without English in the middle:

```
inkhaven language cross Eldar Khuz --text "aran makil mapta"
```

#section("A memory that learns your voice")

The engine is only as good as your rules — and no set of rules captures every turn
of a real language. So the engine keeps a *translation memory*: when a translation
comes out right, or you fix it into shape, you teach it back:

```
inkhaven language remember Eldar --source "The king drew his sword." --target "aran makil mapta"
```

From then on the engine reuses that approved pair, and pairs like it, instead of
rebuilding the sentence from scratch. Over a project's life this is where a
language's *idiom* accumulates — the phrasings that are correct but no rule would
have produced. See what it holds at any time:

```
inkhaven language memory Eldar
```

#term("Translation memory")[
  A store of approved source–target sentence pairs the engine reuses. It is how the
  translator learns the phrasings your rules do not capture on their own — the fixed
  expressions, the idiomatic word choices, the sentences you corrected once and want
  kept. Each pair you `remember` makes the next similar translation truer to the
  language as you actually speak it.
]

#section("Measuring the engine against your own texts")

Your Sample-texts chapter — the passages you translated by hand as you built the
language — is itself a *parallel corpus*: sentences in two languages, side by side.
The engine can read it:

```
inkhaven language corpus Eldar
```

And, more usefully, it can *grade itself* against it — translating each source
sentence and comparing to your hand-done target:

```
inkhaven language eval Eldar
```

The score tells you how much of your own corpus the engine reproduces, and, where
it falls short, exactly which sentences it gets wrong. That is a to-do list in
disguise: each failure points at a missing word, an unwritten rule, or a phrasing
worth adding to the memory. As your language matures, watch the score climb.

#section("Taking the memory with you")

The translation memory is portable. Export it as a self-contained `.itm` pack — for
a backup, for a collaborator, or to move it between projects:

```
inkhaven language export-translation Eldar --out eldar.itm
```

Everything the engine learned travels in that one file, so a language you translate
in is never locked to a single machine.

#recap((
  [Inkhaven translates with a *rule-based engine* over your phonology, lexicon, and
   grammar — `translate` (into your language), `reverse` (back to English), and
   `cross` (between two of your own). Every result comes glossed.],
  [The engine keeps a *translation memory*: `remember` an approved sentence pair and
   it is reused thereafter — this is where the language's idiom accumulates beyond
   what any rule captures. `memory` shows what it holds.],
  [Your Sample-texts are a *parallel corpus*: `corpus` reads it, `eval` grades the
   engine against it — and each failure is a pointer at a missing word or rule.],
  [`export-translation` writes an `.itm` pack, so the memory travels with the
   language.],
))
