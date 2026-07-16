# LING-1 — Pre-Flight & Implementation Plan (working)

Companion to RFC LING-1 (The Inkhaven Linguistic Layer). Grounded in a three-scout
verification of the actual codebase (ConLang primitives; TUI/Inner-family/para/Output;
Bund/CLI/store/config). This is the plan to (a) close 1.6, (b) harden/generalize the
reused foundations, (c) amend the RFC's reuse claims, (d) sequence the build, (e) cut.

---

## 1. Verdict

- **"Zero new crates" — TRUE.** DuckDB, serde, `include_str!`, the Bund sandbox, the
  para registry, `book_rag`, the export match — all present and idiomatic.
- **"All reuse" — PARTIAL.** The *plumbing* is solid and reusable. The RFC's headline —
  the **analysis direction** (`/parse`, `/tree`, `/movement`, `/binding`) — is largely
  **net-new engine work filed as reuse**. Correct this before estimating.
- **Scale — a program, not a release.** 15 phases, ~60 CLI verbs, ~40 Bund verbs,
  5 DuckDB tables, ~680 KB static data. **1.7.0 ships a slice**; the rest are 1.7.x on
  the theology-arc cadence.
- **Test baseline is stale:** RFC says 2395 (1.6.15). Current is **2464 (1.6.22)**.
  Rebased target ≈ **2834**.

---

## 2. Foundation ledger

**SOLID (drop-in reuse):** SPE rewriter `apply_ordered`; paradigm generator (incl.
non-concatenative infix/circumfix/ablaut/reduplication — already exists); agreement;
gloss indexer; clause generators (`assemble`/relative/complement/coordinate);
diachronic `derive_form`/`derive_lexicon`; family tree; translate RBMT + corpus/memory;
lexicon `analyze`/`stats`. · DuckDB `StorageEngine::new(path, INIT_SQL, pool)` with the
`inner_socrates` 6-table store as template + `ensure_schema_version` migrations. ·
Bund policy tiers `store_read`/`store_write`/`ai_write` (table-driven, 57 `ink.lang.*`
verbs). · `inkhaven language` CLI host (72 subcommands). · `book_rag::retrieve`
subtree-scoped by `book_id`. · Inner-family (fast/slow split; `run_unified_check` hook;
tick; personas `bundled()`/`active()`; intent raw-core `consult_raw`/`list_intent_rows_raw`
already generic). · `LING_TYPES` parallel to `UTOPIA_TYPES`. · Output `Message::new` +
kinds + glyph. · Config nested `#[serde(default)]` sub-structs. · `include_str!` of
`assets/conlang/` (precedent: `english-pool-v1.txt`).

**GENERALIZE FIRST (reuse, but not generic today):**
- Research-TUI *skeleton* is reusable, but `research/app.rs` carries ~24 research-specific
  pollers + slash-commands + provenance-gated commit → **extract a shared shell**.
- `FactsTree` root is **hardcoded to `SYSTEM_TAG_FACTS`** → parameterize by system-tag.
- INNER-GROUND-1 has **no source registry** → adding a Language source edits
  `build_grounding` inline + the 6-language `Labels`.

**NET-NEW (not reuse, despite RFC framing):**
- **Syntax tree/parser / movement / binding.** Syntax engine is a *generator*, no tree,
  no c-command, roles ephemeral. `/tree`,`/movement`,`/binding` (L-P6b) built from scratch.
  *The RFC's hardest, least-founded piece.*
- **Typed grammar blocks** (`ug_parameters`/`movement`/`binding`/`pragmatics`/`verb_classes`/
  `constructions`/`colexifications`/`prosody`). Zero code; current grammar is
  `BTreeMap<String,String>`. Each = new struct + reader + validation.
- **Morphological parser** (generate-and-score). Rewriter is forward-only/non-invertible;
  existing "parse" = forward-generate-then-index (`gloss.rs`). L-P5 `/parse` is new.
