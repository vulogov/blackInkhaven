# Developing a Constructed Language with Inkhaven

A complete, beginner-friendly guide to building a constructed language (conlang)
with Inkhaven's ConLang Suite — from an empty project to a finished language with
its own dictionary, grammar, writing system, and printed books.

It assumes **no prior knowledge**: every linguistic term is defined where it
first appears, and the book starts from installing Inkhaven. By the end, a reader
who knew neither linguistics nor Inkhaven can build their own language.

## Reading it

The compiled book is [`CONLANG_DEVELOPMENT.pdf`](CONLANG_DEVELOPMENT.pdf)
(~156 pages, B5). To rebuild it from source you need [Typst](https://typst.app):

```sh
typst compile CONLANG_DEVELOPMENT.typ CONLANG_DEVELOPMENT.pdf
```

It uses only fonts Typst bundles, so it compiles warning-free with no font setup.

## Structure

- **Master file:** [`CONLANG_DEVELOPMENT.typ`](CONLANG_DEVELOPMENT.typ) — includes
  every chapter and sets the reading order.
- **Design system:** [`design.typ`](design.typ) — page chrome, the cover (using
  the project logo), and the helper boxes (term definitions, callouts, recaps).
- **Chapters:** [`chapters/`](chapters/) — one file per chapter, grouped into
  ten parts plus three appendices.

## What it covers

1. **Foundations** — what a conlang is, what Inkhaven is, getting set up.
2. **The sounds** — phonemes, syllables, phonotactics, allophony, stress.
3. **Words** — building and maintaining a lexicon.
4. **Grammar** — morphology, word-building, typology, idioms.
5. **A history** — sound change, proto-languages, language families.
6. **A language in a world** — dialects and registers, contact and borrowing,
   speech communities and language ecology.
7. **A writing system** — designing glyphs and compiling a real font.
8. **The books** — producing the dictionary, grammar, and tutorial.
9. **Sharing your language** — interchange with other linguistics tools.
10. **A complete walkthrough** — building one language end to end.

Appendices: a full command reference, a configuration-block reference, and a
glossary of every linguistic term used.

The book documents the ConLang Suite (RFC LANG-1); the canonical command
reference is [`Documentation/CONLANG.md`](../../Documentation/CONLANG.md).
