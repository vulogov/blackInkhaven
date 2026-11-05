# LANG-BUGFIX-1 — fix-now defects in the ConLang development suite

| | |
|---|---|
| **Track** | LANG-BUGFIX-1 |
| **Status** | all eleven items landed on 3.17.0-dev |
| **Created** | 2026-11-04 |
| **Author** | Vladimir Ulogov |
| **Target version** | 3.17.0 |
| **Depends on** | LANG-1 (ConLang Development Suite), LANG-2 (Sociolinguistics & Contact), LANG-3 (Translation) |
| **Companion docs** | `Documentation/CONLANG.md`, tutorials 49 / 50 / 74 / 75 |

A three-source audit of the language subsystem (`src/conlang/`, `src/cli/language/`,
the conlang panes in `src/tui/`): a static sweep of the code, a sweep of the
language docs and plans, and a hands-on run of 86 `inkhaven language …`
commands against a scratch project (`Avesha`, 13 phonemes, 3 classes, 4
constraints, 2 allophony rules, penultimate stress, ~20 lexicon entries).
Nothing panicked. Every finding below is a **silent wrong answer, a lockout, or
a data-loss path** that was reproduced on the `3.17.0-dev` binary. Cheap wins
(message wording, `--json` coverage, doc drift) and the larger roadmap items
are out of scope here and tracked separately.

Each fix lands with a regression test. Findings are ranked; the suggested
landing order is at the end.

---

## P1 — lockouts and data loss

### LB-1 · One unknown constraint kind locks out the whole language

**Status: landed (3.17.0-dev).** `PhonotacticConstraint::Unknown(kind)`
replaces the parse error, so the block loads and the one constraint is
inert; `Phonology::validate` reports it as `constraints[N]: unknown
constraint kind …` with the known kinds listed. `language doctor` parses
every Phonology / Grammar / Morphology block (`doctor::block_findings`),
reports parse errors by paragraph title and validation findings by location
under a new *Blocks* section (and `blocks.findings` in `--json`). The CLI
loader prints each finding as a `phonology error:` / `phonology warning:`
line and continues. Malformed HJSON is still a hard error, now prefixed with
the paragraph title. Deviation from the plan: the serde position is not
recovered (serde_hjson custom errors carry none); the index + kind in the
message replaces it.

**Symptom.** A single typo in a `constraints[].kind` (e.g. `bogus_kind_typo`)
makes *every* language command fail — `sentence`, `translate`, `parse`,
`syllabify`, `generate-word`, the Oracle — with:

```
inkhaven: config error: phonology HJSON parse failed: unknown constraint kind `bogus_kind_typo` at line 0 column 0
```

`language doctor` on the same project reports `Phonology : 1 rules` and no
error, because it only counts paragraphs under the Phonology chapter
(`src/cli/language/doctor.rs:98`, printed at `:304`); it never parses them.

**Where.** `src/conlang/types/constraint.rs:77` returns the error from a
`TryFrom<RawConstraint>` inside a custom `Deserialize`, so serde loses the
position (`line 0 column 0`). The loader at `src/conlang/types/mod.rs:98`
wraps it as a hard error and every caller propagates it.

**Fix.**
1. `doctor` runs `Phonology::load` (and the morphology / grammar / diachronic
   loaders) and reports each parse error as a finding with the paragraph title,
   instead of counting paragraphs.
2. Carry a position: deserialise constraints as `RawConstraint` in the
   `Phonology` struct and convert after the parse, so the error can name the
   constraint index and `kind` (`constraint #3 (kind "bogus_kind_typo")`).
3. Unknown kinds become a *warning* that drops the one constraint, not an
   error that disables the language. The strict behaviour stays behind
   `doctor`.

**Test.** A phonology block with one bad constraint: `doctor` reports it;
`syllabify` still runs; the error names the constraint.

### LB-2 · Undeclared class names in constraints and templates are silent no-ops

**Status: landed (3.17.0-dev).** `src/conlang/types/validate.rs` —
`Phonology::validate() -> Vec<Finding>` with `Severity::{Warning, Error}`.
Errors: undeclared class in `forbid_in_onset` / `forbid_in_coda` or a
template; a rule target or context (allophony, tone sandhi, romanization
contextual rules) that is neither a declared unit nor a class; a target that
spans ≥ 2 declared units (`ai > e`). Warnings: class member / bigram outside
the inventory, duplicate phoneme, multi-unit replacement, tone-class member
outside `tones`. Multi-segment targets are reported, not supported (the
follow-up stays open). 9 unit tests on `validate`, 4 on `block_findings`.

