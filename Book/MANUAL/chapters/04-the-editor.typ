#import "../design.typ": *

#chapter(number: 4, title: "The Editor")

The Editor is the pane you will live in. The last chapter mapped the whole
window; this one goes to the centre of it and stays there, because the great
majority of a writing session is one paragraph open in front of you and your
hands on the keys. Everything the Editor does — move, select, cut, find,
replace, save, split, check — is reachable without the mouse and without a
menu, and almost all of it follows the plain conventions your fingers already
know. The handful of places where Inkhaven departs from those conventions it
departs for a reason, and this chapter names each one so nothing surprises you
mid-sentence. Read it once for the shape, then let the muscle memory take over.

#callout(label: "One paragraph at a time")[
  The Editor holds a *single paragraph* — one `.typ` file — not a whole
  chapter scrolled end to end. This is deliberate: a paragraph is Inkhaven's
  unit of prose, of snapshot, of search, and of every reading intelligence in
  the later chapters. You move between paragraphs in the Tree (Chapter 5); you
  write *within* one here. Opening another paragraph saves this one first, so
  the boundary costs you nothing.
]

#section("Opening a paragraph, and reading its state")

You open a paragraph from the Tree: move the row cursor to it and press
`Enter`. Focus jumps to the Editor, the file's text loads into the buffer with
a line-number gutter down the left, and from that moment the pane is a running
readout of where the words stand. You never have to wonder whether your work is
safe — the Editor tells you, in three places at once.

The most vivid signal is the *border colour*, and it speaks only while the
Editor has focus. A *green* border means the buffer matches what is on disk —
you are saved, nothing is pending. A *yellow* border means you have unsaved
edits. When the pane is *unfocused* the border goes plain white and hands the
dirty signal off to two always-on indicators: the `[modified]` suffix in the
title, and the red `●` chip in the status bar. Between them you can tell the
state of your paragraph from anywhere in the window.

#term("Dirty")[
  A buffer is *dirty* when it holds edits not yet written to disk. Inkhaven
  shows dirtiness three ways — a yellow border (focused), a `[modified]` title
  suffix, and the red `●` in the status bar — and it clears all three the
  instant a save completes. "Clean" is the opposite: buffer and file agree.
]

The Editor's title carries the paragraph's name, the `[modified]` chip when
dirty, and a live cursor read-out in the form `L<row> C<col>` — the line and
column your cursor sits on, updated as you move. It is the quiet coordinate you
glance at when a message says "jump to line 40" or when you want to know how
far down a long paragraph you have wandered.

#screen(caption: "The Editor title — name, dirty chip, live cursor")[```
┌─ Editor · Opening Scene [modified] · L4 C18 ────────┐
│  1  The rain came sideways off the harbour, and     │
│  2  Mara counted the lamps as they went dark, one   │
│  3  by one, the length of the quay. Six days'       │
│  4  ride still lay ahead of her, and the city▏      │
│  5  behind had already begun to forget her name.    │
└─────────────────────────────────────────────────────┘
```]

One kind of paragraph opens in a state you cannot type into. If the paragraph
lives inside the *Help* system book, the Editor goes read-only: the border
turns teal, the title gains a `[read-only]` mark, and every key that would
change the text is intercepted with a `Help is read-only` status message.
Movement, copy, search, and scrolling all still work — you can read and lift
from Help freely — but `Ctrl+S` is a no-op and nothing you press alters the
file. Help is reserved for the material that answers `F1` lookups; your own
prose simply never lives under it.

#section("Moving the cursor")

Movement is entirely conventional, which is the point — your existing habits
transfer whole. The arrow keys move a cell or a line at a time. `Ctrl+←` and
`Ctrl+→` jump by *word*, landing on each word boundary, for crossing a line
faster than a character at a time. `Home` is *smart*: the first press lands on
the first non-blank column of the line — where the words actually start — and a
second press, once you are already there, goes on to column 0; `End` snaps to the
end of the line. `Ctrl+Home` and `Ctrl+End` leap to the very top and bottom of
the paragraph. `PageUp` and `PageDown` move by a viewport for the longer
paragraphs. There is nothing here to unlearn.

#chord_table((
  chord_row("← → ↑ ↓", "Move one character or one line."),
  chord_row("Ctrl+←", "Jump to the previous word boundary."),
  chord_row("Ctrl+→", "Jump to the next word boundary."),
  chord_row("Home", "First non-blank column; press again for column 0."),
  chord_row("End", "End of the current line."),
  chord_row("Ctrl+Home", "Top of the paragraph."),
  chord_row("Ctrl+End", "Bottom of the paragraph."),
  chord_row("PageUp / PageDown", "Move one viewport up / down."),
))

#subsection("Jumping between scene breaks")

A long paragraph of fiction is often a run of scenes separated by a break
line — `* * *`, `***`, `---`, `___`, `###`, `~~~`, or a lone `§`. Inkhaven
recognises all of these, and gives you a pair of chords to leap between them
without hunting: `Ctrl+B >` jumps the cursor forward to the next scene break,
`Ctrl+B <` back to the previous one. They are the fast way to walk the beats of
a scene-heavy passage. (The mnemonic borrows vim's angle brackets; the `<`
avoids a collision with the tag-search chord that wanted the same key.)

