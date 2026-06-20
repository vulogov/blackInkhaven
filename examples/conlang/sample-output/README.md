# Sample output — Avesha

These are the **actual artifacts** produced by
[`../build-sample-language.sh`](../build-sample-language.sh) in one real run,
checked in so you can see what the ConLang Suite generates without running it
yourself.

The glyphs were **drawn by an AI** (DeepSeek `deepseek-chat`) from one-line
descriptions, each passed through the suitability preflight before being bound.
The font was compiled in-process; the three books were rendered to Typst and
compiled with Typst 0.14.2.

| File | What it is |
|---|---|
| `Avesha.ttf` | the compiled conscript font (11 glyphs, one per phoneme) |
| `glyph-*.svg` | the AI-drafted glyph artwork (one per phoneme) |
| `avesha-dictionary.{typ,md,pdf}` | two-column A5 dictionary; headwords in the native script (3 pp) |
| `avesha-grammar.{typ,md,pdf}` | reference grammar with a table of contents (5 pp) |
| `avesha-tutorial.{typ,md,pdf}` | graded learner walkthrough (5 pp) |

The `.typ` books reference the font by family name; recompile any of them with:

```sh
typst compile --font-path . avesha-dictionary.typ
```

> The AI output varies run to run, so a fresh run will produce different glyph
> shapes (and the books will look slightly different). This snapshot is one
> representative result.
