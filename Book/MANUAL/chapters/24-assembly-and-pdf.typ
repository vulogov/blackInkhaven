#import "../design.typ": *

#chapter(number: 24, title: "Assembly and PDF")

For twenty-three chapters your book has been a tree — books, chapters,
subchapters, and paragraphs, each a plain `.typ` file that Inkhaven manages,
indexes, and watches. This chapter is where the tree becomes a document. It is
the shortest distance in the whole tool between *what you have written* and
*something you can hold*: two keystrokes to a compiled PDF, three to drop that
PDF in the folder you launched from. Nothing here is a black box. Every file the
process writes is Typst you can open and read, laid out in a directory you can
walk, and the whole of it is regenerated from your tree each time you ask — so
you never have to trust that the output matches the source. It always does,
because it was just built from it.

#section("The two chords, and the third")

Producing a book is a small ladder of three chords, each doing everything the
one before it did and one thing more. You reach them from anywhere — the Tree,
the Editor, it does not matter — because they act on *the book your cursor is
inside*, not on whatever pane has focus.

#chord_table((
  chord_row("Ctrl+B A", "Assemble — turn the tree into a Typst-compilable
    directory under the artefacts folder. No PDF yet, just the source a
    compiler can run."),
  chord_row("Ctrl+B B", "Build — assemble, then run `typst compile` on the
    result. The PDF lands next to the assembled source."),
  chord_row("Ctrl+B O", "Take the book — build, then copy the finished PDF
    into the directory you launched Inkhaven from, timestamped."),
))

Read the ladder from the bottom. `Ctrl+B A` is assembly alone — useful when you
want to inspect the generated Typst, hand it to `typst watch`, or drive the
compile yourself. `Ctrl+B B` is the everyday chord: assemble and compile in one
motion, and if the compile fails, hand the error straight to the AI. `Ctrl+B O`
— *O* for the thing you *take* away — is the finishing move: it does everything
`Ctrl+B B` does and then lifts the PDF out of the ephemeral artefacts cache and
sets it down where your shell can see it.

#callout(label: "Which book?")[
  All three chords act on the *user book* your tree cursor currently sits
  inside. If the cursor is on a system book — Notes, Research, Places, and the
  rest — the chord refuses with a status message asking you to pick a user
  book. System books are author tools; they are never part of a manuscript.
]

Before any of the three does its real work it quietly saves. If the open
paragraph — or the second paragraph in a split edit — has unsaved changes, they
are flushed to disk first, so the thing you compile is the thing on your screen.
A save failure is logged and stamped on the status bar but never aborts the
build; you can always dismiss the splash with `Esc`.

#section("What assembly writes")

Your manuscript lives as `.typ` files, but not in a shape Typst can compile
directly. A paragraph file holds only its prose. A chapter is a *folder*, not a
file, and it carries no text of its own. Nothing anywhere says "include this,
then that, then wrap the whole in a book." Assembly's job is to synthesise all
of that missing structure — the include tree, the wrappers, the page setup —
into a fresh directory, leaving your source tree untouched.

#subsection("Where the output lands")

Assembled books do not go in your project. They go in a per-user cache
directory, outside the tree, so `git status`, your backups, and shell
tab-completion never trip over build intermediates:

#screen(caption: "The artefacts directory for one book")[```
<cache>/inkhaven/artefacts/<project>/<book-slug>/
├── <book-slug>.typ      ← the root; hand THIS to typst
├── globals.typ          ← the wrap_* helper definitions
├── settings.typ         ← page / fonts / layout config
├── sources.bib          ← bibliography (if you cite)
├── snippets/            ← reusable Typst includes
└── book/
    ├── index.typ        ← the book's include list
    ├── 01-prologue/
    │   ├── index.typ    ← this chapter's wrapper
    │   └── 01-opening.typ
    └── 02-arrival/
        ├── index.typ
        └── 01-the-quay.typ
```]

The one file you ever name to a compiler is the root `<book-slug>.typ` at the
top. Everything else it reaches by `#import` and `#include`. The `book/`
subtree mirrors your manuscript's shape one-to-one: every chapter and
subchapter becomes a folder with its own `index.typ`, every prose paragraph
becomes a `.typ` file, ordered by the same `NN-` prefix the Tree pane uses.

