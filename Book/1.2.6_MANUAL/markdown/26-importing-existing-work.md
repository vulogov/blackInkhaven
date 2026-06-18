# 26 — Importing existing work

Most writers arrive at inkhaven with manuscripts already underway — a markdown folder, a Scrivener project, a directory of Typst files. Three importers ship with the binary; all are CLI commands that produce a normalised inkhaven tree.

## Markdown / Typst directory tree

`inkhaven import-help` was originally for populating the Help book but works for any directory of `.md` / `.typ` files. Subdirectories become chapters; files become paragraphs:

```
inkhaven import-help \
  --documents-directory ~/Documents/old-book/
```

A directory like:

```
old-book/
├── chapter-01/
│   ├── opening.md
│   ├── conflict.md
│   └── resolution.md
└── chapter-02/
    ├── new-day.md
    └── crisis.md
```

becomes:

```
Help/
├── chapter-01/
│   ├── opening
│   ├── conflict
│   └── resolution
└── chapter-02/
    ├── new-day
    └── crisis
```

The target is the Help book by default (the importer's name gives this away) — re-purpose later with `inkhaven mv` if you want it in a user book.

## The F3 file picker

For one-off imports inside the TUI:

| Chord | What it does |
|-------|--------------|
| F3 (Tree pane) | Open the file picker — navigate to a directory or .md/.typ file. |
| Enter (file) | Import as a paragraph at the cursor. |
| Enter (directory) | Import the tree under cursor. |

Useful when you've got a single file you want to drop into the current chapter without leaving the TUI.

## Scrivener import

`inkhaven import-scrivener` walks a Scrivener `.scriv` package, parses the binder XML, converts every RTF body to Typst markup, and materialises the hierarchy:

```
inkhaven import-scrivener \
  --source ~/Documents/MyBook.scriv

# Dry-run — preview without writing.
inkhaven import-scrivener --source … --dry-run
```

Single-binary — no Scrivener / pandoc / textutil required. Walks the binder structure, maps it to inkhaven nodes per the rules:

| Scrivener | inkhaven |
|-----------|----------|
| DraftFolder | Becomes a user Book. |
| Folder (under Draft) | Becomes a Chapter; nested becomes Subchapter (max one level). |
| Text | Becomes a Paragraph with the converted body. |
| Characters / Places / Notes folders | Mapped to the matching inkhaven system books. |
| Research / Notes top-level | Imported into the Research / Notes system books. |

## Scrivener keywords → tags (1.2.6+)

Scrivener's per-document keywords come across as inkhaven tags. Both shapes handled:

- **Modern Scrivener 3.x** — project-level `<Keywords>` registry + per-item `<KeywordRef ID="N"/>` references.
- **Older / lighter exports** — inline `<MetaData><Keywords>foo, bar; baz</Keywords></MetaData>` with comma / semicolon / newline separators.

Both end up on `Node.tags` after import — source order preserved, case kept, duplicates dropped. Scope: paragraphs only (Scrivener allows keywords on folders too, but inkhaven's tag picker is paragraph-focused).

## Scrivener — what's NOT imported

- **Snapshots** — Scrivener's snapshot history doesn't cross over. The first paragraph version becomes the only one.
- **Section types** — Scrivener "section types" don't map to inkhaven's status ladder; everything imports as Status:None. Run `Ctrl+B R` to set rungs after.
- **Compile groups** — same story; use tags + `--tag` filter on export to recover the slicing.
- **Custom metadata** — `<CustomMeta>` blocks are dropped for now (1.2.8+ may bring a configurable mapping).

## Typst help reference

Inkhaven ships with a curated Typst reference that you can opt into:

```
inkhaven import-typst-help
```

Creates a `Typst reference` chapter in the Help book. F1 RAG against the Help book now answers typst questions from grounded context — useful when you can't remember the syntax for some `#let` form or how to call `#image()`.

## Importing the Book of Inkhaven into Help

Once you've got the markdown mirror of this book (under `Book/markdown/`), import it into the Help book:

```
inkhaven import-help \
  --documents-directory ./Book/markdown/
```

F1 now answers questions about inkhaven itself from this book. End-state docs become end-state RAG.

## Recap

- Three importers: directory tree (`import-help`), Scrivener (`import-scrivener`), bundled Typst help (`import-typst-help`).
- `F3` in Tree pane opens an inline file picker for one-off imports.
- Scrivener keywords import as inkhaven tags automatically (1.2.6+).
- Snapshots, section types, and custom metadata don't cross over.
- Import the book's markdown mirror into Help so F1 covers it.
