# 49 — Language book end-to-end

The Language book is inkhaven's invented-language
workbench: dictionary, grammar, phonology, sample
texts, and an AI translation flow that round-trips
between your manuscript's working language and a
conlang you defined.  This tutorial walks the full
authoring loop from empty project to translated
prose.

The feature ships across 1.2.13 in five phases
(A through D plus a D.1 polish round); this
tutorial uses the surface that landed in
1.2.13 — every chord and command below works on
that release.

## Why this exists

Authors building secondary-world novels,
roleplaying-game settings, or worldbuilding
journals routinely need invented vocabulary
that stays self-consistent across the
manuscript.  Pre-1.2.13 the options were:

- Keep a parallel `.txt` glossary the editor
  doesn't know about.
- Use the existing Artefacts / Places books for
  individual words, losing the dictionary
  structure.
- Pay for external tools (PolyGlot, ConWorkShop)
  that don't talk to your manuscript.

The Language book treats invented languages as
first-class project content — same store, same
backup, same search, same AI integration.  Your
manuscript and your dictionary live next to each
other.

## Step 1 — scaffold the language

Two equally valid entry points:

**From the TUI** — open the project, focus the
tree pane (F8), navigate to the `Language`
system book (or any node already inside it),
and press **`b`** (Add Book).  Status bar
prompts `new language — type a name, Enter to
scaffold; Esc to cancel`.  Type the language
name, hit Enter.  The commit handler auto-
creates the 5 standard chapters (`Meta`,
`Dictionary`, `Grammar`, `Phonology`, `Sample
texts`) and seeds `Meta/overview` with the
starter HJSON template.  Confirmation: `added
language ` + name ` — 5 chapters scaffolded;
edit Meta/overview to set the alphabet`.

(Pressing `b` from anywhere else in the tree
still works the way it always has — slots a
new top-level user book above the system block.
The Language-scaffold path only fires when the
cursor is on or inside the Language system
book.)

**From the shell** —

```
$ inkhaven language init Quenya
created language book `Quenya` at language/quenya
  · Meta
  · Dictionary
  · Grammar
  · Phonology
  · Sample texts

Next steps:
  · edit `Language/Quenya/Meta/overview` to set the alphabet + metadata
  · add dictionary entries under `Language/Quenya/Dictionary`
  · add grammar rules under `Language/Quenya/Grammar`
```

Either path produces the same scaffold.  Pick
whichever is faster from where your cursor
already is.

## Step 2 — populate `Meta/overview`

Open `Language/Quenya/Meta/overview` and edit
the HJSON block.  The defaults assume a Latin
A-Z alphabet; non-Latin authors override
`alphabet` with the groupings they want:

```hjson
{
  name: "Quenya"
  language_kind: constructed
  family: Elvish
  iso_code:
  alphabet: ["A", "B", "C", "D", "E", "F", "G", "H", "I",
             "J", "K", "L", "M", "N", "O", "P", "Q", "R",
             "S", "T", "U", "V", "W", "X", "Y", "Z"]
  reading_direction: ltr
  stemmer:
  example_corpus_ref: "Tolkien — Etymologies"
}
```

The `alphabet` field drives the Dictionary's
bucket subchapter auto-creation in the next
step.  For Hebrew letter names you might write
`alphabet: ["Aleph", "Beth", "Gimel", ...]`; for
paired-case Latin, `["Aa", "Bb", ...]`.

## Step 3 — add dictionary entries

```
$ inkhaven language add-word Quenya aiya \
    --type interjection \
    --translation hail \
    --example "Aiya Eärendil!"
created subchapter `A`
added `aiya` to `Quenya/Dictionary/A` (interjection · hail)
```

`add-word` resolves the language sub-book by
case-insensitive title, finds the Dictionary
chapter, derives the alphabet bucket from the
word's first character (consulting the
`Meta/overview.alphabet` list first for non-
Latin orthographies, falling back to first-
char uppercase for Latin / Cyrillic / Greek),
auto-creates the bucket subchapter when
missing, and seeds the entry paragraph with
four HJSON fields.

The created entry looks like this:

```hjson
{
  word:         "aiya"
  type:         "interjection"
  translation:  "hail"
  example:      "Aiya Eärendil!"
}
```

Open the entry paragraph in the editor to add
optional fields the seed leaves blank — most
useful are `inflection` (paradigm forms) and
`notes` (etymology, register, related
entries):

```hjson
{
  word:         "aiya"
  type:         "interjection"
  translation:  "hail"
  example:      "Aiya Eärendil!"
  inflection: {
    plural:    "aiyar"
    emphatic:  "aiyala"
  }
}
```

Every form in the `inflection` map gets added to
the lexicon overlay alongside the lemma — so
prose containing `aiyar` or `aiyala` will light
up the same as `aiya`.

Type a wrong word?  Remove it the same way:

```
$ inkhaven language remove-word Quenya aiya
removed `aiya` from `Quenya/Dictionary/A`
```

## Step 4 — see the overlay in your manuscript

Write a manuscript paragraph that uses the
invented word and the lexicon overlay will
paint it italic in `theme.language_word_fg`
(default soft mauve-teal `#b4a8e1`):

> "Aiya, friend," she said as Eärendil's ship
> sailed past the western horizon.

`Aiya` lights up italic.  Position the cursor
on the word and the editor footer chip shows
`[aiya · interjection · hail]` — the lemma,
part of speech, and translation lifted live
from the entry's HJSON.

When the cursor moves off the word the chip
goes away and (if the paragraph has a
word-count goal set) the progress gauge comes
back.

## Step 5 — translate prose into the language

