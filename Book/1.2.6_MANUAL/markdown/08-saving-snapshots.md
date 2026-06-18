# 8 — Saving and snapshots

Saves are atomic. Snapshots are deliberate. The distinction matters: save commits your current paragraph to the on-disk file + database every time you press Ctrl+S; snapshot makes a versioned copy you can return to.

This chapter walks through both, plus the 1.2.6 annotation prompt that turns a wall of timestamp-only snapshots into a labelled revision history.

## F5 — create snapshot (with annotation)

`F5` (Editor scope) opens a small annotation prompt over the editor:

![figure: f5-annotation-prompt](images/f5-annotation-prompt.png) — F5: annotation prompt. Type a note describing this version, Enter commits, Esc cancels.

| Chord | What it does |
|-------|--------------|
| F5 | Open the annotation prompt + capture the current buffer. |
| Type | Build up the annotation text. |
| Enter | Commit the snapshot. |
| Enter (empty) | Commit without an annotation — same as the pre-1.2.6 path. |
| Esc | Cancel — no snapshot written. |

The buffer to snapshot is captured at prompt-open time, so you can keep typing in the editor while the prompt is open without changing what gets saved.

Annotations live in the snapshot's metadata, not the body. They show up in the F6 picker as italic cyan second lines under the row.

## F6 — the snapshot picker

`F6` lists every snapshot of the open paragraph:

![figure: f6-picker](images/f6-picker.png) — F6 picker. Annotated snapshots show their note on a second line; un-annotated ones are single-line.

| Chord | What it does |
|-------|--------------|
| ↑ / ↓ | Move cursor. |
| Home / End | Newest / oldest. |
| Enter | Pre-restore safety snapshot of the current buffer + restore the selected one. |
| V | Side-by-side diff: snapshot vs live buffer (see below). |
| D / Del | Confirm + delete the selected snapshot. |
| Esc | Close the picker. |

## Pre-restore safety

When you press Enter on a snapshot the system snapshots your current buffer FIRST (un-annotated), then loads the selected one. So you can always step back from a restore — the "undo" of a restore is another restore from the auto-saved pre-restore snapshot.

> **Safety net:** Inkhaven never overwrites your live work silently. If you've made changes since the last save, the auto-save before the restore captures them. If you restore the wrong snapshot, F6 → arrow keys to the new pre-restore entry → Enter recovers.

## V — side-by-side diff

`V` inside the F6 picker opens a snapshot diff view: left side is the snapshot, right side is your current buffer. Use it to sanity-check what you're about to restore.

![figure: snapshot-diff](images/snapshot-diff.png) — Snapshot diff (`V` in F6). Left = snapshot; right = current buffer. Coloured markers show insertions / deletions per line.

| Chord | What it does |
|-------|--------------|
| ↑ / ↓ / PgUp / PgDn | Scroll synchronised. |
| Esc | Back to the F6 picker. |

## When to annotate

Not every F5 fire deserves a note. The decision-point snapshots earn their labels:

- **Before a structural rewrite** — "before the lighthouse rewrite"
- **After a complete draft** — "first complete draft"
- **A version that landed well** — "version Maria loved"
- **A milestone** — "submission draft v1"
- **A risky experiment** — "trying second-person POV"

Skip annotations for the noisy mid-revision F5s. The un-annotated ones are still there; they just don't contribute label noise.

## Snapshot lifecycle

Snapshots are bdslib documents with `kind: "snapshot"` and a `parent_id` back-reference to the paragraph. They DO NOT appear in semantic search (we use `add_document_no_embed`) so a paragraph with 50 revisions doesn't pollute search results. They DO travel with backups (Chapter 11).

Deleting a paragraph deletes its snapshots automatically via the cascade in `Store::delete_subtree`.

## Recap

- `F5` snapshots with an optional one-line annotation. Empty annotation = un-annotated.
- `F6` picker; Enter restores; V diffs; D deletes; pre-restore safety snapshot fires automatically.
- Save and snapshot are separate concepts: save = current state to disk; snapshot = versioned copy.
- Snapshots stay out of semantic search; they DO travel with backups.
