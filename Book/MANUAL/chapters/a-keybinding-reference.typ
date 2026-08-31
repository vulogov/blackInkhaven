#import "../design.typ": *

#appendix(letter: "A", title: "The Keybinding Map")

Inkhaven is driven from the keyboard. Almost every action in the editor has a
chord, and the ones that do not have a chord have a menu you reach with one.
This appendix is the complete, printable map of that keyboard — every chord the
TUI honours, organised by the pane or overlay that owns it. It is the reference
you keep open beside the tutorials until the muscle memory sets; it is also, on
a bad terminal day, the place you come to find out which key your multiplexer
just ate.

The map mirrors the shipping `Documentation/KEYBINDING.md`, cross-checked
against the binding tables in `src/tui/keybind.rs` — where the two ever
disagree, the code wins, and this appendix follows the code. A short note at
the very end lists the disagreements worth knowing about.

#section("The two prefixes, and how to read a chord")

Most single keystrokes belong to whatever pane has focus: a letter typed in the
editor is a letter, the same letter in the tree is a command. To reach the
hundreds of commands that are _not_ pane-native, Inkhaven uses *prefix chords* —
you press a prefix, release it, and the _next_ key selects the action.

There are three prefixes, all rebindable:

#chord_table((
  chord_row("Ctrl+B", [The *meta* prefix — hierarchy, panels, the reading intelligences, translation. The second key's meaning depends on which pane has focus.]),
  chord_row("Ctrl+V", [The *view* prefix — exporters, pickers, links, bookmarks, timeline, and the prose/craft tools.]),
  chord_row("Ctrl+Z", [The *Bund* prefix — the embedded scripting language and the OS shell pane.]),
))

A chord is written the way you type it: `Ctrl+B Shift+I` means _hold Ctrl and
press B, release both, then press Shift+I_. A plain `Shift+I` (no prefix in
front) is a single keystroke. Case is not significant to the parser — `Ctrl+s`,
`Ctrl+S` and `Ctrl+Shift+S` all match the same binding, because terminals
disagree about how they report case with modifiers held.

When a prefix is pending, the status bar shows a yellow *META* / *VIEW* / *BUND*
chip and lists the actions available for the focused pane. `Esc` cancels a
pending prefix without running anything; an unrecognised second key cancels with
a hint naming which pane's table it consulted.

#screen(caption: "The meta prefix is pane-specific")[```
  Ctrl+B ...   (press Ctrl+B, release, then one more key)

    Tree focus     c chapter · s subchapter · p paragraph
                   d delete · m morph-type · Up/Down reorder
    Editor focus   s read-aloud · n snapshot · t retitle
                   q translate · c character · p place
    AI focus       c clear chat
    Any focus      h help · w world · j inner readers · Esc cancel
