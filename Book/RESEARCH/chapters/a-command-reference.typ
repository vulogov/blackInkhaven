#import "../design.typ": *

#appendix(letter: "A", title: "Command Reference")

Every command this book taught, in one place, grouped by the part of the workflow
it belongs to. In the Research Assistant, type a single `/` to open the searchable
command palette — you never have to memorise these; this list is for browsing.

#section("Asking and keeping")

#list(
  [*(a plain question)* — ask in ordinary language; the answer is grounded on the
   facts you have already kept.],
  [`/fact <claim>` — keep a claim as a trusted *Fact*; crosses the confirmation
   gate.],
  [`/note <claim>` — keep a speculative claim as a *Note* instead.],
  [`/promote` — promote a selected Note into the Facts book.],
  [`u` *(key, in the Facts tree)* — toggle the selected fact *undisputed* (※): an
   authorial axiom, exempt from fact-checking.],
)

#section("Authoritative sources")

#list(
  [`/web <query>` — search and ground a *cited* answer on real pages; a `/fact`
   from it is fact-checked at the gate.],
  [`/web --ingest <query>` — embed the fetched pages into your corpus as searchable
   material instead of answering.],
  [`/wikidata <query>` — structured facts by *Q-id*; gate-skipped.],
  [`/geonames <query>` — real places from the GeoNames gazetteer; gate-skipped.
   Needs a free `research.geonames.username`.],
  [`/openalex <query>` — the top scholarly work (DOI); auto-files its citation to
   Sources.],
  [`/arxiv <query>` — the top preprint; auto-files its citation.],
  [`/gutenberg <query>` *(alias `/pg`)* — ingest a public-domain book; `<PG#>`
   selects an exact edition, `--chapter N` a single chapter.],
)

#section("Cross-checking and maintenance")

#list(
  [`/triangulate <claim>` — cross-check a claim against the structured and
   scholarly sources at once; reports SUPPORTS / CONTRADICTS / SILENT.],
  [`/factcheck` — audit the whole Facts book for per-fact truth and cross-fact
   consistency; marks each fact with a verdict glyph (✓ / ? / ✗).],
  [`/whatswrong` — explain why the selected flagged fact failed, and what the
   correct information appears to be.],
  [`/upgrade [facts/path]` — re-ground a `model` fact on a corroborating source and
   raise its provenance tier in place (the wording is never changed).],
  [`/stale [days]` — list `model` / `web` facts older than *days* (default 90) for
   re-verification.],
  [`/undisputed` — check your undisputed (authorial) facts for *internal coherence*
   (PLAUSIBLE / ODD / INCOHERENT), in the project language.],
)

#section("Computing")

#list(
  [`/calc <expr>` — compute a fact: unit conversions, great-circle distances,
   compound growth, list reductions, and domain formulas (geography, astronomy,
   climate, economy). Provenance `computed`.],
  [`/world` — browse the project's World-simulation facts; `/calc` can read them.],
)

#section("Composing out")

#list(
  [`/synthesize <topic>` — a grounded, cited overview drawn only from your kept
   facts, honest about where the corpus is thin.],
  [`/outline <topic>` — a fact-citing outline to write into; uncovered points
   marked `(needs research)`.],
  [`/gaps <topic>` — the open questions your corpus cannot yet answer.],
  [`/bibliography` — collect the Sources book's citations into BibTeX.],
)

#section("Headless (command line)")

#list(
  [`inkhaven research --batch <file> [--out report.md]` — research a list of
   questions unattended; proposes by default.],
  [`… --auto-confirm --confidence <0..1>` — insert findings above the confidence
   threshold automatically; report the rest.],
  [`inkhaven research --import <path>` — ingest a document or folder from the
   command line.],
  [`inkhaven research --sync <folder>` — register a folder for re-import whenever
   its contents change.],
  [`inkhaven research --gutenberg "<query|PG#>"` — ingest a public-domain book
   headlessly (accepts a leading `--chapter N`).],
  [`inkhaven research --bibliography [--out refs.bib]` — write the bibliography to
   a file, or to standard output.],
)