- **PHOIBLE distinctive-feature matrix.** Phoneme model is sonority + V/C only. Needed for
  `/naturalness`, `/pairs` (feature-distance), natural classes, `/correspondences` alignment.
- **Persistence:** `conlang_lexicon`/`conlang_usage`/`conlang_cognates` **do not exist**.
  ConLang = HJSON in a Languages system-book + one JSON sidecar (translation memory).
  **No usage tracking at all.** Cognates recomputed on the fly.
- `inkhaven assemble` CLI verb (only `assemble_book` exists, called from `build`/TUI).
- `linguistic:` config block.

---

## 3. Pre-1.7 stability gate (complete BEFORE cutting `1.7.0-dev`)

### Gate A — hygiene & 1.6 closeout (small, fast)
- [x] **Fix `RIGOR` Output-source wiring** — added `message_source` arm + `SOURCES` entry
      + a `source_classification` assertion (`confront`/`locus`/`rigor` are now filterable
      by provenance, not lumped into `"other"`).
- [x] **Bund verb-category coverage test** — `every_registered_word_is_classified` enumerates
      the VM's registered `ink.*` verbs and asserts each is either gated (`WORD_CATEGORIES`)
      or on the new explicit `PURE_UNCATEGORISED` allowlist. **Surfaced a real latent gap:**
      16 store-read verbs (5 `event.critique.*`, 6 `inner_socrates.*`, 5
      `world…timeline.*`) were uncategorised → escaping `disabled_categories`; now gated
      `store_read`. 19 genuinely-pure verbs (`ink.lang.dict` + 18 in-memory PDF-handle
      transforms; FS gated only at `load`/`save`) documented on the allowlist. Second test
      guards the allowlist against stale/conflicting entries.
- [x] Dep freshness clean (`cargo update --dry-run` shows no yanked/removed; the 1.6.22 spin
      fix holds). 7 ignored tests reviewed — all intentional (heavy Typst/PDF fidelity gates,
      network-ONNX fixtures, manual docx inspection); none broken. *`cargo-audit` binary not
      installed — optional `cargo install cargo-audit` for a CVE sweep.*
- [ ] Triage the 1.6 backlog (Output-pane free-text search + fullscreen) → ship or defer.

*Gate-A result: suite 2464 → **2466** (net +2 policy tests), 0 failed, clean build, no
warnings, no attribution. Committed on `1.6.23-dev` (GPG-signed).*

### Gate B — enabling refactors (non-behavioral, land on 1.6.23-dev)
- [x] **Extract a reusable companion-TUI shell** — new `src/tui_host.rs`: `TuiHost` trait +
      `run_loop`/`run_loop_with` (injectable `InputSource` → headless-testable) +
      `with_terminal` lifecycle. Research TUI rides it (behaviour unchanged; 24 pollers →
      `poll_all`, spinner → `tick`, `on_key` → `handle_key`). 3 shell tests. Committed.
- [x] **Parameterize the left-pane tree by system-tag** — `FactsTree::new(h, system_tag)`;
      root resolution was the only tag-specific step. Research passes `SYSTEM_TAG_FACTS`; the
      Languages pane passes its own. Promotion to a shared `SystemBookTree` deferred to L-P0
      (shape the API against the real second consumer). Committed.
- [x] **Harden the ConLang suite to the durable bar** — adversarial panic-surface audit
      across all input-facing modules found the suite already *total* against panics (guarded
      `chars().next()`, `saturating_sub`, `is_empty` before indexing; a prior panic-audit
      trail). The real exposure was **unbounded-allocation DoS in paradigm generation**: a
      cell may list a stem-growing morpheme any number of times — `full` reduplication doubles
      the stem per pass (`2^N`), ablaut rules multiply it (`~64^N`) — OOMing the process the
      first time any paradigm generates (also drives gloss/reverse/translate). Fixed with a
      `MAX_STEM_SEGMENTS` (256) cap + adversarial test; also made the `poem` meter arithmetic
      saturating. Committed. *(No HIGH panic surface existed — the suite was in good shape.)*
