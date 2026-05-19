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
| `Tab`                | Cycle focus Tree → Editor → AI → Tree.                      | `next_pane`  |
| `Shift+Tab`          | Cycle in reverse.                                           | `prev_pane`  |
| `Ctrl+/`             | Focus the top Search bar.                                   | `search`     |
| `Ctrl+I`             | Focus the bottom AI prompt bar.                             | `ai_prompt`  |
| `Ctrl+S`             | Save current paragraph + re-embed (no-op if nothing open).  | `save`       |
| `Ctrl+Q`             | Hard quit. Auto-saves the open paragraph first if dirty; if the save fails, refuses to quit so the error stays visible. | no |
| `Ctrl+1`             | Focus the **Editor** pane.                                  | no           |
| `Ctrl+2` / `Ctrl+T`  | Focus the **Tree** pane. Use `Ctrl+T` if your terminal re-encodes `Ctrl+2` as NUL or `Ctrl+@`. | no |
| `Ctrl+3`             | Focus the **AI** pane.                                      | no           |
| `Ctrl+4`             | Focus the **Search** bar (top).                             | no           |
| `Ctrl+5`             | Focus the **AI prompt** bar (bottom).                       | no           |
| `Ctrl+B`             | Enter **meta mode**. The next keystroke is the action selector (see §1.1). | `meta_prefix` |
| `Ctrl+B H`           | Open the pane-aware **Quick reference** floating pane. Works from every pane (Tree / Editor / AI). Scroll with arrows / PgUp / PgDn; close with `Esc`. Routed through the meta prefix so it never collides with the editor's `Ctrl+H` split-scroll. | no |

### 1.1 Meta mode (Ctrl+B prefix)

The meta prefix is a single `Ctrl+B`; the second key selects the action.
**The action table is pane-specific** — `Ctrl+B` then `S` means different
things depending on whether the Tree, Editor, or AI pane has focus. The
status bar shows a yellow **META** chip and a prompt listing the actions
for the current pane while it's pending.

`Esc` cancels meta mode without running anything. Any unrecognized key
cancels with a status hint telling you which pane's table it consulted.

**Tree pane (and Search bar focus)** — hierarchy management:

| Second key | Action                                              |
| ---------- | --------------------------------------------------- |
| `B` / `b`  | Open Add modal — new **book** at the root.          |
| `C` / `c`  | Open Add modal — new **chapter**.                   |
| `S` / `s`  | Open Add modal — new **subchapter**.                |
| `P` / `p`  | Open Add modal — new **paragraph**.                 |
| `D` / `d`  | Open Delete confirm modal for the cursor's node.    |
| `↑`        | Swap the cursor's node with its previous sibling.   |
| `↓`        | Swap the cursor's node with its next sibling.       |
| `H` / `h`  | Open the pane-aware **Quick reference** overlay.    |

**Editor pane** — paragraph operations:

| Second key | Action                                                          |
| ---------- | --------------------------------------------------------------- |
| `S` / `s`  | **Save** the open paragraph (alternative to Ctrl+S).            |
| `N` / `n`  | **New snapshot** of the current buffer (== F5).                 |
| `R` / `r`  | Open the snapshot histo**R**y picker (== F6). Moved off `H` so Help can claim that letter across every pane. |
| `L` / `l`  | Open the **load file** dialog (== F3).                          |
| `F` / `f`  | Toggle **split-edit** mode (== F4). See §3.9.                   |
| `H` / `h`  | Open the pane-aware **Quick reference** overlay.                |

**AI pane (and AI prompt focus)** — inference management:

| Second key | Action                                              |
| ---------- | --------------------------------------------------- |
| `C` / `c`  | **Clear** the current inference (cancel streaming or discard a finished result). |
| `H` / `h`  | Open the pane-aware **Quick reference** overlay.    |

