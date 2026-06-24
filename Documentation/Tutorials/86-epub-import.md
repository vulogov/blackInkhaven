# Tutorial 86 — Importing an EPUB

*Inkhaven 1.3.37*

Inkhaven has exported EPUB since 1.2.18 (see
[Tutorial 57](57-reader-experience-exports.md)). 1.3.37 closes
the loop: **`inkhaven import-epub`** reads any EPUB and
materialises it as a new user book. It's the exact inverse of
`inkhaven epub` — the same single-binary, no-pandoc story as
the [Scrivener importer](23-scrivener-import.md). Pure-Rust on
the in-tree zip reader + quick-xml, no new dependencies.

## TL;DR

```sh
cd ~/Projects/my-inkhaven-project
inkhaven import-epub my-novel.epub --dry-run
inkhaven import-epub my-novel.epub
```

Dry run reports the counts it *would* create — chapters,
paragraphs, images — without touching the project. The second
run creates the nodes.

## Flags

| Flag | What |
|------|------|
| `<file.epub>` (positional) | The `.epub` file to read. Any EPUB — Inkhaven's own export, or one from elsewhere. |
| `--book-name <title>` | Title for the new user book. Defaults to the EPUB's `dc:title`, and if the package has no title, to `"Imported EPUB"`. |
| `--dry-run` | Open, parse, convert — but create no nodes. Reports the same counters the real run would emit. |

## What it reads

An EPUB is a zip with a known shape, and the importer walks
that shape rather than guessing:

1. `META-INF/container.xml` → the path of the OPF package
   document.
2. The **OPF** is parsed for:
   - the title (`dc:title`) and author (`dc:creator`),
   - the **manifest** — every item's `id` → `href` +
     `media-type`,
   - the **spine** — the ordered reading list of XHTML
     documents. The spine *is* the chapter order; the
     importer follows it, not the filesystem order inside the
     zip.

## What gets created and where it goes

| EPUB structure | Mapping in Inkhaven |
|----------------|----------------------|
| The whole package | One user **Book** (titled per the rule above) |
| Each spine document | One **Chapter** (titled from the document's first heading, else `Chapter N`) |
| Each chapter's converted prose | One **Paragraph** node |
| Manifest image items | Extracted to a `<project>/<book-slug>-images/` sidecar folder |

So the hierarchy you land in is `Book / Chapter / Paragraph` —
flat, one paragraph node per chapter. (See the fidelity notes
below; this is the deliberate first cut, not an accident.)

## XHTML → Typst conversion

Each spine document's body is converted to Typst markup:

- **Headings** `<h1>`–`<h6>` → `=` … `======`
- **Paragraphs** → blank-line-separated blocks
- `<strong>` / `<b>` → `*strong*`
- `<em>` / `<i>` → `_emph_`
- `<ul>` / `<li>` → `-` lists; `<ol>` / `<li>` → `+` lists
- `<br>` → line break
- `<img>` → an in-prose comment (see images, below)

Plain text is **typst-escaped** on the way in, so imported
prose can't render as accidental markup — a literal `*` or `#`
in the source XHTML stays a literal character, not a Typst
directive.

## Images

Image items declared in the manifest are extracted verbatim to
a sidecar folder next to your project:

```
<project>/<book-slug>-images/
```

The in-prose `<img>` references are **not** wired up as image
nodes yet. Each becomes a Typst comment you can act on:

```typst
// [imported image: cover.png]
```

Find the file in the sidecar folder and re-link it however
your project wants the image. Full image-node import is a
flagged follow-up — for now the bytes are safe on disk and the
location is marked in the prose.

## The report

```text
$ inkhaven import-epub my-novel.epub
EPUB import complete — book `My Novel`:
  chapters:   12
  paragraphs: 12
  images:     3
```

`paragraphs` tracks `chapters` because the first cut emits one
paragraph node per chapter; `images` counts what landed in the
sidecar folder.

## Errors and exit code

Per-item errors are **collected, not fatal**. One malformed
chapter doesn't abort the import — the importer logs it, moves
on, and prints the collected errors at the end. But it then
**exits non-zero on partial failure**, so a scripted import
can tell a clean run from a partial one:

```sh
if inkhaven import-epub batch/$f.epub; then
  echo "$f: clean"
else
  echo "$f: partial — check the error list" >&2
fi
```

A zero exit means every spine document converted; a non-zero
exit means at least one chapter is missing or incomplete and
the printed error list tells you which.

## The round-trip

`inkhaven epub` exports a book; `inkhaven import-epub` reads
one back. That makes EPUB a viable interchange format — hand
someone a `.epub`, they import it, they edit, they export
again.

The fidelity is honest, not lossless. Know the current limits
before you treat the round-trip as a backup:

- **One paragraph node per chapter.** The export flows a
  chapter's paragraphs as continuous prose; the importer reads
  that chapter back as a single paragraph node. Your prose is
  intact; your *paragraph-node boundaries* are not preserved
  across the round-trip.
- **Footnotes and complex tables aren't round-tripped yet.**
  The export renders footnotes inline and tables as HTML; the
  importer's converter handles the markup novelists actually
  use (headings, emphasis, lists) and doesn't yet reconstruct
  those structures.
- **Images are a sidecar, not inline nodes** (see above).

For interchange and re-editing, that's plenty. For a
byte-exact archive of your project, use Inkhaven's own project
files — not an EPUB round-trip.

## After import

1. `inkhaven reindex` — rebuilds search embeddings and on-disk
   slug paths for the new nodes.
2. Open the TUI (`inkhaven`); the imported Book appears in the tree
   next to anything else you already had.
3. Re-link any images from `<book-slug>-images/` at the
   `// [imported image: …]` markers.
4. `inkhaven stats` for word counts — from Inkhaven's own
   Typst-aware counter.

## See also

- [Tutorial 57 — Reader-experience exports](57-reader-experience-exports.md)
  — the `inkhaven epub` export this command inverts.
- [Tutorial 23 — Importing a Scrivener project](23-scrivener-import.md)
  — the other single-binary importer; same dry-run-then-commit
  shape, same collected-errors discipline.
