# Typst overview

Typst is a markup-based typesetter — think LaTeX-strength layout with a
syntax that reads more like Markdown. Inkhaven assembles your book
tree into a directory of `.typ` files and runs `typst compile` over
the root. The reference paragraphs below answer "how do I do X" for
the most common cases you'll hit while writing.

## Document setup
Set defaults that apply to the whole document at the top of
`settings.typ` (inside the Typst system book, per book).

  ```typst
  #set page(paper: "us-letter", margin: 2.5cm)
  #set text(font: "EB Garamond", size: 11pt, lang: "en")
  #set par(justify: true, leading: 0.7em)
  ```

## set vs show
`#set` configures default arguments for a function. `#show` rewrites
how a specific element type renders. Example: number headings only
at level 1 and put a horizontal rule after them:

  ```typst
  #set heading(numbering: "1.")
  #show heading.where(level: 1): it => {
    it
    line(length: 100%)
  }
  ```

## Markup primitives
- `_italic_` — italic emphasis.
- `*bold*` — bold emphasis.
- `=` / `==` / `===` — heading levels 1 / 2 / 3.
- `+` and `-` — numbered / bulleted list items at the start of a line.
- `` `code` `` — inline raw / code.
- `[content]` — a content block (use inside function calls).
- `{ code }` — a code block (no `#` prefix needed inside it).
- `#` — switch from markup to code in markup scope (`#image(...)`).

## Headings
`#heading(level: 1, "Title")` is equivalent to `= Title` at file
scope. Use the function form when you need to set arguments or place
a heading inside another function call.

## Image
`#image("path", width: 80%, alt: "Cover art")` embeds a raster or SVG
file. Path is relative to the file the call lives in. Inkhaven
generates calls like this automatically for Image nodes under
Chapters / Subchapters; use it by hand inside paragraph text when
you want an inline figure.

## Figure
`#figure(image("..."), caption: [Cover by Vladimir])` wraps content
in a numbered, captioned block. Combine with `kind: "figure"`,
`numbering: "1.1"`, and a top-level `#show figure: ...` rule for
custom styling.

## Paragraph layout
`#par(justify: true, leading: 0.7em, first-line-indent: 1.5em)` tunes
paragraph behaviour. Apply globally via `#set par(...)` in
settings.typ.

## Page setup
`#page(paper: "a4", margin: (top: 2.5cm, bottom: 2.5cm))` configures
page geometry. Common papers: `"a4"`, `"a5"`, `"us-letter"`,
`"presentation-16-9"`. Use `#set page(...)` for global config.

## Page break
`#pagebreak()` forces a break. `#pagebreak(weak: true)` only breaks
if the page already had content — useful for "ensure this chapter
starts on a fresh page". `pagebreak(to: "odd")` lands on an odd
page (typical for new chapters in a book).

## Lists
`- item` and `+ item` start bulleted / numbered list items at the
start of a line. Or call `#list[…]`, `#enum[…]`, `#terms((…))`. Nest
by indentation.

## Tables
`#table(columns: 3, [a], [b], [c], [d], [e], [f])` fills a 3-col
grid row-major. `#table.header(repeat: true, [Name], [Age])` marks
header rows that repeat on every page.

## Math
Inline math: `$x^2 + y^2 = z^2$`. Display math: `$ x^2 = y $`
(spaces inside dollars switch to display mode). Common helpers:
`math.frac`, `math.sqrt`, `math.sum`, `math.integral`, `math.lim`,
`math.vec`, `math.mat`.

## References and labels
`#label(<intro>)` attaches a label to the preceding element.
`#ref(<intro>)` produces a clickable reference. `<intro>` is the
short form of `#label("intro")`. Combine with `#cite(<key>)` for
citations and `#bibliography("refs.bib")` to render the entries.

## Footnotes
`#footnote[Some clarifying text.]` puts the note text at the bottom
of the page. Multiple footnotes auto-number.

## Tables of contents
`#outline()` renders a table of contents using all heading-flagged
elements in scope. `#outline(title: "Contents", depth: 3)` limits
depth.

## Spacing
`#h(1em)` / `#v(1em)` insert horizontal / vertical space.
`#h(1fr)` / `#v(1fr)` insert flexible space that fills the line /
page. `#linebreak()` forces a soft line break. `#parbreak()` ends
the current paragraph.

## Colour
`#rgb("#1e1e2e")` / `#rgb(0, 128, 255)` for explicit colours.
Built-ins: `red`, `blue`, `green`, `yellow`, `black`, `white`, etc.
`#luma(50%)` for greyscale. `#gradient.linear(red, blue, angle: 45deg)`
for gradients.

