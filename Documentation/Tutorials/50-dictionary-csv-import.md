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
CSV uses (plus a couple held in reserve).  If you
skip this step, the import pre-flight will warn
that words contain characters not in the alphabet
(because the empty-default `["A", ..., "Z"]`
allows them, but a tight alphabet is good
discipline).

### Step 3 — (optional) declare a phonology

To exercise the phoneme-inventory validation,
add a Phonology rule via the TUI (`+` under
`Language/Tira/Phonology` → type
`consonant-inventory`):

```hjson
{
  rule_id: "consonant-inventory"
  category: "consonants"
  rule: '''
    Tira has 8 consonants:
      Stops:     p k t
      Nasals:    m n
      Fricative: s
      Liquids:   l r
  '''
  phonemes: ["p", "k", "t", "m", "n", "s", "l", "r"]
}
```

Without the `phonemes` array populated, phonology
validation skips silently (alphabet validation
still runs).  A future release will let `language
doctor` cross-check this against the actual
dictionary for completeness.

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
row and validates each `word` against:

1. **The alphabet** — every non-whitespace,
   non-punctuation character of every word must
   appear in some entry of
   `Meta/overview.alphabet`.  Skipped when the
   alphabet list is empty.

2. **The phoneme inventories** — the union of
   every Phonology rule's `phonemes` field.
   Skipped when no Phonology rule declares
   `phonemes`.

If any word fails validation, the entire import
is aborted with a per-violation report:

```
$ inkhaven language add-word Tira --import bad.csv
Pre-flight validation failed — 2 violation(s) found:

  · row 3: `xeno` contains `x` not in Meta/overview.alphabet
  · row 5: `zara` contains `z` not in Meta/overview.alphabet

Fix by either:
  · updating Meta/overview.alphabet to include the missing characters, OR
  · updating a Phonology rule's `phonemes` list to include them, OR
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
  exceed the current Meta/overview declaration
  (e.g. you're seeding the alphabet FROM the
  CSV — the alphabet check would refuse,
  defeating the purpose).
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
imported `xena` → Tira/Dictionary/X    # 'x' not in alphabet, but --force overrode
...
```

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

Phase D ships `inkhaven language export <lang>
--format <fmt>` — `json` and `anki` and
`dictionary-twocol`.  None of them currently
round-trip back to the import CSV format, but
the `json` export is structurally complete:

```
$ inkhaven language export Tira --format json | \
    jq '.dictionary[] | [.word, .type, .translation] | @csv' -r
"atal","noun","river"
"kima","adjective","green"
...
```

Future candidate: `--format csv` that emits
import-compatible CSV so the round-trip is
fully closed.  For now, this `jq` recipe
covers the common case.

## Cheat sheet

| Action | Command |
|-|-|
| Additive import | `inkhaven language add-word <lang> --import <path.csv>` |
| Wipe-and-replace | `inkhaven language add-word <lang> --import <path.csv> --new` |
| Skip alphabet / phonology validation | `... --force` |
| Comment a row | put `#` in the `word` cell |
| Skip a row | leave the `word` cell empty |
| Inflection format | `nominative=atal;genitive=atale;plural=atatal` |
| Multiple examples | `Sentence one.\|Sentence two.\|Sentence three.` |
| Related entries | `atale;atatal;pelele` |
| Re-import idempotency | safe — existing entries skipped as duplicates |