The Tree pane's plain-letter shortcuts (`B`, `C`, `V`, `A`, `S`, `+`, `P`,
`D`, `-`) still work directly without the meta prefix when Tree has focus —
see §2.2. To run a tree action from the Editor, switch focus first
(`Ctrl+2` or `Tab`) and then use either the plain letter or meta.

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
| `→`                  | **Expand** the cursor's branch (book/chapter/subchapter), revealing its children. No-op on a paragraph or an already-expanded branch. |
| `←`                  | **Collapse** the cursor's expanded branch. If already collapsed (or on a paragraph), moves the cursor to the parent node. Same semantics as the F3 file picker. |
| `Home`               | Jump to first row.                                          |
| `End`                | Jump to last row.                                           |
| `PageUp`             | Move cursor 10 rows up (configurable: `page_up`).           |
| `PageDown`           | Move cursor 10 rows down (configurable: `page_down`).       |
| `Enter`              | Open the cursor's node. Paragraphs load into the editor and shift focus there; if a different paragraph was open with unsaved edits, it's autosaved first. Branches print a status hint and stay in Tree. |
| `F2`                 | Open the **Rename** modal pre-filled with the current node's title. Slug + filesystem entry stay; only the displayed title changes (re-embeds for search). |
| `F3`                 | Open the **file picker** dialog. Enter on a file creates a new paragraph (inserted after the current cursor) with that file's content. Enter on a directory **recursively imports** the tree — subdirectories become branches one level deeper (Book→Chapter→Subchapter), files become paragraphs. If the directory tree exceeds the hierarchy depth, the deeper files are flattened into the deepest legal branch (with `unbounded_subchapters: false`). See §12. |
| `q` or `Q`           | Quit (autosaves the open paragraph first if dirty).         |

**Open-paragraph indicator** — the row of the paragraph currently loaded in
the Editor is rendered with a **green bold "►"** marker (instead of the
usual `¶` glyph) regardless of focus. The marker stays visible whether the
Editor or Tree pane has focus, so you can always see which paragraph is
loaded. If your tree cursor happens to land on the open paragraph, the
REVERSED cursor highlight wins visually but the green color underneath
still marks the row.

### 2.2 Tree-pane shortcuts (modifier-free)

These plain-key shortcuts work only when the Tree pane has focus. They exist
alongside the global meta-prefix chords (§1.1) because terminals and
multiplexers commonly intercept those (see §13 for details). All four open
the same modals as their global equivalents — no destructive action without
confirmation.

**Append at end** — `B`, `C`, `A`, `+` open the Add modal and place the new node at the end of its parent's children. The parent is chosen by walking up from the tree cursor to the nearest node that can host the requested kind.

**Insert after current** — `V`, `S`, `P` open the same Add modal but place the new node immediately after the cursor's same-kind ancestor. All subsequent siblings get their `order` bumped by `+1` and their filesystem entries renamed. If no same-kind ancestor exists (e.g. pressing `P` on a book with no paragraphs), falls back to append-at-end so the action still does something.

| Key       | Action                                                                                  |
| --------- | --------------------------------------------------------------------------------------- |
| `B` / `b` | Add a new **book** at the root. User books are inserted **above** the system block (Notes, Research, Prompts, Places, Characters, Help) by shifting it down; the new book takes Notes' old order. Equivalent to `Ctrl+B` then `B`. |
| `C` / `c` | **Append** a chapter at the end of the book's children. Equivalent to `Ctrl+B` then `C`. |
| `V` / `v` | **Insert** a chapter immediately after the cursor's enclosing chapter.                  |
| `A` / `a` | **Append** a subchapter at the end of the chapter's children. Equivalent to `Ctrl+B` then `S`. |
| `S` / `s` | **Insert** a subchapter immediately after the cursor's enclosing subchapter.            |
| `+`       | **Append** a paragraph at the end of the parent's children. Equivalent to `Ctrl+B` then `P`. |
| `P` / `p` | **Insert** a paragraph immediately after the cursor's enclosing paragraph.              |
| `D` / `d` | Delete the cursor's node — only if it's a **branch** (book/chapter/subchapter). On a paragraph, shows a hint to press `-` instead. |
| `-`       | Delete the cursor's node — only if it's a **paragraph**. On a branch, shows a hint to press `D` instead. |
| `U` / `u` | **Move up** — swap the cursor's node with its previous sibling. Plain-letter form of `Ctrl+B ↑`. |
| `J` / `j` | **Move down** — swap the cursor's node with its next sibling. Plain-letter form of `Ctrl+B ↓`. |

Empty paragraph titles are allowed for `+` and `P` — the first sentence of the body becomes the title on next save.

