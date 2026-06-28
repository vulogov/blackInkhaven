# Tutorial 96 — The Manuscript Outline

*Inkhaven 1.4.13*

The side Tree pane is a navigator — narrow, always a click away, good for
jumping to a paragraph. But when you want to **restructure** a book — drag a
scene to another chapter, reorder beats, promote an aside into its own
section — a thin sidebar fights you. OUTLINE-1 adds a **full-screen, foldable
Outline** of the whole manuscript, with structural editing built in.

## Open it

Press **`Ctrl+2`**. If your terminal swallows that (some re-encode it as NUL
or `Ctrl+@`), use the reliable backup **`Ctrl+B Shift+O`**. (`Ctrl+T` still
focuses the side Tree pane.)

The Outline opens with Books and Chapters expanded and everything deeper
folded — a structural overview you drill into. Its view state (what's open,
where the cursor is, the filter) is saved per project to
`.inkhaven/outline-state.json`, so it reopens exactly where you left it.

When the window is wide enough, a **detail panel** on the right shows the
cursor node's breadcrumb, status, word count vs target, tags, and
last-modified date.

## Navigate and fold

```
j / k   (or ↓ / ↑)   move
g / G                first / last row
Enter / l / →        expand a branch — or step into an open one
h / ←                collapse a branch — or step out to the parent
Space                toggle a fold without moving
```

## Restructure

```
Shift+J / Shift+K    reorder — swap with the next / previous sibling
> / <                demote / promote one nesting level
```

Reorder swaps two siblings (renaming their `NN-` filesystem prefixes for
you). **Promote** (`<`) lifts a node up a level — appending it under its
grandparent; **demote** (`>`) nests it into the preceding sibling. These work
on childless nodes (paragraphs and leaves); branch restructuring stays in the
Tree pane. If a move would break the hierarchy rules (say, a paragraph
directly under a book that has no valid grandparent), the manuscript is left
untouched and the status line tells you why.

## Copy and move a paragraph across chapters

This is the headline. A shared clipboard spans the Outline **and** the Tree
pane:

```
y   copy the cursor paragraph
m   move (cut) it
f   affix it — as the last child of the target's effective parent
```

Put the cursor on a paragraph, press `m`. Navigate to another chapter (or a
paragraph inside it), press `f` — the paragraph relocates there. Press `y`
instead of `m` and `f` *duplicates* it (fresh uuid, prose metadata carried —
tags, status, target, links — but not any timeline event, so you never mint a
duplicate event). A copied paragraph stays on the clipboard, so you can `f`
it into several places.

The footer shows what's held: `[move: harbour-scene]`.

## Filter

Press **`/`** and type. The Outline collapses to the **path-to-match tree** —
every node whose title or slug matches, plus its ancestors — so you see the
matches in context. Matching is case-insensitive and Unicode-aware (it works
in Russian, French, German…). `Enter` applies; `Esc` is staged — first it
exits editing, then clears the filter, then saves and closes the pane.

## From the terminal and from Bund

Everything has CLI parity:

```sh
inkhaven outline                       # print the tree as text
inkhaven outline --filter harbour      # only matching rows
inkhaven paragraph move tale/one/storm tale/two
inkhaven paragraph copy tale/one/storm tale/two
```

`src` / `dest` are the slash-separated slug paths printed by `inkhaven
outline`. The destination is a branch to nest into, or a paragraph to land
alongside.

Bund scripts get the same reach: `ink.outline.print` ( `-- text` ) pushes the
outline, and `ink.outline.paragraph_copy` / `ink.outline.paragraph_move`
( `src dest -- …` ) do the relocation (these mutate the store, so they need
the `store_write` category enabled).

The pane, the Tree clipboard, the CLI, and the Bund words all run on the same
filesystem-aware store primitives — so however you reach for it, a move is a
move.
