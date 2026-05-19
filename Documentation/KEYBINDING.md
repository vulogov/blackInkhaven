# Inkhaven keybinding reference

Every keystroke the TUI recognizes, organized by which pane or overlay has
focus. Keys flagged **configurable** are read from
`<project>/inkhaven.hjson` under the `keys` block; the values below are the
shipping defaults. Everything else is hard-coded and not user-overridable in
this release.

The TUI has five focus states (Tree, Editor, AI, Search bar, AI prompt) plus
three transient overlays (Search results, Prompt picker, and a modal stack of
Add prompt / Delete confirm). Overlays absorb keys; the underlying pane keeps
its visual focus state but does not see input until the overlay closes.

---

## 1. Global

These chords work from any focus except where noted. Chords marked
**configurable** can be remapped in HJSON.

| Chord                | Action                                                      | Configurable |
| -------------------- | ----------------------------------------------------------- | ------------ |
| `Ctrl+Q`             | Hard quit. Works even inside a modal.                       | no           |
| `Tab`                | Cycle focus Tree → Editor → AI → Tree.                      | `next_pane`  |
| `Shift+Tab`          | Cycle in reverse.                                           | `prev_pane`  |
| `Ctrl+/`             | Focus the top Search bar.                                   | `search`     |
| `Ctrl+I`             | Focus the bottom AI prompt bar.                             | `ai_prompt`  |
| `Ctrl+S`             | Save current paragraph + re-embed (no-op if nothing open).  | `save`       |
| `Ctrl+Shift+B`       | Open the Add modal seeded for a new book.                   | `add_book`   |
| `Ctrl+Shift+C`       | Open the Add modal seeded for a new chapter.                | `add_chapter`|
| `Ctrl+Shift+S`       | Open the Add modal seeded for a new subchapter.             | `add_subchapter` |
| `Ctrl+Shift+P`       | Open the Add modal seeded for a new paragraph.              | `add_paragraph` |
| `Ctrl+Shift+D`       | Open the Delete confirm modal for the tree cursor's node.   | `delete_node`|
| `Ctrl+Shift+Up`      | Swap the cursor's node with its previous sibling.           | `move_up`    |
| `Ctrl+Shift+Down`    | Swap the cursor's node with its next sibling.               | `move_down`  |

Add/delete/move chords pick the parent or target by walking up from the tree
cursor's current node until they find a node that can accept the requested
kind (or the node itself for delete/move). They surface an explanatory error
on the status line if no valid target exists.

`Tab` / `Shift+Tab` do **not** cycle focus when the editor pane has an open
paragraph — they cycle anyway in our implementation because we intercept them
before tui-textarea sees them, so they never insert a literal tab.

---

## 2. Tree pane

Focused on launch. Shows the project hierarchy with depth indentation, kind
glyphs (`📖` book, `▸` chapter, `▹` subchapter, `¶` paragraph), and a dim
`Nw` word-count suffix for paragraphs.

### 2.1 Navigation

| Key                  | Action                                                      |
| -------------------- | ----------------------------------------------------------- |
| `↑` / `↓`            | Move cursor one row up/down (within scroll).                |
| `Home`               | Jump to first row.                                          |
| `End`                | Jump to last row.                                           |
| `PageUp`             | Move cursor 10 rows up (configurable: `page_up`).           |
| `PageDown`           | Move cursor 10 rows down (configurable: `page_down`).       |
| `Enter`              | Open the cursor's node. Paragraphs load into the editor and shift focus there. Branches print a status hint and stay in Tree. |
| `q` or `Q`           | Quit (no save prompt — Ctrl+S first if dirty).              |

### 2.2 Tree-pane shortcuts (modifier-free)

These plain-key shortcuts work only when the Tree pane has focus. They exist
alongside the global `Ctrl+Shift+*` chords because terminals and
multiplexers commonly intercept those (see §13 for details). All four open
the same modals as their global equivalents — no destructive action without
confirmation.

| Key       | Action                                                                                  |
| --------- | --------------------------------------------------------------------------------------- |
| `B` / `b` | Open Add modal for a new **book** at the root level. Equivalent to global `Ctrl+Shift+B`. |
| `C` / `c` | Open Add modal for a new **chapter**. Equivalent to global `Ctrl+Shift+C`.              |
| `A` / `a` | Open Add modal for a new **subchapter**. Equivalent to global `Ctrl+Shift+S`.           |
| `+`       | Open Add modal for a new **paragraph**. Equivalent to global `Ctrl+Shift+P`.            |
| `D` / `d` | Open Delete modal — but only if the cursor is on a **branch** (book/chapter/subchapter). On a paragraph, shows a hint to press `-` instead. |
| `-`       | Open Delete modal — but only if the cursor is on a **paragraph**. On a branch, shows a hint to press `D` instead. |