The artefacts location is configurable (`artefacts_directory` in
`inkhaven.hjson`); left blank, it defaults to the OS cache path shown above.
When a chord finishes, the status bar prints the absolute root path so you can
copy it straight into a terminal.

#subsection("The include tree and the wrap_ functions")

Assembly does not concatenate your prose into one big file. It builds a *tree*
of tiny files that include one another, and it threads three helper functions
through them — `wrap_book`, `wrap_chapter` (and `wrap_subchapter`), and
`wrap_paragraph`. Those helpers are defined once in `globals.typ`; the generated
`index.typ` files just call them. This is the seam where your typography lives:
change what `wrap_chapter` does, and every chapter in the book changes with it.

The root `<book-slug>.typ` sets the stage and hands the book to its wrapper:

#screen(caption: "The generated root file (abridged)")[```
// Auto-generated by inkhaven Book assembly.
// Book: The Salt Road

#import "globals.typ": *
#import "settings.typ": *

#wrap_book(include "book/index.typ")

#bibliography("sources.bib", style: "chicago-author-date")
```]

That `book/index.typ` is a flat list of the book's top-level children. Because
it is included at file scope — markup mode — every statement carries a leading
`#`:

#screen(caption: "book/index.typ — a BookRoot index")[```
// Auto-generated by inkhaven Book assembly.
#import "../globals.typ": *

#metadata((node_id: "…"))
#include "01-prologue/index.typ"
#include "02-arrival/index.typ"
#wrap_paragraph(include "03-coda.typ")
```]

A chapter's own `index.typ` is different in one important way. Its body sits
*inside* the second argument of `wrap_chapter(...)`, which is code mode — so the
inner calls drop their `#`:

#screen(caption: "A chapter index.typ — code mode inside the wrap")[```
// Auto-generated by inkhaven Book assembly.
#import "../../globals.typ": *

#metadata((node_id: "…"))
#wrap_chapter("Arrival", {
  wrap_paragraph(include "01-the-quay.typ")
  wrap_paragraph(include "02-the-inn.typ")
})
```]

#callout(label: "Why the markup/code split matters")[
  An early bug rendered bare `{ include … }` blocks as literal text in the PDF
  — braces and all — because file-scope statements need the `#` prefix and the
  code inside a `wrap_*` block must *not* have it. The assembler now tracks
  which mode each `index.typ` line is in. You never write these files, but if
  you ever read one and wonder about the inconsistent `#`, that is the reason.
]

Each generated `index.typ` also emits one invisible `#metadata` element
carrying the tree node's id. It contributes nothing to the page — `#metadata`
is query-only — but it lets the PDF-outline machinery correlate a chapter to
the page it starts on. Empty branches get a `[]` placeholder so a childless
chapter compiles instead of producing a parse-failing `wrap_chapter("X", {})`.

#subsection("The three seed files: globals, settings, root setup")

Where do `globals.typ` and `settings.typ` come from? Not from your manuscript —
from the *Typst system book*. Every project has one, and inside it a chapter
named after each user book, seeded with three paragraphs: `globals.typ`,
`settings.typ`, and `index.typ`. Assembly reads those three and maps them into
the output:

- `globals.typ` is copied out verbatim — your `wrap_*` definitions, any custom
  helpers, image search paths.
- `settings.typ` is composed: Inkhaven synthesises a header of `#set page`,
  `#set text`, and `#set par` rules from your HJSON config (more on this below),
  then appends whatever free-form Typst you wrote in that paragraph.
- `index.typ` is *returned* and stitched into the root `<book-slug>.typ`, just
  before `wrap_book`, so any imports or setup you keep there run before the book
  renders.

To edit your book's typography, then, you do not touch the artefacts copies —
they are overwritten every run. You edit the paragraphs under the Typst system
book, and re-assemble. If assembly can't find a Typst chapter named after your
book, it says so and tells you to open the book once to seed it.

