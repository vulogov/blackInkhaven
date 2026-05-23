# 2 — Installation and your first book

There are three ways to install the binary, in increasing order of "I know what I'm doing":

```
# 1. Pre-built binary via cargo-binstall (fastest)
cargo binstall inkhaven

# 2. Build from source (gets the latest commit)
cargo install --git https://github.com/vulogov/blackInkhaven

# 3. Direct download from the GitHub releases page
#    https://github.com/vulogov/blackInkhaven/releases
```

All three give you a single executable named `inkhaven`. Drop it on your `PATH` (e.g. `~/.cargo/bin/` is on `PATH` after `cargo install`).

> **Linux build dependency:** Source builds on Linux need `libasound2-dev` at compile time (for the typewriter audio feedback). The prebuilt binaries don't need it at runtime.

## Initialise a project

A project is a directory. You initialise one with:

```
inkhaven init ~/Documents/MyBook
```

This creates the directory if it doesn't exist, writes a default `inkhaven.hjson` (the project's config), seeds the DuckDB metadata + vector indices, and pre-creates the seven system books (Notes, Research, Prompts, Places, Characters, Help, Typst). You'll meet the system books in Chapter 4.

![figure: init-output](images/init-output.png) — Running `inkhaven init` shows the layout it just created: config path, books directory, metadata DB, vector store.

## Open the TUI

```
cd ~/Documents/MyBook
inkhaven
```

The four-pane layout paints. Tree is empty (no user books yet, just the system ones).

## Add your first book

From the CLI, before going into the TUI:

```
inkhaven add book "MyFirstBook"
```

This creates a top-level `Book` node in the tree with slug `my-first-book`. Subsequent `add` calls hang chapters and paragraphs underneath:

```
inkhaven add chapter "Chapter 1" --parent my-first-book
inkhaven add paragraph "Opening scene" \
  --parent my-first-book/chapter-1
```

You can also do all of this from inside the TUI; the CLI version is just easier to script.

![figure: first-book-tree](images/first-book-tree.png) — Tree pane after the three add commands: book → chapter → paragraph.

## Open + write a paragraph

Re-launch `inkhaven`. In the Tree pane navigate (arrow keys) to "Opening scene" and press `Enter`. The Editor pane populates with the paragraph's typst skeleton:

```typst
= Opening scene

```

Type a sentence. Press `Ctrl+S`. Status line reports "wrote N bytes". That's the loop — open a paragraph, write, save, move to the next.

> **One paragraph at a time:** Inkhaven holds exactly one paragraph open in the editor at any moment. Switching paragraphs autosaves the previous one. This is intentional: the unit of writing is a paragraph, and the database stores them individually so you can attach status, tags, snapshots, and AI memory at paragraph granularity.

## Build a PDF

`Ctrl+B B` ("build the book") tells Inkhaven to assemble every paragraph in the current user book into one Typst document and compile it to PDF. The PDF lands next to your project in `inkhaven-artefacts/<book-slug>/<book-slug>.pdf`.

![figure: ctrl-b-b-splash](images/ctrl-b-b-splash.png) — Building the book: splash + spinner while typst compiles. Cancellable with Esc.

## Quit

`Ctrl+Q`. If the open paragraph is dirty, it autosaves before exit. Inkhaven also writes a backup automatically — see Chapter 11.

## Recap

- Three install paths; `cargo binstall inkhaven` is the fastest.
- Project = directory + `inkhaven.hjson` + DB + books.
- `inkhaven init <path>` creates everything; `inkhaven add` populates the tree.
- One paragraph open at a time; switching autosaves.
- `Ctrl+B B` builds the book; PDF lands in `inkhaven-artefacts/`.
- `Ctrl+Q` quits; dirty paragraph autosaves.
