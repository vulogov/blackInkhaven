#import "../design.typ": *

#chapter(number: 6, title: "A corpus of Russian")

Everything so far has worked from a lexicon and a grammar — the language in the
abstract. But a language *lived in* leaves traces: the words people actually use, in
the proportions they actually use them, next to the words they actually put them near.
That body of real usage is a *corpus*, and it answers questions no dictionary can. In
this chapter we build one from a real Russian text and interrogate it.

#section("A text to work from")

Project Gutenberg holds the Russian classics in the public domain — Pushkin, Chekhov,
Tolstoy, Dostoevsky — free to download and study. We take the most famous opening in
Russian poetry, the prologue to Pushkin's #emph[Ruslan and Lyudmila] (1820):

```
У лукоморья дуб зелёный;
Златая цепь на дубе том:
И днём и ночью кот учёный
Всё ходит по цепи кругом;
```

Paste a passage like this into a chapter of your manuscript — an ordinary prose
paragraph, in Cyrillic — and it becomes part of your corpus. Inkhaven finds the
Russian words in it the same way it finds invented words in a conlanger's prose: a
paragraph counts once it contains a word your lexicon knows, and within it every word
that reads as Russian is gathered as a token.

#term("Corpus")[
  A structured body of real language — texts, gathered so they can be counted and
  searched. Corpus linguistics is the study of language as it is actually used rather
  than as a grammar idealizes it, and much of what we know about frequency, collocation
  and change comes from it. A corpus need not be large to be useful; a single poem is
  enough to start.
]

#section("What is frequent?")

`frequency` reads the corpus and reports how often each word occurs, with the
descriptive statistics real usage unlocks:

```sh
inkhaven language frequency Russian --source prose
```

```
  texts 1 · tokens 19 · types 18 · lemmas 18 · TTR 0.95
  Zipf slope -0.74 (R² 0.79)

  frequency (by surface, top 20):
      2  и
      2  цепи
      1  дуб
      …
```

In four lines of Pushkin almost every word is unique — a *type–token ratio* near 1,
the signature of dense poetry — and the little conjunction `и` "and" already leads,
as function words always do in any language. Point the same command at a whole novel
and the Zipf curve straightens into the near-perfect power law that is one of the most
robust facts about human language.

#term("Type–token ratio")[
  The number of distinct words (*types*) divided by the total number of words
  (*tokens*). A high ratio means little repetition — varied, dense text; a low one
  means the same words recur, as in casual speech. It is the standard first measure of
  lexical variety, and it falls predictably as a text grows.
]

#section("A word in its contexts")

To see how a word is *used*, not just how often, ask for a concordance — every
occurrence lined up with its neighbours:

```sh
inkhaven language concordance Russian --word цепи --window 3
```

```
  златая цепь на дубе  [том]   и днём и
  всё ходит по         [цепи]  кругом
```

This is the *KWIC* (keyword-in-context) view a lexicographer reaches for before
writing a definition — you read the word's actual company and let its meaning emerge
from use. Add `--lemma` and the search gathers every inflected form of a root at once,
so a query on the lemma `цепь` finds both `цепь` and `цепи` — provided you have
declared the endings that connect them.

#section("The company a word keeps")

`collocations` goes one step further and reports which words a word *habitually*
appears beside — its collocates — ranked not by raw count but by how distinctive the
pairing is:

```sh
inkhaven language collocations Russian --word кот --window 4
```

Collocations are much of what makes a language sound native — Russian *кот учёный*
"learned cat", not *умный кот* — and a corpus is the only place to find them, because
they live in usage, not in any single definition. Over a real body of text the strong
collocations of a word are as revealing as its dictionary entry.

#callout(label: "From poem to library")[
  A four-line poem is a corpus you can read at a glance; its value here is as a
  demonstration. The same three commands scale without change to a whole Gutenberg
  novel — drop the text into your manuscript, and `frequency`, `concordance` and
  `collocations` read all of it. That is when the statistics become trustworthy and
  the collocations become real.
]

#recap((
  [A *corpus* is real usage made countable; paste a public-domain Russian text (Project
   Gutenberg) into your manuscript and Inkhaven gathers its Russian words as tokens.],
  [`frequency` reports word counts plus tokens, types, type–token ratio and the Zipf
   fit; `concordance` shows a word in every context (KWIC), `--lemma` gathering a
   root's inflected forms.],
  [`collocations` finds the words a word habitually keeps company with, ranked by how
   distinctive the pairing is — the idiomatic pairings a dictionary can't give.],
))
