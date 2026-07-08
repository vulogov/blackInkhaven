#import "../design.typ": *

#chapter(number: 2, title: "The desk")

Whatever your track, you will spend your hours in the same place: a terminal, a
tree of chapters on the left, a paragraph under the cursor, and a set of chords
that summon everything else. This chapter is the tour of that desk. Read it once,
and every track guide after it will assume you can move around without being told.

#section("A project is a tree")

An Inkhaven project is not a pile of files; it is a *tree* of typed nodes. At the
top sit *books* (your manuscript, and the companion books Inkhaven keeps beside
it). Inside a book are *chapters*, inside chapters *subchapters*, and at the leaves
*paragraphs* — the unit you actually edit. Every node is real: you can move it,
split it, rename it, search it, and give it a status.

You start a project from the command line, optionally from a template shaped for
your track:

```
inkhaven init "my-book" --template novel
```

The templates — `empty`, `novel`, `nonfiction`, `technical`, `rpg-sourcebook`, and
`nanowrimo` — differ only in the starting structure they lay down; you can reshape
any of them freely afterward. From there, `inkhaven tui` opens the editor, and the
rest of your life happens inside it.

#term("Node")[
  Any element of the project tree — a book, chapter, subchapter, or paragraph —
  stored as a real, addressable object with its own text, status, tags, and links.
  Because structure is data rather than formatting, Inkhaven can reason about it:
  move a scene, search every paragraph by meaning, or render just the chapters you
  mark _ready_.
]

#section("The panes, and how to move between them")

The editor is a set of panes you focus with a single chord. You are always in one
of them; you switch with `Ctrl` and a number or letter.

#chord_table((
  chord_row("Ctrl+1", "The Editor — the paragraph under your cursor. Where you write."),
  chord_row("Ctrl+T", "The Tree — the structure on the left. Navigate and reshape the book."),
  chord_row("Ctrl+2", "The Outline — the whole book as an indented map you can jump through."),
  chord_row("Ctrl+3", "The AI chat pane — a conversation grounded on your book."),
  chord_row("Ctrl+4 / Ctrl+/", "Search — full-text and semantic search across the project."),
  chord_row("Ctrl+I", "The AI prompt line — a quick instruction without leaving the editor."),
  chord_row("Alt+← / Alt+→", "Back and forward through your navigation history, like a browser."),
))

#note[
  Search is not only literal. Inkhaven keeps a semantic index of your prose, so
  `Ctrl+/` can find the paragraph that _means_ what you typed even when it shares
  no words with your query — invaluable in a long manuscript where you remember a
  scene but not its wording.
]

#section("Editing a paragraph")

Inside the Editor the keys are close to what your fingers already expect, with a
few worth learning early:

#chord_table((
  chord_row("Ctrl+S", "Save. (Inkhaven also autosaves on quit — Ctrl+Q.)"),
  chord_row("Ctrl+U / Ctrl+Y", "Undo / redo."),
  chord_row("Ctrl+C / Ctrl+K / Ctrl+P", "Copy / cut / paste."),
  chord_row("Ctrl+D", "Delete the current line; Ctrl+E / Ctrl+W delete to end / start of line."),
  chord_row("Ctrl+F / Ctrl+X / Ctrl+R", "Find / find-next / replace within the paragraph."),
  chord_row("F4 / Ctrl+F4", "Split-edit: open a second version of the paragraph, then accept it."),
  chord_row("F5", "Take a snapshot of the paragraph (same as Ctrl+B N)."),
))

#subsection("Split-edit — revise without fear")

Pressing `F4` splits the editor into two panes holding two versions of the same
paragraph: the one you have, and a place to try another. You can rewrite freely in
the lower pane, compare them side by side, and either discard the attempt or
accept it with `Ctrl+F4`. It is the single most useful habit for revision — a way
to try a bolder sentence without losing the safe one until you are sure.

#subsection("Snapshots — a history you can walk back")

Every paragraph keeps a history of *snapshots*. Press `F5` before a big change and
Inkhaven records the current text; later you can walk back to any earlier version.
This is not the same as undo — snapshots survive across sessions and give you named
points to return to. Revise boldly; the earlier draft is never gone.

#section("Status and tags — telling parts apart")

A manuscript is rarely uniform: some scenes are finished, some are napkin sketches.
Inkhaven lets you mark each paragraph with a *status* — from `napkin` through
`first`, `second`, and `third` draft to `final` and `ready`, cycled with the
`Ctrl+B r` chord. You can then export only the `ready` paragraphs, or filter the tree to see what
still needs work. *Tags* (`Ctrl+B ]`) add your own labels, searchable across the
book. Both matter more on some tracks than others, but every track uses them to
answer "what is done, and what is left?"

