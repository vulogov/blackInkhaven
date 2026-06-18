#import "../design.typ": *

#chapter(number: 7, part: "Part II — The Editor",
  title: "Editor workflow")

#dropcap("T")he editor pane is a `tui-textarea` widget with
inkhaven's overlays painted on top. Movement is keyboard
only — the chord muscle memory is the same as `nano` /
`micro` / VS Code's basic shortcuts, with a few inkhaven
specifics.

#section("Motion")

#chord_table((
  chord_row("Arrow keys", "Character / line."),
  chord_row("Home / End", "Beginning / end of line."),
  chord_row("Ctrl+Home / Ctrl+End", "Top / bottom of paragraph."),
  chord_row("Ctrl+← / Ctrl+→", "Previous / next word."),
  chord_row("PgUp / PgDn", "Scroll a page."),
  chord_row("Ctrl+G", "Jump to line — type a number, Enter."),
))

#section("Selection")

#chord_table((
  chord_row("Shift+arrow", "Extend selection one character / line."),
  chord_row("Shift+Ctrl+arrow", "Extend by word / paragraph."),
  chord_row("Ctrl+A", "Select all."),
  chord_row("Ctrl+Shift+A", "Select current paragraph (within the buffer — paragraph here means a block of prose, not the inkhaven Paragraph node)."),
  chord_row("Esc", "Drop selection."),
))

#section("Cut / copy / paste")

#chord_table((
  chord_row("Ctrl+X", "Cut to system clipboard."),
  chord_row("Ctrl+C", "Copy to system clipboard."),
  chord_row("Ctrl+V", "Note: in inkhaven `Ctrl+V` is the view-prefix chord (see Chapter 16). For paste, use the terminal's own paste binding (Cmd+V on macOS, Shift+Insert on Linux)."),
))

#callout(label: "On Ctrl+V")[
  Inkhaven reclaims `Ctrl+V` as the "View" prefix because
  every modern terminal already handles paste via OS-level
  bindings. If you can't paste, the terminal — not inkhaven —
  is intercepting your shortcut.
]

#section("Undo / redo")

#chord_table((
  chord_row("Ctrl+Z", "Undo. NOTE: in inkhaven `Ctrl+Z` is the Bund prefix (Chapter 29). Use the editor-pane's own undo via the inline chord (in `tui-textarea`)."),
  chord_row("Ctrl+R or Ctrl+Y", "Redo (depending on terminal)."),
))

The undo stack lives in the editor widget, not the database.
Closing the paragraph clears the undo history — once you
switch away, undo no longer reaches back. Snapshots
(Chapter 8) are the durable rollback path.

#section("Search and replace")

#chord_table((
  chord_row("Ctrl+F", "In-buffer find. Type, Enter, n / N walk hits."),
  chord_row("Ctrl+H", "In-buffer find-and-replace. Y/n/a per hit."),
  chord_row("Ctrl+/", "Focus the project-wide search input at the top of the screen."),
))

Project-wide search (`Ctrl+/`) is a different beast — covered
in Chapter 10 with semantic + full-text strategies.

#section("Split-edit (F4)")

`F4` toggles split-edit mode. The editor pane splits in half:
left side shows the most recent snapshot, right side holds
your live buffer. Useful when you want to keep the previous
version visible while you rewrite.

#figure_slot(
  id: "split-edit",
  caption: "Split-edit (F4) — left half is the last snapshot, right half is the live buffer. Ctrl+H/J scroll the snapshot.",
  height: 50mm,
)

#chord_table((
  chord_row("F4", "Toggle split-edit."),
  chord_row("Ctrl+H / Ctrl+J", "Scroll the snapshot pane independently."),
  chord_row("Ctrl+F4", "Accept the snapshot's current scroll position as the new baseline (saves the right pane as a snapshot)."),
))

The F12 critique chord (Chapter 21) behaves differently when
split-edit is active: the AI compares snapshot vs current
and reports on the changes you made.

#section("Save")

`Ctrl+S` saves. Save writes:

1. The file on disk (`books/<book-slug>/.../<paragraph-slug>.typ`).
2. The metadata DB record (modified time, word count).
3. The vector store (re-embeds the prose for semantic search).
4. Fires `hook.on_save(uuid)` (Chapter 29 covers hooks).

Autosave: the editor's idle timer (configurable in
`editor.autosave_idle_seconds`, default 30s) saves
automatically. Switching paragraphs also saves.

#recap((
  [Standard motion / selection / find / replace chords; Ctrl+V is the view prefix, not paste.],
  [Undo lives in the editor widget; snapshots are the durable rollback path.],
  [`F4` enables split-edit (snapshot vs live).],
  [`Ctrl+S` saves to disk + DB + vectors; autosave on idle + paragraph switch.],
  [Project-wide search (`Ctrl+/`) is Chapter 10.],
))
