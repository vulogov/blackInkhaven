# Inkhaven keybinding reference

Every keystroke the TUI recognizes, organized by which pane or overlay has
focus. Keys flagged **configurable** are read from
`<project>/inkhaven.hjson` under the `keys` block; the values below are the
shipping defaults. Everything else is hard-coded and not user-overridable in
this release.

> **Printable companion**: [`INKHAVEN_CHEAT_SHEET.typ`](INKHAVEN_CHEAT_SHEET.typ)
> — a two-column A4 reference. Compile with `typst compile
> Documentation/INKHAVEN_CHEAT_SHEET.typ` or open the file in the TUI
> (F3 → pick → `Ctrl+B B` to build).

The TUI has five focus states (Tree, Editor, AI, Search bar, AI prompt) plus
three transient overlays (Search results, Prompt picker, and a modal stack of
Add prompt / Delete confirm). Overlays absorb keys; the underlying pane keeps
its visual focus state but does not see input until the overlay closes.

---

## 0. Mouse

Inkhaven captures mouse input on startup. Left-click moves focus to the
clicked pane; in the Tree pane the click positions the row cursor, in the
Editor it positions the character cursor (clicks in the gutter are
ignored).

**Scroll wheel** (1.2.8+):

* **Tree pane** — moves the tree cursor up / down 3 rows per tick.
* **Editor pane** — scrolls the viewport up / down 3 lines per tick.
* **AI pane** — scrolls the chat history (older messages on wheel up).
* **OS Shell pane modal** — scrolls the turn buffer (older turns on wheel up).
* **HJSON editor modal** — moves the textarea cursor up / down 3 lines.
* **Kill-ring picker / Fuzzy paragraph picker** — moves the cursor through entries.
* Other modals ignore the wheel (silent no-op).

Floating overlays (search results, prompt picker) still swallow mouse
input so a stray click can't focus a hidden pane.

