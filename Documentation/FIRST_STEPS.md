# First steps

This document takes you from nothing installed to a saved paragraph in your
own project. It does not assume prior experience with Rust, the terminal, or
Typst — but it does assume you can open a terminal window and run commands
in it.

## Table of contents

1. [What you will install](#what-you-will-install)
2. [Install the Rust toolchain](#install-the-rust-toolchain)
3. [Get the Inkhaven source](#get-the-inkhaven-source)
4. [Build the binary](#build-the-binary)
5. [Put `inkhaven` on your PATH](#put-inkhaven-on-your-path)
6. [Optional: install Typst (for PDF export)](#optional-install-typst-for-pdf-export)
7. [Initialise your first project](#initialise-your-first-project)
8. [What the database holds](#what-the-database-holds)
9. [Launch the TUI](#launch-the-tui)
10. [Write your first paragraph](#write-your-first-paragraph)
11. [Set up an AI provider (optional)](#set-up-an-ai-provider-optional)
12. [What to read next](#what-to-read-next)

## What you will install

Three things, in order:

1. **Rust toolchain** (`rustc` + `cargo`) — Inkhaven is written in Rust
   and you compile it yourself.
2. **Inkhaven** — clone the repository and run `cargo build --release`.
3. **Optional: Typst CLI** — only needed if you plan to render your
   manuscript to PDF from inside Inkhaven.

You do **not** need Node, Python, Docker, Postgres, a paid AI account, or
an IDE. Inkhaven is a single binary that owns its own database and
embeddings cache.

Disk footprint of a fresh install:

| Thing | Size |
| ----- | ---- |
| Rust toolchain | ~600 MB (one-time, used for any Rust project) |
| Inkhaven release binary | ~90 MB |
| First-run embedding model | ~120 MB (downloaded to a per-user cache the first time you open a project) |
| A new empty project | ~5 MB (DuckDB + HNSW scaffolding) |

## Install the Rust toolchain

The official installer is **rustup** — a small script that puts the
compiler and `cargo` in your home directory and adds them to your shell
PATH. It does not need sudo.

### macOS and Linux

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Follow the on-screen prompt; the default selection (option 1) is correct
for our purposes. When it finishes, open a new terminal (or run
`source "$HOME/.cargo/env"` in the current one) so `cargo` is on PATH.

### Windows

Download and run `rustup-init.exe` from
[https://rustup.rs/](https://rustup.rs/). Pick the default toolchain.

### Verify

```bash
rustc --version
cargo --version
```

Both should print a version number. If you see "command not found", your
shell hasn't picked up the cargo bin directory yet — close and reopen the
terminal, or check that `$HOME/.cargo/bin` is on your `PATH`.

## Get the Inkhaven source

Inkhaven lives at <https://github.com/vulogov/blackInkhaven>.

```bash
git clone https://github.com/vulogov/blackInkhaven.git
cd blackInkhaven
```

If you don't have git, download the source as a zip from the GitHub page
and `cd` into the unpacked directory.

## Build the binary

```bash
cargo build --release
```

The first build downloads about a hundred crates and takes 3 – 8 minutes
on a modern laptop. Subsequent builds are incremental and reuse the cache.

When it finishes you have a binary at:

```
./target/release/inkhaven
```

You can run it directly from there for the rest of this document.

### Why the `--release` flag

Without `--release`, cargo builds with debug symbols and no optimisation
— the binary still works but is several times slower and a few hundred
megabytes bigger. Use `--release` for day-to-day work.

## Put `inkhaven` on your PATH

So you don't have to type `./target/release/inkhaven` every time:

```bash
# macOS / Linux
sudo install target/release/inkhaven /usr/local/bin/inkhaven
# or, no sudo:
cp target/release/inkhaven ~/.local/bin/   # if ~/.local/bin is on $PATH
```

From this point on, the rest of this guide uses just `inkhaven`.

## Optional: install Typst (for PDF export)

If you want `inkhaven export pdf` to actually build a PDF, install Typst
itself. It is a single static binary on every platform — see
[https://github.com/typst/typst#installation](https://github.com/typst/typst#installation).

Inkhaven works fine without Typst — you just won't have the PDF export
path. `inkhaven export typst` (the one that emits a single combined `.typ`
file) works regardless.

## Initialise your first project

Pick a directory for the project. It does not need to exist; Inkhaven
will create it. The convention is `~/Books/<project-name>`.

```bash
inkhaven init ~/Books/my-first-project
```

If the directory **already exists**, Inkhaven asks for confirmation
before wiping it:

```
Directory `/home/you/Books/my-first-project` already exists.
Remove it and re-initialise? [y/N]
```

Type `y` to proceed; anything else aborts with no changes.

You can pass `--force` to skip the prompt for scripted setups
(`inkhaven init ~/x --force`).

### First-run model download

On the very first project init Inkhaven downloads a multilingual embedding
model (~120 MB) into a per-user cache:

| OS      | Cache location |
| ------- | -------------- |
| macOS   | `~/Library/Caches/dev.inkhaven.inkhaven/embeddings/` |
| Linux   | `$XDG_CACHE_HOME/inkhaven/embeddings/` (defaults to `~/.cache/inkhaven/`) |
| Windows | `%LOCALAPPDATA%\inkhaven\inkhaven\cache\embeddings\` |

The download is one-time per model. Subsequent inits reuse it.

You will see "Initialized inkhaven project at …" and a summary of the
files that were created:

```
Initialized inkhaven project at /home/you/Books/my-first-project
  config:    /home/you/Books/my-first-project/inkhaven.hjson
  prompts:   /home/you/Books/my-first-project/prompts.hjson
  store db:  /home/you/Books/my-first-project/metadata.db
  vecstore:  /home/you/Books/my-first-project/vectors
  books:     /home/you/Books/my-first-project/books
```

## What the database holds

`cd ~/Books/my-first-project && ls -la` shows what Inkhaven just laid
down. You don't have to memorise this — the gist:

- `inkhaven.hjson` — your configuration file. Edit it to change the
  theme, the language, the AI provider, autosave cadence, etc. See
  [CONFIGURATION.md](CONFIGURATION.md) for every field.
- `prompts.hjson` — your prompt library. See [PROMPTS.md](PROMPTS.md).
- `metadata.db`, `blobs.db`, `frequency.db`, `vectors/` — the bdslib
  database. Hierarchy metadata, paragraph bodies, full-text index, and
  HNSW vector index. You won't edit these by hand.
- `books/` — your prose. Each paragraph is a real `.typ` file you can
  open in any other editor.

`inkhaven list` prints the hierarchy. Right after `init` you will see six
**system books** that ship with every project:

```
├─ Notes  [book, notes]
├─ Research  [book, research]
├─ Prompts  [book, prompts]
├─ Places  [book, places]
├─ Characters  [book, characters]
└─ Help  [book, help]
```

These are seeded automatically — you cannot delete or rename them.
Anything new you create lives **above** Notes.

## Launch the TUI

```bash
inkhaven --project ~/Books/my-first-project
```

(`--project` defaults to the current directory, so `cd` first and just
running `inkhaven` works too.)

You should see a five-pane layout:

```
┌── Search ─────────────────────────────────────────────────┐
│                                                           │
├── Tree ─────────┬── Editor ──────────────────┬── AI ─────┤
│ ▾ Notes        │ (no paragraph open —        │ (focus AI │
│ ▾ Research     │  select one in the Tree     │  prompt   │
│ ▾ Prompts      │  pane and press Enter)      │  with     │
│ ▾ Places       │                             │  Ctrl+I)  │
│ ▾ Characters   │                             │           │
│ ▾ Help         │                             │           │
├────────────────┴─────────────────────────────┴───────────┤
│ AI prompt                                                │
├──────────────────────────────────────────────────────────┤
│ Tab=panes · Enter=open · Ctrl+S=save …                   │
└──────────────────────────────────────────────────────────┘
```

If the panes look cramped, just resize the terminal window — the layout
is responsive.

To **quit**, press `Ctrl+Q`. Inkhaven autosaves a dirty paragraph before
exit; it also saves session state (which paragraph was open, where the
cursor sat) so you resume in the same place next time.

## Write your first paragraph

The keyboard is the primary interface. Mouse works too (click to focus,
scroll wheel to scroll) but every action has a key chord.

### Add a book

The Tree pane has focus on startup. Press **`B`** to add a book. A small
dialog appears asking for the title:

```
┌── Add book ─────────────────────────────────────┐
│  Parent: <books root>                           │
│      Where: above the system block              │
│  Title : My First Book│                         │
│                                                 │
│  Enter to confirm · Esc to cancel               │
└─────────────────────────────────────────────────┘
```

Type a title and press Enter. Your book appears at the top of the tree,
above Notes.

### Add a chapter and a paragraph

With the cursor on your book row:

- Press **`C`** to add a chapter underneath. Title → Enter.
- With the cursor on your chapter row, press **`+`** to add a paragraph.
  Leaving the title field empty is fine — Inkhaven will auto-derive a
  title from the first sentence on first save.

The tree now looks like:

```
├─ My First Book
│  ├─ Chapter One
│  │  └─ ¶ Untitled paragraph
│  ├─ Notes  [book, notes]
│  ...
```

### Edit the paragraph

With the cursor on the paragraph row, press **Enter**. Focus moves to the
Editor pane and you see the paragraph's `.typ` template:

```
= Untitled paragraph
```

The `=` is the Typst marker for a level-1 heading — Inkhaven inserts it
automatically for new paragraphs so each one renders as a section in the
final manuscript.

Press `End` to land at the end of the line, then `Enter Enter` to leave a
blank line, and start typing prose:

```
= Untitled paragraph

The thunderstruck mariner stood at the rail. Rain had been falling for
three days, and the deck was slick.
```

The border around the editor turns **yellow** when the buffer has
unsaved changes; **green** after a save.

### Save

Press **`Ctrl+S`**. Three things happen:

1. The `.typ` file is written to disk under `books/`.
2. The paragraph's metadata is updated (word count, modified time) in
   `metadata.db`.
3. A fresh embedding is computed from the new content for semantic
   search.

The status bar reports something like:

```
[Editor] saved books/my-first-book/01-chapter-one/01-…  (14 words, re-embedded)
```

Since you left the paragraph title empty, it has now been replaced with
the first sentence of your prose. Look at the Tree pane — the row title
changed from "Untitled paragraph" to "The thunderstruck mariner stood at
the rail.".

### Quit

`Ctrl+Q`. Session is saved; next time you launch the TUI for this project
the same paragraph re-opens with the cursor where you left it.

## Set up an AI provider (optional)

You can write entire manuscripts without ever using the AI pane. If you
want streaming suggestions, grammar checks, or RAG inference against your
own prose, you need at least one provider.

The shipped `inkhaven.hjson` knows about three providers out of the box:

| Provider  | API key env var      | Notes |
| --------- | -------------------- | ----- |
| `gemini`   | `GEMINI_API_KEY`    | Google's Gemini Pro family |
| `deepseek` | `DEEPSEEK_API_KEY`  | DeepSeek's chat model |
| `ollama`   | — (none)             | Local model via [Ollama](https://ollama.com); no API key needed |

Pick one, set the env var, and you are done:

```bash
export GEMINI_API_KEY='your-key-here'
```

Then `Ctrl+I` inside the TUI focuses the AI prompt bar; type a sentence
and press Enter to stream a response.

To use Ollama, install it ([https://ollama.com](https://ollama.com)),
pull a model (`ollama pull llama3.2`), set `default: ollama` in
`inkhaven.hjson`, and you have a fully local AI workflow.

See [`CONFIGURATION.md`](CONFIGURATION.md#llm) for adding more providers
or changing the default.

## What to read next

- [`Tutorials/01-getting-started.md`](Tutorials/01-getting-started.md) —
  the same first-paragraph walk-through with more screenshots and key
  hints.
- [`KEYBINDING.md`](KEYBINDING.md) — bookmark this. Press `Ctrl+B H`
  inside the TUI for the same content as a floating overlay.
- [`Tutorials/02-organising-your-manuscript.md`](Tutorials/02-organising-your-manuscript.md)
  — folding, reordering, system books, importing existing files.
- [`Tutorials/05-ai-writing-assistant.md`](Tutorials/05-ai-writing-assistant.md)
  — when and how to use scope (F9) and inference modes (F10).

If something does not work, check [`MAINTENANCE.md`](MAINTENANCE.md) for
troubleshooting (first-run model download, log files, recovering from
drift).
