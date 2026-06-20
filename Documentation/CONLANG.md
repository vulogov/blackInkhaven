# ConLang Suite reference

The constructed-language workbench (RFC LANG-1), introduced in **1.3.14** and
continuing across 1.3.x. It layers on the existing `Language` system book
(1.2.13): each language is a `Book` under **Language**, scaffolded by
`inkhaven language init <name>` with `Meta / Dictionary / Grammar / Phonology /
Sample texts` chapters. The engines reconstruct an in-memory model from typed
HJSON blocks you add to those chapters; the book stays the home of record.

> **Authoring tip.** In HJSON an unquoted string runs to end-of-line, so quote
> inline enum values: `kind: "consonant"`, `position: "suffix"`. The parser
> gives a clear error otherwise.

## Where each block lives

| Block | Chapter | Drives |
|---|---|---|
| phoneme inventory / classes / templates / constraints / allophony / stress / tone / romanization | **Phonology** | the phonology engine |
| morphemes / paradigms | **Grammar** (or a hand-added Morphology chapter) | paradigm generation |
| dictionary entries (one HJSON paragraph each) | **Dictionary** | lexicon, overlay, generation |
| `iso_code`, alphabet, world context | **Meta/overview** | `:lang:` resolution, buckets |

## Phonology (the `Phonology` chapter)

```hjson
{
  phonemes: [
    { ipa: "p", romanize: "p", kind: "consonant" }
    { ipa: "ʃ", romanize: "sh", kind: "consonant" }
    { ipa: "a", romanize: "a", kind: "vowel" }
  ]
  classes: { C: ["p", "ʃ"], V: ["a"] }            // named phoneme classes
  templates: { root: [ { pattern: "C V (C)", weight: 1.0 } ] }
  constraints: [
    { kind: "max_cluster_size", value: 2 }
    { kind: "no_geminate" }
    { kind: "forbid_in_coda", classes: ["Stop"] }  // syllable-aware
    { kind: "sonority_sequencing" }
  ]
  allophony: [ { name: "palatalization", rule: "k > tʃ / _ i" } ]
  stress: "penultimate"                            // initial|final|penultimate|antepenultimate|latin
  romanizations: [
    { name: "default", mappings: [ { ipa: "k", roman: "c" }, { ipa: "s", roman: "c" } ],
      contextual: [ { roman: "c", ipa: "s", before: "FrontV" } ] }
  ]
  default_romanization: "default"
  tone: { kind: "contour", tones: ["1","2","3","4"], sandhi: [ { rule: "3 > 2 / _ 3" } ] }
}
```

**SPE rule notation** (allophony, tone sandhi): `LHS > RHS / LEFT _ RIGHT`.
`_` marks the target, `#` a word boundary, `∅`/`0` the empty string (insertion
on the left, deletion on the right). A context token is a class name when one
is declared, else a literal phoneme.

Inspectors: `generate-word`, `syllabify --word`, `ipa --word` (surface),
`stress --word`, `romanize --text [--reverse] [--scheme]`, `tone --tones`.

## Morphology (the `Grammar` chapter)

```hjson
{
  kind: "agglutinative"
  morphemes: [
    { id: "pl",  gloss: "PL",  form: "i",  position: "suffix", precedence: 2 }
    { id: "dat", gloss: "DAT", form: "d",  position: "suffix", precedence: 1 }
    { id: "def", gloss: "DEF", form: "na", position: "prefix" }
  ]
  paradigms: [ { name: "noun", cells: [
    { features: { number: "sg", case: "nom" }, morphemes: [] }
    { features: { number: "pl", case: "dat" }, morphemes: ["dat", "pl"] }
  ] } ]
}
```

`inkhaven language paradigm <lang> --root kata --template noun --gloss stone`
applies each cell's morphemes to the root, runs allophony across the affix
boundaries, and prints the form + Leipzig gloss. (P3.1 covers prefix + suffix.)

Optional **`precedence`** on a morpheme controls how close it sits to the root
when several affixes of the same side stack: `0` (the default) = any position,
keeping the declared order; `1` = immediately next to the root; `2` = the next
slot out; and so on. Above, the case suffix (`dat`, precedence 1) hugs the root
and the number suffix (`pl`, precedence 2) sits outside it, regardless of the
order the cell lists them — `kata` + DAT + PL → `katadi`, glossed `stone-DAT-PL`.

