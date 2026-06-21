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

## Interchange (export)

`inkhaven language export <lang> --format <fmt> [--out F]` writes the lexicon to
a portable artefact:

```
inkhaven language export Avesha --format xliff   > avesha.xlf
inkhaven language export Avesha --format linguex > avesha.tex
inkhaven language export Avesha --format ipa-chart
```

- `json` — full structured dump (dictionary, grammar, phonology, samples).
- `anki` / `csv` — flashcard / round-trippable CSV (the `--import` path re-ingests `csv`).
- `dictionary-twocol` / `grammar` / `phrasebook` — printable Typst (need `--out`).
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
until you pass `--yes`. Duplicate headwords are skipped with a warning. (Tools
that export CSV, such as ConWorkShop, can come in through `add-word --import`.)

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
```

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

## Principles

- **Forms obey the language; meanings come from the AI; nothing duplicates.**
- AI features are **advisory**: proposal-gated, `--yes`-committed, glosses in
  the project working language.
- **Deterministic everywhere it can be** — generation, validation, allophony,
  stress, romanization, tone, paradigms, and the dedup gate are pure functions;
  the AI calls are thin layers over them.
