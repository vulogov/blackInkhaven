// Theology and Philosophy with Inkhaven — master file.
//
// Compile with:
//   typst compile Book/THEOLOGY_AND_PHILOSOPHY/MANUAL/MANUAL.typ
//
// Output: MANUAL.pdf. Each chapter is its own file in chapters/. The book is a
// single worked example: it researches and writes one real essay — "Kant's
// Transcendental Idealism and Eternal Progression" — from the blank project to
// the finished PDF, and every stage (framing, gathering the primary sources with
// the scripture adapters and Project Gutenberg, interrogating the corpus with
// SCHOLAR, confronting the draft, citing loci, and producing the bibliography and
// Index Locorum) is shown on that one question. The essay it produces is the
// companion volume in this directory.
//
// Teaches with fletcher diagrams rather than screenshots; the first compile
// fetches @preview/fletcher + cetz once, then caches.

#import "design.typ": *

#book((
  include "chapters/00-introduction.typ",

  part(number: "I", title: "Framing the Question"),
  include "chapters/01-framing.typ",

  part(number: "II", title: "Gathering the Sources"),
  include "chapters/02-primary-sources.typ",
  include "chapters/03-the-corpus.typ",

  part(number: "III", title: "Interrogating and Reading"),
  include "chapters/04-scholar.typ",
  include "chapters/05-reading.typ",

  part(number: "IV", title: "Composing and Producing"),
  include "chapters/06-loci.typ",
  include "chapters/07-revising.typ",
  include "chapters/08-producing.typ",

  part(number: "V", title: "Closing"),
  include "chapters/09-conclusion.typ",
  include "chapters/99-about-the-author.typ",
))