Terminal-side text selection still works while the alternate screen is
up — hold **Shift** (or Option, depending on the terminal) while
selecting to bypass mouse capture and copy text through the terminal's
own clipboard integration.

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
| `Ctrl+B V`           | Open the **Version / author / credits** floating pane. Shows the running Inkhaven version, the author block, the repository / licence, and the curated list of direct dependencies with their licences. Scrollable; `Esc` closes. | no |
| `F1`                 | Open the **Help-manual** query pane (RAG over the Help book). Type a question, Enter to ask. The default LLM provider streams a grounded answer into the AI pane; the model is constrained to use only the Help excerpts (no external knowledge). Same flow as typing `Help! <question>` into the AI prompt bar. Help answers are **one-shot** and do not enter the AI chat history. | no |
| `F7`                 | **Grammar check** the currently-open paragraph. Resolver precedence for the prompt template: (1) a paragraph titled / slugged `grammar-check` in the **Prompts** system book, (2) an entry of the same name in `prompts.hjson`, (3) a built-in fallback that runs a grammar/punctuation review against the configured `language` (HJSON top-level) and explicitly preserves any Typst markup. Result streams into the AI pane and focus moves there so you can watch the review in real time. | no |
| `F9`                 | **Cycle the AI scope** through `None → Selection → Paragraph → Subchapter → Chapter → Book → None`. The next prompt sent from the AI prompt bar prepends the matching context (selection text / open paragraph / enclosing branch contents) then **auto-resets to `None`**. Mode is shown in the AI prompt title (`AI prompt · scope: Paragraph`) and the status bar. Works from every pane. | no |
| `F10`                | **Toggle inference mode** between `Local` and `Full`. `Local` instructs the model to use only the supplied context (and prior chat turns); `Full` lets it augment with general knowledge. Both modes are shown in the AI pane title (`AI — gemini · done · infer=Full · scope=Paragraph`). Help inference (F1 / `Help! …`) is **pinned to Local** regardless of this toggle so the help-manual answer never invents features. Works from every pane. | no |
| `Ctrl+B C`           | Clear the AI chat history + currently displayed inference. (F9's old behaviour; F9 now drives the scope cycle.) | no |
| `Ctrl+B ]`           | (1.2.5) **Tag the open paragraph** — open the floating tag picker scoped to the editor buffer. Inside the picker: `↑↓` select, `Space` multi-selects, `T` applies selected tags (or the cursor tag if none selected), `A` adds a new tag (prompt), `D` deletes a tag project-wide (y/n confirm), `Enter` applies, `Esc` closes. | no |
| `Ctrl+B }`           | (1.2.5) **Search by tag** — open the floating tag picker in read-only mode. `Enter` on a tag lists every paragraph that carries it with a typeable filter input; `Enter` on a paragraph row opens it in the editor. `A` / `D` still work (tag management is project-wide). | no |
| `Ctrl+B 0`           | (1.2.8) **Edit project HJSON** — open `<project>/inkhaven.hjson` in a full-screen modal editor with HJSON syntax highlighting. `Ctrl+S` saves; when the saved bytes differ from the loaded bytes, a *Restart required* overlay pops up (config applies on next launch). `Esc` closes (status-line warning fires if there are unsaved edits). The editor mirrors the main paragraph editor's chord set: arrows / Home / End / PgUp / PgDn / Shift+arrows for selection / Ctrl+Home,End top/bottom / Ctrl+Left,Right word jumps / Ctrl+Backspace delete-word / Ctrl+U undo / Ctrl+Y redo / Ctrl+K cut / Ctrl+C copy / Ctrl+P paste / Ctrl+A select-all / Ctrl+D delete-line / Ctrl+E delete-to-EOL / Ctrl+W delete-to-BOL. | `bund.edit_project_hjson` |

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
| `T` / `t`  | **Retitle paragraph** — re-derive the displayed title from the buffer's first sentence (same logic that fires on save for placeholder titles, but runnable on demand). |
| `P` / `p`  | **Place-RAG inference** — treats the editor's current selection (or word under cursor) as a place name, sweeps matching paragraphs in the **Places** system book, and prepends them as RAG context to the next AI prompt. If the AI prompt is non-empty, the inference fires immediately and focus jumps to the AI pane. If empty, the context is stashed and focus jumps to the **AI prompt** so you can type your question. |
| `C` / `c`  | **Character-RAG inference** — same flow as `P` but against the **Characters** system book. |
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

### 1.2 View mode (Ctrl+V prefix)

The third meta prefix. Routes to in-process exporters, pickers,
the writing-progress modal, paragraph links, and bookmarks. In
1.2.4+ all view-sub chords are rebindable through HJSON
`keys.bindings.view_sub` and `ink.key.bind_view_sub`; the
prefix itself is rebindable via `keys.view_prefix` (default
`"Ctrl+V"`). See tutorials 15 / 16 / 17 / 19 / 21 for the
full workflows.

| Chord (after `Ctrl+V`) | Pane (focus filter)      | Action                                                                 |
| ---------------------- | ------------------------ | ---------------------------------------------------------------------- |
| `1`                    | Editor / AI-prompt       | Write the **open paragraph's buffer** as markdown via the save-as picker (1.2.4 — default path pre-filled; bare Enter writes there). |
| `2`                    | Editor / AI-prompt       | Write the **containing subchapter's subtree** as markdown via the save-as picker. |
| `1`                    | Tree                     | Write the tree-cursor's **node and all descendants** as markdown via the save-as picker. |
| `S` / `s`              | any                      | Toggle **similar-paragraph mode** — saves the buffer, picks via vector search, opens a second editor side-by-side. Re-press to save both and exit. Both editors autosave on idle (1.2.4). |
| `G` / `g`              | any                      | Open the **writing-progress modal** (today / streak / per-book pace / 30-day sparkline / status-ladder counts / per-book bar chart). |
| `T` / `t`              | any (needs open paragraph) | Set / clear the **per-paragraph word-count target** (1.2.4+). Empty / `0` clears. Saves that cross the target auto-promote status one ladder step when `goals.auto_promote_on_target` is true. |
| `A` / `a`              | any (needs open paragraph) | **Add outgoing paragraph link** (1.2.4) — tree pane enters select-paragraph-to-link mode; Enter confirms. Self-link / duplicate / cycle are rejected with a status-bar message. |
| `I` / `i`              | any (needs open paragraph) | **Add incoming paragraph link** (1.2.4) — tree pane enters select-paragraph-that-will-link-to-current mode. Reverse of `A`. |
| `L` / `l`              | any (needs open paragraph) | **List outgoing links** (1.2.4) — floating picker. Enter opens (autosaves prev); D removes the link. |
| `K` / `k`              | any (needs open paragraph) | **List backlinks** (1.2.4) — paragraphs that link TO the open one. D removes the source's outgoing link. |
| `B` / `b`              | any (needs open paragraph) | Toggle **bookmark** on the open paragraph (1.2.4). |
| `M` / `m`              | any                      | Open the **bookmark picker** (1.2.4). Enter opens; D removes the bookmark. |
| `P` / `p`              | any                      | **Fuzzy paragraph picker** (1.2.4) — type-to-filter modal over every user-book paragraph. Three-tier ranking (title-starts > title-contains > slug-contains). |
| `R` / `r`              | any (needs open paragraph) | (1.2.5) **Render paragraph** — save the buffer, compile it in-process via `typst-render`, float a PNG preview on top of the editor. Inside the preview: `←` / `→` navigate pages (multi-page documents), `Home` / `End` jump to first / last; `Esc` closes; `S` opens a save-as picker for the **current page** at full DPI (288 dpi); `A` opens the picker for **all pages** at full DPI (writes `<base>-page-NNN.png` per page). Cancelling the save picker restores the preview with navigation state intact. |
| `N` / `n`              | any (needs open paragraph) | (1.2.5) **Next typst diagnostic** — move the editor cursor to the next parse or semantic diagnostic in the buffer. Wraps at the end. Refreshes the diagnostic cache up-front so navigation reflects the current buffer state, not the last save. Status bar reports `diag N/M  line L:C  — <message>`. |
| `W` / `w`              | any (needs current user book) | (1.2.5) **Story view** — build a twopi-style radial graph of the current book (book at centre, each depth on a concentric ring) with the hierarchy (chapters / subchapters / paragraphs / scripts / images / json) plus paragraph links (dashed purple) and Characters / Places / Artefacts mentions on an outer ring (dashed green). Rasterised via `resvg` and floated on top of the editor. Inside the modal: `Esc` closes, `S` opens a save-as picker (default `<book-slug>-story-YYYYDDMM-HHMM.png`). |
| `Esc`                  | any                      | Cancel the chord without acting.                                       |

While in similar-paragraph mode, `Tab` inside the editor toggles
keyboard focus between the left and right editor panes (instead
of cycling to the missing AI pane).

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
| `Esc`                | Cycle focus to the **Search bar** (second leg of the Editor → Tree → Search → Editor rotation). |

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
| `Z` / `z` | **Collapse subchapter** — folds the cursor's enclosing Subchapter (or the cursor's node itself if it IS a Subchapter). Lands the tree cursor on the folded row. |
| `X` / `x` | **Collapse all** — folds every expanded branch in the tree. Empty branches and paragraphs are untouched. |
| `Space`   | (1.2.4) **Mark / unmark** the cursor row for multi-select. Status bar shows `marked N`. `Esc` clears all marks. |
| `T` / `t` | (1.2.4) **Cycle node type** (`paragraph → json → script`). No marks: cursor row only (folders skipped). With marks: every marked leaf. |
| `O` / `o` | (1.2.4) **Cycle status** one rung up the ladder (`napkin → first → … → ready → napkin`). No marks: cursor row. With marks: every marked paragraph. |
| `G` / `g` | (1.2.5) **Tag the marked set** — open the floating tag picker scoped to every marked paragraph (or just the cursor row when no marks). Same modal as `Ctrl+B ]`; T applies the selected tag set across every target at once. |

