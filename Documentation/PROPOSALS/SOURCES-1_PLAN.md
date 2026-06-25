# SOURCES-1 — bibliography & citation engine (grounded plan, 1.4.5)

HJSON-authored sources compiled to `.bib` at assembly time; Typst renders the
bibliography. A seventeenth system book — **Sources** — holds citation entries
as structured HJSON paragraphs. `Ctrl+B A` (assembly) compiles them to
`sources.bib` and appends `#bibliography(...)`. `Ctrl+V @` inserts `@key` cite
tokens. `inkhaven sources check` validates scope before compile. **Zero new
runtime crates.** The full RFC is the design intent; this records the grounding
against the real 1.4.5-dev tree + the phase map.

## Grounding — verified against the tree (corrections to the RFC)

| RFC claim | Verified reality | Note |
|---|---|---|
| `SYSTEM_BOOKS: &[(&str,&str)]`, 16 entries | ✓ `store/mod.rs:36`; Notes(0) Research(1) Facts(2) … Intent(15) | The RFC's "Research = index 2 / Facts = index 3" is 1-off; Research is **index 1**, Facts **index 2**. Insert `("sources","Sources")` at array **index 2** (after Research, before Facts). |
| "order-bump machinery handles insertion, no migration" | ✓ `ensure_system_books` (`store/mod.rs:474-518`): on first-time create it bumps every root book with `order >= target_order` up by 1, then creates at the canonical slot. Existing system books keep their `order` otherwise. | Correct. Existing projects get Sources inserted (rest shift down by 1) on first open after upgrade — one-time, idempotent. |
| `#bibliography()` appended near "build_root_typ() … app.rs-adjacent" | ✓ `build_root_typ` is in **`assemble.rs:609`** (not app.rs); it pushes `#wrap_book(include "book/index.typ")` at 623. | Cleaner than stated — all assembly logic is in `assemble.rs`. The bibliography line appends here (or in `assemble_book` after the call). |
| `content_type: "hjson"` free (editor + highlighter + `.hjson` ext) | ✓ `store/node.rs:118,466` | No new content type. |
| `BookRagConfig.exclude_system_books` default to extend | ✓ `config.rs:3345` = `["scripts","prompts","typst","help","intent"]` | Add `"sources"`. |
| `serde-hjson` already present | ✓ `Cargo.toml` | No new crate. |

Everything else (provision_user_book hook, ensure_typst_skeleton pattern,
seed_body_after_create / parent_is_under_threads, Modal::SimilarPicker,
ink_editor_insert, Ctrl+V @ free, the heal-pass) is verified per-phase as it's
consumed.

## Locked decisions (from the RFC, confirmed)

HJSON paragraphs (not a new content type) · scoped collection via `sources.all`
· `sources check` pre-flight · assembly hook in `assemble.rs` · auto-chapter on
new book creation · `Ctrl+V @` cite picker · auto-seed HJSON body under Sources.

## Phase map

- **S-P0 — Foundation (pure).** `("sources","Sources")` at SYSTEM_BOOKS index 2
  + `SYSTEM_TAG_SOURCES`; `src/sources/mod.rs` (`BibEntry` + serde_hjson parse +
  BibTeX serializer + the seed template); `SourcesConfig` in `config.rs`; add
  `"sources"` to the BOOK_RAG exclude default. Unit tests (serializer round-trip,
  empty-key skip, unknown-field tolerance, partial entry, unicode authors).
- **S-P1 — Auto-scaffold + seeding.** `ensure_sources_chapter()` (mirrors
  `ensure_typst_skeleton`) called from `provision_user_book()`;
  `parent_is_under_sources()`; the `seed_body_after_create` HJSON branch +
  placeholder paragraph.
- **S-P2 — Assembly.** `collect_and_emit_sources()` in `assemble.rs` (scoped
  collection, `sources.bib` write); `build_root_typ` appends `#bibliography(...)`
  when entries exist + `auto_bibliography`; `sources.all=false` chapter-match +
  graceful degradation.
- **S-P3 — CLI.** `inkhaven sources check [--book] [--json]` (the `@key` scope
  validator, regex `@([a-zA-Z][a-zA-Z0-9_:-]*)`, exit 1 on missing) · `sources
  list` · `sources import <file.bib>` (handwritten BibTeX extractor → HJSON
  paragraphs).
- **S-P4 — Cite picker TUI.** `Modal::CitePicker` (mirrors `SimilarPicker`);
  `Ctrl+V @` → `open_cite_picker`; fuzzy filter; `Enter` →
  `ink_editor_insert("@<key>")`.
- **S-P5 — Bund + docs + polish.** `ink.sources.list/get/insert/check`;
  CONFIGURATION + KEYBINDING + Tutorial 89. → **cut 1.4.5**.

## Non-goals

Hayagriva `.yaml`, Zotero/Mendeley live sync, AI citation suggestion, CSL
styles, live editor cite-key validation, BibLaTeX beyond `.bib` — all deferred
(SOURCES-2 or out of scope). No new runtime crates; no DuckDB schema changes.

Test baseline 1882 (1.4.4); target ≥ ~1920.
