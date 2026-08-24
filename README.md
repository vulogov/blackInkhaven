# Inkhaven (blackInkhaven)

[![Crates.io](https://img.shields.io/crates/v/inkhaven.svg)](https://crates.io/crates/inkhaven)
[![Downloads](https://img.shields.io/crates/d/inkhaven.svg)](https://crates.io/crates/inkhaven)
[![License](https://img.shields.io/crates/l/inkhaven.svg)](https://crates.io/crates/inkhaven)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![Edition](https://img.shields.io/badge/edition-2024-blue.svg)](https://doc.rust-lang.org/edition-guide/rust-2024/index.html)

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
node for semantic search, keeps versioned
snapshots, embeds the [Bund](Documentation/Bund/README.md)
scripting language for hooks + custom rules, and streams answers
from your chosen LLM provider — six are bundled (**Gemini**,
**Claude**, **OpenAI**, **DeepSeek**, **Grok**, **Ollama**) and any
model [genai](https://github.com/jeremychone/rust-genai) routes is
one HJSON line away.

![Inkhaven screenshot](screen.png)

## Latest release · 3.4.0 — Export Fidelity & a Smoother Revision Loop

The current release is **3.4.0**, a **widen-and-polish** release — no new
capability. It deepens the export formats you already have (so more of your
manuscript survives the trip out) and tightens the Editorial Pass revision loop.
Two tracks (deterministic, no new runtime crates):

- **Export fidelity.** An audit of every export path found one lossy bottleneck
  and a few format-specific holes. The shared **`typst_to_markdown`** converter
  (which feeds markdown export, `export epub`, *and* the HTML prose path) now
  handles `#figure(image(…))`, inline `#footnote[…]` (→ a `[^N]` marker + a
  definition list), and `@key`/`@key[locus]` references (→ pandoc `[@key]`).
  **docx + the Shunn-format PDF** render authored `*bold*`/`_italic_` as real
  emphasis instead of literal `\*bold\*` (central for fiction). CLI
  **`export epub`** routes to the rich, multi-chapter builder (images, footnote
  popups, cover) instead of the old single-chapter converter. And **HTML verse**
  (`para:verse-*`) preserves its line breaks + stanzas.
- **Editorial Pass, round three.** The revision loop kept resetting to the top.
  Now the pass **remembers your place**: after a jump / act / batch it reopens on
  the same finding — or the next, if you just fixed it — with your filters intact;
  a deliberate `Esc` forgets the position; and the `F` batch **auto-reopens** the
  pass when it finishes, so triage continues without a manual `Ctrl+V Shift+R`.

Deferred as follow-ups: the Scrivener non-ASCII import, the EPUB-import round-trip,
and full retirement of the single-chapter epub toy (still used by the Bund/TUI
in-memory exporters). No new config; no new runtime crates.

Read the full notes: [`Documentation/RELEASE_NOTES/3.4.0.md`](Documentation/RELEASE_NOTES/3.4.0.md).

### Recent releases

- **3.3.0 — Wider Reach, Sharper Tools.** Multilingual parity (SENTINEL numeric
  DE+RU, CHORUS register DE+ES, Inner Socrates tense-shift DE/FR/ES), the
  `ink.world.*` Bund reads, and the first Editorial Pass ergonomics (session-skip
  persistence + the `r` response-kind filter).
  [notes](Documentation/RELEASE_NOTES/3.3.0.md)
- **3.2.0 — ENSEMBLE: Your Cast, Connected.** Deepened the people layer: declared
  `rel:` bonds became first-class **`relates` graph edges** (traversable via
  `graph neighbors` / the `Ctrl+B z` hub / F9 Graph chat), and the **Dramatis
  Personae** (`inkhaven cast`, hub → `c`) joined the cast × bonds × arc state into
  one book-wide view.
  [notes](Documentation/RELEASE_NOTES/3.2.0.md) · [`ENSEMBLE.md`](Documentation/ENSEMBLE.md)
- **3.1.0 — BONDS: Are the Bonds Earned?** KEN's sibling: relationship
  *continuity* — are the bonds you declare (`rel:` tags) earned on the page?
  `unwritten_bond` / `unearned_shift` / `dropped_bond` + opt-in `implied_cooling`,
  the `Ctrl+V Shift+O` dashboard, `ink.bonds.*`.
  [notes](Documentation/RELEASE_NOTES/3.1.0.md) · [`BONDS.md`](Documentation/BONDS.md)

### The stable baseline · 3.0.0 — The Stable Edition

**3.0.0** remains the stable baseline. It closed the 2.x arc and promoted Inkhaven
to a **stable, maintained edition** — the intelligence suite that watches, reads,
and remembers itself (SEMNET → GRAPHMIND → CHORUS → SENTINEL → LECTOR → REDLINE →
CHRONICLE → KEN) verified sound by a seven-partition adversarial audit of all 304K
lines (zero Critical), with every surviving finding fixed and zero breaking
changes. The 3.0.x line (through 3.0.9) then hardened the Bund scripting coverage
and the export/import/keymap/assembly subsystems through successive targeted
audits, each leaving a build-time guard behind. **3.1.0 lifted the feature freeze**
(BONDS); **3.2.0** (ENSEMBLE), **3.3.0** (multilingual parity), and **3.4.0** (export fidelity) continue
on that hardened surface. Full 3.0.0 notes:
[`Documentation/RELEASE_NOTES/3.0.0.md`](Documentation/RELEASE_NOTES/3.0.0.md).

### Dependencies & compatibility

No new dependencies, no breaking changes. Existing projects open unchanged. Compiles
warning-free. Test suite → 2972.

Every prior release lives under
[`Documentation/RELEASE_NOTES/`](Documentation/RELEASE_NOTES/).

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
  `llm.default: "ollama"` and run a local model; every other
  inkhaven subsystem (RAG embedding, semantic search, snapshot diff)
  is already on-device.
- **Multilingual.** Snowball stemmers and multilingual embeddings make
  Russian, German, French, Spanish, Italian and others first-class. The
  shipped defaults cover English and Russian.
- **Help, characters, places, artefacts, scripts — built in.** Twenty
  system books are seeded on every project: `Notes`, `Research`,
  `Sources`, `Glossary`, `Snippets`, `Facts`, `Prompts`, `Places`,
  `Characters`, `Artefacts`, `World`, `Threads`, `Planning`, `Language`,
  `Submissions`, `Typst`, `Scripts`, `Help`, `Intent`, `Mythology`.
  Mentions of names from the lexicon books light up in the
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
- **Paragraph flavours** beyond prose: `e` adds a **Jinja template** (`⟡`,
  rendered to Typst at assembly), `i` opens the **structural-subtype** picker
  (`⌨ ⚠ ∫ ≡ ⊞` — code / admonition / math / procedure / table), and `t`/`T`
  cycles a leaf's type (`typst → hjson → jinja → bund`).
- **Document status badge** column — one character per row colour-
  coded to the workflow stage (`n` / `1` / `2` / `3` / `F` / `R`).
- **Reusable snippets** — write a block once in the Snippets book, `#include`
  it anywhere (`Ctrl+V x`); broken references are flagged at save.
- Deletion safety: the confirm shows the **word count** lost; branch deletes
  stash every paragraph into the kill-ring (`Ctrl+B U`) **and** pre-delete
  snapshots (`F6`).
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

### Examined authorship & analysis
- **The companions** — a triad that observes craft without rewriting your prose:
  the **World fact-checker** (`Ctrl+B W`, checks scenes against your worldbuilding
  + timeline), **Inner Socrates** (`Ctrl+B J`, Socratic questions about content
  and structure — fourteen reader personas), and **Inner Editor** (`Ctrl+V O`,
  style observations: richness, filter words, show-don't-tell). `Ctrl+B Shift+C`
  runs them all at once into the Output pane.
- **Narrative voice profiling** (`inkhaven prose` / `Ctrl+V V`) — a
  **deterministic, zero-AI** voice fingerprint per chapter (sentence rhythm,
  lexical diversity, hedging, interiority, sensory balance, passive ratio) in
  five languages; chapter-level drift surfaces as informational findings.
- **Terminology governance** — a **Glossary** book of canonical terms + banned
  synonyms; the editor red-underlines drift (`Ctrl+V z`), `inkhaven terms check`
  is CI-ready.
- **Bibliography & citations** — references as HJSON in a **Sources** book,
  `@key` citations, compiled to `sources.bib` at assembly (`inkhaven sources`).
- Findings land in a filterable, navigable **Output pane** (`Ctrl+B Tab`).

### Storage and backup
- DuckDB metadata + DuckDB blobs + HNSW semantic vectors.  No
  inverted full-text index — search is semantic-only via
  embedded vectors.
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
- `prose profile` / `drift` / `suggest` — deterministic narrative-voice
  metrics (NARR-1; zero-AI, five languages).
- `snippets` / `terms` / `sources` — reusable-block, glossary, and
  bibliography checks (CI-ready; exit non-zero on a problem).
- `inner-socrates` / `inner-editor` / `realworld` — the companions from the
  shell; `check` runs the fast deterministic pass over the project.
- `import-epub` / `import-scrivener` — bring an existing manuscript in.
- `backup` / `restore` — see above.
- `ai "prompt"` — one-shot inference from the shell (no TUI).
- `inkhaven <cmd> --help` for the full surface; most checks take `--json`.

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

### 3. `cargo install inkhaven` (compile from crates.io)

```bash
cargo install inkhaven
```

Inkhaven is published on crates.io — every release tag pushes a
new version (latest: 3.4.0).  The first build takes ~10 minutes on
a modern laptop because of DuckDB + fastembed + ONNX-runtime
compilation; `cargo binstall` above is the fast path.

### 4. `cargo install --git` (compile from a specific tag)

```bash
cargo install --git https://github.com/vulogov/blackInkhaven --tag v3.4.0
```

Useful when you want a specific tag, a pre-release branch, or a
local fork.

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
- [`Documentation/WORLDBUILDING.md`](Documentation/WORLDBUILDING.md) — the
  `realworld` world simulator and fact-checker (RFC WORLD-4).
- [`Documentation/INNER_SOCRATES.md`](Documentation/INNER_SOCRATES.md) — the
  Socratic interrogator for examined authorship (RFC INNER_SOCRATES-1).
- [`Documentation/PROSE_VOICE.md`](Documentation/PROSE_VOICE.md) — deterministic
  narrative-voice profiling (`inkhaven prose`, RFC NARR-1).
- [`Documentation/JINJA_TEMPLATES.md`](Documentation/JINJA_TEMPLATES.md) — Jinja
  template paragraphs (RFC STRUCT-1).
- [`Documentation/STRUCTURAL_PARAGRAPHS.md`](Documentation/STRUCTURAL_PARAGRAPHS.md)
  — structural paragraph subtypes + deletion hardening (RFC STRUCT-2).
- [`Documentation/OUTPUT_PANE.md`](Documentation/OUTPUT_PANE.md) — the Output
  message channel: findings, filters, navigation.

## Built with

- [duckdb](https://duckdb.org/) — metadata + blob persistence
- [vecstore](https://crates.io/crates/vecstore) — HNSW semantic
  vector store
- [fastembed](https://github.com/Anush008/fastembed-rs) —
  embedding model (search is semantic-only; no inverted full-
  text index)
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

## Security Warning and Disclaimer

Inkhaven is provided **"AS IS"** with no warranty.  The author
cannot be held liable for personal, business, or financial
damage arising from its use.  Use is voluntary and **at your
own risk**.

Before opening a project file you did not author — and before
relying on Inkhaven for work you cannot afford to lose —
please read **[`Documentation/SECURITY_WARNING.md`](Documentation/SECURITY_WARNING.md)**.
It enumerates the security issues catalogued in the 1.2.15
audit (both fixed and pending), the design properties that
are inherent rather than bugs, the unknown-risk classes that
no audit can fully eliminate, and the limitation-of-liability
terms under which Inkhaven is distributed.
