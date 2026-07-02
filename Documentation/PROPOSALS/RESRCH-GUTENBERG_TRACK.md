# RESRCH-GUTENBERG — Project Gutenberg as a research source (proposal)

| | |
|---|---|
| **Status** | **PG-P1..P4 shipped** (PG-P1/P2 in 1.5.8; PG-P3 auto-cite + PG-P4 chapters/picker in 1.5.9-dev); CLI `--gutenberg` still open |
| **Builds on** | RESRCH-2 (document import: chunk → embed → retrieve → cite), RESRCH-3 (source adapters + provenance), R3-B (SOURCES-1 auto-cite) |
| **Theme** | Bring **Project Gutenberg** — ~75,000 **public-domain** full-text books — into the corpus as a research source: search the catalogue, ingest a book's text (or a chapter), and let the existing RAG surface the relevant **snippets**. A full-text library adapter, keyless and free. |

## What the user wants

> *Search snippets / chapters from the books in Project Gutenberg.*

Two things: **find** relevant public-domain books, and **pull passages** from them into the corpus so
they ground answers and can be `/fact`ed.

## The two Gutenberg surfaces (grounded)

- **Catalogue search — Gutendex** (`https://gutendex.com/books?search=<query>`): a **free, keyless** JSON
  API over the PG catalogue. Returns `{ count, results: [ { id, title, authors:[{name,…}], subjects,
  bookshelves, languages, formats } ] }`, where `formats["text/plain; charset=utf-8"]` is the book's
  plain-text URL. Search is **metadata** (title / author / subject), *not* full-text.
- **Full text** — the plain-text URL (e.g. `…/ebooks/1342.txt.utf-8`) returns the whole book, wrapped in
  a PG header/footer (`*** START OF THE PROJECT GUTENBERG EBOOK … ***` … `*** END … ***`) that we strip.

**So "search snippets" is two steps:** Gutendex finds the *book* (by title/author/subject); the existing
**RAG retrieval** then finds the *snippets* inside it once ingested. That's exactly how `/import` +
`/web --ingest` already work — the book's chunks are retrieved by semantic search alongside your Facts.

## Grounding (verified reuse — no new mechanisms)

- **`reqwest`** is already a dependency (R2-C); Gutendex + the text fetch are two GETs.
- **The ingestion pipeline already exists** — `imports::chunk_text` + `store.raw().add_document(metadata,
  chunk)` + the `Imports` sidecar (`research-sources.json`), tagged `kind: research_source`. `/web
  --ingest` (`web_ingest`) and `import_one_file` are the exact templates; a Gutenberg book is just another
  text source flowing through it. Retrieved chunks are then cited `[source: name]` and ground `/fact`.
- **Provenance** is an open origin string — `origin=gutenberg` (+ PG id / URL). Document tier of the
  trust ladder (imported text).
- **SOURCES-1 auto-cite** — a PG book *is* a citable source; `add_bibentry` (R3-B/D) can file a `BibEntry`
  (author, title, year, `note: "Project Gutenberg #<id>"`) so it lands in `/bibliography`.
- **No new crates.** Plain-text (not HTML), so no parser needed — just strip the PG header/footer.

## Design

### `/gutenberg <query>` (alias `/pg`)
1. **Search** Gutendex for `<query>` (respecting the project language via `languages=<code>`); take the
   top match (or list a few for the user to pick — see below).
2. **Fetch** its plain-text URL, **strip** the PG header/footer, **chunk** (`chunk_text`), and **embed**
   each chunk as a `research_source` (`origin=gutenberg`, name = `<title> (PG#<id>)`) — the `/web
   --ingest` path exactly.
3. **Auto-cite** (optional, on by default): file a SOURCES-1 `BibEntry` for the book.
4. From then on, normal queries (and `/synthesize`, `/fact`) retrieve the relevant **snippets** from the
   book, cited `[source: <title> (PG#<id>)]`.

