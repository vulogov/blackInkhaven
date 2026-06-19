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
    { id: "pl",  gloss: "PL",  form: "i",  position: "suffix" }
    { id: "dat", gloss: "DAT", form: "d",  position: "suffix" }
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

## Worldbuilding links

Stored in `.inkhaven/conlang-links.json` (the prose books are never modified):

```
inkhaven language link-place Tirion Quenya [--secondary]
inkhaven language link-character Erendil Quenya native   # native|fluent|conversational|broken|reading_only
inkhaven language speakers Quenya
```

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
