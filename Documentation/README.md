# Inkhaven Documentation

You are in the project's `Documentation/` directory. This is the canonical
home for everything beyond the source code: how to install Inkhaven, how
to configure it, how to write with it, how to keep its database healthy,
and how to extend its prompt / knowledge-base systems.

The README at the [repository root](../README.md) gives a one-page
overview and the rationale; this folder is where you go after that.

## Start here

If you have never run Inkhaven before, follow these in order:

1. [**FIRST_STEPS.md**](FIRST_STEPS.md) — install the Rust toolchain,
   build the binary, create your first project, and learn the bare
   minimum needed to write a paragraph.
2. [**Tutorials/**](Tutorials/) — narrative walk-throughs, each focused
   on one workflow. See [`Tutorials/README.md`](Tutorials/README.md) for
   a guided reading order.
3. [**KEYBINDING.md**](KEYBINDING.md) — keep open in another window;
   it is the reference for every keystroke in every pane.

## Reference manuals

These are detail-heavy lookup documents — read once for orientation, then
return whenever you need to remember a specific knob.

| Document | What it covers |
| -------- | -------------- |
| [`CONFIGURATION.md`](CONFIGURATION.md) | Every field in the project's `inkhaven.hjson`: embeddings, LLM providers, editor, theme, keys, hierarchy, backup, language, snowball stemmers. Includes valid value ranges and default values. |
| [`KEYBINDING.md`](KEYBINDING.md)       | Every keystroke the TUI honours, organised by pane (Tree / Editor / AI / Search / AI prompt) and overlay (file picker, prompt picker, modal stack). Mouse semantics included. |
| [`KEYS_REASSIGNMENT.md`](KEYS_REASSIGNMENT.md) | Rebind chords via `keys.bindings` in HJSON or via the `ink.key.*` Bund stdlib. Includes the full action table. |
| [`MAINTENANCE.md`](MAINTENANCE.md)     | Backup, restore, auto-backup-on-exit, the `reindex` command, log files, recovering from drift, troubleshooting first-run model downloads. |
| [`Bund/`](Bund/README.md)              | Bund — the embedded scripting language. Hook lambdas (`hook.on_save`, …), the `ink.*` stdlib, sandbox policy, `.bund` Script nodes. Start at [`Bund/BUND_TUTORIAL.md`](Bund/BUND_TUTORIAL.md). |

## Topic guides

These describe the parts of Inkhaven that have their own mental model.
Read the topic guide when you reach a workflow that depends on it; you
don't need them all up front.

| Document | What it covers |
| -------- | -------------- |
| [`PROMPTS.md`](PROMPTS.md)             | Writing reusable prompt templates: the `prompts.hjson` system library, the `Prompts` system book for project-local prompts, `{{selection}}` / `{{context}}` substitutions, and the picker UI. |
| [`LOCATIONS.md`](LOCATIONS.md)         | Managing the **Places** system book: how to record locations, how the editor highlights them in your prose, and how to ask the AI about a place via `Ctrl+B P`. |
| [`CHARACTERS.md`](CHARACTERS.md)       | Same model as `LOCATIONS.md` but for the **Characters** system book. Yellow-highlight overlay, `Ctrl+B C` RAG inference, multilingual stemming. |

## Tutorials

| Tutorial | Pattern |
| -------- | ------- |
| [`Tutorials/01-getting-started.md`](Tutorials/01-getting-started.md)             | From install to your first saved paragraph. |
| [`Tutorials/02-organising-your-manuscript.md`](Tutorials/02-organising-your-manuscript.md) | Books, chapters, subchapters, paragraphs — building the tree. |
| [`Tutorials/03-the-editor.md`](Tutorials/03-the-editor.md)                       | Movement, selection, find/replace, snapshots, split-edit. |
| [`Tutorials/04-search-and-discovery.md`](Tutorials/04-search-and-discovery.md)   | Semantic and full-text search; how multilingual embeddings find your prose. |
| [`Tutorials/05-ai-writing-assistant.md`](Tutorials/05-ai-writing-assistant.md)   | Scopes, inference modes, chat history, prompt picker. |
| [`Tutorials/06-grammar-check.md`](Tutorials/06-grammar-check.md)                 | F7 grammar workflow, `g`-apply, change highlights. |
| [`Tutorials/07-places-and-characters.md`](Tutorials/07-places-and-characters.md) | Tying worldbuilding to the editor with the Places / Characters books. |
| [`Tutorials/08-importing-existing-docs.md`](Tutorials/08-importing-existing-docs.md) | `inkhaven import-help`, the F3 file picker, and adopting a directory of `.md` / `.typ` files. |
| [`Tutorials/09-exporting-to-typst-and-pdf.md`](Tutorials/09-exporting-to-typst-and-pdf.md) | Concatenating the manuscript, running `typst compile`. |
| [`Tutorials/10-backups-and-recovery.md`](Tutorials/10-backups-and-recovery.md)   | Backup, restore, auto-backup, recovery from drift. |
| [`Tutorials/11-theming.md`](Tutorials/11-theming.md)                             | The dark theme defaults and every colour knob in the HJSON. |
| [`Tutorials/12-configuring-ai-providers.md`](Tutorials/12-configuring-ai-providers.md) | The six bundled provider stanzas + Ctrl+B L live switcher. |
| [`Tutorials/13-ai-full-screen-mode.md`](Tutorials/13-ai-full-screen-mode.md)     | Ctrl+B K layout, persistent chat history, Ctrl+F search, selection mode. |
| [`Tutorials/14-document-status.md`](Tutorials/14-document-status.md)             | Document-status workflow, Ctrl+B R cycle, Ctrl+B 1..7 filter. |

## Release notes

| Version | Notes |
| ------- | ----- |
| **1.2** | *(unreleased — branch `1.2`)* — bdslib + tree-sitter-typst absorbed in-tree (crates.io-publishable), Bund scripting (`ink.*` stdlib, 5 hook points, `.bund` Script nodes, Scripts system book), data-driven keymap with HJSON + Bund rebinding, `Ctrl+B M` cycle-type, `Ctrl+Z` Bund prefix. |
| **1.1** | [`RELEASE_NOTES/1.1.md`](RELEASE_NOTES/1.1.md) — first-class images + ratatui-image preview, eight-book seeding (Artefacts added), Book assembly / build / take pipeline, HJSON-driven `settings.typ`, six bundled LLM providers, full-screen typewriter + AI layouts, document-status workflow, HJSON data nodes, much more. |

## What lives on disk in a project

After `inkhaven init <root>`:

```
my-novel/
├── inkhaven.hjson           HJSON config (see CONFIGURATION.md)
├── prompts.hjson            Prompt library (see PROMPTS.md)
├── .session.json            TUI session state — cursor, open paragraph, focus
├── .inkhaven-backup.json    Timestamp of the last successful backup
├── .inkhaven-chat.json      Persistent AI chat history (full-screen mode)
├── .inkhaven.log            Rotating log file (writes during the TUI session)
├── metadata.db              DuckDB — hierarchy node metadata as JSON
├── blobs.db                 DuckDB BLOB store — paragraph / image / script bodies
├── vectors/                 HNSW vector index (multilingual embeddings)
└── books/
    ├── my-novel/                       (user book — your manuscript)
    │   ├── 01-preface.typ              (Paragraph, content_type = typst)
    │   ├── 02-the-beginning/
    │   │   ├── 01-morning-light/
    │   │   │   ├── 01-opening-scene.typ
    │   │   │   └── 02-the-storm-breaks.typ
    │   │   └── 02-chapter-intro.typ
    │   └── 03-data-notes.hjson         (Paragraph, content_type = hjson)
    └── scripts/                        (Scripts system book)
        └── 01-on-save-warn.bund        (Script node — Bund source)
```

Three leaf kinds live under `books/<...>/`:

- **`.typ`** — Paragraph (default). Typst source; the canonical
  form of your prose. Versionable with git, renderable with
  `typst compile`.
- **`.hjson`** — Paragraph with `content_type=hjson`. Structured
  data nodes (worldbuilding tables, prop catalogs, etc.).
- **`.bund`** — Script node. Bund source evaluated into the Adam
  VM at project open. See [`Bund/`](Bund/README.md).

`inkhaven reindex` reconciles disk against the DuckDB blob store
when something drifts (see [`MAINTENANCE.md`](MAINTENANCE.md)).
Images are stored binary in `blobs.db` and shipped to disk as
`.png` / `.jpg` / `.webp` working copies on assembly.

## The hierarchy

```
Book → Chapter → Subchapter → Paragraph
```

Exactly four levels by default. `Paragraph` is the leaf (a `.typ` file on
disk); the other three are directories. Paragraphs can attach to **any**
branch level — that is how prefaces, chapter intros, and afterwords are
represented (paragraphs attached directly to the book or chapter rather
than buried in a subchapter).

To allow arbitrary subchapter nesting, set in `inkhaven.hjson`:

```hjson
hierarchy: { unbounded_subchapters: true }
```

Each node has a stable UUIDv7, a slug, an `order` integer (which controls
both display order in the Tree pane and the on-disk `NN-` filename
prefix), and a parent pointer. Names like `01-preface.typ` and
`02-the-beginning/` sort correctly under `ls`.

### Nine system books

Every project ships with nine pre-seeded books at the top of the tree,
in this order:

| Book         | Purpose                                                   | Special behaviour |
| ------------ | --------------------------------------------------------- | ----------------- |
| **Notes**       | Editorial notes, TODOs, marginalia                     | Word-matches in prose underlined; `Ctrl+B G` queries this book |
| **Research**    | Background research kept alongside the manuscript      | Plain user book |
| **Prompts**     | Project-local AI prompt templates                      | Surfaced in the `/` picker (see [PROMPTS.md](PROMPTS.md)) |
| **Places**      | Locations referenced by the prose                      | Names light up in cyan; `Ctrl+B P` queries this book (outside `#image(...)`) ([LOCATIONS.md](LOCATIONS.md)) |
| **Characters**  | Characters referenced by the prose                     | Names light up in yellow; `Ctrl+B C` queries this book ([CHARACTERS.md](CHARACTERS.md)) |
| **Artefacts**   | Objects, items, worldbuilding props                    | Names light up in peach; `Ctrl+B Y` queries this book (added in 1.1) |
| **Typst**       | Per-user-book Typst skeleton (globals / settings / index) | Read/write; auto-seeded for every new user book (added in 1.1) |
| **Scripts**     | Bund scripts (`.bund`) loaded into the Adam VM at startup | Default home for project-global hooks / chord rebinds; `Ctrl+Z N` creates a new script (added in 1.2) — see [`Bund/`](Bund/README.md) |
| **Help**        | Inkhaven's own help manual, queryable from F1          | Read-only; populated via `inkhaven import-help` or `inkhaven import-typst-help` |

User-added books are inserted **above** Notes — the system block stays
pinned at the bottom of the root level so your own work always sits on top.

## Where to get help

- Read [`KEYBINDING.md`](KEYBINDING.md) and press `Ctrl+B H` inside the
  TUI for the pane-aware quick reference.
- Press `F1` inside the TUI to ask the bundled Help book a question.
- File issues, ideas, or PRs at
  [github.com/vulogov/blackInkhaven](https://github.com/vulogov/blackInkhaven).
