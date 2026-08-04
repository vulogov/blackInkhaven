#import "../design.typ": *

#appendix(letter: "A", title: "The keybinding reference")

This appendix is the complete map of the desk — every chord Inkhaven's editor
answers to, grouped by where it lives, with a word on what each does. You do not
need to memorise it; press `Ctrl+B H` at any time for the same table inside the
tool, always current for your version and your customisations. Keep this appendix
for the mornings when you know there is a faster way and can't remember the keys.

#section("How the chords are organised")

Most keys fall into one of two families, reached through a _prefix_ you press and
release before the next key. Learn the two doors and the whole tool is two
keystrokes away.

#chord_table((
  chord_row("Ctrl+B …", "The meta chords — the machinery around the manuscript: the companion books, the world, the readers, building, and project-wide actions."),
  chord_row("Ctrl+V …", "The view chords — the readers and the connective tissue: links, citations, the Inner Editor, timelines, threads, and structure views."),
  chord_row("Ctrl+Z …", "The Bund chords — scripting: run a script, open a shell pane, evaluate an expression against the embedded VM."),
))

A handful of the most-used actions have no prefix at all (below), and some chords
change meaning by _scope_ — whether your focus is in the Editor or in the Tree —
which is noted where it matters.

#note[
  Every binding is rebindable. The chord table lives in a config Inkhaven reads at
  startup (and scripts can change with `ink.key.*`), so if a chord fights the muscle
  memory you brought from another editor, change it rather than fighting it. The
  chords printed here are the defaults.
]

#section("Core navigation — no prefix")

#chord_table((
  chord_row("Ctrl+Q", "Quit, saving first."),
  chord_row("Ctrl+S", "Save the current paragraph."),
  chord_row("Ctrl+1", "Focus the Editor — where you write."),
  chord_row("Ctrl+2", "Open the Outline — the whole book as a jumpable map."),
  chord_row("Ctrl+3", "Focus the AI chat pane, grounded on your book."),
  chord_row("Ctrl+4 / Ctrl+/", "Focus Search — full-text and semantic across the project."),
  chord_row("Ctrl+T", "Focus the Tree — the structure on the left."),
  chord_row("Ctrl+I", "The AI prompt line — a quick instruction without leaving the editor."),
  chord_row("Alt+← / Alt+→", "Back / forward through navigation history, like a browser."),
))

#section("Editing — no prefix")

#chord_table((
  chord_row("Ctrl+A", "Select all."),
  chord_row("Ctrl+C / Ctrl+K / Ctrl+P", "Copy / cut / paste."),
  chord_row("Ctrl+U / Ctrl+Y", "Undo / redo."),
  chord_row("Ctrl+D", "Delete the current line."),
  chord_row("Ctrl+E / Ctrl+W", "Delete to end / to start of line."),
  chord_row("Ctrl+F / Ctrl+X / Ctrl+R", "Find / find-next / replace within the paragraph."),
  chord_row("F3", "Load the paragraph into the working buffer."),
  chord_row("F4 / Ctrl+F4", "Split-edit: open a second version of the paragraph / accept it."),
  chord_row("F5", "Take a snapshot of the paragraph (same as Ctrl+B N)."),
  chord_row("Ctrl+H / Ctrl+J", "Scroll the lower split-edit pane."),
))

#section("Meta chords — `Ctrl+B`, in the Tree")

With focus in the Tree, the meta chords reshape structure.

#chord_table((
  chord_row("Ctrl+B c / s / p", "Add a chapter / subchapter / paragraph."),
  chord_row("Ctrl+B d", "Delete the selected node."),
  chord_row("Ctrl+B m", "Morph the node to another type."),
  chord_row("Ctrl+B ↑ / ↓  (u / j)", "Reorder the node up / down among its siblings."),
  chord_row("Ctrl+B q", "Imposition preview — how the pages will fall on a printed sheet."),
))

#section("Meta chords — `Ctrl+B`, in the Editor")

These reach the companion books and the per-paragraph tools.

#subsection("The companion-book lookups")

