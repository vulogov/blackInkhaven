# 49 — Language book end-to-end

The Language book is inkhaven's invented-language
workbench: dictionary, grammar, phonology, sample
texts, and an AI translation flow that round-trips
between your manuscript's working language and a
conlang you defined.  This tutorial walks the full
authoring loop from empty project to translated
prose, with a complete worked example you can type
in to see every chapter exercised end-to-end.

The feature ships across 1.2.13 (Phases A through
D.1).  Every chord and command below works on that
release.

## Why this exists

Authors building secondary-world novels,
roleplaying-game settings, or worldbuilding
journals routinely need invented vocabulary that
stays self-consistent across the manuscript.
Pre-1.2.13 the options were:

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
other; the AI translator reads the dictionary +
grammar + phonology + sample texts as
authoritative when translating into or out of
the language.

## The five-chapter shape

Every language sub-book carries the same five
chapters.  Order matches the order you'll fill
them in:

```
Language
└── <YourLanguage>
    ├── Meta              ← language metadata
    │   └── overview      ← single HJSON paragraph
    ├── Dictionary        ← words go here
    │   ├── A             ← alphabet bucket (auto-created)
    │   │   └── aiya      ← one paragraph per word
    │   └── B
    │       └── bara
    ├── Grammar           ← rules the AI translator consumes
    │   ├── noun-cases
    │   └── verb-tense
    ├── Phonology         ← sound rules
    │   ├── syllable-template
    │   └── vowel-harmony
    └── Sample texts      ← few-shot anchors
        ├── greeting
        └── short-poem
```

Why split this way:

- **Meta/overview** carries the global facts
  (alphabet, word order, morphological type).  Read
  ONCE per translation prompt.
- **Dictionary entries** are RAG-filtered into the
  translation prompt — only entries whose
  `translation` appears in the source text get
  bundled.  Keeps the prompt focused even with
  hundreds of entries.
- **Grammar rules** also RAG-filtered via each
  rule's `applies_when` field.