Why kind-specific delete? Safety. `-` won't nuke an entire chapter if your
cursor accidentally landed on it, and `D` won't kill a paragraph you meant
to keep. If you want delete that doesn't care about kind, use the global
`Ctrl+B` then `D`.

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

**Border color** carries the dirty state at a glance — but only while the
pane has focus:

- **Green (bold)** — focused, in sync with disk + bdslib (saved).
- **Yellow (bold)** — focused, with unsaved edits.
- **White** — pane is *unfocused*. Dirty signaling moves to the title's
  `[modified]` suffix and the red `●` chip in the status bar (both
  always-on indicators).

**Focus-loss autosave**: whenever focus moves away from the Editor pane —
via `Tab`, `Ctrl+1..5`, `Ctrl+T`, `Ctrl+/`, `Ctrl+I`, `Esc` from another
input, etc. — the open paragraph is automatically saved if dirty. So you
can shift focus mid-edit without worrying about losing work; the next save
trigger (idle/quit/switch) won't catch the same change twice.

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

### 3.3 Selection, clipboard, undo

`tui-textarea` maintains a single linear selection range. Shift+arrows extend
it. **Note:** the editor uses non-standard keys for cut and paste because the
conventional bindings now do other things (`Ctrl+X` is "repeat" for search,
`Ctrl+Z` is delete-to-end-of-line). The mapping below has been chosen so
each operation lives on a distinct key with no overlap.

| Key                  | Action                                                      |
| -------------------- | ----------------------------------------------------------- |
| `Shift+←` / `Shift+→`| Extend selection left / right one character.                |
| `Shift+↑` / `Shift+↓`| Extend selection up / down one line.                        |
| `Ctrl+A`             | Select entire document.                                     |
| `Ctrl+C`             | **Copy** selection to system clipboard (falls back to internal yank if `arboard` failed to init). |
| `Ctrl+K`             | **Cut** selection to clipboard. Marks doc dirty.            |
| `Ctrl+P`             | **Paste** from clipboard at cursor (or replace selection). Marks dirty. |
| `Ctrl+U`             | **Undo.**                                                   |
| `Ctrl+Y`             | **Redo.**                                                   |

The line-targeted delete shortcuts (§3.11) all preserve the yank buffer so
they don't clobber clipboard state.

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

### 3.11 Line-targeted delete shortcuts

Four chords that delete chunks of the current line without touching the
clipboard. Each saves and restores the yank buffer around the operation, so
`Ctrl+P` paste still produces the last copy.

| Key       | Action                                                                 |
| --------- | ---------------------------------------------------------------------- |
| `Ctrl+D`  | **Delete current line** — removes the entire line + its trailing newline; cursor lands on the line that takes its place. On the very last line, the content is cleared and an empty line remains (no newline to delete). |
| `Ctrl+E`  | **Delete to end of line** — removes from the cursor to the line end.   |
| `Ctrl+W`  | **Delete to start of line** — removes from the cursor back to column 0. |

*(`Ctrl+Z` is intentionally unbound — see §1 Global. Undo is `Ctrl+U`, delete-to-EOL is `Ctrl+E`.)*

**Note on `Ctrl+W`**: bash, tmux, and some terminals interpret `Ctrl+W` as
"delete previous word" before forwarding the keystroke. If your shell layer
eats `Ctrl+W`, use the meta prefix path (`Ctrl+B`, then a future-defined
alias) or rebind the chord in `inkhaven.hjson` once configurable bindings
for it are added.

### 3.9 Split-edit mode

A two-pane "edit with lookback" view. Toggle with `F4`. While split is
active the editor area is divided 50/50 horizontally: the **upper pane** is
your normal read-write editor and the **lower pane** is a read-only
snapshot of the buffer captured at the moment you pressed F4. The lower
pane scrolls independently so you can keep an earlier passage visible
while you rewrite it above.

| Key       | Action                                                                  |
| --------- | ----------------------------------------------------------------------- |
| `F4`      | Toggle split. Capture the buffer on enter; drop the snapshot on exit.   |
| `Ctrl+F4` | **Accept** the snapshot — replace the live buffer with the captured copy, exit split, mark dirty (bold marks the diff; Ctrl+S commits the rollback). |
| `Ctrl+H`  | Scroll the lower (snapshot) pane up by one line. Only active in split.  |
| `Ctrl+J`  | Scroll the lower pane down by one line. Only active in split.           |

