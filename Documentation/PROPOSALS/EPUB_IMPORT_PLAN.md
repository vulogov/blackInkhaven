# EPUB import — the marquee, in

Cycle **1.3.37+**. EPUB **export** already ships and is comprehensive
(`src/epub.rs` — standards-compliant EPUB3: OPF, nav, ncx, css, cover, images).
The remaining marquee piece is **import**: read a `.epub` and materialise it as an
inkhaven Book → Chapters → Paragraphs, mirroring the Scrivener importer.

## Premise / reuse
- `zip` (v2) + `quick-xml` (0.36) are already in-tree — no new deps.
- Node creation mirrors `scrivener::import`: `Store::create_node(cfg, &hierarchy,
  kind, title, parent, None, End)` then `io_atomic::write` the body +
  `update_paragraph_content`. `ImportReport{books/chapters/paragraphs_created,
  errors}`.
- Untrusted input → follow the hardening ethos: never-panic, graceful fallback,
  non-zero exit on errors (the H4 lesson).

## Increments
- **P1 — Package parse.** zip → `META-INF/container.xml` → OPF path → parse OPF:
  `EpubPackage{ title, author, spine: Vec<href> (reading order), manifest:
  id→(href, media_type) }`. Pure parser over the zip bytes; never-panic proptests.
- **P2 — XHTML → typst prose.** `xhtml_to_typst(body)` — headings (`=`…), `<p>`,
  `<em>/<i>`→`_`, `<strong>/<b>`→`*`, lists, blockquote, `<br>`, entity decode,
  `<img>`→reference; strip the rest, keep text. The inverse of export's
  `typst_to_xhtml`. Pure fn + never-panic proptest.
- **P3 — Orchestrator.** `import_epub(path, store, cfg, opts) -> ImportReport`:
  one Book (title from `dc:title` / `--book-name`), each spine doc → a Chapter
  (title from its first heading / nav / filename), the converted prose → a
  Paragraph node. Extract manifest images to the project. Per-item errors
  collected, never aborts.
- **P4 — CLI.** `inkhaven import-epub <file.epub> [--book-name] [--dry-run]`,
  mirroring `import-scrivener`; non-zero exit when any item errored.
- **P5 — Docs** (+ optional TUI entry).

## Cut criteria
Each increment signed with tests; round-trips a real exported `.epub` back in
(export → import → compare). Folds into the cycle's release.
