# 21 — Navigation pack: bookmarks, fuzzy finder, AI prompt history

Inkhaven 1.2.4 adds four navigation features that turn "find
my stuff fast" from "scroll the tree until you spot it" into
muscle-memory chords. Together they make a project with 100+
paragraphs feel as fluid as one with 10.

## Ctrl+V P — fuzzy paragraph picker

The big one. Press `Ctrl+V P` from anywhere. A modal opens:

```
┌── Find paragraph (8/42) ──────────────────────────────────┐
│ › sto│                                                    │
│  The storm     story/01-arrival/the-storm                 │
│  Storm at sea  story/02-passage/storm-at-sea              │
│  Bell tower    story/01-arrival/bell-tower (matched: sto) │
│  ...                                                       │
│ ↑↓ select · Enter opens · Esc closes                       │
└────────────────────────────────────────────────────────────┘
```

Type any substring of the title or slug-path; the list
narrows in real time. Score order:

1. Title starts with the query
2. Title contains the query
3. Slug path contains the query
4. (excluded)

Stable sort, so within each tier the original ordering by
slug path is preserved.

System books (Help / Scripts / Typst / …) are filtered out —
the picker only shows your manuscript paragraphs.

**Enter** routes through the same `open_search_result` flow
the search-overlay uses: autosaves the previous buffer, moves
the tree cursor onto the target row, opens the picked
paragraph in the editor.

## Ctrl+V B — toggle bookmark

Bookmark the open paragraph. Status bar reports:

```
bookmark added: `The storm`
```

Press it again to remove. Bookmarks are a flag on the node
metadata (`Node.bookmark: bool`), survive restart, and persist
through `inkhaven export`.

## Ctrl+V M — bookmark picker

Open the floating "bookmarks" pane listing every bookmarked
paragraph in the project, sorted by title:

```
┌── Bookmarks (5) ─────────────────────────────────────────┐
│  ★ Bell tower   story/01-arrival/bell-tower               │
│  ★ Lightning    story/01-arrival/lightning                │
│  ★ The storm    story/01-arrival/the-storm                │
│  ★ Three weeks  story/03-aftermath/three-weeks            │
│                                                            │
│ ↑↓ select · Enter opens · D removes bookmark · Esc closes │
│ (1/4)                                                      │
└────────────────────────────────────────────────────────────┘
```

- **Enter** — open the chosen paragraph (autosaves prev).
- **`D`** / `Delete` — clear the bookmark; the row drops out
  of the list.
- **Esc** — close.

Sorted by title (not by recency or slug) so the visual
position of a bookmark stays stable as you add new ones.
Use when you want guaranteed-reachable jump points; use the
fuzzy picker when you know what you're looking for.

## AI prompt history — Up / Down

In the AI prompt input (Ctrl+I), `Up` arrow recalls the last
sent prompt. Continue pressing `Up` to step further back; `Down`
moves forward. Down past the newest entry clears the input.

The history caps at 500 entries — past that, the oldest are
trimmed. Consecutive sends of the same text don't duplicate.

Any edit (typed character, Backspace, Delete) clears the
history cursor, so the next Up arrow starts at the newest
entry again.

This is the shell-style behaviour most users assume — its
absence in 1.2.3 was an oversight.

## Slash-command expansion in AI prompt

Type `/` in the AI prompt to open the prompt-library picker.
Continue typing to filter. 1.2.4 upgrades the ranking from
naïve substring to prefix-prioritized:

| Match               | Score |
|---------------------|-------|
| name starts with query | 3 |
| description word-starts with query | 2 |
| name or description contains query | 1 |

So `/sum` ranks the `summarize` prompt above any prompt that
just happens to contain "sum" mid-word. `/ch` ranks `charact…`,
`chapter…`, `check…` ahead of mid-word matches.

System prompts (from `prompts.hjson`) come before book
prompts (paragraphs under the `Prompts` system book) when
scores tie.

`Tab` or `Enter` on the highlighted entry commits — replaces
the input with the rendered prompt template, ready to send
with another Enter.

## Putting them together

A typical writing session:

```
Ctrl+V P  → fuzzy-find "morning" → land on "The morning ritual"
... edit ...
Ctrl+V B  → bookmark "The morning ritual" for later
Ctrl+V P  → fuzzy-find "storm" → land on "Storm at sea"
... edit ...
Ctrl+I    → AI prompt
Up        → recall last "summarize this paragraph" prompt
Enter     → send
... read response ...
Ctrl+V M  → bookmarks picker → Enter on "The morning ritual" to jump back
```

No tree scrolling. Every jump is targeted.

## When to use what

| Goal | Tool |
|------|------|
| "I know roughly what the paragraph's title says" | Ctrl+V P (fuzzy) |
| "I'll be back to this paragraph repeatedly" | Ctrl+V B → Ctrl+V M |
| "Re-send the AI prompt I just sent with one tweak" | Up arrow → edit → Enter |
| "Run the same canned prompt by name" | `/<name>` in AI prompt |
| "I forgot the title but remember a phrase from inside" | Ctrl+/ (semantic search, not 1.2.4) |
| "Find a paragraph similar to the one I have open" | Ctrl+V S (similar mode — tutorial 16) |

## See also

- [`04-search-and-discovery.md`](04-search-and-discovery.md) —
  full semantic search via `Ctrl+/`.
- [`16-similar-paragraphs.md`](16-similar-paragraphs.md) —
  Ctrl+V S vector-similarity picker.
- [`05-ai-writing-assistant.md`](05-ai-writing-assistant.md) —
  the AI prompt and the prompt library.