Empty paragraph titles are allowed for `+` and `P` — the first sentence of the body becomes the title on next save.

**Multi-select interaction** (1.2.4): with at least one row marked,
`Ctrl+B I` (reindex) and `Del` walk the mark set instead of the
cursor row. `Ctrl+B R` (rename) is intentionally single-row only.

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

*(`Ctrl+Z` is the **Bund-meta prefix** in 1.2+ — runs scripting actions
like `Ctrl+Z R` run-buffer, `Ctrl+Z N` new-script, `Ctrl+Z E` eval,
`Ctrl+Z ?` script-picker. See [`KEYS_REASSIGNMENT.md`](KEYS_REASSIGNMENT.md).
Undo is `Ctrl+U`, delete-to-EOL is `Ctrl+E`.)*

*(`Ctrl+V` is the **view-meta prefix** in 1.2.3+ — markdown export,
similar-paragraph mode, and progress tracking. See section 1.2 below
and tutorials 15 / 16 / 17.)*

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
| `F6` | Open the **snapshot picker** overlay listing every snapshot for the open paragraph, newest first. `↑↓` navigates, `Enter` loads the selected snapshot (1.2.4: takes a **pre-restore safety snapshot** of the live buffer first), `V` opens a **side-by-side diff** of the snapshot vs current (1.2.4 — Esc returns to picker), `D` / `Del` removes the snapshot, `Esc` cancels. |