#subsection("A clean slate, every run")

Assembly wipes `<artefacts>/<book-slug>/` entirely before it writes a byte.
This is deliberate: a chapter you deleted, a paragraph you renamed, a stale PDF
from last week — none of them should linger to confuse the next `typst compile`.
The cost is that anything you hand-edited in the artefacts tree is gone on the
next `Ctrl+B A`. Treat that directory as disposable output, never as source.

The Timeline chapter and its event paragraphs are skipped at assembly — they are
metadata about the manuscript, not prose, and nothing about them should leak
into the PDF. The same is true across the Markdown, TeX, and EPUB export paths.

#section("Compiling — Ctrl+B B")

`Ctrl+B B` runs assembly and then compiles its output. While `typst` works, a
splash animates with a spinner and an elapsed-seconds counter; you can press
`Esc` to interrupt a stuck compile. On success the status bar reports the PDF's
path.

Inkhaven can drive two compile engines behind one interface. The default is
*external*: it finds `typst` on your `PATH` and spawns it as a child process.
The opt-in alternative is *in-process* (`typst_compile.engine = "inprocess"` in
HJSON), which links the Typst compiler directly and needs no external binary at
all — useful on a machine where you would rather not install one. The splash and
the credits pane both print a one-line engine summary so you always know which
is active and, for the external engine, exactly which binary would run.

#callout(label: "Typst is a separate tool")[
  For the external engine you install `typst` yourself; Inkhaven only shells out
  to it. If it is missing, the compile fails with a clean error pointing at the
  install docs — or at the in-process knob. Assembly (`Ctrl+B A`) never needs
  Typst; only the compile step does.
]

#subsection("When the compile fails")

A typesetting error should not send you hunting through a wall of stderr. When
`typst compile` fails, Inkhaven captures its diagnostics and does two things
without any further keystroke from you.

First it scans the error text for unresolved cross-references — a dangling
`@fig:` or `@eq:` — and promotes each into a precise, actionable finding in the
Output pane, because that is a real defect, not noise.

Then it opens a fresh AI chat. The chat history is cleared so the new context is
clean, the inference mode is forced to Full, and a system prompt tuned for Typst
errors — one that knows Inkhaven's generated file layout — is loaded. The
captured stderr is packaged with the book's name, slug, and root path into a
message that asks for the smallest concrete fix, and it is sent automatically.
Focus jumps to the AI pane, and the answer streams in while you read the error.

#screen(caption: "A compile failure, handed to the assistant")[```
┌─ AI · llama · streaming · typst-error ──────────────┐
│ you  typst compile failed with the following error. │
│      Please diagnose it … --- typst stderr ---       │
│      error: unknown variable: wrap_paragrah          │
│        ┌─ book/02-arrival/index.typ:6:2              │
│ ai   The call on line 6 is misspelled — `wrap_       │
│      paragrah` should be `wrap_paragraph`. But you   │
│      don't edit these generated files; the typo is   │
│      in your globals.typ under the Typst book…       │
└─────────────────────────────────────────────────────┘
```]

Because the assembled tree is kept — not deleted after a failed compile — you
can also open the offending `index.typ` or paragraph file directly at the line
Typst named, see it in context, fix the source in Inkhaven, and rebuild.

#section("Take the book — Ctrl+B O")

`Ctrl+B O` is `Ctrl+B B` plus a delivery step. Once the PDF compiles, it is
copied out of the artefacts cache and into the working directory you launched
Inkhaven from, under a timestamped name:

#screen(caption: "What Take writes to your launch cwd")[```
salt-road-20260805-1432.pdf
```]

The stem is the book slug; the stamp is `YYYYDDMM-HHMM`. The original stays in
the artefacts directory too — Take *copies*, it does not move — so a later
`Ctrl+B Q` imposition pass or a re-take still has the source PDF to work from.
The status bar reports both the delivered path and the source.

Take can also emit *extra formats* alongside the PDF. If you have configured
additional outputs (the `Ctrl+B O` extras knob in HJSON — Markdown, TeX, EPUB),
they are written next to the delivered PDF with the same stem. An extra that
fails is reported on the status bar but never aborts the take: the PDF you
actually asked for is already on disk before the extras run. See Chapter 25 for
the full multi-format story.

#section("Images")

Images are first-class tree nodes, and assembly gives each one the treatment
its position calls for. When `write_branch` meets an Image child it emits a call
to one of three helpers, chosen by the parent's level:

- an image directly under a *Book* becomes `wrap_image_book` — frontispiece and
  book-art treatment;
- under a *Chapter*, `wrap_image_chapter`;
- under a *Subchapter*, `wrap_image_subchapter`.

Each call carries the image's filename, its title, and — as Typst string
literals or the bare keyword `none` — its caption and alt text. As with the
paragraph wrappers, the call is `#`-prefixed at book scope and bare inside a
chapter's code-mode block. You define what each helper *does* — figure frame,
full-bleed, caption styling — in `globals.typ`.

