# Stability pass — H1–H6 + M1–M10

Cycle **1.3.37**. Acting on the pre-1.4.0 audit. Sixteen fixes, grouped into
signed increments; each builds + tests before the next.

## High (work-loss / hang / silent failure / safety)
- **H1** Split-view `Ctrl+S` saves the wrong buffer — route save/quit/crash-mirror
  through the *focused* doc, flush both.
- **H2** Non-atomic multi-store save desyncs body↔meta↔index — reconcile the vector
  index against DuckDB on open (rebuildable cache); document the cross-DB limit.
- **H3** External `typst` compile pipe-fill deadlock — drain stdout/stderr on threads.
- **H4** `recover` / `import-scrivener` exit 0 on total failure — non-zero on errors.
- **H5** A panicking Bund hook crashes the editor — `catch_unwind` around hook eval.
- **H6** `recover` project-less branch skips path validation — reject absolute/`..`.

## Medium
- **M1** `longest_streak` O(D²) — single linear backward pass.
- **M2** Markdown export `#raw(...)` swallows the file — balance-aware fence close.
- **M3** RTF→Typst emits `**bold**` — single `*`.
- **M4** RTF non-UTF-8 rejected in structured path — lossy-decode first.
- **M5** Config overlay replaces security lists wholesale — guard the shell block-list.
- **M6** Render panic on zero-height wrapped pane — clamp scroll before slicing.
- **M7** Non-atomic writes (replace / import / backup / config_tui) — `io_atomic::write`.
- **M8** `usage::record` poisoned-mutex drop + cross-process clobber — recover guard,
  unique temp name.
- **M9** No on-disk schema versioning — `schema_meta(version)` table + open check.
- **M10** Calendar `compose` non-saturating `+` — `saturating_add`.

## Commit grouping
1. H1 (focused-doc save/quit).
2. H4 + H6 (recover/import exit codes + path validation).
3. H5 (hook panic isolation).
4. H3 (typst pipe drain).
5. H2 + M9 (storage: vector reconcile + schema version).
6. M7 + M8 (atomic writes + usage mutex).
7. M2 + M3 + M4 (export/import fidelity).
8. M1 + M5 + M6 + M10 (perf + config merge + render clamp + calendar overflow).