**Symptom.** `forbid_in_coda: { classes: [NoSuchClass] }` is accepted and never
fires. A template typo (`CVN` where `N` is undeclared) makes `generate-word`
return nothing, and `check` passes every word. The probe's `ai > e / _`
allophony rule (multi-segment target) was likewise accepted and never matched.

**Where.** `constraint.rs:60-77` stores class names as raw strings;
`Phonology::class_members` (`types/mod.rs:111`) returns an empty slice for an
unknown name. `types/allophony.rs:114 parse_rule` → `parse_atom` turns an
unknown multi-character token into a `Symbol` that can never equal a single
segment; `phonology/rewrite.rs:34 apply_rule` fixes `focus_len` at 1.

**Fix.** A `Phonology::validate(&self) -> Vec<Finding>` pass run by the loader
(warn) and by `doctor` (report): every class named by a constraint, a
template, or a rule context must exist; every rule focus / target must be a
declared phoneme, a declared class, or a single segment. Multi-segment targets
either get real support (sequence focus in `apply_rule`) or a hard error — the
plan recommends the error now and sequences as a follow-up.

**Test.** Unknown class in `forbid_in_coda` → finding; unknown class in a
template → finding; `ai > e` → finding; a valid block → no findings.

### LB-3 · No Unicode normalisation anywhere in the segmenter

**Status: landed (3.17.0-dev).** `conlang::types::norm` (`nfc` / `fold` /
`describe_segment`); `Phonology::from_hjson` runs `normalize()` over the
inventory, classes, bigram constraints, allophony + sandhi atoms, romanization
mappings and tone labels; `segment` composes its input; `add-word`, CSV import
and the TUI entry path go through one NFC pass in `create_dictionary_entry`;
`alphabet_bucket`, `MetaOverview::bucket_for_word` and `first_unknown_letter`
compare composed forms. Unknown segments that carry a combining mark print
with their codepoint. Tests in `types/mod.rs`, `types/norm.rs`, `oracle.rs`,
`cli/language/mod.rs`.

**Symptom.** `Phonology::segment` (`types/mod.rs:143-166`) does byte-prefix
matching against graphemes, so the same word in NFC and NFD forms segments
differently:

| input | result |
|---|---|
| `ãta` (precomposed, U+00E3) | 3 phonemes, `check` ✓ |
| `ãta` (a + U+0303) | 4 phonemes, `check` reports `segment(s) not in the inventory: ̃` (a bare combining mark) |

The same defect reaches the lexicon: a decomposed `éle` lands under bucket
`E` while a precomposed `éle` gets its own `É` bucket; `ą́` produced bucket
slug `03-a` and a later `atal` produced `05-a-2` (see LB-6).

**Where.** No `unicode-normalization` call exists under `src/conlang/` or
`src/cli/language/`. Inventory graphemes, rule contexts, lexicon headwords and
user input are all compared raw.

**Fix.** One `nfc()` pass at the three doors: (1) `Phonology::load` normalises
every grapheme, IPA string, class member and rule atom; (2) `segment` normalises
its input; (3) `add-word` / CSV import / Scrivener import normalise the
headword before bucketing and before the duplicate check. Error messages that
print a single segment print it with its codepoint (`̃ (U+0303)`) so a
combining mark is never shown bare.

**Test.** NFC and NFD spellings of the same word give identical `segment`,
`check`, `syllabify` and bucket results; a lexicon with a decomposed headword
round-trips through `export`.

### LB-4 · CSV export does not round-trip through CSV import

**Status: landed (3.17.0-dev).** `conlang::lexicon::headword::validate_headword`
is the one check; `cli::language::HeadwordChecker` loads the alphabet and the
`Phonology` block once and `add-word`, the CSV pre-flight, `language import`
(Toolbox / PolyGlot) and the TUI tree-pane add all call it. `--force` now
works on a single `add-word` too. One deviation from the fix as written: the
alphabet and the inventory are not both applied — the Phonology inventory is
the authority when declared, and `Meta/overview.alphabet` is the whitelist
only when it is not (the scaffold ships an A–Z alphabet that would otherwise
reject every `ŋ` the inventory declares). Alphabet matching is by letter
(longest-match, `"Ch ch"` declares a digraph) with diacritics folded to the
base letter.

