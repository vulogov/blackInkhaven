#import "../design.typ": *

#chapter(number: 25, title: "EPUB, Web and More")

The last chapter took your manuscript to the printed page — Typst to PDF, the
destination a book has had since the first press. This chapter is about every
*other* place the same words can go: an EPUB a reader opens on a phone, a
website a reader opens in a browser, a Word document an agent opens on a
submission desk, a LaTeX bundle a server compiles for a preprint archive, and
an audiobook a reader listens to on a walk. Inkhaven treats all of these as
*exports* — one manuscript, many destinations — and the point of this chapter
is that reaching any of them is a single command with a small, shared set of
flags. Learn the shape once and every format is a variation on it.

#section("One manuscript, many destinations")

Almost every export in Inkhaven starts from the *same assembled source*. The
tool walks your book in tree order — depth-first, top to bottom — and
concatenates each paragraph's `.typ` file into one long document, exactly the
way it does for the PDF build. Branch nodes (books, chapters, subchapters)
contribute nothing themselves; the headings live in the paragraphs, written by
the `= Title` line each paragraph carries. The Timeline chapter and any event
paragraphs are skipped, because a timeline is metadata about the manuscript,
not manuscript prose. Whatever comes out of that walk is what every exporter
converts.

Because the source is shared, so are the *filters* that decide which paragraphs
reach it. The same four flags apply to every format the `inkhaven export`
command produces, and to the reader-facing exports besides.

#term("The export flags")[
  `--book-name <name>` picks *which* user book to export (a project can hold
  several); it matches a book's title or slug, case-insensitively, and is
  required only when the project has more than one user book. `--status
  <rung>` keeps only paragraphs at or above a rung on the workflow ladder
  (`napkin → first → second → third → final → ready`). `--tag <tag>` keeps
  only paragraphs carrying that tag. `--profile <dim>=<value>` selects one arm
  of conditional (single-sourced) content. They *combine*: a paragraph must
  pass all of them to ship.
]

The general form is `inkhaven export <format>`, where the format is one of
`typst` (the default — the raw concatenated source), `pdf`, `markdown`, `tex`,
`epub`, or `html`. Everything else in this chapter is either a richer variant of
one of these (the full `inkhaven epub`, the arXiv `--bundle`) or a destination
with its own top-level command (`inkhaven docx`, `inkhaven audiobook`).

#screen(caption: "The export command and its shared flags")[```
$ inkhaven export markdown -o draft.md
wrote draft.md (markdown)

$ inkhaven export epub --book-name "Harbor" \
    --status final -o harbor.epub
wrote harbor.epub (epub)

$ inkhaven export html -o site/ --templates my-theme/
wrote HTML site to site/
```]

#callout(label: "Two books, one flag")[
  If a project holds only one user book, `--book-name` is optional and that
  book is used. The moment you add a second, every export refuses until you
  name one — and the error lists the books it found, so you never have to guess
  the spelling. System books (Help, Prompts, Places, Characters, Facts, Notes,
  and the rest) are never candidates; they are Inkhaven's internals, not your
  manuscript.
]

#section("EPUB — the e-book readers actually open")

An EPUB is the format e-readers consume: a zip of XHTML pages with a spine that
orders them and a navigation document that indexes them. Inkhaven writes one
directly — no external converter, no `pandoc` — because it already holds your
prose as Typst and a `zip` library in the binary. There are two ways to reach
it, and the difference matters.

The full path is the dedicated command:

#screen(caption: "Exporting a complete EPUB 3")[```
$ inkhaven epub --book-name "The Harbor Code" \
    -o harbor.epub
  embedding cover (image/jpeg, 184 KB)
