# Tutorial 57 — Reader-experience exports: ePub + audiobook

*Inkhaven 1.2.18+*

Inkhaven has always targeted typst → PDF for the
printed page.  1.2.18 adds the two formats readers
actually *consume*: a standards-compliant **EPUB 3**
e-book and a chapter-marked **.m4b audiobook**.

Both are CLI subcommands that walk a user book's
chapters in order — no TUI needed, so they slot into
build scripts + CI.

## ePub export

```bash
$ inkhaven epub --book-name "My Novel" --output my-novel.epub
Exporting `My Novel` → EPUB (12 chapters)…
EPUB: my-novel.epub (12 chapters · 240 KB)
```

* `--book-name` — optional when the project has exactly
  one user book; required otherwise.
* `--output` / `-o` — defaults to
  `<project>/<book-slug>.epub`.
* `--title` — defaults to the book's title.
* `--author` — defaults to `editor.comment_author` in
  HJSON, else "Unknown Author".

The language tag comes from the project's `language`
field (mapped to an ISO code).

### Cover image (1.2.20+)

Drop a `cover.png` (or `cover.jpg` / `cover.jpeg`) next
to your `inkhaven.hjson` and `inkhaven epub` embeds it
automatically — no flag, no config key:

```text
$ inkhaven epub --book-name "My Novel"
Exporting `My Novel` → EPUB (12 chapters)…
  embedding cover (image/png, 184 KB)
EPUB: my-novel.epub (12 chapters · 424 KB)
```

The cover becomes the reader's library thumbnail (via
the EPUB 3 `cover-image` property, with an EPUB 2
`<meta name="cover">` for older readers) and the book's
opening page.  `cover.png` wins if more than one is
present; an unreadable or empty file is skipped with a
warning rather than failing the export.  No cover file →
the text-only output is unchanged.

### What's in the box

Inkhaven builds the EPUB 3 container directly — no
pandoc, no external converter, zero new dependencies.
Each top-level Chapter node becomes one XHTML document;
its paragraphs (and any subchapters, surfaced as `<h2>`)
flow as continuous prose.  The package includes a
`nav.xhtml` navigation document, a `toc.ncx` for older
readers, and a minimal stylesheet.

The typst → XHTML conversion handles the markup inkhaven
prose actually uses:

| Typst | XHTML |
|-------|-------|
| `= …` / `== …` / `=== …` | `<h1>` / `<h2>` / `<h3>` |
| `_emphasis_` | `<em>` |
| `*strong*` | `<strong>` |
| `#footnote[…]` | inline `<span class="footnote">` |
| blank-line blocks | `<p>` |

Each paragraph's leading `= title` (e.g. "001.
Approach") is treated as organisational scaffolding and
stripped — the reader sees flowing chapter prose, not
scene labels.  Chapter headings are cleaned too:
`Chapter 3: The Box` → `The Box`.

### Validating

The output passes well-formedness on every member
(container, package, nav, ncx, chapter XHTML).  Run it
through [epubcheck](https://github.com/w3c/epubcheck)
for full conformance before publishing:

```bash
$ epubcheck my-novel.epub
```

## Audiobook export

```bash
$ inkhaven audiobook --book-name "My Novel" --output my-novel.m4b
audiobook: backend=piper voice=en_US-lessac-medium
  [1/12] synthesising `Arrivals`…
  [2/12] synthesising `The Wharf`…
  …
audiobook: muxing 12 chapters → m4b…
Audiobook: my-novel.m4b (12 chapters · 4h18m · 121 MB)
```

A single `.m4b` with a **chapter marker per Chapter
node** — the resumable, jump-by-chapter format
audiobook players expect.  Same flags as `epub`
(`--book-name` / `--output` / `--title` / `--author`).

### Requirements

* **TTS enabled** — `editor.tts.enabled = true` with a
  voice configured (see [Tutorial 56](56-tts-piper.md)).
  Uses whatever backend is active (Piper or macOS
  `say`).
* **ffmpeg + ffprobe on PATH** — there's no pure-Rust
  m4b muxer with chapter support, so ffmpeg does the
  concat + chapter-metadata mux.  Install via
  `brew install ffmpeg` / `apt install ffmpeg` /
  `winget install ffmpeg`.

The command pre-flights both + gives a clear error if
either is missing.

### How it works

1. Each chapter's prose is stripped to clean spoken
   text (markup removed, footnotes dropped — they're
   disruptive read aloud) and synthesised to a temp
   audio file via the TTS engine.
2. `ffprobe` measures each chapter's duration.
3. An ffmpeg metadata file accumulates the chapter
   start/end timestamps.
4. `ffmpeg` concatenates the chapter audio + muxes it
   into an AAC `.m4b` with the chapters embedded.

### A note on wall-clock time

Synthesis is roughly real-time: a four-hour audiobook
takes about four hours to produce.  This is a batch
export — per-chapter progress prints on stderr so you
can watch it advance (or background it).  Parallel
synthesis is a planned follow-up.

## Which export when?

| Goal | Command |
|------|---------|
| Print / typeset PDF | `inkhaven build --compile` |
| E-reader (Kindle, Kobo, Apple Books) | `inkhaven epub` |
| Audiobook player (resumable, chaptered) | `inkhaven audiobook` |

## See also

* [Tutorial 56 — TTS Piper](56-tts-piper.md) — the
  voice backend the audiobook export drives.
* [Tutorial 58 — Reading pace](58-reading-pace.md) —
  the reading-time chip + reader-pace preview, which
  estimate audiobook length before you commit to a
  full synthesis run.
* `Documentation/RELEASE_NOTES/1.2.18.md` — R.1 + R.2
  implementation log.
