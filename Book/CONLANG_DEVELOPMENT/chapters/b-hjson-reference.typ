#import "../design.typ": *

#appendix(letter: "B", title: "Configuration block reference")

The HJSON blocks you place into your language's chapters, in one place. Remember
the rule: quote short word-like values (`kind: "consonant"`), and run `inkhaven
reindex --adopt` after editing files by hand.

#section("Meta chapter — the overview block")

The `Meta/overview` paragraph (seeded by `language init`) holds the language's
identity and the defaults the rest of the suite reads. Every key is optional:

```
{
  name: "Eldar"
  family: "Elvish"
  language_kind: "constructed"   // "constructed" | "natural"
  iso_code: "qya"                // optional ISO 639-3 code
  alphabet: ["Aa", "Bb", "Cc"]   // canonical order; drives Dictionary buckets
  reading_direction: "ltr"       // "ltr" | "rtl"
  script: "latin"
  word_order: "SVO"
  morphology: "fusional"
  tonal: false
  has_cases: true
  has_gender: false
  stemmer: "suffix"              // how search strips inflection
  example_corpus_ref: ""
  notes: ""
}
```

A few of these do real work: `language_kind` tightens how strictly the AI
translator adheres to your rules (`constructed`) versus leaning on its pretraining
(`natural`); `alphabet` drives the Dictionary's letter buckets and `add-word`'s
placement; `reading_direction` and `script` shape rendering; `stemmer` governs how
`query` strips inflection when it searches. The rest are descriptive.

#section("Phonology chapter")

The full phonology block. Every field except `phonemes` is optional.

```hjson
{
  phonemes: [
    { ipa: "k", romanize: "k", kind: "consonant" }   // kind: consonant | vowel
    { ipa: "a", romanize: "a", kind: "vowel" }
  ]
  classes: { C: ["k"], V: ["a"] }                     // named phoneme groups
  templates: { root: [ { pattern: "C V (C)", weight: 1.0 } ] }
  constraints: [
    { kind: "max_cluster_size", value: 2 }
    { kind: "no_geminate" }
    { kind: "forbid_in_coda", classes: ["Stop"] }     // or forbid_in_onset
    { kind: "sonority_sequencing" }
  ]
  allophony: [ { name: "palatalization", rule: "k > tʃ / _ i" } ]
  stress: "penultimate"        // initial | final | penultimate | antepenultimate | latin
  romanizations: [
    { name: "default"
      mappings: [ { ipa: "k", roman: "c" } ]
      contextual: [ { roman: "c", ipa: "s", before: "FrontV" } ] }
  ]
  default_romanization: "default"
  tone: { kind: "contour", tones: ["1","2","3"], sandhi: [ { rule: "3 > 2 / _ 3" } ] }
}
```

A separate block, also in the Phonology chapter, declares descent from a proto:

```hjson
{ diachronics: {
    proto: "ProtoEldarin"
    rules: [ { rule: "p > f / _ #" }, { rule: "k > h / V _ V" } ]
} }
```

The `font` block (built for you by `font-import-glyph`) also lives here:

```hjson
font: {
  family: "Eldar"
  upm: 1000
  glyphs: [
    { name: "a", codepoint: "a", phoneme: "a" }
    { name: "sun", codepoint: "U+E000", phoneme: "o" }
  ]
}
```

The `loan_phonology` block — how the language nativises borrowings (Chapter 18) —
also lives in the Phonology chapter:

```hjson
{ loan_phonology: {
  repair: "epenthesis"                    // epenthesis (insert a vowel) | deletion
  epenthetic_vowel: "u"                   // empty → the first declared vowel
  substitutions: { "θ": "t", "r": "l" }   // a donor sound we lack → nearest native
} }
```

#section("Grammar chapter")

The morphology block — affixes, processes, paradigms, derivations, agreement:

