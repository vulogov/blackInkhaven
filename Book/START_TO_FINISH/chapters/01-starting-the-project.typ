#import "../design.typ": *

#chapter(number: 1, title: "Starting the Project")

Every book begins the same way: with nothing. A title you are half-sure of, a
first scene you can almost see, and an empty place to put them. This chapter is
about making that empty place — turning "I want to write *The Ninth Lantern*"
into a project on disk you can open tomorrow morning and the morning after that.

We will run exactly one command, open what it made, and build the first three
rungs of the book — a book, a chapter, a paragraph — ending with a single
sentence saved to disk as plain text. It is a small amount of work. The point of
doing it slowly, once, is that everything else in this book happens *inside* the
thing we make here.

#callout(label: "This is the journey, not the reference")[
  Chapter 2 of *The Inkhaven Manual* names every file `init` writes and every
  one of the twenty seeded system books. This chapter does not repeat that
  anatomy — it *starts a specific book* and points to the manual when you want
  the full map. Read them side by side if you like; you do not need to.
]

#section("One command")

Open a terminal, go to wherever you keep your writing, and run `inkhaven init`
with the name of the project. That name becomes a directory — one project per
book is the convention — so pick the slug you want to live with. Ours is
`the-ninth-lantern`.

#screen(caption: "Starting The Ninth Lantern")[```
$ cd ~/Books
$ inkhaven init the-ninth-lantern
Initialized inkhaven project at
  /home/you/Books/the-ninth-lantern
  config:    …/the-ninth-lantern/inkhaven.hjson
  prompts:   …/the-ninth-lantern/prompts.hjson
  store db:  …/the-ninth-lantern/metadata.db
  vecstore:  …/the-ninth-lantern/vectors
  books:     …/the-ninth-lantern/books
```]

That is the whole of it. The directory did not exist a moment ago; `init` made
it and laid down a small, legible project inside. Two of those files are yours to
edit by hand — `inkhaven.hjson` (theme, language, AI provider) and
`prompts.hjson` (your prompt library) — and the rest are the local store:
`metadata.db` remembers the *shape* of the book, `vectors/` powers semantic
search, and `books/` is where your actual prose will live, one folder per book,
as ordinary text files you could read with any editor on earth.

#callout(label: "The first init is slower")[
  The very first time you ever run `init` on a machine, Inkhaven downloads a
  small multilingual embedding model (about 120 MB) into a shared per-user cache
  — not into the project. It is what makes search and the reading intelligences
  work, entirely on your own computer, offline. Every project after the first
  reuses it and starts instantly.
]

We took the default, which is an *empty* project — the system books and nothing
else. There is a `--template novel` that would have scaffolded a three-act
skeleton and a Characters stub for us, and it is a fine shortcut. We are
declining it on purpose: building the first few rungs by hand, once, teaches the
tree better than any template can, and *The Ninth Lantern* is short enough that
we lose nothing by starting from bare stone.

#section("Opening the empty project")

A project on disk is inert until you open it. Running `inkhaven` with no
arguments opens whatever project sits in the current directory, so step in and
launch it.

#screen(caption: "Opening the editor")[```
$ cd the-ninth-lantern
$ inkhaven
```]

The full-screen editor comes up. Chapter 3 walks its panes properly; for now
notice only that the left-hand *Tree* has focus, and that the project you
believed was empty is not. A stack of books is already there — the *system
books*, seeded into every project, protected so you cannot delete them by
accident, waiting to hold things you have not written yet: your cast, your
places, your sources, your facts. Anything you make lands *above* them.

#screen(caption: "A fresh project — the system books, waiting")[```
  TREE ─────────────────────────────────
▸ Notes            [book · notes]
▸ Research         [book · research]
▸ Sources          [book · sources]
▸ Facts            [book · facts]
▸ Places           [book · places]
▸ Characters       [book · characters]
▸ Threads          [book · threads]
  …fourteen more system books…
