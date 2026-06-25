# Tutorial 89 — Bibliography & Citations

*Inkhaven 1.4.5*

Scholarly and non-fiction work needs references, and inkhaven compiles to
Typst — which has a real bibliography engine. SOURCES-1 wires the two together:
you keep your references as structured entries in a dedicated system book, cite
them in prose with Typst's `@key` syntax, and at Book assembly inkhaven writes a
`sources.bib` and appends `#bibliography(...)` so `typst compile` renders the
reference list and resolves every citation. No new tools, no external manager —
your bibliography lives inside the project, versioned alongside the manuscript.

## 1. Where citations live

Every project has a **Sources** system book (it appears among Notes, Research,
Facts, … in the tree). When you create a user book, inkhaven auto-creates a
matching chapter inside Sources and seeds it with one placeholder entry so you
can see the shape:

```
Sources
└── My Novel
    └── example        ← a placeholder citation entry (HJSON)
```

Each citation is one **HJSON paragraph**. Open the placeholder and you'll see
the schema:

```hjson
{
  // Citation key — insert in prose as @smith2024
  key: change-me
  // entry_type: article | book | misc | online | inproceedings | …
  entry_type: article
  author: Last, First
  title: Title of the work
  year: 2024
  // Optional — delete unused fields:
  // journal: Journal Name
  // volume: 1
  // pages: 10-20
  // doi: 10.xxxx/xxxxx
}
```

> **HJSON tip.** Unquoted strings run to the end of the line, so keep one field
> per line and keep `//` comments on their *own* line — an inline comment after
> a value becomes part of the value. Numbers may be unquoted (`year: 2024`);
> inkhaven coerces them to strings.

The only required field is `key` — a keyless draft is simply skipped at compile
time. Cite keys follow Typst's grammar: start with a letter, then letters,
digits, `_ : -` (`smith2024`, `doe:2023b`, `nguyen-1999`).

## 2. Adding entries

Three ways, pick whichever fits:

- **In the editor** — put the tree cursor under the Sources book and press
  **`P`** (add paragraph). The title you type becomes the cite key
  (`Smith 2024` → `smith2024`), and the body is pre-seeded with the schema.
- **Import a `.bib`** — `inkhaven sources import refs.bib --book-name "My Novel"`
  reads a BibTeX file (the format Zotero, JabRef, Google Scholar export) and
  materialises each entry as an HJSON paragraph under the book's Sources
  chapter. Duplicate keys are skipped.
- **By hand** — copy the placeholder, fill it in.

List what's defined any time:

```
$ inkhaven sources list
1 citation entry(ies):
  @smith2024              2024  Smith, Jane — On Things  [My Novel]
```

## 3. Citing in prose

Write `@key` wherever you want a citation, exactly as in Typst:

```
As @smith2024 demonstrated, the effect is robust across samples.
```

Need to remember a key? In the editor press **`Ctrl+V @`** (the citation sigil)
to open the **cite picker** — a fuzzy list of every defined entry. Type to
filter by key, author, or title; `↑↓` to select; **Enter** inserts `@key` at
the cursor. (E-mail addresses like `a@b.com` are *not* treated as citations.)

## 4. Building the bibliography

Assemble the book — **`Ctrl+B A`** in the TUI, or `inkhaven build`. inkhaven:

1. collects the citation entries (see scope below),
2. writes them to `<artefacts>/<book>/sources.bib`,
3. appends `#bibliography("sources.bib", style: "ieee")` to the root `.typ`.

`typst compile` then renders the reference list and resolves your `@key`
citations. The assembly status reports the count:

```
Assembly OK · root: …/my-novel.typ (42 files)
  bibliography: 1 citation(s) -> sources.bib
```

## 5. Validate before you build

`inkhaven sources check` scans your prose for `@key` tokens and verifies each
one is defined — it reads the same files assembly compiles, and **exits
non-zero** when anything is undefined, so it slots into a pre-build step or CI:

```
$ inkhaven sources check
sources check: 1 undefined citation key(s) of 3 (scope: all entries):
  @missingkey  — My Novel › Opening

Define them in the Sources book (or fix the @key spelling).
```

Add `--json` for a machine-readable report, `--book-name` to scope to one book.

## 6. One bibliography or many?

The `sources` config block controls scope:

```hjson
sources: {
  all: true                 // every entry → every book's sources.bib
  bibliography_style: ieee  // any CSL style Typst accepts
  auto_bibliography: true   // append #bibliography(...) automatically
}
```

Set `all: false` to scope citations **per book**: only the Sources chapter
named after the assembled book is collected — so a project with two manuscripts
keeps two independent bibliographies. `sources check` and the cite picker
honour the same scope. Change the rendered style with `bibliography_style`
(`apa`, `chicago-author-date`, `mla`, …), or set `auto_bibliography: false` to
place the `#bibliography(...)` line yourself in your Typst setup.

## 7. From a script

The read-only `ink.sources.*` Bund words expose the bibliography:

```
ink.sources.list        ( -- list )    every entry as { key, type, author, title, year, chapter }
ink.sources.get         ( key -- dict | NODATA )   one entry, all fields
ink.sources.check       ( -- list )    undefined @key citations as { key, book, paragraph }
ink.sources.bibtex      ( -- string )  the compiled BibTeX for all entries
```

All four are `store_read` (default-allowed). Authoring stays in the CLI and
TUI — Bund reads the bibliography, it doesn't rewrite your sources.

---

**See also:** [CONFIGURATION.md → `sources`](../CONFIGURATION.md) ·
[KEYBINDING.md → `Ctrl+V @`](../KEYBINDING.md) · `inkhaven sources --help`.
