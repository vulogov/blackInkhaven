# 1 — Getting started

Goal: a working Inkhaven project with one book, one chapter, one
paragraph, and one save.

If you haven't installed Inkhaven yet, follow
[`../FIRST_STEPS.md`](../FIRST_STEPS.md) first; this tutorial picks up
from a working `inkhaven` binary on your PATH.

## 1. Pick a project location

Inkhaven projects are ordinary directories. Anywhere your shell can
write is fine — `~/Books/<project>`, `~/Documents/manuscripts/<project>`,
or a fresh checkout under `~/git/`. We'll use `~/Books/sample-novel`.

```bash
$ inkhaven init ~/Books/sample-novel
```

If the directory does not exist, Inkhaven creates it and seeds the
required files. If it **does** exist, Inkhaven asks before touching
anything:

```
Directory `/home/you/Books/sample-novel` already exists.
Remove it and re-initialise? [y/N]
```

Answer `n` to abort, `y` to wipe and re-seed. For automated setups
(e.g. CI), `--force` skips the prompt:

```bash
$ inkhaven init ~/Books/sample-novel --force
```

On a fresh install Inkhaven downloads the multilingual embedding model
(~120 MB) into a user-level cache the first time. Subsequent `init`s
reuse it.

Output looks like:

```
Initialized inkhaven project at /home/you/Books/sample-novel
  config:    /home/you/Books/sample-novel/inkhaven.hjson
  prompts:   /home/you/Books/sample-novel/prompts.hjson
  store db:  /home/you/Books/sample-novel/metadata.db
  vecstore:  /home/you/Books/sample-novel/vectors
  books:     /home/you/Books/sample-novel/books
```

## 2. Look at what was created

```bash
$ ls ~/Books/sample-novel
backup        frequency.db   inkhaven.hjson   prompts.hjson
blobs.db      metadata.db    vectors/         books/
```

Three groups of files:

- **Database** (`metadata.db`, `blobs.db`, `frequency.db`, `vectors/`)
  — bdslib's DuckDB tables and HNSW vector index. Don't edit.
- **Configuration** (`inkhaven.hjson`, `prompts.hjson`) — text files
  you may edit later; see [`../CONFIGURATION.md`](../CONFIGURATION.md)
  and [`../PROMPTS.md`](../PROMPTS.md).
- **Prose** (`books/`) — empty for now; this is where your `.typ`
  paragraph files land. Each book gets a subdirectory, each chapter a
  sub-subdirectory, and so on.

Six system books are already registered in the database:

```bash
$ inkhaven --project ~/Books/sample-novel list
├─ Notes  [book, notes]
├─ Research  [book, research]
├─ Prompts  [book, prompts]
├─ Places  [book, places]
├─ Characters  [book, characters]
└─ Help  [book, help]
```

You cannot delete these — they back various Inkhaven features (the F1
help-manual, the Places overlay, etc.).

## 3. Launch the TUI

```bash
$ inkhaven --project ~/Books/sample-novel
```

Or `cd ~/Books/sample-novel && inkhaven` — the project defaults to
the current directory.

Your terminal switches to a full-screen layout:

```
┌── Search ─────────────────────────────────────────────────┐
│                                                           │
├── Tree ─────────┬── Editor ─────────────────┬── AI ──────┤
│ ▾ Notes        │ (no paragraph open —       │ (focus AI  │
│ ▾ Research     │  select one in the Tree    │  prompt    │
│ ▾ Prompts      │  pane and press Enter)     │  with      │
│ ▾ Places       │                            │  Ctrl+I)   │
│ ▾ Characters   │                            │            │
│ ▾ Help         │                            │            │
├────────────────┴────────────────────────────┴────────────┤
│ AI prompt                                                │
├──────────────────────────────────────────────────────────┤
│ Tab=panes · Enter=open · Ctrl+S=save · …                 │
└──────────────────────────────────────────────────────────┘
```

Five panes:

- **Search** (top) — full-text and semantic search across the project.
- **Tree** (left) — hierarchy navigator. Has focus on startup.
- **Editor** (middle) — your current paragraph.
- **AI** (right) — streaming inference results.
- **AI prompt** (bottom) — type a question for the LLM.

The yellow / pink border indicates which pane has focus.

## 4. Add a book

