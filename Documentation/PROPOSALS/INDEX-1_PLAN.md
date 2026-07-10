# INDEX-1 — Back-of-book index

| | |
|---|---|
| **RFC** | INDEX-1 |
| **Title** | A curated back-of-book index (terms → locations) |
| **Status** | Proposed — targets a 1.6.x point release |
| **New dependency** | none |
| **Audience** | nonfiction authors |

## The idea

Inkhaven has a word-frequency *concordance* but no real **index** — the alphabetised
list of terms, names, and topics with the places they appear that every serious
nonfiction book carries. INDEX-1 builds one from terms the author already curates.

## Design

### Where the terms come from
- **The Glossary** — every canonical term is an index entry (opt-out `docs.index.from_glossary: false`). Each Glossary *synonym* becomes a *see*-reference to its canonical term.
- **`docs.index.terms`** — an explicit list of extra index terms (names, topics) the author adds in config.

### Where they're found
Walk the manuscript's chapters/subchapters/paragraphs; each is a *unit* with a chapter
title, a section title, an anchor (`file#slug`), and its plain text. A term matches a
unit when it appears as a **whole word** (case-insensitive). Locations are deduplicated
to the chapter (classic index granularity), each keeping a section anchor for the web.

### What it produces
`inkhaven index [--book-name] [--format md|typst|json] [--out <file>]` — an
alphabetised index:

```
Peace ......... The Long Peace, The Numbers
  see also War
War ........... Origins, The Numbers
```

- **`md`** / **`typst`** — a formatted index artefact to drop into a manuscript.
- **`json`** — for tooling.
- **HTML** — `docs.html.include.index` folds an **Index** page into the site, with each
  location a real anchor link to the section (a precise index; the web has anchors, not
  pages).

## Phases

- **P0** — config `docs.index` (`from_glossary`, `terms`) + `docs.html.include.index`.
- **P1** — `src/book_index.rs`: pure `build(terms, units) -> Vec<IndexEntry>` (whole-word
  match, dedupe-by-chapter, alphabetise, `see`-refs). Tested.
- **P2** — CLI `inkhaven index` — gather terms (Glossary + config) + units (manuscript
  walk, `typst_to_plain`), render md/typst/json.
- **P3** — HTML: an Index companion page (anchor links) under `include.index`.
- **P4** — docs (the nonfiction chapter).

## Non-goals (v1)

- True PDF **page numbers** (needs Typst layout + back-references; chapter refs for
  now — the web index is anchor-precise).
- Auto-detected index terms (named-entity extraction) — a later seed.
- Sub-entries beyond `see`-references.