```]

#callout(label: "Rebinding")[
  Every prefix, and every sub-chord under it, is configurable. The three
  prefixes live in the `keys` block of `inkhaven.hjson`; the sub-chords are
  listed as overrides in `keys.bindings` or bound at runtime through the
  `ink.key.*` Bund words. The full mechanism is in Appendix C and the
  configuration chapter — this appendix documents the shipping defaults.
]

#section("Global chords")

These work from any focus except where a narrower table overrides them. The
italic name in parentheses is the configurable `keys.*` slot, where one exists.

#chord_table((
  chord_row("Tab", [Cycle focus Tree → Editor → the active right pane → Tree (`next_pane`).]),
  chord_row("Shift+Tab", [Cycle focus in reverse (`prev_pane`).]),
  chord_row("Ctrl+/", [Focus the top Search bar (`search`).]),
  chord_row("Ctrl+I", [Focus the bottom AI prompt bar (`ai_prompt`).]),
  chord_row("Ctrl+S", [Save the open paragraph and re-embed it (`save`); no-op with nothing open.]),
  chord_row("Ctrl+Q", [Hard quit. Autosaves the open paragraph first; refuses to quit if that save fails, so the error stays on screen.]),
  chord_row("Ctrl+1", [Focus the Editor pane.]),
  chord_row("Ctrl+2", [Open the full-screen Outline pane. Use `Ctrl+B Shift+O` if your terminal re-encodes `Ctrl+2` as NUL.]),
  chord_row("Ctrl+T", [Focus the side Tree pane.]),
  chord_row("Ctrl+3", [Focus the AI pane.]),
  chord_row("Ctrl+4", [Focus the Search bar.]),
  chord_row("Ctrl+5", [Focus the AI prompt bar.]),
  chord_row("Ctrl+V Space", [Open the command palette — fuzzy-find any command by name, chord, or description and run it (`global.command_palette`).]),
  chord_row("Ctrl+Shift+M", [Toggle mouse capture on/off. Off lets the terminal do native drag-select and clipboard; on restores click-to-focus and wheel-scroll. Session-only.]),
  chord_row("Alt+←", [Step backward through visited paragraphs.]),
  chord_row("Alt+→", [Step forward again (after stepping back).]),
))

The function keys are global too, though a few only do their work with a
paragraph open in the editor:

#chord_table((
  chord_row("F1", [Open the Help-manual query pane — a grounded RAG answer over the Help book streams into the AI pane. One-shot; not added to chat history.]),
  chord_row("F8", [Open the Typst diagnostics list for the most-recent paragraph; `Enter` jumps the cursor to a diagnostic. Works from any pane.]),
  chord_row("F9", [Cycle the AI scope: None → Selection → Paragraph → Subchapter → Chapter → Book → Facts → Socrates → Editor → Graph → None. Sticky scopes persist; the rest auto-reset after one prompt.]),
  chord_row("F10", [Toggle inference mode Local ↔ Full. Local confines the model to the supplied context; Full lets it draw on general knowledge. Help stays pinned to Local.]),
  chord_row("Ctrl+F6", [Open the project-wide snapshot browser — every snapshot across all paragraphs, newest first. Works from any pane.]),
))

#section("The Tree pane")

The tree is focused on launch. It shows the hierarchy with kind glyphs
(`📖` book, `▸` chapter, `▹` subchapter, `¶` paragraph) and a dim word count on
each paragraph. The row of the paragraph currently open in the editor is marked
with a green bold `►` regardless of focus.

#subsection("Navigation")

#chord_table((
  chord_row("↑ / ↓", [Move the cursor one row up / down.]),
  chord_row("→", [Expand the cursor's branch, revealing its children. No-op on a paragraph or an already-open branch.]),
  chord_row("←", [Collapse the cursor's branch; if already collapsed (or on a paragraph), move to the parent.]),
  chord_row("Home / End", [Jump to the first / last row.]),
  chord_row("PageUp / PageDown", [Move the cursor 10 rows up / down (`page_up` / `page_down`).]),
  chord_row("Enter", [Open the cursor's node. Paragraphs load into the editor and shift focus there, autosaving any previously-open dirty paragraph; branches print a hint and stay.]),
  chord_row("Shift+Enter", [Pin the focused paragraph into the split-view secondary pane instead of opening it primary.]),
  chord_row("F2", [Open the Rename modal for the cursor node (changes the displayed title only; slug and file stay).]),
  chord_row("F3", [Open the file picker: `Enter` on a file imports it as a new paragraph; `Enter` on a directory recursively imports the tree.]),
  chord_row("q / Q", [Quit (autosaves the open paragraph first if dirty).]),
  chord_row("Esc", [Cycle focus to the Search bar.]),
))

#subsection("Modifier-free shortcuts")

These plain-key commands work only when the Tree pane has focus. They exist
alongside the `Ctrl+B` meta chords because terminals and multiplexers often
intercept the prefix. Uppercase (implicit Shift) is accepted; a real `Ctrl` /
`Alt` modifier is rejected, so `Ctrl+A` will _not_ fire add-subchapter.

#chord_table((
  chord_row("B / b", [Add a new book at the root, inserted above the system-book block.]),
  chord_row("C / c", [Append a chapter at the end of the book's children.]),
  chord_row("V / v", [Insert a chapter immediately after the cursor's enclosing chapter.]),
  chord_row("A / a", [Append a subchapter at the end of the chapter's children.]),
  chord_row("S / s", [Insert a subchapter immediately after the cursor's enclosing subchapter.]),
  chord_row("+", [Append a paragraph at the end of the parent's children.]),
  chord_row("P / p", [Insert a paragraph immediately after the cursor's enclosing paragraph.]),
  chord_row("D / d", [Delete the cursor's node — only if it is a branch (book/chapter/subchapter).]),
  chord_row("-", [Delete the cursor's node — only if it is a paragraph.]),
  chord_row("U / u", [Move the node up — swap with the previous sibling.]),
  chord_row("J / j", [Move the node down — swap with the next sibling.]),
  chord_row("Z / z", [Collapse the cursor's enclosing subchapter.]),
  chord_row("X / x", [Collapse every expanded branch in the tree.]),
  chord_row("Space", [Mark / unmark the cursor row for multi-select; `Esc` clears all marks.]),
  chord_row("T / t", [Cycle the leaf's type: paragraph(typst) → paragraph(hjson) → paragraph(jinja) → script. Walks the marked set if any.]),
  chord_row("E / e", [New Jinja template paragraph (rendered to Typst at assembly).]),
  chord_row("I / i", [New structural paragraph — pick a subtype (code / admonition / math / procedure / table) with matching boilerplate.]),
  chord_row("O / o", [Cycle status one rung up the ladder (napkin → first → … → ready → napkin). Walks the marked set if any.]),
  chord_row("G / g", [Tag the marked set (or the cursor row) — opens the tag picker scoped to every target.]),
  chord_row("y", [Copy the cursor paragraph onto the cross-pane clipboard (shared with the Outline pane).]),
  chord_row("m", [Move (cut) the cursor paragraph onto the clipboard.]),
  chord_row("f", [Affix the clipboard paragraph as the last child of the cursor's effective parent.]),
  chord_row("?", [Open the pane-aware Quick reference overlay (Tree pane only).]),
))

Why kind-specific delete? Safety: `-` will not nuke a whole chapter if your
cursor slipped onto it, and `D` will not kill a paragraph you meant to keep. If
you want a delete that ignores kind, use the global `Ctrl+B D`.

#section("The Editor pane")

The backing widget is `tui-textarea` driven without its built-in shortcuts, so
Inkhaven can intercept the modern conventions itself. The border colour carries
the dirty state at a glance while the pane is focused: green when saved, yellow
with unsaved edits, white when unfocused (dirtiness then moves to the title's
`[modified]` suffix and the red `●` chip in the status bar).

#subsection("Cursor movement")

#chord_table((
  chord_row("← / →", [One character left / right.]),
  chord_row("↑ / ↓", [One line up / down.]),
  chord_row("Home", [Smart Home — first press jumps to the first non-blank column; a second press (already there) jumps to column 0. `Shift+Home` still extends the selection to line start.]),
  chord_row("End", [End of the current line.]),
  chord_row("PageUp / PageDown", [One viewport up / down.]),
  chord_row("Ctrl+←", [Previous word boundary.]),
  chord_row("Ctrl+→", [Next word boundary.]),
  chord_row("Ctrl+Home", [Top of the document.]),
  chord_row("Ctrl+End", [Bottom of the document.]),
))

#subsection("Editing")

#chord_table((
  chord_row("any character", [Insert at the cursor; replaces the selection if one exists. With `editor.auto_close_pairs` on (the default), brackets auto-pair; quotes (`'` / `"`) auto-pair _only as opening quotes_ — never when adjacent to a word character — so `don't` stays `don't`.]),
  chord_row("Enter", [Insert a newline.]),
  chord_row("paste (terminal)", [A terminal paste — `Cmd`/`Ctrl+V`, middle-click — arrives as a _bracketed paste_: inserted in bulk, not replayed key by key, so a multi-line paste never trips auto-pair or a snippet and never submits at the first newline in the AI-prompt or Search bar.]),
  chord_row("Backspace", [Delete the character before the cursor (or the whole selection).]),
  chord_row("Delete", [Delete the character at the cursor.]),
  chord_row("Ctrl+Backspace", [Delete the previous word.]),
  chord_row("Ctrl+S", [Save to disk and re-embed; reloads the tree so word counts refresh.]),
))

#subsection("Selection, clipboard, undo")

The editor uses non-standard cut/paste keys because the conventional ones were
claimed by other functions (`Ctrl+X` is search-repeat; `Ctrl+Z` is the Bund
prefix). If `arboard` fails to initialise, copy/cut/paste fall back silently to
an internal yank buffer that stays within the session.

#chord_table((
  chord_row("Shift+← / Shift+→", [Extend the linear selection by one character.]),
  chord_row("Shift+↑ / Shift+↓", [Extend the selection by one line.]),
  chord_row("Ctrl+A", [Select the entire document.]),
  chord_row("Ctrl+C", [Copy the selection to the system clipboard.]),
  chord_row("Ctrl+K", [Cut the selection to the clipboard; marks the document dirty.]),
  chord_row("Ctrl+P", [Paste from the clipboard at the cursor (or over the selection).]),
  chord_row("Ctrl+U", [Undo.]),
  chord_row("Ctrl+Y", [Redo.]),
))

