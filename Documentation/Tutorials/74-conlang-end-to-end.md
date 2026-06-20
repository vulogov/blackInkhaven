# Tutorial 74 — A constructed language, end to end

*Inkhaven 1.3.16+*

The ConLang Suite (RFC LANG-1) turns inkhaven into a constructed-language
workbench: phonology, lexicon, morphology, diachronics, a writing system with a
real compiled font, and book-quality output — all under `inkhaven language`,
all reference-documented in [`CONLANG.md`](../CONLANG.md). This tutorial walks
the whole pipeline by building one small language, **Avesha**, from nothing to
three finished PDFs.

There is a runnable script for everything below:
[`examples/conlang/build-sample-language.sh`](../../examples/conlang/build-sample-language.sh).
Run it, then read this to understand each step.

```sh
INKHAVEN=inkhaven PROVIDER=deepseek \
  examples/conlang/build-sample-language.sh ~/avesha-project
```

## 1. Phonology

A language starts with its sounds. The inventory, syllable templates,
phonotactics, allophony, and stress live in a `{ … }` HJSON block in the
**Phonology** chapter of the language book:

```hjson
{
  phonemes: [ { ipa: "t", kind: "consonant" } { ipa: "i", kind: "vowel" } … ]
  classes:  { C: ["p","t","k","s","m","n","l","r"], V: ["a","i","u"] }
  templates: { root: [ { pattern: ["C","V"] }, { pattern: ["C","V","C"] } ] }
  constraints: [ { kind: "no_geminate" }, { kind: "max_cluster_size", value: 2 } ]
  allophony: [ { rule: "t > s / _ i" } ]   // t palatalises before i
  stress: { primary: "penultimate" }
}
```

> A constraint's discriminator key is `kind:` (not `type:`).

`inkhaven language stats Avesha` then prints a descriptive profile — the
consonant/vowel balance, phoneme frequency, the syllable-length distribution,
which onsets and codas get used. The phonology inspectors (`ipa`, `syllabify`,
`stress`, `romanize`) show the engine working on any word; `language ipa Avesha
--word tati` reveals the surface form after allophony.

## 2. Lexicon

```sh
inkhaven language add-word Avesha pata --type noun --translation stone
```

`language audit` runs the deterministic half of the dedup gate — headwords that
break the phonotactics, homophones (entries that collide *after* allophony), and
duplicate meanings. (The AI generator, `language generate-lexicon`, coins whole
batches behind the same gate plus an embedding-based near-synonym check.)

## 3. Morphology

Affixes and derivations live in the **Grammar** chapter:

```hjson
{
  morphemes: [
    { id: "DAT", gloss: "DAT", form: "ti", position: "suffix", category: "case", value: "dative" }
  ]
}
```

Inflection runs the allophony engine across the affix boundary, so `pata` + DAT
(`ti`) surfaces as **patasi** — the `t→s/_i` rule fires at the join. The
tutorial output shows this paradigm automatically.

## 4. A writing system — drawn by the AI

Describe a glyph and the model draws it:

```sh
inkhaven language glyph-draft Avesha \
    --describe "a tall cross, one vertical stroke crossed near the top" \
    --phoneme t --codepoint U+E001 --name t --provider deepseek --yes
```

Every draft is run through the **suitability preflight** before it can be bound:
it must be filled vector paths (no strokes, rasters, or gradients), and the
prompt tells the model to cut counters with reverse-wound subpaths rather than
white fills. The draft is advisory — `--yes` binds a *usable* result; an
unusable one is skipped and reported.

Hand-drawn artwork imports the same way (`font-import-glyph --svg`), and you can
inspect any SVG first with `glyph-lint`.

## 5. The font

```sh
inkhaven language font-build --language Avesha --format ttf --out Avesha
```

This compiles the bound glyphs to a TrueType font **fully in-process** — the SVG
outlines become a UFO, then a complete OpenType table set, with no external
tool. Type the script with `language transliterate Avesha --text kira` (romanized
→ glyph codepoints, longest key first so digraphs win).

For block scripts (Hangul-style syllable squares, hieroglyphic quadrats), see
`font-compose` (bake a precomposed block) and `spatial-typst` (arrange
components at layout time) — one `SpatialTemplate` engine, two binding times.

## 6. The books

```sh
inkhaven language dictionary   Avesha --format typ --font Avesha --out dict.typ
inkhaven language grammar-book Avesha --format typ --font Avesha --out grammar.typ
inkhaven language tutorial     Avesha --format typ --font Avesha --out learn.typ
```

Each renders in Markdown or Typst. The **dictionary** is a paginated two-column
A5 book whose headwords appear in the native script beside their romanization.
The **grammar** is a reference volume with a table of contents — phonology,
morphology, typology, expressions, sample texts. The **tutorial** is a graded
learner walkthrough — the sounds, a starter vocabulary, a worked paradigm, and a
glossed sample sentence. Build the font and compile any of them with `typst
compile --font-path <dir> dict.typ` to get a PDF that embeds and renders the
conscript.

## Where to go next

- The full command surface and every HJSON block: [`CONLANG.md`](../CONLANG.md).
- Diachronics — evolve a daughter language from a proto via sound-change chains
  (`sound-change`, `derive-lexicon`, `family-tree`, `cognates`).
- The editor: `Ctrl+B X` opens the ConLang hub; type `:lang:` in a manuscript to
  insert a word from the lexicon.
