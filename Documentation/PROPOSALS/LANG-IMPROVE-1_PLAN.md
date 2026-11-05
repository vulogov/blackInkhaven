# LANG-IMPROVE-1 — the ConLang suite, quieter, faster, and consistent

| | |
|---|---|
| **Track** | LANG-IMPROVE-1 |
| **Status** | LI-1 landed on 3.18.0-dev; LI-2 … LI-10 proposed |
| **Created** | 2026-11-05 |
| **Author** | Vladimir Ulogov |
| **Target version** | 3.18.0 |
| **Depends on** | LANG-BUGFIX-1 (3.17.0), LANG-1, LANG-2, LANG-3 |
| **Companion docs** | `Documentation/CONLANG.md`, tutorials 49 / 50 / 74 / 75 |

LANG-BUGFIX-1 fixed the eleven *wrong answers* in the language subsystem and
closed with a list of things that were not defects but still cost the author
every day: a wall of warnings on every command, a model rebuilt from the store
for every word, a dictionary whose order the alphabet does not govern, a rule
form the engine accepts and never applies, flags that differ from command to
command, and documentation naming commands that do not exist. This track picks
that list up. It adds no new linguistic capability; it makes the existing
capability behave like one program.

The findings below come from a static sweep of `src/scripting/policy.rs`,
`src/store/`, `src/conlang/`, `src/cli/language/` (102 `language`
subcommands) and the language docs, on the `3.18.0-dev` tree. Line numbers are
as of `798d1cd4`. Every item lands with a test; the suggested landing order is at
the end.

---

## P1 — cost paid on every command

### LI-1 · 91 WARN lines on the first store write

**Status: landed (3.18.0-dev).** `apply_policy` now logs the per-word
`denying …` lines at DEBUG and one summary at INFO (`policy: 91 word(s)
denied by default (ai_write, editor_write, fs_write, keymap, store_write,
theme_write); set scripting.enabled_categories, scripting.enabled_words or
scripting.no_default_deny in inkhaven.hjson to change`). `hooks::fire` asks
`scripting::scripting_present()` before `init_adam`: the VM is built only
when it already exists, `scripting.bootstrap` is set, or the store holds at
least one `Script` node — counted inside DuckDB by the new
`Store::count_nodes_of_kind` (`JsonStorage::count_by_field`), not by a
hierarchy load. Trust is still decided inside `init_adam`, so the
pending-opt-in and deny notices are unchanged. The `.inkhaven/trust` file
proposed as a third signal was dropped: it grants trust, it does not define
lambdas. One correction to the text below: `denied_stub` never named the
word (an inline handler cannot see the word it was called as); its error now
names the knobs and the DEBUG target instead. The stale comment at
`policy.rs:95-96` is rewritten. Tests: `default_policy_logs_one_summary_line_not_a_flood`
(captured subscriber under the CLI filter), `scripting::presence_tests`,
`json_store_count_by_field_filters_exactly`.

**Symptom.** `inkhaven language add-word …` (and any CLI command that creates
or saves a node) prints about ninety lines of
`WARN inkhaven::scripting::policy: denying ink.tree.add (category store_write)`
before its own one-line answer. Authors run with `RUST_LOG=error` and lose the
real warnings with the noise — including the new `phonology warning:` lines
from LB-1/LB-2.

**Where.** `src/scripting/policy.rs:856-861`, `apply_policy`: one
`tracing::warn!` per registered word whose category is in the default deny set
(store_write 50, fs_write 18, editor_write 9, ai_write 9, keymap 4,
theme_write 1 — 91 words on the shipped `no_default_deny: false`). The doc
comment at `policy.rs:95-96` still says inkhaven registers *zero* words in those
categories; it was true in 1.2.11 when the line was written as a diagnostic,
and stopped being true as `ink.*` grew. The VM is built lazily by
`hooks::fire` (`src/scripting/hooks.rs:81`) **before** it checks whether any
hook lambda exists (`:97`), so `add-word` — which never runs Bund — builds the
whole VM, applies the policy and logs the flood just to discover there is no
`hook.on_create`. The CLI logger writes WARN to stderr for every non-TUI
subcommand (`src/main.rs:146-176`); the TUI routes it to the log file, which is
why only CLI users see it.