- [ ] *(optional, deferred)* Give INNER-GROUND-1 a small source registry — additive; do it
      with the Language source at L-P0.

*Gate-B result: suite 2466 → **2470** (+4: 3 shell, 1 DoS cap), 0 failed, **zero warnings
tree-wide** (bin + tests), no attribution. Four GPG-signed commits on `1.6.23-dev`. Companion
TUI shell + system-tag-parameterised tree + hardened ConLang core are ready for L-P0.*

### Gate C — architecture decisions (amend the RFC)
- [ ] **Reframe net-new pieces as new, not reuse**; re-estimate L-P5/L-P6b upward.
- [ ] **Persistence decision:** `conlang_usage` doesn't exist. For the first cut, compute
      usage from the manuscript on the fly (lexicon-overlay scan exists) or scope
      `/zipf`,`/ttr` to IGT + generated corpus. Don't block on a new usage table.
- [ ] **Feature matrix = separate IPA-keyed lookup table**, not a `Phoneme` change.
- [ ] **Fix invented bindings/glyphs:** `→ L` collides with "view ledger" — pick a free
      letter; `✎` is Inner Editor's — pick distinct Linguist glyphs; `Ctrl+B ]` picker
      doesn't exist — reuse the `i`-in-Tree structural picker.
- [x] **Split `cli/language.rs`** (9.3 k-line flat match) into a directory module before
      +40 arms. `language.rs` → `language/mod.rs` (run dispatch + shared core: loaders,
      dictionary CRUD, scaffold, stats) + **11 per-family submodules**: render, import,
      writing, translate, syntax, varieties, books, diachronic, phonology, contact,
      morphology. Each does `use super::*`; mod.rs does `pub(crate) use <sub>::*` so every
      external `crate::cli::language::*` path (stdlib::lang, export::html::companions) stays
      stable. mod.rs **9314 → 4233 lines (−55%)**; the +40 linguistic-layer arms now have
      logical homes. Pure code move, zero warnings (bin + tests), suite 2470. 5 signed commits
      on `1.7.0-dev`. *(Remaining shared-core families — dictionary CRUD, scaffold, expressions
      — are interleaved with loaders; tidy opportunistically during L-P0.)*
- [ ] **Add `inkhaven assemble`** as a thin verb over `assemble_book`.
- [ ] **Module naming:** `src/linguistic/` new, wrapping `src/conlang/` primitives.
- [ ] **Rebase test baseline** 2395 → 2464; target ≈ 2834.
- [ ] Bundle static data via `include_str!` of `assets/linguistic/*` data files — **not**
      giant `static` Rust literals (compile-time).

---

## 4. Re-sequenced phase plan (waves = 1.7.x releases)

Reordered from the RFC's L-P0..L-P13 by **foundation readiness** — deterministic work on
solid ground first; the net-new engine later. Each phase notes foundation + net-new + risk.

### Wave 1 — 1.7.0 "The Linguist's Desk" (solid ground; visible value)
- **L-P0 TUI shell** — reuse the extracted shell + parameterized tree; new `src/linguistic/`,
  session JSON, AI chat grounded on pinned Language chapters. *Foundation: SOLID (post-Gate-B).*
- **L-P2 metrics (feature-matrix-free subset)** — `/zipf` `/ttr` `/entropy` `/saturation`
  `/foot` `/meter` `/mora` + `prosody` block. Defer `/naturalness` `/pairs` `/distribution`
  `/harmony` to Wave 2 (need feature matrix). *Foundation: SOLID; usage computed on the fly.*
- **L-P3 typological universals** — `/universals` `/survey` `/morphotype` `/parameter-coverage`;
  static WALS/Greenberg over the existing grammar block; extend 16→22 features.
  *Foundation: SOLID (static tables) + new typed reads.*

### Wave 2 — 1.7.1–1.7.2 (the interactive loop + the feature matrix)
- **L-P1 Consequence Tracer** — re-parse the model with the pending change + diff.
  *Net-new: the model-diff.* Depends on the language model being cheaply re-buildable (it is).
