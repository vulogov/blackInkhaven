# ConLang example — a complete language, end to end

[`build-sample-language.sh`](build-sample-language.sh) builds a full constructed
language inside an inkhaven project and produces three finished books. It is the
worked example for the ConLang Suite (RFC LANG-1; reference
[`../../Documentation/CONLANG.md`](../../Documentation/CONLANG.md)).

```sh
./build-sample-language.sh <project-path> [language-name]
```

It runs the whole pipeline as real `inkhaven language` commands:

1. **Phonology** — inventory, syllable templates, phonotactics, allophony
   (`t → s / _ i`, `n → m / _ p`), penultimate stress.
2. **Lexicon** — ten words across nouns, verbs, and adjectives; audited for
   homophones and duplicate meanings.
3. **Morphology** — dative + plural suffixes and an agent-noun derivation;
   allophony fires across the affix boundary (`pata` + DAT → `pata`**`s`**`i`).
4. **Typology + expressions** — SOV, nominative–accusative, postpositions; an
   idiom and a metaphor.
5. **A writing system, drawn by the AI** — one glyph per phoneme is drafted by
   `glyph-draft` from a short description, run through the suitability preflight,
   and bound to its sound + a Private-Use codepoint.
6. **A font** — the script is compiled to a TrueType file, fully in-process.
7. **Three books** — a **dictionary**, a **reference grammar**, and a **learner
   tutorial**, each as a Typst document that embeds the font (and a Markdown
   edition), compiled to PDF if `typst` is on PATH.

## Requirements

- The `inkhaven` binary on PATH (or set `$INKHAVEN`).
- An AI provider for the glyph drafting. Set `$PROVIDER` (default `deepseek`)
  and the matching key (e.g. `DEEPSEEK_API_KEY`). Any provider inkhaven supports
  works; see `inkhaven language glyph-draft --help`.
- Optional: [Typst](https://typst.app) on PATH to compile the `.typ` books to
  PDF.

## Output

Everything lands in `<project-path>/conlang-out/`:

```
avesha-dictionary.{typ,md,pdf}   # two-column A5 book, native-script headwords
avesha-grammar.{typ,md,pdf}      # reference grammar with a table of contents
avesha-tutorial.{typ,md,pdf}     # graded learner walkthrough
Avesha.ttf                       # the compiled conscript font
glyph-*.svg                      # the AI-drafted glyph artwork
```

The AI output varies run to run; every glyph is preflight-gated, so an
occasional unusable draft is skipped (and reported) rather than breaking the
build.

A checked-in result from one real run lives in
[`sample-output/`](sample-output/) — the AI-drawn font, the glyph SVGs, and all
three books (including the compiled PDFs) — so you can see what the script
produces without running it.
