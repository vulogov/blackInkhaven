# The Book of Inkhaven

A companion volume to the inkhaven TUI. Walks every feature
in the order a working author would meet them — install
through scripting — with chord tables, configuration
examples, and a recap per chapter.

Two artefacts share this directory:

| Artefact                              | Purpose |
|---------------------------------------|---------|
| `BOOK_OF_INKHAVEN.typ` → PDF          | The bound book; designed for print. |
| `markdown/*.md`                       | Mirror, one .md per chapter; suitable for `inkhaven import-help` so F1 RAG covers the book. |

## Building the PDF

```
typst compile Book/BOOK_OF_INKHAVEN.typ
```

Output: `Book/BOOK_OF_INKHAVEN.pdf` (~30-40 pages without
screenshots, more once the figures land).

If the host doesn't have the bundled fonts (Linux Libertine
+ Linux Biolinum), typst falls back gracefully — but the
typography is built for those families. Install via your
package manager (`fonts-linuxlibertine` on Debian/Ubuntu;
the corresponding Homebrew formula on macOS) for the
intended look.

## Indexing the markdown into Help

```
inkhaven import-help \
  --documents-directory Book/markdown/
```

F1 RAG against the Help book now answers questions from
this book. Run again after editing.

## Filling in the screenshots

Every `#figure_slot(id: "...")` placeholder in the typst
chapters corresponds to one PNG that goes into
`Book/images/<id>.png`. See `SCREENSHOTS.md` for the full
catalog with capture instructions.

The placeholders compile to grey rectangles labelled
`[ figure: id ]` so you can review the book's structure
before any screenshots exist; dropping a PNG with the
matching id swaps the placeholder for the real image
on the next `typst compile`.

## Directory layout

```
Book/
├── BOOK_OF_INKHAVEN.typ       master typst doc; #include chapters
├── design.typ                  book-author design tokens + helpers
├── chapters/                   33 .typ files (00-prologue → appendix-c)
├── markdown/                   33 .md files mirroring the typst
├── images/                     drop PNGs here (see SCREENSHOTS.md)
├── SCREENSHOTS.md              capture catalog
└── README.md                   this file
```

## Editing

The chapter files are short, focused, and self-contained.
To edit a chapter, find both:

- `chapters/<NN>-<slug>.typ` — the bound version.
- `markdown/<NN>-<slug>.md` — the F1-RAG mirror.

Edit both. They're not auto-synced; the duplication is
intentional so each rendering target can use its native
markup. Chapter additions / re-ordering update
`BOOK_OF_INKHAVEN.typ`'s include list too.

## Versioning

Inkhaven feature version + book version are kept in sync.
`design.typ` carries the version string in `book_version`;
each major feature release bumps both the inkhaven binary
and the book.

| Inkhaven binary | Book version | What's new in the book |
|-----------------|--------------|------------------------|
| 1.2.6           | 1.2.6        | First public edition. Covers 1.2.0 through 1.2.6 fully; 1.2.7 timeline preview chapter. |

## Licence

Same as the inkhaven binary: Apache-2.0 OR MIT, at your
option. The book text + the typst design are both source-
available; you may fork, translate, redistribute.
