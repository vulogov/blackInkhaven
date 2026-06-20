#import "../design.typ": *

#chapter(number: 10, title: "A healthy lexicon")

A growing vocabulary can drift into trouble: two words that sound identical, two
words that mean the same thing, a word that quietly breaks your own sound rules.
This chapter shows how Inkhaven keeps the lexicon honest — and how it can invent
whole batches of new words for you, on a theme, without ever creating a
duplicate.

#section("Auditing for problems")

The *audit* command is your lexicon's health check. It reads every entry and
reports three kinds of trouble:

```sh
inkhaven language audit Eldar
```

#set enum(numbering: "1.")
+ *Phonotactic violations* — a word whose sounds break the constraints you set in
  Chapter 6 (perhaps imported from an old list before you tightened the rules).
+ *Homophones* — two different words that come out pronounced the same after
  allophony. Sometimes you want these; often they are accidents.
+ *Duplicate meanings* — two words with the same translation, which may be an
  oversight.

#term("Homophone")[
  A word that sounds the same as another but means something different — like
  English *to*, *too*, and *two*. A few are natural; many by accident usually
  signal a mistake. The audit catches them even when two words only *become*
  identical after your allophony rules apply.
]

The audit only reports; it changes nothing. Use it whenever the lexicon grows, or
before you publish your dictionary, to catch surprises.

#section("Generating words with AI")

Building a few hundred words by hand is slow. Inkhaven can generate a batch for
you on a theme — but with an important guarantee about *where the words come
from*. The *forms* (the actual sound-shapes) are made by Inkhaven's own
generator, so they always obey your phonotactics. The AI only supplies the
*meanings*. This split — forms from the engine, meanings from the AI — is what
keeps generated words consistent with your language.

```sh
inkhaven language generate-lexicon Eldar --topic seafaring --count 20
```

This proposes twenty seafaring words, each a legal Eldar form with an AI-chosen
meaning. By default it only *prints* the proposals; add `--yes` to actually add
them to the dictionary.

#term("The dedup gate")[
  A set of automatic checks every proposed word passes before it is accepted, so
  generation never creates a duplicate. It rejects a word that breaks the
  phonotactics, that sounds identical to an existing word, that repeats a meaning
  already in the lexicon, or — with the optional `--semantic` check — that is too
  close in meaning to an existing word (a near-synonym). "Forms obey the
  language; meanings come from the AI; nothing duplicates."
]

#subsection("Catching near-synonyms")

Two words can have different translations yet mean almost the same thing —
"stone" and "rock", say. To reject such near-synonyms too, add `--semantic`:

```sh
inkhaven language generate-lexicon Eldar --topic seafaring --count 20 --semantic --yes
```

This compares the meaning of each proposal against your existing words using a
language-model technique and drops the ones that are too close. It is slower but
keeps a tight, non-redundant vocabulary.

#callout(label: "AI here is advisory, like everywhere")[
  Without `--yes`, generation shows you the proposals (and the reasons any were
  rejected) and changes nothing. You stay in control: review the list, then
  commit. Meanings come back in your working language. If you would rather not
  use AI at all, build the lexicon by hand with `add-word` — the audit and the
  phonotactic checks work just the same.
]

#section("Finding undefined words in your writing")

If you have been writing prose that *uses* the language, Inkhaven can scan your
manuscript for words that look like your language but are not yet in the
dictionary — names and terms you coined on the fly and never recorded:

```sh
inkhaven language scan-manuscript Eldar
```

It lists candidate undefined words, so you can add the ones worth keeping. This
closes the loop between writing and the lexicon.

#section("What is your lexicon still missing?")

A young lexicon always has holes. Inkhaven can tell you *which* basic concepts you
have not coined yet, by comparing your dictionary against a reference list of
concepts:

```sh
inkhaven language gaps Eldar
```

By default it checks against the *Swadesh-100* — the classic list of the hundred
most fundamental words every language has (I, water, sun, eat, big…), here
translated into your working language. It reports your coverage and lists what is
missing, most-core words first:

#term("Swadesh list")[
  A short list of universal, culture-independent concepts (body parts, basic
  verbs, natural features, pronouns) compiled by the linguist Morris Swadesh.
  Because every language has words for them, it is the standard yardstick for
  "have I covered the basics?" and for comparing related languages.
]

The missing list is shaped to hand straight back to the generator — coin exactly
the gaps with `generate-lexicon`. You can also point `--scope` at your own concept
list (an HJSON file of a topic like seafaring or cookery) to check coverage of any
domain you care about.

#recap((
  [`audit` reports phonotactic violations, *homophones*, and duplicate
   meanings — it only reports, never changes.],
  [`generate-lexicon --topic … --count …` invents themed words: *forms* from the
   engine (always legal), *meanings* from the AI.],
  [The *dedup gate* blocks illegal, homophone, and duplicate-meaning words;
   `--semantic` also blocks near-synonyms.],
  [Nothing is added without `--yes`; AI is optional.],
  [`scan-manuscript` finds language-like words in your prose that are not yet
   defined.],
  [`gaps` compares your lexicon against the *Swadesh-100* (or your own concept
   list) and reports what is still missing.],
))