#chord_table((
  chord_row("Ctrl+B >", "Jump to the next scene break in the paragraph."),
  chord_row("Ctrl+B <", "Jump to the previous scene break."),
))

#section("Selecting text")

Inkhaven has two kinds of selection: the ordinary *linear* run you know from
every editor, and a *rectangular* block for column work. They are separate
models, and only one is ever active at a time.

#subsection("Linear selection")

Hold `Shift` and move: `Shift+←` and `Shift+→` extend the selection a character
at a time, `Shift+↑` and `Shift+↓` a line at a time. Combine the two — a few
`Shift+→` to reach across a phrase, `Shift+↓` to take whole lines — and you
build up exactly the run you want, drawn in reversed video so it stands out
against the syntax colours. `Ctrl+A` selects the entire paragraph at a stroke.
The cut, copy, and paste chords in the next section all operate on whatever
linear selection is live; with none, they act on the cursor position instead.

#subsection("Vertical block selection")

Sometimes what you want is not a run of text but a *rectangle* of it — a column
of leading numbers, a stack of names, the aligned left edge of a verse stanza.
The terminal-independent way in is `Ctrl+Z v`: it drops an anchor at the cursor
and enters block-select mode, and from there *plain* arrow keys (no modifier)
grow a rectangle from that anchor to the cursor, redrawn each frame in reversed
video. When the rectangle covers what you want, `c` or `Enter` copies it to the
system clipboard as a multi-line string — one line per row of the block — and
exits the mode; `Esc` (or any other key) cancels without copying. Because it
leans on nothing but plain arrows, it works on every terminal, macOS Terminal.app
included.

The older path still works where your terminal delivers it: hold `Alt` and move,
and `Alt+C` copies the rectangle. But `Alt`+arrow is exactly the input many
terminals drop or re-encode (see the terminal note in Chapter 1), and
`Alt+←`/`Alt+→` also collide with the back/forward navigation chords — so reach
for `Ctrl+Z v` first and keep `Alt`+arrows as the fallback.

