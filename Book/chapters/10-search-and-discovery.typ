#import "../design.typ": *

#chapter(number: 10, part: "Part III — Search, Backup, Export",
  title: "Search and discovery")

#dropcap("E")very paragraph you save gets embedded into a
vector store. The result: inkhaven's search is dual — exact
text matching AND semantic similarity. You don't have to
remember the words; you can search for the idea.

#section("The search bar")

`Ctrl+/` focuses the search bar at the bottom of the screen.
Type, Enter. Results overlay the tree pane.

#figure_slot(
  id: "search-results",
  caption: "Search results overlay — semantic similarity scores on the left, exact-text match icon on the right. Arrows + Enter open.",
  height: 55mm,
)

#chord_table((
  chord_row("↑ / ↓", "Move cursor."),
  chord_row("Enter", "Open the cursor paragraph in the editor."),
  chord_row("Esc", "Close the overlay — tree pane returns."),
  chord_row("Tab", "Toggle between semantic and exact-match modes (default: semantic first)."),
))

#section("Two engines")

The semantic engine uses fastembed locally (no network calls)
to embed your query, then queries the vector store via
HNSW for the K nearest matches. The exact engine is plain
substring search via the metadata DB. Both run in
parallel; results merge by score.

The first time you launch inkhaven, the embedding model
downloads (~120 MB; sits in `~/.cache/fastembed/`). After
that, everything is offline.

#section("Fuzzy paragraph picker (`Ctrl+V P`)")

A second navigation tool — a popup picker over every
paragraph title in the project:

#chord_table((
  chord_row("Ctrl+V P", "Open the picker."),
  chord_row("Type", "Fuzzy-match against title + slug-path."),
  chord_row("Enter", "Open the selected paragraph."),
  chord_row("Esc", "Close."),
))

Faster than the search bar when you remember the title.
Slash commands (`/recent`, `/bookmarks`) reorder the list.

#section("Bookmarks (`Ctrl+V B / M`)")

#chord_table((
  chord_row("Ctrl+V B", "Toggle bookmark on the open paragraph."),
  chord_row("Ctrl+V M", "Open the bookmark picker."),
))

Bookmarks are stored on the node (`Node.bookmark: bool`). The
picker shows every bookmarked paragraph in tree order; Enter
opens, D removes the bookmark.

#section("Help RAG (`F1`)")

`F1` opens a query pane against the Help book — every
paragraph in the system Help book gets retrieved by
semantic similarity and stuffed into the AI context.
Useful for "how do I do X in inkhaven" questions when you
forget the chord.

After you import this Book of Inkhaven into the Help book
(see Chapter 26), F1 covers the full user surface.

#callout(label: "F1 = always-on docs")[
  Once the Help book is populated, F1 is the fastest path
  from "I want to do something" to "found the chord". Your
  Help RAG can index this book, the inkhaven tutorials, and
  the reference docs all at once.
]

#section("Search hooks (Bund)")

`ink.search.text` (Bund stdlib) lets scripts query the search
engine directly:

```bund
"the storm came" 10 ink.search.text     // ( query limit -- list )
```

Returns a list of hit hashes with `id`, `title`, `score`,
`kind`, `document`. Useful for hooks that react to writing
patterns ("oh, you mentioned 'storm' — relevant scenes:
…").

#recap((
  [`Ctrl+/` focuses the search bar; semantic + exact in parallel.],
  [`Ctrl+V P` is the fuzzy paragraph picker (faster when you remember the title).],
  [`Ctrl+V B / M` for bookmarks.],
  [`F1` opens Help RAG — RAG over the Help book.],
  [`ink.search.text` Bund stdlib for programmatic queries.],
  [First launch downloads the fastembed model; everything offline after.],
))