**Auto-gloss.** A dictionary entry can declare the paradigm it inflects by
(`paradigm: "noun"`); then `inkhaven language gloss <lang> --text "kata katai
katat"` prints an aligned interlinear (the words over their Leipzig glosses).
It recognises inflected *and* allophony-altered forms (`katat` → `stone-DAT`)
by generating each entry's paradigm forward and matching.

**Derived forms.** A `derivations` list in the Morphology block coins new
lexemes (agent nouns, diminutives, …):

```hjson
derivations: [
  { name: "agent", form: "ron", position: "suffix", from_pos: "verb",
    to_pos: "noun", gloss_template: "one who {}s" }
]
```

`inkhaven language derive <lang> --root kata --gloss build --pos verb [--yes]`
applies every rule whose `from_pos` matches, with allophony, and prints the
proposed `form / gloss / pos`. Advisory — `--yes` adds them to the Dictionary
(recording the etymology); dry-run otherwise.

## Grammar (typology)

`inkhaven language grammar <lang>` lists a WALS-aligned catalog of 16
typological features (word order, alignment, case, gender, number,
definiteness, tense/aspect/mood, evidentiality, negation, question formation,
relative clause, …) with the language's current answers and coverage.

```
inkhaven language grammar Eldar --set word_order=sov
inkhaven language grammar Eldar --set alignment=ergative_absolutive
```

Answers are validated against the catalog and stored as a `{ grammar: { … } }`
block in the Grammar chapter; the AI grammar book reads them.

## Diachronics (sound change)

A language can derive from a proto by an ordered chain of sound changes (same
SPE notation as allophony), declared in a `diachronics` block in the
**Phonology** chapter:

```hjson
{ diachronics: {
    proto: "ProtoEldarin"
    rules: [ { rule: "p > f / _ #" }, { rule: "k > h / V _ V" } ]
} }
```

- `inkhaven language sound-change Eldar --form tap` → `tap > taf` (evolve one
  proto-form through the chain).
- `inkhaven language derive-lexicon Eldar [--yes]` → applies the chain to every
  entry of the proto's dictionary, proposing the daughter's lexicon (with the
  gloss carried forward + an etymology); `--yes` commits.

The proto's inventory drives segmentation and the rule classes (the changes are
defined on proto sounds).

- `inkhaven language family-tree` prints the genealogical tree (each language
  under its declared `proto`).
- `inkhaven language cognates ProtoEldarin --form takap` traces a proto-form's
  reflex in every daughter (each daughter's chain applied) — e.g. `Eldar takaf`
  vs `Sindarin tahaf`.
- `inkhaven language reconstruct --forms "tava taba" [--gloss water]` — AI
  comparative reconstruction: proposes the proto-form from cognate forms.
- `inkhaven language realism-check Eldar` — AI assessment of whether the
  language's sound-change chain is typologically plausible.

## Idioms + metaphors

```
inkhaven language idiom-add Eldar --form "kala men" --literal "cold heart" --meaning "unforgiving" [--register formal]
inkhaven language metaphor-add Eldar --source JOURNEY --target LIFE [--example "…"]
inkhaven language idioms Eldar
```

Idioms (a phrase with a literal word-by-word gloss + a separate idiomatic
meaning) and declared conceptual metaphors are stored in the Grammar chapter;
the AI translation consults them to stay idiomatic rather than literal.

## Lexicon

Dictionary entries are HJSON paragraphs under **Dictionary** (created by
`add-word`, the CSV `--import`, or the AI generator):

```hjson
{ word: "makil", type: "noun", translation: "sword",
  register: "formal", domain: ["weapon"], era: "third_age" }
```

| Command | Does |
|---|---|
| `language generate-lexicon --topic … --count … [--semantic] [--yes]` | AI generation behind the dedup gate (illegal / homophone / duplicate-meaning / near-synonym) |
| `language audit [--json]` | phonotactic violations, homophones, duplicate meanings |
| `language query [--register] [--domain] [--era] [--pos] [--text]` | filter by the rich fields |
| `language scan-manuscript [--json]` | candidate undefined conlang words in the prose |

## Analysis

```
inkhaven language stats Avesha [--json]
```

A descriptive profile of the language (vs `audit`, which hunts for problems):
inventory balance (consonants / vowels), phoneme frequency across the lexicon,
the syllable-length distribution (with bars), which onsets and codas actually
get used, and the part-of-speech spread. Computed over the headwords that
segment cleanly into the inventory. This is the snapshot the grammar book and
dictionary output draw on.

## Dictionary output

```
inkhaven language dictionary Avesha --format md|typ [--out dict.typ] [--font Eldar]
```

Renders the dictionary as a real document. **Markdown** (`md`) is a clean,
alphabetized listing (headword, pronunciation, POS, gloss, tags, etymology).
**Typst** (`typ`) is the showpiece: a manual-style **B5 book** — a title page, a
table of contents, an overview, and a two-column lexicon where (when the
language has a `font` block) each headword appears in the **native script**
(transliterated by the input method) beside its romanization. Build the font
(`font-build --language … --format ttf`) and compile with `typst compile
--font-path <dir> dict.typ` to get a PDF that embeds and renders the conscript.

```
inkhaven language grammar-book Avesha --format md|typ [--out g.typ] [--font Eldar] \
    [--study --provider …]
```

The companion volume: a **reference grammar**, drawing every section from the
language's own data — phonology (consonant/vowel inventory, syllable structure,
phonotactics, allophony, stress, tone), morphology (affixes, derivation), the
typology answers (with their consequences), idioms & metaphors, and the sample
texts. Markdown is a flat reference; **Typst** is the same manual-style B5 book
(title page, contents, sections) as the dictionary, with the conscript font.

