#import "../design.typ": *

#appendix(letter: "B", title: "Configuration block reference")

The HJSON blocks you place into your language's chapters, in one place. Remember
the rule: quote short word-like values (`kind: "consonant"`), and run `inkhaven
reindex --adopt` after editing files by hand.

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

#section("Dictionary chapter")

Each word is one small block (written for you by `add-word`):

```hjson
{ word: "makil", type: "noun", translation: "sword",
  register: "formal", domain: ["weapon"], era: "third_age" }
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
