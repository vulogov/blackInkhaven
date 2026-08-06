#import "../design.typ": *

#chapter(number: 2, title: "Your First Project")

A book, to Inkhaven, is a *directory*. Not a file with an opaque extension, not
a row in some cloud database you cannot see — a plain folder on your own disk,
with your prose in readable text files and a small local database beside them
that remembers the shape of the thing. This chapter creates one, opens it up,
and names every part, so that when a later chapter says "the Facts book" or
"the `.inkhaven/` sidecars" or "a structural leaf" you already know exactly
where on disk it lives and what it is for.

We will run one command, read what it laid down, meet the twenty *system books*
Inkhaven seeds into every project, and then make a book, a chapter, and a
paragraph of your own. By the end you will have a project you can open, close,
back up, and put under version control — and a mental map of every file in it.

#section("The one command that starts everything")

Everything begins with `inkhaven init`. It takes a single positional argument —
the directory the project should live in — and creates it if it does not yet
exist. The convention is one project per book (or per tightly-related set), kept
somewhere like `~/Books/`.

#screen(caption: "Creating a project")[```
$ inkhaven init ~/Books/my-first-project
Initialized inkhaven project at /home/you/Books/my-first-project
  config:    …/my-first-project/inkhaven.hjson
  prompts:   …/my-first-project/prompts.hjson
  store db:  …/my-first-project/metadata.db
  vecstore:  …/my-first-project/vectors
  books:     …/my-first-project/books
```]

The directory need not exist beforehand — Inkhaven makes it. If it *does* exist,
Inkhaven refuses to touch it silently, because `init` starts from a clean
database and re-initialising means *wiping* what is there. It asks first:

#screen(caption: "The overwrite guard")[```
$ inkhaven init ~/Books/my-first-project
Directory `/home/you/Books/my-first-project` already exists.
Remove it and re-initialise? [y/N]
```]

Only `y` or `yes` proceeds; a bare Enter, an `n`, or end-of-input all abort with
your files left untouched. There is one further safety interlock: Inkhaven will
*refuse* to wipe a directory that your current shell is sitting inside, because
deleting the ground beneath your own feet leaves the terminal in a broken state.
`cd` out first.

#subsection("The two flags")

`init` has exactly two options beyond the path.

#chord_table((
  chord_row("--force", "Skip the [y/N] prompt and overwrite an existing directory unattended. For scripts and fresh-checkout automation — never needed interactively."),
  chord_row("--template <name>", "Scaffold a starting tree instead of an empty one. Defaults to empty."),
))

The templates are pre-built manuscript skeletons applied *after* the standard
init, so a template never changes what init itself lays down — it only adds
books and chapters on top. Run `inkhaven template list` to see them all; the
most common are:

#screen(caption: "inkhaven init --template <name>")[```
empty          the default — system books only, no manuscript
novel          three-act manuscript + Characters stubs
nonfiction     intro / parts / conclusion + Research method
rpg-sourcebook Setting/Rules/Adventures + Places/Artefacts/Threads
technical      Overview / Reference / Tutorials / Index
nanowrimo      like novel, with a 50 000-word goal + pacing
```]

If you are unsure, take `empty`. You can build the same tree by hand in the TUI
in a minute, and this chapter shows you how. A template is a convenience, never
a commitment.

#callout(label: "First run downloads a model")[
  The very first `init` on a machine fetches a multilingual embedding model
  (about 120 MB) into a *per-user* cache — not into the project. On macOS that
  is `~/Library/Caches/dev.inkhaven.inkhaven/embeddings/`; on Linux
  `~/.cache/inkhaven/embeddings/`; on Windows under `%LOCALAPPDATA%`. Every
  later `init` reuses it, so only the first project pays the download. This is
  the model that powers semantic search and the reading intelligences — all of
  it runs on your own machine, offline.
]

#section("What init writes to disk")

`cd` into the new project and list it. Ignoring the model cache (which lives
elsewhere), a fresh project is small and entirely legible:

#screen(caption: "A fresh project, top level")[```
my-first-project/
├── inkhaven.hjson      the configuration file
├── prompts.hjson       your prompt library
├── metadata.db         hierarchy + node metadata (DuckDB)
├── blobs.db            paragraph bodies, snapshots, blobs
├── frequency.db        the full-text search index
├── vectors/            the HNSW semantic-search index
└── books/              your prose — one folder per book
```]

Two of these you will edit by hand; the rest Inkhaven owns. The two yours are
`inkhaven.hjson` (theme, language, AI provider, autosave cadence — Chapter and
appendix on configuration cover every field) and `prompts.hjson` (your library
of reusable AI prompts). The four database entries — `metadata.db`, `blobs.db`,
`frequency.db`, and the `vectors/` directory — are the local store, created and
maintained by the embedded database engine. You never open them with a text
editor.

#term("The store is three databases and an index")[
  Inkhaven's metadata lives in DuckDB files beside your prose. `metadata.db`
  holds the *hierarchy* — every book, chapter, and paragraph as a node with a
  stable UUID, its title, order, and file path. `blobs.db` holds paragraph
  bodies and versioned snapshots. `frequency.db` is the full-text index behind
  keyword search. `vectors/` is the HNSW index that makes *semantic* search and
  the reading intelligences possible. Your words themselves, though, are never
  trapped in there — they are the `.typ` files under `books/`, and the database
  merely points at them.
]

This split is the heart of Inkhaven's data model, and worth dwelling on for a
moment. The *prose is the filesystem*. The *database is an index over the
filesystem*. If the two ever drift apart — you edited a paragraph in another
editor, or a crash left them out of step — `inkhaven reindex` re-reads the
`.typ` tree and rebuilds the database from it. The files are the source of
truth; the database is derived, and therefore always rebuildable. That is why
your manuscript is safe even if you never trust the database at all: it is
ordinary text, in ordinary folders, that you can read, `grep`, diff, and commit
to Git without Inkhaven's help.

#subsection("Where session, backups, and caches live")

Beyond the store, Inkhaven keeps a handful of *dotfiles* and one *dot-directory*
in the project root. None of these exist right after `init` — they appear as you
use the project — but knowing them now saves confusion later.

#screen(caption: "The runtime sidecars (appear with use)")[```
my-first-project/
├── .session.json           cursor + which paragraph was open
├── .inkhaven-backup.json   timestamp of the last backup
├── .inkhaven.log           the runtime log (append-only)
└── .inkhaven/              advisory sidecars + local caches
    ├── drift.json          continuity-drift findings
    ├── facts_scan.json     fact-check results
    ├── tensions.json       pacing / tension curve
    ├── continuity.json     the continuity bible cache
    ├── submissions.json    submission-package tracker
    ├── ai_usage.json       rolling AI cost ledger
    ├── prose.duckdb        per-chapter prose metrics
    ├── voices/             character-voice profiles
    └── digest-<slug>.json  per-book cached digests
