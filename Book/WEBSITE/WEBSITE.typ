// Publish Your Book to the Web — master file.
//
// Compile with:
//   typst compile Book/WEBSITE/WEBSITE.typ
//
// Output: WEBSITE.pdf. A plain-language guide to Inkhaven's HTML export for
// authors who are neither web experts nor Inkhaven experts: from the one command
// that makes a site, through styling, templates, and variables, to going live.

#import "design.typ": *

#book((
  include "chapters/00-introduction.typ",

  part(number: "I", title: "Your First Website"),
  include "chapters/01-your-first-site.typ",

  part(number: "II", title: "The Commands"),
  include "chapters/02-the-command-line.typ",

  part(number: "III", title: "Making It Yours"),
  include "chapters/03-styling.typ",
  include "chapters/04-templates.typ",
  include "chapters/05-variables.typ",

  part(number: "IV", title: "Publishing Choices"),
  include "chapters/06-what-to-publish.typ",

  part(number: "V", title: "Going Live"),
  include "chapters/07-going-live.typ",

  include "chapters/a-tags-and-status.typ",

  include "chapters/99-about-the-author.typ",
))
