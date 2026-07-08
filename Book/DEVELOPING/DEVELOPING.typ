// Developing a story with Inkhaven — master file.
//
// Compile with:
//   typst compile Book/DEVELOPING/DEVELOPING.typ
//
// Output: DEVELOPING.pdf. Each chapter is its own file in chapters/. The reading
// path runs from "what kind of book are you making" through the shared desk to a
// full working guide for each track — fiction, utopia, science fiction,
// nonfiction, scenarios, technical, scientific, and theology/philosophy — each
// tying structure, worldbuilding, research, and the AI readers into one process.
//
// Teaches with fletcher diagrams rather than screenshots; the first compile
// fetches @preview/fletcher + cetz once, then caches.

#import "design.typ": *

#book((
  include "chapters/00-introduction.typ",

  part(number: "I", title: "The Shape of the Work"),
  include "chapters/01-the-tracks.typ",

  part(number: "II", title: "The Desk"),
  include "chapters/02-navigation-and-editing.typ",

  part(number: "III", title: "The Track Guides"),
  include "chapters/03-fiction.typ",
  include "chapters/04-utopia.typ",
  include "chapters/05-science-fiction.typ",
  include "chapters/06-nonfiction.typ",
  include "chapters/07-scenarios.typ",
  include "chapters/08-technical.typ",
  include "chapters/09-scientific.typ",
  include "chapters/10-theology-and-philosophy.typ",

  part(number: "IV", title: "Closing"),
  include "chapters/11-conclusion.typ",

  include "chapters/a-keybindings.typ",
  include "chapters/b-cli-reference.typ",

  include "chapters/99-about-the-author.typ",
))