#subsection("Rectangular block selection")

A second, separate selection model, always rectangular, drawn reversed on top of
the syntax highlighting. Rectangular _cut_ and _paste_ are deferred in this
release; copy covers the common cases (a column of numbers, a list of names, a
verse stanza).

The terminal-independent way in is `Ctrl+Z v`, which extends the rectangle with
_plain_ arrows and so works on every terminal (macOS Terminal.app included). The
`Alt`+arrow path still works where the terminal delivers the modifier, but many
do not, and `Alt+←`/`Alt+→` also collide with back/forward navigation — prefer
`Ctrl+Z v`.

#chord_table((
  chord_row("Ctrl+Z v", [Enter block-select mode: anchor at the cursor, then _plain_ arrows extend the rectangle; `c` / `Enter` copy to the clipboard and exit, `Esc` (or any other key) cancels.]),
  chord_row("Alt+↑ ↓ ← →", [(legacy, only if the terminal delivers `Alt`) Enter block-select mode if needed, then move the cursor one cell without touching the linear selection.]),
  chord_row("Alt+C", [Copy the rectangle to the system clipboard as a multi-line string; clears the anchor.]),
  chord_row("Esc", [Cancel block-select; keep the document open.]),
))

#subsection("Line-targeted deletes")

Each of these saves and restores the yank buffer around the operation, so a
following `Ctrl+P` paste still produces the last copy.

#chord_table((
  chord_row("Ctrl+D", [Delete the current line and its trailing newline.]),
  chord_row("Ctrl+E", [Delete from the cursor to the end of the line.]),
  chord_row("Ctrl+W", [Delete from the cursor back to the start of the line. (Some terminals eat `Ctrl+W` as delete-word; rebind if yours does.)]),
))

#subsection("Find and replace (regex)")

In-buffer regex search over Rust's `regex` syntax. Matches highlight red; the
current match brightens.

#chord_table((
  chord_row("Ctrl+F", [Open the Find modal — type a regex, `Enter` to run; the cursor jumps to the first match _at or after the cursor_ (not the document top), wrapping if it is past the last.]),
  chord_row("Ctrl+X", [Repeat — in search mode jump to the next match; in replace mode replace the current match and advance. Active only while a search is in progress.]),
  chord_row("Ctrl+G", [Previous match — the mirror of `Ctrl+X`, stepping to the prior match (wraps). Active only while a search is in progress.]),
  chord_row("Ctrl+R", [First press opens Find & Replace and applies the first replacement; a second press in replace mode replaces every remaining match.]),
  chord_row("Ctrl+B", [(inside the modal) Toggle scope between this paragraph and the whole book; book scope opens a per-match review modal.]),
  chord_row("Esc", [Clear the active search — drops the highlights and exits replace mode.]),
))

#subsection("Split-edit, snapshots, files")

#chord_table((
  chord_row("F3", [Open the file picker; `Enter` on a file replaces the open paragraph's buffer. Directories are rejected here.]),
  chord_row("F4", [Toggle split-edit: a read-only snapshot of the buffer, captured on enter, in the lower pane.]),
  chord_row("Ctrl+F4", [Accept the split snapshot — replace the live buffer with the captured copy, exit split, mark dirty.]),
  chord_row("Ctrl+H", [(split only) Scroll the lower snapshot pane up one line.]),
  chord_row("Ctrl+J", [(split only) Scroll the lower snapshot pane down one line.]),
  chord_row("Shift+F4", [Toggle the full-screen two-paragraph split-view; the right pane is the secondary slot filled by any picker's `Shift+Enter`.]),
  chord_row("F5", [Save a versioned snapshot of the open paragraph (opens an annotation prompt).]),
  chord_row("F6", [Open the snapshot picker: `Enter` loads (safety-snapshots first), `V` diffs against current, `D` removes, `/` filters by annotation.]),
  chord_row("F2", [Rename the open paragraph.]),
  chord_row("F7", [Grammar-check the open paragraph; the review streams into the AI pane.]),
  chord_row("F12", [AI critique of the open paragraph — mode-aware (critique-edit, or evaluate-changes in split mode).]),
  chord_row("Ctrl+F12", [Send the Typst diagnostic at the cursor to the AI pane with an explain-or-fix prompt.]),
  chord_row("Esc", [Defocus to the Tree without closing the document (clears a block selection first if one is active).]),
))

#callout(label: "Autosave")[
  You never have to save to move. Whenever focus leaves the editor — via `Tab`,
  `Ctrl+1..5`, `Ctrl+T`, `Esc`, opening another paragraph — a dirty paragraph is
  saved automatically. An idle timer (`editor.autosave_seconds`, default 5) and
  quit both save too. Characters typed since the last save render bold until the
  next save clears the marker.
]

#section("The AI pane")

Focus lands here when you bounce off the AI prompt with `Esc`. The action keys
fire only once an inference is `Done` and non-empty; while streaming or on error
only `q` and `Esc` respond.

#chord_table((
  chord_row("Esc", [Bounce focus back to the AI prompt bar.]),
  chord_row("r / R", [Replace the editor selection (or the whole document) with the AI text; refocus the editor.]),
  chord_row("i / I", [Insert the AI text at the cursor.]),
  chord_row("t / T", [Prepend the AI text to the top of the paragraph.]),
  chord_row("b / B", [Append the AI text to the bottom of the paragraph.]),
  chord_row("c / C", [Copy the AI text to the system clipboard only — no editor change.]),
  chord_row("g / G", [Grammar-apply — lift only the corrected paragraph from the response and overwrite the buffer; the changed characters stay highlighted across saves.]),
  chord_row("q / Q", [Quit.]),
))

Where a response comes from a generator (a submission draft, a structure
analysis), an extra `L` files it as a paragraph into the matching system book.

#subsection("The Output / Thoughts pane")

The right region cycles between three panes; content auto-surfaces the relevant
one unless you are actively reading there. Findings arrive newest-first; the
selection is anchored to the message, not its row, so an incoming finding never
slides your cursor onto a different one. Rest on the top row and the pane
_follows the newest_ arrival (a `⟳follow` marker shows in the title bar); moving
down stops following, returning to the top resumes it.

