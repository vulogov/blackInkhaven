# 20 — Snapshot diff and the safety net

Inkhaven 1.2.4 adds two changes around the F6 snapshot picker
that turn it from "load and pray" into a real revision-control
loop:

1. **V key in the snapshot picker** opens a side-by-side diff
   of the chosen snapshot against the live buffer.
2. **Pre-restore safety snapshot** — pressing Enter (load)
   automatically snapshots the current buffer FIRST so the
   pre-restore state is recoverable.

## Open the snapshot picker

Press **F6** with a paragraph open. The picker shows every
snapshot for the current paragraph, newest first:

```
┌── Snapshots — The storm ──────────────────────────────────┐
│ 2026-05-21 14:23:11 -0600   421w   The storm scene as of   │
│ 2026-05-21 13:48:02 -0600   388w   Earlier draft           │
│ 2026-05-20 09:11:55 -0600   312w   Original sketch         │
│                                                            │
│ ↑↓ navigate · Enter loads · V diff vs current · D / Del   │
│ delete · Esc cancel                                        │
└────────────────────────────────────────────────────────────┘
```

## V — diff the cursor row against current

Press **V** on a row. The snapshot diff modal opens:

```
┌── Diff · `The storm` · snapshot 2026-05-21 13:48:02 → current ──┐
│   = The storm                  │   = The storm                  │
│                                │                                │
│ ~ The wind came up at three.   │ ~ The wind came at three.      │
│   The clouds drew tight.       │   The clouds drew tight.       │
│ - It was the worst storm in    │                                │
│ - twenty years.                │                                │
│                                │ + Lightning cracked above the  │
│                                │ + foretop. Three deck-hands    │
│                                │ + saw the bell move.           │
│                                                                 │
│ ↑↓ / PgUp/PgDn / Home/End scroll · Esc back (4/12)              │
└─────────────────────────────────────────────────────────────────┘
```

Left pane is the snapshot, right is the current buffer. Colour
buckets:

| Glyph | Meaning   | Colour      |
|-------|-----------|-------------|
| ` `   | Unchanged | dim         |
| `-`   | Removed (snapshot had, buffer doesn't) | red bold |
| `+`   | Added (buffer has, snapshot doesn't)   | green bold |
| `~`   | Changed — adjacent delete + insert fused into one row | yellow |

The diff is computed by the `similar` crate's Myers algorithm.
Consecutive Delete→Insert pairs fuse into a single Changed row
so a one-line rewrite renders as one yellow row instead of red
+ green on separate lines.

**Esc** in the diff closes the diff and returns you to the
snapshot picker — *not* all the way out. One more Esc closes
the picker. The picker is stashed in the diff modal's
`return_to` field for this exact one-Esc-back UX.

## Enter — load with safety net

When you press Enter on a snapshot row to restore it:

1. Inkhaven takes a fresh snapshot of the current editor buffer
   first.
2. Then replaces the buffer with the chosen snapshot's bytes.
3. Reports both events on the status bar:

   ```
   loaded snapshot from 2026-05-21 13:48:02 -0600 — bold marks
   the change vs saved · safety snapshot abcd1234 created
   ```

The safety snapshot lands in the same history list as the rest.
Hit F6 again and it's at the top with a fresh timestamp.

If the safety-snapshot creation itself fails (disk full, store
offline), the load **aborts entirely** — the whole point of
the net is data safety, so the editor buffer stays untouched
and a status message explains the cause.

## Recovery: undo a snapshot restore you didn't mean

```
1. Hit F6 again.
2. Top row is the safety snapshot you just made (newest
   timestamp).
3. Enter → restores it.
4. The "restore that you didn't want" is now itself a safety
   snapshot at the top of the list, so the chain stays intact.
```

You can flip back and forth between "what was there" and
"what the snapshot has" indefinitely. Every Enter writes a
new safety snapshot of whatever was in the editor at that
moment.

## Delete (`D`) — still works

`D` (or `Delete`) on a row removes the snapshot. No further
confirmation — snapshots are explicit creations (F5 / Ctrl+B
N) and the list is regenerated fresh after a delete. If you
deleted the wrong one, the safety-snapshot chain from any
recent Enter has you covered.

## How it pairs with similar-paragraph mode

Both flows have safety nets:

- **Similar-paragraph mode (Ctrl+V S → second editor)**: both
  the primary and secondary editors autosave on idle and on
  Ctrl+V S exit, so the right pane stops being a write-only
  black hole.
- **Snapshot restore (F6 Enter)**: pre-restore snapshot
  preserves the live buffer.

Combined: opening an older snapshot side-by-side, copying a
line you want back, dismissing the diff, then loading the old
snapshot fully — every step is recoverable.

## Implementation notes

* `Modal::SnapshotDiff { rows, scroll, return_to }` — the
  picker's modal is stashed in `return_to: Box<Modal>` so Esc
  pops back into it. Cleanest way to nest one modal inside
  another without losing the parent's state.
* `compute_line_diff` lives in `tui/app.rs`; uses
  `similar::TextDiff::from_lines`. Four unit tests cover
  identical / pure-add / pure-remove / fused-change.
* Safety snapshot uses the standard `Store::create_snapshot`
  path — fires `hook.on_snapshot` like any other.

## See also

- [`03-the-editor.md`](03-the-editor.md) — the split-edit `F4`
  mode is the lighter cousin: same-buffer historical view
  without leaving the editor.
- [`Bund/BUND_TUTORIAL.md`](../Bund/BUND_TUTORIAL.md) — write
  a `hook.on_snapshot` lambda if you want to mirror snapshots
  to git or another store.
