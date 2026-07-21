// Linguistic Research with Inkhaven — master file.
//
// Compile with:
//   typst compile Book/LINGUISTIC/LINGUISTIC.typ
//
// Output: LINGUISTIC.pdf. A companion to "Developing a Constructed Language":
// the same toolset, pointed not at inventing a language but at analysing a real
// one. Russian is the running worked example throughout.

#import "design.typ": *

#book((
  include "chapters/00-introduction.typ",

  part(number: "I", title: "The Workbench"),
  include "chapters/01-a-workbench-for-real-languages.typ",
  include "chapters/02-modelling-russian.typ",

  part(number: "II", title: "Sound"),
  include "chapters/03-the-sound-system.typ",

  part(number: "III", title: "Words"),
  include "chapters/04-morphology-and-glossing.typ",

  part(number: "IV", title: "Sentences"),
  include "chapters/05-syntax.typ",

  part(number: "V", title: "Usage"),
  include "chapters/06-a-corpus-of-russian.typ",

  part(number: "VI", title: "Structure and History"),
  include "chapters/07-typology.typ",
  include "chapters/08-historical-linguistics.typ",

  include "chapters/a-command-reference.typ",

  include "chapters/99-about-the-author.typ",
))
