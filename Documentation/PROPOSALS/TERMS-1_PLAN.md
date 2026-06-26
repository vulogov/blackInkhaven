# TERMS-1 — Terminology governance (grounded plan, 1.4.8)

A **Glossary** system book of HJSON entries (canonical term + definition +
banned synonyms), wired into the style-warning highlight pipeline as a
red-underline overlay; a project-wide `inkhaven terms check`; LLM
canonicalisation suggestions; a `ink.terms.*` Bund API; intent-ledger
suppression. **Zero new crates, no new DB tables.** The full RFC is the design
intent; this records the grounding against the real 1.4.8-dev tree.

## Grounding — verified against the tree (corrections to the RFC)

| RFC claim | Verified reality | Note |
|---|---|---|
| Target **1.4.7** | ✗ 1.4.7 shipped (AUDIENCE-1.1). We're on **1.4.8-dev**. | This is **1.4.8**. |
| Mirror Threads/Planning for the system-book + seed pattern | ✓ — but **SOURCES-1 (1.4.5) is the fresher, closer mirror**: `src/sources/mod.rs` (schema + `from_hjson` + `seed_sources_body_for_tui`), `parent_is_under_sources()` (`app.rs:6432`), the `seed_body_after_create` chain (`app.rs:21505`), `exclude_system_books` already lists `"sources"`. | `glossary.rs`, `parent_is_under_glossary()`, the seed branch all mirror SOURCES-1, not Threads. |
| `SYSTEM_BOOKS` gains `("glossary","Glossary")` after sources, 18th | ✓ `store/mod.rs:36`; `("sources","Sources")` at array index 2. Insert glossary at **index 3** (after sources, before facts). | `ensure_system_books` order-bump handles existing projects (same as SOURCES-1). |
| `Ctrl+V z` free | ✓ — but the RFC's free-list is stale: of `j`/`v`/`x`/`z`, only **`z`** is still free (`j`=InnerSocrates, `v`=Credits, `x`=ConlangHub, `@`=CitePicker were all taken since 1.4.3). Selection holds. | `entry("z", Action::ViewToggleTermsOverlay, Scope::Any)`. |
| `StyleWarningKind` has 5 variants | ✓ FilterWord, RepeatedPhrase, ShowDontTell, Echo, Anachronism (`style_warnings.rs:46`). Add `BannedSynonym` = 6th. | One line. |
| `AnachronismDetector` is the structural template | ✓ `style_warnings.rs:664`; `new(cfg)`, `is_empty()`, `detect()` with byte→char map + `unicode_word_indices()`. **It is single-word only.** | `BannedSynonymDetector::detect` extends to **multi-word** synonyms (1–3-gram sliding window) — the real added complexity. |
| theme `style_warning_*_fg` + `_modifier` | ✓ `theme.rs:36+`/`83+`. Add `banned_synonym` fg (red) + modifier (underline). | `highlight.rs:369 style_warning_style_at` adds one arm. |
| Glossary excluded from BOOK_RAG | ✓ `exclude_system_books` default (`config.rs:3357`) currently `[…,"sources"]`. Add `"glossary"`. | |
| Intent ledger: `list_intent_rows_raw` / `add_intent` | Verify exact method names in `inner_socrates/storage.rs` at T-P4 (Inner Editor uses them). | Read-for-suppression + write-for-declare. |
| Test baseline 1879 (1.4.3) | ✗ Now **1910** (1.4.7). | Target ≥ ~1945. |

## Locked decisions (from the RFC, confirmed)

Route banned synonyms through **style_warnings, not the lexicon** (the synonym
is flagged, not the canonical — inverse of lexicon semantics) · HJSON entries
in a system book (no new content type) · self-gating empty detector · red
underline · `Ctrl+V z` toggle · intent-ledger suppression via the existing
`coverage` string column (no new table/enum).

## Phase map

- **T-P0 — Foundation.** `("glossary","Glossary")` at SYSTEM_BOOKS idx 3 +
  `SYSTEM_TAG_GLOSSARY`; `"glossary"` → BOOK_RAG exclude; `src/glossary.rs`
  (`GlossaryEntry` + `parse_glossary_entry` + `glossary_entries_from_store` +
  `GLOSSARY_TEMPLATE` + `seed_glossary_body_for_tui`); `parent_is_under_glossary()`
  + seed branch in `app.rs`. Tests: full parse, no-synonyms entry valid,
  unknown-field tolerated, scope filter (global/match/non-match), empty book.
- **T-P1 — Detector + overlay.** `StyleWarningKind::BannedSynonym`;
  `BannedSynonymDetector` (`from_store` → synonym_lc→canonical map + intent
  suppression set; `detect` with 1–3-gram sliding window); theme fields;
  `highlight.rs` arm; `panes.rs` detector block (primary + split); `Ctrl+V z`
  toggle + `terms_overlay_toggle` field; cursor-on-hit footer hint. Tests:
  empty fast-path, synonym flagged / canonical not, multi-word + Cyrillic,
  scope filter, intent suppression.
- **T-P2 — CLI `terms check`.** `TermsCommand` + `cli/terms.rs`: walk user-book
  paragraphs, run detector per line, report `chapter/paragraph line N: "syn" →
  use "canonical"`; `--book` scope, `--json` sidecar, exit 1 on findings.
- **T-P3 — `terms suggest` (LLM).** concordance-cluster the target book → prompt
  for HJSON GlossaryEntry blocks → stream to Output; `--auto-create` drafts
  them under Glossary.
- **T-P4 — Intent declare + Bund.** `ink.terms.declare_intent/list/get/check`;
  TUI declare on a hit. Suppression via `coverage:["banned_synonym"]`.
- **T-P5 — Story bible + docs → cut 1.4.8.** GLOSSARY bible section;
  CONFIGURATION + KEYBINDING; Tutorial 91.

## Non-goals

Positive (green) highlighting of the correct term · phrases > 3 words (TERMS-2)
· build rejection (the `--json` exit-1 covers CI) · style-guide import · DITA
typing. No new runtime crates; no new DuckDB tables.

Test baseline 1910 (1.4.7); target ≥ ~1945.
