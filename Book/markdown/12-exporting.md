# 12 — Exporting your book

Export assembles every paragraph in a user book into one Typst document, then either ships it as-is (`typst` format), compiles it to PDF, or transforms it into Markdown / LaTeX / EPUB.

## From the TUI

| Chord | What it does |
|-------|--------------|
| Ctrl+B B | Build PDF of the current user book. PDF lands in `inkhaven-artefacts/<book-slug>/<book-slug>.pdf`. |
| Ctrl+B O | Build extra formats listed in `output.extra_formats`. |
| Ctrl+B A | Assemble the typst-only source (no compile) into `inkhaven-artefacts/<book-slug>/<book-slug>.typ`. |

`Ctrl+B B` opens a splash with a spinner while typst compiles. Esc cancels. When the engine is `inprocess` (the default since 1.2.5) the spinner stays inside the binary.

## From the CLI

```
inkhaven export typst                    # source-only assembly
inkhaven export pdf                      # full PDF
inkhaven export markdown                 # paragraph→md
inkhaven export tex                      # paragraph→latex
inkhaven export epub                     # paragraph→epub3
```

Multi-book projects need `--book-name`:

```
inkhaven export pdf --book-name "Aerin Saga"
```

## Filter flags

Two predicates ship in the CLI (1.2.4 + 1.2.6):

```
# Only paragraphs at Status:Final or above.
inkhaven export pdf --status final

# Only paragraphs carrying the `draft` tag.
inkhaven export pdf --tag draft

# Combined — both must pass (AND).
inkhaven export pdf --status final --tag draft
```

Status is the workflow ladder from Chapter 9; tag is the project-wide tag set from Chapter 14.

## Configuration — `output.extra_formats`

`Ctrl+B O` walks the list:

```hjson
output: {
  extra_formats: ["markdown", "epub"]
}
```

Each format produces its own artefact in `inkhaven-artefacts/<book-slug>/`.

## Per-paragraph extraction

`Ctrl+V 1` (Editor scope) writes the OPEN paragraph as markdown to the current working directory. `Ctrl+V 2` does the same for the surrounding subchapter. `Ctrl+V 1` from the Tree pane variant writes the cursor's subtree.

Useful for quick "give me just this scene as markdown" exports without invoking the full pipeline.

## EPUB metadata

EPUB writers want title + author at the very minimum. Inkhaven derives:

- **Title** — the book node's title.
- **Author** — `output.epub_author` in HJSON, falling back to the git config's `user.name`.

```hjson
output: {
  epub_author: "Vladimir Ulogov"
}
```

## Where everything lands

By default everything goes under `inkhaven-artefacts/<book-slug>/<book-slug>.<ext>` next to the project. Configurable:

```hjson
artefacts_directory: ""    # empty → sibling
                            # `inkhaven-artefacts/<project>/`
```

The directory is gitignored in the default project template.

## Render preview vs build

`Ctrl+V R` (Chapter 24) is paragraph-level render — quick, no surrounding context. `Ctrl+B B` is the full book — slower but correct. Use the preview for "does this paragraph look right?" and the build for "is this book shippable?".

## Recap

- `Ctrl+B B` builds a PDF; `Ctrl+B O` walks `output.extra_formats`.
- CLI: `inkhaven export <typst|pdf|markdown|tex|epub>` with optional `--book-name`, `--status`, `--tag`.
- Per-paragraph quick export: `Ctrl+V 1` / `Ctrl+V 2`.
- EPUB metadata: `output.epub_author` (falls back to git user).
- Artefacts land in `inkhaven-artefacts/<book>/` by default; configurable.