```]

The three top-level dotfiles are small and transient. `.session.json` remembers
which paragraph you had open and where the cursor sat, so re-opening the project
puts you back exactly where you left off. `.inkhaven-backup.json` is a single
timestamp the auto-backup logic checks against your configured `backup.max_age`.
`.inkhaven.log` is the append-only log — the first place to look when something
misbehaves.

The `.inkhaven/` directory is a growing collection of *advisory sidecars*: the
cached output of scans and the reading intelligences. Every file in it is
disposable. Each records what some analysis last found — drift, fact-checks,
tension curves, voice profiles — so that opening a view is instant instead of
re-computing from scratch. Delete any of them and Inkhaven simply recomputes on
next use. Crucially, *nothing here is your prose*: these are derived opinions
about your prose, kept to one side. Chapters throughout Parts IV and V describe
each sidecar where its feature is covered; for now, treat `.inkhaven/` as "the
scratchpad the intelligences share."

#callout(label: "For version control")[
  Commit `inkhaven.hjson`, `prompts.hjson`, and `books/` — the prose and its
  configuration. You may commit the `.db` files too (they are your index and
  snapshots), but they are large and rebuildable, so many authors add
  `*.db`, `vectors/`, `.inkhaven/`, `.session.json`, and `.inkhaven.log` to
  `.gitignore` and let `inkhaven reindex` rebuild the store on a fresh clone.
]

#section("The anatomy of the manuscript tree")

Open `books/` and you find the real structure. Inkhaven's hierarchy —
*Book → Chapter → Subchapter → Paragraph* — is mirrored one-to-one onto the
filesystem: branches are folders, leaves are files, and every entry carries a
zero-padded numeric prefix so a plain directory listing sorts in reading order.

#screen(caption: "One book on disk")[```
books/
└── my-first-book/
    ├── 01-chapter-one/
    │   ├── 01-the-storm.typ
    │   ├── 02-landfall.typ
    │   └── 02-a-quiet-town/          ← a subchapter
    │       ├── 01-the-inn.typ
    │       └── 02-the-innkeeper.typ
    └── 02-chapter-two/
        └── 01-morning.typ
