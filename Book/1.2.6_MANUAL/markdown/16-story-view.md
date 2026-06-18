# 16 — The story view

The story view is a radial graph of your book's topology — chapters, paragraphs, wiki-links, lexicon mentions — rendered as a PNG and floated over the editor. Pure visualisation: never mutates anything, just shows the shape.

## The two chords

| Chord | What it does |
|-------|--------------|
| Ctrl+V Shift+W | Book view — book at the centre, everything on concentric rings. |
| Ctrl+V w | Paragraph mini view — open paragraph at the centre, hop-1 wiki-link neighbours + lexicon mentions on outer ring. |

Case differentiates scope, mirroring the timeline chord (Chapter 17): lowercase = focus, uppercase = full read.

## Book view

![figure: story-view-book](images/story-view-book.png) — Ctrl+V Shift+W: book story view. Book at centre. Chapters, paragraphs, wiki-link dashed edges, lexicon mentions dotted edges.

Edge legend:

| Style | Meaning |
|-------|---------|
| Solid | Hierarchy (parent → child). |
| Dashed | Wiki-link (Chapter 15). |
| Dotted | Lexicon mention (paragraph → matched Place / Character / Artefact). |

## Paragraph mini view

`Ctrl+V w` is the 20-node version — fast, focused, always readable. Inner ring: outgoing wiki-link targets on the right, incoming sources on the left. Outer ring: every Place / Character / Artefact whose title appears in the paragraph's body.

![figure: story-view-paragraph](images/story-view-paragraph.png) — Ctrl+V w: paragraph mini view. Open paragraph at centre; hop-1 neighbours on inner ring; lexicon on outer.

## Save the PNG

`S` inside either view opens a save-as picker pre-populated with `<book-slug>-story-YYYYMMDD-HHMM.png` (book) or `<paragraph-slug>-story-YYYYMMDD-HHMM.png` (paragraph). Enter writes; Esc cancels back to the view.

## From Bund — `ink.story.render`

Write the book view's PNG to a filesystem path without opening the modal:

```bund
"Aerin Saga" "~/Desktop/aerin-saga-story.png" ink.story.render
```

Stack: `( book-name path -- )`. Case-insensitive book lookup against title + slug. `~/` expansion supported.

Policy: `fs_write` (default-denied — opt in via `scripting.enabled_categories`).

Useful as a `hook.on_save` side-effect that keeps a fresh graph on disk every time you save a paragraph — or as a nightly cron via a Bund script.

## When the graph is too dense

Long projects produce dense graphs. A few patterns:

- Use the paragraph view (`Ctrl+V w`) — it's always readable.
- Run the book view per-book (Ctrl+V Shift+W respects the current book scope).
- Save and view externally — the PNG is full-DPI; open it in your image viewer for proper zoom.

## Recap

- `Ctrl+V Shift+W` is the book view; `Ctrl+V w` is the paragraph mini view.
- Three edge types: hierarchy (solid), wiki-link (dashed), lexicon mention (dotted).
- `S` saves to PNG; `Esc` closes.
- `ink.story.render` Bund word writes from a script (fs_write policy).
- Pure visualisation — never mutates.
