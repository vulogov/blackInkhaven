# Grounding Your Book in Fact

*Researching Fiction and Non-Fiction with Inkhaven's Research Assistant.*

A complete, beginner-friendly guide to grounding the facts your book leans on —
from a first question to a fact-checked, cited knowledge base you can compose
from — using Inkhaven's built-in Research Assistant.

It assumes **no prior knowledge** of research *or* of Inkhaven: every idea is
defined where it first appears, and the book serves **fiction and non-fiction
authors equally**, splitting a task into two tracks wherever the work differs by
audience. The focus throughout is the **author's workflow** — what you do, in
what order, and why — not a feature tour.

It teaches with **diagrams** (built with [fletcher](https://typst.app/universe/package/fletcher))
rather than screenshots, which age badly, and it centres one idea: the **trust
ladder** — not all facts are equally trustworthy, and every fact you keep records
its rung (its *provenance*).

## Reading it

The compiled book is [`RESEARCH.pdf`](RESEARCH.pdf) (B5). To rebuild it from
source you need [Typst](https://typst.app):

```sh
typst compile Book/RESEARCH/RESEARCH.typ Book/RESEARCH/RESEARCH.pdf
```

Fonts are the ones Typst bundles (Libertinus + New Computer Modern + DejaVu Sans
Mono), so there is no font setup. The **first** compile fetches two packages from
the Typst universe — `fletcher` and its dependency `cetz` — which then cache
locally; after that it compiles offline.

## Status

Being composed. Parts I–II are written (introduction + six chapters, ~41 pages);
the remaining parts are laid out as a roadmap in [`RESEARCH.typ`](RESEARCH.typ)
and land chapter by chapter:

- **I — Grounding Your Book** *(written)*: why ground your book · the Research
  Assistant · your first fact
- **II — Authoritative Sources** *(written)*: structured facts & real places
  (Wikidata, GeoNames) · the literature & the library (OpenAlex, arXiv, Gutenberg)
  · the web, earned (`/web`, fact-check gate)
- **III — Trust & Cross-Checking**: triangulation · fact-checking · refutation & upgrade
- **IV — Computed Facts**: `/calc` & the World book
- **V — Fiction's Own Facts**: undisputed (authorial) facts
- **VI — Composing Out**: synthesize · outline · gaps · bibliography
- **VII — Working at Scale**: headless research (`--batch`, `--import`, `--sync`)
- **VIII — A Complete Walkthrough** + reference appendices

Closes with an **About the author** afterword.
