# Language book — design proposal

Status: research / pre-implementation (drafted during 1.2.13).
Owner: vulogov.
Target release: 1.2.13 if the cycle has space; otherwise 1.2.14.

## Problem

Worldbuilders inventing fictional languages (Tolkien
with Quenya / Sindarin; Sanderson with Alethi;
Martin with Dothraki) have rich linguistic state to
manage — dictionary, grammar, phonology, sample
texts — and inkhaven has no dedicated surface for
it.  The current workarounds:

  * Stuff dictionary entries into the Notes book —
    loses structure (no headword / POS / translation
    slots); makes alphabetical browsing painful.
  * Put grammar in plain typst paragraphs — fine
    for reading; impossible for an LLM to consume
    as translation context.
  * Hand-roll a separate document outside inkhaven
    — loses the integration with the manuscript
    (no overlay highlighting; no AI translation
    flow).

The Language book fills this gap.  It pairs with
the 1.2.11 multilingual-prompts plumbing to make
**AI-assisted text-to-text translation** between
the author's working language and the invented
language a first-class workflow.

Natural-language authors benefit too — a French
author writing in English who maintains a French-
dialogue vocabulary book gets the same machinery
with `language_kind: "natural"`.

## Requirements

From the design brief, verbatim:

* **HJSON content-type** for entries with a
  structured template the renderer visualises as
  a dictionary entry.  Core fields: `word`, `type`
  (POS), `translation`, `example`.
* **Two-column dictionary export** in standard
  dictionary format.
* **Grammar chapter** structured to be consumed
  by an LLM-driven text-to-text translation
  flow.  Nice visualisation.
* **Hierarchy shape**: `Language` (top-level
  book) → `<Language name>` (sub-book) →
  `Dictionary` chapter + `Grammar` chapter.
  Dictionary has subchapters by alphabet.

Plus the suite of additions §11 surfaces.

## Design

### 1. Hierarchy shape

```
Language                                        # SYSTEM_TAG_LANGUAGES (new)
├── Quenya                                       # Book; one per invented language
│   ├── Meta                                     # Chapter (auto-created)
│   │   └── overview.typ                         # Language metadata (HJSON)
│   ├── Dictionary                               # Chapter (auto-created)
│   │   ├── A                                    # Subchapter (alphabet section)
│   │   │   ├── aiya                             # Paragraph (HJSON entry)
│   │   │   ├── ainur                            # Paragraph
│   │   │   └── ...
│   │   ├── B
│   │   │   └── ...
│   │   └── ...
│   ├── Grammar                                  # Chapter (auto-created)
│   │   ├── noun-cases                           # Paragraph (HJSON rule)
│   │   ├── verb-conjugation                     # Paragraph
│   │   └── ...
│   ├── Phonology                                # Chapter (auto-created, §5)
│   │   ├── consonants                           # Paragraph (HJSON)
│   │   └── ...
│   └── Sample texts                             # Chapter (auto-created, §6)
│       └── ...
└── Drow                                         # Book; second language
    └── ...
```

The five chapters (`Meta`, `Dictionary`, `Grammar`,
`Phonology`, `Sample texts`) are auto-created when
a new language book is scaffolded.  Authors can
leave any of them empty.

Alphabet subchapters auto-route on entry-add: when
the user adds the word `aiya`, the picker offers
the existing `A` subchapter (or creates it if
absent).  Cross-alphabet languages (Russian → А-Я,
Greek → Α-Ω, English → A-Z, etc.) — the subchapter
list comes from the language's metadata
(`alphabet: ["А", "Б", "В", …]`).

### 2. Language metadata — `Meta/overview`

One paragraph per language book, HJSON-typed,
carries the per-language configuration:

```hjson
{
  name: Quenya
  language_kind: constructed     # "constructed" | "natural"
  family: Elvish                 # optional; relates to sibling Sindarin
  iso_code:                      # optional ISO 639-3 if registered
  alphabet: ["A", "B", "C", "D", "E", "F", "G", "H", "I", "L",
             "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V",
             "Y"]
  reading_direction: ltr
  example_corpus_ref: tolkien-letters
}
---
# Free-form notes
Quenya is the High-elven language of Aman, codified by Tolkien...
```

