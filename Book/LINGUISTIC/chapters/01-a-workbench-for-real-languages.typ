#import "../design.typ": *

#chapter(number: 1, title: "A workbench for real languages")

Linguistics is an empirical science. A grammar is a set of claims about a language —
"Russian has six cases", "stress is unpredictable", "adjectives agree with their
nouns" — and each claim is answerable against data. What a working linguist needs is
somewhere to write the claims down *formally*, apply them to real forms, and see
whether they hold. That is exactly what Inkhaven's language model gives you: a
declared inventory, a lexicon, a morphology, a grammar, and a set of tools that read
them and report.

#section("The shape of the workflow")

Studying a language in Inkhaven follows one arc, whatever the language:

#block(breakable: false)[
  #set enum(numbering: "1.")
  + *Model* the language — create it, declare its phoneme inventory, add a lexicon.
    You are building an explicit, machine-readable description.
  + *Measure* the sound system — its inventory, its statistics, its minimal pairs.
  + *Analyse* words and sentences — parse the morphology, gloss real text, draw the
    syntax.
  + *Gather* usage — build a corpus from real texts and query it.
  + *Situate* the language — against typological universals, and against its own
    history.
]

Every step is a command, and every command reads the model you built and prints what
it finds. Nothing is hidden in a black box: the phonology you declare is the
phonology the tools use, so when a result surprises you, the fix is always to look at
the model.

#term("Model (of a language)")[
  An explicit, formal description — here, the phoneme inventory, lexicon, morphology
  and grammar you declare in Inkhaven. A model is deliberately a simplification; its
  value is that, being explicit, it can be *tested*. When the model and the language
  disagree, you have learned something.
]

#section("Why model Russian?")

Russian is a rewarding subject for a first study. Its sound system has the famous
*hard/soft* (palatalization) contrast that doubles its consonants and fills the
language with minimal pairs. Its morphology is *fusional* and rich — six cases,
three genders, two numbers, verbal aspect — so there is a great deal for a
morphological parser to chew on. Its word order is freer than English, which makes
its syntax interesting. It has a deep, well-understood history of sound change. And
it is written in Cyrillic, which is a useful test in itself: a workbench that only
worked for the Latin alphabet would be no workbench at all.

#callout(label: "A note on Cyrillic")[
  Every tool in this book handles Cyrillic text — the lexicon, the metrics, the
  glosser, the corpus. The one place the Latin bias of a shared reference table
  shows through is the distinctive-*feature* analysis of Chapter 3, and we meet it
  head-on there. Everywhere else, Russian is a first-class citizen.
]

#section("What you will build")

By the end you will have a working model of a slice of Russian — an inventory of its
sounds, a lexicon of common words, a handful of glossed sentences, a small corpus
drawn from a real text, and a register of hypotheses about its history — and, more
importantly, a method you can turn on any language you like. The next chapter starts
where every study starts: creating the language and giving it its sounds.

#recap((
  [Inkhaven's language model — inventory, lexicon, morphology, grammar — is a formal,
   testable description; the same tools that build an invented language *analyse* a
   real one.],
  [The workflow is model → measure → analyse → gather → situate, each step a command
   that reads your model and reports.],
  [Russian is the running example: a palatalization-rich sound system, fusional
   morphology, free-ish word order, a deep history — and Cyrillic, which the toolset
   handles throughout.],
))
