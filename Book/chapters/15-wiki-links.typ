#import "../design.typ": *

#chapter(number: 15, part: "Part IV — World Building",
  title: "Wiki-links and backlinks")

#dropcap("W")iki-links are typed references between paragraphs.
They live as metadata, never as embedded markers in the
typst source. The link never leaks into your PDF; it sits
in the paragraph's `linked_paragraphs` field next to
`status`, `tags`, etc.

The metadata-only choice is deliberate: nothing about `[[X]]`
syntax leaks into export, renaming a paragraph never breaks
the link (UUIDs are stable), and the AI inference path can
read structured references without parsing markup.

#section("The four chords")

#chord_table((
  chord_row("Ctrl+V A", "Outgoing — add a link FROM the open paragraph TO a picked paragraph."),
  chord_row("Ctrl+V I", "Incoming — add a link FROM a picked paragraph TO the open one (reverse of A)."),
  chord_row("Ctrl+V L", "Open the linked-paragraphs picker (outgoing links)."),
  chord_row("Ctrl+V K", "Open the backlinks picker (paragraphs linking TO the open one)."),
))

#section("Adding a link")

`Ctrl+V A` flips the tree pane into "select a target" mode.
Navigate (arrows, slash-filter via `Ctrl+/`) to the
paragraph you want to link to, press Enter. The link lands;
focus returns to the editor.

#figure_slot(
  id: "link-pick-tree",
  caption: "Tree in link-pick mode (Ctrl+V A). Title bar shows the purpose; Enter confirms; Esc cancels.",
  height: 40mm,
)

#section("Guards")

Three checks fire at link-add time:

#chord_table((
  chord_row("Self-link", "Rejected with `can't link a paragraph to itself`."),
  chord_row("Duplicate", "Reports `already linked`, does nothing."),
  chord_row("Cycle", "A DFS over the candidate target's outgoing closure looks for the owner. If found, rejected with `You can not create circular references`."),
))

The cycle check makes outgoing-link metadata safe to walk —
there's no infinite-recursion risk because there's no
cycle. Walks AI-pass payloads, walks the timeline scope
filter (Chapter 17), walks the story view edges (Chapter
16).

#section("Listing + removing")

`Ctrl+V L` lists outgoing; `Ctrl+V K` lists backlinks:

#figure_slot(
  id: "link-picker",
  caption: "Linked-paragraphs picker. Each row shows direction (→) and slug-path. D removes; Enter opens.",
  height: 45mm,
)

#chord_table((
  chord_row("↑ / ↓", "Move cursor."),
  chord_row("Enter", "Open the linked paragraph."),
  chord_row("D", "Remove the link (just this one)."),
  chord_row("Esc", "Close."),
))

#section("AI integration")

When you ask the AI a question with the paragraph in scope,
the AI sees `(out: [list of titles])` and
`(in: [list of titles])` as RAG context. So "what does this
scene set up?" gets a useful answer informed by which other
scenes reference this one.

#section("Story-view edges")

Wiki-links appear as dashed edges in the Ctrl+V W book story
view (Chapter 16). Visual auditing — see which paragraphs
link out, which form hubs of reference, which are
disconnected islands.

#section("Bund — `ink.event.link_paragraph`")

For events (Chapter 17), `ink.event.link_paragraph` adds a
link from an event to a paragraph. The same scrub on delete
(1.2.6 AC, Chapter 14) walks `linked_paragraphs` AND
`EventData.characters` / `places` — no stale link lingers
when you delete its target.

#recap((
  [Wiki-links are metadata, not source markup. They never appear in the PDF.],
  [`Ctrl+V A` adds outgoing; `Ctrl+V I` adds incoming; `Ctrl+V L` / `Ctrl+V K` list.],
  [Guards: self-link, duplicate, and cycle — refused at add time.],
  [Story view (Chapter 16) renders wiki-links as dashed edges.],
  [Delete-time scrub keeps links clean automatically.],
))
