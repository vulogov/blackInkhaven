#import "../design.typ": *

#chapter(number: 27, title: "Building your conlang with Bund")

Everything you have built so far, you built by *describing* it. You wrote blocks
of HJSON — a list of phonemes, a set of templates, a table of affixes — and
Inkhaven read them. That is the *declarative* way: you state what is true, and the
program works out the rest. It is clear, it is durable, and for most languages it
is all you will ever need.

But there is a second way in, and it is worth knowing about even if you never
reach for it. Inkhaven carries a small programming language of its own, called
*Bund*, and through it you can *build* a language step by step — invent a hundred
words in a loop, inspect what you have and act on it, run the whole pipeline from
empty project to finished dictionary in one recipe. This chapter is a gentle look
at that programmatic way: what it is, why it is sometimes more powerful than a
stack of HJSON files, and enough of the shape of it to read a script without fear.

#callout(label: "You do not need to be a programmer")[
  If the word \"script\" makes you uneasy, relax: nothing in this book requires
  it, and your language is no less real for being hand-written. Think of this
  chapter as a tour of a workshop's power tools. You can admire them, try one,
  and still do most of your work by hand.
]

#section("Two ways to say the same thing")

Declarative and programmatic are not rivals; they are two doors into the same
room. When a script defines a phoneme block, it writes *exactly* the HJSON a
hand-author would have typed — byte for byte the same paragraph in the same
chapter. A language built by script opens, inspects, and prints identically to one
built by hand, and you can mix the two freely: hand-write the phonology, script
the thousand-word lexicon. Nothing is lost either way.

#term("Declarative")[
  Describing *what* you want as data and letting the program act on it — the HJSON
  blocks of this book. You say "these are my phonemes"; you do not say how to read
  them.
]

#term("Programmatic")[
  Giving an ordered list of *instructions* — do this, then this, then this. A
  program can repeat, decide, and compute, which inert data cannot.
]

#section("What Bund is")

#term("Bund")[
  A small *stack-based* programming language built into Inkhaven. Its conlang
  vocabulary — the words beginning `ink.lang.` (or the short `lang.`) — reaches
  every part of the ConLang Suite: the same generators, validators, and
  renderers the `inkhaven language` commands use.
]

Bund reads a little unusually at first, because the value comes *before* the
action. Where English says "add one and two", Bund says `1 2 add` — push the one,
push the two, then run `add`, which takes the two numbers and leaves their sum.
Everything works this way: you lay values down, and a *word* picks them up.

#term("Stack")[
  A pile of values, like a stack of plates: the last one put down is the first
  taken up. Bund words take their inputs from the top of the stack and leave their
  results there. Reading a script is just following what is on the pile.
]

#term("Word (in Bund)")[
  One instruction — a named operation. `lang.add_word` is a word; so is `lang.ipa`.
  Each word's inputs and outputs are written in a *stack-effect* comment like
  `( lang word pos translation -- )`: the names left of the `--` are taken from the
  stack, those on the right are left behind.
]

So when you read, in the reference of Appendix C, that `lang.ipa` has the effect
`( lang word -- ipa-surface-string )`, it means: give it a language name and a
word, and it hands back that word's pronunciation. You would write
`"Eldar" "tap" lang.ipa` — the two values, then the word that consumes them.

#section("Why a script can do more than a file")

If the two are equivalent, why ever script? Because a program can do three things
a static file cannot.

#subsection("It can repeat")

A file lists each word by hand. A script writes a *loop*: generate fifty
word-shapes, derive every cell of a paradigm, evolve a whole proto-dictionary into
a daughter — all from one short instruction, however many items there are. The
tedium that makes a large language daunting to hand-author simply vanishes.

#subsection("It can decide")

A file cannot look at itself. A script can: it can run the `audit`, read the
`gaps` report, check the `stats`, and *act on what it finds* — add a word only if
it would not collide, coin only the basic concepts still missing, accept a
generated form only if it passes the phonotactics. The language becomes
self-checking.

#subsection("It is a recipe, not just a result")

A hand-built language is a *result*; a script is the *recipe* that produces it.
Keep the recipe and you can re-run it to rebuild the language from nothing, change
one sound rule at the top and regenerate everything that flows from it, or hand
the whole recipe to someone else who can reproduce your language exactly. This is
the deepest difference: declarative data captures *what your language is*;
a program captures *how it came to be*.

