#import "../design.typ": *

#chapter(number: 6, title: "Snapshots and History")

The last chapter taught you to write into a paragraph; this one teaches you to
write *without fear*. Every long book is a graveyard of sentences the author
was glad to lose and a handful the author would give a finger to have back, and
the difference is never obvious in the moment you cut them. Inkhaven's answer is
to make going back cheap and reliable at three different grains: a versioned
history *inside* each paragraph, a project-wide index over *all* of it, and a
recovery layer beneath both that survives the tool crashing under you. Learn
this chapter and the delete key stops being a decision — it becomes a gesture
you can always take back.

#section("What a snapshot is")

A snapshot is the smallest unit of Inkhaven's memory: a frozen, dated copy of
one paragraph's prose, kept beside the paragraph but never confused with it.
When you take a snapshot the current bytes of the open buffer are written into
the project database as a document of their own, tagged with the paragraph they
came from, the moment they were taken, a word count, a one-line preview, and an
optional note. The paragraph goes on changing; the snapshot does not. It is a
photograph, not a mirror.

#term("Snapshot")[
  A *snapshot* is an independent, dated copy of a single paragraph's body,
  stored in the project database as its own document (`kind:"snapshot"`) with a
  `parent_id` back-reference to the paragraph it was cut from. It is not the
  live file, not an autosave, and not part of the search index — it is a
  point-in-time bookmark you can read, compare, and restore.
]

Three properties follow from that independence, and each one matters when you
need a snapshot most.

First, *snapshots outlive saves*. Autosave and `Ctrl+S` overwrite the paragraph
in place — they commit the *latest* version and keep no history. A snapshot is a
separate document, so saving the paragraph a thousand times never touches it.

Second, *snapshots outlive the paragraph*. Because a snapshot is not stored
inside the paragraph, deleting the paragraph does not delete its snapshots; they
remain in the database, findable, as a recovery hatch when a whole branch goes.
This is the seam that connects the two halves of this chapter — the versioning
layer and the deletion layer meet here.

Third, *snapshots are never collected*. Inkhaven does not garbage-collect them,
expire them, or cap them. A paragraph you have revised for a year carries its
whole revision history, oldest to newest, until you delete a row by hand. They
cost only disk, and disk at literary scale is free.

#callout(label: "Not in search")[
  Snapshots are deliberately excluded from the vector index — they carry no
  embedding and never surface in semantic search or the AI's grounding. A
  history of forty drafts of one paragraph would otherwise drown the one live
  version in every "find similar" result. Snapshots are a versioning surface,
  not a corpus.
]

#section("When snapshots are taken")

You take snapshots two ways: deliberately, with a keystroke, and automatically,
whenever Inkhaven is about to do something to your prose that you might regret.
The second kind is the one that saves manuscripts, because it fires exactly at
the moments you are least likely to think of it yourself.

The *manual* trigger is a single key, covered in the next section. The
*automatic* triggers all share one rule — *nothing rewrites your prose without a
restorable copy landing first* — and there are five of them:

- *Before an AI rewrite is applied.* When you accept a diff from any AI rewrite
  — the sentence-rhythm rewrite, an Editorial Pass fix, a reconciliation — the
  pre-rewrite paragraph is snapshotted before the new text replaces it, under an
  annotation naming the rewrite. Reject the diff and nothing is written at all.

- *Before a project-wide find-and-replace.* Each paragraph a book-scope replace
  touches is snapshotted first, annotated `replace: X → Y`, so `F6` is the undo
  for a substitution that went wider than you meant.

- *Before a branch is deleted.* Deleting a chapter, subchapter, or book
  snapshots *every paragraph leaf* inside it first, annotated `pre-delete`, so
  any single paragraph from a vanished branch can be found and restored even
  though the branch itself is gone for good.

- *Before a snapshot restore.* Loading an old snapshot over the live buffer
  first snapshots the live buffer — the *pre-restore safety net*, described
  below. Restoring is itself a restorable act.

- *On demand from a Bund script.* The `hook.on_snapshot` hook fires on every
  snapshot however it was taken, so a script can mirror snapshots to git or
  stamp them with the paragraph's workflow status.