With **`--study`** it also becomes **study material**: an AI-written study guide
leads the book, defining and explaining every linguistic term the reference uses
(phoneme, allophony, grammatical case, SOV word order, nominative–accusative
alignment, agent nouns, …) and how this language applies them. The reference
itself stays deterministic; only the study guide is AI-authored (needs a
provider).

```
inkhaven language tutorial Avesha --format md|typ [--out learn.typ] [--font Eldar] [--provider …]
```

An **AI-written learner's textbook** — a complete graded course the model
authors from the language's own data (the prose is generated, never hardcoded):
a warm introduction, a pronunciation guide, graded lessons that *explain* the
grammar (word order, the cases, word-building) with worked examples, a reading
passage, and a practice exercise per lesson. The model is constrained to the
language's actual sounds, words, and rules — it never invents vocabulary or
grammar. Markdown comes straight from the model; the Typst path converts that to
a paginated A5 book (embedding the conscript font) behind a deterministic page
scaffold, so it always compiles. The gentle on-ramp the dictionary and grammar
back up.

For a complete worked example that builds a language from nothing to all three
books — with an **AI-drawn font** — see
[`examples/conlang/build-sample-language.sh`](../examples/conlang/build-sample-language.sh)
and [Tutorial 74](Tutorials/74-conlang-end-to-end.md).

## Worldbuilding links

Stored in `.inkhaven/conlang-links.json` (the prose books are never modified):

```
inkhaven language link-place Tirion Quenya [--secondary]
inkhaven language link-character Erendil Quenya native   # native|fluent|conversational|broken|reading_only
inkhaven language speakers Quenya
```

## Writing systems + fonts

A constructed script can be compiled into a usable font from a directory of
glyph SVGs (one per glyph; filename stem = glyph name, and a single-character
stem also becomes the glyph's Unicode codepoint):

A script can be part of the **language definition** — glyphs bound to phonemes
and codepoints in a `font` block, stored in the Phonology chapter:

```hjson
font: {
  family: "Eldar"
  upm: 1000
  glyphs: [
    { name: "a", codepoint: "a", phoneme: "a" }       # printable ASCII → literal
    { name: "o_glyph", codepoint: "U+E000", phoneme: "o" }   # else → hex
  ]
}
```

Glyph artwork lives in the project glyph store
(`.inkhaven/glyphs/<language>/<name>.svg`). The workflow:

```
inkhaven language glyph-lint --svg ./a.svg                 # suitability preflight
inkhaven language glyph-draft Eldar --describe "a vertical stroke \
    with a hook" --phoneme p [--out p.svg] [--yes]         # AI text-to-SVG draft
inkhaven language font-import-glyph Eldar --svg ./a.svg \
    --phoneme a [--codepoint U+E000] [--name a]            # bind + store + record
inkhaven language font-config Eldar [--json]               # show the bindings
inkhaven language font-build --language Eldar \
    [--format ufo|ttf|both] [--out Eldar] [--upm 1000]     # compile from the book
```

- **`glyph-lint`** reports whether an SVG is fit for a font outline (filled
  paths required; stroke-only / image / gradient glyphs are flagged). It also
  warns on non-black fills — a near-white fill among darker ones is almost
  always a counter the author drew with white paint, which a monochrome font
  won't honour (cut counters with a reverse-wound subpath instead).
- **`glyph-draft`** asks the AI to draft an SVG glyph from a description, runs it
  through the same preflight, and previews the result. Advisory: it prints the
  SVG (or writes `--out`) and the verdict; only `--yes` (and only a *usable*
  draft) binds it into the `font` block — the same path as `font-import-glyph`.
- **`font-import-glyph`** preflights the SVG (refusing unusable artwork), copies
  it into the glyph store, and binds it — to a `--phoneme` and a Unicode
  `--codepoint` (a single character or hex; a single-character glyph name
  implies its own codepoint) — recording it in the `font` block.
- **`font-config`** lists every binding with its codepoint, phoneme, and
  artwork status (✓ usable / ⚠ unusable / ✗ missing).
- **`font-build`** runs the preflight on every glyph (skipping unusable ones),
  converts each filled path to font contours (y-flipped + scaled into the em),
  and emits — per `--format`:
  - **`ufo`** (default): a **UFO** font source you can edit or compile with
    `fontc` / `fontmake` / FontForge / Glyphs;
  - **`ttf`**: a ready-to-use **TrueType** binary, compiled fully in-process
    (cubics are quadified for the `glyf` table; a complete OpenType table set
    is assembled — no external tool);
  - **`both`**: the editable source *and* the binary, sharing one stem.

  Source the glyphs from the language's own `font` block (`--language Eldar`,
  family + units-per-em taken from the config) or from a loose directory
  (`font-build Eldar --glyphs ./glyphs/`, filename stem → glyph name).