Exporting `The Harbor Code` → EPUB (14 chapters)…
EPUB: harbor.epub (14 chapters · 612 KB)
```]

`inkhaven epub` builds a standards-compliant EPUB 3 the way a reader expects it:
*one spine document per Chapter node*, in display order, each holding that
chapter's descendant paragraphs converted to XHTML, with any subchapter titles
surfaced as `<h2>` headings inside the flow. The archive it produces has the
shape the EPUB spec demands — the `mimetype` entry first and uncompressed, a
`META-INF/container.xml`, a package document listing every chapter in the
manifest and spine, an EPUB 3 `nav.xhtml` plus an EPUB 2 `toc.ncx` for older
readers, a stylesheet, and `chapter-001.xhtml`, `chapter-002.xhtml`, and so on.

#screen(caption: "Inside the .epub archive")[```
mimetype                 (stored, first — the EPUB rule)
META-INF/container.xml
OEBPS/content.opf        (metadata + manifest + spine)
OEBPS/nav.xhtml          (EPUB 3 navigation)
OEBPS/toc.ncx            (EPUB 2 back-compat)
OEBPS/style.css
OEBPS/cover.jpg          (if a cover was found)
OEBPS/cover.xhtml
OEBPS/chapter-001.xhtml
OEBPS/chapter-002.xhtml
...
```]

The metadata is filled in for you and overridable. The title defaults to the
book's title (`--title` overrides); the author to your `editor.comment_author`
config, falling back to "Unknown Author" (`--author` overrides); the language
to the ISO code mapped from the project's `language` field; and the identifier
to a fresh `urn:uuid`, stable within one export so a re-export replaces cleanly
in a reader's library. Without `--output`, the file lands at
`<project>/<book-slug>.epub`.

#subsection("What the converter understands")

The Typst-to-XHTML pass handles the markup Inkhaven prose actually uses, and is
honest about the rest. Headings (`=`, `==`, `===`) become `<h1>`/`<h2>`/`<h3>`;
`_emphasis_` becomes `<em>`; `*strong*` becomes `<strong>`; blank-line-separated
blocks become `<p>` paragraphs. Each paragraph node's leading `= title` line is
*organisational scaffolding* — the scene label you see in the Tree — so it is
stripped, and the reader sees flowing chapter prose rather than "001. Approach"
labels. XML special characters are escaped, and an unpaired `_` (a filename, a
stray underscore) passes through literally rather than opening an
unterminated tag.

Footnotes get special care. A `#footnote[…]` in your prose becomes a proper
EPUB 3 *popup footnote*: a numbered reference anchor in the flow, and a
collected `<aside>` section at the chapter's end with a backlink. Apple Books
and similar readers render these as tap-to-pop popups; readers that do not
simply show the notes at the foot of the chapter.

#subsection("Covers and inline images")

Drop a `cover.jpg` or `cover.png` in the project root and `inkhaven epub`
finds it, embeds it as the library thumbnail, and makes it the opening page —
the console line confirms the format and size when it does. Images referenced
inline in a chapter's prose are carried into the archive too, written verbatim
(already-compressed formats are stored, not re-deflated) and declared in the
package manifest so they render where the prose places them.

#callout(label: "Two EPUB paths, one to prefer")[
  `inkhaven export epub` is the *minimal* writer: it runs the prose through the
  Typst-to-Markdown converter and packs the result as a single-chapter EPUB —
  handy when you want the `.epub` and the `.md` to match byte for byte, and it
  is the variant wired into the batch-export flow. For a real e-book — chapters
  as separate spine documents, a cover, inline images, popup footnotes — use the
  dedicated `inkhaven epub` command. When in doubt, that is the one you want.
]

#section("Importing an EPUB")

The inverse of the export is `inkhaven import-epub <file.epub>`, which turns an
existing e-book into a user book you can edit. It parses the package, then
materialises a *Book*, a *Chapter* per spine document, and one *Paragraph* per
chapter holding that chapter's prose converted back from XHTML to Typst. Images
referenced by a chapter become Image nodes under it; manifest images that no
chapter references (cover art, orphans) are extracted to a
`<book-slug>-images/` sidecar folder for you to place by hand. Image references
in the prose are rewritten to Typst comments so an imported chapter still
compiles cleanly while telling you where the picture belonged.

#screen(caption: "Round-tripping a book back in")[```
$ inkhaven import-epub harbor.epub
EPUB import complete — book `The Harbor Code`:
  chapters:   14
  paragraphs: 14
  images:     6 imported, 1 extracted
  author:     A. Writer  (set your book metadata …)
  (1 unreferenced image(s) extracted to
   `the-harbor-code-images/` for hand-placement)