**Symptom.** `language export --output words.csv` followed by
`language import words.csv` fails pre-flight on the exporter's own file:

```
Pre-flight validation failed — 4 violation(s) found:
  · row 4: `éle` contains `́` not in Meta/overview.alphabet
  · row 16: `ą́` contains `ą` not in Meta/overview.alphabet
  · row 17: `ŋaʔtʰə` contains `ŋ` not in Meta/overview.alphabet
  · row 19: `Ŋa` contains `Ŋ` not in Meta/overview.alphabet
```

Every one of those words was accepted by `add-word`, which runs no alphabet
check at all. The two doors disagree.

**Where.** `src/cli/language/import.rs:256` → `first_unknown_letter`
(`import.rs:620`) iterates `word.chars()` and compares each *char* against the
alphabet, so a digraph letter, a combining mark, or any phoneme grapheme longer
than one char is flagged. `add-word` (`src/cli/language/lexicon.rs:505`) only
derives a bucket.

**Fix.** One shared `validate_headword(word, &meta, &phonology) -> Result<()>`
in `src/conlang/lexicon/` used by `add-word`, CSV import, Scrivener import and
the TUI. It checks by *grapheme* using `Phonology::segment` (every segment is a
declared phoneme) and, when an alphabet is declared, by alphabet letter with
longest-match. Keep `--force` as the escape hatch on import.

**Test.** `export` → `import --new` of the probe lexicon is a no-op diff;
`add-word` rejects the same words `import` rejects.

### LB-5 · The CSV pre-flight phonology check is a stub

**Status: landed (3.17.0-dev).** `collect_phonology_inventories` and
`first_unknown_letter` are gone; the pre-flight segments each row's word
with `Phonology::segment` and names the first unknown segment (with its
codepoint when it is a combining mark). Tutorial 50 describes the real check
and the `--format csv` round trip.

**Symptom.** Tutorial 50 (line 148) says the import "will" cross-check words
against the phoneme inventory "in a future release". The code looks like it
does, but never can.

**Where.** `import.rs:563-606 collect_phonology_inventories` deserialises
`struct PhonologyRule { phonemes: Vec<String> }`. The shipped schema has
`phonemes` as a list of objects, so the whole-body parse fails, the fenced
fallback is a comment that returns `None`, and the function returns an empty
inventory — which `import.rs:263` then treats as "no phonology declared, skip".

**Fix.** Delete the function. The pre-flight uses `Phonology::load` and the
shared `validate_headword` from LB-4. Update tutorial 50 to describe the real
check.

**Test.** A CSV with a word containing an undeclared segment fails pre-flight
with the segment named; `--force` imports it.

### LB-6 · `add-word` derives three different keys for one word

**Status: landed (3.17.0-dev).** `create_dictionary_entry` enforces the
shape (`a headword must be 1–64 characters`, no control characters) before
anything touches the store, keys the duplicate check on the case-folded NFC
headword, admits a homograph with a different `type` as `word (2)` (same or
empty `type` is still the duplicate error, which the importer reports as a
skip), and buckets on the longest-matching alphabet letter or, as the
fallback, the first base letter (`ą́` and `a` share `A`; no more `a` / `a-2`
subchapters).

Three independent decisions are made from the raw headword, and they disagree:

- **Bucket** — `lexicon.rs:505-508 derive_alphabet_bucket`/`alphabet_bucket`
  takes the first char; `ą́` and `atal` both slug to `a`, producing two
  subchapters `03-a` and `05-a-2`. An empty word reports
  `could not derive alphabet bucket from \`\``; a 600-char word reaches the
  filesystem and reports `io error: File name too long (os error 63)`.
- **Duplicate check** — `lexicon.rs:531-534` compares with
  `eq_ignore_ascii_case`, so `ŋa` and `Ŋa` are both accepted while a real
  homograph (same spelling, different POS/sense) is rejected with
  `word \`pata\` already defined under \`Avesha/Dictionary/P\``.
- **Node title** — the raw string.