**Change.**
1. `apply_policy` logs one line: `info!("policy: {n} word(s) denied by
   default ({categories}); set scripting.no_default_deny or enable_categories
   to change")`, and each word at `debug!`. The `denied_stub` error
   (`policy.rs:872-875`) already names the offending word at call time, so the
   per-word WARN is not needed for its original purpose.
2. `hooks::fire` asks a cheap predicate before `init_adam`: the project has a
   bootstrap script configured, or the Scripts system book has children, or a
   `.inkhaven/trust` opt-in exists. None → return without building the VM
   (this makes the doc at `hooks.rs:49`, "silent no-op when Adam hasn't been
   initialised", true). The TUI keeps its eager `init_adam` (`src/tui/app.rs:457`).
3. Fix the stale comment at `policy.rs:95-96`.

**Test.** A test that builds the VM under the default policy and asserts the
captured log has one `policy:` line at INFO and none at WARN; a store test that
creates a node in a project with no scripts and asserts `ADAM` is still unset.

### LI-2 · The language model is rebuilt from the store for every call

**Symptom.** Every `language` command, every Bund `ink.lang.*` word, every
Oracle scan and every TUI translate builds Phonology, Morphology, the grammar
spec, the dictionary and the translation memory from scratch: a full
`Hierarchy::load` (`SELECT id, document FROM json_docs` over every node,
`src/store/hierarchy.rs:48`, `src/storage/engine.rs:557`) plus one
`get_content` SQL round trip per paragraph. A read-only command does at least
three full hierarchy scans before any conlang work (two in `Store::open`'s
`ensure_system_books`, one in `open_lang_book`). `load_phonology` re-prints
every validation finding on every load (`src/cli/language/mod.rs:1427-1435`),
so a command that loads twice warns twice.

Nothing in the conlang tree is cached: no `OnceLock`, no mtime, no hash
(`grep` finds only the WALS table in `src/conlang/features.rs:50`). The
translator rebuilds `GlossIndex` on every `translate()` call
(`src/conlang/translate/mod.rs:418`) and `ReverseIndex` — paradigm expansion
of every noun and verb — on every `reverse()` (`reverse.rs:270`);
`corpus::generate` and `eval::round_trip` do that per sentence
(`corpus.rs:100`, `eval.rs:49-51`). `GlossIndex` is a `Vec<Sense>` scanned
linearly, with a `to_lowercase` allocation per sense per check
(`lexmap.rs:59, 95, 103-128`). `oracle::check_word` rebuilds the root map by
segmenting every entry **per word** (`src/conlang/oracle.rs:79`,
`src/conlang/parse.rs:58-64`), and the TUI on-save scan reloads phonology,
morphology and the whole dictionary for every language on **every paragraph
save** (`src/tui/app.rs:19330-19338`, default on).

**Change.** One `LanguageModel` (new, `src/conlang/model.rs`):

```rust
pub struct LanguageModel {
    pub book: Node, pub overview: MetaOverview,
    pub phonology: Option<Phonology>, pub morphology: Option<Morphology>,
    pub grammar: Option<GrammarSpec>, pub entries: Vec<DictionaryEntry>,
    pub findings: Vec<Finding>,            // validate() run once, printed once
    gloss: OnceCell<GlossIndex>, reverse: OnceCell<ReverseIndex>,
    roots: OnceCell<HashMap<String, …>>,   // the Oracle's root map
    pub stamp: ModelStamp,                 // max(updated_at) over the book's nodes
}
impl LanguageModel {
    pub fn load(store: &Store, hierarchy: &Hierarchy, lang: &str) -> Result<Self>;
    pub fn gloss(&self) -> &GlossIndex; pub fn reverse(&self) -> &ReverseIndex; …
}
```

- `load` walks the language book **once**: the Phonology, Grammar, Dictionary,
  Texts and Meta paragraphs are fetched in one pass (a `get_content_batch(ids)`
  on the blob store, `SELECT data FROM blobs WHERE id IN (…)`, replaces D
  round trips with one). The eight loaders in `src/cli/language/mod.rs`
  (`load_phonology`, `load_morphology`, `load_grammar_spec`, `load_varieties`,
  `load_contact`, `load_loan_phonology`, `load_diachronics`, `load_dictionary`)
  and the duplicate in `src/tui/conlang_hub.rs:187` become thin views over it.
- `translate`, `reverse`, `cross`, `corpus`, `eval` and the Oracle take
  `&LanguageModel` and use its lazily built indexes; `GlossIndex` gains a
  `HashMap<String, Vec<SenseId>>` over folded gloss tokens so `has_sense` and
  `map` stop scanning. `TranslationMemory` is loaded once into the model and
  saved by the one writer (`remember`, `corpus --commit`).
- The TUI and the Bund `ctx()` (`src/scripting/stdlib/lang.rs:127`, 43 call
  sites) keep a `HashMap<lang, Arc<LanguageModel>>` and revalidate by `stamp`
  (the language book's max `updated_at`, read from the hierarchy they already
  hold) — so a script looping over `ink.lang.translate` loads once, and the
  on-save Oracle scan loads only after the language book itself changed.
- `INKHAVEN_PERF_TRACE` marks (`src/store/mod.rs:2126`) are added to
  `LanguageModel::load` and `translate`.

**Test.** Unit: `LanguageModel::load` on a fixture book issues one hierarchy
load and one content batch (counted through a store spy); `gloss()` twice is
one build; a modified dictionary entry changes `stamp`. Bench: a new
`benches/conlang.rs` (load, translate a 10-clause paragraph, reverse, scan a
200-word paragraph) added to `bench.yml`'s list — the suite has no conlang
bench today.

### LI-3 · Import costs O(rows × nodes) and rewrites the vector index per row

**Symptom.** `add-word --import` of a few hundred rows takes minutes. Per row,
`create_dictionary_entry` (`src/cli/language/lexicon.rs:575`) does **three**
full `Hierarchy::load`s (`:591`, `:630`, `:666`), re-reads and re-parses
`Meta/overview` to derive the bucket (`derive_alphabet_bucket`, `:1091`), then
`create_node` embeds the template twice, **rewrites the entire HNSW graph,
`records.bin` and `vecmap.json` to disk** (`Store::sync`, `src/store/mod.rs:1058`
→ `VecStore::save`, `src/storage/hnsw_store.rs:403`), writes the body file
twice, and `update_paragraph_content` re-reads and re-embeds the entry as two
more single-text inferences. Summary per row: 3 hierarchy scans, ~3 point
reads, 5 DuckDB writes, 4 embedding inferences, 1 full index rewrite, 2 file
writes, 2 hook fires. Eight call sites funnel through this path (CSV import,
Toolbox/PolyGlot import, `generate-lexicon`, `derive`, varieties, daughter
lexicon, TUI accept-words, Bund `add_word` / `derive_add`).

**Change.** A `DictionaryWriter` that owns one batch:

- Loaded once: the hierarchy, the `Alphabet` (from the `HeadwordChecker`
  already built for the pre-flight), the bucket → children map. After each
  `create_node` the writer updates its own map from the returned node instead
  of reloading the hierarchy. Hierarchy scans per import: 1, not 3 × rows.
- `create_dictionary_entry` takes the final body up front, so the template
  write + overwrite and the create-then-update double embed collapse into one
  `add_document` with the real text: 1 embedding batch per row, no re-embed.
- The vector-index flush is deferred: `Store::begin_batch()` /
  `end_batch()` (or a `sync_policy: Deferred` on the writer) turns the per-row
  `sync()` into one at the end. Hooks still fire per node (LI-1 makes that
  cheap when there are no scripts).
- Bund `ink.lang.add_word` runs the `HeadwordChecker` like every other door
  (`lang.rs:743-762` skips it today — the one door LB-4 missed).

**Test.** Import 200 fixture rows against a store spy: hierarchy loads = 1,
`VecStore::save` = 1, embeddings = 200; the resulting tree equals the one the
per-row path produces today (`export --format csv` byte-identical). Bench row in
`benches/conlang.rs`.

## P2 — results the author expects and does not get

### LI-4 · A multi-segment rule target never fires

**Symptom.** `allophony: [{ rule: "ai > e / _ #" }]` loads and does nothing;
since LB-2, `language doctor` says so (`` target `ai` spans 2 phonemes (a, i) but
a rule matches one segment — declare `ai` as a phoneme, or write one rule per
segment ``, `src/conlang/types/validate.rs:130-132`). Neither workaround is
right: declaring `ai` a phoneme changes syllabification (LB-8), and
monophthongisation cannot be written one segment at a time. The replacement
side has the mirror problem: `t > ts` is emitted as one item `"ts"`
(`src/conlang/phonology/rewrite.rs:45-46`), which later stages segment
differently from a declared `t` + `s`.

**Where.** `AllophonyRule.lhs: Option<PatternAtom>` and `rhs: Option<String>`
(`src/conlang/types/allophony.rs:30-40`); `parse_rule` turns the whole
left-hand side into one `Symbol` (`:155`) while contexts are already split on
whitespace (`parse_context`, `:130`); `apply_rule` hard-codes `focus_len = 1`
(`rewrite.rs:34`) and `focus_matches` tests one atom (`:66`). The same engine
serves tone sandhi (`tone_eval.rs:12`), diachronic change
(`diachronic/apply.rs:17`), morphology (`morphology/paradigm.rs:78`,
`parse.rs:95`) and variety `sound_changes` (`types/variety.rs:33`), so all of
them gain the capability; romanization and stress have their own matchers and
are untouched.

**Change.** `lhs: Vec<PatternAtom>` (empty = insertion), `rhs: Vec<String>`
(empty = deletion). `parse_rule` splits target and replacement on whitespace
like contexts; a single token that is not a declared unit or class is split
against the inventory in `Phonology::normalize` (which has the inventory; the
matcher does not) so `ai > e` and `a i > e` are the same rule.
`apply_rule` matches `seq[i..i+lhs.len()]` and splices `rhs`. `validate`'s
sequence finding becomes a *warning only when the split is ambiguous*; the
"declare or write one rule per segment" wording is retired. `trace`'s own
parser wrapper (`src/conlang/trace.rs:59`) follows.

**Test.** Beside the existing seven in `allophony_eval.rs:60-103`:
`monophthongisation_ai_to_e`, `affrication_t_to_t_s_emits_two_segments`,
`sequence_target_with_class_in_context`; parser tests for the whitespace and
inventory-split forms; the validate test at `validate.rs:376` inverted.

### LI-5 · The declared alphabet names buckets but does not order anything

**Symptom.** With `alphabet: ["A", "B", "Ch", "D", …]`, buckets appear in the
order words were first added (`10-ng` before `03-k` if `ŋa` came first),
entries inside a bucket in insertion order, and `export` and the printed
dictionary sort by lowercased **code point** (`render.rs:127`,
`src/conlang/output.rs:56`) with sections keyed by the first character
(`output.rs:49`, `render.rs:324-334`) — so `Ch` words land under `C`, `ŋ` sorts
after `z`, and the book's order contradicts the author's declaration.

**Where.** `create_dictionary_entry` places both bucket and entry with
`InsertPosition::End` (`lexicon.rs:619-627`, `:667-675`); `Alphabet`
(`headword.rs`) knows each letter's index but exposes no ordering API; the
store's only reorder primitive is `swap_siblings` (`src/store/mod.rs:1245`).

**Change.**
- `Alphabet::collate(a, b) -> Ordering`: compare headwords letter by letter by
  alphabet index (longest-match, folded, marks stripped as in `bucket_for`),
  unknown letters after known ones by code point, then the homograph suffix.
- `create_dictionary_entry` inserts the bucket at its alphabet position
  (`InsertPosition::Before` the first bucket with a higher index) and the entry
  at its collated position among its siblings.
- `Store::reorder_children(parent, ids)` — one primitive that sets the
  permutation of `order` and renames the `NN-` file prefixes in one pass
  (today a sort would be an O(n²) chain of `swap_siblings`, each followed by a
  hierarchy reload).
- `language sort-dictionary <lang> [--dry-run]` reorders an existing
  dictionary with it and reports moves; the TUI tree follows automatically.
- `export`, `dictionary`, `grammar-book` and the two-column render sort with
  `collate` and section by alphabet entry (`Ch` is a section) rather than by
  first character.

**Test.** `collate` on `["A","B","Ch","D"]`: `cha` after `ba`, before `da`;
`ŋa` after every declared letter; `pata (2)` after `pata`. Store test:
`reorder_children` renames files and `children_of` returns the permutation. CLI
test: `sort-dictionary --dry-run` on a scrambled fixture lists the moves;
export is in alphabet order.

## P3 — one surface

### LI-6 · Output-path flags: `--out`, `--output`, `--svg`

**Symptom.** Inside `language`, ten commands take `--out` (`sketch`,
`scaffold`, `dictionary`, `grammar-book`, `tutorial`, `font-build`,
`font-compose`, `spatial-typst`, `glyph-draft`, `export-translation`; mixed
`String` / `PathBuf`), `export` alone takes `-o/--output` (`src/cli/mod.rs:3868`),
and `ecology`'s output is `--svg` (`:4438`). `infer_long_args` is off, so the
wrong one is a hard error. The rest of the CLI is split the same way: the
book-export family (`export`, `manuscript`, `docx`, `epub`, `audiobook`) is
`-o/--output`; tooling (`pdf *`, `backup`, `package-help-corpus`) is `--out`.
`font-build` writes `<family>.ufo` / `.ttf` into the current directory when
`--out` is absent (`writing.rs:176-178`).

**Change.** Every `language` command that writes a user-named file takes
`-o, --output <PATH>` (`PathBuf`), with `--out` (and `--svg` on `ecology`) kept
as hidden aliases so existing scripts keep working; `font-build` without
`--output` writes under `<artefacts>/<lang>/fonts/` and says where. The
rest of the CLI is **not** touched in this track; a one-paragraph convention
("`-o/--output` for a file the user names") goes into `Documentation/CLI.md`
for new commands.

**Test.** clap `debug_assert` test over `LanguageCommand`: every variant with a
path-valued output argument names it `output` with short `o`; aliases parse.

### LI-7 · `--count 0` blames the constraints

**Symptom.** `generate-word --count 0` prints `no words satisfied the
constraints in 0 attempts — loosen the phonotactic constraints` and exits 0
(`src/cli/language/phonology.rs:213-219`); a *genuine* failure also exits 0.
`generate-word --count 1000000` is silently capped at 100 000
(`src/conlang/generate/word.rs:45,52`) but the summary reports the uncapped
number (`phonology.rs:224-230`). `compose`, `dialects`, `generate-lexicon`
(the LLM is asked to "Produce 0 dictionary entries", `generate/lexicon.rs:323`)
and `propose-loans` (`contact.rs:486`) accept 0 the same way.

**Change.** Every `--count` is `u32` with `value_parser!(u32).range(1..)` and a
documented maximum where one exists (`MAX_GENERATE_BATCH`), so clap refuses 0
and over-cap with the real reason; a generation that passes nothing exits 1
with `0 of N candidates passed the constraints — …`.

**Test.** clap tests for `--count 0` and over-cap; a constrained fixture where
nothing passes returns `Err`.

### LI-8 · Message wording

**Symptom.** Eight things an author sees: `STRESS RULE: {:?}` debug output in
the grammar-book and tutorial prompts (`books.rs:61,333`); `unknown --kind`
lists six kinds and omits `ceremony` and the aliases it accepts
(`authoring.rs:122` vs `:86`); `` `{character}` commands no language ``
(`contact.rs:238`); "create it first (e.g. `ink.lang.init`)" in a CLI error
(`scaffold.rs:134`) where every other path says `run inkhaven language init
<name> first`; hints that name `font-build …` without the `inkhaven language`
prefix (`books.rs:247,478,562`, `writing.rs:710`); "no languages defined" on
stderr in one command and "no languages yet" on stdout in another
(`scaffold.rs:529`, `diachronic.rs:113`); `entr(y/ies)` (`translate.rs:580`);
"wrote …" confirmations split between stdout and stderr; `serialise` /
`serializing` both present.

**Change.** One pass over `src/cli/language/`: `Display` for stress rules;
the kind list generated from the match; one `not_found_hint(lang)` helper;
confirmations on stderr, data on stdout (the rule `--json` already follows);
British spelling as the rest of the tree. One commit.

**Test.** Snapshot tests on the eight messages.

### LI-9 · `--json` where a script needs it

**Symptom.** `generate-word`, `syllabify`, `ipa`, `stress`, `romanize`,
`tone`, `scan`, `list`, `paradigm`, `sentence`, `gloss`, `memory`,
`varieties`, `dialects`, `cognates`, `family-tree` and `sound-change` print
text only; 40 of the 102 subcommands have `--json`. On the Bund side
`ink.lang.export` writes the `.itm` translation bundle (matches CLI
`export-translation`, not `export`), `ink.lang.generate_word` returns one
seeded word where the CLI returns a batch, and `ink.lang.list` drops the counts
the CLI table shows.

**Change.** A shared `emit(json: bool, &T: Serialize, text: impl Fn)` helper in
`src/cli/language/mod.rs`; the seventeen commands above gain `--json` with the
structures the Bund words already return where one exists (so CLI and script
agree on shape). Bund: `ink.lang.export_translation` as the canonical name
with `ink.lang.export` kept as an alias; `ink.lang.generate_words ( lang role
count -- list )`; `ink.lang.list` returns `[{name, entries, phonemes}]`.

**Test.** Each new `--json` output deserialises back into the type; the
`ink.lang.*` alias table test extended.

### LI-10 · Docs name commands that do not exist, and omit many that do

**Symptom.** `CONLANG.md:564,575` tells users `export … --out`; the flag is
`--output`. Tutorial 49 (`:766-767`, `:958-959`) describes a "headless
`inkhaven language test` corpus-driven drift detector" that was never a
command (the nearest are `eval` and `corpus`), and lists export formats
`dictionary|grammar|anki|json` with `phrasebook` "still open" (`:964-966`) —
the real list is `json, anki, dictionary-twocol, csv, grammar, phrasebook,
xliff, linguex, ipa-chart`. `OUTPUT_PANE.md:148` names `lang.translate` where
the canonical word is `ink.lang.translate`. Meanwhile the whole hypothesis
group (`hypothesize`, `hypotheses`, `hypothesis`, `hypothesis-status`,
`hypothesis-check`), the corpus/IGT group (`igt`, `texts`, `frequency`,
`concordance`, `collocations`, `trace`), the analysis group (`metrics`,
`universals`, `grammar-check`, `pairs`, `naturalness`, `harmony`,
`distribution`, `sketch`, `suggest-phonemes`, `scaffold`), the syntax checkers
(`parse`, `tree`, `movement`, `binding`, `check`, `check-clause`,
`check-agreement`, `link`) and `define-rule` appear in no doc as commands.

**Change.**
- Fix the six mismatches.
- A generated **command index** section in `CONLANG.md` (one line per
  subcommand from clap's `about`, grouped as above), produced by a small
  maintainer subcommand `inkhaven language --index-markdown` (hidden) and
  pasted in; the release checklist regenerates it with the README.
- A **drift test** `tests/docs_language_surface.rs`: every `inkhaven language
  <name>` and `--flag` mentioned in `CONLANG.md` and `Documentation/Tutorials/*.md`
  must exist on `LanguageCommand` (clap introspection), so the next drift fails
  CI instead of waiting for a reader.

**Test.** The drift test itself (it fails on today's tree and passes after the
six fixes).

---

## Landing order

| step | items | why first |
|---|---|---|
| 1 | LI-1 (policy noise, lazy VM) | one afternoon; makes every later probe readable without `RUST_LOG=error` |
| 2 | LI-2 (`LanguageModel`) | the substrate LI-3, the Oracle and the TUI cache build on; the bench lands with it |
| 3 | LI-3 (`DictionaryWriter`, deferred sync) | the measurable win: import from minutes to seconds; closes the Bund door LB-4 missed |
| 4 | LI-4 (sequence targets) | independent engine change, one commit, five engines gain it |
| 5 | LI-5 (alphabet collation, `sort-dictionary`) | needs `Alphabet::collate` and the store primitive; export/render follow |
| 6 | LI-6 + LI-7 + LI-8 + LI-9 (surface) | mechanical, one commit each, after the engine work so messages are written once |
| 7 | LI-10 (docs + drift test) | last, so it indexes the final surface; the drift test then guards it |

Seven to nine commits. The LANG-BUGFIX-1 probe script (86 commands against the
`Avesha` project) is the integration check, re-run without `RUST_LOG`; the new
`benches/conlang.rs` gives the before/after numbers for LI-2 and LI-3 in the
release notes.

## Out of scope (tracked elsewhere)

- **Code-switching** (LANG-2 §4, postponed to the translation track) and the
  **Tier 3 resolver** and **TUI translate-pane cutover** (LANG-3 P2, held for
  the TUI rearchitecture) are capability work, not consolidation; they stay on
  their RFCs.
- The `--out` / `--output` split **outside** `language` (`pdf *`, `backup`,
  `index*` vs the book-export family) is noted in LI-6 and left alone; changing
  it is a CLI-wide decision.
- A read-only `Store::open` that skips `ensure_system_books` would save two
  hierarchy scans for every CLI command, not just `language`; it belongs to a
  store track.
- The Windows GNU release build (`ort-sys` has no prebuilt for that target) is
  unrelated to the language suite.