```]

Two options shape the import. `--book-name` overrides the title of the created
book (otherwise it takes the EPUB's `dc:title`, falling back to "Imported
EPUB"); `--dry-run` reports what *would* be created — chapter, paragraph, and
image counts — without writing anything. The import never aborts on a single
bad chapter: per-item failures are collected and printed, and a real (non-dry)
run that hit any failure exits non-zero, so a scripted import can tell a clean
run from a partial one.

#section("A website from your book")

`inkhaven export html --output <dir>` renders your book as a *self-contained
static website* — one HTML page per chapter, an index page from the book's
front matter, a navigation list linking them all, and a client-side search box,
all written into the directory you name. It is the third consumer of the same
node tree, localised to the book's language, with images copied alongside and
the profile and variable machinery applied exactly as the other exports apply
them.

#screen(caption: "Building the site")[```
$ inkhaven export html -o public/
wrote HTML site to public/

$ ls public/
index.html          chapter-01.html   theme.css
appendix-index.html chapter-02.html   search.js
search-index.js     ...
```]

The look is driven by *overridable templates*. Inkhaven ships a default set
split into two concerns — the `functional/` machinery (the page skeleton,
the nav, the search wiring) and the `theme/` look (the CSS, the visual
choices) — and you replace either by pointing `--templates <dir>` at your own,
or by setting `docs.html.template_dir` in config. A file you supply overrides
the bundled default of the same name; anything you leave out falls back to the
default, so a theme can be as small as one stylesheet. To start from the
built-ins rather than a blank page, scaffold them:

#screen(caption: "Scaffolding the default templates to edit")[```
$ inkhaven export html --eject-templates my-theme/
wrote default HTML templates to my-theme/ — edit
them and export with --templates my-theme/
```]

The `docs.html` config block governs the rest: `site_title` overrides the page
title, `search` toggles the search box, and an `include` table decides which
companion books become *appendix pages* of the same site — Sources, Glossary,
Places, Characters, the Language, the World, Mythology, Notes, and a
back-of-book index each fold in when enabled. One book, one contents list, one
website.

#callout(label: "The companion book on this")[
  A whole companion — *Publish Your Book to the Web* — is devoted to the HTML
  export: your first site, the command line, styling, templates, variables,
  what to publish, and going live. This section is the map; that book is the
  territory. Reach for it when you want to make the site genuinely your own.
]

#section("The arXiv and scientific bundle")

For an academic paper, LaTeX is the currency, and a preprint server compiles
your source on its own machine — so it needs *everything the build touches* in
one place. `inkhaven export tex --bundle <dir|zip>` writes exactly that: a
self-contained submission holding the `.tex` (converted from your Typst via the
`tylax` library), a `sources.bib` compiled from your Sources book, every figure
your prose references (copied in flat, with its `\includegraphics` path
rewritten to match), and a `MANIFEST.txt` that explains the package and flags
anything you must check by hand. A `.zip` extension writes one archive;
otherwise it writes a directory.

#screen(caption: "Writing an arXiv-ready bundle")[```
$ inkhaven export tex --bundle submission.zip
wrote zip bundle to submission.zip — paper.tex,
12 bib entries, 3 figure(s)
```]

The bundle quietly corrects the two things that would otherwise stop the
submission compiling. It rewrites citation commands so a bibliography key
`@smith2020` becomes `\cite{smith2020}` while a genuine cross-reference stays
`\ref{…}`, and it maps your Typst citation style to a real LaTeX bibliography
style (`ieee` to `IEEEtran`, `apa` to `apalike`, and so on, defaulting to
`plain` for anything unknown, which the MANIFEST notes). Figures referenced but
missing from disk are listed in the MANIFEST rather than silently dropped, so
you know exactly what to add before you upload.

The bundle composes with the paper *front matter*. When a project defines a
`frontmatter` block — authors, affiliations, abstract, keywords, funding — that
title block is prepended to the Typst, `tex`, `pdf`, and `typst` exports. The
`--blind` flag omits the identifying parts (authors, affiliations, ORCID,
corresponding author, funding) while keeping title, abstract, keywords, and the
availability statements — the shape a double-blind review wants — and it
combines with `--bundle` for a blind submission archive.

#section("DOCX, Markdown, and the manuscript formats")

Two more destinations serve the desk rather than the reader: the standard
manuscript formats an agent or editor expects, and a plain-text dump for
sharing or pasting.

#subsection("Word, in standard manuscript format")

`inkhaven docx` writes a Word document in *Shunn standard manuscript format* —
the layout submissions actually require. It is a title page (your contact block
in one corner, a rounded word count in another, the title and byline centred),
then double-spaced 12-point body with a one-inch margin and a half-inch
first-line indent, each chapter beginning a fresh page, scene breaks rendered as
a centred `#`, and a `Surname / KEYWORD / page` running header from page two.
The file is real OOXML, hand-built over the same `zip` library as the EPUB
writer, so it opens cleanly in Word, LibreOffice, and Google Docs.

