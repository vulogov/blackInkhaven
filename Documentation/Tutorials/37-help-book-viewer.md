# 37 — The Help book as a rendered-markdown viewer

The Help system book (slug `help`) is a special read-only
book that ships with every Inkhaven project.  Its purpose
is documentation — your own notes, imported tutorials,
quick reference cards — not story content.

In 1.2.8 paragraphs inside the Help book render as
**fully-rendered markdown** instead of source.  Heading
prefixes (`#`), emphasis delimiters (`**`, `*`, `_`), and
link `[brackets](and-urls)` are stripped or styled rather
than shown verbatim, so the pane behaves like a document
viewer: bullets become real `•` markers, code fences get
a string-coloured background, blockquotes are dimmed +
italic.

This tutorial covers what the Help book is for, how
paragraphs land there with the right content type, and
how the rendered viewer differs from the regular source
editor.

## What the Help book is for

The Help book is meant to hold reference material.
Examples:

- A copy of `KEYBINDING.md` so you can search the chord
  table without leaving the TUI (F1's help-query pane
  searches the Help book by default).
- Tutorial markdown you copy-paste from this repo as you
  learn each feature.
- Project-specific runbooks: backup procedure, deploy
  steps, AI prompts that worked well.
- Documentation imported via `inkhaven import-help`.

It is **not** meant to be edited paragraph-by-paragraph
inside the TUI.  Edits to documentation belong in your
text editor of choice, where you can grep, version-
control, and round-trip with the on-disk files.

## How rendered-view detection works

A paragraph displays in rendered mode when BOTH:

1. `read_only == true` — set automatically when the
   paragraph's ancestor chain includes the system `help`
   tag.  Sticky across renames: rename the Help book to
   "Manual" and the rendered view keeps working because
   the tag stays attached.
2. `content_type == "markdown"` — set automatically for
   new paragraphs created under the Help book.

Both conditions together identify the Help book without
false positives — other read-only views (snapshots, diff
panes) keep the existing source view so you can compare
content line-by-line.

## What gets rendered

The pulldown-cmark pipeline that already powers the AI
pane's streamed assistant turns handles the rendering.
Supported markdown:

- **Headings** `# …` through `###### …` — bold, with
  level-appropriate styling.
- **Emphasis** `**bold**`, `*italic*`, `_italic_` —
  modifier-only styled spans.
- **Inline code** `` `like this` `` — different colour.
- **Code fences** ` ```lang ` — multi-line styled block.
- **Lists** — bullets (`- `, `*`, `+`) and numbered
  (`1.`, `1)`) become `•` / `1.` glyphs.  One level of
  nesting cosmetic.
- **Blockquotes** `> …` — dimmed + italic.
- **Links** `[text](url)` — link text inlined.
- **Soft / hard breaks** — preserved.
- **Strikethrough** `~~text~~` — strikethrough modifier.

Out of scope: tables, footnotes, definition lists, HTML
inlines, reference-style links, image embedding beyond
the simple `![alt](path)` form.

## Scrolling the rendered view

Because the rendered view has no cursor, the editor's
usual cursor-tracking scroll behaviour doesn't apply.
Scroll keys adjust the viewport directly:

| Key             | Action                              |
| --------------- | ----------------------------------- |
| `↑` / `↓`       | one line up / down                  |
| `PgUp` / `PgDn` | one visible page up / down          |
| `Home`          | top of the document                 |
| `End`           | bottom (or bottom-clamped scroll)   |
| `←` / `→`       | swallowed — renderer hard-wraps     |
| Mouse wheel     | three lines per tick                |

Any other keypress that would mutate the buffer is gated
by the read-only check at the top of the editor key
handler.  You'll see `Help is read-only` on the status
line.

## Creating a Help paragraph

Standard paragraph creation works:

1. Focus the tree (`Ctrl+2` or `Tab`).
2. Move the cursor to the Help book (or a chapter inside
   it).
3. `Ctrl+B P` opens the add-paragraph modal.
4. Type a title, `Enter`.

The new paragraph lands on disk as `.../<slug>.typ`
(yes — `.typ` extension is shared, but the content type
is markdown).  The template is `# Title\n\n` rather than
Typst's `= Title\n\n`.

## Importing existing documentation

The `inkhaven import-help` CLI subcommand ingests a
directory of markdown files into the Help book wholesale:

```
$ inkhaven import-help /path/to/docs/
```

Files become paragraphs; subdirectories become chapters /
subchapters.  See [tutorial 08](08-importing-existing-docs.md)
for the import flow.

After import, every paragraph in the Help book renders as
markdown — the import path stamps the content type
automatically.

## Differences vs the regular editor

| Aspect              | Regular editor    | Help book viewer       |
| ------------------- | ----------------- | ---------------------- |
| Render path         | Source w/ highlight | pulldown-cmark output |
| Cursor visible      | Yes               | No (read-only)          |
| Editing             | Yes               | No (status warns)       |
| Gutter line numbers | Yes               | No                      |
| Typst diagnostics   | Yes               | No                      |
| Match overlay       | Yes (`Ctrl+F`)    | Yes (search works)      |
| Snapshot            | Yes               | No (no edits to snap)   |
| Tab autocomplete    | Yes               | N/A                     |

`Ctrl+F` search still works in the rendered view — the
substring match runs against the underlying source so you
can find a `# Section` heading by typing `Section`.

## When to use the rendered viewer

The viewer is the right surface for:

- Reading documentation while you work without leaving
  the TUI.
- Running `F1` queries against the Help book — results
  open in the rendered view.
- Linking to a known-good runbook from a paragraph
  (`Ctrl+B P` add-paragraph + paste a paragraph link →
  follow the link → land on a clean rendered page).

If you need to *edit* a Help paragraph, drop to your
external editor (the file is plain markdown on disk under
`books/help/...`).  The next time you open the paragraph
in the TUI, the rendered view shows your changes.

## Disabling markdown rendering for the Help book

There's intentionally no HJSON knob for this.  The
rendered viewer is the entire point of having a special
Help-book content type.  If you want to view the source
of a Help paragraph, open the file in an external editor;
no inkhaven feature mutates Help-book paragraph content,
so disk + viewer stay in sync.
