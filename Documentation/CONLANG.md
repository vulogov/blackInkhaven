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
  diphthongs: ["ai", "au"]                         // one nucleus each; other vowel pairs are hiatus (a.i.a)
  romanizations: [
    { name: "default", mappings: [ { ipa: "k", roman: "c" }, { ipa: "s", roman: "c" } ],
      contextual: [ { roman: "c", ipa: "s", before: "FrontV" } ] }
  ]
  default_romanization: "default"
  tone: { kind: "contour", tones: ["1","2","3","4"], sandhi: [ { rule: "3 > 2 / _ 3" } ] }
}
```

**SPE rule notation** (allophony, tone sandhi, sound changes, ablaut):
`LHS > RHS / LEFT _ RIGHT`. `_` marks the target, `#` a word boundary, `∅`/`0`
the empty string (insertion on the left, deletion on the right). A token is a
class name when one is declared, else a literal phoneme. Target and
replacement are sequences like the contexts: `a i > e / _ #` monophthongises,
`t > t s / _ i` affricates into two segments. A joined token that is not a
declared phoneme or class (`ai > e`, `t > ts`) is split against the inventory,
so both spellings are the same rule; a declared `ai` phoneme stays one segment.

Inspectors: `generate-word`, `syllabify --word`, `ipa --word` (surface),
`stress --word`, `romanize --text [--reverse] [--scheme]`, `tone --tones`.

**Blocks are validated, not just parsed.** An unknown `constraints[].kind`
no longer disables the language: the constraint is ignored and reported.
Every command that loads the phonology prints one `phonology error:` /
`phonology warning:` line per finding to stderr, and `language doctor` lists
the same findings under *Blocks* with their location (`constraints[2]`,
`templates.root[0] "C V N"`, `allophony[1] "ai > e / _"`; `--json` carries
them under `blocks.findings`). Errors are things that silently never apply: an
unknown constraint kind, a class named in a constraint or template but not
declared under `classes`, or a rule target or context that is neither a
phoneme nor a class. Warnings cover a class member or bigram outside the
inventory, a duplicate phoneme, and a joined rule token whose split is
ambiguous (`abc` over an inventory with both `ab` and `bc`: it is read greedily
as `ab c`, and the warning shows the other reading — write it with spaces).
Malformed HJSON under Phonology or Grammar is reported by paragraph title.

**Segmentation is case- and normalisation-insensitive.** Every word, headword,
grapheme, class member and rule atom is NFC-normalised when loaded or typed, and
the segmenter case-folds its input, so `Pata`, `pata`, and a decomposed `pãta`
(`a` + U+0303) all read as the same phoneme sequence. A segment that still
fails to match prints with its codepoint (`̃ (U+0303)`) so a stray combining
mark is never shown as a bare accent. Graphemes that differ only by case are
therefore not distinguishable.

## Morphology (the `Grammar` chapter)

```hjson
{
  kind: "agglutinative"
  morphemes: [
    { id: "pl",  gloss: "PL",  form: "i",  position: "suffix", precedence: 2, category: "number", value: "plural" }
    { id: "dat", gloss: "DAT", form: "d",  position: "suffix", precedence: 1, category: "case",   value: "dative" }
    { id: "def", gloss: "DEF", form: "na", position: "prefix", category: "definiteness" }
    // non-concatenative: an infix, a circumfix, ablaut, and reduplication
    { id: "ag",  gloss: "AG",   form: "um", position: "infix", anchor: "before_first_vowel" }
    { id: "ptcp", gloss: "PTCP", form: "ge_t", position: "circumfix" }   // ge_ + stem + _t
    { id: "pst", gloss: "PST", process: "ablaut", rules: [ { rule: "i > a" } ] }
    { id: "rdp", gloss: "INTENS", process: "reduplication", reduplicate: "initial_cv" }
  ]
  paradigms: [ { name: "noun", cells: [
    { features: { number: "sg", case: "nom" }, morphemes: [] }
    { features: { number: "pl", case: "dat" }, morphemes: ["dat", "pl"] }
  ] } ]
  agreement: [
    { dependent: "adjective", head: "noun", features: ["number", "case"], paradigm: "adj" }
  ]
}
```

`inkhaven language paradigm <lang> --root kata --template noun --gloss stone`
applies each cell's morphemes to the root, runs allophony across the affix
boundaries, and prints the form + Leipzig gloss.

**Affix types.** A morpheme is either a concatenative affix at a `position`
(`prefix` | `suffix` | `infix` | `circumfix`) or a non-concatenative `process`:
- **infix** — inserted inside the stem; `anchor` is `before_first_vowel` (the
  default, i.e. after the first consonant — `sulat` → `s‑um‑ulat`) or
  `after_first_vowel`.
- **circumfix** — wraps the stem; `form` uses `_` for the stem slot (`ge_t` →
  `ge` + stem + `t`).
- **ablaut** — `process: "ablaut"` with SPE `rules` applied inside the stem
  (`kit` + `i > a` → `kat`).
- **reduplication** — `process: "reduplication"` with `reduplicate`: `full` |
  `initial_cv` | `initial_syllable` | `final_syllable` (`kata` → `kakata`).

**`precedence`** controls how close a stacked affix sits to the root: `0` (the
default) = any position (declared order kept); `1` = next to the root; `2` = the
next slot out; … — so `kata` + DAT(1) + PL(2) → `katadi`, `stone-DAT-PL`,
regardless of cell order.

**`category` / `value`** (`number` / `plural`, `case` / `dative`) tag a morpheme
for the reference grammar, which groups affixes by category.

**Agreement (concord).** `agreement` rules make a `dependent` part of speech
copy `features` from its `head`, realised through a named `paradigm`:
`inkhaven language agree <lang> --word mira --pos adjective --gloss bright
--features "number=pl,case=nom"` inflects the adjective to agree (`mira` →
`mirai`, `bright-PL`).

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

## Syntax (sentences)

`inkhaven language sentence <lang>` assembles a clause from its parts and prints
the surface form, an interlinear gloss, and a literal rendering — putting word
order, case, and agreement together into something speakable:

```
inkhaven language sentence Eldar --subject kira:bird --verb nami:see \
    --object pata:stone --object-adj mira:bright
```

The engine orders the words by the typology `word_order` (`sov`, `svo`, …),
case-marks the nouns by `alignment` (nominative–accusative → subject nominative,
object accusative; ergative–absolutive → subject ergative, object absolutive),
inflects each noun through the `noun` paradigm to its case (and number), and runs
agreement (an adjective takes its noun's case; a verb agrees with its subject via
the `verb` agreement rule). Words are `root` or `root:gloss`; adjective placement
follows the `adjective_order` typology feature. It degrades gracefully — a
missing paradigm or case just leaves a word bare. The **grammar book** uses this
to print a worked example sentence from the lexicon.