#callout(label: "The through-line")[
  Read the list once more and notice what it guarantees: there is *no path* in
  Inkhaven that overwrites or destroys prose without a dated, restorable copy
  existing first. Manual `F5` is the belt; the five automatic triggers are the
  braces. You never have to remember to protect yourself before a risky edit —
  the tool has already done it.
]

#section("Taking a snapshot — F5 and the annotation prompt")

Press `F5` (or its chord form, `Ctrl+B N`, for *new* snapshot) with a paragraph
open. Rather than snapshot silently, Inkhaven floats a one-line *annotation
prompt* — a chance to label this version with a note you will thank yourself for
later.

#screen(caption: "F5 — the annotation prompt over the editor")[```
┌─ Snapshot annotation · F5 ──────────────────────────────┐
│                                                         │
│  Snapshot `the-quay` — annotation:                      │
│  › before the lighthouse rewrite▏                       │
│                                                         │
│  Enter commits (empty = no note) · Esc cancels          │
└─────────────────────────────────────────────────────────┘
```]

The prompt is forgiving in both directions. Type a note and press `Enter` to
commit a *labelled* snapshot; press `Enter` on an empty line to commit an
*unlabelled* one — the old reflex of "F5, Enter" still fires as fast as it ever
did. Press `Esc` to cancel, and *no snapshot is written at all*.

One subtlety earns its keep: the buffer to be snapshotted is captured the
*instant the prompt opens*, not when you press `Enter`. You can keep typing in
the editor behind the prompt — or take your time composing the note — without
changing what gets saved. The annotation lives in the snapshot's metadata, never
in its body, so it never contaminates the prose you are versioning.

#subsection("When to annotate")

You do not need a note on every snapshot, and most of yours will not have one. A
label earns its place on the *decision-point* snapshots — the versions you might
actually want to walk back to:

- Before a structural rewrite — `before the lighthouse rewrite`.
- After a milestone draft — `first complete draft, before pass 1`.
- A version that landed — `the version Maria loved`.
- A risky experiment — `trying second-person POV`.

Annotations are what turn a long history from a wall of identical timestamps
into a labelled list of states you can scan. They are how you find "the good
one" three weeks later without reading every row.

#section("The per-paragraph picker — F6")

Press `F6` with a paragraph open and Inkhaven
lists every snapshot of *that* paragraph, newest first. This is the workbench
for one paragraph's past — where you read it, diff it, restore it, and prune it.

#screen(caption: "F6 — the snapshot picker for the open paragraph")[```
┌─ Snapshots · the-quay ──────────────────────────────────┐
│ › 2026-06-19 14:23:11   421w   The rain came sideways…  │
│       ✎ after the lighthouse rewrite                    │
│   2026-06-19 11:02:48   388w   The rain came sideways…  │
│       ✎ first complete draft                            │
│   2026-06-18 16:40:09   205w   She counted the lamps…   │
│                                                         │
│ ↑↓ move · Enter load · V diff · D delete · / filter     │
│ Shift+Enter pin to split · Esc close                    │
└─────────────────────────────────────────────────────────┘
```]

Every row shows a timestamp, the snapshot's word count, and a preview of its
first prose line. Annotated snapshots carry the note on a second indented line
under a `✎` glyph, so labelled rows stand out from the unlabelled ones without
your reading a single preview. The keys are:

#chord_table((
  chord_row("↑ ↓", "Move the cursor row through the history."),
  chord_row("Home / End", "Jump to the newest / oldest snapshot."),
  chord_row("Enter", "Load the selected snapshot into the buffer (safety-snapshotting the live buffer first)."),
  chord_row("V", "Side-by-side diff of the snapshot against the live buffer."),
  chord_row("D / Del", "Delete the selected snapshot."),
  chord_row("/", "Filter the list by annotation text; Esc leaves the filter, keeping the query."),
  chord_row("Shift+Enter", "Pin the snapshot into the split-view secondary pane, read-only."),
  chord_row("Esc", "Close the picker."),
))

The `/` filter narrows the visible rows to those whose annotation contains what
you type — a case-insensitive substring match. It is filter-focus: typed
characters build the query, `Backspace` edits it, and `Esc` exits the filter
back to row navigation while keeping the narrowed list. The filter resets each
time you open `F6`, so a previous session's query never haunts the next picker.

#subsection("Viewing a diff — V")

Reading a raw old version tells you little; seeing *what changed* tells you
everything. Press `V` on a row and Inkhaven opens a side-by-side diff — the
snapshot on the left, the paragraph's current text on the right.

#screen(caption: "V — the snapshot diff, snapshot left, current right")[```
┌─ Diff · the-quay · snapshot 11:02:48 → current ─────────┐
│  = The rain came sideways      │  = The rain came        │
│  ~ off the harbour at three.   │  ~ off the harbour.     │
│  - It was the worst storm      │                         │
│  - in twenty years.            │                         │
│                                │  + Lightning cracked    │
│                                │  + above the foretop.   │
│                                                          │
│ ↑↓ PgUp/PgDn Home/End scroll · Esc back (4/12)           │
└──────────────────────────────────────────────────────────┘
```]

The diff is computed with the Myers algorithm and sorted into four colour
buckets: unchanged lines are dim, lines *removed* since the snapshot are red,
lines *added* since are green, and a one-line rewrite — a delete and an insert
that sit adjacent — is *fused* into a single yellow "changed" row rather than
shown as red-then-green. The result reads the way you think about an edit.

`Esc` in the diff returns you to the picker, *not* all the way out — the picker
is stashed underneath so you can scan a row, diff it, scan the next, diff that,
without ever losing your place in the list. One more `Esc` closes the picker.

#subsection("Restoring an old version — Enter")

Press `Enter` on a row to load that snapshot's bytes into the editor. The buffer
is replaced and marked *dirty* — nothing is committed to disk until you save, so
a restore is a proposal you can still walk away from. The status bar reports
both the load and the safety snapshot it took first.

Because every restore leaves the live buffer dirty rather than saved, and
because every restore first snapshots what *was* there, you can flip back and
forth between "what the snapshot has" and "what was in the editor" indefinitely,
comparing them in place, until one of them is the one you save.

#subsection("The pre-restore safety net")

The single most important thing `F6` does happens where you cannot see it. When
you press `Enter` to restore, Inkhaven first takes a fresh snapshot of whatever
is currently in the editor — *then* replaces it. Without this, restoring an old
version would silently discard any unsaved typing. With it, the state you were
about to lose becomes one more row at the top of the history.

#callout(label: "Safety before restore, always")[
  If that pre-restore snapshot cannot be written — the disk is full, the store
  is offline — the restore *aborts entirely* and the buffer is left untouched.
  The whole point of the net is data safety, so Inkhaven would rather do nothing
  than do the replace without the copy. Fix the underlying cause and try again.
]

This is also the undo for a restore you did not mean. Pressed `Enter` on the
wrong row? Press `F6` again: the safety snapshot of your real work is now the
top row, newest timestamp. `Enter` on it and you are back where you started —
and *that* restore made its own safety snapshot, so the chain never breaks.

#subsection("Deleting a snapshot — D")

`D` (or `Del`) removes the selected snapshot from the history. There is no
confirmation dialog: snapshots are explicit creations, the list regenerates
fresh after a delete, and the safety-snapshot chain from any recent restore has
you covered if you prune the wrong one. Use it to keep a decision-heavy
paragraph's history readable — but you rarely need to, since nothing forces you
to look at the old rows.

#section("The project-wide snapshot browser — Ctrl+F6")

The `F6` picker answers "what has this paragraph been?" The browser answers a
different question — "what was I touching last Tuesday?" — because it shows every
snapshot in the *whole project* at once, one flat list, newest first, regardless
of which paragraph each came from. Press `Ctrl+F6` from *any* pane; you need no
paragraph open and no particular place in the tree.

#screen(caption: "Ctrl+F6 — every snapshot in the project, newest first")[```
┌─ Snapshots · all paragraphs ──────────────────────────────┐
│ 2026-06-19 14:23  421w  the-quay      The storm scene     │
│ 2026-06-19 11:02  388w  the-quay      Earlier draft       │
│ 2026-06-18 16:40  205w  harbour-mist  Opening sketch      │
│ 2026-06-17 22:05  140w  chapter-head  Working title       │
│                                                           │
│ ↑↓ move · / filter · V diff vs current · Enter open ¶     │
│ Esc close                                                 │
└───────────────────────────────────────────────────────────┘
```]

Each row is *timestamp · words · paragraph · annotation*. Because the list is
flat and sorted purely by time, one paragraph's history interleaves with every
other's — which is exactly what you want when you are hunting by *when* you did
something rather than *where*. The browser is the same engine as `F6`, widened;
it is how you find the row, and it hands the acting-on-it back to `F6`:

#chord_table((
  chord_row("↑ ↓", "Move through the flat, project-wide list."),
  chord_row("/", "Filter by paragraph title OR annotation; Enter or Esc leaves the filter, a second Esc closes."),
  chord_row("V", "Diff the selected snapshot against its OWN paragraph's current text; Esc returns to the browser."),
  chord_row("Enter", "Open that snapshot's paragraph in the editor and drop straight into its F6 picker."),
  chord_row("Esc", "Close the browser."),
))

Two of these repay a second look. The `V` diff resolves the snapshot's own
paragraph and diffs against *that* paragraph's live text — even though you opened
the browser from somewhere else entirely, the comparison is always
apples-to-apples. And `Enter` is a hand-off, not an action: the browser
deliberately *does not restore or delete* from the project-wide view, because
those are destructive, paragraph-local acts. Instead it opens the owning
paragraph and drops you into its `F6` picker, where the pre-restore safety net
is already wired up and you can act with full local context.

The browser lists itself in the command palette (`Ctrl+V Space`) under
"snapshot", so you can reach it by name without remembering `Ctrl+F6`.

#section("Deleted-paragraph history — the kill-ring")

Snapshots version a paragraph that still exists. A different problem is the
paragraph you *deleted* — cut from the Tree in a fit of restructuring, then
wanted back. For this Inkhaven keeps a *kill-ring*: a short, ordered stash of
the most recently deleted paragraphs, whole and restorable.

#term("Kill-ring")[
  The *kill-ring* is a session-local stash of your most recently deleted
  paragraphs — up to `editor.deleted_paragraph_history` of them (default 10),
  oldest rolling off as new deletes arrive. Each entry captures the paragraph
  whole, enough to re-create it in place. It lives in memory only and is not
  written to `.session.json`, so restarting Inkhaven clears it.
]

When you delete a single paragraph from the Tree (`-`, then confirm), it is
pushed onto the front of the ring, and the status bar tells you both that it is
gone and that `Ctrl+B U` will bring it back. What comes back is not just the
prose but the whole paragraph: its title and slug (so the file lands at the same
path), its body, its tags, its workflow status, its per-paragraph word target,
its content type, its outgoing paragraph links, and any timeline event data.

#callout(label: "A new identity")[
  A restored paragraph gets a *fresh* uuid. It returns to the same place with
  the same words, but paragraph links elsewhere that pointed at the *old* uuid
  stay broken — the status line is upfront about this. If preserving identity
  matters (an incoming link you need to keep live), reach instead for the
  branch-level snapshot restore, which keeps the uuid.
]

#subsection("Restore the most recent — Ctrl+B U")

`Ctrl+B U` restores the paragraph at the *front* of the ring — the one you just
deleted — without opening anything. It re-creates the paragraph at its original
position: back in its old slot if a sibling anchor survives, or at the end of
the (now shorter) child list if it had been the first child. This is the
one-key "undo that delete" you reach for the instant you realise you cut the
wrong row. Press it with an empty ring and it is a harmless no-op — the status
reads `nothing to restore`.

#subsection("The restore picker — Ctrl+V Shift+U")

`Ctrl+B U` only ever gives you back the *latest* delete. When the one you want
is further down the ring — you deleted three paragraphs, then wanted the first
of them — open the *restore picker* with `Ctrl+V Shift+U`. It lists the ring's
entries, most recent first, each with its word count and where it came from, and
`Enter` restores the highlighted one at its original position.

#screen(caption: "Ctrl+V Shift+U — pick which deleted paragraph to restore")[```
┌─ Recently deleted paragraphs ───────────────────────────┐
│ › morning         128w   Rillmark ▸ 2 The City          │
│   the-ledger       64w   Rillmark ▸ 3 Departure         │
│   a-false-start   212w   Rillmark ▸ 1 Arrival           │
│                                                         │
│ ↑↓ move · Enter restore at original position · Esc      │
└─────────────────────────────────────────────────────────┘
```]

The word count on each row is the quiet detail that makes the picker usable —
when three deleted paragraphs share a forgettable slug, the size is how you
recognise the two hundred words you actually want among the throwaways.

#subsection("What the ring does and does not cover")

The kill-ring is for *single-paragraph* deletes only. Deleting a whole branch —
a chapter, a subchapter, a book — is a larger and more consequential act that
the ring does not try to undo, and a branch delete no longer clears the ring
either, so older single-paragraph entries stay valid recoveries after one. What
protects a deleted branch is the *other* half of this chapter: the automatic
`pre-delete` snapshot of every paragraph in it, which you recover through the
`Ctrl+F6` browser and the `F6` picker. The two systems divide the work — the
kill-ring for the everyday single cut you want back in one keystroke, snapshots
for the durable, branch-scale safety net.

#callout(label: "Three grains of undo")[
  Keep the layers straight and you will always reach for the right one. A *typo*
  is `Ctrl+U` in the editor. A *deleted paragraph* is the kill-ring
  (`Ctrl+B U` / `Ctrl+V Shift+U`). A *deleted chapter* — or any older version of
  living prose — is a snapshot (`F6` / `Ctrl+F6`). The zip backup, from the
  health chapter, is the grain beneath all three.
]

#section("The crash mirror and recovery")

Everything so far assumes Inkhaven exits cleanly. The last layer assumes it does
not — that the process panics, the host loses power, or something sends a
`kill -9` between two keystrokes. The promise here is concrete: your dirty
buffers survive it, and the next run picks them back up. You should never need
this section; when you do, it should feel like a non-event.

#subsection("The crash mirror")

While you edit, a background task quietly mirrors every *dirty* buffer to disk on
a short cadence — `editor.crash_mirror_seconds`, two seconds by default. This is
not autosave and it is not a snapshot; it is a side copy that exists so a sudden
death loses as little typing as possible. A lower value loses fewer keystrokes
at the cost of more disk churn; `0` mirrors on every tick. The worst case is
narrow: if the power fails between two mirror ticks, only the last couple of
seconds of typing can be lost, and everything older is already on disk.

#subsection("What a panic writes")

If Inkhaven panics, its handler writes two kinds of artefact before it goes
down, both atomically (temp file, fsync, rename, directory fsync), and both
silent on failure — the tool would rather get partial state out than panic
inside its own panic handler:

- *A crash report*, `inkhaven-crash-<UTC>.hjson`, in the launch directory: the
  panic message and location, the project root and the paragraph open at the
  time, a manifest of the rescued buffers, the last fifty actions you took, and
  a small environment fingerprint (`$TERM`, `$LANG`, OS, version).

- *A rescue file* per dirty paragraph, `<paragraph>.typ.inkhaven-rescue`, beside
  the real file — the in-memory buffer flushed to disk. These are the actual
  prose; the report is only the index over them.

The report is deliberately conservative about what it records. It does *not*
include your LLM prompts or responses, your search queries, snapshot bodies, the
project database, your full config, or any environment variable beyond `$TERM`
and `$LANG`. You can read it yourself before attaching it to a bug report.

#subsection("Running inkhaven recover")

After a crash, restart Inkhaven on the project, then run `inkhaven recover` from
a shell, pointing it at the crash report. It walks each rescued buffer, showing
its size and the delta from what is currently on disk, and asks what to do.

#screen(caption: "inkhaven recover — the per-buffer walk")[```
$ inkhaven recover ./inkhaven-crash-20260805T0321.hjson

  inkhaven version : 3.0.0
  panic at         : src/foo.rs:42:7
  message          : called Option::unwrap() on None

