# 50 — Bulk-importing a dictionary from CSV

Tutorial 49 covers the Language book end-to-end.
This one zooms in on `inkhaven language add-word
--import` — the bulk-loader for dictionaries
prepared in a spreadsheet, generated from
another tool, or pre-validated in CI.

A sample CSV ships alongside this tutorial:
[`49-language-book-tira-starter.csv`](49-language-book-tira-starter.csv).
Copy it, point it at a scaffolded Tira sub-book,
and the rest of this tutorial walks through
what'll happen.

## When CSV import is the right tool

Use CSV import when:

- You're seeding a project with vocabulary
  pre-built in a spreadsheet (Excel, Numbers,
  Google Sheets, LibreOffice).
- You're generating vocabulary from a script
  (corpus extraction, LLM-generated wordlists,
  conversion from another conlang format).
- You want CI-style "dictionary as source of
  truth" — the .csv lives in version control;
  `--new` re-imports it on every regen.
- You're seeding 20+ words and the per-entry TUI
  `+` chord becomes tedious.

Skip CSV import when:

- You're adding 1-2 words mid-writing — use the
  TUI `+` chord; you'll be in the editor anyway
  to fill the entry's prose notes.
- You need full per-entry HJSON fidelity
  (custom inflection paradigm names, deeply
  nested examples).  The CSV's `inflection`
  column handles flat key=value pairs; richer
  structures need hand-editing post-import.

## The CSV format

Header row drives column mapping.  Column names
are case-insensitive and order-independent.

| Column | Required? | Format |
|-|-|-|
| `word` | yes | invented-language word (becomes the entry slug + lemma) |
| `type` | yes | part of speech (free-form string) |
| `translation` | yes | working-language gloss |
| `example` | no | canonical sample sentence |
| `pronunciation` | no | IPA (`/.../` phonemic, `[...]` phonetic) |
| `etymology` | no | derivation note (plain text) |
| `related` | no | `;`-separated word slugs |
| `inflection` | no | `;`-separated `key=value` paradigm pairs |
| `examples` | no | `|`-separated additional sentences |
| `register` | no | formal / informal / literary / archaic / sacred |
| `era` | no | when the word entered the language |
| `notes` | no | freeform usage notes |

**Quoting.**  RFC 4180-style.  Wrap cells in
`"..."` when they contain commas, quotes, or
newlines.  Double an embedded quote: `""`.

**Why `;` and `|`** for complex fields:
- `inflection` and `related` use `;` because
  paradigm values and word slugs almost never
  contain semicolons.
- `examples` uses `|` because example sentences
  frequently contain commas, and `|` is rarely
  punctuation inside a sentence.

**Skip rules:**
- Row with empty `word` cell → silent skip
  (useful for visual grouping rows).
- Row where `word` starts with `#` → comment,
  skipped.
- Duplicate `word` (already in the dictionary
  before this import) → skipped with warning;
  makes re-imports idempotent.

## Walk-through with the Tira starter CSV

The bundled sample defines 12 Tira entries: 7
core nouns / verbs / particles + 5 ritual
vocabulary words from the river-cult register.

### Step 1 — scaffold Tira

```
$ inkhaven language init Tira
created language book `Tira` at language/tira
  · Meta
  · Dictionary
  · Grammar
  · Phonology
  · Sample texts
```

### Step 2 — populate Meta/overview.alphabet

Open `Language/Tira/Meta/overview` and set the
alphabet field to match the letters your
dictionary uses.  For the sample CSV:

```hjson
{
  // ...
  alphabet: ["A", "E", "I", "K", "L", "M", "N", "O",
             "P", "R", "S", "T", "U"]
  // ...
}
```

These are the letters every word in the sample
CSV uses (plus a couple held in reserve).  The
alphabet drives the Dictionary bucket each word
files under, and — only while the language has
no Phonology inventory yet — it is the
whitelist the import pre-flight checks spelling
against.  Letters are matched longest-first (an
entry such as `"Ch ch"` declares a digraph) and
diacritics fold to their base letter, so the
scaffold's default `["A", ..., "Z"]` accepts
`éle` under `E` but rejects `ŋa`.

### Step 3 — (optional) declare a phonology

Once the language has a Phonology block with a
phoneme inventory, that inventory — not the
alphabet — is what the pre-flight checks every
word against.  Open `Language/Tira/Phonology`
(the `inventory` paragraph, or any paragraph
under the chapter) and declare the phonemes:

```hjson
{
  phonemes: [
    { ipa: "p", kind: "consonant" }, { ipa: "t", kind: "consonant" },
    { ipa: "k", kind: "consonant" }, { ipa: "m", kind: "consonant" },
    { ipa: "n", kind: "consonant" }, { ipa: "s", kind: "consonant" },
    { ipa: "l", kind: "consonant" }, { ipa: "r", kind: "consonant" },
    { ipa: "a", kind: "vowel" }, { ipa: "e", kind: "vowel" },
    { ipa: "i", kind: "vowel" }, { ipa: "o", kind: "vowel" },
    { ipa: "u", kind: "vowel" }
  ]
}
```

