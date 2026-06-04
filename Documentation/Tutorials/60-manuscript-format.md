# Tutorial 60 — Submitting your manuscript

*Inkhaven 1.2.19+*

When the manuscript is done — drafted, revised, the
continuity checked — the last step is getting it out the
door in the format agents and editors expect.  That
format is **Shunn standard manuscript format**:
monospace, double-spaced, a title page with your contact
details and a rounded word count, a running page header,
chapters starting fresh pages, scene breaks as `#`.

`inkhaven manuscript` produces exactly that.

## Export

```bash
$ inkhaven manuscript \
    --book-name "My Novel" \
    --author "Jane Author" \
    --contact "Jane Author\n12 Wharf Lane\nPortcity\njane@example.com" \
    --output my-novel-submission.typ
Manuscript: my-novel-submission.typ (18 chapters · 82417 words, ~82000 rounded)
compile to PDF with: typst compile my-novel-submission.typ
```

* `--book-name` — optional when the project has exactly
  one user book.
* `--output` / `-o` — defaults to
  `<project>/<book-slug>-manuscript.typ`.
* `--title` — defaults to the book's title.
* `--author` — your byline; defaults to
  `editor.comment_author`.  The surname (last word) drives
  the running header.
* `--contact` — the title-page contact block; use `\n`
  for line breaks.

The output is a **self-contained typst document** — no
project imports — so it compiles anywhere typst runs:

```bash
$ typst compile my-novel-submission.typ my-novel-submission.pdf
```

## What you get

The document follows the modern (electronic-submission)
Shunn standard:

* **12pt Courier**, **double-spaced**, **1-inch margins**.
* **Title page** — your contact block in the top-left
  corner, the word count (rounded to the nearest 100 for
  shorter work, the nearest 1000 for novels) in the
  top-right, and the title centred about a third of the
  way down, followed by "by" and your byline.  No header
  on this page.
* **Running header** from page two on:
  `Surname / KEYWORD / page`, where KEYWORD is a word from
  your title ("The Harbor Code" → `HARBOR`).
* **Chapters** each start a fresh page with a centred
  heading a third of the way down.
* **Scene breaks** — any paragraph that is just a divider
  (`* * *`, `***`, `---`, `# # #`, or a lone `§`) renders
  as a centred `#`, the standard manuscript convention.
* **`# # #`** at the very end.

Paragraph prose is escaped, so stray `#` / `*` / `_` in
your text render literally rather than as typst markup.

## Word count

Shunn convention rounds the word count — agents don't
want "82,417", they want "about 82,000".  Inkhaven rounds
to the nearest 100 below 25,000 words and the nearest
1,000 above, and prints both the exact and rounded counts
so you can sanity-check.

Scene-break marker lines aren't counted as prose.

## Which export, again?

1.2.18 + 1.2.19 give three finished-book exports, each
for a different audience:

| Goal | Command |
|------|---------|
| E-reader (Kindle, Kobo, Apple Books) | `inkhaven epub` |
| Audiobook player | `inkhaven audiobook` |
| Agent / editor submission | `inkhaven manuscript` |

The typeset interior PDF (for print) is still
`inkhaven build --compile`.

## See also

* [Tutorial 57 — Reader-experience exports](57-reader-experience-exports.md)
  — the ePub + audiobook companions.
* [Tutorial 59 — Revision & continuity](59-revision-and-continuity.md)
  — clean the manuscript before you submit it.
* `Documentation/RELEASE_NOTES/1.2.19.md` — X.1
  implementation log.