**Negation and questions** layer on top, realized by the `negation` and
`question` typology features:

```
inkhaven language sentence Eldar --subject kira:bird --verb nami:see \
    --object pata:stone --negate --negator na:not
inkhaven language sentence Eldar --subject kira:bird --verb nami:see \
    --object pata:stone --question --q-particle ka:Q
```

- `--negate` follows the `negation` strategy: `particle`/`auxiliary` sets the
  negator (`--negator`) as a separate word before the verb (glossed `NEG`);
  `affix` fuses it onto the verb form (`NEG-…`). With no `--negator`, only the
  gloss is marked — the engine never coins a word.
- `--question` follows the `question` strategy: `particle` appends the question
  particle (`--q-particle`, glossed `Q`); `word_order` fronts the verb
  (inversion); `morphology` tags the verb (`.Q`); all add a surface `?`.

**Relative clauses** modify a noun — "the bird that sees the stone":

```
inkhaven language relative Eldar --head kira:bird --role subject \
    --verb nami:see --with pata:stone --relativizer ya:that
```

The head plays a role inside the embedded clause — its `subject` ("the bird that
sees …") or its `object` ("the stone that … sees"); that role is the *gap*, and
`--with` supplies the other argument. The embedded clause is assembled by the
same engine, so it case-marks and agrees correctly (the example yields `kira ya
nami patan` — *bird REL see stone-ACC*). Placement follows the `relative_clause`
typology: `prenominal` puts the clause before the head (Japanese, Chinese),
otherwise after it (English, the default).

**Coordination** joins noun phrases or whole clauses with a conjunction:

```
inkhaven language coordinate Eldar --np kira:bird --np pata:stone --conjunction na:and
inkhaven language coordinate Eldar --conjunction na:and \
    --clause "kira:bird nami:see pata:stone" --clause "muru:river tasa:fall"
```

Give two or more `--np` (each a single `root:gloss` noun) **or** two or more
`--clause` (each space-separated `root:gloss` words — subject, verb, optional
object). The conjunction sits between adjacent conjuncts, glossed by its own
gloss; clause conjuncts are each assembled (so case marking still applies:
`kira nami patan na muru tasa` — *bird see stone-ACC and river fall*).

**Complement clauses** let a whole clause be the object of a verb of speech or
cognition — "I know *that the bird sees the stone*":

```
inkhaven language complement Eldar --subject mi:I --verb tira:know \
    --complementizer ya:that \
    --comp-subject kira:bird --comp-verb nami:see --comp-object pata:stone
```

The matrix subject and verb (`--subject`/`--verb`) wrap an embedded clause
(`--comp-subject`/`--comp-verb`/`--comp-object`) introduced by an optional
complementizer (glossed `COMP`). The embedded clause runs through the same engine
(its object stays accusative), and because the complement fills the matrix
**object** slot, word order positions it correctly: an SVO language prints it
after the matrix verb (`mi tira ya kira nami patan`), a verb-final language before
it (`mi ya kira patan nami tira`).

## Creative text (compose)

`inkhaven language compose <lang> --kind <kind>` generates creative text. The
deterministic kinds are grounded in what you've built (seed with `--seed` for a
different draw):

```
inkhaven language compose Avesha --kind names  --count 6 --seed 3
inkhaven language compose Avesha --kind prose  --count 3
inkhaven language compose Avesha --kind poem   --meter 5,7,5
```

- **`names`** — phonotactically-valid, capitalised names from the `root`
  templates (the same generator that feeds the lexicon — every name is sayable).
- **`prose`** — grammatical sample sentences assembled through the **syntax
  engine** (word order, case, agreement), each with an interlinear gloss and a
  literal — the language actually speaking.
- **`poem`** — metered verse: one line per `--meter` syllable count (e.g.
  `5,7,5`), each line drawing real words until it scans.

The themed kinds **`blessing`**, **`curse`**, and **`incantation`** are AI-composed
(need `--provider`) but **constrained to the existing lexicon** — the model
arranges real words, never invents them, and emits the native text, an
interlinear gloss, and a translation. Any token not found in the lexicon is
flagged. `compose` only prints — nothing is written to the book.

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

## Varieties — dialects, registers, sociolects (LANG-2 P1)

A language is not uniform. A **variety** is a *delta* on the base language —
declared in a `varieties` block in the **Grammar** chapter:

```hjson
{ varieties: [
  { id: "lowland", kind: "dialect", axis: "region", prestige: "low",
    sound_changes: [ { rule: "t > d / V _ V" } ],   // SPE, as in diachronics
    lexicon: { "water": "móru" } }                  // suppletive overrides
  { id: "high", kind: "register", axis: "formality", prestige: "high",
    sound_changes: [ { rule: "k > q / # _" } ] }
] }
```

