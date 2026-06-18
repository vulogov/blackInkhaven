#import "../design.typ": *

#chapter(number: 8, part: "Part II — The Editor",
  title: "Saving and snapshots")

#dropcap("S")aves are atomic. Snapshots are deliberate. The
distinction matters: save commits your current paragraph
to the on-disk file + database every time you press
Ctrl+S; snapshot makes a versioned copy you can return to.

This chapter walks through both, plus the 1.2.6 annotation
prompt that turns a wall of timestamp-only snapshots into
a labelled revision history.

#section("F5 — create snapshot (with annotation)")

`F5` (Editor scope) opens a small annotation prompt over the
editor:

#figure_slot(
  id: "f5-annotation-prompt",
  caption: "F5 — annotation prompt. Type a note describing this version, Enter commits, Esc cancels.",
  height: 30mm,
)

#chord_table((
  chord_row("F5", "Open the annotation prompt + capture the current buffer."),
  chord_row("Type", "Build up the annotation text."),
  chord_row("Enter", "Commit the snapshot."),
  chord_row("Enter (empty)", "Commit without an annotation — same as the pre-1.2.6 path."),
  chord_row("Esc", "Cancel — no snapshot written."),
))

The buffer to snapshot is captured at prompt-open time, so
you can keep typing in the editor while the prompt is open
without changing what gets saved.

Annotations live in the snapshot's metadata, not the body.
They show up in the F6 picker as italic cyan second lines
under the row.

#section("F6 — the snapshot picker")

`F6` lists every snapshot of the open paragraph:

#figure_slot(
  id: "f6-picker",
  caption: "F6 picker. Annotated snapshots show their note on a second line; un-annotated ones are single-line.",
  height: 60mm,
)

#chord_table((
  chord_row("↑ / ↓", "Move cursor."),
  chord_row("Home / End", "Newest / oldest."),
  chord_row("Enter", "Pre-restore safety snapshot of the current buffer + restore the selected one."),
  chord_row("V", "Side-by-side diff: snapshot vs live buffer (see below)."),
  chord_row("D / Del", "Confirm + delete the selected snapshot."),
  chord_row("Esc", "Close the picker."),
))

#section("Pre-restore safety")

When you press Enter on a snapshot the system snapshots your
current buffer FIRST (un-annotated), then loads the selected
one. So you can always step back from a restore — the
"undo" of a restore is another restore from the auto-saved
pre-restore snapshot.

#callout(label: "Safety net")[
  Inkhaven never overwrites your live work silently. If
  you've made changes since the last save, the auto-save
  before the restore captures them. If you restore the
  wrong snapshot, F6 → arrow keys to the new pre-restore
  entry → Enter recovers.
]

#section("V — side-by-side diff")

`V` inside the F6 picker opens a snapshot diff view: left
side is the snapshot, right side is your current buffer.
Use it to sanity-check what you're about to restore.

#figure_slot(
  id: "snapshot-diff",
  caption: "Snapshot diff (`V` in F6). Left = snapshot; right = current buffer. Coloured markers show insertions / deletions per line.",
  height: 55mm,
)

#chord_table((
  chord_row("↑ / ↓ / PgUp / PgDn", "Scroll synchronised."),
  chord_row("Esc", "Back to the F6 picker."),
))

#section("When to annotate")

Not every F5 fire deserves a note. The decision-point
snapshots earn their labels:

- #strong[Before a structural rewrite] — "before the lighthouse rewrite"
- #strong[After a complete draft] — "first complete draft"
- #strong[A version that landed well] — "version Maria loved"
- #strong[A milestone] — "submission draft v1"
- #strong[A risky experiment] — "trying second-person POV"

Skip annotations for the noisy mid-revision F5s. The
un-annotated ones are still there; they just don't
contribute label noise.

#section("Snapshot lifecycle")

Snapshots are bdslib documents with `kind: "snapshot"` and
a `parent_id` back-reference to the paragraph. They DO NOT
appear in semantic search (we use `add_document_no_embed`)
so a paragraph with 50 revisions doesn't pollute search
results. They DO travel with backups (Chapter 11).

Deleting a paragraph deletes its snapshots automatically
via the cascade in `Store::delete_subtree`.

#recap((
  [`F5` snapshots with an optional one-line annotation. Empty annotation = un-annotated.],
  [`F6` picker; Enter restores; V diffs; D deletes; pre-restore safety snapshot fires automatically.],
  [Save and snapshot are separate concepts: save = current state to disk; snapshot = versioned copy.],
  [Snapshots stay out of semantic search; they DO travel with backups.],
))
