# 10 — Search and discovery

Every paragraph you save gets embedded into a vector store. The result: inkhaven's search is dual — exact text matching AND semantic similarity. You don't have to remember the words; you can search for the idea.

## The search bar

`Ctrl+/` focuses the search input at the top of the screen. Type, Enter. Results overlay the tree pane.

![figure: search-results](images/search-results.png) — Search results overlay. Semantic similarity scores on the left, exact-text match icon on the right. Arrows + Enter open.

| Chord | What it does |
|-------|--------------|
| ↑ / ↓ | Move cursor. |
| Enter | Open the cursor paragraph in the editor. |
| Esc | Close the overlay — tree pane returns. |
| Tab | Toggle between semantic and exact-match modes (default: semantic first). |

## Two engines

The semantic engine uses fastembed locally (no network calls) to embed your query, then queries the vector store via HNSW for the K nearest matches. The exact engine is plain substring search via the metadata DB. Both run in parallel; results merge by score.

The first time you launch inkhaven, the embedding model downloads (~120 MB; sits in `~/.cache/fastembed/`). After that, everything is offline.

## Fuzzy paragraph picker (`Ctrl+V P`)

A second navigation tool — a popup picker over every paragraph title in the project:

| Chord | What it does |
|-------|--------------|
| Ctrl+V P | Open the picker. |
| Type | Fuzzy-match against title + slug-path. |
| Enter | Open the selected paragraph. |
| Esc | Close. |

Faster than the search bar when you remember the title. Slash commands (`/recent`, `/bookmarks`) reorder the list.

## Bookmarks (`Ctrl+V B / M`)

| Chord | What it does |
|-------|--------------|
| Ctrl+V B | Toggle bookmark on the open paragraph. |
| Ctrl+V M | Open the bookmark picker. |

Bookmarks are stored on the node (`Node.bookmark: bool`). The picker shows every bookmarked paragraph in tree order; Enter opens, D removes the bookmark.

## Help RAG (`F1`)

`F1` opens a query pane against the Help book — every paragraph in the system Help book gets retrieved by semantic similarity and stuffed into the AI context. Useful for "how do I do X in inkhaven" questions when you forget the chord.

After you import this Book of Inkhaven into the Help book (see Chapter 26), F1 covers the full user surface.

> **F1 = always-on docs:** Once the Help book is populated, F1 is the fastest path from "I want to do something" to "found the chord". Your Help RAG can index this book, the inkhaven tutorials, and the reference docs all at once.

## Search hooks (Bund)

`ink.search.text` (Bund stdlib) lets scripts query the search engine directly:

```bund
"the storm came" 10 ink.search.text     // ( query limit -- list )
```

Returns a list of hit hashes with `id`, `title`, `score`, `kind`, `document`. Useful for hooks that react to writing patterns ("oh, you mentioned 'storm' — relevant scenes: …").

## Recap

- `Ctrl+/` focuses the search bar; semantic + exact in parallel.
- `Ctrl+V P` is the fuzzy paragraph picker (faster when you remember the title).
- `Ctrl+V B / M` for bookmarks.
- `F1` opens Help RAG — RAG over the Help book.
- `ink.search.text` Bund stdlib for programmatic queries.
- First launch downloads the fastembed model; everything offline after.
