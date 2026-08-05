// The Inkhaven Manual — master file.
//
// Compile with:
//   typst compile Book/MANUAL/MANUAL.typ
//
// Output: MANUAL.pdf. Each chapter is its own file in chapters/. This is the
// front door and the operator's tour: the whole of Inkhaven, install to
// publish — every feature, how to reach it, how to run it — delegating the
// deep dives to the topical companions (Know Your Book, Building the World,
// Poetry, Grounding Your Book in Fact, …). It replaces the stale 1.2.6 manual.
//
// Teaches with monospace terminal `screen()` mockups — the app IS a terminal,
// so a faithful frame is truer than a diagram and keeps the book self-contained.

#import "design.typ": *

#book((
  include "chapters/00-introduction.typ",

  part(number: "I", title: "Getting Started"),
  include "chapters/01-installing.typ",
  include "chapters/02-your-first-project.typ",
  include "chapters/03-the-panes.typ",

  part(number: "II", title: "Writing"),
  include "chapters/04-the-editor.typ",
  include "chapters/05-the-tree-and-files.typ",
  include "chapters/06-snapshots-and-history.typ",
  include "chapters/07-search.typ",
  include "chapters/08-style-overlays.typ",

  part(number: "III", title: "The AI Assistant"),
  include "chapters/09-the-ai-pane.typ",
  include "chapters/10-chat-with-your-book.typ",
  include "chapters/11-prompts.typ",
  include "chapters/12-cost.typ",

  part(number: "IV", title: "The World & Its Facts"),
  include "chapters/13-places-and-characters.typ",
  include "chapters/14-the-world.typ",
  include "chapters/15-the-graph.typ",
  include "chapters/16-the-timeline.typ",

  part(number: "V", title: "The Intelligences"),
  include "chapters/17-continuity-and-knowledge.typ",
  include "chapters/18-the-read-through-and-the-voices.typ",
  include "chapters/19-revision-and-history.typ",
  include "chapters/20-the-inner-family.typ",

  part(number: "VI", title: "Language, Verse & Scholarship"),
  include "chapters/21-constructed-languages.typ",
  include "chapters/22-poetry.typ",
  include "chapters/23-research-and-sources.typ",

  part(number: "VII", title: "Producing the Book"),
  include "chapters/24-assembly-and-pdf.typ",
  include "chapters/25-epub-html-and-more.typ",
  include "chapters/26-technical-docs.typ",

  part(number: "VIII", title: "Scripting with Bund"),
  include "chapters/27-bund.typ",

  part(number: "IX", title: "Keeping It Healthy"),
  include "chapters/28-backup-and-doctor.typ",
  include "chapters/29-configuration.typ",

  include "chapters/a-keybinding-reference.typ",
  include "chapters/b-cli-reference.typ",
  include "chapters/c-config-reference.typ",
  include "chapters/d-feature-index.typ",

  include "chapters/99-about-the-author.typ",
))