Why kind-specific delete? Safety. `-` won't nuke an entire chapter if your
cursor accidentally landed on it, and `D` won't kill a paragraph you meant
to keep. If you want delete that doesn't care about kind, use the global
`Ctrl+Shift+D`.

Shortcuts ignore the `Shift` modifier (uppercase implies Shift on most
layouts) but reject `Ctrl` / `Alt` / `Super` — so `Ctrl+A` will *not* fire
Add-subchapter.

All global chords also fire from the Tree pane.

---

## 3. Editor pane

Focused automatically when a paragraph is opened. Backing widget is
`tui-textarea` driven by `input_without_shortcuts`, so emacs-style defaults
(Ctrl+A → start of line, Ctrl+P → previous line, etc.) are **off**. We
intercept the modern conventional shortcuts ourselves; everything else falls
through to tui-textarea's typing / cursor handling.

### 3.1 Cursor movement

| Key                  | Action                                                      |
| -------------------- | ----------------------------------------------------------- |
| `←` / `→`            | One character left / right.                                 |
| `↑` / `↓`            | One line up / down.                                         |
| `Home`               | Start of current line.                                      |
| `End`                | End of current line.                                        |
| `PageUp` / `PageDown`| One viewport up / down (tui-textarea internal).             |
| `Ctrl+←`             | Previous word boundary.                                     |
| `Ctrl+→`             | Next word boundary.                                         |
| `Ctrl+Home`          | Top of document.                                            |
| `Ctrl+End`           | Bottom of document.                                         |

### 3.2 Editing

| Key                  | Action                                                      |
| -------------------- | ----------------------------------------------------------- |
| any character        | Insert at cursor. Replaces selection if one exists.         |
| `Enter`              | Insert newline.                                             |
| `Backspace`          | Delete character before cursor (or whole selection).        |
| `Delete`             | Delete character at cursor.                                 |
| `Ctrl+Backspace`     | Delete previous word.                                       |
| `Ctrl+S`             | Save current paragraph to disk and re-embed in bdslib. Triggers a tree reload so word counts refresh. |

### 3.3 Selection and clipboard (continuous range)

`tui-textarea` maintains a single linear selection range. Shift+arrows extend
it. The standard conventional chords operate on this range.

| Key                  | Action                                                      |
| -------------------- | ----------------------------------------------------------- |
| `Shift+←` / `Shift+→`| Extend selection left / right one character.                |
| `Shift+↑` / `Shift+↓`| Extend selection up / down one line.                        |
| `Ctrl+A`             | Select entire document.                                     |
| `Ctrl+C`             | Copy selection to system clipboard (falls back to internal yank if `arboard` failed to init). |
| `Ctrl+X`             | Cut selection to clipboard. Marks doc dirty.                |
| `Ctrl+V`             | Paste from clipboard at cursor (or replace selection). Marks dirty. |
| `Ctrl+Z`             | Undo.                                                       |
| `Ctrl+Y` or `Ctrl+Shift+Z` | Redo.                                                 |

If `arboard::Clipboard::new()` fails at startup (typical on headless or some
Wayland setups), copy/cut/paste silently fall back to tui-textarea's
internal yank buffer — the chords still work within the editor session, but
don't cross process boundaries.

### 3.4 Vertical block selection (rectangular)

A second, separate selection model independent of tui-textarea's native
range. Always rectangular: anchor + current cursor define inclusive
`(row_min..row_max, col_min..col_max)`. Drawn with REVERSED style on top of
the syntax highlighting.

| Key                          | Action                                                  |
| ---------------------------- | ------------------------------------------------------- |
| `Alt+↑` / `↓` / `←` / `→`    | Enter block-select mode (if not already), then move cursor by one cell without changing tui-textarea's linear selection. Rectangle redraws each frame. |
| `Alt+C`                      | Copy the rectangle to system clipboard as a multi-line string (each row a line). Clears the anchor. |
| `Esc`                        | Cancel block-select; keep the doc open.                 |
| any non-Alt key              | Cancels block-select implicitly (falls through to normal editor handling). |

