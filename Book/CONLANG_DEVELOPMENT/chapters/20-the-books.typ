#import "../design.typ": *

#chapter(number: 20, title: "Producing the books")

Everything you have built — sounds, words, grammar, history, a script — can now be
turned into finished, printable books. Inkhaven produces four kinds of document
from your language's own data: a *dictionary*, a *reference grammar*, a *study
guide*, and a learner's *tutorial*. Each comes in two flavours: plain Markdown, or
a beautifully typeset book (the same B5 design as the book in your hands), ready
to compile into a PDF that embeds your conscript font.

#section("A quick word about output formats")

Every output command takes `--format md` or `--format typ`. *Markdown* (`md`) is
plain text you can read anywhere or paste into other tools. *Typst* (`typ`)
produces a typeset book; you turn it into a PDF with the free Typst program:

```sh
typst compile --font-path <folder-with-your-ttf> book.typ book.pdf
```

The `--font-path` points Typst at the folder holding your `Eldar.ttf`, so the
PDF can render your script. When a command takes `--font Eldar`, it embeds that
font and shows your words in their native form.

#section("The dictionary")

```sh
inkhaven language dictionary Eldar --format typ --font Eldar --out dict.typ
```

This renders your lexicon as a real dictionary: a title page, a table of contents,
an overview of the language, and a two-column, alphabetised list of every word
with its pronunciation, part of speech, and meaning. When you pass `--font`, each
headword also appears in your native script beside its romanized form. Compile it
with Typst and you have a printable dictionary of your language.

#section("The reference grammar")

```sh
inkhaven language grammar-book Eldar --format typ --font Eldar --out grammar.typ
```

The companion volume: a reference grammar drawn entirely from your language's
data — the phonology (inventory, syllable structure, phonotactics, allophony,
stress, tone), the morphology (affixes and derivations), the typology answers,
the idioms and metaphors, and the sample texts. It is a faithful, deterministic
description of exactly what you built.

In the morphology section, affixes are *grouped by category* — all the case
endings together, all the number endings together — using the `category` and
`value` tags you gave each morpheme (Chapter 11), with each one's kind (prefix,
suffix, infix, circumfix, ablaut, reduplication) shown. Any agreement rules get
their own short section. The more carefully you tag your morphemes, the clearer
this grammar reads.

#section("The study guide (AI)")

A bare reference grammar can be daunting to a newcomer who does not know what
"allophony" or "nominative–accusative alignment" mean. Add `--study` and Inkhaven
prepends an AI-written *study guide* that defines and explains every linguistic
term the grammar uses, grounded in your language's own examples:

```sh
inkhaven language grammar-book Eldar --format typ --font Eldar \
    --study --provider deepseek --out grammar.typ
```

The reference itself stays exactly as before — only the study guide is
AI-written, and it needs an AI provider. It turns a technical reference into
something a beginner can actually learn from.

#section("The learner's tutorial (AI)")

Finally, the gentlest on-ramp of all — a complete beginner's *textbook* for your
language, written by the AI from your data:

```sh
inkhaven language tutorial Eldar --format typ --font Eldar \
    --provider deepseek --out learn.typ
```

The model authors a graded course: a warm introduction, a pronunciation guide,
lessons that *explain* the grammar with worked examples drawn from your words, a
reading passage, and a practice exercise per lesson. It is constrained to your
language's actual sounds, words, and rules — it never invents vocabulary or
grammar. This is the book you would hand someone who wants to *learn* your
language, rather than look things up in it.

#callout(label: "Deterministic where it counts, AI where it helps")[
  Notice the division of labour. The dictionary and the grammar reference are
  generated *deterministically* — they are an exact, trustworthy description of
  your data. The study guide and the tutorial, where the value is in *teaching
  prose*, are AI-written. You always get an accurate reference; you optionally get
  a friendly teacher on top. Both kinds of book embed your font and look the same
  on the shelf.
]

#section("Sharing your lexicon with other tools")

Your language need not live only inside Inkhaven. The `export` command writes the
lexicon to portable formats:

```sh
inkhaven language export Eldar --format xliff     > eldar.xlf
inkhaven language export Eldar --format linguex   > eldar.tex
inkhaven language export Eldar --format ipa-chart
```

- *xliff* is a translation-memory file that loads into computer-assisted
  translation tools (OmegaT, memoQ, Weblate), with each entry as a
  working-language → invented-word pair.
- *linguex* is LaTeX for the `linguex` package — bold headwords with glosses and
  numbered examples, ready to drop into a paper or grammar sketch.
- *ipa-chart* is a Markdown inventory of your sounds.

(The older `json`, `anki`, and `csv` formats are still there for flashcards and
spreadsheets.) Going the other way, `import` pulls a lexicon *in* from another
tool — Toolbox / Lexique Pro Standard-Format databases, or a PolyGlot dictionary:

```sh
inkhaven language import Eldar --file lexicon.sfm --format toolbox        # preview
inkhaven language import Eldar --file lexicon.sfm --format toolbox --yes  # write
```

Import previews what it would add and writes nothing until you pass `--yes`, so
you always see the entries before they land in your Dictionary.

#section("Creative text: names, verse, and ritual")

For the fun of hearing the language speak, `compose` generates text grounded in
what you have built:

```sh
inkhaven language compose Eldar --kind names --count 6
inkhaven language compose Eldar --kind prose --count 3
inkhaven language compose Eldar --kind poem  --meter 5,7,5
```

`names` draws phonotactic, capitalised names; `prose` assembles real grammatical
sentences through the syntax engine (with glosses); `poem` writes metered verse
that actually scans against your syllable counts. The themed kinds — `blessing`,
`curse`, `incantation` — are AI-composed but *constrained to your existing
lexicon*, so the model arranges real words rather than inventing them, and prints
the native text with a gloss and a translation. Everything `compose` makes is
printed for you to keep or discard; nothing is written into your books.

#recap((
  [Four documents come from your language: *dictionary*, *grammar reference*,
   *study guide*, and *tutorial*.],
  [`--format md` is plain text; `--format typ` is a typeset book you compile with
   `typst compile --font-path … `.],
  [`--font Eldar` embeds your script; dictionary and grammar are *deterministic*.],
  [`grammar-book --study` and `tutorial` are *AI-written* teaching materials (need
   a provider).],
  [`export` writes your lexicon to *xliff*, *linguex*, *ipa-chart* (and json /
   anki / csv); `import` reads Toolbox/Lexique and PolyGlot back in.],
  [`compose` generates names, prose, and verse — grounded, deterministic, with
   AI-composed blessings/curses constrained to your lexicon.],
))
