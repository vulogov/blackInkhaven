# 16 — Similar paragraphs: side-by-side editing

Long manuscripts grow self-referential. You write a battle scene
in chapter 3 and want to remember exactly how you described
"thunder" in chapter 1. Or you're rewriting a paragraph and need
the old version pulled up next to the new one for comparison.

Inkhaven 1.2.3 adds **similar-paragraph mode**: press `Ctrl+V` `S`
and a vector-similarity search ranks every other paragraph in
the project against your current buffer. Pick one and it loads
next to the open paragraph as a fully-editable second editor —
the AI pane steps aside for the duration.

## The workflow

1. Open a paragraph in the editor.
2. `Ctrl+V` `S`.
3. Inkhaven saves your buffer (so the search sees on-disk text)
   and shows the similar-paragraph picker:
   ```
    Similar paragraphs (12 hits)
      62%  Storm at sea     books/story/01-arrival/the-storm  · Lightning bleached the sky…
      48%  The first morning  books/story/02-aftermath/sunrise · Smoke still rose from the…
      …
    ↑↓ select · Enter open side-by-side · Esc cancel    (1/12)
   ```
4. `Enter` on a row.
5. The right pane (where the AI pane lived) becomes a second
   editor. Both panes show their full slug path as a dim footer.
6. `Tab` toggles keyboard focus between left and right editor.
7. Edit, navigate, save (`Ctrl+S`) in either pane independently.
8. `Ctrl+V` `S` again: inkhaven saves **both** buffers and
   drops the second editor. The AI pane returns.

## What "similar" means

The current paragraph's body is embedded with the same fastembed
model that powers `Ctrl+/` search, then the HNSW index returns
the nearest neighbours by cosine similarity. The result is:

- The current paragraph is filtered out (it would always top
  the list with score = 1.0).
- Only `Paragraph` nodes are surfaced — Notes / Places / Help
  content lives elsewhere.
- The score shown next to each title is the similarity (0..1,
  rendered as a percent).
- Roughly synonymous prose ranks high even with no shared
  vocabulary — that's the point of vector search.

The default cap is 20 hits. If none surface, the status bar
reports `no similar paragraphs found (need more indexed content)`
— check that paragraphs have actually been saved (`inkhaven
reindex` if the vector index drifted from disk).

## Editing in similar mode

While `self.secondary` is set:

| Chord     | Effect |
| --------- | ------ |
| `Tab` / `Shift+Tab` (inside Editor) | Toggle focus between left and right editor pane. |
| `Ctrl+S` | Save the focused editor's buffer. |
| `Ctrl+V` `S` | Exit similar mode: save **both** buffers, drop the secondary, AI pane returns. |
| `Esc` (in Tree) | Same as before — cycles focus normally. |

The keyboard always targets the focused pane. Under the hood the
implementation swaps `App.opened ↔ App.secondary` on every Tab,
so every existing editor handler keeps working unchanged — it
just operates on "whichever doc the user is currently editing".

## Slug-path footers

In similar mode each editor pane carves off its bottom row to
show the full slug path of the doc inside, e.g.:

```
┌── The storm  ─────────────────────┐  ┌── Lightning ·  (similar)  ─────────────┐
│  = The storm                      │  │  = Lightning over the mast            │
│                                   │  │                                        │
│  The wind came at three.          │  │  The first bolt struck the foretop.    │
│  …                                │  │  …                                     │
│ story/01-arrival/the-storm        │  │ story/01-arrival/lightning            │
└───────────────────────────────────┘  └───────────────────────────────────────┘
```

The footer is dim and never highlights, so it doesn't compete
with the focused-pane border.

## Saving — what about autosave?

The primary doc still autosaves on idle, just like normal mode.
The **secondary doc does not autosave** in 1.2.3 — to flush it,
either press `Ctrl+S` while focused on the right pane, or exit
similar mode with `Ctrl+V` `S` (which saves both).

This is intentional: the secondary is meant for read-mostly
side-by-side comparison; if you want full editing parity with
the primary, save explicitly.

## Common workflows

### Comparing two passes of the same scene

```
1. Open the new version.
2. Ctrl+V S → pick the older snapshot's matching paragraph.
3. Tab between the two; copy / paste / cherry-pick lines.
4. Ctrl+V S exits — both buffers are saved.
```

### Discovering forgotten prior art

```
1. You're about to write a sea-storm scene.
2. Sketch a one-paragraph stub.
3. Ctrl+V S — see if a similar storm exists already.
4. If yes, decide whether to merge / reference / reuse imagery.
```

### Editing two paragraphs that influence each other

```
1. Open paragraph A.
2. Ctrl+V S → pick paragraph B (which depends on A).
3. Edit A on the left; check the impact on B by tabbing right
   and adjusting.
4. Ctrl+V S exits with both saved.
```

## Limits

- Single secondary pane in v1 — no three-up split.
- Read-only Help-book paragraphs follow the same rules they do
  in the primary editor (modifications blocked at the textarea
  layer).
- The picker doesn't paginate beyond 20 hits. If you need to
  search deeper, use `Ctrl+/` for full semantic search and then
  open paragraphs into the editor manually.

## See also

- [`04-search-and-discovery.md`](04-search-and-discovery.md) —
  the underlying semantic search.
- [`03-the-editor.md`](03-the-editor.md) — focus / Tab / save
  semantics in the editor pane.