The `alphabet` field drives the Dictionary's
subchapter list — auto-create on first entry
under that letter.

### 3. Dictionary entry — HJSON schema

Core fields (per the brief) plus the obvious
extensions:

```hjson
{
  # Required
  word:         aiya
  type:         interjection     # noun | verb | adjective | adverb |
                                  # pronoun | preposition | conjunction |
                                  # interjection | particle
  translation:  hail
  example:      Aiya Eärendil!   # canonical sample sentence

  # Optional but encouraged
  examples:     []                # additional sample sentences
  pronunciation: /ˈaɪ.ja/         # IPA
  etymology:    "from PE *ai- (bright)"  # plain text or [[wikilink]] to other entries
  related:      [aire, ai]        # cross-references to sibling entries
  inflection:   {                  # paradigm table (optional)
    accusative: aiyan
    dative:     aiyas
    genitive:   aiyo
  }
  notes:        ""                 # usage notes, register, era
  frequency:    0                  # auto-tracked count of mentions in manuscript
}
---
# Free-form prose continues below the frontmatter
Aiya is the formal greeting used in High Elven contexts.
Less formal: aire (cf. entry).
```

The renderer (§7) reads the frontmatter and paints
a dictionary card; the prose below the `---` is
rendered as the entry's usage notes.

### 4. Grammar rule — HJSON schema for AI consumption

Grammar paragraphs are structured to be consumed
by the LLM translation flow.  Each rule carries
*executable* context — the LLM reads the rule and
applies it.

```hjson
{
  # Identification
  rule_id:      noun-case-system
  title:        "Noun cases"
  category:     morphology         # phonology | morphology | syntax |
                                    # orthography | semantics | pragmatics

  # Machine-readable rule body
  rule:         """
    Quenya nouns inflect for 10 cases.  Case is marked by a suffix on the
    noun stem; the stem is the citation form minus any final vowel.

    NOM: zero suffix.        aran    (king)
    ACC: -n.                  aran   → aranin   (king-acc)
    DAT: -en.                 aran   → aranen   (king-dat)
    GEN: -o.                  aran   → arano    (king-gen)
    ABL: -llo.                aran   → aranello (king-abl)
    ALL: -nna.                aran   → arannannna (king-all)
    LOC: -sse.                aran   → aranesse (king-loc)
    INST: -nen.               aran   → aranen   (king-inst)
    RESP: -s.                 aran   → aranes   (king-resp)
    POSS: -va.                aran   → aranva   (king-poss)
  """

  # Examples bundled into the prompt envelope at translation time
  examples: [
    { source: "the king",          target: "aran",        gloss: "king.NOM" }
    { source: "to the king",       target: "aranen",      gloss: "king.DAT" }
    { source: "from the king",     target: "aranello",    gloss: "king.ABL" }
  ]

  # Dependencies — other rules this one assumes
  depends_on:   ["phonology.vowel-harmony", "morphology.stem-formation"]

  # When this rule applies (LLM uses this to decide whether to include it
  # in the translation prompt)
  applies_when: "the target sentence contains a noun in a non-nominative role"
}
---
# Prose exposition continues here for the human reader
The case system was formalised in Tolkien's Etymologies (1937)...
```

The rule body is plain text inside the HJSON
string — the LLM reads it the same way the human
reader does.  `examples` becomes few-shot data
for the translation prompt.  `depends_on` lets
the RAG layer pull in dependent rules
automatically.

### 5. Phonology chapter (added)

Separate from Grammar so the sound rules don't
inflate every translation prompt.  Useful for:

  * Generating phonetically-consistent neologisms
    (`Ctrl+B Shift+W` — see §8).
  * Validating that a candidate translation
    respects the language's phonotactics.
  * Pronunciation guidance for the dictionary's
    IPA field.

