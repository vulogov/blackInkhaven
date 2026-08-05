# Back-of-Book Index (INDEX-1)

*(INDEX-1 — `inkhaven index`; not to be confused with the semantic-search index or
the Index Locorum / Index Verborum)*

A scholarly or non-fiction book wants the appendix a reader turns to last: an
alphabetised list of terms, each pointing at the chapters where it appears.

> **INDEX-1 builds a back-of-book index from the terms you already maintain — the
> Glossary's canonical terms plus any extras you list — locates each in the
> manuscript by whole-word match, deduplicates its hits to the chapter, and renders
> an alphabetised term → chapters index in Markdown, Typst, or JSON.**

It is **deterministic and free** (pure text search — no LLM), and the index core is
pure and I/O-free, so what the CLI emits and what the HTML site export folds in are
built by the same function.

---

## How terms are collected

Two sources, unioned:

1. **The Glossary** — every canonical term (when `docs.index.from_glossary` is on,
   the default). Each Glossary synonym becomes a **see-reference** pointing at its
   canonical term.
2. **`docs.index.terms`** — a config list of extra terms (names, topics, anything
   not in the Glossary).

With no terms from either source the command errors rather than emit an empty index.

Each term is located across every user book's prose (or one book with
`--book-name`). A paragraph's Typst is stripped to plain text and matched
**whole-word, case-insensitively** — *"art"* matches the standalone word, never
*"artist"* or *"start"*. Multiple hits in the same chapter collapse to **one
location** (the index points at chapters, not every occurrence). A term found
nowhere is dropped. Entries are sorted case-insensitively by term.

A see-reference is only emitted when its canonical term actually made it into the
index (a cross-reference to an absent term is useless), and a synonym identical to
its canonical is skipped.

---

## Output formats

`--format` selects the renderer (default `md`); `--out FILE` writes to a file
instead of stdout.

| `--format` | Renders |
| ---------- | ------- |
| `md` (default) | Markdown — `**term** — Chapter A, Chapter B`, or `**term** — *see* canonical` |
| `typ` / `typst` | Typst markup — `*term* — Chapter A, Chapter B` under an `= Index` heading, ready to `#include` |
| `json` | `{ "index": [ { term, see, locations: [ { chapter, anchor } ] } ], "count": N }` |

```
inkhaven index                          # Markdown to stdout, all user books
inkhaven index --book-name "Treatise"   # one book
inkhaven index --format typst -o appendix-index.typ
inkhaven index --format json            # each location carries chapter + anchor
```

---

## Anchors

Every located term carries an `anchor` alongside its `chapter`. In the standalone
`inkhaven index` command the anchor resolves to the chapter (the CLI's units are
chapter-scoped), and the JSON format surfaces it as a first-class field for
downstream tooling; the Markdown and Typst renderers print the chapter name only.

The **real hyperlinked anchors** are produced by the HTML static-site export. With
`docs.html.include.index` on, the site export runs the same index builder over the
chapters and folds in an `appendix-index.html` page where each location is a live
`<a href="…">chapter</a>` link into the chapter's HTML file — so on the web the
back-of-book index is clickable, and in print (Typst) it is a clean alphabetised
appendix.

---

## Configuration

```hjson
docs: {
  index: {
    from_glossary: true    // seed from the Glossary's canonical terms + synonyms
    terms: []              // extra index terms beyond the Glossary
  }
  html: {
    include: { index: true }   // fold a hyperlinked back-of-book index page into the site
  }
}
```

---

## What it is not

- Not the semantic-search index, and not the Index Locorum (`@key[locus]` citations)
  or Index Verborum (scholarly-lexicon usages) — those are separate commands.
- Not a concordance — it points at chapters, not every occurrence or page.
- Not AI — it is whole-word text search over your own term list.