#chord_table((
  chord_row("Ctrl+B Tab", [Cycle the right region Output → AI → Thoughts.]),
  chord_row("Ctrl+B Shift+Tab", [Cycle the right region in reverse.]),
  chord_row("Ctrl+Z f", [Fullscreen the current right pane (Output / Thoughts; the AI pane uses `Ctrl+B K`).]),
  chord_row("↑ ↓ (or k j)", [Select the previous / next message.]),
  chord_row("g / G", [First / last message.]),
  chord_row("o / Space", [Expand / collapse the selected message's structured detail.]),
  chord_row("a", [Ask the AI about the selected message (carries its full detail).]),
  chord_row("d", [Dismiss the selected message; the cursor stays on the row that shifts up into its place.]),
  chord_row("p", [Pin / unpin (pinned messages sort to the top).]),
  chord_row("Enter", [Jump to the source paragraph of _any_ finding that records one (fact-check, continuity, Socratic, …); findings with a kind-specific primary action — accept a proposal, insert a translation, jump to an event — still do that instead.]),
  chord_row("r / e", [(translations) remember / edit-and-remember.]),
  chord_row("i / m / x", [(Socratic) record-as-intent / make-note / mark-addressed.]),
))

Output filtering — the title shows `shown/total · filter`, persisted in
`.session.json`:

#chord_table((
  chord_row("f", [Cycle the source filter: off → fact-check → socrates → timeline → world → translation → lexicon → variety → ai → bund → other → off.]),
  chord_row("S", [Cycle the minimum severity: off → Info → Warning → Contradiction → off.]),
  chord_row("t", [Toggle this-paragraph-only — show just the messages tied to the open paragraph.]),
  chord_row("c", [Clear all filters.]),
))

#section("The AI prompt bar")

Activated by `Ctrl+I`. A leading `/` opens the Prompt picker; a leading `Help!`
routes the line through the F1 Help-RAG flow; anything else is sent verbatim as a
streaming inference.

#chord_table((
  chord_row("printable char", [Insert at the cursor. A leading `/` opens the Prompt picker; otherwise it closes.]),
  chord_row("Backspace / Delete", [Edit the buffer; refreshes the picker if visible.]),
  chord_row("← / → / Home / End", [Cursor movement within the buffer.]),
  chord_row("↑ / ↓", [(picker open) Move the selection.]),
  chord_row("Tab", [(picker open) Expand the selected template into the buffer with `{{selection}}` / `{{context}}` substituted.]),
  chord_row("Enter", [Picker open: expand the template. Otherwise: spawn a streaming inference; focus stays on the prompt bar.]),
  chord_row("Esc", [Picker open: close it. Otherwise: bounce focus to the AI pane to read the answer.]),
))

#section("The Search bar")

Activated by `Ctrl+/`. Semantic search over the whole project.

#chord_table((
  chord_row("printable char", [Insert at the cursor; closes the results overlay (the query changed).]),
  chord_row("Backspace / Delete", [Edit the query; closes the results overlay.]),
  chord_row("← / → / Home / End", [Cursor movement within the query.]),
  chord_row("↑ / ↓", [(overlay open) Move the result cursor.]),
  chord_row("Enter", [Overlay open: open the highlighted result. Otherwise: run the search and show results.]),
  chord_row("Esc", [Overlay open: close it. Otherwise: cycle focus to the Editor.]),
))

#section("The meta prefix — Ctrl+B")

The largest table in the app, and pane-specific: `Ctrl+B` then a key means
different things in the Tree, Editor and AI panes. The sub-tables below are in
that order, followed by the chords that fire from _any_ pane. `Esc` cancels a
pending meta prefix.

#subsection("Ctrl+B in the Tree pane")

