#import "../design.typ": *

#chapter(number: 11, title: "Out Into the World")

*The Ninth Lantern* has a PDF. The last chapter walked the tree into a Typst
directory and let `typst compile` turn it into a typeset book — the destination
prose has had since the first printing press. It is the format you send to a
printer, and the one you are proud to hold.

But a printed page is only one of the places a reader lives. One reader opens a
book on a phone on the train; another opens it in a browser and never downloads
anything at all. Neither wants a fixed A5 page zoomed to a thumbnail. They want
prose that *reflows* to the glass in front of them. So before we call the book
finished, we send it to the two other formats a reader actually uses — an EPUB
for the e-reader, and a small web edition for the browser — and we do it, as
Inkhaven does everything at the end, with a single command each.

#section("An EPUB the e-reader opens")

An EPUB is what an e-reader consumes: underneath, a zip of web pages with a
*spine* that orders them and a navigation document that indexes them. Inkhaven
writes one directly — no `pandoc`, no external converter — because it already
holds your prose as Typst and carries a `zip` library in the binary. For a
project with a single user book, the command needs nothing but its name:

#screen(caption: "Exporting the finished e-book")[```
$ inkhaven epub
  embedding cover (image/jpeg, 96 KB)
Exporting `The Ninth Lantern` → EPUB (7 chapters)…
EPUB: the-ninth-lantern.epub (7 chapters · 148 KB)
```]

Three lines, and there is the whole shape of the export in them. Read them in
order.

*It found the cover.* Before it wrote a single chapter, `inkhaven epub` looked
in the project root, found the `cover.jpg` we dropped there, and reported the
format and size as it embedded it. That is the entire ritual: drop a `cover.jpg`
or `cover.png` beside `inkhaven.hjson`, and it becomes both the thumbnail a
reader's library shows and the opening page of the book. No flag, no config.

*One spine document per chapter.* The middle line is the important one. A real
e-book is not one long scroll — it is a chapter the reader can jump to from the
table of contents, a place their progress bar can measure against. So
`inkhaven epub` writes *one spine document per Chapter node*, in display order:
`The Cold Lantern` becomes `chapter-001.xhtml`, the next becomes
`chapter-002.xhtml`, and so on down our seven. Each holds that chapter's
paragraphs converted to XHTML, with any subchapter titles surfaced as headings
inside the flow. The reader gets a proper spine; the e-reader gets its jump
points.

*The result reflows.* The last line is the payoff. Unlike the PDF, nothing in
the EPUB is pinned to a page. The e-reader lays the words out to its own screen
at its own type size — that is what "reflowable" means, and it is the whole
reason the format exists. The 148 KB is small because there are no fixed pages
to store, only prose and the structure that orders it.

#subsection("What rode along inside")

The archive `inkhaven epub` produced has exactly the shape the EPUB spec
demands, and you never have to think about any of it: the `mimetype` entry
first and uncompressed, a `META-INF/container.xml`, a package document listing
every chapter in its manifest and spine, an EPUB 3 `nav.xhtml` for modern
readers and an EPUB 2 `toc.ncx` for older ones, a stylesheet, the cover, and
the seven chapter files.

#screen(caption: "Inside the-ninth-lantern.epub")[```
mimetype                 (stored, first — the EPUB rule)
META-INF/container.xml
OEBPS/content.opf        (metadata + manifest + spine)
OEBPS/nav.xhtml          (EPUB 3 navigation)
OEBPS/toc.ncx            (EPUB 2 back-compat)
OEBPS/style.css
OEBPS/cover.jpg
OEBPS/cover.xhtml
OEBPS/chapter-001.xhtml
OEBPS/chapter-002.xhtml
...
```]

The converter is honest about the markup our prose actually used. The `= title`
line each paragraph carries — the scene label you saw in the Tree, "The Cold
Lantern" — is *scaffolding*, so it is stripped, and the reader sees flowing
chapter prose rather than "001." headings. `_emphasis_` becomes an italic,
`*strong*` a bold, blank-line-separated blocks become paragraphs, and the XML
special characters are escaped. And the one footnote in Aldous's chapter became
a proper EPUB 3 *popup*: a numbered reference in the flow and a collected note
at the chapter's end, which Apple Books renders as a tap-to-pop and plainer
readers show at the foot. The metadata — title, author (from your
`editor.comment_author` config), language, a stable identifier — was filled in
for you, and every part of it takes a flag if you want to override it.

#callout(label: "Two EPUB paths, one to prefer")[
  There is also a minimal `inkhaven export epub`, which packs the whole book as
  a single-chapter EPUB so it matches the Markdown dump byte for byte — handy in
  a batch build, but not what a reader wants. For the real e-book — chapters as
  separate spine documents, the cover, popup footnotes — use the dedicated
  `inkhaven epub`, the command we just ran. When in doubt, that is the one.
]

#section("A small web edition")

The second reader never downloads a file at all — she follows a link. For her
we build a website straight from the same book. One command, one directory:

#screen(caption: "Building the web edition")[```
$ inkhaven export html -o site/
wrote HTML site to site/