- **PHOIBLE feature matrix** (enables the deferred L-P2 phonology metrics) — new IPA-keyed
  lookup table + `/naturalness` `/pairs` `/distribution` `/harmony` `/suggest-phonemes`.
- **Typed grammar blocks** (`ug_parameters`/`movement`/`binding`/`pragmatics`/`verb_classes`/
  `constructions`/`colexifications`) — the schema foundation the later engine needs.
- **L-P4 scaffold + elicit + grammar-sketch.** *Foundation: SOLID (AI + static frames).*

### Wave 3 — 1.7.3–1.7.5 (the net-new analysis engine — the hard part)
- **L-P5 morphological parser + linker + theme** — generate-and-score parser; RRG `/link`
  over the new `verb_classes` block; `/theme`. *Net-new; perf-sensitive.*
- **L-P6a Oracle levels 1–4** — phonotactics/morphology/agreement/syntax over the parser +
  generators. `para:ling-passage` fires on save (hash-cached, deterministic-only).
- **L-P6b Oracle levels 5–6 + X-bar** — `/tree`, `/movement`, `/binding`; `ling_movement`
  table; island/Binding tables. **The RFC's most ambitious, least-founded phase — budget
  generously.**

### Wave 4 — 1.7.6–1.7.8 (research-process instruments)
- **L-P7 IGT corpus + Annotation Workbench** (`ling_igt`, `ling_annotation`).
- **L-P8 Hypothesis Register + AI semantic layer** (`ling_hypothesis`; `/semantic` `/field`
  `/colexify` `/realia`; CLICS table).
- **L-P9 Corpus Engine** (`ling_corpus_run`; batch gen + violation types; export formats).

### Wave 5 — 1.7.9+ (readers, comparison, publishing)
- **L-P10 Inner Linguist** (`inner_linguist.db`; fast/slow; free chord ≠ L; distinct glyphs;
  extended grounding).
- **L-P11 lexicostatistics + `/correspondences`** (needs a cognate store or on-the-fly + the
  feature matrix for alignment).
- **L-P12 paper mode** (`para:ling-example`; journal templates; `inkhaven assemble` verb).
- **L-P13 diachronic simulation** (extends `derive_form` with a diffusion scheduler).

### Companion book
*Linguistics with Inkhaven* — a worked example (develop → verify → analyze → research one
language), mirroring the theology manual. Grow it wave by wave; ships pieces per cut.

---

## 5. Test targets (rebased)

Baseline **2464**. RFC deltas hold directionally (+370 across L-P0..L-P13) → final ≈ **2834**.
Expect L-P5/L-P6b to exceed their RFC deltas (net-new engine).

---

## 6. Immediate next actions

1. **Gate A** (hygiene) — RIGOR wiring + Bund-verb-category test + audit/ignored-tests/backlog.
2. **Gate B** enabling refactors, in order: TUI shell → tree parameterization → ConLang hardening.
3. **Amend the RFC** with §3 Gate C decisions (reuse→net-new, persistence, feature matrix,
   bindings/glyphs, cli split, assemble verb, rebased numbers).
4. Then cut **`1.7.0-dev`** and build Wave 1 (L-P0/L-P2-subset/L-P3).

---

## 7 · RFC Amendments (formal)

Amendments to RFC LING-1 (Final), driven by the three-scout codebase verification. Each
is mapped to the RFC section/family/phase it modifies and stated as an authoritative
delta. Status: **Proposed** (fold into the RFC on acceptance). Chord/glyph choices are
proposals to confirm against the live tables at implementation.

### A-1 — Reclassify the analysis engine as NET-NEW, not reuse
*Targets: §6 Reuse Map, §11 Gap Register, §12 Phase Map, §0 mandate.*
The verification found no foundation for the analysis-direction primitives the reuse map
credits as existing. Reclassify and re-estimate:
- **Syntax tree / movement / binding** — the syntax engine (`conlang/syntax.rs`) is a
  *generator*: it orders constituents and case-marks, returning a flat word list; the
  `Role` enum is ephemeral (production-only), and there is **no tree, no c-command, no
  movement**. Reuse-map rows *"X-bar tree construction … reuses existing syntax engine
  role assignments," "Binding c-command computation,"* and *"AGREE feature checking"* are
  corrected to **NET-NEW** (Family C levels 5–6; `/tree`,`/movement`,`/binding`; L-P6b).