Press `Ctrl+B Q` with the cursor in any
manuscript paragraph.  The AI translation
flow:

1. Composes a prompt envelope from the
   language sub-book's chapters: Meta/overview,
   every Grammar rule, every Phonology rule,
   Dictionary entries filtered to words that
   appear in the source text (RAG-style
   relevance), and up to 3 Sample texts as
   register anchors.
2. Streams the LLM's response into the AI
   pane.

When more than one language is defined, a
picker pops first: `↑↓` to highlight, `Enter`
to commit, or just **type the first letter of
the language name** to jump-and-commit
(`q` for Quenya, `d` for Drow).  Single-
language projects skip the picker entirely.

The AI pane title shows `translate[on]` in
italic while the stream is in flight so you
know the upcoming `I` apply chord will use
translation extraction, not raw insert.

The response includes:

- The translated text wrapped in
  `<<<TRANSLATION>>>` / `<<<END>>>` markers.
- A per-token gloss table (source · gloss ·
  target).
- A list of which grammar rules fired and
  which dictionary entries were applied.
- Confidence flags — "dictionary missing for
  X" with suggested entries you can add via
  `add-word`.

Press `I` in the AI pane to insert ONLY the
target-language prose at your cursor (the
gloss table and applied-rules list stay in
the AI pane for reference but don't pollute
the manuscript).  If the LLM forgot the
markers, a second `I` press falls back to
inserting the full body.

## Step 6 — roundtrip-test the grammar

`Ctrl+B Shift+Q` reverses the direction:
translate FROM the invented language back to
the working language.  Same envelope shape,
flipped from/to labels.  Same picker semantics
if you have multiple languages.

The natural roundtrip workflow:

1. `Ctrl+B Q` an English paragraph → land the
   invented translation in the next paragraph
   via `I`.
2. `Ctrl+B Shift+Q` the invented translation
   → AI pane shows the back-translation.
3. Compare against the original.  When the
   back-translation drifts beyond register
   ("the king's wisdom" → "the wisdom of
   regal pomp"), the grammar rules or
   dictionary entries have an inconsistency
   the manuscript will eventually trip over.

This is the in-TUI version of what'll
eventually be the headless `inkhaven language
test` corpus-driven drift detector.

## Step 7 — health check + export

`inkhaven language doctor Quenya` emits a
report:

```
Language doctor — `Quenya`

  name           : Quenya
  kind           : constructed
  family         : Elvish
  alphabet       : 26 entries
  direction      : ltr

Chapters
  Dictionary     : 42 parseable entries
  Grammar        : 6 rules
  Phonology      : 3 rules
  Sample texts   : 4 samples

Dictionary coverage
  with example   : 38/42 (90%)
  with paradigm  : 12/42 (28%)
  missing example: 4
  missing paradigm: 30 (overlay won't catch inflected forms)

Manuscript gap analysis
  unique words (≥2 chars) in manuscript prose: 1247
  covered by dictionary: 31/1247 (2%)
  uncovered words (sample, max 15):
    · ...
```

Pass `--json` for CI-friendly output you can
grep with `jq`:

```
$ inkhaven language doctor Quenya --json \
    | jq '.coverage.with_example_pct < 80'
false
```

`inkhaven language list` summarises every
defined language in one table:

```
$ inkhaven language list
  name      words  grammar  phonology  samples
  ------------------------------------------------
  Quenya       42        6          3        4
  Sindarin     17        2          1        2
```

When you're ready to publish, export to the
format that matches your downstream tool:

```
# Two-column printable Typst dictionary
$ inkhaven language export Quenya \
    --format dictionary-twocol \
    --output dist/quenya-dict.typ

# Anki / SuperMemo / Mochi flashcard deck
$ inkhaven language export Quenya \
    --format anki \
    --output dist/quenya.csv

# Full structured JSON for downstream tooling
$ inkhaven language export Quenya \
    --format json \
    --output dist/quenya.json
```

## Cheat sheet

| Action | Chord / Command |
|-|-|
| Scaffold a new language (TUI) | Tree pane (F8) → cursor on `Language` → `b` |
| Scaffold a new language (shell) | `inkhaven language init <name>` |
| Add a dictionary entry | `inkhaven language add-word <lang> <word> --type <pos> --translation <text>` |
| Remove a dictionary entry | `inkhaven language remove-word <lang> <word>` |
| Translate INTO the language | `Ctrl+B Q` in editor |
| Translate FROM the language | `Ctrl+B Shift+Q` in editor |
| Insert translation at cursor | `I` in AI pane (lifts only the marker block) |
| Health report | `inkhaven language doctor <lang>` (add `--json` for CI) |
| List defined languages | `inkhaven language list` |
| Export | `inkhaven language export <lang> --format <fmt> --output <path>` |

## What's not in 1.2.13

Phase D.2 candidates (the §12 / §13 / §14
parts of `Documentation/PROPOSALS/LANGUAGE_BOOK.md`
that didn't ship):

- `--format grammar` and `--format phrasebook`
  exports — need rule HJSON schema design.
- `inkhaven language test` headless roundtrip
  drift CLI.
- `inkhaven language translate` headless
  translation CLI.
- `Ctrl+B Shift+R` reverse-lookup picker
  ("find the entry whose translation is `X`").
- `Ctrl+B Shift+W` word-of-the-day floating
  card in the manuscript editor + phonotactic
  generator in the Language book.
- Card renderers for Dictionary / Grammar /
  Phonology paragraphs viewed inside the
  Language book (the §7 / §10 visualisations
  from the proposal).

The plumbing for all of these is in place;
they're chord / render work, not data-model
work.