#callout(label: "bdslib is the source of truth for image bytes")[
  The image *bytes* are not copied from the working file under `books/…`. They
  are pulled from Inkhaven's embedded store, which holds the authoritative copy,
  and written into the assembled tree next to the `index.typ` that references
  them. A hand-edit of the on-disk working copy is therefore never accidentally
  re-ingested — the store is what ships into the PDF.
]

Inside the editor, `Ctrl+B P` with the cursor in an `#image("…")` path opens a
sibling picker so you can point a figure at another image in the same folder
without typing the path. Enter on an Image row in the Tree pops a terminal-native
preview (kitty, sixel, iTerm2, or half-block).

#section("Reusable snippets — the REUSE-1 sidecar")

Some Typst you write once and include in many places — a styled warning block, a
recurring table header, a house-style callout. Inkhaven keeps those in the
*Snippets* system book, one paragraph per snippet, and at assembly it copies
each into a `snippets/` sidecar in the output:

#screen(caption: "Snippets copied at assembly")[```
<book-slug>/
├── snippets/
│   ├── warning.typ
│   └── house-callout.typ
└── book/
    └── 01-prologue/
        └── 01-opening.typ   ← #include "…/snippets/warning.typ"
```]

Anywhere in your prose a `#include "…/snippets/<slug>.typ"` then resolves at
compile time. The editor writes those includes for you: `Ctrl+V x` fuzzy-picks a
snippet and inserts the correctly depth-relative path — `../../snippets/…`
computed from the paragraph's place in the tree — or, with the cursor inside an
existing snippet include, replaces the path in place. A save-time validator
flags any include whose snippet slug isn't defined, so a renamed snippet can't
silently break a later build.

The sidecar is written only when the Snippets book has content; a project that
uses none pays nothing, and no `snippets/` directory appears. The output name is
always `<slug>.typ` even when the source snippet was authored as a Jinja
template — so the include resolves regardless of how the snippet was written.

#section("Jinja templates at assembly")

A paragraph can be a *Jinja template* rather than plain Typst (the `⟡` flavour,
added with the `e` key in the Tree). Assembly renders each such paragraph to
Typst before the compiler ever sees it. Every Jinja snippet is registered under
a name derived from its slug path — `snippets/macros/warning.jinja` — so
templates can `{% include %}` one another, and each manuscript template renders
against a context exposing its own title and slug, its enclosing book and
chapter, the project language and genre, and a `linked` map of the HJSON data
from any paragraphs it links to.

By default a template that fails to render aborts the build with a precise error
naming the paragraph. Set `jinja.continue_on_error` and a failure instead writes
a loud red error block into the PDF and moves on, so you can fix a batch of
templates one at a time rather than one-per-rebuild. The full treatment of the
template system — context, filters, the data-linking workflow — is in Chapter 5.

#section("The bibliography and the scholarly apparatus")

If your book cites sources, assembly builds the bibliography for you. It walks
the *Sources* system book, parses each entry, compiles a `sources.bib` file into
the output directory, and — when there are entries and you have not disabled the
auto-line — appends a `#bibliography("sources.bib", style: …)` call to the root
file, after `wrap_book`. Typst then resolves the `@key` citation tokens in your
prose against it. The citation style is configurable; the scope (this book's
sources only, or every entry under Sources) is a one-line HJSON switch.

