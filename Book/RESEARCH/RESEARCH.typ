// Grounding Your Book in Fact — master file.
//
// Compile with:
//   typst compile Book/RESEARCH/RESEARCH.typ
//
// Output: RESEARCH.pdf. Each chapter is its own file in chapters/. The reading
// path runs from "why ground your book" to a fact-checked, cited knowledge base
// you can compose from — for fiction and non-fiction authors alike, assuming no
// prior knowledge of research or Inkhaven.
//
// Uses fletcher (from the Typst package universe) for its diagrams instead of
// screenshots; first compile fetches @preview/fletcher + cetz once, then caches.

#import "design.typ": *

#book((
  include "chapters/00-introduction.typ",

  part(number: "I", title: "Grounding Your Book"),
  include "chapters/01-why-ground-your-book.typ",
  include "chapters/02-the-research-assistant.typ",
  include "chapters/03-your-first-fact.typ",

  part(number: "II", title: "Authoritative Sources"),
  include "chapters/04-wikidata-and-geonames.typ",
  include "chapters/05-scholarly-and-books.typ",
  include "chapters/06-the-web.typ",

  part(number: "III", title: "Trust & Cross-Checking"),
  include "chapters/07-triangulation.typ",
  include "chapters/08-fact-checking.typ",
  include "chapters/09-refutation-and-upgrade.typ",

  part(number: "IV", title: "Computed Facts"),
  include "chapters/10-calc-and-world.typ",

  part(number: "V", title: "Fiction's Own Facts"),
  include "chapters/11-undisputed.typ",

  part(number: "VI", title: "Composing Out"),
  include "chapters/12-synthesize-and-outline.typ",
  include "chapters/13-bibliography.typ",

  part(number: "VII", title: "Working at Scale"),
  include "chapters/14-headless.typ",
  include "chapters/15-autonomous-research.typ",
  include "chapters/16-review-queue.typ",

  part(number: "VIII", title: "A Complete Walkthrough"),
  include "chapters/17-walkthrough.typ",

  include "chapters/a-command-reference.typ",
  include "chapters/b-provenance-reference.typ",
  include "chapters/c-glossary.typ",

  include "chapters/99-about-the-author.typ",
))
