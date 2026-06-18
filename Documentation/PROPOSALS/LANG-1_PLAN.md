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
| **P5** | Writing systems + **config-driven pure-Rust font compilation** (+ AI text-to-SVG glyph draft) + input methods + Artefact ↔ inscription | **fontc, norad, write-fonts, read-fonts, unicode-normalization** | net-new, dep-heavy |
| **P6** | Analysis suite; generators (name / ceremony / curse / poetry / sample-text); translation pane; AI grammar book; importers (PolyGlot / Lexique / CWS / Toolbox); exporters (TSV/CSV/JSON/XLIFF/Anki/linguex/IPA-chart/dictionary-PDF) | none | net-new |
| **P7** | `language tutorial`; `Documentation/CONLANG.md` + tutorial; example language; perf pass (10k entries); **selectable output format** (see below) | none | — |

### P2 — AI-assisted dictionary generation

The lexicon-building loop. The non-negotiable invariant: **forms obey the
language; meanings come from the AI; nothing duplicates; nothing
auto-commits.** The deterministic P1.1 word generator supplies every candidate
form (so no proposal can violate the phonotactics) — the AI only chooses among
valid forms and assigns meaning. Everything lands in a proposal queue
([[feedback-ai-advisory]]); every gloss is in the project working language
([[feedback-multilingual]]).

**Query → words, in two stages:**

1. **Semantic frame.** From the request — `--topic`, `--count`, optional
   `--era` / `--register` / `--culture` — plus the language `overview`
   (environment, society), the AI proposes a ranked *concept list* (topic
   "seafaring" → hull, tide, mast, to-navigate, harbor, …). Optionally seeded
   from **Swadesh-100** core vocabulary (`scope_swadesh_100`). The list is
   reviewable before any word is coined.
2. **Form + sense per concept.** For each accepted concept the **deterministic
   generator** emits N phonotactically-valid candidate forms (P1.1); each is
   scored (phoneme-frequency fit, no working-language collision via whatlang,
   sound-symbolism alignment); the AI picks the best and fills the
   `DictionaryEntry` — gloss, senses, POS, semantic features, register, era, a
   usage example, and a short etymological rationale.

**Dedup & consistency gate (required — runs before anything is queued).**
Generation must never produce a duplicate or an accidental synonym, and must
never coin the same thing twice:

- **No double-generated form** — each candidate is checked against the existing
  lexicon's headwords *and* inflected `surface_forms()`, and against the other
  candidates in the same batch. Collisions are regenerated (new seed) or
  dropped, never silently emitted as a homophone.
- **No same meaning** — each proposed concept/gloss is checked against existing
  entries by (a) normalized gloss match and (b) **semantic similarity via
  embeddings** (fastembed, in-tree — the same vector machinery drift/RAG use),
  so "stone" vs "rock" is caught, not just exact strings. Above threshold → the
  concept is dropped from the frame or flagged as a deliberate variant.
- **No double-coined concept** — the semantic frame is deduped against itself
  and against the lexicon before form generation, so a batch never coins two
  words for the same concept.
- **Consistency** — POS / semantic-feature sanity, and an *etymology* check:
  if a concept is derivable from an existing root (P3 morphology), the pipeline
  proposes a **derived form** rather than an unrelated coinage, and flags it.

A wanted synonym (different register/dialect) stays possible — but only when
**explicitly** requested, never as an accident. Any residual collision the
author chose to keep surfaces in the queue as a **conflict badge**
(homophone / synonym / derivable-from-root) for an explicit keep / merge /
mark-variant / reject decision.

**Output → proposal queue** (`conlang_proposals`, TTL-expiring). Accepted
entries are written as `DictionaryEntry` HJSON paragraphs into the Dictionary
chapter under their alphabet bucket (book stays system-of-record) and projected
into the `conlang_lexicon` index; `source = AI_Proposal_Accepted`.

