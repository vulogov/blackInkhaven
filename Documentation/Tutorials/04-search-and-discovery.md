# 4 — Search and discovery

This tutorial covers Inkhaven's search story: the Search bar in the
TUI, the semantic+full-text mix under the hood, and the `inkhaven
search` CLI. Every paragraph is indexed; finding the one you're
thinking of takes one keystroke and an approximate phrase.

## What "indexed" means here

When you save a paragraph (`Ctrl+S` or autosave), Inkhaven does three
things in addition to writing the `.typ` file:

1. **Tantivy full-text index** — the prose is tokenised and stored
   for keyword search.
2. **HNSW vector index** — the prose is fed to a multilingual
   embedding model (default `MultilingualE5Small`) and the resulting
   vector is added to a nearest-neighbour graph.
3. **Metadata refresh** — word count, modified time, title (if
   placeholder) are updated in DuckDB.

Search blends keyword and vector hits via bdslib's hybrid scoring. You
do not pick one or the other — the score column in the results
overlay tells you the strength of the match.

If you have not seen this before: "embedding" means the paragraph is
represented as ~384 floating-point numbers (a "vector"). Two passages
that talk about similar things have vectors close together in that
space, even if they use different words. That is why semantic search
can find "the moment the lighthouse fails" given a query about "a
sudden loss of guidance at sea" — neither sentence shares words but
their meanings are nearby.

## Search from the TUI

### Open the Search bar

`Ctrl+/` from any focus (or click the Search bar at the top). The bar
takes focus:

```
┌── Search ──────────────────────────────────────────────────┐
│ │                                                          │
└────────────────────────────────────────────────────────────┘
```

### Type a query

Anything goes. A keyword:

```
> mariner
```

A phrase:

```
> stood at the rail at dawn
```

A loose paraphrase:

```
> the moment the storm broke and someone left the deck
```

### Run the search

`Enter`. Results overlay drops down:

```
┌── Results for `the moment the storm broke and …` (5) ──────┐
│  0.902  [paragraph ] Sample Novel › Chapter One › Morning   │
│         Storm breaks                                         │
│         The boards under his boots heaved as the storm…      │
│                                                              │
│  0.857  [paragraph ] Sample Novel › Chapter Two              │
│         Aftermath                                             │
│         By the time the wind had eased, half the…            │
│                                                              │
│  0.842  [paragraph ] Sample Novel › Notes                    │
│         Storm research                                       │
│         I keep returning to the question of whether…         │
└──────────────────────────────────────────────────────────────┘
```

Each row is three lines:

1. **Score**, **kind**, and a human-readable breadcrumb (built from
   the node titles — not the slug path).
2. The paragraph **title**.
3. A one-line snippet from the body.

### Open a result

`↑` / `↓` to move the cursor in the results. `Enter` opens the
selected paragraph in the Editor pane (focus moves there) and
positions the tree cursor on the same row.

### Dismiss the overlay

`Esc` once closes the overlay (search bar still has focus). `Esc`
again cycles focus to the Editor (or Tree if no paragraph is open).

## Search syntax tips

bdslib treats the query as a natural-language phrase by default.
A few things that work well:

- **Plain keywords** — exact matches always come back near the top
  via the Tantivy keyword side of the hybrid score.
- **Paraphrases** — works because the embedding side picks up
  semantic similarity even when no keyword matches.
- **Multilingual** — set `language: russian` and queries like
  `утренний рассвет на палубе` find Russian prose written in different
  inflections. The multilingual embedding model handles ~100 languages.
- **Long queries** — long queries (a sentence or two) often retrieve
  better than three-word ones because they give the embedder more
  signal.

What does **not** work:

- Boolean operators (`AND`, `OR`, `NOT`) — bdslib's interface is
  natural-language; for that level of control use the CLI with
  multiple separate queries.
- Field-restricted search (e.g. `title:foo`) — not supported.
- Regex search — the in-buffer Ctrl+F find supports regex, but the
  project-wide search bar does not.

## Search from the CLI

```bash
$ inkhaven --project ~/Books/sample-novel search "the moment the storm broke"
```

Output:

```
0.902  [paragraph]  sample-novel/chapter-one/01-morning/02-storm-breaks
       Storm breaks
       The boards under his boots heaved as the storm…

0.857  [paragraph]  sample-novel/chapter-two/01-aftermath
       Aftermath
       By the time the wind had eased, half the…
```

Useful when scripting around Inkhaven — pipe to `grep` for a UUID
column, list the top matches to a file before a manuscript-wide edit,
etc.

Flags:

| Flag | Default | What it does |
| ---- | ------- | ------------ |
| `--limit N` | 10 | Maximum number of hits to return. |

## Tuning relevance

Two HJSON knobs influence search quality. Both default sensibly; you
rarely need to touch them.

### `embeddings.model`

The default `MultilingualE5Small` covers ~100 languages with good
recall. If you want higher quality on a beefy machine, switch to:

- `MultilingualE5Base` (768-dim, ~300 MB) — markedly better at the
  cost of disk and inference time.
- `MultilingualE5Large` (1024-dim, ~1.1 GB) — best quality.
- `BGEM3` — strong English performance, also multilingual.

After switching, **re-index**:

```bash
$ inkhaven --project ~/Books/sample-novel reindex
```

The new model reprocesses every paragraph (which is why we re-embed
on every save: future searches use the live model, not whatever was
indexed last week).

### `embeddings.chunk_size` and `chunk_overlap`

For very long paragraphs, the embedder splits the text into chunks of
~`chunk_size` characters with `chunk_overlap` fraction of overlap
between adjacent chunks. Smaller chunks → finer-grained similarity;
larger chunks → more context per vector. Defaults (800 / 0.15) are a
good baseline for prose.

## Discovery patterns

A few habits that pay off:

### Find by mood

Embedding search shines here. Try queries like:

- `a quiet pause in the action`
- `someone delivers bad news`
- `the protagonist doubts themselves`

You will find passages that fit even if you forgot the literal text.

### Find by character POV

Search for the character's name + a verb phrase:

- `Aragorn realises`
- `Frodo's hand`

This works because the embedder learns "this passage is about
Aragorn" semantically.

### Find a research note you took weeks ago

The Research and Notes system books are indexed alongside prose.
Search for the topic:

- `lighthouse keepers in 19th-century Norway`

Both prose and research show up in one ranked list.

### Find untitled / placeholder paragraphs

Search for `Untitled paragraph` — the placeholder title shows up in
results until you save (Inkhaven derives a real title from the first
sentence on first save). This is a useful "find drafts I forgot to
finish" query.

### Find a Place / Character entry

The Places and Characters system books are indexed too. Search for
the entry name (`Москва`, `Aragorn`); the entry's body comes back.
Useful when you want to read the lore without dropping out to the
Tree pane.

## What you have learned

- Saving a paragraph indexes it for both Tantivy (keyword) and HNSW
  (semantic vector) search automatically.
- `Ctrl+/` opens the Search bar; `Enter` runs the query; `Enter` on
  a result opens the paragraph.
- The results overlay shows score + kind + title-breadcrumb + snippet.
- CLI: `inkhaven search "<query>" [--limit N]`.
- Switching `embeddings.model` followed by `reindex` updates the
  search quality.
- Discovery patterns: search by mood, POV, research topic.

## Next steps

- [`05-ai-writing-assistant.md`](05-ai-writing-assistant.md) — using
  search results to feed the AI scope.
- [`07-places-and-characters.md`](07-places-and-characters.md) — how
  the lexicon overlays interact with search.