Snapshots are independent documents — they survive paragraph saves and aren't
deleted when their parent is deleted, so they can act as a recovery hatch.

**Pre-restore safety net (1.2.4)**: Enter in the snapshot picker first creates
a snapshot of the live buffer, then replaces. If creating the safety snapshot
fails, the load aborts entirely — the buffer stays untouched. To undo an
unwanted restore: F6 again, the safety snapshot is at the top, Enter.
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

Focus lands here automatically only via the AI prompt's `Esc` bounce — when
you submit a query the prompt bar **keeps** focus so follow-ups are one
keystroke away. Pane title shows provider, streaming status, and a
`· N turn(s)` chip whenever the chat history has accumulated content.

| Key       | Condition                       | Action                                              |
| --------- | ------------------------------- | --------------------------------------------------- |
| `Esc`     | always                          | Bounce focus back to the **AI prompt** bar (mirror of the AI-prompt → AI Esc). |
| `r` / `R` | inference done, doc open        | Replace editor selection (or entire doc if no selection) with the AI text. Marks dirty, refocuses Editor. |
| `i` / `I` | inference done, doc open        | Insert AI text at cursor. Marks dirty.              |
| `t` / `T` | inference done, doc open        | Prepend AI text to top of paragraph (with blank line separator). |
| `g` / `G` | inference done, doc open        | **Grammar-check apply**: lifts only the corrected paragraph from the response (between `<<<CORRECTED>>>` / `<<<END>>>` markers, or last fenced code, or after a "Corrected …" heading) and overwrites the editor buffer wholesale (constructs a fresh `TextArea` so nothing of the old buffer survives). Skips the markdown→Typst conversion because the grammar prompt preserves Typst markup verbatim. Changed characters render in `theme.grammar_change_fg` (default red) and the highlight **survives saves** — dismiss it explicitly with `Ctrl+B` then `C`, or by switching paragraphs. Refuses with a status message if no extraction pattern matches. |
| `b` / `B` | inference done, doc open        | Append AI text to bottom of paragraph.              |
| `c` / `C` | inference done                  | Copy AI text to system clipboard only (no editor change). |
| `q` / `Q` | always                          | Quit.                                               |

Action keys fire only when `inference.status == Done` and the response is
non-empty. While streaming or on error, single-character keys do nothing
(except `q` to quit and `Esc` to bounce).

**Chat history.** Each non-Help inference appends a `(User, Assistant)` pair
to the in-memory chat history; the next prompt replays the whole history to
the model so the conversation is continuous. The title's `· N turn(s)` chip
shows the current depth. Press `F9` (or `Ctrl+B` then `C`) at any time to
clear both the history and the currently displayed inference.

Help (`F1` / `Help! …`) inferences are deliberately **one-shot** — they use
a strict RAG system prompt and are not added to the chat history, so a
prior set of chat turns won't dilute their grounding.

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
| `Esc`                | If results overlay is open, close it (one press); else cycle focus to the **Editor** pane (third leg of the Editor → Tree → Search → Editor rotation). |

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
| `Enter`              | If picker open: same as Tab — expand selected template. Otherwise: spawn a streaming inference. Focus **stays** on the AI prompt bar (it does not jump to the AI pane). The buffer is sent verbatim, except: a leading `/name` is resolved against the prompt library, and a leading `Help!` (case-sensitive) routes the rest of the line through the F1 Help-RAG flow. |
| `Esc`                | If picker open, close it; else bounce focus to the **AI pane** so you can read or scroll the answer. Pressing `Esc` again from the AI pane brings you straight back here. |
| `Ctrl+1`             | Focus the **Editor** pane (global shortcut, works from this input too). |
| `Ctrl+T`             | Focus the **Tree** pane (global shortcut, works from this input too). |