#screen(caption: "A submission-ready Word document")[```
$ inkhaven docx --title "The Harbor Code" \
    --author "Jane Writer" --font courier
wrote the-harbor-code-manuscript.docx
```]

Its flags mirror the reader-facing exports: `--book-name`, `--output`,
`--title`, `--author`, a `--contact` block (use `\n` for line breaks), and
`--font` to choose the body typeface — `times` (the default) or `courier`, the
two Shunn accepts. If you would rather submit a typeset PDF, `inkhaven
manuscript` produces the same Shunn layout as a Typst document you compile with
`typst compile`.

#subsection("Markdown, and plain LaTeX")

`inkhaven export markdown` runs the manuscript through a Typst-to-Markdown
converter — headings, bold and italic, lists, images, and citations map across;
anything it does not recognise is preserved verbatim in a fenced block so
nothing silently disappears. It is *lossy by design*: Markdown cannot represent
everything Typst can, and the goal is a readable dump good enough to share or
paste, not a round trip. (It is also the intermediate the minimal EPUB writer
converts from.) `inkhaven export tex` gives you the plain LaTeX without the
bundle wrapper — the same `tylax` conversion, written to a file or standard
output.

#section("The audiobook and reading aloud")

Inkhaven can *speak* your prose — a paragraph at a time while you write, or a
whole book synthesised to an audiobook. Both run through one text-to-speech
engine, and the whole feature is off until you turn it on.

#term("The TTS engine")[
  Inkhaven resolves one of two backends from your `editor.tts.engine` setting.
  *Piper* is a neural, cross-platform engine whose voice models are downloaded
  per project; *System* is the host's own voice (macOS `say`). `auto` (the
  default) prefers Piper and falls back to System; `piper` forces Piper and
  errors if it cannot resolve; `system` forces the host voice. Nothing speaks
  unless `editor.tts.enabled = true` — TTS is opt-in.
]

#subsection("Managing voices from the command line")

The `inkhaven tts` family manages the whole stack headlessly, mirroring the
in-editor surfaces so you can script it. `tts engine` reports which backend is
active and why; `tts binary status` and `tts binary download` manage the Piper
binary; `tts voice list`, `tts voice download`, and `tts voice remove` handle
per-voice CRUD against the Hugging Face catalog; `tts catalog refresh` clears
the cached catalog; and `tts test "<phrase>"` synthesises and plays a phrase as
a diagnostic (`--voice` to A/B a voice, `--output` to write a file instead of
playing).

#screen(caption: "Browsing and fetching voices")[```
$ inkhaven tts voice list --filter en
inkhaven voices — .inkhaven/voices
catalog: fresh
count:       11

key                    language quality  status
en_GB-alan-medium      en_GB    medium   [⬇ available]
en_US-lessac-medium    en_US    medium   [✓ downloaded]
en_US-ryan-high        en_US    high     [⬇ available]

$ inkhaven tts voice download en_US-ryan-high
downloading ... OK
```]

#subsection("Reading aloud in the editor")

Three chords bring speech into the writing flow. `Ctrl+B S`, with a paragraph
open in the Editor, *reads it aloud* through the resolved engine — a small modal
shows the elapsed time, the voice, and the opening of the paragraph while it
plays, and any key stops it. (In the Tree, `Ctrl+B S` keeps its structural
meaning — add a subchapter — so the two never collide.) `Ctrl+B Shift+V` opens
the *voice picker*: the full catalog plus every voice already downloaded, sorted
by language and quality, filterable as you type, with `Enter` fetching an
available voice or selecting a downloaded one and `d` removing one. And `Ctrl+B
Shift+R` *saves the open paragraph as an audio file* through the macOS `say`
engine, opening a path picker pre-filled under `<project>/audio/` — the format
follows the extension you give it.

#chord_table((
  chord_row("Ctrl+B S", "Editor: read the open paragraph aloud. (Tree: add a subchapter.)"),
  chord_row("Ctrl+B Shift+V", "Open the Piper voice picker — browse, filter, download, select, remove."),
  chord_row("Ctrl+B Shift+R", "Editor: save the open paragraph as an audio file (macOS say)."),
))

#subsection("The whole book as an audiobook")

`inkhaven audiobook` synthesises a user book to a single `.m4b` — the resumable,
jump-by-chapter format audiobook players expect — with *one chapter marker per
Chapter node*. It synthesises each chapter's prose to a temporary audio file,
measures the durations, and muxes the lot with chapter metadata. Two things must
be in place: `editor.tts.enabled = true`, and `ffmpeg` plus `ffprobe` on your
`PATH` (there is no pure-Rust muxer with chapter support, so this one external
is required — the command says so clearly if it is missing).

#screen(caption: "Producing a chapter-marked audiobook")[```
$ inkhaven audiobook --book-name "Harbor" -o harbor.m4b
audiobook: backend=piper voice=en_US-ryan-high
  [1/14] synthesising `Arrivals`…
  [2/14] synthesising `The Wharf`…
  ...
