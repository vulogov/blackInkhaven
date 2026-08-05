// Know Your Book — master file.
//
// Compile with:
//   typst compile Book/KNOW_YOUR_BOOK/KNOW_YOUR_BOOK.typ
//
// Output: KNOW_YOUR_BOOK.pdf. Each chapter is its own file in chapters/. The
// reading path runs from "what does it mean to know your own book" through every
// intelligence Inkhaven has for understanding a manuscript — the facts beneath it,
// the graph over it, the continuity that watches it, who knows what within it, how
// it reads, how its voices sound, and whether it is getting better draft to draft.
// For fiction and non-fiction authors alike, assuming no prior knowledge.
//
// Teaches with monospace terminal `screen()` mockups — the app IS a terminal, so a
// faithful frame is truer than a diagram and keeps the book self-contained.

#import "design.typ": *

#book((
  include "chapters/00-introduction.typ",

  part(number: "I", title: "The Ground Truth"),
  include "chapters/01-the-facts-beneath.typ",
  include "chapters/02-the-knowledge-graph.typ",

  part(number: "II", title: "The Book Watches Itself"),
  include "chapters/03-continuity.typ",
  include "chapters/04-who-knows-what.typ",

  part(number: "III", title: "The Book Reads Itself"),
  include "chapters/05-the-read-through.typ",
  include "chapters/06-the-voices.typ",

  part(number: "IV", title: "Knowing You're Getting Somewhere"),
  include "chapters/07-did-it-get-better.typ",

  part(number: "V", title: "All Together"),
  include "chapters/08-a-scene-through-every-check.typ",

  part(number: "VI", title: "The Readers Who Question"),
  include "chapters/09-inner-socrates.typ",
  include "chapters/10-inner-editor.typ",
  include "chapters/11-inner-theologian.typ",
  include "chapters/12-inner-poet.typ",

  include "chapters/a-the-knowledge-commands.typ",
  include "chapters/b-glossary.typ",

  include "chapters/99-about-the-author.typ",
))