Submitting a query when no API key is set in the environment surfaces a
status-line error like `GEMINI_API_KEY not set in environment — `export
GEMINI_API_KEY=...`` and does not spawn a request. **Local providers** like
Ollama omit `api_key_env` in their `llm.providers` block entirely; the
check is skipped and genai routes to `http://localhost:11434/` from the
model name. Provider, model, and API key env var are all driven by the
`llm` block in `inkhaven.hjson`.

**Continuous chat.** Each submitted query plus its assistant response is
appended to the chat history and replayed back on the next prompt. The AI
pane title shows the current `· N turn(s)`. Press `F9` (or `Ctrl+B` then
`C`) to clear it.

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

Floating magenta panel anchored just above the AI prompt bar. Two sources
are merged, in this order:

1. **System prompts** from `prompts.hjson` — well-known templates that ship
   with the project. Shown with a cyan `[ system ]` chip.
2. **Book prompts** — every paragraph nested under the **Prompts** system
   book. The paragraph's slug supplies the `/name` identifier and the
   title supplies the description. Body is the template. Shown with a
   green `[ book ]` chip.

A name or description that contains the text after `/` in the bar (case-
insensitive) is included. Filter updates live as you type.

Routed to the AI prompt bar (§6) — the picker has no separate focus.

| Key                  | Action                                                      |
| -------------------- | ----------------------------------------------------------- |
| `↑` / `↓`            | Move selection.                                             |
| `Enter` or `Tab`     | Expand the selected prompt's template into the buffer.      |
| `Esc`                | Close the picker without expanding.                         |
| Backspace / Delete   | Modify the filter; picker re-filters live.                  |

The leading `= Title` Typst heading is stripped from book-prompt bodies on
expansion so it doesn't end up in the LLM prompt — the heading is editor
chrome, not prose. `{{selection}}` / `{{context}}` substitutions in the
expanded body still fire for both sources.

A direct `/name` typed into the AI prompt bar and submitted with `Enter`
(no picker open) is also resolved against both sources, system-first.

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
  meta_prefix:      Ctrl+b           // chord prefix for tree / editor / AI actions
  bund_prefix:      Ctrl+z           // chord prefix for Bund scripting (1.2+)
  view_prefix:      Ctrl+v           // chord prefix for view sub-chords (1.2.4+)
  bindings:         []               // user overlay; see KEYS_REASSIGNMENT.md
}

editor: {
  // ...
  autosave_seconds: 5      // idle-trigger save in editor; 0 disables
}

// Background flush interval. 0 disables.
sync_interval_seconds: 600
```

**Rebinding sub-chords** (the letters under `Ctrl+B …`, `Ctrl+Z …`,
and `Ctrl+V …`) went data-driven in 1.2 — list overrides in
`keys.bindings` or, at runtime, via the `ink.key.*` Bund stdlib.
The full action table and both rebinding channels are documented
in [`KEYS_REASSIGNMENT.md`](KEYS_REASSIGNMENT.md).

**F-keys in the binding table (1.2.4)** — F1 through F10 and the
Shift-F variants migrated from hardcoded matches into
`Layer::TopLevel`. HJSON overlays accept single-token chords:

```hjson
keys: {
  bindings: [
    { layer: "view_sub",  key: "P",  action: "view.fuzzy_paragraph_picker" }
    { layer: "top_level", key: "F7", action: "view.add_link" }  // rebind F7
  ]
}
```

Non-configurable bindings (the editor's modern shortcut overrides, the
AI-action `r/i/t/b/c` keys, the modal `y/n` confirmations, and
`Ctrl+Q` hard-quit) remain hard-coded.

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
                Esc          clear search (if active) · else cycle to Tree
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


---

## 1.2.5 + 1.2.6 — chord additions

Every chord introduced between the original document and the
1.2.6 release, organised by feature. This section is
maintained in delta-style so the canonical chord-by-pane
tables above stay readable; the entries below are the
new ground.

### Tag workflows (1.2.5+)

```
Ctrl+B ]   open the tag picker on the open paragraph
Ctrl+B }   open the project-wide tag-search picker
g (tree)   open the tag picker for the tree-cursor's paragraph
           (or every marked paragraph at once)