The key idea: **a dialect's sound differences are an ordered set of sound changes
— the same `AllophonyRule` engine as diachronics — applied _synchronously_** (a
living variety) instead of _diachronically_ (a daughter language). A variety may
also override individual words (suppletive forms the sound changes don't derive).

```
inkhaven language varieties Eldar                      # list them
inkhaven language lect Eldar lowland --word kata        # → kada (t > d / V_V)
inkhaven language lect Eldar lowland --text "kata tira" # word by word
inkhaven language dialects Eldar [--count N]            # comparison table
```

`varieties` lists each variety with its axis/prestige and delta sizes; `lect`
renders a form or text *in* a variety; `dialects` prints the classic
dialectology comparison — each headword across the base and every variety (a
trailing `*` marks a word override). All are scriptable from Bund
(`lang.varieties`, `lang.lect`).

## Contact — borrowing (LANG-2 P2)

A loanword is a **phonotactic repair**: a donor word is *perceived* against the
recipient's inventory, then any sequence the recipient forbids is fixed. How a
language nativises borrowings is declared in a `loan_phonology` block in its
**Phonology** chapter:

```hjson
{ loan_phonology: {
  repair: "epenthesis",          // epenthesis (insert a vowel) | deletion
  epenthetic_vowel: "u",         // empty → the first declared vowel
  substitutions: { "θ": "t", "r": "l" }   // a donor sound the recipient lacks → nearest native
} }
```

```
inkhaven language borrow Eldar --form tras --from Drake            # tras → tulasu
inkhaven language borrow Eldar --form θuk --from Drake --gloss demon --yes
```

Adaptation runs in two steps: **perceive** (apply the substitutions, keep sounds
the recipient has, map the rest to the nearest native phoneme by sonority) and
**repair** (consonant runs longer than the recipient's templates allow get the
epenthetic vowel — Japanese *sutoraiku* — or the offending consonant is deleted).
The donor form is given *phonemically* (one symbol per sound). With `--yes` and a
`--gloss`, the adapted word joins the recipient's Dictionary with the donor
recorded in its etymology (so cognates/etymology stay coherent). Scriptable from
Bund as `lang.borrow` (advisory — returns `{ donor, adapted, steps }`; commit
with `lang.add_word`).

### Areal features — Sprachbund (LANG-2 P3)

Languages in contact converge on shared structures. A `contact` block in the
**Grammar** chapter declares a language's membership in a *linguistic area*:

```hjson
{ contact: {
  region: "the Inner Sea"
  with: [ "Sindar", "Khuz" ]                                  // neighbours in contact
  areal_features: { word_order: "sov", alignment: "ergative_absolutive" }
} }
```

```
inkhaven language areal Eldar      # per-language convergence overlay
inkhaven language areal            # the whole-world regional view
```

`areal <lang>` assesses each areal feature against that language's own typology —
**converged** (already has it, `✓`), **shift** (a different value, would change,
`→`), or **adopt** (unanswered, would gain it, `+`). It is an *advisory overlay*:
it never rewrites the grammar. `areal` with no language prints the regional
Sprachbund view — each contact area, its members, and a per-member status for
every shared feature. Contact also shows up as **horizontal edges** in
`language family-tree` (`Eldar ⇄ Sindar`), alongside the vertical inheritance.
Scriptable as `lang.areal` (returns `{ region, with, convergence }`).

### AI advisories (LANG-2 P6)

The AI proposes *choices*, the deterministic engine *applies* them — so the forms
stay legal and nothing is written without `--yes`:

```
inkhaven language propose-dialect Eldar --describe "a harsh mountain dialect" [--yes]
inkhaven language propose-loans Eldar --from Drake --topic seafaring --count 6
inkhaven language areal-check Eldar
```

- **`propose-dialect`** — the model suggests a coherent set of sound changes + a
  few lexical swaps for the requested flavour; each rule is validated against the
  inventory and previewed; `--yes` writes the variety into the Grammar chapter.
- **`propose-loans`** — proposes concepts a language would borrow from a donor in
  a topic domain, with donor forms, then the **deterministic adapter** (the
  borrowing engine) nativises each (so `stɔrm → sitarimi`). Add the ones you like
  with `borrow … --yes`.
- **`areal-check`** — assesses whether a declared Sprachbund is typologically
  plausible (the contact analogue of `realism-check`). All key off the project
  working language.

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

**One headword check at every door.** `add-word`, `add-word --import`, the
in-TUI `+` chord and `language import` (Toolbox / PolyGlot) all run the same
check, so a word one accepts the others accept and `export --format csv` →
`--import` is a no-op. A headword must be 1–64 characters with no control
characters. Its spelling is checked against the Phonology inventory when one is
declared (every greedy-matched segment must be a declared phoneme), else
against `Meta/overview.alphabet` (every letter declared, longest-match, case-
and diacritic-insensitive); `--force` skips the spelling check on the CLI. The
headword is keyed once: NFC for the title, case-folded for the duplicate check
(`ŋa` and `Ŋa` are one word), and bucketed on its first alphabet letter — or,
with no alphabet entry covering it, on its first base letter (`ą́tal` and
`atal` both file under `A`). A homograph (same spelling, different `type`) is
allowed and titled `pata (2)`.

**The alphabet orders the book.** Buckets sit in `Meta/overview.alphabet`
order (an undeclared bucket such as `Ŋ` after every declared one), entries
inside a bucket collate letter by letter by the declaration (`Ch` is a letter
between `B` and `D`, `ŋ` sorts after `z`, `pata (2)` follows `pata`), and
`export`, `dictionary` and the two-column render use the same order with one
section per alphabet entry. Every door keeps the order as it adds words;
`language sort-dictionary <lang> [--dry-run]` repairs a dictionary built
before 3.18 or rearranged by hand, renaming the `NN-` file prefixes in one pass
per bucket and listing each move.

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

### Semantic-gap finder

```
inkhaven language gaps Avesha [--scope swadesh_100 | scope.hjson] [--json]
```

Diffs the lexicon against a reference *concept scope* and reports which concepts
are still missing, frequency-ranked (most-core first) — the exact list to coin
next. The default scope is the **Swadesh-100** core vocabulary, bundled in every
working language (en/ru/fr/de/es) and matched against your glosses
Unicode-aware (articles and `to`-infinitives don't hide a match, so "the sun"
covers *sun*). Point `--scope` at an HJSON file for a topic of your own:

```hjson
{ name: "Seafaring", concepts: ["hull", "tide", "mast", { label: "harbor", aliases: ["port"] }] }
```

The missing list is shaped to hand straight to `generate-lexicon --topic …`.

**Output paths.** Every `language` command that writes a file the user names
takes `-o, --output <PATH>` (3.18). The older `--out` spelling, and `--svg` on
`ecology`, still parse as hidden aliases. `font-build` without `--output`
writes under `<project>/exports/<lang>/fonts/` and prints the path. See
[`CLI.md`](CLI.md) for the convention.

## Dictionary output

```
inkhaven language dictionary Avesha --format md|typ [-o dict.typ] [--font Eldar]
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
inkhaven language grammar-book Avesha --format md|typ [-o g.typ] [--font Eldar] \
    [--study --provider …]
```

The companion volume: a **reference grammar**, drawing every section from the
language's own data — phonology (consonant/vowel inventory, syllable structure,
phonotactics, allophony, stress, tone), morphology (affixes, derivation), the
typology answers (with their consequences), a sample sentence, idioms &
metaphors, and the sample texts. When the language declares varieties or contact
(LANG-2), it also gains a **Variation** section (the dialects/registers + a
dialect-comparison table) and a **Contact** section (the linguistic area, shared
areal features, and loanword adaptation). Markdown is a flat reference; **Typst**
is the same manual-style B5 book (title page, contents, sections) as the
dictionary, with the conscript font.

With **`--study`** it also becomes **study material**: an AI-written study guide
leads the book, defining and explaining every linguistic term the reference uses
(phoneme, allophony, grammatical case, SOV word order, nominative–accusative
alignment, agent nouns, …) and how this language applies them. The reference
itself stays deterministic; only the study guide is AI-authored (needs a
provider).

