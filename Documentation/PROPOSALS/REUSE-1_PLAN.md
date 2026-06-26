# REUSE-1 — Reusable content blocks (grounded plan, 1.4.9)

Reusable prose written once as Typst paragraphs in a **Snippets** system book,
referenced anywhere with a standard Typst `#include "../../snippets/<slug>.typ"`.
The assembler emits a `snippets/` sidecar so every include resolves at
`typst compile`; a save-time validator flags broken include paths; `Ctrl+V x`
inserts/replaces an include; `Ctrl+V Shift+X` lists snippets + reference counts.
**No new content type, no new NodeKind, no new DB tables, zero new crates.**

## Grounding — verified against the tree (corrections to the RFC)

| RFC claim | Verified reality | Note |
|---|---|---|
| No release pin | We're on **1.4.9-dev** | Target **1.4.9**. |
| `("snippets","Snippets")` after `("glossary","Glossary")`, 19th | ✓ — Glossary IS now present (TERMS-1, `store/mod.rs`): notes(0) research(1) sources(2) glossary(3) facts(4)… Insert snippets at **index 4** (after glossary, before facts). | `ensure_system_books` order-bump handles upgrades (same as SOURCES-1/TERMS-1). |
| `Ctrl+V x` + `Ctrl+V Shift+X` free | ✓ confirmed against the live layer tables: `x`→OpenConlangHub (1454) and `Shift+x`→FactCheck (1527) are in **meta_sub** (`Ctrl+B`), a different prefix; **view_sub** has neither. `z`/`Shift+z` are TERMS-1's. | Both chords valid. |
| Assembler is `content_type`-blind; a paragraph body with `#include` is copied verbatim | ✓ `write_branch`/`copy_paragraph_file`. The sidecar mirrors **`collect_and_emit_sources`** (SOURCES-1, `assemble.rs:146`) exactly — find book, walk paragraphs, read disk, `strip_leading_heading`, write file. | `emit_snippets_directory` writes `<out_book>/snippets/<slug>.typ`. |
| `refresh_typst_diagnostics_for_opened` save hook; `TypstDiagnostic`; `detect_image_call_context`; `book_of_node`; `hierarchy.ancestors()` | Verify each per-phase as consumed (line numbers drifted across SOURCES-1/AUDIENCE-1/TERMS-1). | The structural templates exist. |
| BOOK_RAG exclude extension | ✓ `exclude_system_books` default currently `[…,"sources","glossary"]`. Add `"snippets"`. | |
| CLI/Bund mirror | ✓ **`cli/terms.rs`** (TERMS-1) is the freshest `list`/`check` mirror; **`scripting/stdlib/terms.rs`** the Bund mirror. | `cli/snippets.rs`, `stdlib/snippets.rs`. |
| Test baseline | **1926** (1.4.8). | Target ≥ ~1960. |

## Locked decisions (from the RFC, confirmed)

Inline Typst `#include` (no transclusion content type) · assembler `snippets/`
sidecar · save-time include validator through the existing diagnostics pipeline
· `Ctrl+V x` context-aware insert/replace · `Ctrl+V Shift+X` overview · paragraph
**tags** for snippet scope (HJSON metadata is REUSE-2) · single-line includes
only (multi-line is REUSE-2).

## Phase map

- **R-P0 — Snippets book + assembler sidecar.** `("snippets","Snippets")` at idx
  4 + `SYSTEM_TAG_SNIPPETS`; `"snippets"` → BOOK_RAG exclude;
  `emit_snippets_directory()` (mirrors `collect_and_emit_sources`) writing
  `<out_book>/snippets/<slug>.typ`; one call in `assemble_book` + `count_work`.
  Tests: empty/absent book → no dir; one/two snippets → files; heading stripped;
  slug from `node.slug`.
- **R-P1 — `#include` path validator.** `check_includes(source, base_dir) ->
  Vec<TypstDiagnostic>` in `typst_check.rs`; appended in
  `refresh_typst_diagnostics_for_opened` (base dir from open-paragraph ancestry);
  Output `typst_include_ok`/`typst_include_missing` with `group_key`; graceful
  Info when no artefacts dir. Tests + proptest (never panics).
- **R-P2 — `Ctrl+V x` insert/replace picker.** `detect_include_context` (mirrors
  `detect_image_call_context`); `IncludeContext` + `SnippetPickerMode` in
  `state.rs`; `Modal::SnippetPicker`; depth-relative path via
  `hierarchy.ancestors()`; Insert = `insert_str`, Replace = select/cut/insert.
  `Action::InsertSnippetInclude` + `entry("x", …, Editor)`. Tests: context
  detection, depth path, Insert/Replace, pre-select.
- **R-P3 — `Ctrl+V Shift+X` overview + CLI + Bund.** `Modal::SnippetsOverview`
  (slug/preview/ref-count) + `entry("Shift+x", …)`; `inkhaven snippets
  list/check`; `ink.snippets.{list,get,check}`.
- **R-P4 — Docs.** KEYBINDING, CONFIGURATION, Tutorial 92. → **cut 1.4.9**.

## Non-goals

Transclusion content type · path rewrite on rename (`snippets check` reports) ·
conditional content / variable substitution (Typst functions cover it) ·
multi-line includes · cross-project snippets. No new crates; no new DuckDB tables.

Test baseline 1926 (1.4.8); target ≥ ~1960.
