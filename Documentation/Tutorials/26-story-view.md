# 26 — Story view

Inkhaven 1.2.5 added a **story view** — a floating PNG of
the current book's structure rendered on top of the editor.
1.2.6 added a paragraph-scoped variant. Both let you see the
book's shape at a glance: chapters, paragraphs, paragraph links,
and Characters / Places / Artefacts mentions, laid out
radially.

The view is pure visualisation. It never edits anything; it
exists to surface the topology you can't easily see in the
tree pane.

## The two chords

| Chord            | Scope     | What it renders |
|-------------------|-----------|------------------|
| `Ctrl+V Shift+W` | Book      | Book at the centre. Every chapter / subchapter / paragraph + paragraph links + lexicon mentions on concentric rings. |
| `Ctrl+V w`       | Paragraph | Open paragraph at the centre. Paragraph link neighbours (one hop out + one hop in) on the inner ring, lexicon mentions on the outer ring. |

The case distinction is intentional — lowercase `w` is the
fast, focused view; Shift+W is the full read. Reuses the
pattern Ctrl+V S / Shift+S etc.

## Book view (Ctrl+V Shift+W)

`Ctrl+V Shift+W` renders the current user book as a radial
graph. Layout uses a twopi-style pure-Rust placement:

```
              ┌── Story view `Aerin Saga` · 1280×960 · S saves · Esc closes ──┐
              │                                                                │
              │                       ┌── Chapter 1 ──┐                        │
              │                       │   The Arrival │                        │
              │                       └───────────────┘                        │
              │                       │       │                                │
              │                       │       │                                │
              │                ┌──────┴──┐ ┌──┴───────┐                       │
              │                │ Storm  │ │ Bell tower │                       │
              │                └─────┬──┘ └──┬─────────┘                       │
              │                      │       │                                  │
              │                  ┌───┴───────┴───┐                              │
              │                  │ Aerin (place) │                              │
              │                  └───────────────┘                              │
              │                                                                │
              └──────────────────────────────────────────────────────────────┘
```

(actual output is a rendered PNG — terminals with kitty /
iterm2 / sixel graphics display it inline; half-block-capable
terminals fall back to a coarser preview).

**Edge types** colour-code the relationships:

- **Hierarchy edges** — parent → child structure (chapter →
  paragraph). Solid.
- **Paragraph links** — outgoing `linked_paragraphs` (1.2.4 paragraph link
  feature, see [`19-wiki-links.md`](19-wiki-links.md)). Dashed.
- **Lexicon mentions** — paragraphs that name a Place /
  Character / Artefact from the corresponding system book.
  Dotted; one endpoint per mention.

## Paragraph mini view (Ctrl+V w)

Open a paragraph and press `Ctrl+V w`:

```
              ┌── Story view (¶) `The Storm` · 800×600 · S saves · Esc closes ──┐
              │                                                                  │
              │            outgoing paragraph link ──┐  ┌── incoming paragraph link        │
              │                                 ▼  ▼                            │
              │                       ┌── The Storm ──┐                          │
              │                       └────────┬───────┘                          │
              │                                │                                 │
              │                       (lexicon mentions)                         │
              │                                ▼                                 │
              │                  ┌─── Aerin (character) ───┐                    │
              │                  ├─── Highkeep (place)     │                    │
              │                  └─── Snowfall (artefact)  │                    │
              │                                                                  │
              └──────────────────────────────────────────────────────────────────┘
```

Inner ring carries hop-1 neighbours; outer ring carries every
Place / Character / Artefact whose title appears in the
paragraph's body. The lexicon scan is the same one that
highlights names inline in the editor (see
[`07-places-and-characters.md`](07-places-and-characters.md)).

## Save the PNG

Inside either view, `S` opens a save-as picker pre-populated
with `<book-slug>-story-YYYYMMDD-HHMM.png` (book view) or
`<paragraph-slug>-story-YYYYMMDD-HHMM.png` (paragraph view).
Enter writes; Esc cancels back to the view.

`Esc` from the view closes the modal entirely.

## Custom output via Bund (`ink.story.render`)

`ink.story.render` (1.2.6+) writes the book view's PNG to a
filesystem path without opening the modal:

```bund
"Aerin Saga" "~/Desktop/aerin-saga-story.png" ink.story.render
```

Stack: `( book-name path -- )`. The book name is the same
case-insensitive title-or-slug match `inkhaven export
--book-name` uses. `~/` expansion is supported on the
destination path.

Policy: `fs_write` (default-denied — opt in via
`scripting.enabled_categories`).

Useful in scripts that snapshot the book graph after a writing
session, or as a `hook.on_save` side-effect that keeps a fresh
graph on disk.

## When the graph is too dense

Long projects produce dense graphs. A few patterns:

- **Use the paragraph view** instead — `Ctrl+V w` is a
  20-node mini view that's always readable.
- **Run the book view per-book** in multi-book projects —
  Ctrl+V Shift+W honours the current book scope.
- **Save and view externally** — PNG output is full-DPI; open
  it in your image viewer for proper zoom.
- **Resize the terminal** — the modal scales to fit.

## Recap

- `Ctrl+V Shift+W` — book story view (everything in the book).
- `Ctrl+V w` — paragraph mini view (current paragraph + hop-1
  paragraph link neighbours + lexicon mentions).
- `S` inside either view saves to a PNG file with a
  date-stamped default name.
- `ink.story.render` (Bund, fs_write policy) writes the book
  graph from a script.
- The view is read-only — it surfaces topology, never mutates.