Each word is segmented greedily into the
inventory's written forms (`romanize` when
given, else the IPA), so a digraph such as
`{ ipa: "ʃ", romanize: "sh" }` is one letter.
A segment that matches no phoneme fails the
pre-flight with that segment named.  The same
check runs on `language add-word`, on the
in-TUI `+` chord, and on `language import`
(Toolbox / PolyGlot), so a word any door
accepts, every door accepts.

### Step 4 — import the CSV

```
$ inkhaven language add-word Tira \
    --import Documentation/Tutorials/49-language-book-tira-starter.csv
imported `atal` → Tira/Dictionary/A
imported `sora` → Tira/Dictionary/S
imported `mi`   → Tira/Dictionary/M
imported `nan`  → Tira/Dictionary/N
imported `ta`   → Tira/Dictionary/T
imported `peli` → Tira/Dictionary/P
imported `kima` → Tira/Dictionary/K
imported `mora` → Tira/Dictionary/M
imported `samu` → Tira/Dictionary/S
imported `lo`   → Tira/Dictionary/L
imported `sa`   → Tira/Dictionary/S

Import summary for `Tira`
  imported:        11
  skipped (#):     1
```

The comment row (`# the words below are
vocabulary for the river-cult's ritual
vocabulary`) was skipped because it starts with
`#`.

### Step 5 — verify

```
$ inkhaven language list
  name      words  grammar  phonology  samples
  ------------------------------------------------
  Tira         11        0          1        0

$ inkhaven language doctor Tira
Language doctor — `Tira`
...
Chapters
  Dictionary     : 11 parseable entries
  Grammar        : 0 rules
  Phonology      : 1 rules
  Sample texts   : 0 samples

Blocks (Phonology / Grammar HJSON)
  every block parses and validates

Dictionary coverage
  with example   : 7/11 (63%)
  with paradigm  : 7/11 (63%)
  missing example: 4
  missing paradigm: 4 (overlay won't catch inflected forms)
...
```

Open any imported entry (e.g.
`Language/Tira/Dictionary/A/atal`) — it renders
as syntax-highlighted HJSON with every field the
CSV provided, no commented-out stubs (the import
path uses a compact body builder distinct from
the interactive seed template):

```hjson
{
  word:         "atal"
  type:         "noun"
  translation:  "river"
  example:      "Atal nan ta-mi sora."
  examples: [
    "Atal sora-mi."
    "Pelele atal-e."
  ]
  pronunciation: "/ˈa.tal/"
  etymology:    "from proto-Tira *a-tal 'flowing water'"
  related:      ["atale", "atatal"]
  inflection: {
    genitive: "atale"
    nominative: "atal"
    plural: "atatal"
  }
  register:     "formal"
  notes:        "Central worldbuilding word — the river-cult takes its name from this root"
}
```