## Boxes and blocks
`#box[…]` is inline (sits within a line). `#block[…]` is block-
level (takes its own line). Both accept layout arguments — width,
height, inset, fill, stroke, radius.

## Alignment
`#align(center)[…]`, `#align(right)[…]`, `#align(top + right)[…]`.
Combine `top` / `bottom` / `horizon` (vertical) with `left` /
`right` / `center` (horizontal).

## Padding
`#pad(left: 2em, right: 2em)[…]` indents content. `#pad(x: 2em)` is
shorthand for left+right; `#pad(y: 1em)` for top+bottom.

## Rotate / scale / move
`#rotate(45deg, [content])` rotates content. `#scale(x: 200%)`
stretches horizontally. `#move(dx: 1em, dy: -.5em, [content])`
translates.

## Counters and state
`#counter("page")` accesses the running page counter. Custom
counter: `#let mycount = counter("mycount")` then `#mycount.step()`
and `#mycount.display()`. `#state("key", initial)` is the mutable
equivalent for non-numeric state.

## Imports and includes
`#import "globals.typ": *` brings all public bindings into scope.
`#include "chapter.typ"` returns the content of another file (used
heavily by inkhaven's assembled `book/index.typ`).

## Variables and let
`#let x = 1` binds a name. `#let title(content) = strong(content)`
binds a function. Bindings are scoped to the surrounding code block.

## Show rules
`#show heading: it => { v(1cm); it; v(0.5cm) }` rewrites heading
rendering. Filter with `.where(...)`: `#show heading.where(level: 1):
...`.

## Functions and arguments
`#fn(positional1, positional2, named: value, body)` calls a function.
Trailing content blocks become the implicit `body` argument:
`#fn(arg)[body]`.

## Bibliography
`#bibliography("refs.bib", style: "chicago-author-date")` renders
the bibliography. Cite from prose with `#cite(<key>)`. Inkhaven
projects can keep `refs.bib` next to `settings.typ` in the
artefacts directory and reference it by relative path.

## Inkhaven assembly
Inkhaven generates a typst tree under
`<parent>/inkhaven-artefacts/<project>/<book>/` every time you press
Ctrl+B A (or B / O). The root `<book-slug>.typ` imports
`globals.typ` and `settings.typ` and calls `wrap_book(include
"book/index.typ")`. Customise the `wrap_*` functions in `globals.typ`
to control layout — they're called automatically for every node in
the tree.

## Inkhaven HJSON-driven settings.typ
`settings.typ` in the artefacts tree has an auto-generated header
synthesised from the `typst_page` / `typst_fonts` / `typst_layout`
stanzas in `inkhaven.hjson`. Change values there — paper, margins,
fonts, line spacing, heading numbering — and re-run Ctrl+B A. The
header is overwritten every assembly so don't edit it by hand. Your
`Typst → <book> → settings.typ` paragraph content is appended below
the header as a free-form override for anything HJSON doesn't
expose.

## Inkhaven wrap functions
- `wrap_book(body)` — wraps the entire book.
- `wrap_chapter(title, body)` — wraps each Chapter; the chapter title is
  passed in as a string.
- `wrap_subchapter(title, body)` — wraps each Subchapter.
- `wrap_paragraph(body)` — wraps each Paragraph (called for every leaf).
- `wrap_image_book(path, title, caption, alt: none)` — book art /
  frontispiece, called for Image nodes directly under Books.
- `wrap_image_chapter(path, title, caption, alt: none)` — chapter art.
- `wrap_image_subchapter(path, title, caption, alt: none)` — section
  art.
- `wrap_image_inline(path, title, caption, alt: none)` — call from
  paragraph prose for inline figures.

## Inkhaven include paths
Inkhaven generates calls that resolve relative to the assembled
file's directory. From `book/<chap>/index.typ` use
`../../globals.typ`; from a paragraph file at the same depth,
`#image("01-cover.png")` resolves to the sibling image.

## Common errors
- `error: file not found ...` — the path in `#image(...)` doesn't
  resolve relative to the calling file. Inkhaven's `Ctrl+B P` picker
  inside `#image("…")` inserts the correct sibling filename.
- `error: unknown variable: wrap_chapter` — `globals.typ` wasn't
  imported, or the chapter index.typ is missing its `#import
  "../../globals.typ": *` line. Re-run Ctrl+B A to regenerate.
- `error: type error: expected content, found none` — typst function
  default values rely on `none` for omitted args; check that the
  argument you're passing is actually content (wrap in `[...]`).