```
inkhaven language tutorial Avesha --format md|typ [-o learn.typ] [--font Eldar] [--provider …]
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

## Interchange (export)

`inkhaven language export <lang> --format <fmt> [-o F]` writes the lexicon to
a portable artefact:

```
inkhaven language export Avesha --format xliff   > avesha.xlf
inkhaven language export Avesha --format linguex > avesha.tex
inkhaven language export Avesha --format ipa-chart
```

- `json` — full structured dump (dictionary, grammar, phonology, samples).
- `anki` / `csv` — flashcard / round-trippable CSV (the `--import` path re-ingests `csv`).
- `dictionary-twocol` / `grammar` / `phrasebook` — printable Typst (need `--output`).
- **`xliff`** — XLIFF 1.2 translation memory: each entry is a `trans-unit`
  (working-language *source* → invented-word *target*), loadable into CAT tools
  (OmegaT, memoQ, Weblate). The source language code follows the project working
  language; the conlang gets a BCP-47 `art-x-<slug>` tag.
- **`linguex`** — LaTeX using the `linguex` package: bold headword + POS + gloss,
  with any example as a numbered `\ex.` — paste-ready for a paper or grammar sketch.
- **`ipa-chart`** — Markdown IPA inventory: consonants and vowels grouped, each
  with its romanization.

## Interchange (import)

Pull a lexicon in from another tool. `add-word --import <csv>` reads Inkhaven's
own round-trippable CSV; `language import` reads foreign formats:

```
inkhaven language import Avesha --file lexicon.sfm --format toolbox        # preview
inkhaven language import Avesha --file lexicon.sfm --format toolbox --yes  # write
inkhaven language import Avesha --file MyLang.pgd  --format polyglot --yes
```

- **`toolbox`** — Toolbox / MDF **Standard Format** (`\lx … \ps … \ge …`), the
  lingua franca of descriptive lexicography. The same SFM that **SIL Toolbox**,
  **FieldWorks**, and **Lexique Pro** read and write, so all three import here.
- **`polyglot`** — a **PolyGlot** dictionary: pass the native `.pgd` archive (its
  `PGDictionary.xml` is unzipped for you) or a raw exported `.xml`. The
  part-of-speech table is resolved so each word lands with its class.

Import **previews by default** — it prints what it would add and changes nothing
until you pass `--yes`. Duplicate headwords, and words that fail the headword
check above, are skipped with the reason. (Tools that export CSV, such as
ConWorkShop, can come in through `add-word --import`.)

## Worldbuilding links

Stored in `.inkhaven/conlang-links.json` (the prose books are never modified):

```
inkhaven language link-place Tirion Quenya [--secondary] [--variety lowland]
inkhaven language link-character Erendil Quenya native [--native-variety court]
inkhaven language speakers Quenya
```

### Speech communities & ecology (LANG-2 P4)

The links carry **varieties** too: a Place speaks a particular dialect/register
of its language (`--variety`), and a Character has a **native variety** — the
base of their idiolect (`--native-variety`).

```
inkhaven language ecology                 # who speaks what (and which variety) where
inkhaven language ecology --svg atlas.svg  # a node-link atlas of the contact areas
inkhaven language idiolect Tost --text "kata tira"   # render in a character's dialect
```

`ecology` reports the whole speech-community picture — places with their language
+ variety, characters with their commanded languages + native variety, and the
contact areas (P3); `--svg` writes a labelled atlas. `idiolect <character>`
renders a form or text in that character's native variety (their primary language
+ native variety, run through the P1 variety engine) — so a marsh-dweller's
speech comes out in the lowland dialect. Scriptable from Bund as `lang.ecology`
and `lang.idiolect`.

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
    with a hook" --phoneme p [-o p.svg] [--yes]            # AI text-to-SVG draft
inkhaven language font-import-glyph Eldar --svg ./a.svg \
    --phoneme a [--codepoint U+E000] [--name a]            # bind + store + record
inkhaven language font-config Eldar [--json]               # show the bindings
inkhaven language font-build --language Eldar \
    [--format ufo|ttf|both] [-o Eldar] [--upm 1000]        # compile from the book
```

- **`glyph-lint`** reports whether an SVG is fit for a font outline (filled
  paths required; stroke-only / image / gradient glyphs are flagged). It also
  warns on non-black fills — a near-white fill among darker ones is almost
  always a counter the author drew with white paint, which a monochrome font
  won't honour (cut counters with a reverse-wound subpath instead).