Add the remaining fields (`era`, `frequency`,
`notes` on the entries that didn't have them) by
opening each paragraph and editing the HJSON.

## --new — wipe-and-replace semantics

The default import is **additive**: existing
entries are kept (duplicates skipped); new rows
are added.  Useful for incremental updates.

Pass `--new` to make the import **wipe-and-
replace**: every existing paragraph + bucket
subchapter under `Dictionary` is deleted before
the CSV is read.  The Dictionary chapter itself
is preserved.

```
$ inkhaven language add-word Tira --import tira-v2.csv --new
--new: wiped 11 existing entries across 7 buckets from `Tira/Dictionary`
imported `atal` → Tira/Dictionary/A
...
Import summary for `Tira`
  imported:        15
```

Use `--new` when:
- The CSV is the source of truth (version-
  controlled; re-imported on every regen).
- You want to drop typos or schema-evolution
  artefacts from earlier import passes.
- You're scripting "regenerate the dictionary
  from upstream" in CI.

**Validation ordering:** pre-flight runs BEFORE
the wipe, so a bad CSV doesn't destroy your
existing dictionary then fail to populate the
replacement.  Belt-and-braces.

## Pre-flight validation

Before any writes — including the `--new` wipe
— the import pre-flight pass walks every CSV
row and runs each `word` through the same
headword check `language add-word` uses:

1. **Shape** — 1–64 characters, no control
   characters.  Never skipped: the headword
   becomes a paragraph title and a file name.

2. **Spelling, against one authority** — when
   the language declares a Phonology inventory,
   every segment of the word (greedy
   longest-match on the phonemes' written forms)
   must be a declared phoneme.  Otherwise, when
   `Meta/overview.alphabet` is non-empty, every
   letter must be declared there (longest-match,
   case- and diacritic-insensitive).  With
   neither declared, only the shape is checked.

Whitespace, hyphens and apostrophes separate
letter runs and are never segments themselves.

If any word fails validation, the entire import
is aborted with a per-violation report:

```
$ inkhaven language add-word Tira --import bad.csv
Pre-flight validation failed — 2 violation(s) found (checked against the Phonology inventory):

  · row 3: `xeno` contains `x`, which is not a phoneme of the declared Phonology inventory
  · row 5: `zara` contains `z`, which is not a phoneme of the declared Phonology inventory

Fix by either:
  · adding the missing phonemes to the Phonology block (or letters to Meta/overview.alphabet), OR
  · correcting the CSV, OR
  · re-running with --force to bypass validation.

Error: import aborted — 2 alphabet/phonology violation(s)
```

**Why hard-stop rather than warn:**

A partial import would leave the dictionary in
a confused state (some valid entries from the
CSV imported, the rest missing); the gap
analysis in `language doctor` would then
misreport coverage.  Hard-stop keeps the
dictionary in a known-good state.

**Bypass with `--force`** when:
- You're intentionally importing words that
  exceed the current declaration (e.g. you're
  seeding the alphabet FROM the CSV — the
  alphabet check would refuse, defeating the
  purpose).
- You're importing loanwords that use phonemes
  outside the native inventory (Tira loans
  `samu` from `Atal-Kele` using the same
  consonant set — no `--force` needed; a
  language with truly alien borrowings would).
- You know the validation is wrong (typo in
  alphabet; phoneme inventory deliberately
  incomplete during the language's design
  phase).

```
$ inkhaven language add-word Tira --import loanwords.csv --force
imported `xena` → Tira/Dictionary/X    # 'x' not in the inventory, but --force overrode
...
```

`--force` works the same way for a single word:
`inkhaven language add-word Tira xena --type noun
--translation stranger --force`.

**Homographs.**  A row whose `word` is already
defined with the *same* `type` is skipped as a
duplicate; the same spelling with a *different*
`type` (`pata` noun, `pata` verb) is imported as
a second entry titled `pata (2)`.

## Combining flags

The three flags compose:

| Flags | Behaviour |
|-|-|
| `--import <path>` | Validate; if clean, additive import (skip duplicates) |
| `--import <path> --new` | Validate; if clean, wipe Dictionary then import |
| `--import <path> --force` | Skip validation; additive import |
| `--import <path> --new --force` | Skip validation; wipe Dictionary then import |

`--force` always implies skipping the pre-flight
check; `--new` always implies the wipe step.
They're orthogonal.

## CSV authoring tips

**Build in a spreadsheet first.**  Excel /
Numbers / Google Sheets / LibreOffice all
export to UTF-8 CSV cleanly.  Build the
dictionary in a spreadsheet (with columns
matching the schema names exactly — case
doesn't matter), then export.  Reshaping after
export is harder than reshaping in the
spreadsheet.

**Reserve `|` and `;` characters carefully.**
The CSV cell parser treats them as
sub-separators within the `examples` (pipe)
and `inflection` / `related` (semicolon)
columns.  If your prose actually contains them,
use a different sentence format.

**Comment liberally.**  `#`-prefixed rows are
free.  Use them to group vocabulary by domain
(`# kinship terms`, `# colours`, `# verbs of
motion`) — the dictionary stays organised AND
the spreadsheet stays readable.

**Re-imports are idempotent.**  Edit the CSV,
re-import — existing entries are skipped as
duplicates, new ones are added.  This is the
"adding more entries over time" workflow.

**For wipe-and-replace workflows, version-
control the CSV.**  Commit `tira-dict.csv`
alongside the manuscript; CI step regenerates
the dictionary on every push:

```yaml
# .github/workflows/dict.yml
- name: regenerate Tira dictionary
  run: |
    inkhaven language add-word Tira \
      --import tira-dict.csv --new
- name: verify health
  run: |
    inkhaven language doctor Tira --json | \
      jq -e '.coverage.with_paradigm_pct >= 80'
```

## Round-tripping with export

`inkhaven language export <lang> --format csv`
emits the import-compatible CSV, and because
`add-word` and the import pre-flight run the
same headword check, an exported dictionary
re-imports without `--force`:

```
$ inkhaven language export Tira --format csv --output tira.csv
$ inkhaven language add-word Tira --import tira.csv --new
```

The other formats (`json`, `anki`,
`dictionary-twocol`) are one-way; the `json`
export is structurally complete if you need to
post-process:

```
$ inkhaven language export Tira --format json | \
    jq '.dictionary[] | [.word, .type, .translation] | @csv' -r
"atal","noun","river"
"kima","adjective","green"
...
```

## Cheat sheet

| Action | Command |
|-|-|
| Additive import | `inkhaven language add-word <lang> --import <path.csv>` |
| Wipe-and-replace | `inkhaven language add-word <lang> --import <path.csv> --new` |
| Skip the phonology / alphabet spelling check | `... --force` (also on a single `add-word`) |
| Comment a row | put `#` in the `word` cell |
| Skip a row | leave the `word` cell empty |
| Inflection format | `nominative=atal;genitive=atale;plural=atatal` |
| Multiple examples | `Sentence one.\|Sentence two.\|Sentence three.` |
| Related entries | `atale;atatal;pelele` |
| Re-import idempotency | safe — existing entries skipped as duplicates |