#section("A whole language in one breath")

Here is a complete small language — sounds, a grammar setting, three words — and
then a sentence built from them, start to finish in a single script:

```bund
"Avesha" lang.init
"Avesha" "Phonology" "{ phonemes:[{ipa:\"k\",kind:\"consonant\"}{ipa:\"a\",kind:\"vowel\"}]
   classes:{C:[\"k\"] V:[\"a\"]} templates:{root:[{pattern:\"C V C V\"}]} }" lang.define
"Avesha" "Grammar" "{ grammar:{word_order:\"sov\",alignment:\"nominative_accusative\"} }" lang.define
"Avesha" "kira" "noun" "bird"  lang.add_word
"Avesha" "nami" "verb" "see"   lang.add_word
"Avesha" "pata" "noun" "stone" lang.add_word
"Avesha" "kira:bird" "nami:see" "pata:stone" lang.sentence println
```

`lang.init` makes the language and its chapters; each `lang.define` writes one
HJSON block into a chapter (the `\"` marks are just quotes carried safely inside a
quoted string); `lang.add_word` adds dictionary entries; and `lang.sentence`
assembles a subject–verb–object clause and `println` prints it. Open *Avesha*
afterwards with `inkhaven language …` and it is indistinguishable from one you
typed by hand — because it *is* the same data.

#section("Three kinds of word, and a safety gate")

The conlang vocabulary comes in three flavours, and Inkhaven guards the
last two so a script can never surprise you:

#term("Inspectors, mutators, AI words")[
  *Inspectors* only read — `lang.ipa`, `lang.stats`, `lang.sentence` — and are
  always allowed. *Mutators* change your project — `lang.init`, `lang.add_word` —
  and *AI words* call a language model — `lang.compose`, `lang.generate_lexicon`.
  These last two are switched *off* until you opt in.
]

You enable the guarded categories in `inkhaven.hjson`, naming exactly what a
script may do:

```hjson
scripting: { enabled_categories: ["store_write", "ai_write"] }
```

#callout(label: "Advisory, like everything AI in Inkhaven")[
  The AI words follow the same rule as the rest of the suite: they *return*
  suggestions and never write your book themselves. `lang.generate_lexicon` hands
  back a list of proposed words; your script decides which to keep and commits
  them with `lang.add_word`. Nothing reaches the page without an instruction you
  wrote.
]

#section("Building artefacts the native way")

Writing HJSON inside quoted strings (with all those `\"` marks) is the simplest
path for whole blocks, but Bund can also build the data structures directly.
Because Bund's curly braces mean something else, a small constructor turns a flat
list into a dictionary, and it answers to friendly names so a script reads like
what it makes:

```bund
[ "ipa" "k" "kind" "consonant" ] phoneme
```

`phoneme` here is the dictionary constructor (it is the same word as `rule`,
`block`, and `word` — aliases you can read as labels). Hand the result to
`lang.define` and it is serialised to the same HJSON as before. Most authors reach
for this only when generating blocks in a loop; for a fixed block, the string form
is plainer.

#section("Running a script")

Two ways to run Bund. From the command line, pass the script and point at your
project:

```sh
inkhaven bund "\"Eldar\" \"tap\" lang.ipa println" --project .
```

Or, inside the editor, write the script into a *Script* book and run it there —
handy for a refresh routine you re-run as the language grows. Appendix C lists
every conlang word, its stack effect, and its safety category, so you can assemble
recipes of your own.

#recap((
  [Inkhaven offers two equivalent ways to build a language: *declarative* HJSON
   (what you used all book) and *programmatic* *Bund* scripts. A script writes the
   same HJSON, so the two mix freely.],
  [Bund is *stack-based*: values come before the action (`1 2 add`), and each
   *word* takes inputs from the stack and leaves outputs — written `( in -- out )`.],
  [A script beats a file when there is *repetition* (loops over words or
   paradigms), *decision* (inspect, then act), or a need for a *reproducible
   recipe* rather than a fixed result.],
  [Words come in three kinds — *inspectors* (always on), *mutators* and *AI words*
   (opt in via `scripting.enabled_categories`); AI words stay advisory.],
  [Run a script with `inkhaven bund "…" --project .` or from a Script book; the
   full word list is Appendix C.],
))