**Roots vs derived words.** P2 generates flat words. With morphology (**P3**)
the loop deepens — the AI generates **roots**, then the morphology engine
derives the productive family (agent, nominalization, compounds), so "seafarer"
is *derived from* the root for "sea" rather than coined independently and
etymology stays coherent. The **semantic-gap finder** (P6) closes the loop:
diff lexicon coverage vs a scope frame → ranked missing concepts → feed back
into generation.

**Surfaces.** CLI `inkhaven language generate-lexicon <lang> --topic … --count …
[--era …] [--register …]` (or `--scope <hjson>` for the full frame); TUI from
the `Ctrl+B X` hub → generator → proposal-queue review; Bund
`ink.conlang.generate.lexicon` (`fs_write`).

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
   Never the default; always user-replaceable. (See *text-to-SVG* below.)
4. **Compilation** — driven by a declarative **font-generation config** (next
   section): norad assembles a UFO in memory, fontc compiles it to
   `assets/fonts/<lang>.ttf|otf` (`language font build`). Composite syllabaries
   get auto-generated GSUB jamo-composition lookups (experimental).
5. **Typing** — input methods map key sequences → codepoints so the author can
   write the native script in the editor; the compiled font renders it in `.typ`
   output (and in previews where the terminal can load the PUA font). The TUI
   otherwise shows romanization with a `[native]` marker.

### Font-generation pipeline + config (P5 — accepted refinement)

The glyph **collection → font** step is **config-driven**, not flag-driven: a
`font` block in the Writing-system chapter is the single declarative control
surface (HJSON-everywhere, the same philosophy as `imposition` / `cover`). The
pipeline is `glyphs (bound) + font config → UFO (norad) → TTF/OTF (fontc)`,
re-runnable and deterministic.

```hjson
// Language/<name>/Writing system  →  `font` block
font: {
  family:        "Tengwar Eldar"
  style:         "Regular"          // also drives ss-set naming for variants
  format:        "otf"              // otf | ttf
  units_per_em:  1000
  metrics:       { ascent: 800, descent: -200, cap_height: 700, x_height: 500 }
  pua_start:     "U+E000"           // auto-assign codepoints from here
  advance:       "proportional"     // proportional | monospace(width)
  fit_to_em:     true               // normalize imported SVG into the em box
  slant_deg:     0                  // synthesize an oblique from the upright
  features: {
    ligatures:   [ { seq: ["t","h"], glyph: "th" } ]
    kerning:     [ { left: "A", right: "V", value: -80 } ]
    stylistic_sets: { ss01: "courtly", ss02: "lapidary" }  // calligraphic variants
  }
  notdef:        "box"              // glyph drawn for unmapped codepoints
  out:           "assets/fonts/eldar.otf"
  embed_in_book: true               // dictionary / grammar book uses it
}
```

- The config is the *contract* the compiler reads; glyph artwork + bindings are
  the inputs. Editing the config and re-running `language font build` rebuilds
  deterministically — no hidden state.
- Validation up front (every referenced glyph exists, codepoints don't collide,
  metrics are sane) with actionable errors, before any fontc call.
- Calligraphic **variants** are alternate SVGs per glyph surfaced as
  `ss01..ss20`; `--variant courtly` on export swaps the whole run.
- Bund: `ink.conlang.font.build` honours the same config; `ink.conlang.font.config`
  reads/writes it.

### Text-to-SVG: AI glyph draft (P5/P6 — nice-to-have)

`inkhaven language font ai-glyph <lang> --char <c> --describe "a tall upright
with a top hook and a single crossbar"` → the AI returns an **SVG path** scoped
to the em box, staged as a *draft* glyph in the **proposal queue** (never
auto-bound). The author previews, accepts/edits/rejects, and refines externally.

- **Honest scope:** LLM vector art is uneven — raw path data is often malformed
  or crude. So the output is validated through resvg (must parse + fit the
  viewBox) and treated strictly as a **starting point**, the AI counterpart to
  the procedural bootstrap (step 3), not a finished-typeface generator.
- Advisory-consistent ([[feedback-ai-advisory]]): proposal-queued, user-gated,
  prompt in the Prompts book (`conlang/glyph-from-description`), reproducible.
