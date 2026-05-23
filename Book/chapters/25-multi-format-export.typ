#import "../design.typ": *

#chapter(number: 25, part: "Part VII — Typst Mastery",
  title: "Multi-format export")

#dropcap("E")xport beyond PDF: the inkhaven binary ships
converters for Markdown, LaTeX, and EPUB. The same
filter flags (`--status`, `--tag`, `--book-name`) work
across all formats.

#section("The five formats")

#chord_table((
  chord_row("typst", "Source-only assembly. The combined .typ file inkhaven would feed to the compiler."),
  chord_row("pdf", "Full PDF via typst-pdf."),
  chord_row("markdown", "Typst → Markdown. CommonMark-ish output, retains code blocks + math."),
  chord_row("tex", "Typst → LaTeX via the bundled `tylax` crate. Useful for academic submission."),
  chord_row("epub", "Typst → Markdown → EPUB3. The bundle uses your title + author metadata."),
))

#section("From the CLI")

```
inkhaven export typst                       # source-only
inkhaven export pdf
inkhaven export markdown
inkhaven export tex
inkhaven export epub

# Multi-book project — pick one book.
inkhaven export pdf --book-name "Aerin Saga"

# Filter: only Status:Final or above.
inkhaven export pdf --status final

# Filter: only paragraphs tagged `draft`.
inkhaven export pdf --tag draft

# Combined — AND.
inkhaven export pdf --status final --tag draft
```

#section("Ctrl+B O — extra formats from inside the TUI")

Configure your common formats once:

```hjson
output: {
  extra_formats: ["markdown", "epub", "tex"]
}
```

`Ctrl+B O` walks the list and writes each artefact to
`inkhaven-artefacts/<book-slug>/`. Useful as a "publish my
draft" one-keystroke loop.

#figure_slot(
  id: "ctrl-b-o-extra-formats",
  caption: "Ctrl+B O — splash showing each format being built, one at a time. Esc cancels (already-built formats survive).",
  height: 40mm,
)

#section("Per-paragraph quick extracts (`Ctrl+V 1/2`)")

`Ctrl+V 1` (Editor scope) writes the OPEN paragraph as
markdown to the current working directory. `Ctrl+V 2` does
the same for the surrounding subchapter. `Ctrl+V 1` from
the Tree pane writes the cursor's subtree.

Useful for "give me just this scene as markdown" without
going through the full pipeline.

#section("EPUB notes")

EPUB readers need a title + author at minimum:

```hjson
output: {
  epub_author: "Vladimir Ulogov"
}
```

Falls back to `git config user.name` when unset. The book
title comes from the user book's title.

Cover image: if `books/<book-slug>/cover.png` exists, it
gets used as the EPUB cover. Otherwise the EPUB ships
coverless.

#section("LaTeX notes")

LaTeX export uses `tylax` — a pure-Rust Typst→LaTeX
converter. Most prose round-trips cleanly; complex Typst
constructs (custom math environments, advanced layout
functions) translate to escape hatches and may need hand
editing.

The `tex` output is one file: `<book-slug>.tex`. Useful for
academic journals that require LaTeX submission.

#section("Markdown round-trips")

The Typst→Markdown converter handles:

- Headings (`= H` → `# H`)
- Bold / italic (`*b*` / `_i_` → `**b**` / `*i*`)
- Lists (numbered + bulleted)
- Code blocks (preserved)
- Footnotes
- Links

Things it doesn't preserve: typst-specific layout
(margins, breaks), custom function calls, math typesetting
(simplified to `$ \text{like this} $`).

#section("Filter combinations")

The two predicates compose:

```bash
# Only Status:Ready + tagged `submission` + the Aerin Saga book.
inkhaven export pdf \
  --book-name "Aerin Saga" \
  --status ready \
  --tag submission
```

A paragraph must pass ALL active predicates. Useful for
shipping precisely-scoped slices to specific readers.

#recap((
  [`inkhaven export <typst|pdf|markdown|tex|epub>` — five formats.],
  [Filters: `--book-name`, `--status`, `--tag` — combine with AND.],
  [`Ctrl+B O` walks `output.extra_formats` in HJSON.],
  [`Ctrl+V 1` / `Ctrl+V 2` — per-paragraph / per-subchapter markdown extracts.],
  [EPUB needs `output.epub_author`; falls back to git config.],
))