#chord_table((
  chord_row("Ctrl+B C", [Add a chapter.]),
  chord_row("Ctrl+B S", [Add a subchapter.]),
  chord_row("Ctrl+B P", [Add a paragraph.]),
  chord_row("Ctrl+B D", [Delete the cursor's node (with confirmation) — kind-agnostic, unlike the plain `D` / `-`.]),
  chord_row("Ctrl+B M", [Morph the leaf's type: paragraph(typst) → paragraph(hjson) → paragraph(jinja) → script.]),
  chord_row("Ctrl+B ↑", [Reorder — swap with the previous sibling (also `Ctrl+B u`).]),
  chord_row("Ctrl+B ↓", [Reorder — swap with the next sibling (also `Ctrl+B j`).]),
  chord_row("Ctrl+B Q", [Imposition preview (Q for quire) — the production-layout plan for the selected book, over its built PDF. `Enter` imposes; tree-scoped so it never collides with the editor's `Ctrl+B Q`.]),
))

Book-adding has no meta chord — use the plain-letter `B` in the tree (see the
note at the end of this appendix).

#subsection("Ctrl+B in the Editor pane")

#chord_table((
  chord_row("Ctrl+B S", [Read the open paragraph aloud via the OS TTS engine. Gated by `editor.tts.enabled`.]),
  chord_row("Ctrl+B N", [New snapshot of the current buffer (same as F5).]),
  chord_row("Ctrl+B R", [Cycle the paragraph's status: None → Napkin → First → Second → Third → Final → Ready.]),
  chord_row("Ctrl+B F", [Open the Typst function picker — type to filter, `Enter` inserts `#name(…)`.]),
  chord_row("Ctrl+B T", [Retitle the paragraph from its first sentence.]),
  chord_row("Ctrl+B M", [Morph the paragraph's content type (as in the tree).]),
  chord_row("Ctrl+B P", [Inside `#image("…")`, pick a sibling image; otherwise run a Places RAG over the selection.]),
  chord_row("Ctrl+B C", [Character RAG — query the selection against the Characters book; the answer streams into the AI pane.]),
  chord_row("Ctrl+B G", [Notes RAG — query the selection against the Notes book.]),
  chord_row("Ctrl+B Y", [Artefacts RAG — query the selection against the Artefacts book.]),
  chord_row("Ctrl+B Q", [Translate the paragraph INTO an invented language defined under the Language system book; the result streams into the AI pane.]),
  chord_row("Ctrl+B Shift+Q", [Translate the paragraph FROM an invented language — the reverse direction, for round-trip testing.]),
  chord_row("Ctrl+B D", [Deterministic (rule-based) translation of the paragraph into an invented language; result and trace land in the Output pane.]),
))

The editor scope also owns a large family of prose-craft and analysis chords —
mostly `Ctrl+B Shift+…`. They are collected in the next table.

#subsection("Ctrl+B — prose, craft, and analysis")

Most are editor-scoped, but the toggles and overlays — `Ctrl+B Shift+F`,
`Ctrl+B Shift+L`, `Ctrl+B Shift+P`, `Ctrl+B Shift+N`, `Ctrl+B Shift+G`, and
`Ctrl+B Shift+Y` — fire from any pane; the rest are editor-scoped. Several open a
modal or stream into the AI pane.

#chord_table((
  chord_row("Ctrl+B Shift+F", [Toggle the inline style-warning overlay (filter words, hedges) — a session-local override of `editor.style_warnings`.]),
  chord_row("Ctrl+B Shift+K", [Toggle the live echo overlay — underline words echoing across nearby paragraphs of the chapter.]),
  chord_row("Ctrl+B Shift+H", [Open the sentence-rhythm gauge — sentence lengths, mean/stdev/CV, and a Monotone / Steady / Varied / Choppy verdict.]),
  chord_row("Ctrl+B Shift+M", [AI sentence-rhythm rewrite — mix short and long sentences; auto-opens a diff modal (accept snapshots first).]),
  chord_row("Ctrl+B Shift+T", [AI show-don't-tell scan — flags telling passages and proposes rewrites into the AI pane.]),
  chord_row("Ctrl+B Shift+L", [Open the project-wide concordance — every lexical stem with counts and KWIC samples; `Enter` jumps to a sample.]),
  chord_row("Ctrl+B Shift+P", [Toggle the status-bar POV / character chip.]),
  chord_row("Ctrl+B Shift+N", [Toggle prompt-language mode: None → book-defined → paragraph-detected → None (session-local).]),
  chord_row("Ctrl+B Shift+E", [Reader-pace preview — a teleprompter that advances word-by-word at `editor.reading_wpm`.]),
  chord_row("Ctrl+B Shift+Y", [Next stanza — create a sibling verse paragraph of the same `para:verse-*` type, opened for editing (structure only, never a line of verse).]),
  chord_row("Ctrl+B Shift+X", [AI fact-check against the Facts book — flags claims that contradict established world facts.]),
  chord_row("Ctrl+B Shift+S", [Facts semantic-search modal — find facts, mark several, send them to a targeted Facts chat.]),
  chord_row("Ctrl+B Shift+J", [Jump to the next fact finding from the last `Ctrl+B Shift+X` check.]),
  chord_row("Ctrl+B Shift+D", [Verify the open paragraph's `verify`-marked code listings through their runners; results to the Output pane.]),
  chord_row("Ctrl+B Shift+R", [Save the open paragraph as an audio file via macOS `say -o` (opens a path picker). macOS only.]),
  chord_row("Ctrl+B Shift+G", [Writing-streak heatmap — the last 91 days of word deltas as a 13×7 grid, with streak and totals.]),
))

#subsection("Ctrl+B in the AI pane")

#chord_table((
  chord_row("Ctrl+B C", [Clear the chat history and the currently-displayed inference (F9 drives the scope cycle instead).]),
))

#subsection("Ctrl+B from any pane")

#chord_table((
  chord_row("Ctrl+B H", [Open the pane-aware Quick reference overlay — the live keymap plus a static cheatsheet.]),
  chord_row("Ctrl+B V", [Open the version / author / credits pane.]),
  chord_row("Ctrl+B I", [Open the current book's info panel — paths, stats, PDF status.]),
  chord_row("Ctrl+B L", [Switch the active LLM provider; the choice is persisted to `inkhaven.hjson`.]),
  chord_row("Ctrl+B E", [Toggle typewriter sound effects (Enter / focus-out clicks); persisted.]),
  chord_row("Ctrl+B A", [Assemble the book — emit a Typst-compilable tree under the artefacts dir.]),
  chord_row("Ctrl+B B", [Build the book — assemble then run `typst compile` (PDF to the artefacts dir).]),
  chord_row("Ctrl+B O", [Take the book — build, then copy the PDF (and configured extras) into the launch directory.]),
  chord_row("Ctrl+B Shift+B", [Run a project backup now, ignoring the exit-hook recency cooldown.]),
  chord_row("Ctrl+B U", [Undo the most-recent paragraph delete (restores the front of the kill-ring). From every pane EXCEPT the Tree, where `Ctrl+B U` reorders the node up (see the Tree section).]),
  chord_row("Ctrl+B K", [Toggle full-screen AI mode — the AI pane fills the window with the chat history and prompt.]),
  chord_row("Ctrl+B W", [Open the World overview — the `world.hjson` definition and every compiled layer (astronomy, geology, climate, hydrology, demographics), each marked once materialized. `C` compiles the world.]),
  chord_row("Ctrl+B Shift+W", [Toggle distraction-free / focus mode — hide every other pane and give the editor the full window.]),
  chord_row("Ctrl+B J", [Open the Inner Socrates overview — the entrance to the whole inner-reader family (see the next section). From every pane EXCEPT the Tree, where `Ctrl+B J` reorders the node down (see the Tree section).]),
  chord_row("Ctrl+B X", [Open the ConLang hub — a read-only overview of every constructed language: phoneme inventory, template/constraint counts, prosody, romanization, lexicon size, speakers.]),
  chord_row("Ctrl+B z", [Open the knowledge-graph hub — `n` the paragraph's one-hop edges, `i` the edge inbox (P promotes, d rejects), `w` walks the graph to answer the AI-prompt question.]),
  chord_row("Ctrl+B Shift+O", [Open the full-screen Outline pane (the reliable backup for `Ctrl+2`).]),
  chord_row("Ctrl+B Shift+C", [Unified review pass — every fast deterministic checker at once (fact-check + Inner Socrates over the paragraph, timeline critique over the project) into the Output pane.]),
  chord_row("Ctrl+B Shift+I", [SENTINEL continuity ledger — the ranked deterministic continuity findings grouped by kind; `Enter` jumps, `k` runs the LLM coherence pass.]),
  chord_row("Ctrl+B Shift+A", [LECTOR read-through dashboard — the measured intensity curve, per-chapter scene/sequel beat, and reader findings; `k` runs the synthetic first-read.]),
  chord_row("Ctrl+B Shift+U", [CHRONICLE draft-history dashboard — the trend since your last milestone and the cleared-vs-introduced split; `m` marks this draft.]),
  chord_row("Ctrl+B Shift+Z", [KEN knowledge dashboard — epistemic continuity (premature_knowledge / leaked_secret / dropped_reveal); `Enter` jumps to the offending paragraph.]),
  chord_row("Ctrl+B $", [AI cost dashboard — today's LLM call tallies per capped subsystem against their daily caps.]),
  chord_row("Ctrl+B ]", [Tag the open paragraph — the tag picker scoped to the buffer.]),
  chord_row("Ctrl+B }", [Search by tag — the tag picker in read-only mode; `Enter` lists every paragraph carrying a tag.]),
  chord_row("Ctrl+B 0", [Edit project HJSON — a full-screen editor for `inkhaven.hjson`; `Ctrl+S` saves, `Ctrl+R` fires an LLM review.]),
  chord_row("Ctrl+B Shift+0", [Project doctor panel — the `doctor --scan` findings as a modal; `r` repairs the highlighted one, `R` repairs all.]),
  chord_row("Ctrl+B Shift+V", [Piper TTS voice picker — the catalog plus downloaded voices; `Enter` downloads/selects, `d` removes.]),
  chord_row("Ctrl+B 1..7", [Status filter — show only paragraphs of a given status under the cursor: 1 Ready, 2 Final, 3 Third, 4 Second, 5 First, 6 Napkin, 7 None.]),
  chord_row("Ctrl+B <", [(editor) Jump the cursor to the previous scene-break line.]),
  chord_row("Ctrl+B >", [(editor) Jump the cursor to the next scene-break line.]),
))

#section("The inner-family submenu — Ctrl+B J")

`Ctrl+B J` opens the Inner Socrates overview — the doorway to Inkhaven's family
of _inner readers_, each a different critical sensibility. From the overview a
second key branches into the family. All of them observe and question; none
rewrite your prose.

#chord_table((
  chord_row("Ctrl+B J then S", [Select the active Reader Persona.]),
  chord_row("Ctrl+B J then L", [View the intent ledger — the deliberate authorial choices the interrogator respects.]),
  chord_row("Ctrl+B J then F", [Fast-check the open paragraph (deterministic, free) → Output.]),
  chord_row("Ctrl+B J then E", [Engage the slow LLM Socratic track over the paragraph.]),
  chord_row("Ctrl+B J then A", [Toggle ambient Socratic checks.]),
  chord_row("Ctrl+B J then C", [Open a Socratic conversation about the paragraph.]),
  chord_row("Ctrl+B J then T", [Inner Theologian — a tradition-neutral reader poses moral questions through eleven lenses → the Thoughts / Output pane. It asks, never judges.]),
  chord_row("Ctrl+B J then P", [Inner Poet — scan the open verse paragraph's metre and rhyme against its declared `poem:` form. Sub-keys: F scan · E engage · D declare a form · T two-column translation · A ambient.]),
  chord_row("Ctrl+B J then Y", [Inner Stylist (CHORUS) — synthesise voice-at-scale (distinctiveness, POV/head-hop, tense, register). Sub-keys: F pillars → Output · E engage the coach · R the voice-report dashboard.]),
  chord_row("Ctrl+B J then R", [Inner Rigor — run the reasoning-rigor reader over the open paragraph.]),
  chord_row("Ctrl+B J then N", [New-persona wizard — create a new Reader Persona.]),
))

#callout(label: "The Inner Editor lives elsewhere")[
  The second inner reader, the *Inner Editor*, is reached not through `Ctrl+B J`
  but through `Ctrl+V O` (O for Observe) — because `Ctrl+B E` was already the
  sound toggle. It observes literary craft as Praise / Note / Concern. Its
  sub-keys: `E` engage · `C` converse · `A` ambient · `F` jump to findings.
]

#section("The view prefix — Ctrl+V")

The `Ctrl+V` family routes to in-process exporters, pickers, the
writing-progress tools, paragraph links and bookmarks, the timeline, and the
prose/craft checks. All of it is rebindable through `keys.bindings.view_sub`.

#subsection("Export, progress, targets")

#chord_table((
  chord_row("Ctrl+V 1", [(editor / AI) Export the open paragraph's buffer as markdown. (tree) Export the cursor node and all descendants.]),
  chord_row("Ctrl+V 2", [(editor / AI) Export the containing subchapter's subtree as markdown.]),
  chord_row("Ctrl+V S", [Toggle similar-paragraph mode — vector search opens a second editor side by side.]),
  chord_row("Ctrl+V G", [Open the writing-progress modal (today / streak / per-book pace / sparkline). `e` opens the goals editor.]),
  chord_row("Ctrl+V T", [Set / clear the per-paragraph word-count target (empty or 0 clears).]),
  chord_row("Ctrl+V Shift+G", [Project word-count goal modal — projects the finish date from the last-30-day delta.]),
))

#subsection("Links and bookmarks")

#chord_table((
  chord_row("Ctrl+V A", [Add an outgoing paragraph link — the tree enters select-to-link mode.]),
  chord_row("Ctrl+V I", [Add an incoming paragraph link — the reverse of `A`.]),
  chord_row("Ctrl+V L", [List outgoing links (floating picker; `D` removes one).]),
  chord_row("Ctrl+V K", [List backlinks — paragraphs that link to the open one.]),
  chord_row("Ctrl+V B", [Toggle a bookmark on the open paragraph.]),
  chord_row("Ctrl+V M", [Open the bookmark picker.]),
  chord_row("Ctrl+V Shift+B", [Sibling-book lookup — find the same-slug paragraph under a different book and pin it to the split-view secondary (the translation workflow).]),
))

#subsection("Pickers and navigation")

#chord_table((
  chord_row("Ctrl+V P", [Fuzzy paragraph picker over every user-book paragraph.]),
  chord_row("Ctrl+V Shift+P", [Recent-paragraph picker — the same list sorted by most-recently modified.]),
  chord_row("Ctrl+V Shift+U", [Kill-ring picker — choose any of the up-to-10 buffered deleted paragraphs to restore.]),
  chord_row("Ctrl+V R", [Render the open paragraph — compile it in-process and float a PNG preview; `S` saves a page, `A` saves all.]),
  chord_row("Ctrl+V N", [Jump the cursor to the next Typst diagnostic in the buffer.]),
  chord_row("Ctrl+V W", [Paragraph mini story-view — a radial graph of the paragraph's links and mentions.]),
  chord_row("Ctrl+V Shift+W", [Book story-view — the radial hierarchy graph of the whole book, rasterised and floated.]),
  chord_row("Ctrl+V H", [Hidden-character report on the open paragraph (tabs, trailing whitespace, CRs) — status bar only.]),
  chord_row("Ctrl+V Shift+S", [Show the cursor's breadcrumb path on the status bar (Book ▸ Chapter ▸ Subchapter ▸ Paragraph).]),
))

#subsection("Editing inserts")

#chord_table((
  chord_row("Ctrl+V @", [Cite picker — fuzzy-find a citation defined in the Sources book and insert `@key`.]),
  chord_row("Ctrl+V #", [Package import picker — insert an `#import "@preview/…"` line for a Typst Universe package.]),
  chord_row("Ctrl+V &", [Cross-reference picker — insert `@label` for a label defined in the manuscript.]),
  chord_row("Ctrl+V x", [Snippet `#include` — pick a reusable snippet from the Snippets book and insert (or replace) its include.]),
  chord_row("Ctrl+V Shift+X", [Snippets overview — every snippet with its reference count; `Enter` jumps to the source.]),
  chord_row("Ctrl+V d", [(editor) AI continuation drafting — continue the paragraph in your voice; the AI pane's `I` lifts only the draft block.]),
  chord_row("Ctrl+V f", [(editor) Insert an inline footnote — `#footnote[…]` (Typst) or a markdown `[^id]`.]),
  chord_row("Ctrl+V j", [(editor) Reflow the paragraph at the cursor to the editor's text width (one undoable edit).]),
  chord_row("Ctrl+V y", [(editor) Style-transfer rewrite — pick a reference paragraph; the AI rewrites the open one in that voice.]),
  chord_row("Ctrl+V Shift+Y", [(editor) WordNet thesaurus — replace the word under the cursor with a chosen synonym / antonym / hypernym / hyponym.]),
))

#subsection("Research and citation checks")

#chord_table((
  chord_row("Ctrl+V ?", [Confront the open paragraph against the research corpus (Facts + ingested Sources) — graded against / supporting findings to the Output pane.]),
  chord_row("Ctrl+V c", [Lint the open paragraph's `@key[locus]` citations against their sources' reference schemes (deterministic).]),
  chord_row("Ctrl+V Shift+C", [Sourcing pass — flag sentences that make a checkable factual claim but carry no `@key` citation.]),
))

#subsection("Structure, revision, the bible")

#chord_table((
  chord_row("Ctrl+V Shift+K", [Structure outline — the `plan check` report as a per-beat position bar with drift and a tension overlay; interactive (map beats, link threads, cycle status).]),
  chord_row("Ctrl+V Shift+R", [The Editorial Pass — one ranked worklist unifying every reader; `f` acts on a finding (rewrite / decision / brief), `F` batch-fixes. Every prose change is snapshotted first.]),
  chord_row("Ctrl+V Shift+L", [Story bible — every Character with its tracked attributes, plus Places, Artefacts, Facts, and the Glossary; `Enter` jumps to a source.]),
  chord_row("Ctrl+V Shift+F", [Deep AI world refresh in the background — facts check + scan + drift + continuity extract; advisory, writes sidecars only.]),
  chord_row("Ctrl+V Shift+J", [Manuscript intelligence dashboard — every metric (words, structure, pacing, threads, comments) in one view; `e` exports.]),
  chord_row("Ctrl+V O", [Inner Editor overview (see the inner-family callout above).]),
))

#subsection("Threads, prose voice, dialogue, character, myth")

#chord_table((
  chord_row("Ctrl+V Shift+H", [Threads picker over the Threads book; `Enter` opens a swim-lane weave view.]),
  chord_row("Ctrl+V Shift+A", [AI thread audit — send every thread's beats and waypoints to the LLM for blind spots.]),
  chord_row("Ctrl+V Shift+D", [Thread doctor modal — the same blind-spot checks deterministically (zero links / payoff unfired / dormant).]),
  chord_row("Ctrl+V V", [Prose voice check (NARR-1) in the background — chapter metrics that drift past threshold emit to Output.]),
  chord_row("Ctrl+V Shift+V", [Toggle ambient prose checks — re-run after an editing pause, gated by a cooldown.]),
  chord_row("Ctrl+V Shift+Q", [Dialogue fingerprint — the per-character voice signature for the nearest speaker.]),
  chord_row("Ctrl+V Shift+N", [Character arc — the declared arc, chapter-by-chapter state chain, agency scores, and coverage gaps.]),
  chord_row("Ctrl+V Shift+M", [Mythology heatmap — symbol density, motif presence, and archetype coverage per chapter → the Thoughts pane.]),
  chord_row("Ctrl+V z", [Toggle the terminology overlay — red-underline banned synonyms of canonical Glossary terms.]),
  chord_row("Ctrl+V Shift+Z", [(editor) Declare the banned synonym under the cursor a deliberate variant (stop flagging it).]),
))

#subsection("Submissions and the timeline")

#chord_table((
  chord_row("Ctrl+V u", [Submission tracker — the `.inkhaven/submissions.json` log; `Space` / `s` cycles status, `d` removes.]),
  chord_row("Ctrl+V q", [Submission-package generator — pick a query letter / synopsis / comps / logline; streams into the AI pane.]),
  chord_row("Ctrl+V e", [Chronological event picker over the story timeline.]),
  chord_row("Ctrl+V Shift+E", [New event at the cursor — opens the timeline and the new-event prompt.]),
  chord_row("Ctrl+V Shift+I", [Edit the timing metadata of the open event paragraph.]),
  chord_row("Ctrl+V Shift+T", [Swim-lane timeline view (lowercase `t` stays the word-count target).]),
))

#section("The Bund prefix — Ctrl+Z")

`Ctrl+Z` fronts the embedded Bund scripting language and the OS shell pane.

#chord_table((
  chord_row("Ctrl+Z R", [Run the current buffer as a Bund script.]),
  chord_row("Ctrl+Z N", [New Bund script.]),
  chord_row("Ctrl+Z E", [Open the eval modal.]),
  chord_row("Ctrl+Z ?", [Open the script picker — scripts in the cursor's branch; `A` toggles to the Scripts system book.]),
  chord_row("Ctrl+Z o", [Open / close the embedded nushell pane (state and history preserved across close).]),
  chord_row("Ctrl+Z O", [Drop the cached shell engine and turn buffer and open a fresh shell.]),
  chord_row("Ctrl+Z h", [(inside the shell) Toggle history-selection mode — `c` copies a turn, `i` inserts it into the editor.]),
  chord_row("Ctrl+Z p", [Emit a haiku to the Output pane, in the book's language.]),
  chord_row("Ctrl+Z f", [Fullscreen the current right pane (Output / Thoughts).]),
  chord_row("Ctrl+Z g", [(editor) Go to line — prompt for a line number and jump there, centred in the viewport.]),
  chord_row("Ctrl+Z m", [(editor) Jump to the matching bracket for the one at or just before the cursor — across lines, nesting-aware.]),
  chord_row("Ctrl+Z w", [(editor) Toggle soft-wrap for the open buffer at runtime (session-only; the persisted default is `editor.wrap`).]),
  chord_row("Ctrl+Z t", [(editor) Strip trailing whitespace from every line as one undoable edit (`Ctrl+U` reverts).]),
  chord_row("Ctrl+Z c", [(editor) Add an inline comment on the selection — a sidecar `.comments.json` beside the paragraph. Moved here from `Ctrl+V c`, which the LOCI citation check now owns.]),
  chord_row("Ctrl+Z Shift+C", [Open the project-wide comments panel over every `.comments.json` sidecar. Moved here from `Ctrl+V Shift+C`, now the sourcing check.]),
))

#section("Modals and overlays")

Overlays absorb input while they are up; the underlying pane keeps its visual
focus but sees no keys until the overlay closes. The conventions are consistent:
`Enter` confirms, `Esc` cancels, `Ctrl+Q` always hard-quits through a modal.

#subsection("Add modal")

#chord_table((
  chord_row("printable char", [Insert into the title buffer.]),
  chord_row("Backspace / Delete", [Edit the title.]),
  chord_row("← / → / Home / End", [Cursor navigation within the title.]),
  chord_row("Enter", [Commit — derive the slug, create the file and record, reload the tree, move the cursor to the new node.]),
  chord_row("Esc", [Cancel without creating anything.]),
))

#subsection("Delete-confirm modal")

#chord_table((
  chord_row("y / Y / Enter", [Confirm — remove the filesystem subtree and the records; the cursor lands on the parent.]),
  chord_row("n / N / Esc", [Cancel.]),
))

#subsection("File picker (F3)")

#chord_table((
  chord_row("↑ / ↓", [Move the cursor one entry.]),
  chord_row("PageUp / PageDown", [Jump by 10.]),
  chord_row("Home / End", [First / last entry.]),
  chord_row("→", [Expand a directory inline.]),
  chord_row("←", [Collapse an expanded directory, else move to the parent.]),
  chord_row("Enter", [Commit — replace the buffer (editor) or import the file/directory (tree).]),
  chord_row("Esc", [Cancel.]),
))

#subsection("Snapshot picker (F6)")

#chord_table((
  chord_row("↑ / ↓", [Navigate the snapshots (newest first).]),
  chord_row("Enter", [Load the selected snapshot (takes a pre-restore safety snapshot first).]),
  chord_row("V", [Side-by-side diff of the snapshot vs current; `Esc` returns to the picker.]),
  chord_row("D / Del", [Remove the snapshot.]),
  chord_row("/", [Enter filter mode — narrow by annotation substring.]),
  chord_row("Esc", [Cancel.]),
))

#subsection("Prompt picker")

#chord_table((
  chord_row("↑ / ↓", [Move the selection.]),
  chord_row("Enter / Tab", [Expand the selected template into the buffer.]),
  chord_row("Backspace / Delete", [Edit the filter; the picker re-filters live.]),
  chord_row("Esc", [Close without expanding.]),
))