```]

This is worth a moment's respect before we write anything. Saltmarch will need a
*gazetteer* — the Long Mole, the nine pillars, the quays — and it already has a
home in *Places*. Mira and Aldous and the harbourmaster will need a *cast list*,
and *Characters* is standing ready. The secret at the centre of the book — that
Aldous put the lantern out himself — will one day be checked against *Facts* so
no one acts on it too early. None of that is our concern this morning. The point
is only that when each of those needs arrives, the shelf for it already exists,
tagged the same in this project as in every project you will ever open.

#two_track(
  [The world books — *Places*, *Characters*, *Facts*, *Threads* — are where a
  novel keeps its ground truth. You will feed them as you invent, and in return
  the editor highlights your names and the continuity readers get something to
  watch. We meet them properly in Part I's next chapters.],
  [The same shelves serve an argument. *Facts* holds a non-fiction book's
  verified claims instead of a world's invariants; *Sources* becomes your
  bibliography; *Research* holds the material you gathered. The book you are
  writing changes what goes on the shelves, never that they are there.],
)

#section("The first book")

The Tree is driven by single letters — one per level of the hierarchy. To lay
down the spine of a manuscript you need only three of them, plus Enter to open a
leaf for editing.

#chord_table((
  chord_row("B", "add a book at the root, above the system books"),
  chord_row("C", "append a chapter under the selected book"),
  chord_row("+", "append a paragraph under the selected branch"),
  chord_row("Enter", "open the selected paragraph in the Editor"),
  chord_row("F2", "rename the selected node"),
))

Press `B`. A small dialog asks for a title. This is the book itself — the top of
the tree, the thing the whole project is named for — so give it the real title.

#screen(caption: "Naming the book")[```
┌── Add book ──────────────────────────────────┐
│  Parent: <books root>                        │
│      Where: above the system block           │
│  Title : The Ninth Lantern▏                  │
│                                              │
│  Enter to confirm · Esc to cancel            │
└──────────────────────────────────────────────┘
```]

Press Enter, and *The Ninth Lantern* appears at the very top of the tree, sitting
above `Notes` where every book of yours will sit. It is empty — a titled branch
with nothing beneath it — but it is the book, and the cursor is on it.

#section("The first chapter, the first scene")

With the cursor on the book, press `C` to hang a chapter beneath it. The story
opens on the morning the ninth lantern is found cold, so that is the chapter's
name — "The Cold Lantern." Then, with the cursor on the chapter, press `+` to add
a paragraph under it. You may leave the paragraph's title blank; Inkhaven will
name it from your first sentence when you save, which for an opening scene is
exactly what you want.

#screen(caption: "The spine of the book, three rungs deep")[```
  TREE ─────────────────────────────────
▾ The Ninth Lantern
  ▾ The Cold Lantern
      ¶ Untitled paragraph
▸ Notes            [book · notes]
▸ Research         [book · research]
▸ Sources          [book · sources]
  …the other system books…
```]

That little tree is the whole model in miniature. The book is a folder. The
chapter is a numbered folder inside it. The paragraph is a numbered `.typ` file
inside *that* — the leaf, the actual unit of prose you edit, versioned and
searched on its own. A "paragraph" can hold a whole scene despite the name; think
of it as the smallest titled section Inkhaven keeps track of. All of it is plain
Typst text under `books/`, and the database beside it merely points at the files.

#callout(label: "Where this lives on disk")[
  After the next save, the tree above is mirrored one-to-one under `books/`:
  `books/the-ninth-lantern/01-the-cold-lantern/01-<slug>.typ`. Folders are
  branches, files are leaves, and the zero-padded numbers keep a plain directory
  listing in reading order. Chapter 2 of the manual dissects that mirror in full.
]

#section("The first words")

Put the cursor on the paragraph row and press Enter. Focus jumps to the Editor in
the middle pane, which shows the paragraph's starting template — a single Typst
heading line and an empty body waiting below it.

#screen(caption: "A new paragraph, before a word is written")[```
  EDITOR ───────────────────────────────
= Untitled paragraph

▏
```]

Press `End` to reach the end of the heading, Enter twice to leave it behind, and
write the first line of the book. It has been waiting since we picked the title.

#screen(caption: "The opening line of The Ninth Lantern")[```
  EDITOR ───────────────────────── ● edited
= Untitled paragraph

