// Developing a Constructed Language with Inkhaven — master file.
//
// Compile with:
//   typst compile Book/CONLANG_DEVELOPMENT/CONLANG_DEVELOPMENT.typ
//
// Output: CONLANG_DEVELOPMENT.pdf. Each chapter is its own file in chapters/.
// The reading path runs from "what is a conlang" to a finished, printed
// language, one pillar at a time.

#import "design.typ": *

#book((
  include "chapters/00-introduction.typ",

  part(number: "I", title: "Foundations"),
  include "chapters/01-what-is-a-conlang.typ",
  include "chapters/02-meet-inkhaven.typ",
  include "chapters/03-setup.typ",

  part(number: "II", title: "The Sounds of Your Language"),
  include "chapters/04-phonemes.typ",
  include "chapters/05-syllables-and-words.typ",
  include "chapters/06-phonotactics.typ",
  include "chapters/07-allophony.typ",
  include "chapters/08-stress-romanization-tone.typ",

  part(number: "III", title: "Words"),
  include "chapters/09-building-vocabulary.typ",
  include "chapters/10-a-healthy-lexicon.typ",

  part(number: "IV", title: "Grammar"),
  include "chapters/11-morphology.typ",
  include "chapters/12-word-building.typ",
  include "chapters/13-typology.typ",
  include "chapters/14-idioms.typ",

  part(number: "V", title: "A History for Your Language"),
  include "chapters/15-sound-change.typ",
  include "chapters/16-language-families.typ",

  part(number: "VI", title: "A Language in a World"),
  include "chapters/17-dialects-and-registers.typ",
  include "chapters/18-languages-in-contact.typ",
  include "chapters/19-speech-communities.typ",

  part(number: "VII", title: "A Writing System"),
  include "chapters/20-designing-glyphs.typ",
  include "chapters/21-building-the-font.typ",
  include "chapters/22-complex-scripts.typ",

  part(number: "VIII", title: "Producing the Books"),
  include "chapters/23-the-books.typ",

  part(number: "IX", title: "Putting the Language to Work"),
  include "chapters/24-translating.typ",

  part(number: "X", title: "Sharing Your Language"),
  include "chapters/25-interchange.typ",

  part(number: "XI", title: "A Complete Walkthrough"),
  include "chapters/26-walkthrough.typ",

  part(number: "XII", title: "Scripting Your Language"),
  include "chapters/27-scripting-with-bund.typ",

  part(number: "XIII", title: "Analysing Your Language"),
  include "chapters/28-the-inner-linguist.typ",
  include "chapters/29-measuring-your-language.typ",

  include "chapters/a-command-reference.typ",
  include "chapters/b-hjson-reference.typ",
  include "chapters/c-bund-api.typ",
  include "chapters/d-glossary.typ",
))