### Granularity
- **First cut:** ingest the whole book text (chunked). Snippet retrieval is free via RAG — this already
  satisfies "search snippets from PG books."
- **Chapter option (follow-on):** split the stripped text into chapters (heuristic on `CHAPTER` /
  roman-numeral / `\n\n\n` boundaries) and ingest per-chapter as named sources, or a `--chapter N`
  selector, so the corpus carries just the relevant chapter, not a whole novel.
- **Picker (follow-on):** `/gutenberg <query>` lists the top N matches (title · author · PG#) and the
  user chooses which to ingest (mirrors the thread picker); the bare form ingests the top hit.

### Config — `research.gutenberg`
`enabled` (default true), `endpoint` (default `https://gutendex.com`, override for a mirror),
`max_chars` (cap per book to bound embedding cost), `auto_cite`. Keyless.

## Phases

| Phase | Content |
|---|---|
| **PG-P1** | `research/gutenberg.rs`: Gutendex search (`GutenbergBook { id, title, authors, subjects, text_url }`) + plain-text fetch + `strip_pg_boilerplate`. Fixture-tested parse (Gutendex is blockable from CI). **✅ Shipped 1.5.8-dev** — keyless, project-language, `max_chars` cap; 3 fixture tests. |
| **PG-P2** | `/gutenberg <query>` command (+ `CommandSpec` for the UX-P1 palette): search → fetch → chunk + embed as a `research_source` (`origin=gutenberg`) via the existing import path; `Imports` sidecar entry; provenance. **✅ Shipped 1.5.8-dev** (TUI) — `/gutenberg` / `/pg`, `ingest_gutenberg` mirrors `/web --ingest`; cited `[source: <Title> (PG#<id>)]`. *(CLI `--gutenberg` deferred — needs a runtime for the async fetch.)* |
| **PG-P3** | Auto-cite the book as a SOURCES-1 `BibEntry` (feeds `/bibliography`). **✅ Shipped 1.5.9-dev** — `GutenbergBook::to_bibentry` (`book`, key `<surname>pg<id>`, note *Project Gutenberg #id*) → `add_bibentry`; gated by `research.gutenberg.auto_cite` (default on). |
| **PG-P4** | Chapter split (`--chapter`) + a multi-result picker. **✅ Shipped 1.5.9-dev** — `--chapter N` / `--ch N` (`split_chapters` heuristic → ingest one chapter); `fetch` returns up to 4 **alternatives**, listed with their PG ids, and a bare `/gutenberg <PG#>` fetches that exact book (`/books/<id>`). |

## Etiquette & limits
- **Public domain** — PG texts carry no copyright; ingesting them is unproblematic. Gutendex is a
  sanctioned free API; the plain-text files are meant to be downloaded.
- **Be polite** — a descriptive `User-Agent`, one book per command (not bulk crawling — PG discourages
  automated *mass* downloading of `gutenberg.org`), and a `max_chars` cap. `endpoint` lets a user point at
  a mirror. Degrades cleanly offline / when `enabled=false`.
- **Not full-text search across all of PG** — Gutendex searches *metadata*; snippet-level search happens
  *after* ingest, via the corpus RAG. (True cross-corpus full-text search over all 75k books is a
  different, much larger service and is out of scope.)

## Recommended first cut
**PG-P1 + PG-P2** — search + ingest, reusing the `/web --ingest` pipeline end-to-end. It delivers the ask
(find a public-domain book, pull its passages into the corpus) with no new mechanisms; PG-P3 (auto-cite)
and PG-P4 (chapters, picker) follow.

## Relationship to the tracks
This is a **source adapter** in the RESRCH-3 mould (one command, an origin tag, a trust-ladder slot) but
over a **full-text library** rather than structured/scholarly metadata — so it rides the RESRCH-2
ingestion pipeline. It also composes with RESRCH-5: an ingested PG book feeds `/synthesize` and
`/bibliography` immediately.