```]

Read that tree carefully, because its rules are the whole model:

- A *book* is a folder named by its bare slug (`my-first-book`) — no number,
  because books are ordered among themselves in the database, and their folders
  sit at the top level of `books/`.
- A *chapter* and a *subchapter* are numbered folders (`01-chapter-one`,
  `02-a-quiet-town`). A subchapter is simply a chapter-level folder that lives
  inside a chapter — the same kind of container, one level down.
- A *paragraph* is a numbered `.typ` *file* (`01-the-storm.typ`). This is the
  leaf: the actual unit of prose you edit. Despite the name, a "paragraph" can
  hold as much text as you like — think of it as a titled *section* of the
  manuscript, the smallest thing Inkhaven versions, embeds, and re-orders.

The numeric prefixes are Inkhaven's bookkeeping, not yours to hand-edit. When
you reorder rows in the tree, Inkhaven renumbers the files on disk to match; the
slug (the human-readable part after the number) comes from the title, and a
paragraph with an empty title gets its slug — and title — from the first
sentence you type on first save.

#term("The manuscript is a Typst project in disguise")[
  Each paragraph file is a fragment of #link("https://typst.app")[Typst], the
  typesetting language. A new paragraph opens with a single `=` heading line —
  Typst's level-one heading marker — so the paragraph renders as its own section
  in the final book. When you export, Inkhaven walks the tree in order,
  assembles the fragments into one Typst document, and hands it to the
  typesetter. Nothing about the on-disk format is proprietary: it is Typst, all
  the way down.
]

#section("The node model at a glance")

Every row in the tree is a *node*, and every node has a *kind*. Four kinds are
the structural spine you have already met — Book, Chapter, Subchapter,
Paragraph. Alongside the prose paragraph, though, Inkhaven allows several
*non-prose leaves*: siblings in the same tree that carry something other than
Typst prose. Each has its own on-disk extension and its own chapter later in
this manual; here they are only named, so you recognise them in a tree.

#screen(caption: "Leaf kinds at a glance")[```
KIND          ON DISK        WHAT IT HOLDS
paragraph     NN-slug.typ    prose (Typst) — the default leaf
image         NN-slug.png    a picture: png / jpg / webp / svg
hjson  ❴      NN-slug.hjson  structured data (a place, a source…)
jinja  ⟡      NN-slug.jinja  a template rendered to Typst on export
bund   λ      NN-slug.bund   an embedded Bund script
```]

A few notes to fix the picture:

- An *image* is a first-class node, not a paragraph. Drop a `.png`/`.jpg`/
  `.webp`/`.svg` into the tree and Inkhaven ships the bytes into the assembled
  book with the right image call; a caption and alt-text ride along in the
  metadata.
- An *hjson* leaf is a paragraph whose content type is data rather than prose.
  This is how the world books store structure — a place, a character, a
  bibliography entry — as an HJSON record instead of paragraphs of text.
- A *jinja* leaf is a template: it holds Jinja markup that Inkhaven renders down
  to Typst at assembly time, for repeated or generated structure.
- A *bund* leaf is a script in Inkhaven's embedded Bund language, evaluated into
  the scripting engine when the project opens — the home of custom hooks and
  rules. Its default home is the Scripts system book, but it can live anywhere.

There is one more flavour that is *not* a separate file type but a *subtype* of
an ordinary Typst paragraph — the *structural paragraph*. In the editor, the
structural-subtype picker turns a paragraph into a piece of recognised
scaffolding — code block, admonition, mathematics, procedure, or table — each
with its own glyph in the tree (`⌨ ⚠ ∫ ≡ ⊞`). It is still a `.typ` file; the
subtype only tells Inkhaven how to treat and render it. A single leaf can be
cycled through its types — `typst → hjson → jinja → bund` — as your needs
change. Every one of these gets a full treatment in a later chapter; for now,
the point is simply that *the tree holds more than prose*, and each non-prose
leaf is still a plain, readable file.

#section("The system books")

Here is the part that surprises people opening a fresh project: it is not empty.
Before you have written a word, `inkhaven list` shows a stack of books already
there. These are the *system books* — reference and machinery books Inkhaven
seeds into *every* project, on every open, whether you use them or not. They
carry stable internal tags, they are marked *protected* (you cannot delete or
rename them), and anything you create lands *above* them, at the top of the
tree.

#callout(label: "How many, really")[
  Older documentation counted "six" and then "nine" system books; the project
  has grown since. The authoritative list is the `SYSTEM_BOOKS` table in the
  source, and as of this edition it seeds *twenty*. If a future release adds
  one, it appears automatically the next time you open any existing project —
  the seeder is idempotent and back-fills.
]

They divide cleanly into three groups: reference books you write into, world
books that hold your fiction's ground truth, and machinery books Inkhaven and
its tools own. First, the reference and prose books:

#screen(caption: "System books · reference & prose")[```
Notes        free-form scratch prose, kept out of the manuscript
Research     research notes and gathered source material
Sources      bibliography entries (HJSON) → compiled to a .bib
Glossary     canonical terms + banned synonyms (terminology)
Snippets     reusable Typst blocks, #include'd elsewhere
Prompts      your AI prompt library (seeded with examples)
Help         Inkhaven's own manual — F1 answers by RAG here
```]

Then the *world* books — the ground truth a work of fiction must stay
consistent with. These are where Parts IV and V of this manual do their work:

#screen(caption: "System books · the world")[```
Facts        the world's invariants — the fact-check reference
Places       your gazetteer — locations, named and highlighted
Characters   the cast — names light up in the editor
Artefacts    objects of significance — items, relics, devices
World         simulation output (the realworld compiler writes here)
Threads      narrative plot threads and their arc shapes
Planning     story structure — beats and acts of a framework
Mythology    declared symbols, motifs, and archetypes
Intent       your declared authorial choices (silences findings)
```]

And finally the *machinery* books — homes for the tooling that surrounds the
prose:

#screen(caption: "System books · machinery")[```
Typst        reusable Typst templates and skeletons
Scripts      .bund scripts auto-loaded into the engine at open
Language     invented-language books (one child book per conlang)
Submissions  the submission package — query letter, synopsis…
```]

A handful of these deserve a sentence of *why* now, because you will meet them
again and again:

- *Notes* and *Research* are the free-form catch-alls — prose that supports the
  book without being in it. The AI can be scoped to read them.
- *Facts* is the reference the continuity and fact-checking tools ground
  against: the climate, the geography, the chronology your chapters must not
  contradict. *Grounding Your Book in Fact* is the companion book for it.
- *Places*, *Characters*, and *Artefacts* are *lexicon* books — Inkhaven
  highlights their names where they appear in your prose, and you can ask the AI
  about any of them by name.
- *Prompts* ships pre-seeded with `.example` prompts for the built-in AI
  actions; rename one to drop the `.example` suffix and it overrides the
  default. *Help* is Inkhaven's own documentation, which is why pressing `F1`
  can answer questions about the program itself.
- *Intent* is where you record "I meant to do that" — a declared choice that
  tells the reading intelligences to stop flagging something they would
  otherwise report.

You do not need to touch most of these on day one. They are there so that when a
later feature needs a home — a bibliography, a cast list, a plot thread — it
already has one, in a known place, tagged the same in every project you will
ever open.

#section("Opening the project and making your first book")

You have a project on disk. Now open it. Running `inkhaven` with no arguments
opens the project in the *current* directory; `--project <path>` opens one
elsewhere.

#screen(caption: "Opening the editor")[```
$ cd ~/Books/my-first-project
$ inkhaven
# — or, from anywhere —
$ inkhaven --project ~/Books/my-first-project
```]

The full-screen editor comes up in a multi-pane layout — a search bar across the
top, the *Tree* on the left, the *Editor* in the middle, an *AI* pane on the
right, and a status line along the bottom. Chapter three walks the panes in
detail; here we only need the Tree, which has focus on startup, and where you
see the system books stacked and waiting.

The Tree is driven by single letters. To build the spine of a book you need just
three of them — one for each level — plus Enter to open a leaf for editing.

#chord_table((
  chord_row("B", "add a book at the root (above the system books)"),
  chord_row("C", "append a chapter under the selected book"),
  chord_row("A", "append a subchapter under the selected chapter"),
  chord_row("+", "append a paragraph under the selected branch"),
  chord_row("Enter", "open the selected paragraph in the Editor"),
  chord_row("F2", "rename the selected node"),
))

Each of `C`, `A`, and `+` has an *insert-after* twin — `V`, `S`, and `P`
respectively — that drops the new node immediately below the current row rather
than at the end of its parent. You will reach for those when a manuscript grows
and you need to slot something in the middle; for a first pass, append is all
you want.

Press `B`. A small dialog asks for the title; type one and press Enter, and your
book appears at the very top of the tree, above `Notes`:

#screen(caption: "Adding a book")[```
┌── Add book ─────────────────────────────────┐
│  Parent: <books root>                       │
│      Where: above the system block          │
│  Title : My First Book▏                     │
│                                             │
│  Enter to confirm · Esc to cancel           │
└─────────────────────────────────────────────┘
```]

With the cursor on your new book, press `C` to add a chapter beneath it; with
the cursor on the chapter, press `+` to add a paragraph. You may leave the
paragraph's title blank — Inkhaven will name it from your first sentence when you
save. The tree now shows your work sitting above the seeded books:

#screen(caption: "Your first structure")[```
▾ My First Book
  ▾ Chapter One
      ¶ Untitled paragraph