- **Phonology rules** are NOT bundled into every
  translation prompt (they'd bloat it).  Used by
  the phonotactic generator (Phase D.2) and
  available to the LLM on demand.
- **Sample texts** — up to 3 included as register
  anchors in every translation.

## The worked example: Tira

Throughout the rest of this tutorial we'll build
a small original conlang called **Tira**.  Tira
is small enough to fit in a tutorial but large
enough to exercise every feature: it has 12
consonants + 5 vowels, a CV(C) syllable, two
grammatical cases, and plural marked by reduplication
of the first syllable.

Why a made-up conlang instead of Quenya or
Klingon: those are licensed properties with
existing dictionaries, and the point of this
tutorial is the **process**, not the
**vocabulary**.  Substitute your own
secondary-world language as you read.

## Step 1 — scaffold the language

Two equally valid entry points:

**From the TUI** — open the project, focus the
tree pane (`F8`), navigate to the `Language`
system book (or any node already inside it), and
press **`b`** (Add Book).  Status bar prompts
`new language — type a name, Enter to scaffold;
Esc to cancel`.  Type `Tira`, hit Enter.

Confirmation: `added language `Tira` — 5 chapters
scaffolded; edit Meta/overview to set the
alphabet`.

(Pressing `b` from anywhere else in the tree
still slots a new top-level user book above the
system block — the Language scaffold path only
fires when the cursor is on or inside the
Language system book.)

**From the shell** —

```
$ inkhaven language init Tira
created language book `Tira` at language/tira
  · Meta
  · Dictionary
  · Grammar
  · Phonology
  · Sample texts
```

Either path produces the same scaffold.  Pick
whichever is faster from where your cursor
already is.

## Step 2 — populate `Meta/overview`

Tree pane → navigate to `Language/Tira/Meta/overview`
→ Enter to open in the editor.  The body is
already a fully-commented HJSON template with every
field stubbed in.  Edit in place; the paragraph's
content type is `[hjson]` so syntax highlighting
shows you the structure.

For Tira, fill the fields like this:

```hjson
{
  // ─────────────────────────────
  // IDENTITY
  // ─────────────────────────────
  name: "Tira"
  family: "Standalone constructed"
  language_kind: constructed
  iso_code: ""

  // ─────────────────────────────
  // ORTHOGRAPHY
  // ─────────────────────────────
  alphabet: ["A", "E", "I", "K", "L", "M", "N", "O",
             "P", "R", "S", "T", "U"]
  reading_direction: ltr
  script: "Latin"

  // ─────────────────────────────
  // LINGUISTIC SHAPE
  // ─────────────────────────────
  word_order: "SOV"
  morphology: "agglutinative"
  tonal: false
  has_cases: true
  has_gender: false

  // ─────────────────────────────
  // RUNTIME / TOOLING
  // ─────────────────────────────
  stemmer: ""           // no off-the-shelf stemmer applies
  example_corpus_ref: ""

  notes: "Tira is spoken by the river-cult of Atal in the
  northern valleys. Formal register only — no informal/casual
  forms in the manuscript."
}
```

Save with `F4` (the standard inkhaven save chord).
A few notes:

- `alphabet` only carries the letters Tira actually
  uses — `B`, `C`, `D`, etc. are dropped.  This drives
  the Dictionary's bucket auto-creation in the next
  step.
- `word_order`, `morphology`, `has_cases`,
  `has_gender` are the quick-reference summary the
  AI translator reads BEFORE composing translation
  prompts.  Tight values here mean better
  translations.
- `notes` is freeform — the LLM doesn't read it,
  but you do.

## Step 3 — add dictionary entries

Three ways to add a word.  Pick whichever fits
your authoring rhythm:

### 3a. The TUI quick path

Tree cursor anywhere under
`Language/Tira/Dictionary` (the chapter itself or
an existing bucket subchapter), press **`+`** (Add
Paragraph).  Type the word (`atal`), hit Enter.

The commit handler:

1. Walks the parent chain to identify Tira as the
   target language.
2. Derives the alphabet bucket (`A`) from the
   word's first character — consulting
   `Meta/overview.alphabet` first, falling back to
   first-char uppercase.
3. Auto-creates the `A` subchapter under
   `Dictionary` if it doesn't exist yet.
4. Creates the entry paragraph under `A`.
5. Seeds the body with the full commented HJSON
   template, with `word: "atal"` pre-filled.

Status: `added \`atal\` to Tira/Dictionary/A — open
the paragraph to fill POS / translation`.

Open the new paragraph and fill in:

```hjson
{
  // CORE
  word:         "atal"
  type:         "noun"
  translation:  "river"
  example:      "Atal nan ta-mi sora."

  // OPTIONAL — uncomment the fields you need
  // examples:     []
  pronunciation: "/ˈa.tal/"
  etymology:    "from proto-Tira *a-tal 'flowing water'"
  // related:      []

  inflection:   {
    nominative: "atal"
    genitive:   "atale"
    plural:     "atatal"        // first-syllable reduplication
  }

  // register:     ""
  // era:          ""
  // frequency:    0

  notes: "Central worldbuilding word — the river-cult takes
  its name from this root."
}
```

Save with `F4`.  The paradigm forms (`atale`,
`atatal`) are added to the lexicon overlay — when
your manuscript prose contains any of those three
forms, they'll light up italic in mauve-teal.

### 3b. The shell bulk path

Better for adding many words at once or scripting
from a CSV:

```
$ inkhaven language add-word Tira atal \
    --type noun \
    --translation river \
    --example "Atal nan ta-mi sora."
created subchapter `A`
added `atal` to `Tira/Dictionary/A` (noun · river)

$ inkhaven language add-word Tira sora \
    --type verb \
    --translation flow \
    --example "Atal sora-mi."
added `sora` to `Tira/Dictionary/S` (verb · flow)

$ inkhaven language add-word Tira mi \
    --type particle \
    --translation "(present tense marker)"
added `mi` to `Tira/Dictionary/M` (particle · (present tense marker))
```

Open each paragraph in the editor afterward to
fill the optional fields (paradigms, etymology,
etc.) that the shell command doesn't accept.

### 3c. The bulk-import path — CSV dictionary

The fastest path when you've prepared vocabulary
in a spreadsheet or generated it from another
tool: bulk-import a CSV.  This section summarises
the surface; **[Tutorial 50 — Bulk-importing a
dictionary from CSV](50-dictionary-csv-import.md)**
covers the full workflow including `--new`
wipe-and-replace, pre-flight alphabet + phonology
validation, `--force` bypass, CI patterns, and a
ready-to-copy sample CSV
([`49-language-book-tira-starter.csv`](49-language-book-tira-starter.csv)).

```
$ inkhaven language add-word Tira --import tira-starter.csv
imported `atal` → Tira/Dictionary/A
imported `sora` → Tira/Dictionary/S
imported `mi`   → Tira/Dictionary/M
imported `nan`  → Tira/Dictionary/N
imported `ta`   → Tira/Dictionary/T
imported `peli` → Tira/Dictionary/P
imported `kima` → Tira/Dictionary/K

Import summary for `Tira`
  imported:        7
```

**CSV format.**  Header row maps column names to
row positions, so columns can appear in **any
order** and **any subset** (only the required
ones must be present).

| Column | Required? | Format |
|-|-|-|
| `word` | yes | invented-language word (becomes the entry slug + lemma) |
| `type` | yes | part of speech (free-form string) |
| `translation` | yes | working-language gloss |
| `example` | no | canonical sample sentence |
| `pronunciation` | no | IPA (`/.../` for phonemic, `[...]` for phonetic) |
| `etymology` | no | derivation note (plain text) |
| `related` | no | `;`-separated word slugs |
| `inflection` | no | `;`-separated `key=value` paradigm pairs |
| `examples` | no | `|`-separated additional sentences |
| `register` | no | formal / informal / literary / archaic / sacred |
| `era` | no | when the word entered the language |
| `notes` | no | freeform usage notes |

**Quoting.**  RFC 4180-style — wrap a cell in
`"..."` if it contains commas, quotes, or
newlines.  Double an embedded quote: `""`.

**Skip rules:**
- Row with empty `word` cell → skipped silently
  (lets you leave blank rows for visual
  grouping).
- Row where `word` starts with `#` → comment,
  skipped.
- Duplicate `word` (already in the dictionary)
  → skipped with `row N: \`X\` already exists`
  warning.  Makes re-imports idempotent.

**Worked Tira starter CSV** (`tira-starter.csv`):

```csv
word,type,translation,example,pronunciation,inflection,examples,etymology,notes
atal,noun,river,"Atal nan ta-mi sora.",/ˈa.tal/,"nominative=atal;genitive=atale;plural=atatal","Atal sora-mi.|Pelele atal-e.","from proto-Tira *a-tal 'flowing water'","Central worldbuilding word"
sora,verb,flow,"Atal sora-mi.",/ˈso.ɾa/,,,,
mi,particle,(present tense marker),,,,,,
# pronouns block
nan,pronoun,you,,/nan/,"nominative=nan;genitive=nane",,,
ta,particle,(subject/object marker),,,,,,
peli,noun,mountain,"Peli kima-mi.",/ˈpe.li/,"nominative=peli;genitive=pelie;plural=pepeli",,,
kima,adjective,green,,,"nominative=kima;plural=kikima",,,
```

**Tips:**
- Generate the CSV from any spreadsheet (Excel,
  Numbers, Google Sheets, LibreOffice Calc) —
  export as CSV with UTF-8 encoding.
- The `inflection` column gives the LLM the
  paradigm forms it needs for translation AND
  feeds the lexicon overlay so the inflected
  forms light up in your manuscript prose.
- After import, open the entries to add anything
  the CSV doesn't carry (fields exist in the
  schema for `frequency`, `era`, `register`
  even if you didn't populate them in the CSV).
- Re-importing the SAME CSV after edits is safe:
  existing entries are skipped, new rows are
  added.  To *update* an entry, `remove-word`
  first, then re-import.

### 3d. Remove an entry

Mirror of `add-word`:

```
$ inkhaven language remove-word Tira mi
removed `mi` from `Tira/Dictionary/M`
```

Errors when the entry's already gone rather than
silent no-op, so scripts know.

### Recommended Tira starter vocabulary

To make later steps interesting, populate Tira
with at least these:

| Word | POS | Translation | Paradigm |
|-|-|-|-|
| `atal` | noun | river | atal / atale / atatal |
| `sora` | verb | flow | (see Grammar §4) |
| `mi` | particle | (present marker) | — |
| `nan` | pronoun | you | nan / nane / — |
| `ta` | particle | (subject marker) | — |
| `peli` | noun | mountain | peli / pelie / pepeli |
| `kima` | adjective | green | kima / — / kikima |

## Step 4 — define grammar rules

Tree cursor on `Language/Tira/Grammar` → `+` →
type a rule_id (`noun-cases`) → Enter.  The
paragraph body is seeded with the full commented
grammar template.

Edit it for Tira's case system:

```hjson
{
  rule_id:      "noun-cases"
  title:        "Noun cases — nominative and genitive"
  category:     morphology

  rule:         '''
    Tira nouns inflect for two cases:

      NOM (nominative, subject):  zero suffix.
        atal       (river, as subject)

      GEN (genitive, possession): -e suffix.
        atale      (of the river)

    The genitive suffix follows the bare noun stem
    with no vowel-harmony modification.  Plurals
    take the case suffix on the reduplicated form:

        atatal     (rivers, NOM)
        atatale    (of the rivers, GEN)
  '''

  examples: [
    { source: "the river",          target: "atal",     gloss: "river.NOM" }
    { source: "of the river",       target: "atale",    gloss: "river.GEN" }
    { source: "the mountain",       target: "peli",     gloss: "mountain.NOM" }
    { source: "of the mountains",   target: "pepelie",  gloss: "mountain.PL.GEN" }
  ]

  applies_when: "the target sentence contains a noun that owns or modifies another noun"
  depends_on:   ["plural-reduplication"]
  conflicts_with: []

  productivity: "core"
  register:     ""
  notes:        ""
}
```

Add a second rule for plural reduplication:

```hjson
{
  rule_id:      "plural-reduplication"
  title:        "Plural — first-syllable reduplication"
  category:     morphology

  rule:         '''
    Plural is marked by reduplicating the noun's
    first syllable.  The first syllable is the
    longest CV or CVC sequence at the start of
    the word.

        atal  → atatal     (river → rivers)
        peli  → pepeli     (mountain → mountains)
        kima  → kikima     (green → greens)
  '''

  examples: [
    { source: "rivers",      target: "atatal",  gloss: "river.PL" }
    { source: "mountains",   target: "pepeli",  gloss: "mountain.PL" }
  ]

  applies_when: "the source sentence contains a plural noun"
  depends_on:   []
  productivity: "core"
  notes:        ""
}
```

And a third for verb tense:

```hjson
{
  rule_id:      "verb-tense-particles"
  title:        "Verb tense via particles"
  category:     syntax

  rule:         '''
    Tira verbs don't inflect for tense.  Tense is
    marked by a particle placed immediately AFTER
    the verb:

        -mi    present
        -lo    past
        -sa    future

    Tira is SOV, so the order is:
        SUBJECT  OBJECT  VERB-TENSE

        atal nan ta-mi sora.   (the river flows you)
                               actually: river  you-OBJ  flow-PRES
                               → "you make the river flow", roughly.
  '''

  examples: [
    { source: "the river flows",          target: "atal sora-mi",      gloss: "river flow-PRES" }
    { source: "the river flowed",         target: "atal sora-lo",      gloss: "river flow-PAST" }
  ]

  applies_when: "the source sentence contains a verb"
  depends_on:   []
  productivity: "core"
  notes:        ""
}
```

The `rule` field is a multi-line HJSON string —
use `'''` to open and close.  Indent doesn't
matter to the parser; readability is the goal.

The `examples` array is **few-shot data** —
during translation, the LLM sees these as worked
examples of the rule applied.  More examples →
better translations.

## Step 5 — define phonology rules

Tree cursor on `Language/Tira/Phonology` → `+` →
type `syllable-template` → Enter.

```hjson
{
  rule_id:      "syllable-template"
  title:        "Syllable template — CV(C)"
  category:     phonotactics

  rule:         '''
    Tira syllables follow the template CV(C):

      ONSET:    exactly one consonant (no clusters)
      NUCLEUS:  exactly one vowel (no diphthongs)
      CODA:     zero or one consonant, only from
                {l, n, r, s}

    Examples:
      a-tal      (V.CVC — onset can be null word-initial)
      pe-li      (CV.CV)
      ki-ma      (CV.CV)
      a-ta-tal   (V.CV.CVC — reduplicated plural)
  '''

  examples: [
    { input: "atal",   output: "/ˈa.tal/",   gloss: "river" }
    { input: "peli",   output: "/ˈpe.li/",   gloss: "mountain" }
  ]

  exceptions: []
  register:     ""
  notes:        "Word-initial vowels are allowed (null onset)."
}
```

And the consonant inventory:

```hjson
{
  rule_id:      "consonant-inventory"
  title:        "Consonant inventory"
  category:     consonants

  rule:         '''
    Tira has 12 consonants:

      Stops:     p   t   k
      Nasals:    m   n
      Fricatives: s
      Liquids:   l   r
      Glides:    (none)

    Voiced stops (/b/, /d/, /g/) do NOT appear in
    native vocabulary; loan words may be respelled
    with the voiceless equivalents.
  '''

  examples: []
  exceptions: []
  notes:        "Symbol inventory matches IPA except where noted."
}
```

## Step 6 — add sample texts

Tree cursor on `Language/Tira/Sample texts` → `+`
→ type a title (`river-greeting`) → Enter.

Sample-text paragraphs are NOT seeded with a
template — they're free-form prose.  Write a
short text in Tira with a gloss on the next line:

```
Atal nan ta-mi sora.
"The river flows for you."  (literally: river you-OBJ flow-PRES)
```

Add 2-3 more so the translation prompt has
register variety:

```
Pepeli kima-mi.
"The mountains are green."  (mountain.PL green-PRES)

Atatale sora-lo.
"It flowed of the rivers."  (river.PL.GEN flow-PAST)
```

The translation prompt envelope picks up to 3
sample texts as register anchors.

## Step 7 — see the overlay in your manuscript

Open any user-book paragraph and write prose that
includes a Tira word.  For example, in a chapter:

> The traveller knelt beside the **atal** and
> washed the dust of the road from her hands.
> Above her, the **pepeli** caught the last light
> of the setting sun.

`atal` and `pepeli` light up in italic
`theme.language_word_fg` (default mauve-teal
`#b4a8e1`).  Move the cursor onto `atal` and the
editor footer chip reads:

```
[atal · noun · river]
```

— the lemma, part of speech, and translation lifted
live from the entry's HJSON.  Move off → chip
disappears, goal gauge (if any) comes back.

The overlay catches paradigm forms too: `atale`
(genitive) and `atatal` (plural) light up the same
way because they're listed in the entry's
`inflection` field.  This is the closing of the
"Snowball gap" for invented languages — no
off-the-shelf stemmer knows Tira, but the
inflection paradigm tells the lexicon walker
which forms to recognise.

## Step 8 — translate INTO Tira

Cursor in a user-book paragraph (the source), press
**`Ctrl+B Q`**.

Single-language project → translation kicks off
directly.  Multi-language project → picker pops
showing every defined language; use `↑↓` + `Enter`
or just type the first letter (`t` for Tira) to
jump-and-commit.

The composer assembles the prompt envelope:

1. **System prompt** — explains the LLM's role as
   a translator between the working language and
   Tira.
2. **Meta/overview** — Tira's identity, alphabet,
   word_order, morphology.
3. **Grammar rules** — all three you wrote
   (`noun-cases`, `plural-reduplication`,
   `verb-tense-particles`).
4. **Phonology rules** — `syllable-template`,
   `consonant-inventory`.
5. **Dictionary** — RAG-filtered to entries whose
   `translation` appears in the source paragraph.
6. **Sample texts** — first 3 paragraphs from
   `Sample texts`.
7. **Source paragraph** — the actual prose.

The envelope size at this scale runs about 3-5K
tokens — comfortably within any modern model's
window.

The AI pane title shows `translate[on]` in italic
mauve-teal while the stream is in flight so you
know the `I` apply chord will use translation
extraction.

The LLM responds with:

```
<<<TRANSLATION>>>
Atal sora-mi nane ta. Pepeli kima-mi, sora-lo ka-mi sora.
<<<END>>>

Per-token gloss:
| Source       | Gloss          | Target  |
|--------------|----------------|---------|
| river        | river.NOM      | atal    |
| flows        | flow.PRES      | sora-mi |
| for you      | you.OBJ        | nane ta |
| mountains    | mountain.PL    | pepeli  |
| are green    | green.PRES     | kima-mi |

Applied rules:
  - noun-cases (atal.NOM, nane.GEN→OBJ via context)
  - plural-reduplication (pepeli)
  - verb-tense-particles (sora-mi, kima-mi)

Confidence flags:
  - "for you" — Tira doesn't distinguish dative from
    accusative; rendered with OBJ particle `ta`.
    Suggest adding entry `ta: dative/object marker`
    if not already.
```

Press **`I`** in the AI pane.  The Insert chord
lifts ONLY the `<<<TRANSLATION>>>` block at your
cursor — the gloss table + applied-rules list +
confidence flags stay in the AI pane for your
reference but don't pollute the manuscript.

If the LLM forgot the markers, a second `I` press
falls back to inserting the full body verbatim.

## Step 9 — reverse-translate

**`Ctrl+B Shift+Q`** translates FROM Tira back to
the working language.  Same envelope shape, flipped
direction labels.

The natural roundtrip workflow:

1. Cursor on an English paragraph → `Ctrl+B Q` →
   land the Tira translation in the next paragraph
   via `I`.
2. Cursor on the Tira paragraph you just landed →
   `Ctrl+B Shift+Q` → AI pane shows the
   back-translation.
3. Compare against the original.

When the back-translation drifts beyond register
(e.g., "the river flows" → "the river of regal
pomp"), the grammar rules or dictionary entries
have an inconsistency the manuscript will
eventually trip over.  This is the in-TUI version
of what'll eventually be the headless `inkhaven
language test` corpus-driven drift detector
(Phase D.2).

## Step 10 — health check

```
$ inkhaven language doctor Tira

Language doctor — `Tira`

  name           : Tira
  kind           : constructed
  family         : Standalone constructed
  alphabet       : 13 entries
  direction      : ltr

Chapters
  Dictionary     : 7 parseable entries
  Grammar        : 3 rules
  Phonology      : 2 rules
  Sample texts   : 3 samples

Dictionary coverage
  with example   : 7/7 (100%)
  with paradigm  : 3/7 (42%)
  missing paradigm: 4 (overlay won't catch inflected forms)

Manuscript gap analysis
  unique words (≥2 chars) in manuscript prose: 412
  covered by dictionary: 2/412 (0%)
  uncovered words (sample, max 15):
    · above
    · and
    · beside
    · …
```

The gap analysis is honest about coverage —
Tira covers two manuscript words because Tira is
a new language sparsely used in prose.  The numbers
go up as you add vocabulary and weave Tira into
more passages.

For CI / shell scripting, pass `--json`:

```
$ inkhaven language doctor Tira --json | jq '.coverage.with_paradigm_pct'
42
```

Use this to gate releases: e.g., refuse to merge a
PR that drops paradigm coverage below 80%.

## Step 11 — list and export

`inkhaven language list` summarises every defined
language at a glance:

```
$ inkhaven language list
  name      words  grammar  phonology  samples
  ------------------------------------------------
  Tira          7        3          2        3
```

When you're ready to publish or share:

```
# Two-column printable Typst dictionary
$ inkhaven language export Tira \
    --format dictionary-twocol \
    --output dist/tira-dict.typ

# Anki / SuperMemo / Mochi flashcard deck
$ inkhaven language export Tira \
    --format anki \
    --output dist/tira.csv

# Full structured JSON for downstream tooling
$ inkhaven language export Tira --format json > dist/tira.json
```

The Typst output renders entries grouped under
alphabet headers (`— A —`, `— B —`, …), each entry
formatted as bold headword + italic POS +
translation + indented example + small-font
paradigm line.  Compile with `typst compile
dist/tira-dict.typ` to get a printable PDF.

## Authoring rhythm — what to do in what order

The fastest path from "I have an idea for a
language" to "the AI is translating my prose into
it":

1. **Sketch the linguistic shape first.**
   Open `Meta/overview` and fill `word_order`,
   `morphology`, `has_cases`, `has_gender`,
   `alphabet`.  This takes 5 minutes and sets the
   constraints the rest of the workflow operates
   within.

2. **Write 2-3 grammar rules.**  The minimum
   useful set: a case system (or word-order rule),
   a tense system (or aspect rule), a number-marking
   rule.  Without these the AI translator will
   guess randomly.

3. **Seed 10-20 dictionary entries** for words
   your manuscript actually uses.  Don't try to
   pre-populate every conceivable word — Tira
   grew its vocabulary as the manuscript needed
   it.  The `doctor` gap analysis tells you which
   prose words are uncovered.

4. **Write 3-5 sample texts.**  These are the
   register anchors the LLM uses to pitch its
   output.  Keep them in the register you want the
   manuscript translations to match.

5. **Translate, review, adjust.**  Most issues
   surface during translation:
   - LLM produces an off-register translation →
     adjust sample texts.
   - LLM mis-applies a grammar rule → make
     `applies_when` tighter or add more
     `examples`.
   - LLM asks for a missing word → `add-word` the
     entry it suggests.

6. **Roundtrip-test periodically.**  `Ctrl+B Q`
   → `Ctrl+B Shift+Q` → compare to original.
   Drift = rule inconsistency = manuscript bug
   waiting to happen.

## Cheat sheet

| Action | Chord / Command |
|-|-|
| Scaffold a new language (TUI) | Tree (`F8`) → cursor on `Language` → `b` |
| Scaffold a new language (shell) | `inkhaven language init <name>` |
| Add a dictionary entry (TUI) | Cursor under `<lang>/Dictionary` → `+` → type word → Enter |
| Add a dictionary entry (shell) | `inkhaven language add-word <lang> <word> --type <pos> --translation <text>` |
| Bulk-import a dictionary (CSV) | `inkhaven language add-word <lang> --import <path.csv>` |
| Remove a dictionary entry | `inkhaven language remove-word <lang> <word>` |
| Add a grammar / phonology rule (TUI) | Cursor under `<lang>/Grammar` or `<lang>/Phonology` → `+` → type rule_id → Enter |
| Translate INTO the language | `Ctrl+B Q` in editor |
| Translate FROM the language | `Ctrl+B Shift+Q` in editor |
| Insert translation at cursor | `I` in AI pane (lifts only the `<<<TRANSLATION>>>` block) |
| Health report (text) | `inkhaven language doctor <lang>` |
| Health report (JSON, CI-friendly) | `inkhaven language doctor <lang> --json` |
| List defined languages | `inkhaven language list` |
| Export | `inkhaven language export <lang> --format <fmt> --output <path>` |

## Common pitfalls

- **"My entry shows `= aag\n\n` instead of the
  template."**  You scaffolded on a pre-`90e51d7`
  build; the seed body wrote to bdslib but not to
  disk.  Delete the entry via `remove-word` and
  re-create it; new entries write through to disk
  correctly.
- **"The lexicon overlay doesn't light up inflected
  forms."**  The entry's `inflection: {...}` map
  is empty or missing.  Fill the paradigm — every
  value in that map gets added to the lexicon as
  an extra surface form.
- **"The translation is grammatically wrong."**
  Check the rule's `applies_when` — if it's too
  vague, the rule fires when it shouldn't.  Or
  add more `examples` so the LLM has more
  worked-pattern data.
- **"`Ctrl+B Q` shows zero matching dictionary
  entries."**  The RAG filter matches the entry's
  `translation` field against words in the source.
  If your entries' translations are multi-word
  phrases, the source has to contain the entire
  phrase verbatim (case-insensitive substring).
- **"Pressing `b` from anywhere creates a top-level
  book, not a language sub-book."**  The Language-
  scaffold path only fires when the cursor is on
  or inside the Language system book.  Move the
  cursor onto `Language` first.

## What's not in 1.2.13

Phase D.2 candidates (the §12 / §13 / §14 parts of
`Documentation/PROPOSALS/LANGUAGE_BOOK.md` that
didn't ship):

- `--format grammar` and `--format phrasebook`
  exports — need rule HJSON schema design (the
  current template is the right shape; the
  exporter doesn't yet parse it).
- `inkhaven language test <name>` headless
  roundtrip drift CLI.
- `inkhaven language translate` headless
  translation CLI.
- `Ctrl+B Shift+R` reverse-lookup picker
  ("find the entry whose translation is `X`").
- `Ctrl+B Shift+W` word-of-the-day floating
  card in the manuscript editor + phonotactic
  generator in the Language book.
- Card renderers for Dictionary / Grammar /
  Phonology paragraphs viewed inside the Language
  book (the §7 / §10 visualisations from the
  proposal).

The plumbing for all of these is in place; they're
chord / render work, not data-model work.