- **Morphological parser** — `phonology::rewrite::apply_ordered` is **forward-only and
  non-invertible**; the existing "parse" is forward-generate-then-index (`morphology/gloss.rs`),
  which loses on unseen words and ambiguity. Family D `/parse` (generate-and-score) is
  **NET-NEW** (L-P5). `apply_ordered` remains a genuine reuse in the *forward* direction
  (paradigm realization, diachronic derivation).
- **Typed grammar blocks** — `ug_parameters`, `movement`, `binding`, `pragmatics`,
  `verb_classes`, `constructions`, `colexifications`, `prosody` have **zero code**; the
  current per-language grammar is `GrammarSpec { grammar: BTreeMap<String,String> }`.
  Each block is **NET-NEW** (typed struct + HJSON reader + validation).
- **Distinctive-feature matrix** — the `Phoneme` model is `{ ipa, romanize, kind:
  Vowel|Consonant, sonority }`; there is **no place/manner/voice** decomposition. The
  PHOIBLE matrix and a feature-aware phoneme lookup are **NET-NEW** (bundled data + code)
  and are a hard dependency of `/naturalness`, `/pairs` (feature-distance),
  `/suggest-phonemes`, and `/correspondences` alignment.

### A-2 — Persistence: correct the `conlang_*` table assumption
*Targets: §4 schema, §6 reuse map, Family G (G.1, G.5), Family E provenance.*
There are **no `conlang_lexicon` / `conlang_usage` / `conlang_cognates` DuckDB tables.**
ConLang is stored as **HJSON paragraphs inside the "Languages" system book**, with one
`.inkhaven/` JSON sidecar (translation memory). **There is no usage tracking of any kind.**
- `ling.duckdb` (5 tables) stands as a **new** store (was never claimed to exist).
- Family G metrics that read `conlang_usage` — `/zipf` ("vs. manuscript usage"),
  `/ttr --corpus manuscript` — are re-based: **compute usage on the fly** from the
  manuscript via the existing lexicon-overlay scan, or scope to the IGT + generated
  corpus. **No new usage table is required for Wave 1.**
- `/correspondences` "reuses `conlang_cognates`" → cognates are **recomputed on the fly**
  (`derive_form` over the book hierarchy). Keep on-the-fly (recommended) or add an optional
  cognate cache in L-P11; either way it is not an existing table to extend.

### A-3 — Rebase the test baseline
*Targets: masthead "Test baseline," §13.*
Baseline **2395 (1.6.15) → 2464 (1.6.22)**. Directional delta (+370) holds; absolute target
**≈2834** (not 2765). Expect L-P5 and L-P6b to exceed their per-phase deltas (net-new engine);
revise when those phases are specced.

### A-4 — TUI / Inner-family reuse is a skeleton, not drop-in
*Targets: Family A, Family I, §6.*
- The research TUI is reusable **as a skeleton** (terminal lifecycle, sync poll loop,
  `spawn_chat_stream` drain, textarea prompt, thread-JSON, confirmation-overlay). Its
  **~24 pollers, slash-command surface, and provenance-gated commit are research-specific.**
  **Prerequisite (Gate B): extract a shared TUI shell** before L-P0, so `inkhaven linguistic`
  is not a fork of `research/app.rs`.
- The left-pane tree (`FactsTree`) roots on a **hardcoded `SYSTEM_TAG_FACTS`**;
  **parameterize by system-tag** (Gate B) before the Language pane reuses it.
- INNER-GROUND-1 has **no source registry**; adding a Language source edits `build_grounding`
  inline + the 6-language `Labels`. Additive; optionally add a registry.

