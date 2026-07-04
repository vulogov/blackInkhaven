// Building the World with Inkhaven — master file.
//
// Compile with:
//   typst compile Book/BUILDING_THE_WORLD/BUILDING_THE_WORLD.typ
//
// Output: BUILDING_THE_WORLD.pdf. Each chapter is its own file in chapters/. The
// reading path runs from "why build a world at all" to a living world — with a
// place, a past, and a people — that is present at your desk while you write. It
// assumes no prior knowledge of worldbuilding or of Inkhaven.
//
// Teaches with fletcher diagrams (from the Typst package universe) rather than
// screenshots; the first compile fetches @preview/fletcher + cetz once, then
// caches.

#import "design.typ": *

#book((
  include "chapters/00-introduction.typ",

  part(number: "I", title: "What a World Is"),
  include "chapters/01-why-build-a-world.typ",
  include "chapters/02-the-world-as-a-system.typ",
  include "chapters/03-your-first-world.typ",

  part(number: "II", title: "The Physical World"),
  include "chapters/04-the-sky.typ",
  include "chapters/05-the-land.typ",
  include "chapters/06-weather-and-water.typ",
  include "chapters/07-people-on-the-map.typ",

  part(number: "III", title: "Giving the World a Past"),
  include "chapters/08-history-and-chronology.typ",

  part(number: "IV", title: "Giving the World a People"),
  include "chapters/09-nations.typ",
  include "chapters/10-cultures-and-tongues.typ",
  include "chapters/11-life.typ",

  part(number: "V", title: "The Author's Hand"),
  include "chapters/12-declared-and-emergent.typ",
  include "chapters/13-rules-and-magic.typ",

  part(number: "VI", title: "The World at the Desk"),
  include "chapters/14-writing-against-the-world.typ",
  include "chapters/15-keeping-prose-true.typ",
  include "chapters/16-into-your-book.typ",

  part(number: "VII", title: "A Complete Walkthrough"),
  include "chapters/17-walkthrough.typ",

  include "chapters/a-command-reference.typ",
  include "chapters/b-glossary.typ",

  include "chapters/99-about-the-author.typ",
))