The Tree pane has focus. Single-letter chords add hierarchy items.
Press `B`:

```
┌── Add book ─────────────────────────────────────┐
│  Parent: <books root>                           │
│      Where: above the system block              │
│  Title : │                                      │
│                                                 │
│  Enter to confirm · Esc to cancel               │
└─────────────────────────────────────────────────┘
```

Type `Sample Novel` and press Enter. The book appears at the top of
the tree, above Notes:

```
├─ Sample Novel
├─ Notes
├─ Research
…
```

The on-disk side: `books/sample-novel/` now exists.

## 5. Add a chapter

With the Tree cursor on the `Sample Novel` row, press `C`:

```
┌── Add chapter ──────────────────────────────────┐
│  Parent: sample-novel                           │
│      Where: append at end                       │
│  Title : │                                      │
│  …                                              │
└─────────────────────────────────────────────────┘
```

Type `Chapter One` and Enter:

```
├─ Sample Novel
│  └─ Chapter One
├─ Notes
…
```

`books/sample-novel/01-chapter-one/` now exists on disk.

## 6. Add a paragraph

With the Tree cursor on the `Chapter One` row, press `+`. Leave the
title empty (Inkhaven will derive one from the first sentence on
first save) and press Enter:

```
├─ Sample Novel
│  └─ Chapter One
│     └─ ¶ Untitled paragraph
…
```

The new paragraph row opens; press **Enter** on it. Focus moves to
the Editor pane and you see:

```
1 = Untitled paragraph
2
```

The `= Untitled paragraph` is Typst's level-1 heading syntax — Inkhaven
inserts it on `add paragraph` so each paragraph renders as a section
in the eventual PDF.

## 7. Write a sentence

Position the cursor at the end of line 2 (`End` key) and type:

```
The thunderstruck mariner stood at the rail. Rain had been falling
for three days, and the deck was slick.
```

The editor border turns **yellow** the moment you type — that means
the buffer has unsaved changes.

## 8. Save

Press `Ctrl+S`. Three things happen:

1. The `.typ` file is written to
   `books/sample-novel/01-chapter-one/01-untitled-paragraph.typ`.
2. The paragraph's metadata is updated in `metadata.db`.
3. A fresh embedding is computed.

The status bar reports:

```
[Editor] saved books/sample-novel/01-chapter-one/01-untitled-paragraph.typ
         (16 words, re-embedded) · named `The thunderstruck mariner stood at
         the rail.` from first sentence
```

Look at the Tree pane: the paragraph row's title changed from
`Untitled paragraph` to `The thunderstruck mariner stood at the rail.`.
Inkhaven detected the first sentence on save and used it as the
display title.

The editor border is now **green** — saved.

## 9. Open a different paragraph (or come back)

`Tab` moves focus back to the Tree. Press `Enter` on a row to load
that paragraph into the editor. Try opening the paragraph you just
saved (it has the same title now).

If you make edits and try to leave the editor without saving, Inkhaven
**autosaves silently** before swapping — no data loss. You can also
turn off idle autosave by setting `editor.autosave_seconds: 0` in
`inkhaven.hjson` (see [`../CONFIGURATION.md`](../CONFIGURATION.md)).

## 10. Quit

Press `Ctrl+Q`. The TUI exits. Session state is written to
`.session.json`, so next time you launch:

```bash
$ inkhaven --project ~/Books/sample-novel
```

the same paragraph reopens with the cursor where you left it.

## 11. What you have learned

- `inkhaven init <path>` creates a project, with confirmation if the
  path exists.
- Six system books are seeded; user books sit above them.
- In the Tree pane, **B** adds a book, **C** a chapter, **A** a
  subchapter, **+** a paragraph.
- **Enter** on a paragraph row opens it in the Editor pane.
- **Ctrl+S** saves; the border colour tells you the state.
- Paragraph titles auto-fill from the first sentence on first save.
- **Ctrl+Q** quits cleanly with session persistence.

## Next steps

- [`02-organising-your-manuscript.md`](02-organising-your-manuscript.md)
  expands the tree workflow: V/S/P insert-after, U/J reordering, Z/X
  folding, F2 rename.
- [`03-the-editor.md`](03-the-editor.md) covers everything in the
  editor pane: navigation, selection, find/replace, snapshots,
  split-edit.
