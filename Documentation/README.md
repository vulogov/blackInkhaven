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
| [`INKHAVEN_CHEAT_SHEET.typ`](INKHAVEN_CHEAT_SHEET.typ) | Two-column A4 cheat sheet — print or pin it next to your terminal. Compile with `typst compile Documentation/INKHAVEN_CHEAT_SHEET.typ`. Companion to `KEYBINDING.md`. |
| [`KEYS_REASSIGNMENT.md`](KEYS_REASSIGNMENT.md) | Rebind chords via `keys.bindings` in HJSON or via the `ink.key.*` Bund stdlib. Includes the full action table. |
| [`MAINTENANCE.md`](MAINTENANCE.md)     | Backup, restore, auto-backup-on-exit, the `reindex` command, log files, recovering from drift, troubleshooting first-run model downloads. |
| [`CONLANG.md`](CONLANG.md)             | The ConLang Suite (1.3.14+, RFC LANG-1) — build a constructed language inside the editor: phonology / lexicon / morphology HJSON blocks, the `inkhaven language` CLI surface, AI dictionary generation + dedup gate, `Ctrl+B X` hub, `:lang:` insertion. |
| [`WORLDBUILDING.md`](WORLDBUILDING.md) | World simulation (1.3.25+, RFC WORLD-4) — declare a world's physics in `world.hjson` and a deterministic compiler derives astronomy / geology / climate / hydrology / demographics, materialized into the **World** book; the `inkhaven realworld` CLI (compile / propose / map / coherence), plakat maps, and the live multilingual fact-checker (`Ctrl+B W` hub). |
| [`INNER_SOCRATES.md`](INNER_SOCRATES.md) | Examined authorship (1.3.x, RFC INNER_SOCRATES-1) — a Socratic interrogator that surfaces **questions** about your prose (never corrections): a deterministic Fast track + an LLM Slow track, 15 categories, five Reader Personas, the intent ledger (the magic ledger's prose sibling), and the `Ctrl+B J` hub. Five languages; zero new deps. |
| [`GRAPH.md`](GRAPH.md)                 | The semantic net (2.0, RFC SEMNET-1) + **GRAPHMIND** (2.x) — a typed-edge knowledge-graph layer over the nodes you already have, and the AI surfaces over it: the edge model (kinds / endpoints / origin & durability), how to populate it (`graph rebuild` structural edges, `graph lexical` WordNet bridge, live confront stance), the `inkhaven graph` command (`stats` / `neighbors` / `contradicting` / `loci` / `paths` / `promote` / `dismiss` / `pending` / `link` / `ask`), **chatting with your graph** (the F9 **Graph** scope, and walking the graph via `graph ask` or the in-editor `Ctrl+B z → w`), multilingual behaviour, and `edges.db` crash-safety. |
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
| [`CHORUS.md`](CHORUS.md)               | Voice & style at book scale (2.1, RFC CHORUS-1) — NARR-1 profiles the narrator; CHORUS profiles the **cast**: character voice fingerprints + a distinctiveness matrix ("these two read alike") + per-character drift, POV/head-hop + (English-only) tense discipline, and register drift — all synthesised by the **Inner Stylist** (the 7th inner-family reader) into Praise/Note/Concern coaching. `inkhaven chorus voices / scan / report / stylist`; the `pov:<name>` scene tag; the honest Russian-tense exclusion. |
| [`CONTINUITY.md`](CONTINUITY.md)       | Continuity intelligence — the book watches itself (2.2, RFC SENTINEL-1). One engine unifies every deterministic continuity detector (co-location · timeline · numeric · character-fact drift) and adds the **referenced-before-introduced** invariant; ranked + deduped into one ledger, one review-pass line, one `continuity:` config block. The `inkhaven continuity check` CLI (`--only`/`--skip`/`--json`/`--coherence`, CI exit), the `Ctrl+B Shift+L` dashboard (jump-to-paragraph · `k` runs the LLM coherence pass), the incremental on-save **watch** (`continuity.ambient`), the `ink.continuity.*` Bund words, and the per-detector multilingual coverage. |
| [`OUTPUT_PANE.md`](OUTPUT_PANE.md)     | The **Output pane** (1.3.24+, RFC PANE-1) — the right-region notice board for structured one-way messages (translation results, lexicon proposals, Bund `print`, finished jobs). `Ctrl+B Tab` cycling, per-kind `Enter` actions, severities / lifetimes, the `ink.io.*` scripting surface, and the `inkhaven output` CLI. |

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
| [`Tutorials/15-multi-format-export.md`](Tutorials/15-multi-format-export.md)     | Markdown / TeX (via tylax) / EPUB export, `--book-name`, Ctrl+B O extras, Ctrl+V markdown. |
| [`Tutorials/16-similar-paragraphs.md`](Tutorials/16-similar-paragraphs.md)       | Ctrl+V S — vector-similarity picker + side-by-side editor. |
| [`Tutorials/17-writing-goals.md`](Tutorials/17-writing-goals.md)                 | Writing-progress subsystem, the `goals:` HJSON stanza, Ctrl+V G overview modal. |
| [`Tutorials/18-bund-pane-and-script-picker.md`](Tutorials/18-bund-pane-and-script-picker.md) | The floating Bund pane, Ctrl+Z ? script picker, `ink.input` prompt modal. |
| [`Tutorials/19-wiki-links.md`](Tutorials/19-wiki-links.md) | Metadata-only paragraph links + backlinks (Ctrl+V A / I / L / K), AI inference integration. |
| [`Tutorials/20-snapshot-diff.md`](Tutorials/20-snapshot-diff.md) | F6 V snapshot diff view + pre-restore safety snapshot on Enter. |
| [`Tutorials/21-navigation.md`](Tutorials/21-navigation.md) | Ctrl+V P fuzzy picker, Ctrl+V B/M bookmarks, AI Up-arrow history, slash-command ranking. |
| [`Tutorials/22-tree-multiselect.md`](Tutorials/22-tree-multiselect.md) | Tree-pane Space mark set, T cycles type, O cycles status — bulk on demand. |
| [`Tutorials/23-scrivener-import.md`](Tutorials/23-scrivener-import.md) | `inkhaven import-scrivener` — single-binary `.scriv` ingest with RTF→Typst. |
| [`Tutorials/24-typst-in-process.md`](Tutorials/24-typst-in-process.md) | `typst_compile.engine = "inprocess"` — bundled compiler + fonts + `@preview` packages, diagnostics (parse + semantic), Ctrl+V R render preview + page nav + save-all, Ctrl+V N diagnostic navigation, `inkhaven doctor`. |

## Release notes

| Version | Notes |
| ------- | ----- |
| **1.2.5** | [`RELEASE_NOTES/1.2.5.md`](RELEASE_NOTES/1.2.5.md) — **Typst goes in-process**: `typst_compile.engine = "inprocess"` runs `typst::compile + typst-pdf` inside inkhaven (bundled fonts, `@preview` packages). Parse + opt-in semantic diagnostics. `Ctrl+V R` render-paragraph preview with ←/→ page navigation, S saves current, A saves all. `Ctrl+V N` next-diagnostic. Esc cancels in-flight compiles; autosave before Ctrl+B A/B/O. New `inkhaven doctor` CLI. Embedded logo banners the credits pane. |
| **1.2.4** | [`RELEASE_NOTES/1.2.4.md`](RELEASE_NOTES/1.2.4.md) — wiki-links + backlinks (Ctrl+V A/I/L/K), per-paragraph word-count goals + auto-promote, active-time tracking, per-book bar chart, snapshot diff (F6 V) + safety snapshot, save-as picker, F-keys in keybind table, theme persistence to HJSON, Bund stdlib expansion (`ink.fs.*`, `ink.editor.replace_all`, `ink.search.load`, `ink.ai.poll`, `ink.ai.send_blocking`), Ctrl+V P fuzzy picker, Ctrl+V B/M bookmarks, AI prompt history, tree multi-select with T / O chords, `inkhaven stats`, startup splash, Scrivener importer, Windows CI re-enabled. |
| **1.2.3** | [`RELEASE_NOTES/1.2.3.md`](RELEASE_NOTES/1.2.3.md) — multi-format export (markdown / TeX / EPUB) + `--book-name`, writing-progress subsystem (Ctrl+V G), similar-paragraph mode (Ctrl+V S), Bund output pane + Ctrl+Z ? script picker + `ink.input`, dynamic Quick Help. |
| **1.2.1** | [`RELEASE_NOTES/1.2.1.md`](RELEASE_NOTES/1.2.1.md) — bdslib + tree-sitter-typst absorbed in-tree (crates.io-publishable), Bund scripting (`ink.*` stdlib, 5 hook points, `.bund` Script nodes, Scripts system book), data-driven keymap with HJSON + Bund rebinding, `Ctrl+B M` cycle-type, `Ctrl+Z` Bund prefix. |
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