#screen(caption: "A block selection — a rectangle, not a run")[```
┌─ Editor · Roster ───────────────────────────────────┐
│  1  ▓14▓  Mara      quay-watch                       │
│  2  ▓09▓  Tomas     ropewalk                         │
│  3  ▓22▓  Elin      the counting-house               │
│  4  ▓07▓  Bran      harbour gate                     │
│                                                     │
│  Ctrl+Z v · ↑↓←→ grow · c/Enter copy · Esc cancel   │
└─────────────────────────────────────────────────────┘
```]

#callout(label: "Block mode is copy-only")[
  Rectangular *copy* is supported; rectangular *cut* and *paste* are not, in
  this release — the multi-line character surgery they need is more than the
  underlying text widget exposes cleanly. Copy-only still covers the everyday
  cases: pulling out a column of figures, a list of names, or the aligned start
  of a stanza to reuse elsewhere.
]

#section("The clipboard — cut, copy, and paste")

Here is the one place the Editor departs from the keys your fingers expect, and
it departs for a concrete reason: the conventional `Ctrl+V` collides with the
terminal's own paste on many setups, and `Ctrl+X` and `Ctrl+Z` are already
spoken for elsewhere in Inkhaven. So the clipboard chords are chosen to sit on
distinct, unambiguous keys. `Ctrl+C` copies, `Ctrl+K` cuts, and `Ctrl+P`
pastes; `Ctrl+A` selects everything first. Learn this one triad and the rest of
the editor is convention.

#chord_table((
  chord_row("Ctrl+C", "Copy the selection to the system clipboard."),
  chord_row("Ctrl+K", "Cut the selection to the clipboard (marks dirty)."),
  chord_row("Ctrl+P", "Paste at the cursor, replacing any selection."),
  chord_row("Ctrl+A", "Select the entire paragraph."),
))

The clipboard is the *real, system* clipboard — Inkhaven talks to it through
the `arboard` library, so what you copy here you can paste into another
application and vice versa. On a headless or restricted session where no system
clipboard is reachable — a bare SSH login, some Wayland setups — the chords do
not fail. They fall back to an internal yank buffer, so cut, copy, and paste
keep working *within* the editor session; the only thing you lose is crossing
the process boundary to other apps.

A paste made with the *terminal's own* gesture — `Cmd`/`Ctrl+V`, a middle-click —
is handled as a *bracketed paste*: the whole run of text arrives as one bulk
insert rather than being replayed key by key. That is what you want for pasted
prose. Because the characters are not seen as individual keystrokes, a multi-line
paste never trips the auto-close pairs or a snippet expansion, and it never
submits at the first newline when you paste into the AI prompt bar or the Search
bar — the paragraph lands whole, and you press `Enter` yourself.

#callout(label: "Why Ctrl+P for paste")[
  If your muscle memory reaches for `Ctrl+V`, retrain it here to `Ctrl+P`.
  `Ctrl+V` is Inkhaven's *view* prefix (the pickers and panels of Chapter 3),
  and on many terminals it is also the native paste — leaving it as an editor
  chord would mean two pastes fighting over one key. `Ctrl+P` is the explicit,
  unambiguous Inkhaven paste, and it behaves identically everywhere.
]

#section("Deleting by the chunk")

Beyond `Backspace` and `Delete`, four chords remove whole *pieces* of a line at
once — the fast erasers for revision. `Ctrl+Backspace` deletes the word before
the cursor. `Ctrl+D` removes the entire current line and closes the gap.
`Ctrl+E` clears from the cursor to the end of the line; `Ctrl+W` clears from the
cursor back to the start. None of them touch your clipboard — each saves and
restores the yank buffer around itself — so a big delete never costs you the
last thing you copied.

#chord_table((
  chord_row("Ctrl+Backspace", "Delete the word before the cursor."),
  chord_row("Ctrl+D", "Delete the whole current line."),
  chord_row("Ctrl+E", "Delete from the cursor to the end of the line."),
  chord_row("Ctrl+W", "Delete from the cursor back to the start of line."),
))

#callout(label: "If your terminal eats Ctrl+W")[
  Some shells and multiplexers (bash, tmux) grab `Ctrl+W` for their own
  "delete previous word" before Inkhaven ever sees it. If `Ctrl+W` seems inert,
  that is your terminal layer intercepting it, not a bug — the fix is to rebind
  it in `inkhaven.hjson` or reach the same delete another way. Chapter 27 on
  configuration covers the rebinding.
]

#section("Undo and redo")

Undo is `Ctrl+U`; redo is `Ctrl+Y`. The history is *per paragraph* — each open
buffer keeps its own stack — so undoing in one paragraph never reaches back
into another. If you wonder why undo is not the usual `Ctrl+Z`, the answer is
the same distinctness principle as the clipboard: `Ctrl+Z` is Inkhaven's *Bund*
scripting prefix (Part VIII), and on many terminals the bare `Ctrl+Z` also
suspends the process to the shell's job control. Putting undo on `Ctrl+U`
sidesteps both. Reach for `Ctrl+U` to step back, `Ctrl+Y` to step forward
again.

#chord_table((
  chord_row("Ctrl+U", "Undo the last edit (per-paragraph history)."),
  chord_row("Ctrl+Y", "Redo."),
))

#section("A few editor conveniences")

Four small utilities live under the Bund prefix `Ctrl+Z`, where the free letters
were. `Ctrl+Z g` prompts for a line number and jumps there, centred in the
viewport — the quick way to a Typst compile error the diagnostics list reported
by line. `Ctrl+Z m` jumps to the *matching* bracket: put the cursor on (or just
after) one of `(`, `[`, or `{` and it hops to its partner, across lines and
respecting nesting, which keeps a long `#figure(…)` or `#footnote[…]` honest.
`Ctrl+Z w` toggles soft-wrap for the open buffer at runtime — wrapped for reading
prose, unwrapped for editing a wide table or code block — a session-only flip of
the persisted `editor.wrap` default. And `Ctrl+Z t` strips trailing whitespace
from every line as a single undoable edit, so `Ctrl+U` takes it back if you
change your mind.

#chord_table((
  chord_row("Ctrl+Z g", "Go to line — jump the cursor to a line number, centred."),
  chord_row("Ctrl+Z m", "Jump to the matching bracket (across lines, nesting-aware)."),
  chord_row("Ctrl+Z w", "Toggle soft-wrap for the open buffer (session-only)."),
  chord_row("Ctrl+Z t", "Strip trailing whitespace — one undoable edit."),
))

#section("Every change since the last save")

While you write, Inkhaven quietly renders *bold* every character you have added
since the paragraph was last saved. It is a running visual diff against the
on-disk version: at a glance you can see exactly what this editing pass has
touched, which is invaluable when you have been picking at a paragraph and want
to know what actually changed. The bolding is computed with a fast per-line
diff, so it keeps up with typing at literary scale, and it *clears the moment
you save* — bold means "new since disk", and a save makes the buffer and the
disk agree, so there is nothing left to mark. Watching the bold appear and then
dissolve on `Ctrl+S` is the simplest confirmation that your work has landed.

#section("Finding and replacing")

Search inside a paragraph is regex-powered and lives on two chords: `Ctrl+F` to
find, `Ctrl+R` to find-and-replace. Both take the full Rust regular-expression
syntax, so a plain word matches literally while `\bword\b` pins a whole-word
match, `(?i)` makes it case-insensitive, and `(?s)` lets `.` cross a line.

Press `Ctrl+F` and a magenta-bordered Find modal opens. Type a pattern, press
`Enter`, and every match in the buffer lights up in red while the cursor jumps
to the first match *at or after where the cursor already sits* — not back to the
top of the paragraph — wrapping to the first if the cursor is past the last one;
the status bar reports `match 1 / N` so you know how many you have. From there,
`Ctrl+X` is "repeat" — it advances to the next match and wraps at the end — and
`Ctrl+G` is its mirror, stepping to the *previous* match (also wrapping). The
match your cursor currently sits on is drawn a brighter red-and-bold so it stands
out among its siblings. `Esc` clears the search and drops the highlights,
returning you to plain editing.

#screen(caption: "Ctrl+F — the regex Find modal")[```
┌─ Find (regex) ──────────────────────────────────────┐
│                                                     │
│   Search:  the\s+thunder▏                           │
│                                                     │
│   Enter find · Ctrl+X next · Ctrl+G prev · Esc close │
└─────────────────────────────────────────────────────┘
```]

For replacement, press `Ctrl+R` instead. The modal grows a second field;
`Tab` switches between Search and Replace. The *first* press of `Enter` applies
one replacement — the current match — and keeps you in replace mode so you can
walk the rest one at a time with `Ctrl+X`. A *second* press of `Ctrl+R` while
replace mode is live does the wholesale thing: replace every remaining match
and exit. Re-opening either modal after a search pre-fills your last pattern
and replacement, so refining a search is a matter of editing what is already
there.

#screen(caption: "Ctrl+R — Find & Replace, two fields")[```
┌─ Find & Replace (regex) ────────────────────────────┐
│                                                     │
│   Search:   the\s+thunder                           │
│   Replace:  the storm▏                              │
│                                                     │
│   Enter one · Ctrl+R all · Tab field · Esc cancel   │
└─────────────────────────────────────────────────────┘
```]

There is one more reach in the replace modal. `Ctrl+B` toggles the *scope* of
the replacement between this paragraph and the *whole book*. In book scope,
`Enter` scans every paragraph in your user books and opens a review modal:
matches shown in context, `↑↓` to move, `Space` to skip the one under the
cursor, `a` to apply all, `n` to skip all, `Enter` to apply, `Esc` to cancel.
Book-wide replace starts in the safe whole-word literal mode, and `w`, `i`, `x`
toggle whole-word, ignore-case, and full-regex in place. Every paragraph it
changes is snapshotted first — annotated with the substitution — so `F6` is
always your undo. The same operation is available headless as `inkhaven
replace`, covered in the reference.

#callout(label: "Search matches a line at a time")[
  In-buffer find works line by line, so a pattern that must span a line break
  will not match. In practice the literary tasks — swapping a word, changing a
  character's name, fixing a repeated phrase — all sit within a line, so this
  rarely bites; when you truly need a cross-line change, the whole-book replace
  and the CLI are the wider tools.
]

#section("Saving, and the round trip")

Saving is `Ctrl+S`. It writes the paragraph's `.typ` file to disk, updates the
metadata, re-embeds the text into the semantic index so search stays current,
clears the dirty flag, and reloads the Tree so word counts refresh. The status
bar confirms it with the path and a word count. Because your prose is *plain
files*, this is a genuine round trip: the words Inkhaven holds and the words on
disk are the same bytes, and you are free to open, diff, or version-control
that file with any tool you like — Inkhaven will notice a change made from
outside and reload it.

#term("The round trip")[
  Inkhaven never locks your prose inside a private format. A paragraph *is* a
  `.typ` file; a save is an ordinary write of that file. Edit it in another
  program while Inkhaven is open and, if your buffer here is clean, Inkhaven
  silently reloads the newer version; if your buffer is *dirty*, it warns
  rather than clobber your edits and leaves the choice — overwrite with
  `Ctrl+S`, or reconcile — to you.
]

You will reach for `Ctrl+S` less than you might expect, because Inkhaven saves
for you at three natural moments. It saves on *idle* — after a few seconds of
not typing (the interval is `editor.autosave_seconds`, default five). It saves
on *paragraph switch* — opening another paragraph writes this one first. And it
saves on *focus loss* — the moment your attention leaves the Editor by `Tab`, a
direct jump, or `Esc` from another input, a dirty paragraph is written before
the next pane sees a keystroke. Between the three, moving around the window
costs you no work; the deliberate `Ctrl+S` is there for the moments you want
the confirmation.

#callout(label: "The crash mirror")[
  Against the rare case of a crash between saves, Inkhaven keeps a *mirror* of
  your unsaved buffer. Every couple of seconds while a paragraph is dirty (the
  window is `editor.crash_mirror_seconds`, default two) it pushes the current
  text and cursor position into the crash-report context, so if the program
  ever panics its handler can flush that buffer to a rescue file on the way
  down. The worst you can lose is one debounce window of typing — and a split
  or side-by-side second buffer is mirrored just the same.
]

#section("Split-edit — two versions at once")

Some revisions are easier when you can see where you came from. `F4` toggles
*split-edit*: the Editor area divides in half horizontally. The *upper* pane is
your live, read-write editor, exactly as full-featured as ever. The *lower*
pane is a frozen, read-only snapshot of the buffer as it stood the instant you
pressed `F4`, drawn in dim grey. You rewrite above while the earlier version
stays visible below.

Because a long earlier passage may not line up with where you are editing, the
lower pane scrolls on its own: `Ctrl+H` nudges it up a line, `Ctrl+J` down.
These two chords are routed to the snapshot pane *only while split is active*,
so they shadow nothing in ordinary use. When you are done, `F4` again closes
the split and drops the snapshot — or `Ctrl+F4` *accepts* it, replacing your
live buffer with the frozen copy. Acceptance is the clean way to roll back a
change you decided against: split, rewrite, and if the rewrite is worse, take
the old version wholesale.

#screen(caption: "F4 split-edit — live above, frozen snapshot below")[```
┌─ Editor · Opening Scene [modified] ─────────────────┐
│  1  The rain came hard off the harbour, and Mara    │
│  2  counted the lamps as they failed, one by one.▏  │
├─ snapshot · line 1/4 · Ctrl+H/J scroll ─────────────┤
│  1  The rain came sideways off the harbour, and     │
│  2  Mara counted the lamps as they went dark, one   │
└─────────────────────────────────────────────────────┘
```]

The lower half is one particular *snapshot*, and snapshots are Inkhaven's
point-in-time bookmarks of a paragraph: `F5` takes a fresh one, `F6` opens the
picker of every snapshot for this paragraph to load or compare. They are the
subject of their own chapter; here it is enough to know that split-edit builds
on the same idea — a captured earlier version you can hold beside the present
one and, if you wish, restore.

#chord_table((
  chord_row("F4", "Toggle split-edit; capture the snapshot on enter."),
  chord_row("Ctrl+F4", "Accept the snapshot — replace the live buffer."),
  chord_row("Ctrl+H", "Scroll the lower snapshot pane up (split only)."),
  chord_row("Ctrl+J", "Scroll the lower snapshot pane down (split only)."),
  chord_row("F5", "Take a fresh snapshot of the buffer."),
  chord_row("F6", "Open the snapshot picker (load / diff / delete)."),
))

#section("Grammar check — F7 and the g-apply")

Grammar checking is the Editor's most-used piece of AI, and it is built to be
safe: it never touches your prose until you say so. Press `F7` and Inkhaven
sends the open paragraph to your configured model with a grammar-and-punctuation
prompt keyed to the project's language, explicitly told to preserve every bit of
Typst markup. The review streams into the AI pane and focus moves there, so you
watch it arrive in real time. Nothing has changed in your buffer yet — the
check is a proposal, not an edit.

To *take* the correction, press `g` in the AI pane. This is the "grammar-check
apply": it lifts only the corrected paragraph out of the response — the model
returns it between markers so Inkhaven knows exactly which part is the clean
text — and overwrites the editor buffer with it. Because the grammar prompt
preserves Typst markup verbatim, the apply skips the usual markdown conversion
and drops the corrected prose in as-is. Every character that changed is then
rendered in red, so you can read the correction as a diff against what you
wrote.

#screen(caption: "F7 review in the AI pane, ready for the g-apply")[```
┌─ AI · llama · done · scope=Paragraph ───────────────┐
│  ai  <<<CORRECTED>>>                                │
│      The rain came sideways off the harbour, and    │
│      Mara counted the lamps as they went dark,      │
│      one by one, the length of the quay.            │
│      <<<END>>>                                      │
│                                                     │
│  g apply · c copy · r replace · Esc back to prompt  │
└─────────────────────────────────────────────────────┘
```]

Those red change-highlights stay up while you read back over what the grammar
pass altered, then clear on your *next save* — saving is the gesture that says
*I accept the corrections*. They also clear when you move to another paragraph
or the editor loses focus. If the response has no correction Inkhaven can lift,
`g` refuses with a status message rather than guessing.

#callout(label: "The AI never writes without your key")[
  `F7` only *shows* you a correction; `g` is the separate, deliberate keystroke
  that accepts it. This two-step — propose in the AI pane, apply on demand — is
  the same contract every AI-to-prose path in Inkhaven honours: the assistant
  advises, and no word of your manuscript changes until you press the key that
  says so. The change-highlights then let you audit exactly what it did.
]

#section("The reading-pace preview")

One editor-scoped tool is worth meeting here because it changes how a paragraph
*reads* rather than how it is edited. `Ctrl+B Shift+E` opens the *reading-pace
preview* — a teleprompter that walks a highlight through the open paragraph one
word at a time, at a reader's speed rather than an editor's glance (the rate is
`editor.reading_wpm`, default two hundred words a minute). Words already passed
dim, the current word is reverse-highlighted, the words ahead sit normal, and a
footer tracks your position and the time left. Experiencing your own prose at
the pace a reader will meet it surfaces problems the editing eye skims straight
over — a sentence that drags, a beat that lands too abruptly. `Space` pauses and
resumes, `←` and `→` step the highlight a word at a time, `r` restarts from the
top, and `Esc` closes it. It reads the same clean prose the audiobook export
uses, with the structural markup stripped away.

#chord_table((
  chord_row("F7", "Grammar-check the open paragraph (streams to the AI pane)."),
  chord_row("g", "In the AI pane: apply the correction to the buffer."),
  chord_row("Ctrl+S", "Save the paragraph — clears the grammar change-highlights."),
  chord_row("Ctrl+B Shift+E", "Reading-pace preview (teleprompter over the paragraph)."),
))

#recap((
  [The Editor holds *one paragraph* — a plain `.typ` file — and shows its save
  state three ways: a *green* (saved) or *yellow* (dirty) focused border, the
  `[modified]` title chip, and the red `●` in the status bar.],
  [Movement is conventional — arrows, `Ctrl+←`/`→` by word, `Home`/`End`,
  `Ctrl+Home`/`Ctrl+End`, `PageUp`/`PageDown` — with `Ctrl+B >` / `Ctrl+B <` to
  hop between scene breaks.],
  [Selection is either *linear* (`Shift`+arrows, `Ctrl+A` for all) or a
  *rectangular block* (`Ctrl+Z v` then plain arrows, `c`/`Enter` to copy — the
  terminal-independent path; `Alt`+arrows still work where delivered).],
  [The clipboard breaks convention on purpose: *copy* `Ctrl+C`, *cut* `Ctrl+K`,
  *paste* `Ctrl+P` — the real system clipboard, with an in-editor fallback when
  none is reachable.],
  [Chunk-deletes (`Ctrl+Backspace` word, `Ctrl+D` line, `Ctrl+E` to end,
  `Ctrl+W` to start) never touch the clipboard; undo is `Ctrl+U`, redo
  `Ctrl+Y`, per paragraph.],
  [`Ctrl+F` finds by regex from the cursor (`Ctrl+X` next, `Ctrl+G` previous),
  `Ctrl+R` replaces (again for replace-all, `Ctrl+B` to widen to whole-book with
  a review modal).],
  [`Ctrl+S` saves, but Inkhaven also autosaves on idle, on paragraph switch, and
  on focus loss, and mirrors your dirty buffer against a crash.],
  [`F4` splits the pane to edit against a frozen snapshot (`Ctrl+F4` accepts it,
  `Ctrl+H`/`Ctrl+J` scroll the lower half); `F7` grammar-checks and `g` applies
  the correction, with red change-highlights you dismiss via `Ctrl+B C`.],
))
