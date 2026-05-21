# 15 — Multi-format export: Markdown, TeX, EPUB

Inkhaven 1.2.3 grows the exporter from PDF-only to a family of
formats, all generated in-process from the same combined `.typ`
source. No `pandoc`, no `tex live` install, no external binaries.

| Format    | Backend                          | Notes |
| --------- | -------------------------------- | ----- |
| `typst`   | (none — pass-through)            | The concatenated `.typ` source. |
| `pdf`     | the `typst` CLI on PATH          | Same path as 1.1+. |
| `markdown`| in-process converter             | The Typst subset inkhaven actually emits: headings, lists, emphasis, `#image(…)` with caption. |
| `tex`     | [`tylax`](https://crates.io/crates/tylax) (pure Rust) | Wraps tylax's `typst_document_to_latex`; adds a minimal `\documentclass{book}` preamble when missing. |
| `epub`    | bundled `zip` + `pulldown-cmark` | EPUB3 zip with a single chapter, derived from the markdown intermediate. |

## CLI: `inkhaven export <format>`

The format is the **first positional argument** (defaults to
`typst`). The `--output` flag picks the destination; for `pdf`
and `epub` it is required because both produce binary or
multi-file artefacts that have nowhere sensible to stream.

```sh
# Concatenated Typst (default) — pipes to stdout
inkhaven export typst

# Markdown to stdout
inkhaven export markdown

# Or save it to a file
inkhaven export markdown -o draft.md

# LaTeX via tylax — preamble + body, ready for `xelatex draft.tex`
inkhaven export tex -o draft.tex

# EPUB3 archive
inkhaven export epub -o draft.epub
```

EPUB metadata uses the project directory name as the title; if
you want something nicer, pass `--book-name` (see below).

## Picking one book in a multi-book project

A project can host several user books (`Story`, `Sequel`,
`Workbook`, …). Before 1.2.3 the exporter walked every top-level
node — including the system Help / Typst books — and concatenated
their paragraphs. That was almost always wrong.

1.2.3 changes the default: the exporter scopes to **user books
only**, and when there's more than one it refuses to guess.

```sh
# Single user book → implicit
inkhaven export markdown

# Two or more → must disambiguate
inkhaven export markdown
# inkhaven: store error: export: project has 2 user books —
#   pass --book-name <name>. Available: `Story`, `Workbook`

inkhaven export markdown --book-name "Story"
```

Match is case-insensitive against `Node.title` and falls back to
slug match, so `--book-name story` works too. System books
(Help / Scripts / Typst / Prompts / Places / Characters / Notes /
Artefacts / Research) are excluded everywhere — they're
inkhaven internals, not manuscript content.

## TUI: Ctrl+B O with extra formats

`Ctrl+B O` ("take the book") has always copied the compiled PDF
into the launch cwd with a timestamped filename
(`<book-slug>-YYYYDDMM-HHMM.pdf`). In 1.2.3 it can also produce
markdown / tex / epub side-by-side, gated by `output.extra_formats`
in `inkhaven.hjson`:

```hjson
output: {
  // Generated alongside the PDF on every Ctrl+B O.
  // Unknown entries log a WARN and are skipped.
  // Per-format errors land on the status bar but never abort
  // the take — the PDF the user asked for is already on disk
  // before extras run.
  extra_formats: ["markdown", "tex", "epub"]
}
```

After a successful `Ctrl+B O` you'll see something like:

```
Took the book · /path/story-20262105-1430.pdf · extras:
  story-20262105-1430.md, story-20262105-1430.tex,
  story-20262105-1430.epub  (source PDF: artefacts/story/main.pdf)
```

The same combined-`.typ` source feeds every converter, so the
markdown in the standalone `.md` is byte-identical to the
markdown that ended up inside the `.epub`.

## TUI: Ctrl+V — markdown extraction

`Ctrl+V` is a new third meta prefix (alongside `Ctrl+B` and
`Ctrl+Z`). Two suffix chords cover the "I want a markdown
snippet right now" workflow:

| Focus     | Chord       | Result |
| --------- | ----------- | ------ |
| Editor    | `Ctrl+V` `1`| Markdown of the open paragraph's **buffer** (live in-memory text — unsaved edits flow through). |
| Editor    | `Ctrl+V` `2`| Markdown of the containing subchapter's subtree (falls back to the chapter if no subchapter wraps the paragraph). |
| Tree      | `Ctrl+V` `1`| Markdown of the tree-cursor's node **and all descendants**. |

All three write `<slug>-YYYYDDMM-HHMM.md` into the launch cwd.
The status bar reports the destination on success:

```
view: wrote /Users/you/project-dir/the-storm-20262105-1430.md
```

## Format-specific notes

### Markdown converter (`src/export/markdown.rs`)

Handles the Typst subset inkhaven itself emits:

- `= H1` / `== H2` / … → `#` / `##` / …
- `*bold*` → `**bold**`, `_italic_` → `*italic*`
- Bullet (`- foo`) and ordered (`+ foo` → `1. foo`) lists
- `#image("p.png", caption: "alt")` → `![alt](p.png)`
- Lines starting with `//` (Typst comments) are dropped
- Anything else starting with `#` is preserved verbatim inside
  an inline-code span so you can see what was un-converted

This is **lossy by design**. The goal is "readable plain-text
dump good enough to share / paste / re-format", not round-trip
fidelity.

### tex converter (`src/export/tex.rs`)

Pure delegation to [`tylax`](https://github.com/scipenai/tylax)'s
`typst_document_to_latex`. We add a minimal `\documentclass{book}`
preamble (with `inputenc`, `fontenc`, `graphicx`, `hyperref`) when
tylax's output doesn't already include one, so the resulting
`.tex` compiles standalone under `xelatex` or `pdflatex` without
hand-editing.

If tylax can't translate something it leaves an inline comment in
the LaTeX — we don't second-guess, the file is what it is.

### EPUB writer (`src/export/epub.rs`)

Minimal EPUB3:

- `mimetype` (stored, not deflated — readers reject the archive
  otherwise)
- `META-INF/container.xml`
- `OEBPS/content.opf` — title from `--book-name` or project dir
  name; identifier deterministically derived from title + content
  hash
- `OEBPS/nav.xhtml` — single nav entry
- `OEBPS/chapter.xhtml` — the markdown rendered to XHTML via
  `pulldown-cmark`

Single-chapter layout — if you want a richer EPUB with cover
image + per-chapter splits + cross-references, that's outside
the v1 scope. The output is valid EPUB3 and opens cleanly in
the major readers.

## Troubleshooting

- **`epub export needs --output <path.epub>`** — EPUB is a binary
  zip; there's no streaming-to-stdout shape for it. Pass `-o`.
- **`the typst binary is not on PATH`** — `pdf` export still
  shells out to the `typst` CLI. If you don't have it, use
  `inkhaven export typst -o draft.typ` and feed `draft.typ` into
  whichever `typst` you do have.
- **Extras silently missing after Ctrl+B O** — check the status
  bar for `extra format X error`. The PDF take never aborts on
  extras failures so the message is the only signal.

## See also

- [`09-exporting-to-typst-and-pdf.md`](09-exporting-to-typst-and-pdf.md)
  — the original Typst/PDF workflow.
- [`../CONFIGURATION.md`](../CONFIGURATION.md) — every HJSON field
  including the `output:` stanza.
