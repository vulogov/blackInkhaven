# Tutorial 110 — The Inner Poet

*Inkhaven 1.8.x (POEM)*

Tutorial 100 wrote a haiku. The Inner Poet is the full companion for verse: it
*observes and measures* a poem against its declared form — metre, rhyme,
syllable counts, structural completion — and, on the slow track, offers an LLM
observation. It follows the family's cardinal rule: **it never generates or
rewrites a line of verse.** You write the poem; it reads it.

## Declare a form

Poetry lives in the `para:verse-*` structural family. A poem is measured against
a **declared form** — a `poem:` block beside the stanza. Scaffold one from the
CLI:

```sh
inkhaven poetry forms                 # list the built-in forms
inkhaven poetry forms sonnet en       # print a sonnet's poem: block for English
```

Or, in the editor, open the Inner Poet on a verse paragraph and declare a form
interactively (below).

## Measure a stanza

```sh
inkhaven poetry scan --text "$(cat sonnet.txt)" --form sonnet
```

```
Sonnet — line 1  / × / × / × / × / ×   iambic pentameter ✓
         line 9  / × / × / × × / / ×   Concern: two substitutions in the volta line
Rhyme  ABAB CDCD EFEF GG  — matches (near-rhyme at line 12: "gone / dawn")
```

The glyphs are the scansion: `/` stressed, `×` unstressed, `·` flexible. Other
diagnostics: **`poetry syllabify`** (syllable boundaries + stress),
**`poetry metre`** (scan one line, detect its metre), **`poetry rhyme`**
(classify the rhyme between two words), and **`poetry status`** (a poem's
completion against its form — line ratio, missing refrains).

## In the editor — `Ctrl+B J → P`

Open a verse paragraph and press **`Ctrl+B J → P`** for the Inner Poet, with its
own sub-keys:

- **`F`** — fast-scan metre + rhyme against the declared form → Output pane
  (Praise / Note / Concern). Deterministic, free.
- **`E`** — engage the LLM slow track: an observation on enjambment, sound
  texture, caesura, and (for a sonnet) the turn — never a rewrite.
- **`D`** — declare a form (a picker that writes the language-localised `poem:`
  block).
- **`T`** — the two-column translation view (source ∥ translation).
- **`A`** — ambient: auto fast-scan each verse paragraph as you open it.

While a verse paragraph is open, the status bar shows a live readout — the
current line's syllable count and its position (`♩ 8 syl · l2/4`) — and the
outline shows completion chips (`8/14`, `14/14 ✓`).

## Translation — the trilemma

Verse translation trades off Form, Sound, and Meaning. The trilemma view (and
`inkhaven poetry trilemma`) compares a source stanza and its translation on
**Form** (metre + rhyme) and **Sound** (alliteration) deterministically; the
**Meaning** axis is the AI's, in the editor.

## Works in your language

Metre reuses the ConLang scansion engine; syllabification uses Typst's `hypher`.
Russian scansion is exact; English scansion improves further if you install a
phoneme dictionary (`inkhaven poetry phonemes import <cmudict>`), else it uses an
accent-mark heuristic (`compáre` to fix a stress).

---

**See also:** [KEYBINDING.md → `Ctrl+B J → P`](../KEYBINDING.md) ·
Tutorial 100 (haiku) · `inkhaven poetry --help`.