▸ Notes            [book · notes]
▸ Research         [book · research]
▸ Sources          [book · sources]
  …the other system books…
```]

Put the cursor on the paragraph row and press Enter. Focus moves to the Editor,
which shows the paragraph's starting template — a single Typst heading line:

#screen(caption: "A new paragraph in the Editor")[```
= Untitled paragraph

▏
```]

Press `End`, then Enter twice to leave the heading behind, and write. The border
around the Editor turns *yellow* the moment the buffer is dirty. Press `Ctrl+S`
to save: the `.typ` file is written under `books/`, the metadata is updated, and
a fresh embedding is computed for search — and the border returns to *green*.
Because you left the title empty, the Tree row now reads back your opening
sentence, and the file on disk has been renamed to match its new slug.

That is a complete round trip: a book, a chapter, a paragraph, saved to disk as
plain Typst, indexed for search, and ready to reopen exactly where you left it.
To leave, press `Ctrl+Q` — Inkhaven autosaves anything dirty and records the
session so your next launch reopens this very paragraph.

#section("Two ways to shape the tree first")

Before you pour in words, it is worth a minute's thought about *shape*, because
the tree you build is the outline you will write against — and a novelist and a
non-fiction author reach for it differently.

#two_track(
  [Start with the *spine of the story*, not the prose. Make one book, then a
  chapter per act or major movement, and let paragraphs be *scenes*. Lean on the
  world books early: drop your cast into *Characters* and your locations into
  *Places* as you invent them, so their names highlight in the prose and the
  continuity readers have something to watch. The `novel` template lays exactly
  this down if you would rather not start empty.],
  [Start from the *table of contents*. Make a book, a chapter per part or major
  section, and subchapters for the sections beneath — the tree *is* your
  outline, and it will become the document's structure verbatim. Put your
  gathered material and citations into *Research* and *Sources* from the first
  day, so claims have a home and a bibliography assembles itself. The
  `nonfiction` and `technical` templates scaffold this shape.],
)

Neither choice is binding. The tree reorders freely — rows move up and down,
paragraphs move between chapters, whole branches relocate — and every move
renumbers the files on disk for you. Build the shape you can see today; reshape
it the moment the book tells you to.

#recap((
  [`inkhaven init <dir>` creates a project directory; `--force` skips the
  overwrite prompt and `--template` scaffolds a starting tree, defaulting to
  *empty*. The first ever init downloads a shared embedding model to a per-user
  cache, not into the project.],
  [A project is *plain files*: `inkhaven.hjson` and `prompts.hjson` are yours to
  edit; `metadata.db`, `blobs.db`, `frequency.db`, and `vectors/` are the local
  store; `books/` is your prose. The files are the source of truth and the store
  is a rebuildable index — `inkhaven reindex` restores it from the tree.],
  [The hierarchy *Book → Chapter → Subchapter → Paragraph* is mirrored onto disk
  as folders and numbered `.typ` files; alongside prose paragraphs the tree can
  hold *image*, *hjson*, *jinja*, and *bund* leaves, plus structural-paragraph
  subtypes.],
  [Every project is seeded with *twenty protected system books* — reference
  (Notes, Research, Sources, Glossary, Snippets, Prompts, Help), world (Facts,
  Places, Characters, Artefacts, World, Threads, Planning, Mythology, Intent),
  and machinery (Typst, Scripts, Language, Submissions) — that you cannot delete
  and that your own books sit above.],
  [In the TUI the Tree is driven by single letters — `B` book, `C` chapter, `A`
  subchapter, `+` paragraph, Enter to edit, `Ctrl+S` to save — and the
  `.inkhaven/` directory plus the `.session.json` / `.inkhaven-backup.json` /
  `.inkhaven.log` dotfiles hold disposable session, backup, and cache state.],
))