- A whole-script mode (`--describe-script "angular runic, sharp serifs"`) can
  draft the whole inventory in one pass for a consistent first cut to refine.

### Glyph suitability preflight (accepted refinement)

Every glyph — **AI-drafted *and* hand-authored** — passes a deterministic
**preflight** before it can be bound/built, run on import and again before
`language font build`. This mirrors the existing **PDF preflight**
(`src/pdf/preflight.rs`) for UX + code symmetry, and uses the in-tree
**resvg / usvg** parser (no new dep) to normalize the SVG into absolute-coord
paths, then checks suitability:

| Check | Severity | Why |
|---|---|---|
| parses as SVG | error | unparseable → unusable |
| has ≥1 fill contour (not stroke-only) | error | font outlines are filled, not stroked — stroke-only needs outlining |
| all contours closed | error | open subpaths don't fill predictably |
| monochrome (no gradient / image / filter / clip) | error | basic OTF/TTF outlines are single-colour |
| geometry fits the em box after `fit_to_em` | warn | over/undersized glyph throws off rhythm |
| contour winding consistent (holes opposite) | warn (auto-fixable) | counters fill solid otherwise |
| no self-intersection / overlap | warn | renders wrong without boolean union |
| contour / point count sane | warn | a traced photo bloats the font |

Output is a per-glyph report (`ok` / `warn` / `error`), surfaced in the
`Ctrl+B X` font panel and as **`inkhaven language font lint <lang>`**. Errors
block the build (or substitute `notdef` + warn); warnings are advisory.
Auto-fixable issues (winding) offer a one-key fix. The cheap checks
(parse / fill-count / closed / bbox / colour) are hand-rolled on usvg output;
robust self-intersection is a heuristic warning, with a precise geometry crate
deferred to the P5 dep decision.

### Multi-glyph (Hangul) & hieroglyphic composition (P5 plan)

Two writing-system kinds need glyphs *combined*, not just placed in a row. They
share **one spatial-composition engine** — a `SpatialTemplate` that arranges
component glyph outlines into a 2-D layout — applied at two different times:

**CompositeSyllabary (Hangul-style)** — jamo (initial 초성 / medial 중성 /
optional final 종성) pack into one square syllable block. Two strategies:

- **(a) Precomposed blocks — the default, ships first.** At *build time* the
  composition engine arranges the bound jamo SVGs per the block-layout rules
  (initial top-left; vertical vowel → right; horizontal vowel → bottom; final →
  bottom) into **one fused outline per syllable**, assigned a PUA codepoint.
  Deterministic, needs **no runtime shaper** — works with Typst's built-in
  shaping. We precompose only the **phonotactically-valid** syllable set (not
  all 11 172 Korean cells), and `log()` the count.
- **(b) True OpenType GSUB/GPOS composition — advanced, `--experimental`.**
  Emit `initial + medial (+ final) → block` substitution + jamo-positioning
  lookups so the font composes at shaping time. This is the hard part of P5;
  gated behind `writing.hangul_composition_experimental`.

**Hieroglyphic** — two sub-kinds:

- **HieroglyphicLinear** (Egyptian linear / Mayan reading order) is just an
  alphabet/logography **+ directionality** — the normal font path handles it.
- **HieroglyphicSpatial** (Egyptian **quadrats**, Mayan **glyph-blocks**) packs
  signs 2-D within a block. Real font tech can't arbitrary-pack glyphs, so this
  is **layout-time composition, not a font feature**: the manuscript marks a
  cluster (`#cluster(main, [affixes…])`), and assembly expands it into Typst
  `box` + `place` / `grid` primitives that arrange the component glyph boxes
  (each rendered from the compiled font). The cluster syntax is documented and
  offered by the `:lang:` picker when the system is HieroglyphicSpatial. Framed
  honestly as layout-time (not native shaping), with an upgrade path if a
  pure-Rust shaper lands.

The win: the *same* `SpatialTemplate` engine drives Hangul precomposition
(build-time, fuse to one outline) **and** hieroglyphic quadrats (layout-time,
arrange in Typst) — one mechanism, two binding times.

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