#chord_table((
  chord_row("Ctrl+B p", "Places — locations and settings (or insert an image)."),
  chord_row("Ctrl+B c", "Characters — the cast."),
  chord_row("Ctrl+B g", "Notes — free-form working notes."),
  chord_row("Ctrl+B y", "Artefacts — objects and items."),
))

#subsection("The paragraph tools")

#chord_table((
  chord_row("Ctrl+B n", "Snapshot the paragraph."),
  chord_row("Ctrl+B r", "Cycle the paragraph's status (napkin → … → ready)."),
  chord_row("Ctrl+B t", "Rename the node to its first sentence."),
  chord_row("Ctrl+B f", "The function picker — a menu of paragraph actions."),
  chord_row("Ctrl+B s", "Read the paragraph aloud (text-to-speech)."),
  chord_row("Ctrl+B q / Shift+Q", "Translate the paragraph into / out of an invented language."),
  chord_row("Ctrl+B d", "Deterministic rule-translate the paragraph to the Output pane."),
  chord_row("Ctrl+B Shift+X", "Fact-check the paragraph against the world and the facts."),
  chord_row("Ctrl+B Shift+S", "Search the Facts book."),
  chord_row("Ctrl+B Shift+J", "Jump to the next fact finding."),
  chord_row("Ctrl+B Shift+H / Shift+M", "Sentence-rhythm gauge / AI rhythm rewrite."),
  chord_row("Ctrl+B Shift+T", "Show-don't-tell scan of the paragraph."),
  chord_row("Ctrl+B Shift+F / Shift+K", "Style-warning overlay / echo (repeated-word) overlay."),
  chord_row("Ctrl+B Shift+R / Shift+E", "Save the paragraph as audio / reader-pace estimate."),
  chord_row("Ctrl+B < / >", "Jump to the previous / next scene break."),
))

#section("Meta chords — `Ctrl+B`, project-wide")

#chord_table((
  chord_row("Ctrl+B h", "The quick-reference overlay — this table, live."),
  chord_row("Ctrl+B i / v", "Book info / credits."),
  chord_row("Ctrl+B l", "The LLM provider picker."),
  chord_row("Ctrl+B a / b", "Assemble / build the book."),
  chord_row("Ctrl+B Shift+B", "Back up the whole project to an archive."),
  chord_row("Ctrl+B Shift+C", "The unified review pass — readers and checks in one sweep."),
  chord_row("Ctrl+B Shift+I", "The continuity ledger — every continuity break, ranked; Enter jumps, k runs the coherence pass."),
  chord_row("Ctrl+B Shift+A", "The read-through — the book read forward as a first reader: the shape curve, scene/sequel beat, and reader findings; Enter jumps, k runs the LLM first-read."),
  chord_row("Ctrl+B $", "The cost dashboard — what the AI features have spent."),
  chord_row("Ctrl+B w / Shift+W", "The World overview / typewriter (focus) mode."),
  chord_row("Ctrl+B j", "The Inner Socrates reading overview."),
  chord_row("Ctrl+B k", "The AI pane, full-screen."),
  chord_row("Ctrl+B x", "The ConLang hub — constructed languages."),
  chord_row("Ctrl+B Shift+O", "The Outline, project-wide."),
  chord_row("Ctrl+B 1–7", "Filter the tree by paragraph status."),
  chord_row("Ctrl+B ] / }", "Tag the paragraph / search by tag."),
  chord_row("Ctrl+B 0 / Shift+0", "Edit the HJSON config / open the doctor panel."),
  chord_row("Ctrl+B Shift+V", "The text-to-speech voice picker."),
  chord_row("Ctrl+B Shift+G", "The writing-streak heatmap."),
  chord_row("Ctrl+B Shift+L", "The concordance — every term and where it appears."),
  chord_row("Ctrl+B Shift+P / Shift+N", "The point-of-view chip / prompt-language mode."),
  chord_row("Ctrl+B e / o / u", "Toggle sound / take a 'take' / undo a deletion."),
))

#section("View chords — `Ctrl+V`")

The view chords open the readers and the web of connections between nodes.

#subsection("The readers and craft passes")

