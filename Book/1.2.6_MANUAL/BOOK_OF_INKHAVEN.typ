// The Book of Inkhaven — master file.
//
// Compile with:
//   typst compile Book/BOOK_OF_INKHAVEN.typ
//
// Output: Book/BOOK_OF_INKHAVEN.pdf
//
// Each chapter lives in chapters/ as its own .typ file.
// The chapter ordering reflects an "easy → difficult"
// reading path: install + first book → editor → world-
// building → AI → typst power → scripting.

#import "design.typ": *

#book((
  include "chapters/00-prologue.typ",

  // Part I — Foundations
  include "chapters/01-what-inkhaven-is.typ",
  include "chapters/02-installation.typ",
  include "chapters/03-the-project-tree.typ",
  include "chapters/04-system-books.typ",
  include "chapters/05-configuration.typ",

  // Part II — The Editor
  include "chapters/06-writing-in-typst.typ",
  include "chapters/07-editor-workflow.typ",
  include "chapters/08-saving-snapshots.typ",
  include "chapters/09-status-and-goals.typ",

  // Part III — Search, Backup, Export
  include "chapters/10-search-and-discovery.typ",
  include "chapters/11-backups-and-recovery.typ",
  include "chapters/12-exporting.typ",

  // Part IV — World Building
  include "chapters/13-places-and-characters.typ",
  include "chapters/14-tags.typ",
  include "chapters/15-wiki-links.typ",
  include "chapters/16-story-view.typ",

  // Part V — The Timeline
  include "chapters/17-story-timeline.typ",

  // Part VI — Working with AI
  include "chapters/18-ai-providers.typ",
  include "chapters/19-the-ai-pane.typ",
  include "chapters/20-prompts-and-grammar.typ",
  include "chapters/21-critique-and-memory.typ",
  include "chapters/22-ai-for-diagnostics-and-timeline.typ",

  // Part VII — Typst Mastery
  include "chapters/23-typst-inside-inkhaven.typ",
  include "chapters/24-diagnostics-and-render.typ",
  include "chapters/25-multi-format-export.typ",

  // Part VIII — Importing
  include "chapters/26-importing-existing-work.typ",

  // Part IX — Customisation & Scripting
  include "chapters/27-theming.typ",
  include "chapters/28-reassigning-keys.typ",
  include "chapters/29-bund-scripting.typ",

  // Appendices
  include "chapters/appendix-a-keybinding.typ",
  include "chapters/appendix-b-configuration.typ",
  include "chapters/appendix-c-bund-stdlib.typ",

  // Afterword
  include "chapters/99-about-the-author.typ",
))