```

Inside the tag picker (`Ctrl+B ]` / `Ctrl+B }` / `g`):

```
Space      toggle the cursor tag
A          add a new tag (one-line prompt)
R          rename project-wide (1.2.6+; merges if name exists)
D          delete project-wide (confirm)
T          commit marked tags onto the target
Enter      Search mode: open the per-tag paragraph list
↑↓ Home/End navigate
```

### Story view (1.2.5–1.2.6)

```
Ctrl+V Shift+W   book story view (1.2.5+)
Ctrl+V w         paragraph mini story view (1.2.6+)
```

Inside either view:

```
S       save the rendered PNG to cwd
Esc     close
```

### Diagnostics (1.2.5–1.2.6)

```
F8                 (1.2.6+) typst diagnostics list modal
Ctrl+V N           next diagnostic in the open buffer
Ctrl+V Shift+N     previous
Ctrl+F12           (1.2.6+) AI explain the diagnostic at cursor
                   (was F11 pre-1.2.6 — macOS grabs F11)
```

Inside the F8 modal:

```
↑↓ Home/End    navigate
Enter          jump editor cursor to the diagnostic, close modal
Esc            close
```

### AI critique + diff modal (1.2.6+)

```
F12       AI critique (mode-aware: critique-edit / critique-changes)
```

Inside the AI diff-review modal (`r` / `g` in the AI pane
when `ai.diff_review_on_apply: true`):

```
a / A / Enter   accept — apply and refocus editor
r / R           reject — buffer unchanged
e / E           alias for `a`
↑ ↓ PgUp PgDn   scroll the diff
Home / End      jump top / bottom
Esc             same as reject
```

### Snapshot annotation prompt (1.2.6+)

```
F5    open the annotation prompt over the editor
```

Inside the prompt:

```
Type a line   build up the annotation
Enter         commit (empty = un-annotated)
Esc           cancel — no snapshot
```

### Render-preview zoom (1.2.6+)

Inside `Ctrl+V R`:

```
+ / =     zoom in  (multiply ticks/cell by 0.66)
- / _     zoom out (multiply by 1.5)
0         reset to 1.00×
```

### Story timeline (1.2.6 — opt-in)

```
Ctrl+V e         chronological event picker
Ctrl+V Shift+T   swim-lane timeline view
                 (lowercase Ctrl+V t stays bound to the
                  per-paragraph word-count target modal)
```

Inside Ctrl+V Shift+T:

```
← / →             scroll by ~10 cells
PgUp / PgDn       page by ~60 cells
+ / =             zoom in   (0.66× ticks/cell)
- / _             zoom out  (1.5×)
0                 reset zoom to 1.00×
Home / End        jump to first / last event in the visible set

u / U             up-scope    (subchapter → chapter → book)
d / D             open the inline descent picker
b / B             jump to book scope
p / P             toggle project overlay

Tab               cycle highlighted track
Enter             open the event closest to cursor
n / N             new event at cursor tick (annotation prompt)

y                 AI critique — current scope + current track
Y                 AI critique — current scope + all tracks
Ctrl+Y            AI critique — book scope (widens regardless)
Esc               close
```

Inside the descent picker (`d` from the swim-lane view):

```
↑ ↓ Home/End   navigate
Enter          descend into the selected scope
Esc            return to the same scope
```

Inside Ctrl+V e:

```
↑ ↓ Home/End   navigate
t / T          cycle the track filter (None → t0 → … → None)
Enter          open the event paragraph
Esc            close
```

## 1.2.7 — chord additions

### Paragraph undelete (1.2.7+)

```
Ctrl+B U       restore the most recently deleted paragraph
               (single-slot kill-ring; new uuid; paragraph links to
                old id stay broken).  Cleared by any branch
                delete or another single-¶ delete (the new one
                takes the slot).
```

See [`Tutorials/32-paragraph-undelete.md`](Tutorials/32-paragraph-undelete.md).

### Navigation history (1.2.7+)

```
Alt+←          step backward through visited paragraphs
Alt+→          step forward (after stepping back)
Ctrl+V Shift+P recent-paragraph picker (most-recent-first list,
               up to 32 entries, deduped against the previous)