**Fix.** Key on the normalised headword (LB-3). Bucket on the first *alphabet
letter* with longest-match when an alphabet is declared, else on the first
grapheme. Duplicate check with Unicode case-folding and, when the POS differs,
allow the entry with a numbered title (`pata (2)`) — homographs are ordinary in
a lexicon. Validate length and emptiness up front
(`a headword must be 1–64 characters`).

**Test.** `ą`/`a` share one bucket; `ŋa` + `Ŋa` is a duplicate; `pata` noun +
`pata` verb both exist; empty and oversized words get the validation message.

### LB-7 · Translation treats the whole input as one clause

**Status: landed (3.17.0-dev).** `translate::split_clauses` /
`join_clauses` split on `.` `!` `?` and line breaks (a `.` between two
alphanumerics is not a terminator), `translate` and `reverse` run per clause
and re-join with the source punctuation, `cross` inherits both. A clause the
analyser cannot place is kept as `«…»` with a `clause` trace entry;
confidence is the minimum over clauses.

**Symptom.**

```
language translate Avesha "the stone. the river flows"
atal sora  (SVO · Tier 1 RBMT · confidence 0.90)
```

The first sentence is dropped and confidence is reported *high*. The TUI chord
passes an entire paragraph (`src/tui/app.rs:10802`), so this is the normal
case in the editor, not an edge case.

**Where.** `src/conlang/translate/mod.rs:150-180 structure` finds one verb and
builds one subject NP and one object NP from everything around it;
`english::analyze` has the same single-clause shape.

**Fix.** Split the input on sentence terminators (`. ! ?`, and newline) in
`translate()` before analysis, translate each clause, join with the source
punctuation, and report the *minimum* clause confidence. A clause with no
translatable content keeps its `«…»` markers rather than vanishing. `reverse`
and `cross` get the same split.

**Test.** Two-sentence input yields two clauses in order; a sentence with only
unknown words is preserved as `«stone»`; confidence is the minimum.

---

## P2 — wrong results that propagate

### LB-8 · The syllabifier merges every vowel run into one nucleus

**Status: landed (3.17.0-dev).** Hiatus by default; a new optional
`diphthongs: ["ai", "au"]` list (vowel IPA sequences, split by longest match
over the declared vowels) forms one nucleus, as does a diphthong declared as
its own phoneme. `Phonology::validate` warns on an entry that is not two or
more declared vowels. The one existing test that asserted a merged `ai`
nucleus now declares `diphthongs: ["ai"]`.

**Symptom.** `syllabify --word aia` → `aia`, 1 syllable. `paia` gets a single
stress mark. Stress, scansion, the saturation metric and `generate-word`'s
template check all inherit the error.

**Where.** `src/conlang/phonology/syllable.rs:43-52` collects *maximal runs* of
vowels as a nucleus.

**Fix.** Hiatus by default: each vowel is its own nucleus unless the pair is a
declared diphthong (a phoneme whose grapheme is the two-vowel sequence, or a
new optional `diphthongs: [ai, au]` list in the phonology block). Glides in a
`G` class stay in onset/coda as today.

**Test.** `aia` → 3 syllables; with `ai` declared, `aia` → 2; existing
syllable tests unchanged.

### LB-9 · Case is handled inconsistently before segmentation

**Status: landed (3.17.0-dev).** `Phonology::segment` case-folds (Unicode
lowercase, then NFC) both its input and the inventory graphemes; the inline
`to_lowercase()` at `parse.rs`, `oracle.rs`, `paradigm.rs` and `pronounce` is
gone. The remaining `let word = e.word.to_lowercase()` bindings in the
lexicon-wide metrics stay because the lowercase string is also used as a key
there; they are now redundant for segmentation, not conflicting.

**Symptom.** `check --word Pata` ✓ and `syllabify Pata` → 4 phonemes (the
capital `P` is an unknown segment that the total segmenter emits verbatim).
Scansion never lowercases, so the first word of every verse line segments
wrongly.

**Where.** `parse.rs:51`, `oracle.rs:53`, `paradigm.rs:68`, `metrics.rs:73`,
`pairs.rs:57`, `lexicon/mod.rs:115` lowercase before `segment`;
`cli/language/phonology.rs:68/152/179`, `scansion.rs:159`, `creative.rs:155`,
`diachronic/apply.rs:16`, `derive.rs:41`, `generate/lexicon.rs:106/132`,
`harmony.rs:64` do not.