On the morning the ninth lantern went cold, Mira
Fenn was the first in Saltmarch to see it — a dark
pillar at the end of the Long Mole where, for three
hundred years, there had always been a flame.▏
```]

The moment you touched the buffer the border around the Editor turned *yellow* —
Inkhaven's plain signal that there is unsaved work. Press `Ctrl+S`. Three things
happen at once and none of them ask you anything: the `.typ` file is written
under `books/`, its metadata is updated, and a fresh embedding is computed so the
sentence is findable by meaning as well as by word. The border returns to
*green*.

Because we left the paragraph's title empty, one more thing happened — Inkhaven
took the title from the opening sentence, renamed the file on disk to match, and
the Tree row now reads it back to us.

#screen(caption: "Saved — the leaf names itself")[```
  TREE ─────────────────────────────────
▾ The Ninth Lantern
  ▾ The Cold Lantern
      ¶ On the morning the ninth lantern went…
▸ Notes            [book · notes]
```]

That is a complete round trip: a book, a chapter, a scene, saved to disk as plain
Typst, indexed for search, and named after its own first line. It is also the
whole loop you will run thousands of times — open a leaf, write, `Ctrl+S`, watch
the border go green — for the rest of the manuscript. Everything else this book
teaches is something you reach for *around* that loop.

To stop for the day, press `Ctrl+Q`. Inkhaven saves anything still dirty and
records the session, so tomorrow's `inkhaven` reopens this project with the
cursor back on this very paragraph. The blank project is no longer blank.

#section("A word about shape, before you fill it")

You now have one book, one chapter, one scene. Resist, for a moment, the urge to
pour in the next ten. The tree you build is the outline you will write against,
and it is worth a minute's thought about how to grow it — a difference that falls
out along the fiction / non-fiction line and recurs through the whole book.

#two_track(
  [Grow the tree as the *spine of the story*. One chapter per act or major
  movement of *The Ninth Lantern* — the cold lantern, the wrong suspicion, the
  walk onto the Mole, the reveal, the choice — with paragraphs as scenes beneath
  them. Drop names into *Characters* and *Places* as you invent them so the prose
  lights them up. The `novel` template lays exactly this down if you would rather
  not start from stone.],
  [Grow the tree as the *table of contents*. A chapter per part, subchapters for
  the sections beneath — the tree *is* the outline, and it becomes the document's
  structure verbatim. Put gathered material into *Research* and citations into
  *Sources* from day one, so every claim has a home and a bibliography assembles
  itself. The `nonfiction` and `technical` templates scaffold this shape.],
)

Neither shape is a promise. The tree reorders freely — rows move up and down,
paragraphs move between chapters, whole branches relocate — and every move
renumbers the files on disk for you, so you never think about the numbers at all.
Build the shape you can see this morning. Reshape it the moment the book tells you
to, which, if *The Ninth Lantern* is anything like every book before it, will be
soon.

#recap((
  [`inkhaven init the-ninth-lantern` makes a project directory and lays down a
  small, legible project: `inkhaven.hjson` and `prompts.hjson` are yours to edit;
  `metadata.db` / `vectors/` are the store; `books/` holds your prose as plain
  Typst. The first init on a machine downloads a shared embedding model to a
  per-user cache, not into the project.],
  [`inkhaven` with no arguments opens the project in the current directory. A
  fresh project is not empty — it is pre-seeded with protected *system books*
  (Notes, Research, Sources, Facts, Places, Characters, and more) that hold your
  world's ground truth, and your own books sit above them.],
  [The Tree is driven by single letters: `B` makes a book, `C` a chapter, `+` a
  paragraph, Enter opens a leaf, `Ctrl+S` saves, `Ctrl+Q` quits. We built *The
  Ninth Lantern* → *The Cold Lantern* → the opening scene, mirrored to disk as
  folders and a numbered `.typ` file.],
  [Saving computes a fresh embedding, writes the `.typ` file, and — for an
  untitled leaf — names it from the first sentence and reopens exactly there next
  launch. Grow the tree as the *spine of a story* or as a *table of contents*;
  either way it reorders freely and renumbers the files for you. The manual's
  Chapter 2 has the full on-disk anatomy.],
))