Same HJSON shape as grammar rules.  Categories:

  * `consonants` — IPA chart with allowed
    phonemes.
  * `vowels` — vowel inventory + allowed
    sequences.
  * `phonotactics` — allowed onset / coda
    structures.
  * `stress` — stress placement rule.
  * `sound-changes` — historical / inflectional
    shifts (e.g. "intervocalic /s/ voices to /z/
    in formal register").

### 6. Sample-text chapter (added)

Short stories or sentences in the language with
glossed English (or other working-language)
translations.  Two purposes:

  * **Few-shot examples** for the AI translation
    flow.  When the author asks the LLM to
    translate English → Quenya, the prompt
    envelope can include a handful of sample
    texts as worked examples — gives the model
    style + register guidance the dictionary
    alone can't.
  * **Author reference** — re-reading sample
    texts is the fastest way to recover the
    feel of a language between writing sessions.

Each sample is a paragraph with this shape:

```
= Aiya Eärendil! Title is the source-language phrase

#table(
  columns: (auto, auto, auto),
  "Quenya",      "Gloss",            "English",
  "Aiya",        "hello-INTJ",       "Hail",
  "Eärendil",    "Earendil-NOM",     "Earendil",
  "Elenion",     "star-PL-GEN",      "of the stars",
  "Ancalima",    "brightest-NOM",    "brightest",
)
```

(Typst-rendered table; HJSON frontmatter optional
for source attribution.)

### 7. Visualisation — the dictionary card renderer

When the cursor is on a Dictionary-chapter paragraph
in the tree, the editor pane renders the HJSON
frontmatter as a structured card:

```
┌─ aiya ──────────────────────── interjection ──┐
│                                                │
│  hail                                          │
│                                                │
│  /ˈaɪ.ja/                                      │
│                                                │
│  Etymology · from PE *ai- (bright)             │
│  Related   · aire, ai                          │
│                                                │
│  Example                                       │
│     Aiya Eärendil!                             │
│     "Hail Earendil!"                           │
│                                                │
│  Usage                                         │
│     Aiya is the formal greeting used in        │
│     High Elven contexts.  Less formal: aire    │
│     (cf. entry).                               │
│                                                │
│  · 24 mentions in manuscript                   │
└────────────────────────────────────────────────┘
```

Headword bold, POS italic at the title bar;
translation in the next prominent slot;
pronunciation dimmed; etymology / related dim
metadata; example block; usage prose at the
bottom; manuscript-frequency footer.  Same
renderer powers the inline-hover popover when
the cursor is over a highlighted invented-language
word in the manuscript.

Grammar paragraphs render with a similar card
shape: title at the top; category chip; rule body
in a monospaced block; examples as a 3-column
table (source / gloss / target); depends-on links.

### 8. Lexicon overlay + status-bar preview

Same machinery as Places/Characters/Notes/Artefacts:

  * The build-time lexicon walk
    (`src/tui/lexicon_build.rs::build_lexicon`)
    gains `Some(SYSTEM_TAG_LANGUAGES) => languages
    = Some(node.id)` and pulls every Dictionary-
    entry headword into a `LexCategory::Language`
    set.  Per-language colour comes from a new
    `tree_language_fg` theme slot (one colour per
    language book — auto-generated from the
    `Language/<name>` book's index in the
    hierarchy, or user-overridden in HJSON).
  * The render-time overlay (`src/tui/highlight.rs`)
    lights up matching words in the manuscript
    with the language's colour.  Snowball-stemmed
    where the invented language's
    `Meta/overview.stemmer` (optional field) names
    a recognised algorithm; exact-form otherwise.
  * When the cursor lands on a highlighted
    invented word, the status bar shows
    `[word · POS · translation]` (truncated to fit;
    `Ctrl+I` opens the full card).

### 9. AI text-to-text translation flow

The headline integration with 1.2.11's multilingual
prompts.  Two new chords:

  * **`Ctrl+B Q`** — translate the cursor selection
    (or current paragraph if no selection) from the
    author's working language INTO an invented
    language.  Picks which language via a one-press
    sub-chord (`Ctrl+B Q Q` for Quenya, `Ctrl+B Q
    D` for Drow, etc. — the letter is the language
    book's first letter; conflicts disambiguated by
    a quick picker).  Output renders as a side-by-
    side gloss in the AI pane.
  * **`Ctrl+B Shift+Q`** — translate FROM the
    invented language back to the working
    language.  Useful for roundtrip testing (does
    the LLM understand its own grammar?).

The translation prompt envelope is composed at
fire-time:

  1. **System prompt** — explains the LLM's role
     as a translator between the working language
     and the target invented language; references
     the language's grammar rules + dictionary
     entries as authoritative.
  2. **Grammar context** — every rule in the
     target language's Grammar chapter whose
     `applies_when` matches the source sentence
     (RAG-style relevance).  Default cap: 6 rules.
  3. **Dictionary context** — every Dictionary
     entry for words in the source sentence,
     plus the `related` and `depends_on` closure
     of those entries.
  4. **Sample-text context** — 2-3 Sample-text
     entries closest in register to the source.
  5. **User prompt** — the source sentence + the
     direction (→ invented / → working).

The composed prompt typically lands around 4-8 K
tokens (well within any modern model's window).
The translation response includes:

  * The translated text.
  * A per-token gloss table.
  * Notes on which grammar rules fired.
  * Confidence flags ("dictionary missing for
    'forest'; please add").

The user can `Ctrl+B G` (insert response) the
translation into the editor at the cursor.

### 10. Visualisation — the Grammar / Phonology card

Same HJSON-card pattern as the dictionary card.
A Grammar rule renders as:

```
┌─ Noun cases ────────────────── morphology ────┐
│                                                │
│  Quenya nouns inflect for 10 cases.            │
│  Case is marked by a suffix on the noun       │
│  stem; the stem is the citation form minus     │
│  any final vowel.                              │
│                                                │
│  NOM: zero suffix.        aran    (king)       │
│  ACC: -n.                  aran   → aranin     │
│  DAT: -en.                 aran   → aranen     │
│  GEN: -o.                  aran   → arano      │
│  ABL: -llo.                aran   → aranello   │
│  ...                                           │
│                                                │
│  Examples                                      │
│   ┌──────────────┬──────────────┬───────────┐ │
│   │  Source      │  Gloss       │  Target   │ │
│   ├──────────────┼──────────────┼───────────┤ │
│   │  the king    │  king.NOM    │  aran     │ │
│   │  to the king │  king.DAT    │  aranen   │ │
│   └──────────────┴──────────────┴───────────┘ │
│                                                │
│  Depends on                                    │
│   · phonology.vowel-harmony                    │
│   · morphology.stem-formation                  │
│                                                │
│  Applies when                                  │
│   the target sentence contains a noun in a    │
│   non-nominative role                          │
│                                                │
│  · prose exposition below                      │
└────────────────────────────────────────────────┘
```

The "Applies when" / "Depends on" blocks render
slightly dim — they're for the LLM's benefit, not
the reader's, but visible so the author can audit
the prompt envelope.

### 11. Additions on top of the brief

The "propose something else" invitation surfaces
these:

  * **§5 Phonology chapter** — separate sound
    rules from grammar so they don't bloat every
    translation prompt.  Detailed above.
  * **§6 Sample-text chapter** — few-shot
    examples + author re-immersion.
  * **Inflection paradigm table** in the
    dictionary entry — for inflected languages,
    one HJSON map covers the full paradigm.
  * **Frequency tracking** — auto-count manuscript
    mentions of each dictionary word so the author
    sees vocabulary spread.  Drives a `inkhaven
    language doctor <name>` health check (see
    §13).
  * **Reverse lookup** — `Ctrl+B Shift+R` in the
    Language book opens a "find by translation"
    picker.  Type `hail` → land on `aiya`.
  * **Cross-language family** — `family` field on
    Meta/overview lets sibling languages
    (Quenya / Sindarin / Old English / Middle
    English) declare their relation.  The render
    layer can show "shared roots" cues in the
    sidebar.
  * **Phonotactic generator** — `Ctrl+B Shift+W`
    in the Language book generates candidate
    words conforming to the language's
    phonotactics.  Uses the Phonology rules + the
    LLM.  Helps the author when they need a new
    noun on demand.
  * **Roundtrip translation test** — `inkhaven
    language test <name>` walks a corpus of
    sample texts through `Ctrl+B Q` → `Ctrl+B
    Shift+Q` and reports drift.  Catches
    grammar-rule inconsistencies before they bite
    in the manuscript.
  * **Project `language` integration** — the
    top-level `language` HJSON field already
    accepts `english / russian / french / german /
    spanish`.  Extend to accept any invented
    language declared in a Language book.  When
    set, multilingual-prompts plumbing routes the
    AI flows through the invented language's
    Grammar + Dictionary.  Edge case: stemmer
    plumbing has no algorithm for invented
    languages (Snowball doesn't know Quenya);
    fall back to exact-form matching when
    `Meta/overview.stemmer` is unset.
  * **Word-of-the-day** — `Ctrl+B Shift+W` *in
    the manuscript editor* (separate from the
    phonotactic generator's chord in the
    Language book — context-dispatched) picks a
    random Dictionary entry and shows its card in
    a floating overlay.  Author re-immersion.
  * **Anki / SuperMemo export** — `inkhaven
    language export-flashcards <name> --format
    {csv,anki}` produces a deck.  Useful for
    authors who want to internalise their own
    vocabulary.

### 12. Export formats

`inkhaven export language <name>` writes a
printable reference document.  Output flag picks
the artefact:

  * **`--format dictionary-twocol`** (default) —
    standard two-column dictionary in typst.
    Alphabet headers between sections; entries
    formatted as: bold headword in left margin,
    POS italic, translation, examples indented.
    A4 / US-Letter selectable.
  * **`--format grammar`** — printable grammar
    reference: TOC + chapter per rule category +
    examples tables + cross-refs.
  * **`--format phrasebook`** — Sample-text
    chapter rendered as a phrasebook, gloss
    on the left, English on the right.
  * **`--format anki`** — CSV deck importable by
    Anki / SuperMemo / Mochi.  Front: invented
    word; back: translation + example + POS.
  * **`--format json`** — full structured form for
    downstream tooling.

Sample two-column dictionary layout:

```
─────────────────────────────  A  ─────────────────────────────
aiya   interjection   hail
       /ˈaɪ.ja/   PE *ai- (bright)
       Aiya Eärendil!  "Hail Earendil!"

ainur  noun (pl)   holy spirits (sg. ainu)
       /ˈaɪ.nur/
       The Ainur sang the world into being.

─────────────────────────────  B  ─────────────────────────────
...
```

### 13. CLI subcommands

`inkhaven language` subcommand groups the
Language-book-specific operations:

  * `inkhaven language init <name>` — scaffold a
    new language book with the five-chapter shell
    + an empty `Meta/overview` HJSON paragraph
    pre-populated with sane defaults.
  * `inkhaven language add-word <language> <word>
    --type <pos> --translation <text> [--example
    <text>]` — add a dictionary entry (auto-
    routes to the right alphabet subchapter).
  * `inkhaven language define-rule <language>
    <rule_id> --category <cat>` — open the rule
    template in `$EDITOR` for hand-editing the
    HJSON frontmatter and prose body.
  * `inkhaven language translate <language>
    <text> [--direction to|from]` — fire the AI
    translation flow from the command line; same
    prompt-envelope composition as the in-editor
    chord; emits the translation + gloss table
    + applied-rules list on stdout.
  * `inkhaven language doctor <language>` —
    health report: dictionary size, average
    example coverage, grammar rule count by
    category, sample-text count, words mentioned
    in the manuscript that are missing from the
    dictionary.
  * `inkhaven language export <language>
    --format <fmt> --output <path>` — see §12.
  * `inkhaven language test <language>` —
    roundtrip translation test described in §11.

### 14. New chord table

| Chord | Action | Effect |
|-------|--------|--------|
| `Ctrl+B Q <letter>` | `view.translate_to_invented` | Translate selection (or open paragraph) into the language whose name starts with `<letter>` (picker on ambiguity). |
| `Ctrl+B Shift+Q` | `view.translate_from_invented` | Translate from invented language back to working language. |
| `Ctrl+B Shift+W` (in Language book) | `view.generate_word` | Generate a candidate word conforming to the language's phonotactics. |
| `Ctrl+B Shift+W` (in manuscript editor) | `view.word_of_the_day` | Show a random Dictionary entry as a floating card. |
| `Ctrl+B Shift+R` (in Language book) | `view.reverse_lookup` | Find dictionary entry by translation. |
| (cursor on highlighted invented word) | — | Status bar shows `[word · POS · translation]`; `Ctrl+I` opens the full card. |

All chords bindable via `keys.bindings` /
`ink.key.bind` per the existing 1.2.5 chord-
customisation plumbing.

## Risks + open questions

  * **Alphabet subchapter explosion** — *resolved*.
    `alphabet` is user-defined; the author picks
    the segmentation that works for their
    orthography (Hebrew / Arabic / Asian scripts
    can use logical groupings rather than per-
    letter sections).
  * **Stemmer gap** — *resolved*.  Snowball ships
    English / Russian / French / German / Spanish
    (and several others) — not Quenya / Drow /
    Klingon.  Exact-form matching is the
    fallback.  For inflection-heavy invented
    languages, the `inflection` paradigm field
    in each dictionary entry lists all paradigm
    forms; the lexicon walker expands entries to
    cover every form, giving the same coverage
    Snowball would provide for natural languages
    without requiring the author to author a
    stemmer.
  * **Prompt-envelope size.**  6 grammar rules
    + N dictionary entries + 3 sample texts +
    the user prompt can blow past 8K on a
    sentence-paragraph with many invented words.
    Cap N at 30 entries (cover the most
    frequent + the ones in the source sentence);
    the LLM can ask for more if it needs them.
  * **AI quality on invented languages.**  GPT-5
    / Claude Opus / Gemini 2.5 are uneven on
    fictional languages.  Mitigation: the
    Grammar rules + dictionary entries are
    explicitly authoritative; the system prompt
    instructs the model to defer to them.  The
    roundtrip test (`§11`) measures drift; if
    drift is high, the author has to either add
    more grammar rules or accept that the model
    is doing creative interpolation.
  * **Per-language colour explosion in the
    theme.**  Each Language book wants its own
    `tree_<name>_fg`.  Solution: auto-assign
    from a palette; users override in HJSON.
  * **Interaction with the 1.2.11 multilingual-
    prompts resolver.**  Setting `language:
    quenya` on the project would route every AI
    flow through Quenya prompts.  The five-
    language embedded floor only ships en/ru/es/
    de/fr; the resolver would land in Pass 3
    (any-language) and use the English floor.
    Acceptable — the Language book provides
    the per-language context the embedded
    prompts can't.

## Implementation phases

**Phase A — foundation.**  No translation flow.

  * `SYSTEM_TAG_LANGUAGES` constant + system-book
    bootstrap.
  * Language-book scaffolder (`inkhaven language
    init <name>`).
  * Dictionary / Grammar / Phonology /
    Sample-text chapters auto-created.
  * HJSON content-type already exists; no
    change needed for the structured-paragraph
    parsing.
  * Tree-pane renderer recognises the new system
    book; per-language colour assignment from a
    palette (auto-derived from book index +
    overridable).

**Phase B — lexicon + card renderers.**

  * Build-time lexicon walk picks up Dictionary
    headwords as `LexCategory::Language`.
  * Render-time overlay lights up invented words
    in the manuscript.
  * Status-bar preview when cursor lands on a
    highlighted invented word.
  * Dictionary / Grammar / Phonology card
    renderer for paragraphs in the Language
    book (the §7 / §10 visualisations).

**Phase C — translation flow.**

  * `Ctrl+B Q` + `Ctrl+B Shift+Q` chord
    registration.
  * Prompt-envelope composer (system + grammar
    RAG + dictionary + sample-text + user
    prompt).
  * Translation response renderer (gloss table +
    applied-rules list).
  * `Ctrl+B G` insertion into the editor at the
    cursor.

**Phase D — export + tooling.**

  * `inkhaven language export <name> --format ...`
    subcommand with five output formats.
  * `inkhaven language doctor <name>` health
    report.
  * `inkhaven language test <name>` roundtrip
    drift test.
  * Reverse-lookup picker + word-of-the-day +
    phonotactic generator (the §11 bonus
    features).

Each phase is its own commit / PR; main stays
green between them.

## Recommendation

Phases A + B are the load-bearing parts: they
unblock everything else and provide value on
their own (structured dictionary + lexicon
overlay).  Phase C is the headline AI-translation
feature; ship it once the foundation has settled.
Phase D is polish that ships as time permits.

Suggest **A + B in 1.2.13** if the cycle has
space; **C + D in 1.2.14**.  Inline-comment
work (the other research question's
recommendation) can fit alongside Phase B since
both involve span-anchored renderer extensions.
