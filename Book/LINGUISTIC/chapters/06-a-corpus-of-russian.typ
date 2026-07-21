#import "../design.typ": *

#chapter(number: 6, title: "A corpus of Russian")

Everything so far has worked from a lexicon and a grammar — the language in the
abstract. But a language *lived in* leaves traces: the words people actually use, in
the proportions they actually use them, next to the words they actually put them near.
That body of real usage is a *corpus*, and it answers questions no dictionary can. This
chapter builds one from real Russian and interrogates it four ways — frequency,
context, collocation, and spread.

#term("Corpus linguistics")[
  The study of language through large bodies of real text rather than through
  invented examples or intuition. Its wager is that usage, counted carefully, reveals
  structure that introspection misses — which words are common, which go together,
  how meaning shifts with context. Nearly everything modern lexicography and language
  technology rests on was learned this way.
]

#section("Assembling the corpus")

A corpus in Inkhaven is drawn from two places, and you choose which with `--source`:

#block(breakable: false)[
  #set enum(numbering: "1.")
  + The *stored texts* — the interlinears you glossed and saved in Chapter 4 with
    `igt --save`. These are curated, glossed, lemma-tagged sentences.
  + The *manuscript prose* — any Russian you have written or pasted into your
    project. Inkhaven finds the Russian words in it the way it finds invented words
    in a conlanger's draft: a paragraph counts once it holds a word your lexicon
    knows, and within it every word that reads as Russian is gathered as a token.
]

`--source texts`, `--source prose`, or `--source all` (the default) select among them.
For real study you want text, and Project Gutenberg holds the Russian classics in the
public domain. Take the most famous opening in Russian poetry, the prologue to
Pushkin's #emph[Ruslan and Lyudmila] (1820):

```
У лукоморья дуб зелёный;
Златая цепь на дубе том:
И днём и ночью кот учёный
Всё ходит по цепи кругом;
```

Paste passages like this — several of them, from several authors — into a chapter of
your manuscript, and they become one corpus. A study grows by accretion: add a Chekhov
story, a chapter of Tolstoy, and every command below reads the larger whole without
changing.

#section("What is frequent?")

The first question of any corpus is *how often*. `frequency` counts every word and
reports the statistics real usage unlocks:

```sh
inkhaven language frequency Russian --source prose
```

```
  texts 1 · tokens 19 · types 18 · lemmas 18 · TTR 0.95
  Zipf slope -0.74 (R² 0.79)

  frequency (by surface, top 20):
      2  и
      1  дуб
      …
```

In four lines of Pushkin almost every word is unique — a *type–token ratio* near 1,
the signature of dense poetry — and the little conjunction `и` "and" already leads, as
function words always do. The `--lemma` flag is where a corpus and a grammar meet: it
counts a root's inflected forms *together*, so `цепь` and `цепи` fall under one lemma —
but only if you have declared the endings that connect them (Chapter 4). A frequency
list by lemma is a frequency list of the *vocabulary*; by surface, of the *word-forms*.
The gap between the two is a measure of how inflected the language is, and for Russian
it is large.

#term("Zipf's law")[
  The empirical regularity that in any large body of text a word's frequency is
  roughly inversely proportional to its frequency rank — the second-commonest word
  occurs about half as often as the commonest, the third a third as often, and so on.
  Plotted on log axes it is a straight line of slope ≈ −1. It holds across every human
  language and is one of the most robust quantitative facts we have; a corpus that does
  not show it is too small, or not language.
]

Point `frequency` at a whole novel and the Zipf slope tightens toward −1 and the fit
`R²` toward 1. On four lines it only gestures at the curve; the law is a fact about
*scale*, and it is the first thing a real corpus buys you.

#section("A word in its contexts")

Counting tells you *how much*; to see *how* a word is used, read it in context.
`concordance` lines up every occurrence with its neighbours — the *KWIC*
(keyword-in-context) display, the oldest tool in corpus linguistics:

```sh
inkhaven language concordance Russian --word цепи --window 3
```

```
  златая цепь на дубе  [том]   и днём и
  всё ходит по         [цепи]  кругом
```

A lexicographer reads a page of KWIC lines before writing a single definition,
because a word's meaning is the sum of its contexts. Add `--lemma` and the search
becomes a search for the *word*, not the *form*: a query on the lemma `цепь` gathers
`цепь`, `цепи`, `цепью` and the rest into one concordance — the inflected paradigm
reunited on the page. This is why the morphology of Chapter 4 matters here: without it,
Russian's endings scatter each word across a dozen entries; with it, the corpus can put
them back together.

#section("The company a word keeps")

Some words simply *go together* — not by grammar, but by habit. `collocations` finds
them: the words that fall within a window of the target more than chance would predict,
ranked by how *distinctive* the pairing is:

```sh
inkhaven language collocations Russian --word кот --window 4
```

The ranking is not raw co-occurrence — the commonest words co-occur with everything.
It is *pointwise mutual information*, which asks how much *more* a word appears beside
the target than its overall rate would predict, so a genuine partner outranks a mere
frequent neighbour. Russian *кот учёный* "learned cat" is a collocation; *умный кот*
"smart cat" is just two words. Collocations are much of what makes speech sound native,
and a corpus is the only place to find them, because they live in usage and not in any
definition.

#term("Collocation")[
  A habitual pairing of words stronger than chance — *проливной дождь* "pouring rain",
  not *сильный дождь*; *strong tea*, not *powerful tea*. Collocations are learned, not
  derived; they vary between languages that agree on grammar, and mastering them is
  much of what separates a fluent speaker from a correct one. The measure Inkhaven
  ranks them by, *pointwise mutual information*, is the standard association statistic.
]

#section("Where a word lives in the text")

A frequency count treats a text as a bag of words, but words are not spread evenly. A
term that appears ten times *in one paragraph* and nowhere else is doing something
different from one that appears ten times *throughout*. Reading the concordance shows
this directly — the `text` column of each KWIC line tells you *which* text, and *which*
paragraph, an occurrence comes from, so a word clustered in one passage stands out from
one that recurs everywhere. Over a multi-text corpus this is how you separate a word
that belongs to the *language* from one that belongs to a single *scene*.

#callout(label: "From poem to library")[
  A four-line poem is a corpus you can read at a glance; its value here is as a
  demonstration. The same four questions — frequency, context, collocation, spread —
  scale without change to a whole Gutenberg library: drop the texts into your
  manuscript, and `frequency`, `concordance` and `collocations` read all of it. That
  is when the Zipf curve straightens, the collocations become real, and the statistics
  become something you can publish.
]

#recap((
  [A *corpus* is real usage made countable, drawn from your stored interlinears
   (`--source texts`) or your manuscript prose (`--source prose`); paste public-domain
   Russian (Project Gutenberg) in and Inkhaven gathers its words as tokens.],
  [`frequency` gives word counts plus tokens, types, type–token ratio and the Zipf fit;
   `--lemma` counts a root's inflected forms together (which needs the morphology of
   Chapter 4), turning a form-list into a vocabulary-list.],
  [`concordance` shows a word in every context (KWIC), `--lemma` reuniting a paradigm;
   `collocations` ranks a word's habitual partners by pointwise mutual information —
   the idiomatic pairings a dictionary can't give.],
))
