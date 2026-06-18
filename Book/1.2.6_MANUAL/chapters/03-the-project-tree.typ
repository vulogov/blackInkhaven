#import "../design.typ": *

#chapter(number: 3, part: "Part I — Foundations",
  title: "The project tree")

#dropcap("T")he tree pane is the manuscript's spine. Every
piece of prose, every script, every note hangs from it.
Understanding the kinds of nodes, the parent-child rules,
and how to reorder them is the first move toward feeling
comfortable in inkhaven.

#section("Node kinds")

#chord_table((
  chord_row("Book", "Top-level container. A project can have multiple. System books (Notes, Help, …) sit alongside user books."),
  chord_row("Chapter", "First-level container inside a book."),
  chord_row("Subchapter", "Optional second-level container inside a chapter."),
  chord_row("Paragraph", "A leaf. Carries the prose. Has metadata: status, tags, word target, links, snapshots."),
  chord_row("Script", "Bund (.bund) leaf. Stores executable code instead of prose. See Chapter 29."),
  chord_row("Image", "PNG / JPG leaf. Referenced by paragraphs through `#image()` typst calls."),
))

The hierarchy rule: Book > Chapter > Subchapter > leaves.
Books can hold paragraphs directly (no chapter), chapters
can hold paragraphs directly (no subchapter), but you can't
nest two chapters or put a book inside another book.

#section("Tree-pane chords")

With focus on the Tree pane (`Ctrl+2`):

#chord_table((
  chord_row("↑ / ↓ / PgUp / PgDn", "Navigate."),
  chord_row("← / →", "Collapse / expand a branch."),
  chord_row("Enter", "Open the paragraph (or focus an image / script)."),
  chord_row("F2", "Rename the cursor node."),
  chord_row("B / C / A / +", "Add book / chapter / subchapter / paragraph as a child of the cursor."),
  chord_row("V / S / P", "Insert chapter / subchapter / paragraph as a sibling AFTER the cursor."),
  chord_row("D", "Delete the cursor's branch (confirm prompt)."),
  chord_row("-", "Delete the cursor paragraph (no children — quick path)."),
  chord_row("U / J", "Reorder up / down among siblings."),
  chord_row("Z / X", "Collapse cursor's subchapter / collapse every branch."),
  chord_row("Space", "Multi-select toggle (see Chapter 14 for the picker workflows that use marks)."),
))

#section("Anatomy of a paragraph row")

#figure_slot(
  id: "tree-paragraph-row",
  caption: "A paragraph row carries five things at a glance: indent (depth), kind glyph (¶), status letter (N/F/R/…), title (truncated), and tag pips (#draft, #weather).",
  height: 30mm,
)

The pieces from left to right:

#chord_table((
  chord_row("Indent", "Two spaces per depth level."),
  chord_row("Glyph", "¶ for paragraph, λ for script, ▣ for image, ▾/▸ for expanded/collapsed containers, ► for the open paragraph."),
  chord_row("Status letter", "N (Napkin), F (First), … R (Ready). Spaced reserved when status = None. See Chapter 9."),
  chord_row("Title", "Truncated past 36 chars."),
  chord_row("Progress pip", "Tiny circle (○ ◔ ◑ ◕ ●) when the paragraph has a word target. See Chapter 9."),
  chord_row("Tag pips", "Up to two #tag chips + a +N count. See Chapter 14."),
))

#section("Reordering")

`U` (up) and `J` (down) move the cursor node among its
siblings. The shift is one position per press; long moves
take repetition. The change is immediate — there's no
"save the tree" step.

When you rename a node with `F2`, both its title AND its
slug update. The slug is what shows up in URLs and file
paths; it's auto-generated from the title (kebab-case) but
you can edit it independently in the metadata.

#section("Multi-select")

`Space` toggles whether the cursor node is "marked". Marked
nodes get a `✓` glyph; status badges and reorder actions
apply to all of them at once. This is covered in detail in
Chapter 14 alongside the tag workflows that take advantage
of it.

#section("Folding for focus")

A 200-paragraph manuscript is overwhelming in a flat tree.
`X` collapses every expanded branch; `Z` collapses just
the cursor's enclosing subchapter; `←` collapses the cursor's
direct branch. The opposite (expand) is `→`.

#callout(label: "Tip")[
  Combine `X` with the fuzzy paragraph picker (`Ctrl+V P`,
  Chapter 10): collapse everything, then jump to whatever
  you actually want to work on. Reduces visual noise to
  about one row per chapter title.
]

#recap((
  [Tree kinds: Book → Chapter → Subchapter → Paragraph / Script / Image.],
  [Tree-pane chords mirror file-manager intuitions: arrows, Enter, F2, D.],
  [`U`/`J` reorder siblings; `B`/`C`/`A`/`+` create children; `V`/`S`/`P` create siblings.],
  [Multi-select with `Space` powers project-wide picker workflows.],
  [`X` collapses everything — combine with the fuzzy picker for focused work.],
))
