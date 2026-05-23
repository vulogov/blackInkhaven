# 15 — Wiki-links and backlinks

Wiki-links are typed references between paragraphs. They live as metadata, never as embedded markers in the typst source. The link never leaks into your PDF; it sits in the paragraph's `linked_paragraphs` field next to `status`, `tags`, etc.

The metadata-only choice is deliberate: nothing about `[[X]]` syntax leaks into export, renaming a paragraph never breaks the link (UUIDs are stable), and the AI inference path can read structured references without parsing markup.

## The four chords

| Chord | What it does |
|-------|--------------|
| Ctrl+V A | Outgoing — add a link FROM the open paragraph TO a picked paragraph. |
| Ctrl+V I | Incoming — add a link FROM a picked paragraph TO the open one (reverse of A). |
| Ctrl+V L | Open the linked-paragraphs picker (outgoing links). |
| Ctrl+V K | Open the backlinks picker (paragraphs linking TO the open one). |

## Adding a link

`Ctrl+V A` flips the tree pane into "select a target" mode. Navigate (arrows, slash-filter via `Ctrl+/`) to the paragraph you want to link to, press Enter. The link lands; focus returns to the editor.

![figure: link-pick-tree](images/link-pick-tree.png) — Tree in link-pick mode (Ctrl+V A). Title bar shows the purpose; Enter confirms; Esc cancels.

## Guards

Three checks fire at link-add time:

| Check | Behaviour |
|-------|-----------|
| Self-link | Rejected with `can't link a paragraph to itself`. |
| Duplicate | Reports `already linked`, does nothing. |
| Cycle | A DFS over the candidate target's outgoing closure looks for the owner. If found, rejected with `You can not create circular references`. |

The cycle check makes outgoing-link metadata safe to walk — there's no infinite-recursion risk because there's no cycle. Walks AI-pass payloads, walks the timeline scope filter (Chapter 17), walks the story view edges (Chapter 16).

## Listing + removing

`Ctrl+V L` lists outgoing; `Ctrl+V K` lists backlinks:

![figure: link-picker](images/link-picker.png) — Linked-paragraphs picker. Each row shows direction (→) and slug-path. D removes; Enter opens.

| Chord | What it does |
|-------|--------------|
| ↑ / ↓ | Move cursor. |
| Enter | Open the linked paragraph. |
| D | Remove the link (just this one). |
| Esc | Close. |

## AI integration

When you ask the AI a question with the paragraph in scope, the AI sees `(out: [list of titles])` and `(in: [list of titles])` as RAG context. So "what does this scene set up?" gets a useful answer informed by which other scenes reference this one.

## Story-view edges

Wiki-links appear as dashed edges in the Ctrl+V W book story view (Chapter 16). Visual auditing — see which paragraphs link out, which form hubs of reference, which are disconnected islands.

## Bund — `ink.event.link_paragraph`

For events (Chapter 17), `ink.event.link_paragraph` adds a link from an event to a paragraph. The same scrub on delete (1.2.6 AC, Chapter 14) walks `linked_paragraphs` AND `EventData.characters` / `places` — no stale link lingers when you delete its target.

## Recap

- Wiki-links are metadata, not source markup. They never appear in the PDF.
- `Ctrl+V A` adds outgoing; `Ctrl+V I` adds incoming; `Ctrl+V L` / `Ctrl+V K` list.
- Guards: self-link, duplicate, and cycle — refused at add time.
- Story view (Chapter 16) renders wiki-links as dashed edges.
- Delete-time scrub keeps links clean automatically.