#subsection("Search-results overlay")

#chord_table((
  chord_row("↑ / ↓", [Move the result cursor.]),
  chord_row("Enter", [Open the highlighted result.]),
  chord_row("Esc", [Close the overlay (the Search bar stays focused).]),
  chord_row("typing", [Close the overlay and continue editing the query.]),
))

#subsection("AI diff-review modal")

Pops when an AI rewrite lands and `ai.diff_review_on_apply` is on.

#chord_table((
  chord_row("a / A / Enter / e", [Accept — apply the rewrite (snapshot first) and refocus the editor.]),
  chord_row("r / R", [Reject — leave the buffer unchanged.]),
  chord_row("↑ ↓ PgUp PgDn", [Scroll the diff; `Home` / `End` jump to top / bottom.]),
  chord_row("Esc", [Same as reject.]),
))

#section("The mouse")

Inkhaven captures the mouse on startup (toggle with `Ctrl+Shift+M`). Left-click
moves focus to the clicked pane and positions the cursor there; gutter clicks in
the editor are ignored.

#chord_table((
  chord_row("Left-click", [Focus the clicked pane; position the tree row or editor character cursor.]),
  chord_row("Wheel (Tree)", [Move the tree cursor 3 rows per tick.]),
  chord_row("Wheel (Editor)", [Scroll the viewport 3 lines per tick.]),
  chord_row("Wheel (AI)", [Scroll the chat history (older messages on wheel up).]),
  chord_row("Wheel (modals)", [Scroll the shell turn buffer, HJSON editor, or picker cursor; other modals ignore it.]),
))

