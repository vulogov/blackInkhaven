#import "../design.typ": *

#chapter(number: 32, title: "Interlinear glossed text")

A dictionary lists words in isolation; a grammar states rules in the abstract. What
a language *is* lives in between, in sentences — and the way linguists write a
sentence down so a reader who doesn't speak the language can follow it is
*interlinear glossed text*: the sentence, a gloss under each word, and a free
translation, lined up in columns. It is the single most characteristic artifact of
linguistic description, and Inkhaven builds it for you.

#section("Glossing a sentence")

`igt` takes a sentence of your language and lays it out as a Leipzig interlinear:

```sh
inkhaven language igt Eldar --text "katat nilo"
```

```
kata-t     nilo
stone-DAT  friend
'stone friend'
```

The top line is the sentence *segmented into morphemes* — split at every affix
boundary — and the line under it glosses each morpheme, aligned. This reuses the
auto-gloss engine, so every inflected form is recognised, and it is careful about
the hard case: an affix whose sound changes at the boundary is still split
correctly. A dative *kata + d* that surfaces as *katat* after your final-devoicing
rule is segmented `kata-t`, not `kata-d` — Inkhaven tracks each morpheme's span and
re-slices the surface *after* allophony has run. The third line is a *literal*
translation, the recognised words' lexical meanings, offered as a scaffold you will
rewrite. Words it doesn't recognise — a name, a coinage you haven't defined — pass
through untouched.

#term("Interlinear glossed text (IGT)")[
  The standard three-part format for presenting an example from any language: the
  (morpheme-segmented) text, a morpheme-by-morpheme gloss using standard
  abbreviations (`PL`, `DAT`, `PST`…), and a free translation in quotes. The
  alignment lets a reader see exactly how the meaning is assembled — which is why
  every grammar and every linguistics paper is full of it.
]

#section("Keeping your texts")

A glossed sentence is worth keeping. `--save` stores an interlinear in a `Texts`
chapter of your language book, next to its phonology and lexicon:

```sh
inkhaven language igt Eldar --text "katat nilo" --save --name the-oath-1
```

`inkhaven language texts Eldar` then lists what you have gathered, and `--name`
prints one back. A stored text lives in the tree — versioned, navigable, and
part of what makes the language *documented*. Over time these texts become a small
corpus, and the corpus tools you met while measuring your language — `frequency`,
`concordance`, `collocations` — read them (together with the conlang words in your
manuscript prose) to answer questions about how the language is actually used.

#section("Curating the translation")

The literal translation is only a starting point. Replace it with the sentence as it
should read:

```sh
inkhaven language texts Eldar --name the-oath-1 \
  --set-translation "The oath was sworn upon the stone."
```

Only the free translation changes; the morpheme segmentation and its gloss are left
exactly as they were. In the companion the same edit is `/settrans the-oath-1 = The
oath was sworn upon the stone.`, and `/texts` lists what you have stored.

#section("Out to the page")

When you want your texts in a grammar sketch or a paper, `--format latex` renders
them as a `linguex` document — each text a numbered example with `\gll` (the
segmented sentence over its gloss) and `\glt` (the translation), ready to paste:

```sh
inkhaven language texts Eldar --format latex
```

Every LaTeX special is escaped, and a multi-word gloss is bound so the two lines stay
aligned. What you glossed in the terminal comes out as the numbered, typeset examples
a linguist expects.

#recap((
  [`igt <lang> --text "…"` glosses a sentence as an aligned interlinear: the
   morpheme-segmented sentence, a gloss under each morpheme, and a literal
   translation.],
  [`--save --name N` keeps a text in the language's `Texts` chapter; `texts <lang>`
   lists them and `--name N` prints one — a growing corpus of documented sentences.],
  [`texts --name N --set-translation "…"` curates a text's free translation without
   touching its gloss; `--format latex` exports the texts as a `linguex` document.],
  [In the companion, `/igt`, `/texts` and `/settrans` do the same inline.],
))