Three optional apparatus chapters can follow the bibliography, each gated by a
config flag and each rendered only when it has content, in a fixed order:

- an *Index Locorum* — every primary-source locus you cited, resolved to its
  source and validated against that source's reference scheme;
- an *Index Verborum* — the scholarly-lexicon terms the book uses, with their
  original-language forms, senses, and the chapters that use each;
- a *Glossary* — every defined term, alphabetical, with its senses or
  definition.

All three are localised to the project language (Glossary, Glosario, Glossar,
Глоссарий, …). This is a deliberately brief tour; Chapter 23 covers research,
sources, and the apparatus in full.

#section("Templates, front matter, and page config")

The `settings.typ` header is where your page shape, type, and spacing come from,
and you set almost all of it without writing Typst. The `typst_page`,
`typst_fonts`, and `typst_layout` blocks in `inkhaven.hjson` are synthesised at
assembly into `#set page(...)`, `#set text(...)`, and `#set par(...)` rules —
paper size and margins, body and monospace fonts with bundled-font fallbacks,
language, justification, leading, first-line indent, heading numbering. The
generated header is clearly marked *do not edit*; anything you add below the
"user overrides" line in the `settings.typ` paragraph is preserved across
rebuilds.

Separately, a *front-matter* block — a title-page treatment — can be prepended
to the root file just before `wrap_book`, driven by the `frontmatter` config and
the book's title. Books that don't opt in get an unchanged root. Both surfaces
are covered exhaustively in the configuration appendix (Appendix C); this
chapter only notes where in the pipeline they take effect.

#section("The same path from the command line")

Everything the chords do is available headless, for CI, batch builds, and
end-to-end verification that your synthesised `settings.typ` actually compiles.
`inkhaven build` is the CLI mirror of `Ctrl+B A`/`B`:

#screen(caption: "Building without the TUI")[```
$ inkhaven build --book-name "The Salt Road"
Assembling `The Salt Road` (slug: salt-road)…
  [12/44] book/01-prologue/01-opening.typ
Assembly OK · root: …/salt-road/salt-road.typ (44 files)

$ inkhaven build --book-name "The Salt Road" --compile
PDF: …/salt-road/salt-road.pdf
```]

Without `--compile` it stops after writing the artefacts tree; with it, it runs
`typst compile` and prints only the final PDF path to stdout (progress goes to
stderr, so the command is pipe-friendly). The `--book-name` argument is optional
when the project has exactly one user book, required otherwise. The older
`inkhaven export typst` / `export pdf` subcommands remain for concatenated
single-file output and simpler pipelines; the assembly path is the richer one,
and the one the two chords use.

#recap((
  [Three chords climb one ladder: *`Ctrl+B A`* assembles the tree into a
  Typst-compilable directory, *`Ctrl+B B`* also compiles it to PDF, and
  *`Ctrl+B O`* also copies that PDF, timestamped, into your launch directory.],
  [Assembly writes a *fresh, disposable* tree under the artefacts cache — a root
  `<slug>.typ`, `globals.typ`, `settings.typ`, and a `book/` subtree of
  `index.typ` files that call `wrap_book` / `wrap_chapter` / `wrap_paragraph`;
  it wipes the directory every run, so treat it as output, never source.],
  [Typography lives in the *Typst system book* (`globals.typ` / `settings.typ`)
  and in the `typst_page` / `typst_fonts` / `typst_layout` HJSON blocks — edit
  those and re-assemble, never the artefacts copies.],
  [A failed compile *routes its stderr into a fresh, Typst-aware AI chat*
  automatically, and promotes dangling cross-references to Output-pane findings;
  the assembled tree is kept so you can read the offending line in context.],
  [Images ship their bytes from the store and get `wrap_image_*` calls by level;
  *snippets* are copied to a `snippets/` sidecar; *Jinja* paragraphs render to
  Typst; and *Sources* becomes `sources.bib` with an optional index apparatus.],
))