The upper pane behaves exactly like the full editor — same shortcuts, same
syntax highlighting, same selection / clipboard / undo, same idle autosave,
same diff bolding. The lower pane is fully passive: no cursor, no
highlighting, dim grey text. Its header shows the current visible line and
the snapshot's total line count, plus a reminder of the available keys.

`Ctrl+H` and `Ctrl+J` are routed to the split pane **only while split is
active**. When split is off they fall through to normal editor handling
(tui-textarea's defaults), so they don't shadow anything in regular use.
The Quick-reference overlay is opened via `Ctrl+B` `H` (meta prefix)
precisely so it never contends with the split-scroll chord.

### 3.10 Find and replace (regex)

In-buffer regex search with optional replacement. Matches are highlighted
in **red** on top of the syntax coloring; the cursor's current match gets a
brighter **LightRed + bold** style so it stands out among siblings.

| Key                | Action                                                                |
| ------------------ | --------------------------------------------------------------------- |
| `Ctrl+F`           | Open the **Find** modal (magenta-bordered). Type a regex, Enter to run. Cursor jumps to the first match; all matches stay highlighted. Status bar reports `match 1 / N`. |
| `Ctrl+X`           | **"Repeat"** (multifunction). In search mode: jump to the next match (wraps). In replace mode: replace the current match and advance to the next. Only active while a search is in progress; otherwise the keystroke falls through. |
| `Ctrl+R`           | **First press**: open the **Find & Replace** modal (search + replace fields, `Tab` switches between them). Enter applies the **first** replacement automatically and stays in replace mode. **Second press while in replace mode**: replace every remaining match and exit replace mode. |
| `Esc` (in editor)  | Clear the active search (drops the highlights, exits replace mode).   |

**Regex flavor:** full Rust [`regex`](https://docs.rs/regex) syntax. Use
flags via `(?i)` (case-insensitive), `(?s)` (dot matches newlines), etc.

**Per-line matching:** v1 searches line-by-line so cross-line patterns
won't match. Most literary search/replace tasks (word substitution, name
changes) are within-line anyway.

**Layer order in the renderer:** syntax color → `[modified]` bold → match
red bg → current-line highlight → selection REVERSED. Selection wins
visually when a char is both selected and matched; matches win over the
subtle current-line highlight.

**Pre-fill:** opening `Ctrl+F` or `Ctrl+R` again after an active search
pre-populates the modal inputs with the previous pattern (and replacement).
Edit them and Enter to re-run.

### 3.5 Snapshots and file loading

| Key  | Action                                                              |
| ---- | ------------------------------------------------------------------- |
| `F3` | Open the **file picker** dialog. Pick a file with Enter to replace the open paragraph's editor buffer (bold marks the change vs the saved version). Directories are rejected in this context. See §12 for navigation. |
| `F4` | Toggle **split-edit** mode — see §3.9. |
| `F5` | Save a versioned **snapshot** of the open paragraph's current body (stored as a bdslib document with `kind:"snapshot"` and a `parent_id` back-reference; doesn't appear in vector search). |
| `F6` | Open the **snapshot picker** overlay listing every snapshot for the open paragraph, newest first. `↑↓` navigates, `Enter` loads the selected snapshot into the editor (marks dirty so the next save commits the rollback), `Esc` cancels. |

Snapshots are independent documents — they survive paragraph saves and aren't
deleted when their parent is deleted, so they can act as a recovery hatch.
Currently they're not surfaced from the CLI; that's an easy follow-up if you
need scripted access.

### 3.6 Autosave and background sync

Three save triggers, plus manual `Ctrl+S`:

- **Idle**: when the editor has unsaved edits and the user hasn't pressed a
  key for `editor.autosave_seconds` (default 5; set to 0 to disable).
- **Paragraph switch**: opening another paragraph from the Tree pane
  autosaves the current one first.
- **Quit**: `Ctrl+Q` and the `q` quit chords autosave before exiting.

In addition, a background task calls `Store::sync()` every
`sync_interval_seconds` (default 60). This flushes the HNSW vector index +
DuckDB checkpoint without blocking the UI. Set to 0 to disable.

Every save also resets the bold "added since last save" overlay (§3.7).

