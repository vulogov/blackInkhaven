# Inkhaven

A standalone Rust binary that pairs a full-screen terminal text editor with a
semantic index, an AI writing assistant, and a Typst toolchain — all so you
can write books as a hierarchy of `.typ` files without leaving the terminal.

Backed by [bdslib's `DocumentStorage`](https://github.com/vulogov/bdslib)
(DuckDB + Tantivy + fastembed + HNSW), [ratatui](https://ratatui.rs/) for the
TUI, [tui-textarea](https://github.com/rhysd/tui-textarea) for editing,
[tree-sitter-typst](https://github.com/uben0/tree-sitter-typst) for
highlighting, and [genai](https://github.com/jeremychone/rust-genai) for
provider-neutral LLM streaming.

## Documentation in this folder

- [README.md](README.md) — this file
- [KEYBINDING.md](KEYBINDING.md) — every keystroke the TUI recognizes,
  organized by pane and overlay
- `Tutorials/` — (reserved) end-user tutorials

## Quick start

```bash
# Build
cargo build --release

# Initialize a project — creates inkhaven.hjson, prompts.hjson, books/,
# metadata.db, blobs.db, vectors/. The first run downloads the multilingual
# embedding model (~120 MB) into ~/Library/Caches/dev.inkhaven.inkhaven/
# (macOS) or $XDG_CACHE_HOME/inkhaven/ (Linux).
./target/release/inkhaven init ~/Books/my-novel

# Build the hierarchy from the CLI (or skip — you can do this in the TUI too)
./target/release/inkhaven --project ~/Books/my-novel add book "My Novel"
./target/release/inkhaven --project ~/Books/my-novel \
    add chapter "The Beginning" --parent my-novel
./target/release/inkhaven --project ~/Books/my-novel \
    add paragraph "Opening Scene" --parent my-novel/the-beginning

# Launch the TUI
./target/release/inkhaven --project ~/Books/my-novel tui
```

## What lives on disk

After `inkhaven init <root>`:

```
my-novel/
├── inkhaven.hjson      HJSON config: embedding model, LLM providers, keys, etc.
├── prompts.hjson       Prompt library for the AI (/ picker in the TUI).
├── .session.json       TUI session state (cursor, opened paragraph, etc.) — auto-saved on quit
├── metadata.db         bdslib DuckDB store: hierarchy node metadata as JSON.
├── blobs.db            bdslib BLOB store: paragraph bodies.
├── frequency.db        bdslib auxiliary store.
├── vectors/            bdslib HNSW vector index (multilingual embeddings).
└── books/
    └── my-novel/
        ├── 01-preface.typ
        ├── 02-the-beginning/
        │   ├── 01-morning-light/
        │   │   ├── 01-opening-scene.typ
        │   │   └── 02-the-storm-breaks.typ
        │   └── 02-chapter-intro.typ
        └── 03-finale.typ
```

The `.typ` files are the canonical source of your prose — version them with
git, render them with `typst compile`, edit them in any other tool. bdslib's
database mirrors them for hierarchy queries and semantic search; if it ever
drifts, `inkhaven reindex` reconciles.

## The hierarchy

```
Book → Chapter → Subchapter → Paragraph
```

By default, exactly four levels. `Paragraph` is the leaf (a `.typ` file
on disk); the other three are directories. Paragraphs can attach to *any*
branch level, not just under a `Subchapter` — that's how prefaces,
chapter intros, and afterwords are represented (just paragraphs attached
to the book or chapter directly).

To allow arbitrary subchapter nesting, set in `inkhaven.hjson`:

```hjson
hierarchy: { unbounded_subchapters: true }
```

Each node has a stable UUIDv7, a slug, an `order` integer (which controls
both display order and the on-disk `NN-` filename prefix), and a parent
pointer. Names like `01-preface.typ` and `02-the-beginning/` sort
correctly in `ls`.

## Inkhaven uses two sources of truth, kept in sync

| What                | Source of truth | How it stays in sync                              |
| ------------------- | --------------- | ------------------------------------------------- |
| Prose bytes         | `.typ` files    | TUI Ctrl+S writes file → bdslib `update_content` + `reembed_document`. CLI `reindex` walks disk and resyncs. |
| Hierarchy structure | bdslib          | Every `add`/`delete`/`mv` updates bdslib + renames the corresponding filesystem entry atomically. |
| Vector embeddings   | bdslib HNSW     | Computed from `.typ` content via multilingual fastembed on each save / reindex. |

The TUI never holds an exclusive lock on the database — if you edit a `.typ`
in another tool, run `inkhaven reindex` and the new bytes show up in search.

## CLI reference

Every command takes an optional `--project <path>` (defaults to current
directory). `init` is the only one that doesn't need an existing project.

| Command | What it does |
| ------- | ------------ |
| `inkhaven init <path>` | Create a fresh project at `<path>`. |
| `inkhaven add <kind> <title> [--parent slash/path] [--slug …]` | Add a book / chapter / subchapter / paragraph. |
| `inkhaven mv <slash/path> up\|down` | Swap with previous / next sibling. |
| `inkhaven delete <slash/path> --yes` | Delete a node and its subtree. Without `--yes` shows the descendant count. |
| `inkhaven list` | Print the hierarchy as a tree. |
| `inkhaven search "<query>" [--limit N]` | Semantic search across the project. Multilingual (Russian, English, …). |
| `inkhaven reindex [--prune] [--adopt]` | Reconcile disk with bdslib. `--prune` removes records for missing files; `--adopt` registers orphan `.typ` files under their parent directory's branch. |
| `inkhaven ai "<prompt>" [--provider name]` | One-shot AI streaming to stdout. Honors `LLM provider` env vars (see below). |
| `inkhaven export typst [-o file.typ]` | Concatenate every paragraph in DFS order. With `-o` writes a file; without, prints to stdout. |
| `inkhaven export pdf -o file.pdf` | Build the combined `.typ` and shell out to `typst compile`. The intermediate `.typ` is kept next to the PDF for inspection. Requires `typst` on PATH. |
| `inkhaven tui` | Launch the full-screen editor. Default if no subcommand is given. |

All slash-paths (e.g. `--parent my-novel/the-beginning`) are
slug-paths under the `books/` root.

## The TUI

```
┌─ Search ────────────────────────────────────────────────────────┐
│                                                                 │
├──────────┬──────────────────────────────┬───────────────────────┤
│ Tree     │ Editor (Typst-highlighted)   │ AI                    │
│          │                              │                       │
│ ▾ Book   │  = Opening                   │ > Tighten this        │
│  ▾ Ch.1  │                              │   paragraph…          │
│   ▾ S.1  │  The thunderstruck mariner…  │                       │
│    • P.1 │                              │ [streamed result]     │
│    • P.2 │                              │                       │
│  ▾ Ch.2  │                              │ r=replace i=insert    │
│   …      │                              │ t=top b=bottom c=copy │
├──────────┴──────────────────────────────┴───────────────────────┤
│ ai > /tighten                                                   │
├─────────────────────────────────────────────────────────────────┤
│ [Editor] saved books/.../01-opening.typ (84 words, re-embedded) │
└─────────────────────────────────────────────────────────────────┘
```

Five focus states cycle with `Tab`/`Shift+Tab`: **Tree** (hierarchy
navigator), **Editor** (open paragraph), **AI** (inference results),
**Search bar** (top), **AI prompt** (bottom). Three transient overlays float
on top: search results (yellow), prompt picker (magenta, `/` in AI prompt),
and modal dialogs for add/delete confirmation.

The Editor pane shows **line numbers** in a dim gutter on the left and a
**current-line highlight** behind the row containing the cursor. Both
adapt to wrap mode (line number on the first visual row of each source
line; continuation rows leave the gutter blank).

**Paragraph titles** can be added with the title field left empty in the
Add modal. The paragraph gets a placeholder name (`Untitled paragraph`)
until the first save, at which point the title is replaced with the first
sentence of the body — detected by `.`, `!`, or `?` followed by
whitespace, with `=`-prefixed Typst heading lines and `//` comments
ignored. The title is truncated to 60 characters in the tree pane to keep
each row on a single line; the full title remains in bdslib and in the
editor pane header.

A short cheat sheet:

| Want to…                          | Key                          |
| --------------------------------- | ---------------------------- |
| Quit (autosaves if dirty)         | `Ctrl+Q`                     |
| Save the paragraph                | `Ctrl+S`                     |
| Cycle panes                       | `Tab` / `Shift+Tab`          |
| Jump to specific pane             | `Ctrl+1`/`2`/`3`/`4`/`5`     |
| Search                            | `Ctrl+/`                     |
| AI prompt                         | `Ctrl+I`                     |
| Add book (tree pane)              | `B`  (or `Ctrl+B` then `B`)  |
| Add chapter (tree pane)           | `C`  (or `Ctrl+B` then `C`)  |
| Add subchapter (tree pane)        | `A`  (or `Ctrl+B` then `S`)  |
| Add paragraph (tree pane)         | `+`  (or `Ctrl+B` then `P`)  |
| Delete branch (tree pane)         | `D`  (or `Ctrl+B` then `D`)  |
| Delete paragraph (tree pane)      | `-`  (or `Ctrl+B` then `D`)  |
| Reorder current node              | `Ctrl+B` then `↑`/`↓`        |
| Vertical-block selection          | `Alt+arrows` then `Alt+C`    |

All add/delete/reorder operations go through a meta-prefix chord
(`Ctrl+B` by default). Press `Ctrl+B`, then the action letter (`B`, `C`,
`S`, `P`, `D`) or arrow. This replaces the old `Ctrl+Shift+*` chords that
terminals and multiplexers were eating. The Tree-pane plain-letter
shortcuts (`B`, `C`, `V`, `A`, `S`, `+`, `P`, `D`, `-`) still work
directly without the meta prefix.

If `Ctrl+B` itself is intercepted (tmux uses it as default prefix), set
`meta_prefix` to something else in `inkhaven.hjson` (e.g. `Ctrl+g`).

Full reference: **[KEYBINDING.md](KEYBINDING.md)** — covers every chord in
every pane / overlay, plus the configurable bindings and §12 on terminal
interception.

## Configuration

Per-project at `<root>/inkhaven.hjson`. The shipping defaults live at
`assets/default_project.hjson` and are written verbatim on `init`. Missing
fields fall back to compiled-in defaults, so a config from an older release
keeps working when new fields are added.

```hjson
{
  embeddings: {
    // fastembed model. Defaults are multilingual; Russian works out of the
    // box. Other options: MultilingualE5Base, MultilingualE5Large, BGEM3.
    model: MultilingualE5Small
    chunk_size: 800
    chunk_overlap: 0.15
  }

  llm: {
    default: gemini
    providers: {
      gemini: {
        model: gemini-2.5-pro
        api_key_env: GEMINI_API_KEY
      }
      deepseek: {
        model: deepseek-chat
        api_key_env: DEEPSEEK_API_KEY
      }
    }
  }

  editor: {
    theme: default
    tab_width: 2
    wrap: true          // soft word-wrap in the editor; false → horizontal scroll
  }

  hierarchy: {
    unbounded_subchapters: false
  }

  keys: {
    save:             Ctrl+s
    search:           Ctrl+/
    ai_prompt:        Ctrl+i
    next_pane:        Tab
    prev_pane:        Shift+Tab
    page_up:          PageUp
    page_down:        PageDown
    meta_prefix:      Ctrl+b
  }

  prompts_file: prompts.hjson
}
```

## AI integration

`genai` figures out the provider from the model string — `gemini-*` →
Gemini, `deepseek-*` → DeepSeek, `gpt-*` → OpenAI, `claude-*` → Anthropic,
and so on. Set the corresponding env var:

```bash
export GEMINI_API_KEY='…'
export DEEPSEEK_API_KEY='…'
```

Then either type into the AI prompt bar in the TUI (`Ctrl+I`) or use the CLI
`inkhaven ai "<prompt>"`. No API key set → clean error message, no crash.

Inside the TUI, type `/` in the AI prompt to pick from the prompt library
(`prompts.hjson`). Templates expand `{{selection}}` (current editor
selection or full paragraph if no selection) and `{{context}}` (the
Book › Chapter › Subchapter › Paragraph breadcrumb).

The default library ships with `tighten`, `darker`, `continue`,
`translate-ru`, and `typst-index`. Edit `prompts.hjson` to add your own.

## Export and rendering

```bash
# Single combined .typ
inkhaven --project ~/Books/my-novel export typst -o my-novel.typ

# Build a PDF (requires `typst` on PATH)
inkhaven --project ~/Books/my-novel export pdf -o my-novel.pdf
```

The exporter walks the hierarchy in depth-first preorder and concatenates
each paragraph's `.typ` content. Branch nodes don't emit anything
themselves — paragraphs carry the headings (templated as `= <Title>` by
`inkhaven add paragraph`). Book-level Typst configuration (page setup,
fonts, outline) is just a paragraph attached directly to the book — it
sorts first in DFS order and lands at the top of the export.

## Embedding cache

fastembed downloads model files to a per-user cache directory:

| OS      | Path                                                       |
| ------- | ---------------------------------------------------------- |
| macOS   | `~/Library/Caches/dev.inkhaven.inkhaven/embeddings/`       |
| Linux   | `$XDG_CACHE_HOME/inkhaven/embeddings/` (or `~/.cache/...`) |
| Windows | `%LOCALAPPDATA%\inkhaven\inkhaven\cache\embeddings\`       |

Switching `embeddings.model` in your config triggers a one-time download for
the new model on next start.

## Crate layout

```
src/
├── main.rs                  tokio runtime + tracing + clap dispatch
├── error.rs                 thiserror enum + alias
├── config.rs                Config (HJSON, serde-default)
├── project.rs               ProjectLayout: paths inside a project root
├── store/
│   ├── mod.rs               bdslib wrapper: open, create_node, swap_siblings,
│   │                        delete_subtree, update_paragraph_content
│   ├── node.rs              Node / NodeKind / JSON serialization
│   └── hierarchy.rs         in-memory snapshot built from list_metadata
├── ai/
│   ├── mod.rs               AiClient: genai::Client + provider resolution
│   ├── prompts.rs           PromptLibrary load + lookup
│   └── stream.rs            spawn_chat_stream: tokio::spawn → mpsc channel
├── cli/                     one module per `inkhaven` subcommand
│   ├── mod.rs init.rs add.rs delete.rs mv.rs list.rs
│   ├── search.rs reindex.rs ai.rs export.rs
└── tui/
    ├── mod.rs               public entry: run(project)
    ├── app.rs               App state machine, draw loop, key dispatch
    ├── focus.rs             Focus enum
    ├── input.rs             single-line TextInput buffer
    ├── keymap.rs            KeyChord parser
    ├── highlight.rs         tree-sitter wrapper + theme + wrap_line + selection overlay
    └── search_results.rs    typed SearchHit parsed from bdslib JSON
```

## Development

```bash
cargo build
cargo test                  # unit tests for keymap, input, highlight, search parser
cargo run -- init /tmp/x    # quick smoke
```

Build is clean (zero warnings); the test suite runs ~13 unit tests
covering the parser-heavy modules. The TUI itself is exercised manually —
ratatui's `TestBackend` could automate it in the future.

## Issues, PRs, ideas

[github.com/vulogov/blackInkhaven](https://github.com/vulogov/blackInkhaven)
