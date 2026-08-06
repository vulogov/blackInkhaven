// A Book, Start to Finish — master file.
//
// Compile with:
//   typst compile Book/START_TO_FINISH/START_TO_FINISH.typ
//
// Output: START_TO_FINISH.pdf. A narrative companion to The Inkhaven Manual:
// where the manual is a topical reference, this book follows ONE manuscript —
// a short fantasy mystery, "The Ninth Lantern" — from `inkhaven init` to a
// published PDF and web edition, touching every feature in the order a real
// author reaches for it. Learn by following a book being written.
//
// Teaches with monospace terminal `screen()` mockups, like the rest of the
// library.

#import "design.typ": *

#book((
  include "chapters/00-the-blank-project.typ",

  part(number: "I", title: "The Foundation"),
  include "chapters/01-starting-the-project.typ",
  include "chapters/02-world-and-cast.typ",

  part(number: "II", title: "Drafting"),
  include "chapters/03-writing.typ",
  include "chapters/04-keeping-the-facts-straight.typ",

  part(number: "III", title: "The Middle"),
  include "chapters/05-the-secret.typ",
  include "chapters/06-voices-and-threads.typ",

  part(number: "IV", title: "Revision"),
  include "chapters/07-the-read-through.typ",
  include "chapters/08-the-editorial-pass.typ",
  include "chapters/09-did-it-get-better.typ",

  part(number: "V", title: "Publishing"),
  include "chapters/10-assembling-the-book.typ",
  include "chapters/11-out-into-the-world.typ",

  include "chapters/99-about-the-author.typ",
))