```hjson
{
  morphemes: [
    // Concatenative affixes. position: prefix | suffix | infix | circumfix.
    // Optional precedence: 0 = any (keep declared order), 1 = next to the root, …
    // Optional category / value tag the affix for the grammar book.
    { id: "dat", gloss: "DAT", form: "ti", position: "suffix",
      precedence: 1, category: "case",   value: "dative" }
    { id: "pl",  gloss: "PL",  form: "i",  position: "suffix",
      precedence: 2, category: "number", value: "plural" }
    { id: "ag",  gloss: "AG",  form: "um", position: "infix", anchor: "before_first_vowel" }
    { id: "ptcp", gloss: "PTCP", form: "ge_t", position: "circumfix" }   // ge_ + stem + _t
    // Non-concatenative processes.
    { id: "pst", gloss: "PST", process: "ablaut", rules: [ { rule: "i > a" } ] }
    { id: "rdp", gloss: "PL",  process: "reduplication", reduplicate: "initial_cv" }
    // reduplicate: full | initial_cv | initial_syllable | final_syllable
  ]
  paradigms: [ { name: "noun", cells: [
    { features: { number: "sg", case: "nom" }, morphemes: [] }
    { features: { number: "pl", case: "dat" }, morphemes: ["dat","pl"] }
  ] } ]
  derivations: [
    { name: "agent", form: "ron", position: "suffix",
      from_pos: "verb", to_pos: "noun", gloss_template: "one who {}s" }
  ]
  agreement: [
    { dependent: "adjective", head: "noun", features: ["number","case"], paradigm: "adj" }
  ]
}
```

The typology answers (written for you by `grammar --set`) and idioms / metaphors
(written by `idiom-add` / `metaphor-add`) are also stored in this chapter.

The `varieties` block — the language's dialects and registers (Chapter 17) —
lives here too. Each variety is a *delta*: an ordered list of `sound_changes` (SPE
notation, applied synchronously) plus optional suppletive `lexicon` overrides:

```hjson
{ varieties: [
  { id: "lowland", kind: "dialect", axis: "region", prestige: "low",
    sound_changes: [ { rule: "t > d / V _ V" } ]
    lexicon: { "water": "móru" } }              // a suppletive override
  { id: "high", kind: "register", axis: "formality", prestige: "high",
    sound_changes: [ { rule: "k > q / # _" } ] }
] }                                              // kind: dialect | register | sociolect | idiolect
```

The `contact` block — the language's membership in a linguistic area / Sprachbund
(Chapter 18):

```hjson
{ contact: {
  region: "the Inner Sea"
  with: [ "Sindar", "Khuz" ]                                  // neighbours
  areal_features: { word_order: "sov", alignment: "ergative_absolutive" }
} }
```

The `verb_classes` and `ug_parameters` blocks feed the syntax engine (Chapter 30)
and the clause Oracle (Chapter 31). A verb class records a verb's *valence* — how
many arguments it takes — so `link`, `tree`, `check-clause` and the rest read it
rather than guessing from the argument count. The universal-grammar parameters are
typological switches that `grammar-check` validates and cross-checks against your
typology answers:

```hjson
{ verb_classes: [
    { name: "see",  valence: "transitive",  note: "perception" }
    { name: "give", valence: "ditransitive" }
    { name: "rain", valence: "impersonal" }                   // intransitive | transitive | ditransitive | impersonal
  ]
  ug_parameters: {
    head_final:  false                                        // checked against word_order
    pro_drop:    true
    wh_movement: true
  } }
```

#section("Dictionary chapter")

Each word is one small block (written for you by `add-word`). The required fields
are `word`, `type`, and `translation`; the rest are optional and add depth:

```hjson
{ word: "makil", type: "noun", translation: "sword",
  register: "formal", domain: ["weapon"], era: "third_age",
  pronunciation: "ˈma.kil", etymology: "from mak- 'to cut'",
  related: ["makilya", "makta"], inflection: { plural: "makili" },
  examples: ["The king drew his makil."], notes: "ceremonial, not a field blade" }
```

`pronunciation`, `etymology`, `related` (cross-references), `inflection` (a small
map of forms), `examples`, and `notes` are all preserved and shown in the rendered
dictionary and grammar book.

Idioms and conceptual metaphors also live in the Dictionary chapter, written by
`idiom-add` / `metaphor-add`, under an `expressions` block:

```hjson
{ expressions: {
  idioms:    [ { form: "cold hands", literal: "hands of ice", meaning: "a hard bargainer", register: ["colloquial"] } ]
  metaphors: [ { source: "journey", target: "life", examples: ["the long road of his years"], note: "very common" } ]
} }
```

#section("SPE rule notation")

Used by `allophony`, `diachronics`, and tone `sandhi`:

```text
TARGET > RESULT / LEFT _ RIGHT
```

/ `_`: the position of the target sound.
/ `#`: a word boundary (start or end of a word).
/ `∅` or `0`: the empty string — insertion (on the left) or deletion (on the right).
/ a class name (`V`, `C`, `Stop`): matches any sound in that class.
/ a bare phoneme (`i`, `k`): matches just that sound.
