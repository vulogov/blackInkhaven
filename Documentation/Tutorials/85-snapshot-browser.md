# Tutorial 85 — The project-wide snapshot browser

*Inkhaven 1.3.36*

[Tutorial 20](20-snapshot-diff.md) gave snapshots a real
revision-control loop — but only one paragraph at a time. You
had to *be in* a paragraph to see its history. 1.3.36 lifts
the lid: **Ctrl+F6** opens a single browser over every
snapshot in the whole project, newest first, and lets you jump
straight to the paragraph that owns any of them.

## Recap — the per-paragraph flow (Tutorial 20)

Nothing here changes. To stay oriented:

- **F5** (also **Ctrl+B N**) snapshots the open paragraph.
- **F6** opens the snapshot picker for the **open** paragraph:
  `↑↓` navigate, **Enter** loads (taking a pre-restore safety
  snapshot first), **V** diffs the snapshot against the live
  buffer, **D** / **Del** deletes, `/` filters by annotation.

The new browser is that same engine, just widened to the whole
project. Load, restore, and pin still live in the F6 picker —
the browser is how you *find* the row you want.

## Open the browser — Ctrl+F6

Press **Ctrl+F6** from any pane. You don't need a paragraph
open, and you don't need to be anywhere in particular — the
browser is global. It lists every snapshot across **all**
paragraphs, newest first, one row each:

```
┌── Snapshots · all paragraphs ──────────────────────────────────┐
│ 2026-06-19 14:23:11 -0600  421w  The storm    The storm scene   │
│ 2026-06-19 11:02:48 -0600  388w  The storm    Earlier draft     │
│ 2026-06-18 16:40:09 -0600  205w  Harbour mist Opening sketch    │
│ 2026-06-18 09:11:55 -0600  312w  The storm    Original sketch   │
│ 2026-06-17 22:05:31 -0600  140w  Chapter head Working title     │
│                                                                  │
│ ↑↓ move · / filter · V diff vs current · Enter open paragraph   │
│ Esc close                                                        │
└──────────────────────────────────────────────────────────────────┘
```

Each row is **timestamp · words · paragraph title · annotation**.
Because it's flat and project-wide, you see the same paragraph's
history interleaved with everything else's, sorted purely by
time — handy for "what was I touching last Tuesday?"

## ↑↓ — move through the list

`↑` and `↓` move the cursor row. The view scrolls to follow the
cursor, so a project with hundreds of snapshots stays navigable —
the selected row never leaves the visible window.

## `/` — filter by paragraph title or annotation

Press `/` to start filtering. Type a fragment; the list narrows
in place to rows whose **paragraph title** *or* **annotation**
contains it. The match is case-insensitive, so `storm` finds
`The storm` and `Storm Watch` alike.

- **Enter** or **Esc** leaves the filter input (keeping the
  narrowed list — you're back to navigating rows).
- A **second Esc** closes the browser.

So Esc is layered: one to drop the filter, one to leave. This
mirrors the nested-Esc behaviour you already know from the F6
diff returning to its picker.

## V — diff the selected snapshot against its paragraph

Press **V** on a row and Inkhaven opens the **same** side-by-side
diff modal F6 uses — left pane the snapshot, right pane that
paragraph's **current** text:

```
┌── Diff · `The storm` · snapshot 2026-06-19 11:02:48 → current ──┐
│   = The storm                  │   = The storm                  │
│ ~ The wind came up at three.   │ ~ The wind came at three.      │
│ - It was the worst storm in    │                                │
│ - twenty years.                │ + Lightning cracked above the  │
│                                │ + foretop.                     │
│                                                                  │
│ ↑↓ / PgUp/PgDn / Home/End scroll · Esc back                     │
└──────────────────────────────────────────────────────────────────┘
```

The colour buckets (`=` dim, `-` red, `+` green, `~` fused
yellow) are exactly those documented in Tutorial 20 — it's the
identical diff path. The difference from F6 is only *which*
"current" you're comparing against: the diff resolves the
snapshot's own paragraph and diffs against **that** paragraph's
live text, even though you opened the browser from somewhere
else entirely.

**Esc** in the diff returns you to the **browser** — not all the
way out — so you can scan, diff, scan, diff without losing your
place in the list.

## Enter — open the paragraph and drop into F6

Press **Enter** on a row and Inkhaven opens that snapshot's
paragraph in the editor, then drops straight into its **F6
picker**. From there everything in Tutorial 20 is in reach:
**Enter** to load with the pre-restore safety snapshot, **V** to
diff against the live buffer, **Shift+Enter** to pin to the
secondary pane, **D** / **Del** to delete.

The browser deliberately does **not** restore or delete from the
project-wide view itself. Those are destructive, paragraph-local
acts — so the browser hands you off to the F6 picker, where the
safety-snapshot net (Tutorial 20) is already wired up, and lets
you act there with full context.

## Where it lives in the palette

The browser self-lists in the command palette (**Ctrl+V Space**,
[Tutorial 83](83-command-palette.md)) — search "snapshot" and
it's there alongside the per-paragraph commands, so you don't
have to remember **Ctrl+F6** to reach it.

## See also

- [`20-snapshot-diff.md`](20-snapshot-diff.md) — per-paragraph
  snapshots, the F6 picker, the V diff, and the pre-restore
  safety net the browser hands off to.
- [`83-command-palette.md`](83-command-palette.md) — the
  command palette that lists this browser among everything else.
