# LANG-1 — ConLang Development Suite (implementation plan)

Status: approved, in progress
Owner: vulogov
Spec: RFC LANG-1 (ConLang Development Suite)
Line: **flagship 1.3 feature — begins 1.3.14, continues across 1.3.x until done**
Depends on: the shipped 1.2.13 `Language` system book (`LANGUAGE_BOOK.md`)

---

## 0. Framing — this is an expansion of a shipped feature, not a greenfield build

RFC LANG-1 was written as if inkhaven had no conlang support. It does. The
1.2.13 `Language` system book already ships a real substrate, and this plan
re-bases the RFC onto it rather than building a parallel system.

**Already present, reused as-is:**

| RFC feature | Already shipped (1.2.13+) |
|---|---|
| Languages system book (§8.5) | `Language` book (`SYSTEM_TAG_LANGUAGES = "language"`); per-language `Book` nodes; hierarchy allows Books-under-Books only under this root |
| Language CRUD (RFC P0) | `inkhaven language init / list / add-word / remove-word / define-rule / doctor / export` (`src/cli/language.rs`, ~127 KB) |
| Lexicon entries (§8.9, partial) | `DictionaryEntry { word, pos, translation, example, inflection }` as HJSON paragraphs under author-defined alphabet subchapters (`src/language_entry.rs`) |
| Lexicon highlight overlay (#41) | `DictionaryEntry::surface_forms()` already lights up the lemma + every inflected form in prose |
| Translation (§8.18, basic) | `Ctrl+B Q` (→ invented) / `Ctrl+B Shift+Q` (← invented), RAG-grounded on Dictionary + Grammar + Phonology + Sample-texts |
| Grammar / phonology rule storage | `define-rule --category grammar|phonology` → HJSON rule paragraphs |
| Alphabet bucketing, CSV import, exporters | `src/cli/language.rs` |

So LANG-1 = **add the deep deterministic engines, AI pipelines, and font
output on top of the existing book** — not a second book.

## 1. Storage architecture (decided: extend the existing book in place)

The book stays the **system of record**. The in-memory `Language` value
(RFC §8.1) is *reconstructed* from the book's HJSON chapters on demand — the
same pattern the existing `MetaOverview` / `DictionaryEntry` parsers already
use.

- **In-book HJSON — no new tables.** Everything bounded, author-editable, and
  useful as AI-RAG context: phoneme inventory, classes, syllable/morpheme
  templates, phonotactic constraints, allophony, stress/tone, romanization
  schemes, grammar typological tags, morphology specs, diachronic rules,
  writing-system spec, idioms/metaphors. These extend the existing `Meta` /
  `Grammar` / `Phonology` chapters with new typed HJSON blocks.
- **Normalized DuckDB tables — only the three where queryability genuinely
  demands it**, projected/derived from the book (book remains canonical):
  - `conlang_lexicon` — fast semantic / POS / register / era / etymology
    queries at the RFC's 10 000-entry target;
  - `conlang_cognates` — cross-language reflexes;
  - `conlang_usage` — per-word manuscript counts.
- **No `Languages` plural book. No 25-table schema. No migration of existing
  data.** The RFC's appendix-A schema is descoped to these three.

## 2. Namespaces (decided)

- **CLI:** fold into the existing **`inkhaven language …`** tree — one book,
  one command tree. RFC verbs map on: `conlang generate-word` →
  `language generate-word`, `conlang analyze` → `language analyze`,
  `conlang book` → `language book`, etc.
- **Bund:** **`ink.conlang.*`** exactly as the RFC specifies (no existing
  `ink.language.*` — clean, no collision).
- **TUI:** **`Ctrl+B X`** = the ConLang hub modal. `Ctrl+B U` (the RFC's
  proposed prefix) is already paragraph-undelete; mapping the live meta table,
  `X` is the only globally-free, meaningful plain letter. All RFC sub-letters
  (L / N / R / G / W / D / F / T / S / P / V …) are **modal-local** — captured
  inside the hub, exactly like `Ctrl+B ]` (tag picker) captures A/D/Space/T.
  Existing `Ctrl+B Q` / `Shift+Q` stay as-is and are surfaced from the hub.

## 3. Phases (re-based onto the existing substrate; each independently shippable)

Version prefix is **1.3.x** (flagship 1.3 feature). Phases ship across point
releases as they complete.

| Phase | Scope | New deps | Status vs existing |
|---|---|---|---|
| **P0** (slim) | `src/conlang/` skeleton; `Language` value loader from book HJSON; the 3 normalized tables + sync-on-save | none | CRUD already exists |
| **P1** | Phonology: inventory / classes / templates / constraints; deterministic validator + word generator; IPA table; allophony / stress / tone eval; romanization (bidirectional, multi-scheme) | none | **net-new engine** |
| **P2** | Rich lexicon fields; semantic + full-text search; AI lexicon pipeline + proposal queue; undefined-word manuscript scan; Places ↔ Language, Characters ↔ proficiency; `Ctrl+B X` hub | none | overlay + dict exist |
| **P3** | Morphology spec; paradigm generation; auto-gloss; grammar questionnaire; idioms / metaphors; derived-form proposals | none | net-new |
| **P4** | Diachronics: rule parser + evaluator; daughter-language derivation; cognate sets; family-tree viz (reuses resvg); AI comparative reconstruction | none | net-new |
| **P5** | Writing systems + **pure-Rust font compilation** + input methods + Artefact ↔ inscription | **fontc, norad, write-fonts, read-fonts, unicode-normalization** | net-new, dep-heavy |
| **P6** | Analysis suite; generators (name / ceremony / curse / poetry / sample-text); translation pane; AI grammar book; importers (PolyGlot / Lexique / CWS / Toolbox); exporters (TSV/CSV/JSON/XLIFF/Anki/linguex/IPA-chart/dictionary-PDF) | none | net-new |
| **P7** | `language tutorial`; `Documentation/CONLANG.md` + tutorial; example language; perf pass (10k entries); **selectable output format** (see below) | none | — |

### P6/P7 output: selectable `.md` / `.typ`, and the Typst path is a *real book*

The generated **grammar book**, **dictionary**, and **tutorial** take an
output-format selector (`--format md|typ`, default `typ`):

- **`.md`** — plain Markdown, for quick reading / RAG ingest / diffs.
- **`.typ`** — a polished, print-ready Typst book that uses the appropriate
  **Typst Universe `@preview` packages** for the job: e.g. linguistics glossing
  (`leip`/`glossy`-style interlinear), professional tables (`tablex`/`zebraw`),
  IPA + phonology charts, multi-column dictionary layout with running heads and
  thumb indices, a generated cover (per PDF-1), proper TOC, and the language's
  compiled font (P5) for native-script samples. The goal is an artefact that
  reads like a published reference grammar / dictionary, not a dump. Package
  choices are pinned and fetched through the existing in-process Typst engine
  (`typst_compile.engine = "inprocess"`, already supports `@preview` fetch).
- The `.typ` dictionary / tutorial **embeds and uses the language's generated
  font** (P5): headwords, examples, and inscriptions render in the native
  script next to their romanization (`#text(font: …)` → `assets/fonts/<lang>.ttf`);
  falls back to romanization-only when no font is built yet.

### How glyphs / fonts get populated (answer of record)

A terminal can't be a bezier canvas, so glyph **artwork** comes from outside;
inkhaven owns the **binding, compilation, preview, and typing**:

1. **Source SVG glyphs** — one SVG path per glyph, drawn in any vector tool
   (Inkscape / Illustrator / …) or imported from another conlang tool. Brought
   in with `inkhaven language font import-glyph <lang> --char <c> --svg <path>`
   (or `ink.conlang.font.import_glyph`). resvg parses the path; the outline is
   stored on the `Glyph` record in the Writing-system chapter.
2. **Binding** — each glyph maps to what it represents (phoneme / syllable /
   morpheme / PUA codepoint) in the Writing-system HJSON, managed from the
   `Ctrl+B X` hub's font panel. *This* is the editor's role: association +
   preview, not drawing.
3. **Optional bootstrap** — for authors with no artwork, an optional generator
   emits **starter SVG glyphs** (simple procedural strokes, one per phoneme) to
   import and refine externally; the AI can also propose a glyph-style brief.
   Never the default; always user-replaceable.
4. **Compilation** — norad assembles a UFO in memory, fontc compiles it to
   `assets/fonts/<lang>.ttf|otf` (`language font build`). Composite syllabaries
   get auto-generated GSUB jamo-composition lookups (experimental).
5. **Typing** — input methods map key sequences → codepoints so the author can
   write the native script in the editor; the compiled font renders it in `.typ`
   output (and in previews where the terminal can load the PUA font). The TUI
   otherwise shows romanization with a `[native]` marker.

**Callouts**

1. **P5 is the only dep-heavy phase** (5 new crates). The project's bar is
   zero-new-deps; P1–P4 + P6 stay dep-free, and P5's deps get their own
   go/no-go (possibly a follow-up cycle) when we reach it.
2. **Hieroglyphic spatial composition** is layout-time (Typst `box`/`place`),
   documented as such — not a font feature. **Hangul composition** ships
   behind `--experimental`. Audio/TTS, sign, whistled, cross-project sharing,
   harfbuzz shaping: out (RFC §4).

## 4. First increment — P1.1 (phonology substrate)

Pure-Rust, deterministic, zero new deps, fully testable, genuinely new
capability (the existing `Phonology` chapter is freeform RAG prose with no
engine behind it):

- `src/conlang/{mod.rs, types/{phoneme,template,constraint}.rs,
  phonology/validator.rs, generate/word.rs}`.
- HJSON schema for a typed phoneme block in the `Phonology` chapter
  (`phonemes`, `classes`, `templates`, `constraints`), parsed like the
  existing `MetaOverview` / `DictionaryEntry`.
- Deterministic constraint evaluator (single linear pass) + seeded word
  generator with constraint-retry.
- `inkhaven language generate-word <lang> [--role root|prefix|suffix] [--count N]`.
- Unit + **property tests**: every generated word satisfies all declared
  constraints; identical output for a given seed.

Subsequent P1 increments: IPA table + allophony eval (P1.2), stress/tone
(P1.3), romanization (P1.4).

## 5. Testing strategy (per RFC §13, scoped to each phase)

Unit tests on every deterministic evaluator; property tests on generators
(constraint satisfaction, seed determinism, romanization round-trip);
golden-language fixtures (a Quenya-like, a tone language, a jamo-composition
set) checked in; AI features always route through a proposal queue with
explicit accept/reject (snapshot the structured shape, not the prose).

## 6. Open items carried forward

- RFC §15 open questions (lexicon identity = UUID + indexed headword; senses
  vs entries; proposal TTL default 30d; WALS licensing; font on-disk) resolved
  per the RFC's own recommendations unless a phase surfaces a reason to revisit.
- The standing "Whole-Book AI Editor" 1.4 headline slides behind LANG-1 in the
  roadmap; LANG-1 is the flagship now.