### A-5 — Correct invented UI bindings and a glyph collision
*Targets: Family I keybinding, Family K/Family A glyphs, §6 para-tag picker.*
- **Chord:** RFC's `Ctrl+B J → L` collides — `L` in the Inner Socrates overview is already
  *"view ledger."* Reassign the Inner Linguist to a **free sub-key: propose `→ G`** (Grammar;
  `G` is free). Confirm against `inner_socrates_overview_handle_key` at implementation.
- **Glyph:** RFC's `✎` ("committed to Language book") collides with `INNER_EDITOR_OBSERVATION`.
  Keep `⟐` (deterministic) and `◈` (AI-assisted); **reassign committed to a distinct glyph,
  propose `❖`.** Confirm no collision in the `panes.rs` glyph map.
- **Type picker:** RFC's `Ctrl+B ]` does not exist (`]` = `cycle_editorial_filter`). Set
  `para:ling-*` via the **existing `i`-in-Tree structural picker** (`open_structural_type_picker`),
  a one-row `STRUCTURAL_TYPES`/`LING_TYPES` addition — not a new global chord.

### A-6 — New surfaces (correctly labeled)
*Targets: Family K, §9.*
- **`inkhaven assemble` is a NEW CLI verb.** `assemble_book` exists (called from `build`/TUI),
  but no `assemble` command wraps it. Family K adds a thin verb + `--format` routing over the
  existing template plumbing.
- **`linguistic:` config block is NET-NEW** (idiomatic nested `#[serde(default)]` sub-structs).

### A-7 — Bund policy safety (mandatory)
*Targets: §8, plus a pre-1.7 hygiene fix.*
A `lang.*`/`ink.*` verb **absent from the policy table is silently *allowed*** (uncategorised).
Mandate: **every new verb is registered in BOTH `stdlib/lang.rs` and `policy.rs`**, and a
**coverage test asserts no uncategorised `lang.*`/`ink.*` verb** (Gate A). Related add-a-kind
discipline: a first-class Output category requires all four touch points (`kinds` const,
`message_source` arm, `SOURCES` entry, glyph) — the proptest's `"other"` fallback will **not**
catch a miss (e.g. `RIGOR` is currently half-wired; fix in Gate A).

### A-8 — Split `cli/language.rs` before extending
*Targets: §7 command surface.*
`cli/language.rs` is **9,314 lines with a single flat `match`**. Before adding ~40 arms, split
the handler into per-family submodules (the `LanguageCommand` enum stays; only dispatch moves).
Non-behavioral; do it as the first L-P0 chore.

### A-9 — Static data as bundled files, not Rust literals
*Targets: §5.*
Bake the ~680 KB via `include_str!`/`include_bytes!` of **`assets/linguistic/*` data files**
(precedent: `assets/conlang/english-pool-v1.txt`), parsed at first use — **not** giant `static`
Rust literals (compile-time cost). Size is otherwise fine.

### A-10 — Re-sequence phases by foundation readiness
*Targets: §12 phase map.*
Adopt the five-wave order (§4 above): deterministic work on **solid** foundations first, the
net-new engine later. **1.7.0 = L-P0 + L-P2 (feature-matrix-free subset) + L-P3.** The PHOIBLE
matrix and the typed grammar blocks land in Wave 2 (they gate the deferred metrics and the
engine); L-P5/L-P6b (parser, then tree/movement/binding) are Wave 3. Enabling refactors (A-4,
A-8) precede Wave 1.

### A-11 — Module boundary
*Targets: §12 "src/linguistic/".*
`src/linguistic/` is a **new sibling module that reuses `src/conlang/` primitives**; it does
**not** subsume the ConLang suite. Verb host stays `inkhaven language …` (existing group);
the new TUI is `inkhaven linguistic`.

---

*Amendment status: Proposed. On acceptance, fold A-1…A-11 into RFC LING-1 (reuse map §6, gap
register §11, phase map §12, schema §4, test table §13, glyph/keybinding notes) and mark the
RFC "Final+verification."*