To select and copy text through the terminal's own clipboard while capture is
on, hold `Shift` (or `Option`, per your terminal) as you drag — that bypasses
capture and uses the terminal's native selection.

#callout(label: "When a chord never arrives")[
  If `Ctrl+S`, `Ctrl+Q`, or the `Ctrl+B` prefix seem dead, a layer above
  Inkhaven is eating them. `Ctrl+S` / `Ctrl+Q` are terminal flow control — run
  `stty -ixon`. `Ctrl+B` is tmux's default prefix — rebind tmux's prefix or
  remap Inkhaven's `meta_prefix`. When `Ctrl+Shift+arrow` is not transmitted,
  use the tree's plain-letter shortcuts (`B`, `C`, `A`, `+`, `D`, `-`) instead.
]

#section("A closing note, and two honest caveats")

That is the whole keyboard. The companion TUIs — `inkhaven research`,
`inkhaven worldbuilder`, and `inkhaven prompts-editor` — run outside the main
editor and carry their own, smaller keymaps, documented with each tool.

Two places where this map follows the code against older prose, worth knowing
because the printed defaults and your habits may disagree:

#chord_table((
  chord_row("Add a book", [There is no `Ctrl+B B` for it — that chord builds the book. Add a book with the plain-letter `B` in the Tree pane.]),
  chord_row("Ctrl+B (editor)", [`S` reads aloud, `R` cycles status, `F` is the function picker, `L` swaps the LLM — not the older save / history / split / load meanings some tables still show.]),
))

When in doubt, the live keymap is one keystroke away: `Ctrl+B H` from any pane
prints the Quick reference for the pane you are in, generated from the same
binding table this appendix was compiled against.