audiobook: muxing 14 chapters → m4b…
Audiobook: harbor.m4b (14 chapters · 8h12m04s · 214 MB)
```]

It takes the same `--book-name`, `--output`, `--title`, and `--author` flags as
the EPUB export. Synthesis is *roughly real time*: a long book is hours of audio
and hours to produce, so this is a batch export — the command reports per-chapter
progress on standard error and is not something you run interactively.

#callout(label: "The TTS config block")[
  Everything above keys off the `editor.tts` block: `enabled` (the master
  switch), `engine` (`auto`/`piper`/`system`), `voice`, `speed`, `voices_dir`
  (default `.inkhaven/voices`, sandboxed to the project), `auto_download` (fetch
  missing voices on first use), and `catalog_url`. A `greeting` and `goodbye`
  string, if set, are spoken at startup and shutdown. All of it is inert until
  `enabled` is `true`.
]

#section("Batch export — taking the book once")

You rarely want just one format at a time. The `output.extra_formats` config
list wires the multi-format build to a single chord: `Ctrl+B O` first builds the
PDF (the normal build), then feeds the *same assembled source* to the in-process
converters and drops each result next to the PDF with a matching stem. List any
of `markdown`, `tex`, `epub`, and `docx` there, plus the production entries
`imposed_pdf` and `cover_pdf` that operate on the just-built PDF. Unknown entries
log a warning and are skipped; a per-format error is reported but never aborts
the build.

#screen(caption: "extra_formats in inkhaven.hjson")[```
output: {
  extra_formats: ["epub", "markdown", "docx"]
  extras_step_pause_ms: 400
}
```]

With that set, one `Ctrl+B O` leaves you a PDF, an EPUB, a Markdown dump, and a
Word document side by side — the whole distribution of a draft from one
keystroke. It is the natural close to a writing session: press it, and every
destination this chapter described is written at once.

#recap((
  [Almost every export starts from *one assembled Typst source* — a tree-order
  walk of your paragraphs — and shares four filter flags: `--book-name`,
  `--status`, `--tag`, and `--profile`.],
  [`inkhaven export <format>` reaches `typst`, `pdf`, `markdown`, `tex`,
  `epub`, and `html`; richer variants have their own commands.],
  [`inkhaven epub` writes a real EPUB 3 — a spine document per chapter, a cover,
  inline images, and popup footnotes; `inkhaven import-epub` reverses it into an
  editable book.],
  [`inkhaven export html -o <dir>` builds a self-contained website with
  overridable `functional/` and `theme/` templates and companion appendix pages;
  the *Publish Your Book to the Web* companion covers it in full.],
  [`inkhaven export tex --bundle` writes an arXiv-ready LaTeX package — `.tex`,
  `sources.bib`, figures, and a MANIFEST — with citation and style quirks fixed;
  `--blind` strips identifying front matter.],
  [`inkhaven docx` (and `manuscript`) produce Shunn standard manuscript format
  for submission; `export markdown` is a lossy plain-text dump.],
  [With `editor.tts.enabled`, `Ctrl+B S` reads a paragraph aloud, `Ctrl+B
  Shift+V` picks a voice, and `inkhaven audiobook` synthesises a chapter-marked
  `.m4b` (Piper or macOS `say`, `ffmpeg` required).],
  [`Ctrl+B O` builds the PDF and every format in `output.extra_formats` at
  once — the whole distribution from one chord.],
))