### Composed blocks (Hangul-style syllables, quadrats)

Some scripts build a unit from several component glyphs arranged in 2D — a
Korean syllable square, an Egyptian quadrat. A **spatial template** names the
cells (each a normalized rectangle in the em, `(0,0)` = top-left); a component
glyph drops into each cell and the whole is baked into one precomposed glyph.

```
inkhaven language font-templates Eldar                     # list templates
inkhaven language font-compose Eldar --template lr \
    --name ka --codepoint U+AC00 --phoneme ka \
    --slot left=lead --slot right=vowel [--out ka.svg] [--yes]
```

Built-in templates: `lr` (left/right), `tb` (top/bottom), `quad` (2×2),
`stack3` (three rows); define your own under `templates` in the `font` block (a
config template overrides a built-in of the same name). `font-compose` places
each `--slot SLOT=GLYPH` component into its cell, runs the composite through the
preflight, and — on `--yes` — binds it like any other glyph (the component
glyphs and the composed block coexist in the font). The composition is baked at
compose time; re-run it after editing a component.

The same template can instead arrange components at **layout time** — for a
hieroglyphic script, where base signs combine contextually and precomposing
every quadrat into the font is impractical:

```
inkhaven language spatial-typst Glyphic --template tb \
    --name quadrat_sunbar --slot top=sun --slot bottom=bar [--size 2em] [--out q.typ]
```

This emits a Typst `#let <name> = box(...)[ … ]` that `place`s each component
(rendered as a character of the generated font) into its cell. Build the font
(`font-build --language … --format ttf`), embed it in your Typst document, and
the quadrat renders with the glyphs arranged spatially — no precomposed glyph
required. (Each component must have a codepoint, since Typst renders by
character.)

### Typing the script (input method)

```
inkhaven language transliterate Eldar --text "katha" [--json]
```

Transliterates romanized/phonemic text into the script's codepoints using the
`font` block's glyph→phoneme bindings: at each position the **longest** glyph
key wins, so a digraph key like `th` or `ka` beats `t`+`h`. The result is a
string of codepoints that renders in the generated font; unmatched characters
pass through and are flagged (bind them with `font-import-glyph --phoneme`).
This is the engine a live editor input mode would drive.

## In the editor

- **`Ctrl+B X`** — the ConLang hub: a read-only overview of every language
  (inventory, counts, prosody, romanization, lexicon size, speakers).
- **`:lang:`** — type `:<name-or-iso>:` to open a lexicon picker that inserts
  the chosen word in place of the trigger.
- **`Ctrl+B Q` / `Ctrl+B Shift+Q`** (1.2.13) — translate a paragraph to / from
  an invented language.

## Principles

- **Forms obey the language; meanings come from the AI; nothing duplicates.**
- AI features are **advisory**: proposal-gated, `--yes`-committed, glosses in
  the project working language.
- **Deterministic everywhere it can be** — generation, validation, allophony,
  stress, romanization, tone, paradigms, and the dedup gate are pure functions;
  the AI calls are thin layers over them.