#chord_table((
  chord_row("Ctrl+V o", "The Inner Editor overview — craft attention on the draft."),
  chord_row("Ctrl+V v / Shift+V", "Prose voice check / ambient voice mode."),
  chord_row("Ctrl+V Shift+R", "An editorial pass over the current stretch."),
  chord_row("Ctrl+V y", "A style-transfer rewrite of the selection."),
  chord_row("Ctrl+V Space", "The command palette — search for a chord you've forgotten."),
))

#subsection("Links, citations, and reuse")

#chord_table((
  chord_row("Ctrl+V @", "The cite picker — drop a citation where the cursor is."),
  chord_row("Ctrl+V #", "The Typst Universe picker — insert a package #import (Ctrl+R refreshes)."),
  chord_row("Ctrl+V &", "The cross-reference picker — insert a @label reference to a defined label."),
  chord_row("Ctrl+V a / i", "Add a link / show incoming links to this node."),
  chord_row("Ctrl+V l / k", "List this node's links / backlinks."),
  chord_row("Ctrl+V t", "Open the link target under the cursor."),
  chord_row("Ctrl+V b / m", "Bookmark this node / list bookmarks."),
  chord_row("Ctrl+V x / Shift+X", "Insert a snippet / the snippet overview."),
  chord_row("Ctrl+V z / Shift+Z", "The terminology overlay / declare an intent."),
  chord_row("Ctrl+V p / Shift+P", "The paragraph picker / recent paragraphs."),
  chord_row("Ctrl+V Shift+B", "Look up a sibling book in a split pane."),
))

#subsection("Structure, world, and timeline")

#chord_table((
  chord_row("Ctrl+V g", "Progress toward goals (then e to edit goals)."),
  chord_row("Ctrl+V Shift+K / Shift+L", "The plan outline / the story bible."),
  chord_row("Ctrl+V w / Shift+W", "The story graph for the paragraph / the whole book."),
  chord_row("Ctrl+V Shift+N / Shift+M", "The character-arc view / the myth heatmap."),
  chord_row("Ctrl+V Shift+F", "A deep world refresh."),
  chord_row("Ctrl+V e / Shift+E", "The event picker / a new event."),
  chord_row("Ctrl+V Shift+T", "The timeline swim-lane."),
  chord_row("Ctrl+V Shift+H", "The threads picker — setups and payoffs."),
  chord_row("Ctrl+V Shift+A / Shift+D", "An AI thread audit / the thread doctor."),
))

#subsection("Submissions, comments, and misc")

#chord_table((
  chord_row("Ctrl+V u / q", "The submissions tracker / generate a submission package."),
  chord_row("Ctrl+V Shift+C", "The comments panel."),
  chord_row("Ctrl+V Shift+J", "The journal / intelligence dashboard."),
  chord_row("Ctrl+V Shift+U", "The kill-ring picker (recent cuts)."),
  chord_row("Ctrl+V 1 / 2", "Export the paragraph / subchapter as a Markdown buffer."),
  chord_row("Ctrl+V s", "Similar-mode — find passages like this one."),
))

#section("Bund chords — `Ctrl+Z`")

For scripting the tool itself in the embedded Bund language.

#chord_table((
  chord_row("Ctrl+Z r / n", "Run the script buffer / start a new script."),
  chord_row("Ctrl+Z e / ?", "The eval modal / the script picker."),
  chord_row("Ctrl+Z o / Shift+O", "The Nushell pane / full Nushell."),
  chord_row("Ctrl+Z h", "Run the shell on the selection."),
  chord_row("Ctrl+Z p / f", "A haiku / full-screen the right pane."),
))

#section("A word on scope and discovery")

Two ideas make the whole set learnable. First, _scope_: a bare letter after `Ctrl+B`
often does different things in the Tree than in the Editor — `c` adds a chapter in
the Tree but opens Characters in the Editor — because the tool knows what you are
looking at. Second, _discovery_: you never have to remember any of this cold.
`Ctrl+B H` prints the live table, and `Ctrl+V Space` opens a searchable palette
where you type what you want and let Inkhaven find the chord. Learn those two, and
every other key in this appendix is something you look up once and then, quietly,
stop needing to.
