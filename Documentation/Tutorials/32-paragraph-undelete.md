# 32 — Paragraph undelete

Inkhaven 1.2.7 adds a single-slot kill-ring for paragraph
deletions. `Ctrl+B U` restores the most-recently deleted
paragraph at its original position, with body, tags, status,
linked-paragraph cross-refs, and event timing intact.

The slot is **single-paragraph only** — branch deletes (a
chapter or book) are too consequential to undo without
explicit store-level support, so those clear the slot when
they fire.

## The flow

```
1. Cursor in tree pane on the paragraph you want gone.
2. Ctrl+D  →  delete-confirm modal.
3. Enter   →  paragraph removed; status bar reads:
                deleted paragraph `morning` (0 other nodes removed) ·
                Ctrl+B U to restore (new uuid — wiki-links to
                old id stay broken)
4. Realise you wanted that prose after all.
5. Ctrl+B U →  paragraph re-created at the same position;
                status bar reads:
                ↺ restored `morning` (new uuid abc123… —
                cross-refs to old uuid stay broken)
```

## What gets restored

The kill-ring stash captures everything needed to round-trip
the paragraph through `create_node` + `update_metadata`:

| Field | Restored | Notes |
| --- | --- | --- |
| `title` | ✓ | Verbatim. |
| `slug` | ✓ | File lands at the same on-disk path. |
| `content` (body bytes) | ✓ | Written via the same path `save_current` uses. |
| `tags` | ✓ | Project-wide tag set is consulted again on apply. |
| `linked_paragraphs` | ✓ | The outgoing wiki-links the restored paragraph held. |
| `status` | ✓ | Napkin → Ready ring position preserved. |
| `target_words` | ✓ | Per-paragraph goal preserved. |
| `content_type` | ✓ | `hjson` / `typst` flag preserved. |
| `event` data | ✓ | Calendar timing + track + precision + linked paragraphs (timeline). |

The new paragraph gets a **fresh uuid**. Wiki-links from
elsewhere that pointed at the OLD uuid stay broken; the status
line is upfront about this. If you need the same uuid restored
(e.g. to keep an incoming wiki-link working), use the
snapshot-restore flow on the parent branch instead — that path
preserves identity but operates at branch granularity.

## What clears the slot

- A subsequent single-paragraph delete (the new delete replaces
  the slot — only the most recent kill is restorable).
- Any branch delete (chapter / subchapter / book). Branch
  deletes can't be undone, so leaving an older single-¶ undo
  on offer would mislead.
- TUI restart. The kill-ring is session-local — it lives in
  `App.last_deleted: Option<DeletedParagraphStash>` and isn't
  persisted to `.session.json`.

## Position handling

The stash records both the original parent and the immediate
sibling-before. Restore uses:

```
InsertPosition::After(anchor_id)    when an anchor sibling exists
InsertPosition::End                 when the paragraph was first child
```

So a paragraph deleted from the middle of a chapter lands back
in the same slot; a paragraph that was the first child of its
parent goes to the end of the (now-shorter) child list. The
second case is asymmetric on purpose — restoring the very-first-
child position cleanly requires a `Before(first_sibling)` shape
the store API doesn't expose today.

## Use cases

- **Accidentally deleted the wrong paragraph in tree pane**.
  Hit Ctrl+B U before doing anything else.
- **Restructuring**: cut a paragraph from one chapter, navigate
  to another chapter, then restore. The new uuid means
  incoming wiki-links break; for restructure where you care
  about identity, use the Ctrl+V A link-pick flow plus
  re-targeting the link by hand.
- **Recovering from over-aggressive Ctrl+B Shift+1..7 status
  cycling triggering a delete**: doesn't apply — status
  cycling never deletes. The kill-ring fires for the explicit
  Ctrl+D modal flow only.

## Limits — by design

- **Single-slot.** Hit Ctrl+B U twice in a row: the second
  press is a no-op because the slot is now empty (we already
  used it). The status reads `nothing to restore`.
- **No branch undo.** Deleting a chapter is final; the slot
  is cleared. Use snapshots (F5 to annotate at risk, F6 to
  restore) for branch-level safety.
- **No cross-restart memory.** Restart the TUI and the slot
  is gone. For longer-term recovery, lean on the
  per-paragraph snapshot history (F6) or the project-level
  zip backup (`Ctrl+B Shift+B` or the on-exit auto-backup).

## See also

- [`20-snapshot-diff.md`](20-snapshot-diff.md) — F6 snapshot
  picker, the larger-grained recovery surface.
- [`29-snapshot-annotations.md`](29-snapshot-annotations.md) —
  F5 annotation prompt for naming recovery points before risky
  edits.
- [`10-backups-and-recovery.md`](10-backups-and-recovery.md) —
  project-level zip backups and the restore flow.