Found 3 rescued buffer(s).

  [1/3] manuscript/01-arrival/the-quay.typ
       rescue : …/the-quay.typ.inkhaven-rescue (4823 b)
       on-disk: …/the-quay.typ (4801 b, delta +22)
       apply? [y/N/diff]:
```]

At each prompt, `y` applies the rescue — atomically replacing the on-disk file,
but first copying the current on-disk contents to `<original>.pre-recover-<UTC>`
so you can roll the decision back later with a `mv`. `N` (or a bare `Enter`)
skips the buffer. `d` or `diff` shows a side-by-side line diff of rescue against
on-disk, then re-prompts. When the two are byte-identical — you saved just before
the crash and the rescue is redundant — the walk skips the prompt entirely and
reports "no action needed", making no backup because there is nothing to back
up. After the walk, the report and its rescues are moved into
`<project>/.inkhaven/recovered/` so a second run does not re-apply the same set;
pass `--keep` to leave them in place. For scripted cleanup, `--yes` applies every
rescue without prompting, and a path-traversal guard rejects any rescue entry
whose path would escape the project root — a crafted crash report cannot be used
to write arbitrary files.

#callout(label: "The backstops beneath recovery")[
  If a `kill -9` lands inside the mirror window, the last couple of seconds may
  not be in the rescue. The established backstops still hold under it: `Ctrl+S`
  saves the open paragraph immediately and atomically; `editor.autosave_seconds`
  flushes dirty buffers after an idle window; and `inkhaven backup` (or
  `Ctrl+B Shift+B`) writes the whole project to a timestamped `.zip`, surfacing
  an overdue warning in the health chip. Recovery is the floor, not the only
  net.
]

#section("The configuration knobs")

Four settings in the `editor` block of `inkhaven.hjson` govern the machinery in
this chapter. The defaults are chosen so you never have to touch them, but they
are here when your workflow wants otherwise.

#screen(caption: "The history and recovery knobs, with their defaults")[```
editor: {
  crash_mirror_seconds:        2     // dirty-buffer mirror cadence
  deleted_paragraph_history:   10    // kill-ring depth
  external_change_auto_reload: true  // silent reload of clean files
  autosave_seconds:            5     // idle autosave window
}
```]

`crash_mirror_seconds` sets how often the crash mirror flushes dirty buffers —
lower narrows the worst-case loss window at the cost of more writes; `0` mirrors
every tick. `deleted_paragraph_history` sets how many deleted paragraphs the
kill-ring holds before the oldest rolls off. `external_change_auto_reload`
decides what happens when a *clean* open file changes on disk under you — `true`
reloads it silently, `false` warns and leaves your view untouched, which is the
setting you want when an external `git pull` or a script rewrites files while
you work. `autosave_seconds` is the idle window before an unsaved buffer is
written for you; `0` disables idle autosave, leaving the quit-time and
paragraph-switch autosaves in force. Together they tune how eagerly Inkhaven
protects your words against how much it writes to your disk to do it.

#recap((
  [A *snapshot* is a dated, independent copy of one paragraph's prose — separate
  from the live file, excluded from search, and never garbage-collected. It
  *outlives saves and even the paragraph's own deletion*.],
  [Snapshots are taken manually with *`F5`* (the annotation prompt; `Ctrl+B N`),
  and *automatically* before every risky op — AI rewrites, book-scope replace,
  branch deletes, and each restore — so *no path overwrites prose without a
  restorable copy landing first*.],
  [*`F6`* is the per-paragraph picker: `Enter` restores (behind a
  *pre-restore safety snapshot*), `V` diffs against the live buffer, `D` deletes,
  `/` filters by annotation. *`Ctrl+F6`* is the project-wide browser over every
  snapshot, which hands acting-on-a-row back to `F6`.],
  [The *kill-ring* restores deleted paragraphs — *`Ctrl+B U`* brings back the
  most recent, *`Ctrl+V Shift+U`* opens a word-counted picker over the last
  `deleted_paragraph_history` (default 10). It is single-paragraph and
  session-local; deleted *branches* are recovered through their `pre-delete`
  snapshots instead.],
  [The *crash mirror* copies dirty buffers to disk every `crash_mirror_seconds`
  (default 2); a panic writes a crash report plus per-paragraph
  `.inkhaven-rescue` files, and *`inkhaven recover`* walks them back with
  per-buffer `y` / `N` / `diff`, `.pre-recover` backups, and a path-traversal
  guard.],
))