**Fix.** `segment` case-folds its input itself (Unicode, after the NFC of
LB-3) and the call-site lowercasing is removed. A phonology that genuinely
distinguishes case in graphemes is not a supported case today and is not made
one here.

**Test.** `segment("Pata") == segment("pata")`; `check Pata` and `syllabify
Pata` agree with the lowercase forms; a scansion line starting with a capital
scans the same as its lowercase.

### LB-10 · Typst string literals use the markup escaper

**Status: landed (3.17.0-dev).** `output::typst_string` (quoted, `\\` and
`"` escaped, control characters as `\u{..}`) replaces `typst_text` inside
the `#set document(title: …)` and both `text(font: …)` literals, and in
`writing::compose`'s `text(font: …)`, which interpolated the family raw.

**Symptom.** A language title, font family or document title containing `"`
breaks compilation of the dictionary and grammar book.

**Where.** `src/conlang/output.rs:156` and `:813` interpolate `typst_text(…)`
(the *markup* escaper, `output.rs:126`) inside `"…"` string literals.
`typst_escapes` (`output.rs:116`) is the correct string-literal escaper and
already exists in the same file.

**Fix.** Use `typst_escapes` for every interpolation inside a Typst string
literal; grep the file for `\"{}\"` patterns.

**Test.** Title `Say "hi"` and font `My "Serif"` compile.

### LB-11 · Raw user strings become filesystem paths

**Status: landed (3.17.0-dev).** `norm::fs_safe_name` (case-folded NFC,
`[a-z0-9._-]`, ≤ 64, never empty) keys the translation-memory sidecar; a
memory saved under the old `<lowercased title>.json` is still read and
migrates on the next save. Glyph names are not slugged — case is
significant in a font — but `font::validate_glyph_name` (norad's rules plus
no path separator, no leading `.`, ≤ 64) runs before the glyph store write,
on `--slot` names, on the font block at build time and inside `build_ufo`,
so `norad::Glyph::new` can no longer panic. A `--phoneme /k/` with no
`--name` names the glyph `k`.

- `src/cli/language/writing.rs:371-394` — the glyph name falls through to
  `--phoneme`, then `dest = dir.join(format!("{glyph_name}.svg"))`, so
  `--phoneme /k/` escapes the glyph store (`/k/.svg`). `norad::Glyph::new`
  (`writing/font.rs:98`) panics on a name that is not a valid glyph name.
- `src/conlang/translate/memory.rs:204` — the translation-memory sidecar is
  `format!("{}.json", language.to_lowercase())` from the raw language title.

**Fix.** A single `fs_safe_name()` (slug: NFC, lowercase, `[a-z0-9._-]`, max
64) for both; the glyph name is validated against norad's rules before
`Glyph::new`, with a clean `Error::Config` otherwise. The translation memory
keys on the language book's `fs_name()` (the slug the rest of the store already
uses) rather than its title.

**Test.** `--phoneme /k/` is rejected or slugged; a title with `/` and spaces
yields one sidecar under `.inkhaven/translation-memory/`.

---

## Landing order

| step | items | why first |
|---|---|---|
| 1 | LB-3 + LB-9 (NFC + case-fold in `segment`) | one change removes a whole family of confusing results and is a prerequisite for 4/6 |
| 2 | LB-1 + LB-2 (`doctor` parses; `Phonology::validate`) | turns lockouts into findings; makes the rest testable from `doctor` |
| 3 | LB-4 + LB-5 + LB-6 (shared `validate_headword`; keyed bucket/dup) | user-visible data loss on the import/export path |
| 4 | LB-7 (clause split) | data loss in the editor chord |
| 5 | LB-8, LB-10, LB-11 | independent, one commit each |

Three to four commits; each carries its own tests, and the probe script's
commands become the integration check (`export` → `import --new` no-op,
NFC/NFD parity, two-sentence translate).

## Out of scope (tracked elsewhere)

Message wording (`--count 0`, WARN flood from `scripting::policy`), `--out` vs
`--output`, `language test` / `hypothesis` doc drift, `--json` coverage, the
Oracle / translation / import performance caches, dictionary ordering by
declared alphabet, and the deferred LANG-2 / LANG-3 roadmap items
(code-switching, variety morphology overrides, neural tiers) are improvements
rather than defects and will be filed as their own track.