```

The ring is in-memory only — restart clears it.  Opening a
new paragraph (via Enter / picker / paragraph link / undelete /
similar / timeline-Enter) clears the forward stack.

See [`Tutorials/33-navigation-history.md`](Tutorials/33-navigation-history.md).

### Mouse + external-change behaviour (1.2.7+)

```
Ctrl+Shift+M   toggle mouse capture on / off
               OFF lets the terminal handle drag-select +
                   native clipboard (Cmd/Ctrl+Shift+C).
               ON  restores click-to-focus + wheel-scroll
                   for the active pane.  Session-only;
                   defaults to ON.
```

External-change auto-reload has no chord — it runs passively
on every autosave tick.  Status bar reads:

```
↻ reloaded `<title>` — file changed on disk   (clean buffer)
⚠ `<title>` changed on disk while you have unsaved edits —
  Ctrl+S to overwrite the external change      (dirty buffer)
```

See [`Tutorials/34-mouse-and-external-changes.md`](Tutorials/34-mouse-and-external-changes.md).

### Timeline polish (1.2.7+)

Inside `Ctrl+V Shift+T` (swim-lane view) the navigation model
gained a second focus level mirroring the tree pane:

```
Focus = Track                         (default on open)
  Tab / Shift+Tab    cycle highlighted track
  Space              collapse / expand the focused track
  Enter              expand + drop focus into Event mode

Focus = Event
  Tab / Shift+Tab    cycle events of the expanded track in time
  Enter              open the linked-paragraphs picker for the
                     focused event
  Esc / Backspace    pop back to Track focus

Anywhere in swim lanes:
  ↑ / ↓              select previous / next event by start tick;
                     viewport auto-pans to show whole span
  F12                full-book AI health critique (same payload
                     as Ctrl+Y; alternative chord)
```

Session-restored state (per book, in `.session.json`):
collapsed tracks, expanded track, track highlight, zoom
(`ticks_per_cell`), scroll tick, cursor tick.

See [`Tutorials/31-story-timeline.md`](Tutorials/31-story-timeline.md) "1.2.7 polish".

### F8 from any pane (1.2.7+)

`F8` (typst-diagnostics list modal) now works from any pane,
not just the editor.  Opens against the most-recently-active
paragraph's cached diagnostics.

## 1.2.8 — chord additions

### Kill-ring picker (1.2.8+)

```
Ctrl+V Shift+U   open the kill-ring picker — list of the most
                 recent (up to 10) deleted paragraphs.  Enter
                 restores the cursor selection at its original
                 position.  Esc cancels.

Ctrl+B U         (existing) restores the front of the ring
                 without opening the picker.  Branch deletes
                 (chapter/book) no longer clear the ring —
                 older single-¶ entries remain valid recoveries.
```

### Hidden-character report (1.2.8+)

```
Ctrl+V h         one-shot scan of the open paragraph; status
                 bar reads e.g. "hidden chars: 3 tab(s), 5
                 line(s) with trailing whitespace, 0 CR(s)".
                 Clean buffers report "no tabs, trailing
                 whitespace, or CRs".  No buffer rewrite —
                 visual editor overlay is 1.2.9 work.
```

### Breadcrumb status-line chord (1.2.8+)

```
Ctrl+V Shift+S   print the cursor's hierarchy path on the
                 status bar (`Book ▸ Chapter ▸ Subchapter
                 ▸ Paragraph`).  Pane-aware: tree pane walks
                 from the tree cursor, editor pane walks from
                 the open paragraph.
```

### F1 query history (1.2.8+)

```
Inside the F1 Help-query input:
  Up             previous query (newest first); shell-style.
  Down           next; past the newest entry clears the input.
  Enter          submit; pushes the query onto the ring
                 (dedup against the immediate predecessor).
```

Session-only; F1 history is intentionally not persisted.

### Tag autocomplete (1.2.8+)

Inside the `A` (add-new-tag) prompt opened from `Ctrl+B ]`:

```
Tab              completes to the first existing project
                 tag whose name starts with the typed prefix
                 (case-insensitive).  No-op when no match.
```

### F6 annotation filter (1.2.8+)

Inside the F6 snapshot picker:

```
/                enter filter-focus mode — typed characters
                 narrow the visible list to snapshots whose
                 annotation contains the substring (case-
                 insensitive).
Esc (in filter)  exit filter focus (keeps the query).  Picker
                 returns to chord mode — Up/Down/Enter/D/V
                 again.
Backspace        edit the filter (in focus mode only).
Enter (in filter) commit filter (exits focus) — second Enter
                 loads the snapshot.