- **`glyph-draft`** asks the AI to draft an SVG glyph from a description, runs it
  through the same preflight, and previews the result. Advisory: it prints the
  SVG (or writes `--output`) and the verdict; only `--yes` (and only a *usable*
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
    --slot left=lead --slot right=vowel [-o ka.svg] [--yes]
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
    --name quadrat_sunbar --slot top=sun --slot bottom=bar [--size 2em] [-o q.typ]
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

## The Linguistic companion (`inkhaven linguistic`)

A full-screen TUI over the Language book, sibling to `inkhaven research` and
`inkhaven worldbuilder`:

```
inkhaven linguistic [--language <name>] [--session <name>]
```

The tree of your languages sits on the left; the right pane cycles with `Tab`
through **Chat** (a grounded conversation with the Inner Linguist — each answer
is retrieved from the language sub-book that contains your selection), the
node **Preview**, and the **Metrics** (`m`), **Universals** (`u`) and **Minimal
pairs** (`p`) reports for the selected language. `i` or `/` opens the prompt;
`Enter` sends a question or runs one of the local `/commands` (`/trace`,
`/parse`, `/check`, `/tree`, `/clause`, `/igt`, `/texts`, `/settrans`,
`/frequency`, `/kwic`, `/coll`, `/hypotheses`, `/hcheck`); `Esc` leaves the
prompt. The chat is a named session under `.inkhaven/linguistic-sessions/`
that replays when you reopen it. The full key table is in
[KEYBINDING.md](KEYBINDING.md#inkhaven-linguistic-170-ling-1).

## In the editor

- **`Ctrl+B X`** — the ConLang hub: a read-only overview of every language
  (inventory, counts, prosody, romanization, lexicon size, speakers).
- **`:lang:`** — type `:<name-or-iso>:` to open a lexicon picker that inserts
  the chosen word in place of the trigger.
- **`Ctrl+B Q` / `Ctrl+B Shift+Q`** (1.2.13) — translate a paragraph to / from
  an invented language.

## Translation — Tier 1 RBMT (LANG-3 P0)

A constructed language can be *translated into*, deterministically and offline,
the moment it has a lexicon. RFC LANG-3 plans a three-tier stack (a rule-based
spine, a per-language neural model, an optional resolver); the first tier — the
pure-Rust **rule-based machine translator (RBMT)** — is in:

```
inkhaven language translate <lang> "the bird sees the stone" [--trace] [--json]
```

Why this is feasible for a conlang where it is not for a natural language: a
LANG-1 language carries a *complete formal description*, so translation is mostly
**orchestration over the existing engines**. An English sentence is analysed into
a simple clause, each word is mapped to a headword **by its lexicon gloss**, and
the result is handed to the **LANG-1 syntax engine** (`language sentence`) — which
already orders by `word_order`, case-marks by `alignment`, inflects via the
morphology spec, and runs agreement. So a translation reorders and inflects for
free:

```
the bird sees the stone   →  kira nami pata   (SVO)
                          →  kira pata nami   (SOV — same lexicon, different word_order)
```

Every word carries a confidence and a decision **trace** (`--trace`): which
lexicon entry, or *untranslatable* — an English word the lexicon does not cover
is passed through marked `«word»`, listed so you can coin it or `add-word` it.
Output is human-readable or `--json`.

**A paragraph is translated sentence by sentence.** Input is split on `.`, `!`,
`?` and line breaks; each clause is analysed and rendered on its own and the
results are re-joined with the source punctuation, so `the stone. the river
flows` gives `«stone». atal sora`, never just the last sentence. A clause the
analyser cannot place at all is kept as `«…»`. The reported confidence is the
*minimum* over the clauses, so one untranslatable sentence is not averaged away.
The same split applies to `reverse` and `cross` (the TUI chord passes the whole
paragraph).

It also runs **in reverse and across languages**:

```
inkhaven language reverse <lang> "kira nami pata"          # → "the bird see the stone"
inkhaven language cross <from> <to> "kira nami pata"       # conlang → conlang via English
```

`reverse` (conlang → English) un-inflects each surface word against the lexicon's
*paradigm-generated* forms (so an inflected word finds its lemma), reads roles off
the language's `word_order`, and generates a plain English clause. `cross`
(conlang A → conlang B) is the documented degraded path — it reverses A into
English, then forwards that English into B, **exposing the English pivot** and
multiplying the two passes' confidence (error compounds). So an SVO language's
`kira nami pata` crosses into an SOV language as `turi moki vela`, reordered for
free.

Source parsing is **lexicon-aware**: a word's part of speech comes from the
lexicon, so the verb is found by meaning (not position) and an attributive
adjective is recovered on a noun phrase (`the bright bird` → `mira kira`, the
adjective agreeing with its noun). Number flows both ways — `the birds see the
stones` inflects to `kirai nami patai`, and reverse re-agrees English number and
tense (`kirai nami patai` → *the birds see the stones*, `kira …` → *the bird
**sees** …*).

### Translation memory (Tier 2 — retrieval; LANG-3 Amendment A1)

Tier 2 layers a **translation memory** over the rule-based result rather than a
trained neural model — for a closed conlang the corpus *is* the knowledge, so it
is retrieved at translation time instead of baked into weights (Python-free,
training-free, zero new deps). You confirm a translation and it is reused
**immediately, on the very next call** — no retraining:

```
inkhaven language remember Eldar --english "the bird sees the stone" --conlang "kira pata nami"
inkhaven language translate Eldar "the bird sees the stone"   # → kira pata nami (from memory; RBMT kept as alt)
inkhaven language memory Eldar                                 # list remembered pairs
```

An **exact** remembered translation becomes the primary (the rule-based reading is
kept as an `alt:` so nothing is silently overridden); a **near** match is surfaced
as an alternative to pick. A miss is pure RBMT. Memory lives in the
`.inkhaven/translation-memory/<lang-slug>.json` sidecar — the slug is the
language name lower-cased and reduced to `[a-z0-9._-]`, so a title with spaces
or a `/` still maps to one file (atomic writes; the books are never touched).

Near matches are **semantic**: each remembered English source is embedded once
with the in-tree `fastembed` model (the same one powering manuscript search) and
the vector is cached in the sidecar; a translation embeds only the *query* and
finds the nearest remembered sentence by cosine similarity. So a lexical
paraphrase still recalls a confirmed translation — *"a soldier lifts his blade
toward the sun"* matches a remembered *"the warrior raises his sword to the sun"*
at 92% even with no shared content words. The exact path needs no embedding (a
seeded sentence costs nothing), and the retrieval strategy (exact → semantic →
lexical) lives in one place so the merge policy is identical however the match was
found.

You don't have to seed the memory by hand. **Corpus generation** translates a
pool of English sentences with the RBMT and keeps the ones the language fully
covers:

```
inkhaven language corpus Eldar                 # preview: acceptance rate + samples
inkhaven language corpus Eldar --yes           # seed the accepted pairs into memory
inkhaven language corpus Eldar --pool big.txt  # use a custom English pool
```

A small English pool ships in the binary (core vocabulary); supply a larger one
with `--pool`. The report's **acceptance rate is a reading of lexicon maturity**,
and the **top missing words** tell you exactly what to `add-word` to raise
coverage. Previews by default; `--yes` adds the accepted `(English → conlang)`
pairs to the translation memory.

### Measuring quality (LANG-3 P2)

A conlang has no human reference translations, so quality is gauged without one:

```
inkhaven language eval Eldar                  # coverage + round-trip similarity
inkhaven language eval Eldar --test-set t.txt # against a custom test set
```

**Round-trip** translates each English sentence, reverses the result back to
English, and measures how close the recovered meaning is to the source (embedding
cosine) — a healthy language scores well above 0.7. **Coverage** is the fraction
the lexicon can fully translate (the same maturity signal as `corpus`). The eval
measures the rule-based engine (not the memory, which would echo confirmed pairs);
scriptable as `lang.eval`.

### Shipping a translation pack (LANG-3 P3)

Because the "model" is the translation memory, the whole translation system
exports as one small, portable file you can publish alongside a book:

```
inkhaven language export-translation Eldar              # → eldar-translation.itm
inkhaven language export-translation Eldar -o qya.itm
```

The `.itm` bundle is a zip of the **translation memory** (`memory.tsv` — the
confirmed `English → conlang` pairs), the **lexicon** (`lexicon.tsv`), a
`manifest.hjson`, and a `README.md`. It is browsable as a phrasebook with no
tools, and re-seedable into another project by replaying the pairs through
`language remember`. Embeddings are left out (large, regenerable). Scriptable as
`lang.export` (`fs_write`).

Scope of Tier 1: declarative clauses with an optional adjective per noun phrase.
Subordinate clauses, multi-word phrases beyond one adjective, and the fluency of
a trained neural model arrive with the later LANG-3 tiers (the rule-based parser
hands off to a neural POS+dependency parser behind the same interface). Scriptable
from Bund as `lang.translate` (`{ surface, gloss, literal, confidence, unresolved }`),
`lang.reverse` (`{ english, gloss, confidence, unresolved }`), and `lang.cross`
(`{ english, target, gloss, confidence, unresolved }`) — all `store_read`.

## Scripting (Bund)

The whole ConLang Suite is reachable from **Bund**, so you can *define and
generate a language from a script* as an alternative to hand-authoring HJSON
blocks — the two are equivalent, and you can mix them freely. The `ink.lang.*`
words run against the active project (`inkhaven bund "<script>" --project .` or
from a Script-book paragraph in the TUI).

Every word is also reachable as `lang.X` (the `ink.` prefix dropped), so the
examples below use the short form.

**Read-only inspectors** (the `store_read` category, allowed by default):

```
lang.list             ( -- names )                  lang.gaps         ( lang scope -- report )
lang.generate_word    ( lang role seed -- word )    lang.audit        ( lang -- report )
lang.syllabify        ( lang word -- list )         lang.query        ( lang text -- entries )
lang.ipa              ( lang word -- surface )      lang.stats        ( lang -- profile )
lang.stress           ( lang word -- marked )       lang.paradigm     ( lang root template gloss -- rows )
lang.tone             ( lang tones -- result )      lang.derive       ( lang root gloss pos -- forms )
lang.transliterate    ( lang text -- script )       lang.agree        ( lang word pos features -- form )
lang.gloss            ( lang text -- gloss )        lang.sound_change ( lang form -- evolved )
lang.sentence         ( lang subj verb obj -- clause )   lang.cognates ( proto form -- reflexes )
lang.translate        ( lang text -- {surface,gloss,literal,confidence,unresolved,alternatives} )  English → conlang (RBMT + memory)
lang.reverse          ( lang surface -- {english,gloss,confidence,unresolved} )       conlang → English
lang.cross            ( from to surface -- {english,target,gloss,confidence,unresolved} )  conlang → conlang via English
lang.memory           ( lang -- list-of-{english,conlang} )    the translation-memory pairs
lang.corpus           ( lang -- {scanned,accepted,acceptance_rate,top_missing,pairs} )  RBMT over the bundled English pool (advisory)
lang.eval             ( lang -- {sentences,covered,coverage,round_trip_similarity} )  translation-quality eval
lang.relative         ( lang head role verb with relativizer -- clause )
lang.complement       ( lang subj verb comp comp-subj comp-verb comp-obj -- clause )
lang.coordinate       ( lang clause-list conjunction -- clause )
lang.family_tree      ( -- tree )                   lang.names        ( lang count seed -- list )
lang.prose            ( lang count seed -- clauses )    lang.poem      ( lang meter seed -- lines )
```

Structured results come back as native Bund dicts/lists (e.g. `lang.sentence`
yields `{ surface, gloss, literal }`; `lang.stats` the full analysis profile),
so a script can pull fields directly.

**Mutators** (the `store_write` category — default-denied; enable with
`scripting: { enabled_categories: ["store_write"] }` in `inkhaven.hjson`, the
same gate as `ink.tree.*`):

```
lang.init        ( name -- )                    lang.grammar_set ( lang feature value -- )
lang.define      ( lang chapter block -- )      lang.idiom_add   ( lang form literal meaning -- )
lang.add_word    ( lang word pos translation -- )   lang.metaphor_add ( lang source target -- )
lang.remove_word ( lang word -- )               lang.derive_add  ( lang root gloss pos -- count )
lang.remember    ( lang english conlang -- )    remember a translation (correction loop)
```

A retrieval-first translation pipeline in a few words: `lang.corpus` to seed (loop
its `pairs` through `lang.remember`), then `lang.translate` returns the memorized
form (with the rule-based reading in `alternatives`); a `lang.remember` correction
takes effect on the very next `lang.translate`.

**AI-backed words** (the `ai_write` category — default-denied; enable
`"ai_write"`). They call the LLM and are **advisory** — they *return* data and
never write the book, so a script commits anything it likes via `add_word`. A
trailing `provider` string picks a non-default provider (empty = the configured
default):

```
lang.compose          ( lang kind provider -- text )           kind = blessing|curse|incantation
lang.reconstruct      ( forms gloss provider -- text )         a proto-form from cognate forms
lang.realism_check    ( lang provider -- text )                plausibility of the sound-change chain
lang.generate_lexicon ( lang topic count provider -- words )   themed words (forms obey phonotactics,
                                                                 AI assigns meaning, dedup-gated)
```

`lang.generate_lexicon` returns survivor `{ word, gloss, pos }` dicts; loop them
through `lang.add_word` to keep the ones you want — forms still come from the
deterministic generator, the AI only assigns meaning.

**Building artefacts natively.** `lang.dict` turns a flat Bund list
`[ key val key val … ]` into a real dict (Bund's `{ … }` is a lambda, not a
dict). It is aliased to self-documenting names — `word`, `rule`, `phoneme`,
`block` — so a script reads like what it builds, e.g.
`[ "ipa" "k" "kind" "consonant" ] phoneme`. Hand a dict (or list of dicts, built
with Bund's `push`) to `lang.define` and it is serialized to the same HJSON the
book stores. For whole blocks the HJSON-string form below is usually simplest.

`lang.define` writes a definition `block` as a paragraph under a chapter
(`Phonology` / `Grammar` / `Sample texts` / `Meta`) — exactly the HJSON the book
stores, so a Bund-built language is byte-for-byte a hand-authored one. The block
is a JSON/HJSON string (write `\"` for each quote); the book ends up with clean
HJSON. A whole language, end to end:

```
"Avesha" ink.lang.init
"Avesha" "Phonology" "{ phonemes:[{ipa:\"k\",kind:\"consonant\"}{ipa:\"a\",kind:\"vowel\"}]
   classes:{C:[\"k\"] V:[\"a\"]} templates:{root:[{pattern:\"C V C V\"}]} }" ink.lang.define
"Avesha" "Grammar" "{ grammar:{word_order:\"sov\",alignment:\"nominative_accusative\"} }" ink.lang.define
"Avesha" "kira" "noun" "bird" ink.lang.add_word
"Avesha" "nami" "verb" "see"  ink.lang.add_word
"Avesha" "pata" "noun" "stone" ink.lang.add_word
"Avesha" "kira:bird" "nami:see" "pata:stone" ink.lang.sentence println
```

The same language then opens, inspects, and renders identically through the
`inkhaven language …` CLI and the editor.

## Command index

Every `inkhaven language` subcommand, one line each, generated from the CLI's own help by `inkhaven language _index-markdown` (regenerate it with the README at release time; the `docs_language_surface` test fails when this file names a command or flag that does not exist). `--json` marks a command whose result also comes as JSON; `-o` marks one that writes a file.

### Set-up and lexicon

- `init` — Scaffold a new language sub-book under the top-level `Language` system book
- `list` — List every defined language with summary counts (dictionary entries, grammar / phonology rules, sample texts) · `--json`
- `doctor` — Health report for a language sub-book · `--json`
- `scaffold` — Scaffold a starter phoneme inventory from a short description (AI), as pasteable Phonology-chapter HJSON · `-o`
- `suggest-phonemes` — Recommend phonemes that would round out the inventory (the voiced counterpart missing beside a voiceless obstruent, a near-universal segment the inventory lacks), via the feature matrix · `--json`
- `define-rule` — Define-or-edit a grammar or phonology rule in `$EDITOR`
- `grammar` — The grammar questionnaire · `--json`
- `add-word` — Add a dictionary entry to a language's `Dictionary` chapter
- `remove-word` — Remove a dictionary entry from a language
- `query` — Query a language's dictionary by the rich entry fields: register, semantic domain, in-world era, part of speech, and a free substring over headword + gloss · `--json`
- `sort-dictionary` — Put a language's Dictionary into the declared alphabet's order
- `import` — Import a dictionary from another conlang/linguistics tool
- `export` — Export a language's content to a portable artefact · `-o`
- `generate-lexicon` — AI-assisted dictionary generation
- `derive-lexicon` — Derive a daughter language's lexicon from its proto by applying the daughter's diachronic sound-change chain to every proto dictionary entry
- `derive` — Propose derived lexemes for a root: apply the language's derivational rules (agent nouns, diminutives, …) to the root, with allophony, and print the new word + sense + POS for each rule that fires
- `gaps` — Semantic-gap finder · `--json`
- `audit` — Audit a language's dictionary for consistency: headwords that break the phonotactics, homophones (entries sharing a surface form), and duplicate meanings (accidental synonyms) · `--json`
- `stats` — A descriptive profile of a language: inventory balance (consonants / vowels), phoneme frequency across the lexicon, the syllable-length distribution, which onsets/codas get used, and the part-of-speech spread · `--json`
- `scan-manuscript` — Scan the manuscript for candidate **undefined** conlang words: words that look like the language (segment fully into its inventory + pass its phonotactics) but aren't in the dictionary · `--json`

### Phonology

- `generate-word` — Generate candidate words from a language's phonotactic templates · `--json`
- `syllabify` — Syllabify a word against a language's phonology · `--json`
- `ipa` — Derive the surface pronunciation of a word by applying the language's allophony rules to its underlying form · `--json`
- `stress` — Place primary stress on a word per the language's stress rule · `--json`
- `romanize` — Convert between IPA and a named romanization scheme · `--json`
- `tone` — Apply the language's tone-sandhi rules to a sequence of per-syllable tones · `--json`
- `scan` — Scan a line (or several) of verse: syllabify each word, mark it stressed or unstressed, and name the metre (iambic tetrameter, trochaic trimeter, …) · `--json`
- `trace` — The Consequence Tracer · `--json`

### Morphology and syntax

- `paradigm` — Generate the full paradigm of a root: apply a paradigm template's morpheme sequence (from the `Morphology` chapter) to the root, run the language's allophony across the affix boundaries, and print the surface form + Leipzig gloss for every cell · `--json`
- `agree` — Apply **agreement** (concord): inflect a dependent word (an adjective, a verb) to agree with the grammatical features of its head (its noun, its subject)
- `sentence` — Assemble a **sentence** from a subject, verb, and object · `--json`
- `relative` — Build a noun phrase with a **relative clause** ("the bird that sees the stone"), obeying the `relative_clause` typology (prenominal vs postnominal)
- `complement` — Build a sentence with a **complement clause**: a whole clause serving as the object of a matrix verb of speech or cognition ("I know that the bird sees the stone")
- `coordinate` — **coordinate** noun phrases or clauses with a conjunction ("the bird and the stone", "the bird sees and the river falls")
- `gloss` — Interlinear auto-gloss of conlang text · `--json`
- `igt` — Interlinear glossed text: auto-gloss a sentence and lay it out as an aligned Leipzig block (the morpheme-segmented sentence, the gloss, a literal translation) · `--json`
- `parse` — The morphological parser: analyse a surface word into root + affixes by reversing the morphology (strip known affixes until what remains is a dictionary root) · `--json`
- `tree` — Build the X-bar phrase-structure tree (CP → TP → VP) of a clause from its subject / verb / object(s), placing heads and complements by the language's word order · `--json`
- `movement` — Syntactic movement: front a constituent (wh-movement / topicalisation) over the X-bar tree, leaving a coindexed trace · `--json`
- `binding` — Binding theory: decide whether one argument may refer to another, by c-command and the binding principles (a reflexive object may bind to the subject, a pronoun may not, a name is a Principle-C violation) · `--json`
- `link` — Argument linking: work out a clause's thematic roles, RRG macroroles (actor / undergoer) and grammatical relations from the verb's valence · `--json`
- `check` — The Oracle: judge a candidate word for well-formedness by linguistic level — phonotactics (unknown segments, constraint violations) and morphology (does it analyse as root + affixes?) · `--json`
- `check-clause` — The Oracle over a whole clause (levels 3–4): subject–verb agreement and argument structure · `--json`
- `check-agreement` — The Oracle's agreement check over any head–dependent pair: does a dependent word (an adjective, a determiner, a verb) correctly inflect for its head's features, under the declared agreement rule? · `--json`

### Analysis

- `metrics` — Deterministic quantitative metrics for a language: phoneme-distribution entropy, the Zipf fit of that distribution, phonotactic saturation, and mora weight · `--json`
- `universals` — Judge the language's grammar block against the typological baseline: head-directionality harmony, the classic implicational universals (Greenberg 2/3/4, the OV↔GenN / OV↔RelN correlations), and a word-order + morphotype survey · `--json`
- `grammar-check` — Validate the typed grammar blocks (`ug_parameters`, `verb_classes`) in the Grammar chapter and check them for consistency against the WALS feature answers (e.g · `--json`
- `pairs` — Find minimal pairs in the lexicon and report the distinctive feature each turns on (the functional load of the language's contrasts), via the distinctive-feature matrix · `--json`
- `naturalness` — Judge the phoneme inventory against cross-linguistic tendencies (voicing-pair symmetry, place-series coverage, near-universal segments, size), via the distinctive-feature matrix · `--json`
- `harmony` — Detect vowel harmony (backness, rounding) by measuring how consistently the vowels within a word agree, via the feature matrix · `--json`
- `distribution` — Where each phoneme appears (syllable onset / nucleus / coda and the word edges), and any restricted (defective) distributions — a consonant confined to codas, or barred from a word edge · `--json`
- `sketch` — A one-page grammar sketch: a deterministic prose overview of the language, assembling the phonology, typology and lexicon analyses · `-o`

### Hypotheses

- `hypothesize` — Record a diachronic/comparative hypothesis (a sound change, a cognacy claim, a borrowing) in the language's `Hypotheses` chapter, starting at `proposed` · `--json`
- `hypotheses` — List the language's hypothesis register, optionally filtered by status · `--json`
- `hypothesis` — Show one stored hypothesis in full · `--json`
- `hypothesis-status` — Move a hypothesis along as the evidence comes in (proposed → supported / refuted / retired)
- `hypothesis-check` — Run the Consequence Tracer over a sound-change hypothesis's claim: which words the change predicts, and which merge into homophones · `--json`

### Corpus and texts

- `texts` — The language's stored interlinear texts · `--json`
- `frequency` — A word-frequency list plus descriptive statistics over the language's stored interlinear texts (type–token ratio, Zipf fit) · `--json`
- `concordance` — A KWIC concordance of a word across the stored texts: every occurrence with its context · `--json`
- `collocations` — The collocates of a word: the neighbours that fall within its window across the stored texts, ranked by co-occurrence and scored by PMI (a distinctive collocate outranks a merely-frequent word) · `--json`
- `corpus` — Generate a **synthetic corpus**: translate an English source pool with the RBMT and keep the sentences the language fully covers, seeding the translation memory (Amendment A1) · `--json`
- `eval` — **evaluate** translation quality without a human reference (Amendment A1 / RFC §8.8): *round-trip* semantic similarity (translate then reverse, compared to the source by embedding cosine) and *coverage* (the fraction of the test set the lexicon fully translates) · `--json`

### Translation

- `translate` — **translate** English into the conlang · `--json`
- `reverse` — **reverse** a conlang sentence back into English (Tier 1 RBMT) · `--json`
- `cross` — **cross-translate** one conlang into another by pivoting through English (reverse the source, then translate into the target) · `--json`
- `remember` — **remember** a confirmed English→conlang translation (the correction loop, Amendment A1)
- `memory` — List a language's **translation memory** (the remembered English→conlang pairs `translate` draws on) · `--json`
- `export-translation` — **export** the language's translation system as a portable `.itm` bundle (Amendment A1 / RFC §8.9): a single zip of the translation memory + lexicon + manifest + README, to ship alongside a published work · `-o`

### Diachronics

- `family-tree` — Print the language-family tree (each language under its declared `proto`) · `--json`
- `cognates` — The cognate set of a proto-form: its reflex in every daughter language (each daughter's sound-change chain applied to the proto-form) · `--json`
- `sound-change` — Apply a language's diachronic sound-change chain to a proto-form and print the resulting daughter form · `--json`
- `reconstruct` — AI comparative reconstruction: given cognate forms from daughter languages, propose the most plausible proto-form (with sound correspondences + reasoning)
- `realism-check` — AI genealogical-realism check: assess whether a language's diachronic sound-change chain is typologically plausible (attested) or unnatural, rule by rule

### Varieties and contact

- `varieties` — List the **varieties** (dialects / registers / sociolects) declared for a language, with their axis, prestige, and the size of their sound-change + word-override deltas · `--json`
- `lect` — Render a form *in a variety* (`lect`)
- `dialects` — A **dialect-comparison** table: the first `--count` dictionary headwords rendered across the base form and every declared variety (the classic dialectology display) · `--json`
- `propose-dialect` — (AI) **propose a dialect/register**: the model suggests a coherent set of sound changes + a few lexical swaps for the requested flavour; the deterministic engine validates them (so the variety is always phonologically legal) and previews the result
- `borrow` — **borrow** a word into a language: nativise a donor form to the recipient's inventory + phonotactics (perceive → repair via the `loan_phonology` block)
- `propose-loans` — (AI) **propose realistic loanwords**: which concepts a language would borrow from a donor in a topic domain, each with a plausible donor form, then nativised by the deterministic adapter (P2)
- `areal` — Show **areal** (Sprachbund) convergence
- `areal-check` — (AI) assess whether a language's declared **areal** (Sprachbund) features are typologically plausible — the contact analogue of `realism-check`
- `idiolect` — Render a form / text in a **character's idiolect** — their native variety of their primary language (from the links sidecar)
- `link-place` — Link a Place to a language it's spoken in
- `link-character` — Declare a Character's proficiency in a language (native / fluent / conversational / broken / reading_only)
- `speakers` — List the Places and Characters linked to a language
- `ecology` — The language **ecology**: who speaks what (and which variety) where · `-o`

### Creative text and idiom

- `compose` — Creative text generators
- `idioms` — List a language's idioms and declared metaphors
- `idiom-add` — Add an idiom (a phrase with a literal word-by-word gloss and a separate idiomatic meaning)
- `metaphor-add` — Declare a conceptual metaphor (a source→target domain mapping, e.g

### Books

- `dictionary` — Render the dictionary as a document · `-o`
- `grammar-book` — Render a reference grammar as a document · `-o`
- `tutorial` — Generate a learner's textbook with the AI: a complete graded course (pronunciation guide, vocabulary, grammar lessons with worked examples, a reading, and exercises) authored by the model from the language's own data · `-o`

### Writing systems and fonts

- `font-config` — Show a language's `font` config: family, units-per-em, and every glyph binding with its codepoint, phoneme, and artwork status · `--json`
- `font-templates` — List the spatial templates available to a language: built-in arrangements (`lr`/`tb`/`quad`/`stack3`) plus any custom `templates` defined in its `font` block
- `font-import-glyph` — Import a glyph SVG into a language's writing system: preflight it, copy it into the project glyph store, and bind it to a phoneme and/or Unicode codepoint in the language's `font` config block
- `font-build` — Compile a directory of glyph SVGs into a UFO font source · `-o`
- `font-compose` — Compose component glyphs into a precomposed block (a Hangul-style syllable square, an Egyptian quadrat) per a spatial template, baking them into one glyph · `-o`
- `transliterate` — Input method: transliterate romanized/phonemic text into the script's codepoints using the `font` block's glyph→phoneme bindings (longest key wins, so digraph keys like `th`/`ka` beat their prefixes) · `--json`
- `spatial-typst` — Binding-time B: emit a Typst quadrat that arranges component glyphs spatially at layout time (the hieroglyphic path — no precomposed font glyph) · `-o`
- `glyph-draft` — AI text-to-SVG glyph draft: describe a glyph and the model drafts an SVG, which is run through the suitability preflight · `-o`
- `glyph-lint` — Check whether a glyph SVG is suitable for font compilation: does it parse, does it have a fillable outline (not stroke-only / empty), is it free of raster images, is it monochrome

## Principles

- **Forms obey the language; meanings come from the AI; nothing duplicates.**
- AI features are **advisory**: proposal-gated, `--yes`-committed, glosses in
  the project working language.
- **Deterministic everywhere it can be** — generation, validation, allophony,
  stress, romanization, tone, paradigms, and the dedup gate are pure functions;
  the AI calls are thin layers over them.
