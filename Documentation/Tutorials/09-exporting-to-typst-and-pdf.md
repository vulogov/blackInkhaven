# 9 — Exporting to Typst and PDF

Your manuscript is already a set of `.typ` files on disk. To produce
a single combined file or a PDF, use `inkhaven export`. This
tutorial covers both targets, the depth-first ordering, the role of
a book-level configuration paragraph, and what Typst is doing for you.

## A note on Typst

[Typst](https://typst.app/) is a modern typesetting language — a
spiritual successor to LaTeX with a friendlier syntax and a faster
compiler. Your paragraphs are plain Typst markup:

- `= Heading` (level 1)
- `== Subheading` (level 2)
- `*bold*`, `_italic_`, `` `code` ``
- `#link("url")[text]`, `#figure(image("foo.png"))`, `#cite(<key>)`
- Inline expressions: `#calc.round(3.14)`

You do not have to know much Typst to write prose in Inkhaven — the
`= Heading` line that `add paragraph` inserts is the only required
syntax. The rest is plain text. When you eventually want polished
output, you reach for Typst features (page setup, footnotes,
bibliographies).

Typst-the-binary is a separate tool you install yourself (see
[`../FIRST_STEPS.md`](../FIRST_STEPS.md)). Inkhaven only needs it
for the PDF export path.

## Export to a single Typst file

```bash
$ inkhaven --project ~/Books/my-novel export typst -o my-novel.typ
```

Or print to stdout:

```bash
$ inkhaven --project ~/Books/my-novel export typst > my-novel.typ
```

What this does:

- Walks the hierarchy depth-first in `order` order.
- For each **paragraph** node, reads its `.typ` file content and
  appends it to the output (separator newlines between paragraphs).
- Branch nodes (Book, Chapter, Subchapter) emit nothing themselves
  — their **paragraphs** carry the headings (`= Title` for level 1,
  `== Title` for level 2, etc., depending on how you write them).

So if your project is:

```
My Novel
├── Preface
├── Chapter One
│   ├── Chapter One Intro
│   ├── Morning
│   │   ├── Opening Scene
│   │   └── Storm Breaks
│   └── Afternoon Intro
└── Chapter Two
    └── Reunion
```

…the export concatenates:

```
[Preface contents]

[Chapter One Intro contents]

[Opening Scene contents]

[Storm Breaks contents]

[Afternoon Intro contents]

[Reunion contents]
```

If "Chapter One Intro" starts with `= Chapter One`, the rendered PDF
has a level-1 heading at that point.

### What about the system books?

The exporter walks **only user books** — Notes, Research, Prompts,
Places, Characters, Help are skipped (they are author tools, not
manuscript content). Anything you write in those books stays out of
the export.

If you do want to export a system book's content (rare — usually
Research notes are private), you can manually concatenate via the
CLI:

```bash
# Export just the Research book contents
$ inkhaven --project ~/Books/my-novel export typst | \
    grep -A 9999 "Research" > research-only.typ
```

That's a hack. A cleaner future option would be `--include
research-only`; for now the manual route works.

## Book-level Typst configuration

To set page size, fonts, margins, table of contents, etc., add a
**first paragraph directly under your book** with the Typst
configuration. Inkhaven calls this the "book config paragraph" but
it is just a regular paragraph at the right position.

Example: create a paragraph titled `Book setup` as the **very first
child of your book**. Its content might be:

```typst
#set document(
  title: "My Novel",
  author: "You",
)

#set page(
  paper: "a5",
  margin: (x: 2.5cm, y: 2.5cm),
)

#set text(
  font: "Linux Libertine",
  size: 11pt,
  lang: "en",
)

#set par(
  justify: true,
  leading: 0.65em,
  first-line-indent: 1.5em,
)

#show heading.where(level: 1): set heading(numbering: "1.")

