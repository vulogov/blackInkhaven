# Sample output — Avesha

These are the **actual artifacts** produced by
[`../build-sample-language.sh`](../build-sample-language.sh) in one real run,
checked in so you can see what the ConLang Suite generates without running it
yourself.

The glyphs were **drawn by an AI** (DeepSeek `deepseek-chat`) from one-line
descriptions, each passed through the suitability preflight before being bound.
The font was compiled in-process; the three books share a manual-style B5 design
and were compiled with Typst 0.14.2.

| File | What it is |
|---|---|
| `Avesha.ttf` | the compiled conscript font (11 glyphs, one per phoneme) |
| `glyph-*.svg` | the AI-drafted glyph artwork (one per phoneme) |
| `avesha-dictionary.{typ,md}` | a B5 dictionary; headwords in the native script |
| `avesha-grammar.{typ,md}` | a reference grammar led by an AI-written **study guide** that explains its linguistic terms |
| `avesha-tutorial.{typ,md}` | an AI-written graded **learner's textbook** |

The `.typ` books reference the font by family name; the PDF is one compile away
(PDFs aren't checked in — they're build artifacts):

```sh
typst compile --font-path . avesha-dictionary.typ
```

> The AI output varies run to run, so a fresh run will produce different glyph
> shapes (and the books will look slightly different). This snapshot is one
> representative result.
