// Poetry with Inkhaven — master file.
//
// Compile with:
//   typst compile Book/POETRY/POETRY.typ
//
// Output: POETRY.pdf. The third companion to "Developing a Constructed Language"
// and "Linguistic Research with Inkhaven": the same measure-and-report discipline,
// pointed now at verse. It serves two readers at once — the poet who wants their
// own lines measured, and the critic who wants to interrogate someone else's.

#import "design.typ": *

#book((
  include "chapters/00-introduction.typ",

  part(number: "I", title: "The Line"),
  include "chapters/01-verse-in-inkhaven.typ",
  include "chapters/02-the-sound-of-a-line.typ",

  part(number: "II", title: "Measure"),
  include "chapters/03-metre-and-scansion.typ",
  include "chapters/04-rhyme.typ",

  part(number: "III", title: "The Whole Poem"),
  include "chapters/05-the-inner-poet.typ",
  include "chapters/06-form-and-completion.typ",

  part(number: "IV", title: "Beyond the Original"),
  include "chapters/07-translating-verse.typ",
  include "chapters/08-scripting-and-scholarship.typ",

  part(number: "V", title: "At the Desk"),
  include "chapters/09-writing-a-poem-at-the-desk.typ",

  include "chapters/a-command-reference.typ",

  include "chapters/99-about-the-author.typ",
))