**Deferred in this release**: rectangular cut and rectangular paste require
bulk character-deletion across multiple lines, which tui-textarea doesn't
expose cleanly. Copy-only covers the common cases (extracting a column of
leading numbers, a list of names, a verse stanza).

### 3.5 Pane management while focused

| Key                  | Action                                                      |
| -------------------- | ----------------------------------------------------------- |
| `Esc`                | Defocus to Tree without closing the document. If a block selection is active, Esc clears it first. |
| `Tab` / `Shift+Tab`  | Cycle focus (intercepted globally so they don't insert tab).|

### 3.6 When no paragraph is open

If the editor pane is focused but `opened` is `None`, only one key matters:

| Key       | Action |
| --------- | ------ |
| `q` / `Q` | Quit.  |

Plus all global chords.

---

## 4. AI pane

Focused automatically when an inference starts (Enter in the AI prompt bar).
Shows provider, streaming status, accumulated tokens, and — once streaming
completes — a one-line hint with the action keys.

| Key       | Condition                       | Action                                              |
| --------- | ------------------------------- | --------------------------------------------------- |
| `r` / `R` | inference done, doc open        | Replace editor selection (or entire doc if no selection) with the AI text. Marks dirty, refocuses Editor. |
| `i` / `I` | inference done, doc open        | Insert AI text at cursor. Marks dirty.              |
| `t` / `T` | inference done, doc open        | Prepend AI text to top of paragraph (with blank line separator). |
| `b` / `B` | inference done, doc open        | Append AI text to bottom of paragraph.              |
| `c` / `C` | inference done                  | Copy AI text to system clipboard only (no editor change). |
| `q` / `Q` | always                          | Quit.                                               |

Action keys fire only when `inference.status == Done` and the response is
non-empty. While streaming or on error, single-character keys do nothing
(except `q` to quit).

---

## 5. Search bar (top input)

Activated by `Ctrl+/` from any non-modal focus. Cursor appears as a `│`
character at the buffer's character position.

| Key                  | Behavior                                                    |
| -------------------- | ----------------------------------------------------------- |
| any printable char (no Ctrl/Alt) | Insert at cursor. Closes the results overlay if it was open (query has changed). |
| `Backspace`          | Delete char before cursor; closes results overlay.          |
| `Delete`             | Delete char at cursor; closes results overlay.              |
| `←` / `→`            | Move cursor one char left / right.                          |
| `Home`               | Cursor to start.                                            |
| `End`                | Cursor to end.                                              |
| `↑`                  | (overlay open) Move result cursor up.                       |
| `↓`                  | (overlay open) Move result cursor down.                     |
| `Enter`              | If results overlay is open: open the highlighted result. Otherwise: run `Store::search_text(query, 10)` and show results. |
| `Esc`                | If overlay open, close it (one press); else defocus to Tree.|

Opening a result from this overlay positions the tree cursor on the target
node. Paragraphs additionally load into the editor (focus moves to Editor).

---

## 6. AI prompt bar (bottom input)

Activated by `Ctrl+I`. Behaves like the Search bar with a different submit
action and the `/`-triggered Prompt picker overlay.

| Key                  | Behavior                                                    |
| -------------------- | ----------------------------------------------------------- |
| any printable char (no Ctrl/Alt) | Insert at cursor. If the buffer starts with `/`, opens the Prompt picker; otherwise closes it. |
| `Backspace`          | Delete char before cursor. Refreshes the picker if visible. |
| `Delete`             | Delete char at cursor. Refreshes the picker.                |
| `←` / `→`            | Move cursor one char left / right.                          |
| `Home`               | Cursor to start.                                            |
| `End`                | Cursor to end.                                              |
| `↑`                  | (picker open) Move selection up.                            |
| `↓`                  | (picker open) Move selection down.                          |
| `Tab`                | (picker open) Expand selected prompt template into the buffer with `{{selection}}` / `{{context}}` substituted. |
| `Enter`              | If picker open: same as Tab — expand selected template. Otherwise: spawn a streaming inference. The buffer is sent verbatim, except a leading `/name` is resolved against the prompt library and substituted first. |
| `Esc`                | If picker open, close it; else defocus to Tree.             |

Submitting a query when no API key is set in the environment surfaces a
status-line error like `GEMINI_API_KEY not set in environment — `export
GEMINI_API_KEY=...`` and does not spawn a request. Provider, model, and API
key env var are all driven by the `llm` block in `inkhaven.hjson`.

---

## 7. Search results overlay

Floating yellow panel rendered over the body when a search has run. Top line
shows `Results for `<query>` (N)`; each result occupies three rows
(score+kind+path, title, snippet).

Keys are routed to this overlay implicitly while it is open and the Search
bar is focused (see §5). The pane's own keys are:

| Key                  | Action                                                      |
| -------------------- | ----------------------------------------------------------- |
| `↑` / `↓`            | Move result cursor.                                         |
| `Enter`              | Open the highlighted result.                                |
| `Esc`                | Close the overlay (Search bar stays focused).               |
| Typing               | Closes the overlay and continues editing the query.         |

---

## 8. Prompt picker overlay

Floating magenta panel anchored just above the AI prompt bar. Shows every
prompt from `prompts.hjson` whose name or description contains the text
after `/` in the bar (case-insensitive).

Routed to the AI prompt bar (§6) — the picker has no separate focus.

| Key                  | Action                                                      |
| -------------------- | ----------------------------------------------------------- |
| `↑` / `↓`            | Move selection.                                             |
| `Enter` or `Tab`     | Expand the selected prompt's template into the buffer.      |
| `Esc`                | Close the picker without expanding.                         |
| Backspace / Delete   | Modify the filter; picker re-filters live.                  |

---

## 9. Add modal

Triggered by `Ctrl+Shift+B/C/S/P`. Green-bordered floating box.

```
┌── Add chapter ──────────────────────────────────┐
│  Parent: midnight-library                       │
│  Title : My chapter title│                      │
│                                                 │
│  Enter to confirm · Esc to cancel               │
└─────────────────────────────────────────────────┘
```

| Key                          | Action                                              |
| ---------------------------- | --------------------------------------------------- |
| any printable char (no Ctrl/Alt) | Insert into title buffer.                       |
| `Backspace`                  | Delete previous char.                               |
| `Delete`                     | Delete char at cursor.                              |
| `←` / `→` / `Home` / `End`   | Cursor navigation in the title buffer.              |
| `Enter`                      | Commit: derives slug, creates filesystem entry, inserts bdslib record, reloads tree, moves tree cursor to the new node. |
| `Esc`                        | Cancel without creating anything.                   |
| `Ctrl+Q`                     | Hard quit (modal does not absorb this).             |

Empty title shows a status hint and keeps the modal open. Validation errors
(e.g. trying to add a subchapter under a paragraph) close the modal and
display the error in the status line.

---

## 10. Delete confirm modal

Triggered by `Ctrl+Shift+D`. Red-bordered floating box. Shows the kind,
title, and descendant count.

```
┌── Confirm delete ───────────────────────────────┐
│  Delete chapter `Storm` and 4 descendants?      │
│                                                 │
│  Removes files from disk AND records from bdslib│
│  y / Enter to confirm · n / Esc to cancel       │
└─────────────────────────────────────────────────┘
```

| Key                  | Action                                                      |
| -------------------- | ----------------------------------------------------------- |
| `y` / `Y` / `Enter`  | Confirm. fs subtree removed, bdslib records deleted, tree reloads, cursor lands on the deleted node's parent (or stays put if parent vanished too — i.e. you deleted a book). |
| `n` / `N` / `Esc`    | Cancel.                                                     |
| `Ctrl+Q`             | Hard quit.                                                  |

If the open paragraph is inside the deleted subtree, the editor closes too.

---

## 11. Configurable bindings (HJSON)

The `keys` block in `inkhaven.hjson` accepts the chord strings below. Parser
recognizes:

- **modifiers**: `Ctrl` (or `Control`), `Shift`, `Alt` (or `Meta` / `Option`), `Super` (or `Cmd` / `Command`)
- **named keys**: `Tab`, `Enter` / `Return`, `Esc` / `Escape`, `Space`, `Backspace`, `Delete` (or `Del`), `Insert` (or `Ins`), `Home`, `End`, `PageUp` (or `PgUp`), `PageDown` (or `PgDown` / `PgDn`), `Up`, `Down`, `Left`, `Right`, `F1` through `F24`
- **single characters**: any printable ASCII character

Modifiers are case-insensitive; named keys are case-insensitive; single-letter
chars are normalized (Ctrl+Shift+P, Ctrl+Shift+p, and Ctrl+Shift+P with
either `P` or `p` payload all match).

Defaults shipped in `assets/default_project.hjson`:

```hjson
keys: {
  save:             Ctrl+s
  search:           Ctrl+/
  ai_prompt:        Ctrl+i
  add_book:         Ctrl+Shift+b
  add_chapter:      Ctrl+Shift+c
  add_subchapter:   Ctrl+Shift+s
  add_paragraph:    Ctrl+Shift+p
  delete_node:      Ctrl+Shift+d
  next_pane:        Tab
  prev_pane:        Shift+Tab
  page_up:          PageUp
  page_down:        PageDown
  move_up:          Ctrl+Shift+Up
  move_down:        Ctrl+Shift+Down
}
```

Non-configurable bindings (the editor's modern shortcut overrides, the
AI-action `r/i/t/b/c` keys, the modal `y/n` confirmations, etc.) are
hard-coded for this release. Open a PR or issue if you need any of them
configurable.

---

## 12. When chords don't reach Inkhaven

Some of the configured chords — especially `Ctrl+S`, `Ctrl+Q`, and the
`Ctrl+Shift+*` family — can be eaten by your terminal emulator, your shell,
or a terminal multiplexer (tmux / screen) before they reach Inkhaven. This
is not a bug in Inkhaven; it's a layer above us deciding the chord means
something else.

Common interceptors:

| Chord                  | Often intercepted by                                                |
| ---------------------- | ------------------------------------------------------------------- |
| `Ctrl+S`               | Terminal flow control (XOFF / freeze output). Run `stty -ixon` in your shell to disable. |
| `Ctrl+Q`               | Terminal flow control (XON). Same `stty -ixon` fix.                |
| `Ctrl+Shift+...`       | macOS Terminal.app / iTerm2 may remap these to window/tab shortcuts. Check Preferences → Keys. |
| `Ctrl+B*`              | tmux default prefix. `Ctrl+Shift+B` may also be eaten depending on tmux config. |
| `Ctrl+Shift+Up/Down`   | Some terminals don't transmit the Ctrl modifier with arrow keys. Try the symbol/letter alternatives (`A`, `+`, `D`, `-`) in the Tree pane. |

**Workarounds Inkhaven provides:**

- The Tree pane has modifier-free `A` / `+` / `D` / `-` shortcuts (§2.2) for
  the most common add/delete operations.
- For reorder, both `Ctrl+Shift+Up`/`Down` (TUI) and `inkhaven mv ... up`
  /`down` (CLI) exist; use the CLI in a second pane if the TUI chord is
  blocked.
- Save is also reachable via the CLI: open the `.typ` in an external
  editor, save there, then `inkhaven reindex` from a shell.

**If your terminal swallows Ctrl+S**, the simplest fix is to add this to
your shell rc:

```bash
stty -ixon
```

Then `Ctrl+S` reaches applications normally.

## 13. Quick cheat sheet

For when you just want the high-level map:

```
GLOBAL          Ctrl+Q       quit
                Tab/S-Tab    cycle Tree / Editor / AI panes
                Ctrl+/       focus search
                Ctrl+I       focus AI prompt
                Ctrl+S       save current paragraph

TREE            ↑↓ Home End  navigate
                PgUp PgDn    by 10
                Enter        open paragraph in editor
                B            add book               (or Ctrl+Shift+B)
                C            add chapter            (or Ctrl+Shift+C)
                A            add subchapter         (or Ctrl+Shift+S)
                +            add paragraph          (or Ctrl+Shift+P)
                D            delete branch          (or Ctrl+Shift+D)
                -            delete paragraph       (or Ctrl+Shift+D)
                C-S-Up/Down  reorder within siblings
                q            quit

EDITOR          arrows       move cursor
                Ctrl+arrows  word / top / bottom
                Shift+arrows extend linear selection
                Ctrl+Z/Y     undo / redo
                Ctrl+X/C/V   cut / copy / paste (system clipboard)
                Ctrl+A       select all
                Alt+arrows   extend rectangular block selection
                Alt+C        copy rectangular block
                Ctrl+S       save + re-embed
                Esc          defocus to tree

AI              r            replace selection / doc
                i            insert at cursor
                t            prepend to top
                b            append to bottom
                c            copy to clipboard only

SEARCH BAR      Enter        run search (or open highlighted result)
                ↑↓           navigate results overlay
                Esc          close overlay → defocus

AI PROMPT       /            open prompt picker
                ↑↓           navigate picker
                Tab/Enter    expand template (in picker)
                Enter        send to LLM (outside picker)
                Esc          close picker → defocus

MODALS          Enter        confirm
                Esc          cancel
                y/n          (delete only)
```