### 3.7 Visual change tracking

Characters added to the editor since the last save (Ctrl+S, autosave, or
load) are rendered **bold** on top of the syntax highlighting. The marker
goes away the moment you save. Implemented with a per-line longest-common-
prefix/suffix diff — fast at literary scale, accurate for the common case
of typing within or appending to a line. Cross-line inserts may
misattribute briefly until the next save resets the snapshot.

### 3.8 Pane management while focused

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
| `Esc`                | If overlay open, close it (one press); else defocus back to **Editor** if a paragraph is open, otherwise to Tree.|

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
| `Esc`                | If picker open, close it; else defocus back to **Editor** if a paragraph is open, otherwise to Tree. |

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

Triggered by `Ctrl+B` followed by `B`/`C`/`S`/`P` (or by the Tree pane's plain-letter shortcuts, §2.2). Green-bordered floating box.

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

Triggered by `Ctrl+B` then `D` (or the Tree pane's `D`/`-` shortcuts). Red-bordered floating box. Shows the kind,
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
chars are normalized (Ctrl+s, Ctrl+S, and Ctrl+Shift+S all parse and match
the same way — useful because terminals vary in how they report case with
modifiers).

Defaults shipped in `assets/default_project.hjson`:

```hjson
keys: {
  save:             Ctrl+s
  search:           Ctrl+/
  ai_prompt:        Ctrl+i
  next_pane:        Tab
  prev_pane:        Shift+Tab
  page_up:          PageUp
  page_down:        PageDown
  meta_prefix:      Ctrl+b
}

editor: {
  // ...
  autosave_seconds: 5      // idle-trigger save in editor; 0 disables
}

// Background flush interval. 0 disables.
sync_interval_seconds: 60
```

The add/delete/reorder actions don't have direct chords any more — they
fire through the `meta_prefix` followed by the action letter (§1.1).

Non-configurable bindings (the editor's modern shortcut overrides, the
AI-action `r/i/t/b/c` keys, the modal `y/n` confirmations, etc.) are
hard-coded for this release. Open a PR or issue if you need any of them
configurable.

---

## 12. File picker dialog (F3)

Tree-style filesystem browser overlay, rooted at the shell's current working
directory. Same navigation in both contexts (Editor F3 and Tree F3); only
the Enter action differs.

```
┌── Pick file — /Users/you/some/dir ────────────────────────────────────────┐
│  ▸ 📁 books                                                               │
│  ▾ 📁 imports                                                             │
│      ▸ 📁 chapter-one                                                     │
│      ▸ 📁 chapter-two                                                     │
│        📄 preface.md                                                      │
│    📄 README.md                                                           │
│    📄 todo.txt                                                            │
│                                                                           │
│ ↑↓ navigate · → expand · ← collapse/parent · Enter pick · Esc cancel      │
└───────────────────────────────────────────────────────────────────────────┘
```

| Key                  | Action                                                      |
| -------------------- | ----------------------------------------------------------- |
| `↑` / `↓`            | Move cursor one entry up / down.                            |
| `PageUp` / `PageDown`| Jump by 10.                                                 |
| `Home` / `End`       | First / last entry.                                         |
| `→`                  | If cursor is on a directory: expand it (children inline immediately below). No-op for files or already-expanded directories. |
| `←`                  | If cursor is on an *expanded* directory: collapse it. Otherwise: move cursor to the parent entry. |
| `Enter`              | Commit (see action table below).                            |
| `Esc`                | Cancel; modal closes, nothing happens.                      |

**Sort order within each level**: directories first, then files, each
alphabetical. Hidden entries (names starting with `.`) are skipped.

**Action on Enter:**

| Context (F3 fired in) | Picked entry | What happens |
| --------------------- | ------------ | ------------ |
| Editor pane           | file         | Replaces the open paragraph's buffer with the file content. Marks the document dirty so the next save commits the change (a save will also re-create the snapshot baseline). |
| Editor pane           | directory    | Rejected — status hint says to pick a file. |
| Tree pane             | file         | Creates a new paragraph inserted **after** the cursor's same-kind ancestor (same as the `P` shortcut), titled from the filename, body = the file's bytes. |
| Tree pane             | directory    | **Recursive import**: the directory itself becomes a subchapter under the cursor's nearest valid host, every subdirectory becomes a nested subchapter, every file becomes a paragraph inside its containing subchapter. Sorted alphabetically with dirs-first. Requires `hierarchy.unbounded_subchapters: true` if the dir tree is deeper than two levels under a chapter. |

## 13. When chords don't reach Inkhaven

Some of the configured chords — especially `Ctrl+S`, `Ctrl+Q`, and the
`Ctrl+B` meta prefix — can be eaten by your terminal emulator, your shell,
or a terminal multiplexer (tmux / screen) before they reach Inkhaven. This
is not a bug in Inkhaven; it's a layer above us deciding the chord means
something else.

Common interceptors:

| Chord                  | Often intercepted by                                                |
| ---------------------- | ------------------------------------------------------------------- |
| `Ctrl+S`               | Terminal flow control (XOFF / freeze output). Run `stty -ixon` in your shell to disable. |
| `Ctrl+Q`               | Terminal flow control (XON). Same `stty -ixon` fix.                |
| `Ctrl+B`               | **tmux default prefix.** If you run inkhaven inside tmux, either rebind tmux's prefix (`set -g prefix C-a`) or remap inkhaven's `meta_prefix` in `inkhaven.hjson` to something tmux doesn't eat (e.g. `Ctrl+g`). |
| `Ctrl+Shift+Up/Down`   | Some terminals don't transmit the Ctrl modifier with arrow keys. Use the plain-letter shortcuts (`B`, `C`, `A`, `+`, `D`, `-`) in the Tree pane instead. |

**Workarounds Inkhaven provides:**

- The Tree pane has modifier-free `A` / `+` / `D` / `-` shortcuts (§2.2) for
  the most common add/delete operations.
- For reorder, both `Ctrl+B ↑/↓` (TUI) and `inkhaven mv ... up`
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

## 14. Quick cheat sheet

For when you just want the high-level map:

```
GLOBAL          Ctrl+Q       quit (autosaves if dirty)
                Ctrl+1..5    focus Editor / Tree / AI / Search / AI prompt
                Tab/S-Tab    cycle Tree / Editor / AI panes
                Ctrl+/       focus search
                Ctrl+I       focus AI prompt
                Ctrl+S       save current paragraph
                Ctrl+B       meta prefix (table depends on focused pane):
                  Tree:       B/C/S/P add · D delete · ↑/↓ reorder
                  Editor:     S save · N snapshot · H history · L load · F split
                  AI:         C clear inference
                  Esc         cancel meta

TREE            ↑↓ Home End  navigate
                ←/→          collapse/expand branch (← steps to parent if not expanded)
                PgUp PgDn    by 10
                Enter        open paragraph (autosaves the previous one)
                F2           rename current node
                F3           file picker → insert file or import dir
                B            add book at root
                C            append chapter         (V = insert after current)
                A            append subchapter      (S = insert after current)
                +            append paragraph       (P = insert after current)
                D            delete branch          (or Ctrl+B then D)
                -            delete paragraph       (or Ctrl+B then D)
                Ctrl+B ↑/↓   reorder within siblings
                q            quit (autosaves if dirty)

EDITOR          arrows       move cursor
                Ctrl+arrows  word / top / bottom
                Shift+arrows extend linear selection
                Ctrl+U/Y     undo / redo
                Ctrl+K/C/P   cut / copy / paste (system clipboard)
                Ctrl+A       select all
                Ctrl+D       delete current line
                Ctrl+E       delete cursor → end of line
                Ctrl+W       delete cursor → start of line
                Alt+arrows   extend rectangular block selection
                Alt+C        copy rectangular block
                Ctrl+S       save + re-embed
                Ctrl+F       open find (regex)
                Ctrl+X       "repeat" (next match / replace+next, search active only)
                Ctrl+R       open find&replace · replace all (in replace mode)
                F3           load file → replaces buffer
                F4 / Ctrl+F4 toggle split / accept snapshot
                Ctrl+H/J     (split only) scroll lower pane up/down
                Ctrl+B H     open Quick reference overlay (global)
                F5           create snapshot
                F6           open snapshot picker
                Esc          clear search (if active) · else defocus to tree
                (idle autosave fires after editor.autosave_seconds)
                (new text since last save is rendered bold)

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