```

Filter resets each `F6` open — previous session's filter
doesn't haunt the next picker.

### Active-LLM chip in AI pane (1.2.8+)

The AI pane title always shows `· llm=<provider>` (the bound
`llm.default` from HJSON) so `Ctrl+B L` swap effect is visible
without opening `Ctrl+B I`.  In-flight provider fragment is
suppressed when it matches the bound default; surfaces only
when they diverge (user swapped default mid-stream).

### Shift+letter chord fix (1.2.8+)

Pre-existing bug — `Ctrl+V Shift+P` (recent-¶ picker) collapsed
onto `Ctrl+V p` (fuzzy picker) on terminals without the kitty
disambiguation protocol because the chord matcher required the
SHIFT modifier flag.  Now uppercase letters arriving without
SHIFT are treated as implicit-Shift — `Ctrl+V Shift+P`,
`Ctrl+V Shift+U`, `Ctrl+V Shift+S` all route to their distinct
actions.

### Mouse-capture default knob (1.2.8+)

```hjson
editor: {
  mouse_captured: true    // 1.2.8+ default
}
```

Setting `false` releases mouse capture at startup so the
terminal's native drag-select + system-clipboard copy work
without pressing `Ctrl+Shift+M` first.  The runtime
`Ctrl+Shift+M` toggle still flips state regardless.

### Embedded nushell pane (1.2.8+)

```
Ctrl+Z o         open / close the floating shell pane.
                 Engine state (env vars, defs) + turn
                 buffer + on-disk history all preserved
                 across close+reopen.
Ctrl+Z O         (Shift) drop the cached engine + in-
                 memory turn buffer and open fresh.  Does
                 NOT wipe `.inkhaven/shell_history.db`.
Ctrl+Z h         (inside the pane) toggle history-
                 selection mode.

Inside the pane (normal mode):
  Enter          run the line through the embedded
                 nu_engine; output + stderr land as a new
                 turn in the buffer.  Scroll is reset so
                 the new output is auto-visible.  Typing
                 `exit` (or `quit`) closes the pane
                 instead of forwarding to nu (whose
                 built-in `exit` would kill inkhaven
                 itself).
  Tab            autocomplete the token under the cursor.
                 In command position (start of line or
                 after `|` / `;`) matches against nu's
                 declared command set + executables on
                 $PATH; otherwise filesystem entries
                 under `$env.PWD`.  Single match →
                 splice + trailing space; multiple →
                 splice the longest common prefix and
                 surface the candidates on the status
                 line.

Line editing (readline-style):
  Ctrl+A / Ctrl+E    move cursor to start / end of line
  Ctrl+U             kill from cursor to start
  Ctrl+K             kill from cursor to end
  Ctrl+W             kill the word before the cursor
  Ctrl+Left/Right    move cursor by word
  Alt+B / Alt+F      move cursor by word (readline alias)
  Alt+Backspace      kill word backward
  Ctrl+L             clear scrollback (engine + history kept)
  Ctrl+D             clear input; if input is empty, close pane

Pane help:
  Ctrl+B H           open the OS Shell help overlay.
                     Any key dismisses it; pane state
                     (input, scroll, history) is preserved
                     unchanged underneath.
  ↑ / ↓          walk the per-project command history
                 ring (shell-style; Down past newest
                 clears the input).
  PgUp / PgDn    scroll the turn buffer up / down by 10
                 logical lines.  Title bar shows
                 `↑ scrolled` while above the newest turn.
  Shift+Home     jump to the top of the buffer.
  Shift+End      jump back to the newest output.
  Esc            close the pane (state preserved).

Inside selection mode:
  ↑ / ↓               walk the turn cursor.
  Home / End          jump to first / last turn AND scroll
                      the buffer to match.
  PgUp / PgDn         scroll independently of the cursor.
  c                   copy the highlighted turn's output
                      (stderr appended when failed).
  i                   insert the output into the editor
                      at cursor, wrapped in
                      `shell.insert_template`.  Pane
                      closes + editor refocuses.
  Esc                 exit selection (keep pane open).
  Ctrl+Z h            same — toggle back.
```

Pane gated on `shell.enabled = true` in HJSON (default
true).  See [`Tutorials/35-embedded-shell.md`](Tutorials/35-embedded-shell.md).

