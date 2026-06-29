# MYTH-1 — Mythological & Symbolic Pattern Library

| | |
|---|---|
| **RFC** | MYTH-1 |
| **Title** | Declared symbolic vocabulary, recurring motifs, archetypal roles: lexicon highlight + density scan + LLM consistency/completeness checks + Inner-family grounding |
| **Status** | Shipped — 1.4.19 |
| **Author** | Vladimir Ulogov |
| **New dependency** | none |

A dedicated home (Mythology system book) for an author's **declared** symbolic vocabulary, recurring
motifs, and archetypal character roles; a lexicon highlight colour that makes symbol density visible
while writing; a deterministic density scan (Thoughts-pane heatmap); explicit LLM consistency/
completeness checks; and grounding for Inner Theologian / Inner Socrates / utopian-architect. Reads
only **declared** vocabulary — never discovers symbols the author didn't name. Read-only w.r.t. the
manuscript; never edits prose.

## Audit corrections (the RFC was written against a partly-fabricated surface)

- **Target "unversioned" → 1.4.19.** 1.4.18 (INNER-THEOLOGIAN-1 + THOUGHTS-1) shipped.
- **"Mythology (13th) system book" is wrong.** `SYSTEM_BOOKS` already holds ~19 entries — Mythology is
  the ~20th. Add one `("mythology", "Mythology")` tuple; `ensure_system_books` seeds it on open.
- **`para:myth-*` does NOT use STRUCT-2's `STRUCTURAL_TYPES`.** WORLD-6's `para:utopia-*` uses a
  **parallel** `UTOPIA_TYPES` registry (STRUCTURAL_TYPES is seed-only). MYTH-1 mirrors that with its own
  `MYTH_TYPES` registry (3 tag/glyph entries) + a `myth_glyph` lookup wired into the tree renderer
  beside `utopia_glyph`/`structural_glyph`.
- **Glyphs.** `⊛` (symbol) `∿` (motif) `⍟` (archetype) — verify none collide with the taken set
  (`⊢⚙⇒∅ ⌨⚠∫≡⊞ ⟡ ✦ ⚖`).
- **Lexicon colours are misstated.** Real: Characters = **amber**, Artefacts = peach (not "Characters =
  cyan"). MYTH-1 adds a 4th source (lavender) to `build_lexicon`/the highlight overlay.
- **DuckDB `UBIGINT`/`TIMESTAMPTZ`/`BOOLEAN` → `TEXT`/`INTEGER`.** Hashes = `DefaultHasher` u64
  stringified; timestamps = RFC3339 TEXT; bools = INTEGER 0/1 (the pattern every per-feature store uses).
- **"continuity character mention index" does not exist.** Archetype-absence scans the Characters
  roster + per-chapter whole-word `mentions()` (reuse the CHAR-1 approach), not a pre-built index.
- **Inner Theologian Source 4 is feasible** — `build_grounding(project_root, book_slug, scope)` genuinely
  has 3 sources + Category-6 fallback today; add a 4th reading `myth.duckdb` symbols with non-empty
  `traditions`. **utopian-architect Source 5** likewise (read-only extension).
- **`myth-reader` persona** = the 16th bundled Inner Socrates persona (after `theatergoer`, the 15th).
- **`Ctrl+V Shift+M`** appears free in `view_sub` (`Ctrl+B Shift+M` = rhythm-rewrite is a different
  layer; `Ctrl+Shift+M` = mouse capture). Confirm before binding.
- **Test baseline 2,131 → 2,133.**

## Phases

| Phase | Content |
|---|---|
| M-P0 | `src/myth/` scaffold + types (MythValence, ArchetypeRole, FindingType, MythSymbol/Motif/Archetype/Finding); `MYTH_TYPES` para registry + `myth_glyph`; `mod` gate |
| M-P1 | `mythology` SYSTEM_BOOKS entry (seeded on open) |
| M-P2 | `myth.duckdb` store (6 tables, TEXT/INTEGER/DefaultHasher) — open/upsert/query/suppress |
| M-P3 | HJSON parsing (symbol / motif / archetype blocks) + read-from-Mythology-book |
| M-P4 | Highlight-vocab rebuild (vocabulary → sorted tokens + bigrams → `myth_highlight_vocab`) |
| M-P5 | Lexicon highlight 4th colour (lavender) wired into the highlight pass + `theme.myth_symbol_highlight` |
| M-P6 | Deterministic symbol-density scan (per chapter, content-hash lazy) + motif explicit-tag collection |
| M-P7 | Deterministic archetype checks (vacancy + roster-scan absence) → findings |
| M-P8 | Thoughts-pane heatmap render + Output `myth` category in the review pass |
| M-P9 | LLM checks: symbol consistency / motif completeness / archetype role (prompt + parse + store) |
| M-P10 | CLI `inkhaven myth scan|check|profile|suppress|refresh` (exit codes) |
| M-P11 | Bund `ink.myth.{symbols,motifs,archetypes,density,findings,suppress}` + policy |
| M-P12 | `Ctrl+V Shift+M` chord (open Mythology book at the nearest declared symbol) |
| M-P13 | `myth-reader` Inner Socrates persona (16th) + grounding data |
| M-P14 | Inner Theologian Source 4 grounding (traditions→lens) + utopian-architect Source 5 |
| M-P15 | `myth:` config block + thread + remove dead-code gate + docs (KEYBINDING/CONFIGURATION/tutorial 102) |

## Out of scope (deferred)

- Automatic symbol discovery (only declared vocabulary is detected).
- Symbol interpretation (measures usage consistency / density only).
- MYTH-2 (cross-volume, needs SERIES-1), MYTH-3 (colour/number entry types), Planning-Board `symbols`
  field, IT traditions auto-suggestion.
