# Inkhaven (blackInkhaven)

**Inkhaven** is a standalone terminal application for writing books and
long-form technical documentation. It pairs a full-screen Typst editor with
a local semantic index, an AI writing assistant, versioned snapshots, and a
backup pipeline — so the entire writing workflow lives inside one binary,
without leaving the terminal.

Your manuscript is organised as a hierarchy of `.typ` files
(Book → Chapter → Subchapter → Paragraph), with first-class
**image** (`.png` / `.jpg` / …), **HJSON data** (`.hjson`), and
**Bund script** (`.bund`) leaves alongside paragraphs. Inkhaven
stores metadata in a local DuckDB database, indexes every text
node for full-text and semantic search, keeps versioned
snapshots, embeds the [Bund](Documentation/Bund/README.md)
scripting language for hooks + custom rules, and streams answers
from your chosen LLM provider — six are bundled (**Gemini**,
**Claude**, **OpenAI**, **DeepSeek**, **Grok**, **Ollama**) and any
model [genai](https://github.com/jeremychone/rust-genai) routes is
one HJSON line away.

![Inkhaven screenshot](screen.png)

## Latest release · 1.2.8 — Shell, config editor, and the surface gets sharper

Read the full notes: [`Documentation/RELEASE_NOTES/1.2.8.md`](Documentation/RELEASE_NOTES/1.2.8.md)

1.2.8 picks up the items 1.2.7 left on the table and adds two
significant new surfaces.  The headline features:

- **Embedded nushell pane** (`Ctrl+Z o`). A floating shell
  runs **real nushell in-process** via `nu_engine` +
  `nu_command` — the full default command set (`ls`,
  `where`, `str`, `path`, `into`, pipelines, env
  mutations) without shelling out.  External commands
  get their stdout + stderr captured into the pane;
  full-screen TUI apps (vim, less, top, tmux, …) are
  refused before spawn via a configurable blocklist.
  Per-project SQLite history, Tab autocomplete (commands
  + `$PATH` binaries + filesystem), readline-style line
  editing, `Ctrl+B H` help overlay, wall-clock watchdog
  for wedged externals.
- **HJSON config editor** (`Ctrl+B 0`).  Edit
  `<project>/inkhaven.hjson` from inside the TUI without
  dropping to an external editor.  Syntax-highlighted,
  mirrors the paragraph editor's chord set; saves flag a
  *Restart required* overlay when the config actually
  changed.
- **Help book as rendered-markdown viewer**.  Paragraphs
  under the Help system book render via pulldown-cmark —
  headings, lists, emphasis, code fences, blockquotes,
  links shown as real document presentation rather than
  source.  Read-only viewer, scroll-only keys, mouse
  wheel works.  Scope deliberately limited to the Help
  book; every other book keeps the Typst default.
- **`export-timeline` CLI + SVG / PNG renderers**.  Same
  swim-lane layout that the TUI shows, written to disk
  in three formats (`typst`, `svg`, `png`).  Optional
  `--track <name>` filter; canvas auto-sizes with track
  count.  Useful for query letters, pitch docs, private
  wiki pages.
- **Scrivener `<CustomMeta>` date import.**  Field names
  matching `scrivener.date_fields` (default `Date`,
  `Story Date`, `Event Date`) parse against the project's
  calendar; successful parses attach `EventData` to the
  imported paragraph.  Bad values become non-fatal
  warnings on the import report.
- **Mouse wheel everywhere.**  Tree, Editor, AI chat, OS
  Shell modal, HJSON editor, kill-ring picker, fuzzy
  paragraph picker.
- **Ctrl+Q confirmation modal** (opt-in via
  `editor.confirm_quit: true`).  For terminals where
  Ctrl+Q triggers software flow-control.

Smaller refinements:

- **Multi-slot kill-ring + Shift-letter chord fix.**
  `App.last_deleted` widened to a 10-entry ring; new
  `Ctrl+V Shift+U` picker.  Same commit fixes a
  pre-existing chord-matcher bug that collapsed
  `Ctrl+V Shift+P` onto `Ctrl+V p` on terminals without
  Kitty disambiguation.
- **Surface polish.** Active-LLM chip in the AI pane
  title, F1 query history, Tab autocomplete in the
  add-new-tag prompt, hidden-character report
  (`Ctrl+V h`), breadcrumb on the status bar
  (`Ctrl+V Shift+S`), F6 snapshot annotation filter.

Plus 2 new tutorials (36, 37); updated KEYBINDING,
CONFIGURATION, and the Bund tutorial.

### Previous release · 1.2.7 — Polish + plumbing

Read the full notes: [`Documentation/RELEASE_NOTES/1.2.7.md`](Documentation/RELEASE_NOTES/1.2.7.md)

1.2.7 is a polish cycle on the timeline-heavy 1.2.6 release.
Two themes carry the branch: **timeline + ergonomics
refinements** on the user side, **a large architecture
rework** on the source side (zero behavioural delta — verified
post-hoc).

User-facing highlights:

- **Timeline polish.** Tree-style two-level Tab/Enter
  navigation, `Space` track collapse with `▾`/`▸` glyphs,
  span-aware `↑`/`↓` event selection with viewport auto-pan,
  faint grid stripes, `F12` book-wide critique inside the
  swim-lane view, per-book session persistence in
  `.session.json`, book-slug prefix on cross-book tracks
  (project overlay), and selection-aware `Ctrl+V Shift+E`
  for one-chord quick-event creation from any pane.
- **`Ctrl+B U` paragraph undelete.** Single-slot kill-ring
  that round-trips body + tags + status + linked paragraphs
  + event timing after a Ctrl+D delete. New uuid; incoming
  paragraph links to the old id stay broken (status bar warns).
- **`Alt+←` / `Alt+→` navigation history.** Browser-style
  back / forward through every paragraph-open path —
  tree Enter, fuzzy picker, paragraph-link follow, snapshot
  picker, timeline Enter, undelete. Plus `Ctrl+V Shift+P`
  recent-paragraph picker (32 entries, deduped).
- **`Ctrl+Shift+M` mouse-capture toggle.** Flip terminal-
  native drag-select + system-clipboard copy on / off so
  you can grab snippets to paste elsewhere without
  routing through `Ctrl+C` chat-selection mode.
- **External-change auto-reload.** Passive watcher on every
  autosave tick: clean buffer → silent reload, dirty
  buffer → red warning. CLI / `sed` / `git pull` / Bund
  scripts that touch the open paragraph file no longer
  leave stale text on screen.
- **Snapshot dedupe + today-widget colour + F8 from any pane.**
  Three smaller refinements: F5 / autosave skip
  byte-identical snapshots, the today-words widget gets
  colour by goal progress, the typst-diagnostics modal
  works outside the editor pane.

Around those: 4 new tutorials (32 — 34) plus a "1.2.7 polish"
section on tutorial 31; updated keybinding reference.

### Under the hood — architecture rework

The TUI brain (`src/tui/app.rs`) went from a 26,605-LOC
monolith to a 13,939-LOC central dispatcher with 21
sibling / child modules hanging off it. Every commit on the
branch was byte-equivalent modulo visibility downgrades and
import-path rewrites, verified post-hoc by a programmatic
per-method diff (561 method bodies pre vs post; exactly one
differed, and that turned out to be a pre-existing
`save_current` bug that I fixed). 255 → 260 tests passing;
zero build warnings.

The split makes the source navigable: render painters, modal
state, AI orchestration, timeline navigation, tag picker,
editor buffer methods, etc. each live in named files instead
of being scattered across a single huge `impl App {}`. See
`src/tui/` for the layout.

### Bugfix worth flagging

`Ctrl+S` in the editor was snapping the cursor to the
beginning of the buffer on every save. Pre-existing since
the 1.2.7 external-change auto-reload landed — save wrote
the file but never restamped `loaded_mtime`, so the next
tick saw the fresh mtime and reloaded the buffer (which
resets the cursor). Fixed in this release. If you ever see
that recurrence, file a bug.

### Previous release · 1.2.6 — Tools for the working novelist

Read the full notes: [`Documentation/RELEASE_NOTES/1.2.6.md`](Documentation/RELEASE_NOTES/1.2.6.md)

1.2.6 is a wide-front release. Where 1.2.5 was one headline
(in-process Typst), this one moves on six themes at once:
**tagging as project-wide metadata**, **diagnostics with an
editor surface**, **AI as a writing partner** (critique +
memory + diff modal), **labelled snapshots**, a **paragraph-
mini story view**, and a brand-new **story timeline** with
calendar-aware events.

```hjson
# Opt-in to the timeline (still 1.2.6+).
timeline: {
  enabled: true
  calendar: { preset: "gregorian" }    # or "sols" or "custom"
}

# Opt into the diff modal on AI rewrites (default already on).
ai: {
  per_paragraph_memory: true   # default false
  diff_review_on_apply: true   # default true
}
```

Headlines from 1.2.6:

- **Tagging stack.** `Ctrl+B ]` picker · `Ctrl+B }` project
  search · `R` rename project-wide · `inkhaven export --tag`
  filter · tree-pane `#tag` pips · five `ink.tag.*` Bund
  words · Scrivener keyword import.
- **Diagnostic UX.** Red `●` in the gutter for every line
  with a parse / semantic error · F8 diagnostics list ·
  `Ctrl+V N` / Shift+N navigate · **Ctrl+F12** AI explain
  the diagnostic at cursor (was F11; macOS grabs F11).
- **AI as a partner.** `F12` mode-aware critique
  (`critique-edit` plain / `critique-changes` split-edit) ·
  per-paragraph memory (`ai.per_paragraph_memory: true`) ·
  side-by-side diff modal before any `r` / `g` apply ·
  smart marker extraction across model dialects.
- **Snapshot annotations.** `F5` pops a one-line prompt;
  `F6` picker shows annotations as italic-cyan `✎` lines
  beneath each row.
- **Story view split.** `Ctrl+V w` (lowercase) for the
  paragraph mini view; `Ctrl+V Shift+W` for the book.
  New Bund word `ink.story.render` writes the graph to PNG
  from a script.
- **Render preview live zoom.** `+ / -` zoom in Ctrl+V R;
  `0` resets; cursor column anchored.
- **🌟 Story timeline.** Calendar-aware events (`gregorian`
  / `sols` / `custom`), `inkhaven event add/list/show` CLI,
  `Ctrl+V e` picker, `Ctrl+V t` swim-lane view with
  scope navigation (`u` / `d` / `b` / `p`), AI health
  critique (`y` / `Y` / `Ctrl+Y`), seven `ink.event.*` Bund
  words + two new hooks (`hook.on_event_added` /
  `hook.on_event_orphaned`). Opt-in via `timeline.enabled`.

The companion **Book of Inkhaven** at `Book/` covers all of
1.2.6 in print form. It stays synced to 1.2.6 until the next
major release; 1.2.7 polish lives in the tutorials.

### Previous release · 1.2.5

```hjson
typst_compile: { engine: "inprocess" }
```

Flip it and Inkhaven stops shelling out to the host's `typst`
binary for builds. The full compiler — `typst` + `typst-pdf` +
`typst-kit` (fonts + `@preview` packages) — is linked into
every 1.2.5+ binary and runs inside the inkhaven process. The
external CLI stays the default; the switch is a runtime
decision.

Around that one switch:

- **In-process compile engine.** `typst::compile +
  typst-pdf` runs on a worker thread; the foreground TUI keeps
  the spinner animated. Bundled Computer Modern + Linux
  Libertine fonts ship in the binary; system fonts are also
  searched. `@preview/<pkg>` imports fetch + cache via
  `typst-kit`. Hermetic mode (no system fonts, no package
  fetch) lives behind two HJSON booleans.
- **Parse diagnostics on save / idle.** `typst-syntax` re-
  parses the open paragraph on every save and every
  `diagnostics_idle_seconds` of editor idle. First parse
  error lands on the status bar with `line L:C — <message>`.
  Engine-independent — works in both `external` and
  `inprocess`.
- **Semantic diagnostics (opt-in).** When
  `semantic_diagnostics: true` AND `engine = "inprocess"`,
  a full compile runs after parse passes. Surfaces unknown
  functions, type errors, missing fonts. False positives
  expected for paragraphs that depend on book-level
  definitions — leave off for preamble-heavy manuscripts.
- **Ctrl+V N — diagnostic navigation.** Jump the editor
  cursor to the next typst diagnostic (parse or semantic).
  Wraps. Status bar reports `diag 2/5 line 12:5 —
  <message>`.
- **Ctrl+V R — render paragraph preview.** Saves the buffer,
  rasterises every page in-process via `typst-render`, floats
  the PNG on top of the editor. `← / →` navigate pages,
  `S` saves the current page at full DPI, `A` saves every
  page as `<base>-page-NNN.png`. Esc closes back to the
  editor (cancelling the save picker preserves navigation
  state).
- **TUI-friendly compiles.** Compile splash shows the active
  engine line (`internal · fonts: bundled + system · @preview:
  on` or `external · /usr/local/bin/typst`). **Esc** cancels
  in-flight compiles — external sends SIGTERM, in-process
  abandons the worker (it finishes in the background).
  Ctrl+B A / B / O now autosave the primary editor (and the
  secondary editor in similar-paragraph mode) before walking
  `.typ` files.
- **`inkhaven doctor`.** New CLI — prints a health report
  with engine summary, typst path, font + package counts,
  cache size, and (when run inside an initialised project)
  hierarchy shape + word counts. Notes section calls out
  actionable warnings like `typst NOT on PATH`. Pipe-
  friendly.
- **Embedded logo in credits pane.** `include_bytes!`
  embeds `logo.png` in the binary; `Ctrl+B V` banners it
  above the version + dependency list.
- **Project-wide node tagging.** Three entry points,
  one tag namespace stored as `Node.tags: Vec<String>`:
  `Ctrl+B ]` opens the floating tag picker for the open
  paragraph (Space multi-selects, `T` applies, `A` adds
  a new tag, `D` deletes a tag project-wide with a blast-
  radius confirm); `Ctrl+B }` opens the same picker in
  search mode — Enter on a tag lists every paragraph that
  carries it, with a typeable filter, and Enter on a hit
  opens it; **`g`** in the tree pane runs the picker
  against the marked set (or the cursor row), bulk-tagging
  a whole selection in one go. Editor `T` returns focus
  to the editor; tree `g` stays in the tree.
- **Story view — Ctrl+V W.** Twopi-style radial graph of
  the current book: book at the centre, each structural
  depth on a concentric ring with sibling wedges sized by
  subtree leaf count. Different SVG shape per node kind
  (folder / box / octagon / ellipse / note / parallelogram /
  chevron / egg / diamond / hexagon); long titles wrap to
  multiple lines and node boxes scale to fit. Solid grey
  edges for the structural skeleton, dashed purple for
  `linked_paragraphs` paragraph links, dashed green from
  Characters / Places / Artefacts to the paragraphs that
  mention them. Rasterised via `resvg` + `tiny-skia`,
  displayed via ratatui-image. `S` saves the PNG.
- New tutorial: [Typst in-process](Documentation/Tutorials/24-typst-in-process.md)
  — engine switch, fonts, packages, diagnostics, render
  preview, doctor.

Every prior release lives under [`Documentation/RELEASE_NOTES/`](Documentation/RELEASE_NOTES/).

## Why Inkhaven

- **Terminal-first.** Inkhaven runs over SSH, in tmux, on a tiling WM — no
  browser, no Electron. The TUI uses [ratatui](https://ratatui.rs/) and
  [tui-textarea](https://github.com/rhysd/tui-textarea).
- **Your manuscript is plain files.** A paragraph lives in a `.typ` file
  on disk; the metadata database tracks hierarchy and search but the prose
  is text — you can read it, diff it, version-control it, and edit it with
  another tool any time.
- **Semantic search out of the box.** Embeddings via fastembed and HNSW are
  computed locally. Search for *"the moment the lighthouse fails"* and find
  the paragraph even if it never uses those exact words.
- **AI is a co-author you steer.** Inferences stream live; you control the
  **scope** (selection / paragraph / subchapter / chapter / book), the
  **mode** (Local-only RAG vs. Full general knowledge), and the
  **destination** (replace, insert, top, bottom, copy, grammar-apply).
  Inkhaven does NOT provide inherent privacy when external providers
  (Gemini / Claude / OpenAI / DeepSeek / Grok) are used — prompts
  travel to their servers under their terms. For privacy, set
  `llm.default_provider: "ollama"` and run a local model; every other
  inkhaven subsystem (RAG embedding, semantic search, snapshot diff)
  is already on-device.
- **Multilingual.** Snowball stemmers and multilingual embeddings make
  Russian, German, French, Spanish, Italian and others first-class. The
  shipped defaults cover English and Russian.
- **Help, characters, places, artefacts, scripts — built in.** Nine
  system books are seeded on every project: `Notes`, `Research`,
  `Prompts`, `Places`, `Characters`, `Artefacts`, `Typst`, `Scripts`,
  `Help`. Mentions of names from the lexicon books light up in the
  editor (cyan / amber / peach / underline). `Ctrl+B P` / `C` / `Y` /
  `G` query each via RAG. `F1` answers questions about Inkhaven itself
  by RAG over `Help`. `Scripts` (added in 1.2) holds `.bund` source
  files auto-loaded into the embedded Bund scripting VM at project
  open — see [`Documentation/Bund/`](Documentation/Bund/README.md).
- **First-class images.** Drop PNG / JPG / WebP / SVG into the tree;
  Book assembly emits the right `wrap_image_*` calls and ships the
  bytes into the typst tree. `Ctrl+B P` inside `#image("…")` opens a
  sibling picker. Enter on an Image row pops a ratatui-image preview
  (kitty / sixel / iterm2 / half-block).
- **From buffer to PDF in two chords.** `Ctrl+B A` assembles your tree
  into a typst-compilable directory; `Ctrl+B B` compiles it; `Ctrl+B O`
  builds and copies the PDF into your shell's cwd as
  `<book>-YYYYDDMM-HHMM.pdf`. Compile failures route the captured
  stderr into a fresh AI chat with a typst-aware system prompt.

## Features at a glance

### Editor
- Typst syntax highlighting via [tree-sitter-typst](https://github.com/uben0/tree-sitter-typst).
- Regex find / replace with same-line current-match highlighting.
- Split-edit with versioned snapshots — see two versions of a paragraph
  side by side, accept either.
- Word-aware navigation and deletion shortcuts.
- Vertical block selection (Alt+arrows) with rectangular copy.
- System-clipboard cut / copy / paste, plus per-doc undo / redo.
- Live "changes since last save" bolding; grammar-correction highlights
  what changed after a `g` apply.

### Tree
- Multi-level folding (`←` / `→` / `Z` / `X`).
- Per-kind row colours (book / chapter / subchapter / paragraph / image)
  + open-paragraph marker.
- Plain-letter shortcuts for add (`B`/`C`/`V`/`A`/`S`/`+`/`P`),
  delete (`D`/`-`), reorder (`U`/`J`).
- **Document status badge** column — one character per row colour-
  coded to the workflow stage (`n` / `1` / `2` / `3` / `F` / `R`).
- Mouse: click to focus + select; scroll wheel scrolls.

### AI pane
- Streaming markdown rendering — bold / italic / headings / code / lists.
- Six **scope modes** (cycled by `F9`): None, Selection, Paragraph,
  Subchapter, Chapter, Book — each prepends the matching content to the
  next prompt.
- Two **inference modes** (`F10`): **Local** (use only supplied context)
  and **Full** (augment with general knowledge). Help inferences are pinned
  to Local automatically.
- Persistent **chat history** with one-key clear (`Ctrl+B C`).
- **Full-screen AI layout** (`Ctrl+B K`) — AI pane + scrollable chat
  history + AI prompt; persisted to `.inkhaven-chat.json` between
  sessions; `Ctrl+F` searches; `Ctrl+C` enters a turn-selection mode.
- **Lexicon RAG** — `Ctrl+B P` / `C` / `G` / `Y` in the editor sweep
  the selection through `Places` / `Characters` / `Notes` / `Artefacts`
  and prepend the lookup to the next AI prompt.
- **F1 Help-manual** floating query → grounded answer over the Help book.
  `inkhaven import-typst-help` seeds Help with a curated typst reference.
- **F7 Grammar check** with deterministic correction extraction (`g`
  replaces the buffer with just the corrected text, preserving Typst
  markup).

### Storage and backup
- DuckDB metadata + Tantivy full-text + HNSW vectors via
  [bdslib](https://github.com/vulogov/bdslib).
- Snapshots: `F5` captures the buffer; `F6` opens the snapshot history
  picker.
- `inkhaven backup --out <dir>` zips the entire project.
- `inkhaven restore <archive> --to <dir>` puts it back.
- Auto-backup on TUI exit when the last backup is older than
  `backup.max_age` (humantime: `7d`, `12h`, `30m`, …) — splash screen with
  a progress bar.
- Session persistence: cursor position, focus, tree-scroll, open paragraph
  all survive restarts. Per-paragraph cursor memory: switch around and
  every paragraph remembers where you were.

### CLI tools
- `init` — set up a fresh project (interactive confirmation if the
  directory exists).
- `add` / `delete` / `mv` / `list` — manage the hierarchy from a script.
- `search "phrase"` — semantic search from the shell.
- `reindex` — re-walk `.typ` files into the database.
- `export typst` / `export pdf` — produce a single Typst manuscript or a
  built PDF.
- `import-help --documents-directory <dir>` — populate the Help book from
  a directory of markdown / text / typst files (wipes Help first).
- `backup` / `restore` — see above.
- `ai "prompt"` — one-shot inference from the shell (no TUI).

### Configuration
A single `inkhaven.hjson` in each project root drives every knob:
embedding model, LLM providers, autosave cadence, sync interval, hierarchy
depth, language, snowball stemmers, the full visual theme (per-pane
backgrounds and foregrounds, all syntax colours, lexicon highlight
colours), key bindings, and backup policy.

## Install

Inkhaven ships as a single static binary per platform. Three install paths:

### 1. `cargo binstall` (no compile)

If you already have [`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall):

```bash
cargo binstall inkhaven
```

`cargo-binstall` reads `[package.metadata.binstall]` from `Cargo.toml`,
picks the right asset off GitHub Releases, and drops the binary into
`~/.cargo/bin`. Works on Linux (x86_64), macOS (Intel + Apple Silicon),
and Windows (x86_64).

### 2. GitHub Releases (direct download)

Grab the tarball for your platform from
[Releases](https://github.com/vulogov/blackInkhaven/releases), unpack,
and put `inkhaven` somewhere on your `PATH`. Builds are produced by
the [`release.yml`](.github/workflows/release.yml) workflow on every
tag push.

### 3. `cargo install --git` (compile from source)

```bash
cargo install --git https://github.com/vulogov/blackInkhaven --tag v1.0.0
```

This works because every dependency (including `bdslib` and
`tree-sitter-typst`) is vendored under `vendor/` — no separate registry
fetches, no GitHub auth needed. The first build takes ~10 minutes on a
modern laptop because of DuckDB + Tantivy + fastembed compilation; the
release binary above is the fast path.

> Inkhaven is **not** published on crates.io. See `Cargo.toml`'s
> `publish = false` line and the [`Documentation/`](Documentation/)
> notes for the rationale.

## Quick start

```bash
# Build (if installing from source)
cargo build --release

# Initialise a project (asks for confirmation if the directory exists)
./target/release/inkhaven init ~/Books/my-novel

# Build the hierarchy from the CLI…
./target/release/inkhaven --project ~/Books/my-novel add book "My Novel"
./target/release/inkhaven --project ~/Books/my-novel \
    add chapter "The Beginning" --parent my-novel
./target/release/inkhaven --project ~/Books/my-novel \
    add paragraph "Opening Scene" --parent my-novel/the-beginning

# …or skip the CLI and add everything from the TUI
./target/release/inkhaven --project ~/Books/my-novel
# Inside the TUI: B (book), C/V (chapter), A/S (subchapter), +/P (paragraph)
```

## Use cases

- **Long-form fiction.** Hierarchy fits novels naturally (Book → Part →
  Chapter → Scene). Places / Characters / Research system books keep
  worldbuilding next to prose.
- **Technical documentation.** Each chapter is a `.typ` file; the tree
  doubles as a table of contents. Semantic search makes "where did I
  document the retry policy?" a one-keystroke question.
- **Translation work.** Multilingual embeddings + per-language Snowball
  stemmers let you keep source and target in two parallel books.
- **Research notebooks.** Snapshots track how a draft evolved; the AI pane
  can summarise a chapter when you come back after a week.
- **Help and onboarding writing.** Ship docs as a directory and let
  Inkhaven build a Help book your readers can query through F1.

## Documentation

The full docs live under [`Documentation/`](Documentation/).

Start here:

- [`Documentation/README.md`](Documentation/README.md) — entry point and
  table of contents.
- [`Documentation/FIRST_STEPS.md`](Documentation/FIRST_STEPS.md) — compile,
  install, initialise.
- [`Documentation/Tutorials/`](Documentation/Tutorials/) — narrative
  walk-throughs, each focused on one workflow.

Reference:

- [`Documentation/KEYBINDING.md`](Documentation/KEYBINDING.md) — every
  keystroke the TUI recognises, organised by pane and overlay.
- [`Documentation/CONFIGURATION.md`](Documentation/CONFIGURATION.md) —
  the full HJSON reference.
- [`Documentation/MAINTENANCE.md`](Documentation/MAINTENANCE.md) — backup,
  restore, reindex, logs.
- [`Documentation/PROMPTS.md`](Documentation/PROMPTS.md) — the prompt
  library and the Prompts system book.
- [`Documentation/LOCATIONS.md`](Documentation/LOCATIONS.md) — managing
  Places.
- [`Documentation/CHARACTERS.md`](Documentation/CHARACTERS.md) — managing
  Characters.

## Built with

- [bdslib](https://github.com/vulogov/bdslib) — DuckDB + Tantivy +
  fastembed + HNSW document store
- [ratatui](https://ratatui.rs/), [tui-textarea](https://github.com/rhysd/tui-textarea)
- [tree-sitter](https://tree-sitter.github.io/) +
  [tree-sitter-typst](https://github.com/uben0/tree-sitter-typst)
- [genai](https://github.com/jeremychone/rust-genai) — provider-neutral
  LLM streaming
- [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark),
  [rust-stemmers](https://github.com/CurrySoftware/rust-stemmers),
  [zip](https://github.com/zip-rs/zip2),
  [humantime](https://github.com/tailhook/humantime), and many others —
  see [`Cargo.toml`](Cargo.toml).

## Licence

Apache 2.0 — see [`LICENSE`](LICENSE).