#outline()
#pagebreak()
```

In depth-first export order this paragraph sorts first, so its
configuration is in effect for everything that follows. Set
`order: 0` (the default for the first child) and you're done.

If you don't add a config paragraph, the export still works — Typst
uses default page setup. Good for quick PDF previews; less good for
final manuscripts.

## Export to PDF

```bash
$ inkhaven --project ~/Books/my-novel export pdf -o my-novel.pdf
```

What this does:

1. Builds the combined `.typ` file (same as `export typst`).
2. Writes the intermediate `.typ` next to the target PDF
   (`my-novel.typ`) for inspection.
3. Shells out to `typst compile <combined>.typ <output>.pdf`.
4. Reports success or the typst error.

Requires `typst` on your PATH. If Typst is not installed, the export
fails with a clean error; install Typst and retry.

The intermediate `.typ` is kept on purpose — when Typst reports a
syntax error at "line N", you can open the combined file in any editor
and see the offending line in context (it usually came from one of
your paragraphs).

## Workflow patterns

### Fast preview

Add a Makefile target:

```makefile
preview:
	inkhaven export pdf -o /tmp/preview.pdf
	open /tmp/preview.pdf
```

`make preview` builds and opens in your system PDF viewer. Quick
iterate.

### Continuous build

Use [typst's watch mode](https://github.com/typst/typst) on the
exported `.typ`:

```bash
$ inkhaven export typst -o my-novel.typ
$ typst watch my-novel.typ my-novel.pdf
```

But: this only re-renders when `my-novel.typ` changes. After editing
in Inkhaven you need to re-run `export typst`. A wrapper script:

```bash
#!/usr/bin/env bash
while true; do
  inkhaven export typst -o my-novel.typ
  sleep 5
done
```

…re-exports every 5 seconds and Typst's watcher does the rest.

### Per-chapter builds

You can export a single chapter by manually concatenating the
`.typ` files for that chapter's paragraphs:

```bash
$ cat books/my-novel/01-chapter-one/*/*.typ > chapter-one.typ
$ typst compile chapter-one.typ
```

Or run `inkhaven list` to get the file paths and select the ones you
want.

### Bibliographies and citations

Add a bibliography paragraph somewhere in your manuscript:

```typst
#bibliography("refs.bib", style: "chicago-author-date")
```

…and refer to it elsewhere with `@key`. Put `refs.bib` next to the
exported `.typ` and Typst handles the rest.

## Troubleshooting

### "command not found: typst"

Install Typst from
[https://github.com/typst/typst#installation](https://github.com/typst/typst#installation)
and confirm `typst --version` works.

### Typst compile error

The intermediate `.typ` is preserved next to the PDF. Open it, find
the offending line, identify which paragraph it came from. Fix in
Inkhaven, re-export.

### Wrong order in the export

The order is determined by each node's `order` integer. Reorder in
the Tree pane with `U` / `J` (or `Ctrl+B ↑` / `↓`). The on-disk
filenames (`NN-…`) follow the order automatically.

### Headings show up at the wrong level

`= Heading` is always level 1 in Typst. If you want level-2 headings
for chapter intros, write `== Chapter One` in the relevant paragraph
body. Inkhaven doesn't auto-promote/demote headings — they're literal
markup.

### A paragraph contains plain prose with no heading

That's fine. It exports as plain paragraph text (no heading). Useful
for chapter-spanning prose blocks or "between scenes" interludes.

## What you have learned

- `inkhaven export typst` concatenates every paragraph in depth-first
  order; `-o file.typ` writes a file, no arg prints to stdout.
- `inkhaven export pdf -o file.pdf` builds the combined Typst and runs
  `typst compile`. Requires Typst on PATH.
- System books are excluded from the export.
- A book-level Typst configuration paragraph at the top of the book
  sets page setup, fonts, ToC, etc. for everything that follows.
- Reorder paragraphs with U / J in the Tree pane to change export
  order.
- Typst is the typesetting tool; Inkhaven is the editor. Install Typst
  separately for PDF output.

## Next steps

- [`10-backups-and-recovery.md`](10-backups-and-recovery.md) — keeping
  the project safe.
- [`11-theming.md`](11-theming.md) — making the TUI yours.