$ ls site/
index.html                   ch04-the-reveal.html
ch01-the-cold-lantern.html   search.js
ch02-a-suppliers-fault.html  search-index.js
ch03-onto-the-mole.html      theme.css
...
```]

What lands in `site/` is a *self-contained static website* — one HTML page per
chapter (`ch01-the-cold-lantern.html`, named from the chapter's title), an
`index.html` built from the book's front matter, a navigation list linking them
all, and a `theme.css` for the look. It is the third consumer of the same node
tree the PDF and the EPUB were built from, localised to the book's language,
with any images copied in alongside. You can open `index.html` from disk, or
drop the whole folder on any host that serves files; there is nothing to
install and nothing to run.

The two files worth pointing at are `search.js` and `search-index.js`. Together
they give the site a *search box that works with no server behind it*. Inkhaven
writes the book's text into `search-index.js` as a plain script the page loads
directly — which is why the search runs even from a `file://` URL, opened off a
USB stick with no network in sight — and `search.js` is the small handful of
vanilla JavaScript that matches what you type against it, titles first. No
library, no query going out to anyone. A reader can stand in the middle of *The
Ninth Lantern* and jump to every mention of the sea-fret in the time it takes to
type the word.

#callout(label: "Make the site your own")[
  The look is driven by *overridable templates*: point `--templates <dir>` at
  your own files (or scaffold the defaults with `--eject-templates <dir>` and
  edit those), and anything you leave out falls back to the bundled default, so
  a theme can be one stylesheet. A whole companion — *Publish Your Book to the
  Web* — covers styling, templates, appendix pages, and going live. This section
  is the map; that book is the territory.
]

#two_track[
  Same two commands, no change. *The Ninth Lantern* ships as an EPUB a reader
  buys and opens on a phone, and as a small site you point people to — the
  cover, the seven chapters, the searchable prose, all from the one tree you
  wrote.
][
  Identical path, different shelf. A non-fiction book becomes the same EPUB for
  a reader's library; and `export html` is how a manual or a set of docs becomes
  a *documentation site* — chapters as pages, the client-side search doing the
  work an FAQ never could.
]

#section("From a blank project to a finished book")

Step back and look at the whole distance. Eleven chapters ago there was
nothing — an empty project on a Monday morning, one `inkhaven init` and a
blinking Tree. There is now a finished book in the three formats a book actually
reaches a reader in: a typeset *PDF* for the page and the printer, an *EPUB* for
the e-reader, and a *web edition* for the browser. One manuscript, one tree,
three destinations, each a single command at the end.

#screen(caption: "The distance we came")[```
I    Foundation .. init · the world & the cast
II   Drafting .... the opening · the facts kept straight
III  The Middle .. the secret (KEN) · voices & threads
IV   Revision .... read-through · editorial pass ·
                   did it get better?
V    Publishing .. the PDF · the EPUB · the web
```]

And every feature we passed had its place in that arc. We reached for the world
and the cast when the story needed somewhere to stand, for the who-knows-what
reader when a secret went in that no one could act on early, for the character
voices when five people had to sound like five people, for the read-through and
the editorial pass when the draft was whole enough to judge, and for
"did it get better?" to prove the revision had earned its keep. Not one of them
was reached for its own sake. That is the thing this book set out to show:
Inkhaven is not a pile of features to work through, but a set of tools that each
answer a question a real manuscript asks — and it asks them in roughly this
order, every time.

You will not use all of them on *your* book; few books need all of them. But you
have now seen where each one lives in the life of a manuscript, and that is
enough to reach for the right one at the right moment — and to know which ones
to leave on the shelf. For anything we touched only in passing — the DOCX an
agent expects, the arXiv bundle a preprint server compiles, the audiobook a
reader listens to on a walk — *The Inkhaven Manual* is the reference waiting on
the next shelf, one chapter per feature, ready when your book asks a question
this one did not.

That is the journey. A blank project became a book, and the book has gone out
into the world. Go and write yours — Inkhaven will be there for each question it
raises, in about the order you have just seen. The lantern is lit; the fog is
someone else's problem now.

#recap((
  [`inkhaven epub` writes a real EPUB 3 for the e-reader — *one spine document
  per chapter*, a zero-config `cover.jpg`, popup footnotes, and prose that
  *reflows* to the reader's screen — from a single command, no external tools.],
  [`inkhaven export html -o <dir>` builds a *self-contained static website* —
  one page per chapter, an index, and a *client-side search* (`search.js` +
  `search-index.js`) that runs with no server, even from a `file://` URL;
  `--templates` / `--eject-templates` make the look yours.],
  [The same two commands serve non-fiction just as well — the EPUB for a
  reader's library, `export html` for a documentation site.],
  [We began at a blank project and ended with one book in *three* formats — PDF,
  EPUB, and web — and every feature along the way had its place in that arc.],
  [For anything touched only in passing — DOCX, the arXiv bundle, the audiobook —
  *The Inkhaven Manual* is the per-feature reference. Now go write yours.],
))