#section("The companion books")

Beside your manuscript, Inkhaven keeps a shelf of *system books* — structured
places for everything that is not prose but supports it. You never create them by
hand; they appear when a feature needs them, and they are never exported into your
finished manuscript. The ones you will meet across the tracks:

#chord_table((
  chord_row("Places", "Locations and settings. Ctrl+B p."),
  chord_row("Characters", "The cast — sheets, arcs, agency. Ctrl+B c."),
  chord_row("Notes", "Free-form working notes. Ctrl+B g."),
  chord_row("World", "The worldbuilding model + its checkers. Ctrl+B W."),
  chord_row("Facts", "Continuity facts — what must stay true. Seeded per genre."),
  chord_row("Research", "Grounding material — the research assistant's corpus."),
  chord_row("Sources", "Your bibliography — citations that render to a reference list."),
  chord_row("Threads", "Plot threads — setup, development, payoff. Ctrl+V Shift+H."),
  chord_row("Glossary", "Controlled terminology — canonical terms and banned synonyms."),
  chord_row("Mythology", "Declared symbols, motifs, and archetypes."),
  chord_row("Language", "Constructed languages — one sub-book each. Ctrl+B X."),
))

Each is the home of a track's grounding. Which ones you live in depends entirely on
what you are writing — the novelist rarely opens Sources; the scientist rarely
leaves it.

#section("The two chord families: meta and view")

Almost everything beyond plain editing hangs off two prefixes. Learn the two doors,
and the whole tool is a keystroke away.

*`Ctrl+B` — the meta chords* reach the machinery around the manuscript: the
companion-book lookups above, plus `Ctrl+B B` to build the book, `Ctrl+B W` for the
World overview, `Ctrl+B J` for the Inner Socrates reading, `Ctrl+B Shift+X` to
fact-check the current paragraph, `Ctrl+B $` for the cost dashboard, and `Ctrl+B 0`
to edit the HJSON config from inside the editor.

*`Ctrl+V` — the view chords* reach the readers and the connective tissue:
`Ctrl+V o` for the Inner Editor overview, `Ctrl+V @` to insert a citation,
`Ctrl+V a` to add a link between nodes, `Ctrl+V Shift+T` for the timeline
swim-lane, `Ctrl+V Shift+R` for an editorial pass, and `Ctrl+V Space` for a
searchable command palette when you have forgotten the exact chord.

#tryit[
  You do not have to memorise any of this. Press `Ctrl+B H` for the quick-reference
  overlay — the full, searchable chord table, in the tool, always current. Keep it
  open in your first week. The chords become muscle memory faster than you expect.
]

#note[
  Every binding is rebindable. The chord tables live in a config Inkhaven reads at
  startup (and scripts can set with `ink.key.*`), so if a chord fights your muscle
  memory from another editor, change it rather than fighting it.
]

#section("Getting the book out")

When a draft is ready to leave the desk, one command assembles the chapters and
compiles them:

```
inkhaven build
```

or `Ctrl+B B` from inside the editor. For a specific format, `inkhaven export pdf`,
`inkhaven export epub`, and `inkhaven export docx` each render your manuscript —
scoped, if you like, to a status (`--status ready`) or a tag, and always excluding
the companion books. The `pdf` command alone carries a workshop of finishing
operations — imposition, booklets, covers, watermarks, preflight — for when the
book is genuinely going to press.

#insight[
  The whole desk rests on one idea: your book is _structured data_, not a formatted
  document. Because a paragraph is a real object with a status, a history, links,
  and a place in a tree, Inkhaven can search it by meaning, read it back to you,
  check it against a world, and render just the parts you choose. Everything in the
  track guides that follows is an application of that one fact.
]

#recap((
  [A project is a *tree of typed nodes* — books, chapters, subchapters, paragraphs — each real, movable, searchable, and given a status.],
  [You move between *panes* with `Ctrl` + a key (Editor, Tree, Outline, AI, Search) and edit with familiar keys plus *split-edit* (`F4`) and *snapshots* (`F5`) for fearless revision.],
  [*Companion books* (Places, Characters, World, Facts, Research, Sources, Threads, and more) hold everything that supports the prose; which you live in depends on your track.],
  [Two chord families reach everything: *`Ctrl+B`* (the machinery around the book) and *`Ctrl+V`* (the readers and connective tissue) — with `Ctrl+B H` as the always-current map.],
  [`inkhaven build` (or `Ctrl+B B`) and `inkhaven export pdf|epub|docx` turn the tree into a finished book, scoped by status or tag.],
))
