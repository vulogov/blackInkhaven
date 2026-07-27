#import "../design.typ": *

#appendix(letter: "A", title: "Command Reference")

Every command this book taught, in one place, grouped by the part of the workflow
it belongs to. In the Research Assistant, type a single `/` and a hint bar lists
the matching commands as you narrow it; press `Tab` to complete one, or `Ctrl+B h`
for the full reference on screen. You never have to memorise these.

#section("Asking and keeping")

#list(
  [*(a plain question)* — ask in ordinary language; the answer is grounded on the
   facts you have already kept.],
  [`/fact <claim>` — keep a claim as a trusted *Fact*; crosses the confirmation
   gate.],
  [`/note <claim>` — keep a speculative claim as a *Note* instead.],
  [`/verify` — probe the model's confidence in its last answer before you keep it,
   so a shaky reply doesn't become a Fact.],
  [`/diff` — list the facts already in your corpus most similar to the Assistant's
   last answer, so you notice a near-duplicate before keeping it (top-N tuned by
   `research.diff_top_n`).],
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
  [`/triangulate <claim>` *(alias `/tri`)* — cross-check a claim against Wikidata
   and the two scholarly indexes (OpenAlex, arXiv) in one pass; reports SUPPORTS /
   CONTRADICTS / SILENT.],
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
  [`/review` — triage the untrusted facts an `--agentic` run emitted: step through
   them and *accept* (a), *delete* (d), or mark *undisputed* (u); contradictions are
   flagged with ≠.],
  [`/deadsources` — scan your kept web sources for link-rot and flag the ones that
   no longer resolve, so a citation does not quietly die under you.],
  [`/sources` — list every fact's provenance in one view — the interactive
   companion to the provenance ladder.],
  [`/forget <source>` — remove an imported source and the material it brought in.],
)

#section("Computing")

#list(
  [`/calc <expr>` — compute a fact: unit conversions, great-circle distances,
   compound growth, list reductions, and domain formulas (geography, astronomy,
   climate, economy). Provenance `computed`.],
  [`/world` — browse the project's World-simulation facts; `/calc` can read them.],
  [`/rag <mode>` — choose what an ordinary question is grounded on: `facts+full`
   (both, the default), `facts` (your kept facts only), or `full` (the whole
   corpus).],
  [`/chain <q1 → q2 → q3>` — run a sequence of questions as a pipeline, each
   building on the last, for a multi-step line of research.],
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

#section("Citations — the Sources book")

The Research Assistant files a citation to the *Sources* system book every time it
grounds an answer on a scholarly work or a page. A separate `inkhaven sources`
command manages that book from the shell, for interchange with reference managers.

#list(
  [`inkhaven sources list` — list every citation in the Sources book.],
  [`inkhaven sources check` — validate the entries (missing keys, malformed
   fields); exits non-zero on a problem, so it fits a CI step.],
  [`inkhaven sources import <file.bib>` — bring citations in from a BibTeX file
   (e.g. exported from Zotero).],
  [`inkhaven sources export --format bibtex|csl-json [--out <file>]` — write the
   citations out. *CSL-JSON* closes the round-trip with Zotero and other
   citation managers; BibTeX suits LaTeX.],
)

#section("Headless (command line)")

#list(
  [`inkhaven research --batch <file> [--out report.md]` — research a list of
   questions unattended; proposes by default.],
  [`… --auto-confirm --confidence <0..1>` — insert findings above the confidence
   threshold automatically; report the rest.],
  [`inkhaven research --import <path>` — ingest a document or folder from the
   command line.],
  [`inkhaven research --sync <folder>` — register a folder; each launch re-imports
   the files that changed since last time.],
  [`inkhaven research --agentic "<topic>" [--out run-log.md]` — research a topic
   autonomously, emitting the findings as *untrusted* Facts into the Facts book
   (triage them later with `/review`); `--out` writes an optional run log.],
  [`inkhaven research --snowball "<seed>"` — follow a paper's citations backward and
   forward on OpenAlex and report the neighborhood to ingest selectively.],
  [`inkhaven research --gutenberg "<query|PG#>"` — ingest a public-domain book
   headlessly (accepts a leading `--chapter N`).],
  [`inkhaven research --bibliography [--out refs.bib]` — write the bibliography to
   a file, or to standard output.],
  [`… --thread <name>` — open (or, headless, act on) a named research thread;
   `--list-threads` shows them, `--export-thread <name> --format <fmt> --out <file>`
   writes one out.],
)

#section("Window and navigation")

Housekeeping inside the Research screen — none of it changes your corpus:

#list(
  [`/goto <path>` — jump the Facts tree to a book, chapter, or fact by path.],
  [`/save` — save the current thread; `/clear` — clear the conversation window
   (your kept facts are untouched).],
)
