# 33 — Navigation history

Two 1.2.7 ergonomics chords for jumping back to where you
were:

```
Alt+←         step backward through visited paragraphs
Alt+→         step forward (after stepping back)
Ctrl+V Shift+P  recent-paragraph picker
```

Both share one underlying ring — every time you open a
paragraph in the primary editor, the ring records the
transition. Browser-style back / forward.

## How the ring fills

Any of these actions push an entry onto the visited stack:

- **Enter** on a tree row.
- **Click** on a tree row (with mouse capture on).
- **Ctrl+V P** fuzzy paragraph picker → Enter.
- **Ctrl+V B / M** bookmark picker → Enter.
- **Paragraph link follow** via `Ctrl+V K` backlinks / `L` outgoing.
- **F6 snapshot picker** → Enter (snapshot restore).
- **Ctrl+V S** similar-paragraph picker → primary pane.
- **Ctrl+V t** timeline view → Enter on event → linked paragraph.
- **Ctrl+B U** undelete (restored paragraph becomes current).

Entries are deduped against the immediate predecessor — opening
the same paragraph twice in a row records only one entry. The
ring caps at the most-recent 32 entries; older entries roll
off.

## Stepping

```
Alt+←   pops the visited stack, opens the previous paragraph,
        pushes the current one onto the "forward" stack.
Alt+→   inverse: pops the forward stack, opens it, pushes the
        current onto visited.
```

So the typical "jump → read → come back" pattern is:

```
Reading paragraph A in tree pane.
Enter on B in tree → reading B.   visited: [A]
Alt+←                              reading A again.   forward: [B]
Alt+→                              reading B again.   visited: [A]
```

Opening a NEW paragraph (via Enter / picker / paragraph link)
**clears the forward stack** — same as in a web browser. If
you Alt+← back to A and then Enter on C, the path is now
`A → C` and B is gone from history.

## The recent picker (Ctrl+V Shift+P)

Stepping one entry at a time is slow if you bounced through
six paragraphs. `Ctrl+V Shift+P` opens a modal listing the
most-recent 32 paragraphs in **most-recent-first** order:

```
┌─ Recent paragraphs ──────────────────────────────────────┐
│                                                          │
│   ▶  Morning routine               aerin/chapter-2/      │
│      The grain shipment            aerin/chapter-2/      │
│      ✎ Sketch: harvest scene       aerin/notes/         │
│      Khaal's monologue             khaal/chapter-1/      │
│      Morning routine               aerin/chapter-2/      │← same row, deduped
│      Index page                    /                      │
│                                                          │
│   ↑/↓ select · Enter open · Esc cancel                   │
└──────────────────────────────────────────────────────────┘
```

The slug-path on the right is the navigational breadcrumb.
Hit Enter to jump.

The picker reads the same ring `Alt+←` does, so the order
matches: top = most recent, bottom = oldest within the ring's
32-entry cap.

## Where the ring lives

`App.visited_history: Vec<Uuid>` + `App.visited_cursor: usize`.
The ring **is persisted** to `.session.json` and restored on
launch — entries for paragraphs that no longer exist drop
out silently during restore, the cursor clamps to the
trimmed length. So `Alt+←` works across TUI restarts on the
same project.

Restart hygiene:
- Forward stack persists too — if you Alt+← back to A on
  Monday and quit, Tuesday's `Alt+→` returns to B (unless
  B was deleted Monday night).
- Per-project — the ring lives in the project's
  `.session.json`, so two different projects keep
  independent histories.
- Capped at the in-memory ring's natural depth; no
  separate disk-side cap to tune.

Last-active position also restores: the tree cursor row and
(if a paragraph was open at exit) the opened paragraph + its
cursor row/col all come back. The combined effect is "the
TUI opens where I left it" — no manual hunt through the tree
to resume work in a multi-book project.

## Use cases

- **Paragraph link bounce**. You're editing prose, follow a
  `[[Khaal's wedding day]]` link to its page, scan, want to
  go back: `Alt+←`.
- **Compare two paragraphs without losing your place**.
  Ctrl+V S to open a similar paragraph in the secondary pane;
  Ctrl+V S again to flip back to your primary editor view;
  `Alt+←` jumps the primary pane back to the original.
- **"I was reading something half an hour ago"**.
  `Ctrl+V Shift+P` and pick from the list — faster than
  hunting the tree.

## See also

- [`19-wiki-links.md`](19-wiki-links.md) — outgoing /
  incoming paragraph link chords (the most common reason the ring
  fills up).
- [`21-navigation.md`](21-navigation.md) — fuzzy paragraph
  picker (Ctrl+V P), bookmarks (Ctrl+V B / M).
- [`16-similar-paragraphs.md`](16-similar-paragraphs.md) —
  vector-similarity picker that also writes to the ring.
