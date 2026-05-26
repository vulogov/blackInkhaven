# Configuration

Every Inkhaven project carries its own configuration file:
`<project-root>/inkhaven.hjson`. It is written verbatim by `inkhaven init`
from the template that ships with the binary (`assets/default_project.hjson`)
and is hot-reloadable per-session — change a value and restart the TUI to
see it pick up.

Inkhaven uses [HJSON](https://hjson.github.io/), a strict-JSON superset that
allows comments, unquoted keys, optional commas, and multiline strings.
Examples in this document are real HJSON syntax that you can paste straight
into your file.

## Table of contents

- [How the config is read](#how-the-config-is-read)
- [Top-level fields](#top-level-fields)
- [`embeddings`](#embeddings)
- [`llm`](#llm)
- [`editor`](#editor)
- [`theme`](#theme)
- [`hierarchy`](#hierarchy)
- [`keys`](#keys)
- [`backup`](#backup)
- [`prompts_file` and `language`](#prompts_file-and-language)
- [`typst_compile`](#typst_compile)
- [`output`](#output)
- [`goals`](#goals)
- [`sync_interval_seconds`](#sync_interval_seconds)
- [Migration and forward compatibility](#migration-and-forward-compatibility)

## How the config is read

- The TUI loads `inkhaven.hjson` once on startup and clones the parsed
  result so every subsystem (editor, AI client, theme renderer, backup
  hook) reads it independently.
- Every field is `#[serde(default)]`. Missing fields silently fall back to
  the compiled-in default, so a config written by an older release keeps
  working when new fields are added.
- Unknown fields are ignored. A typo (`heigth: 24`) does not crash the
  loader, but the value has no effect — check `KEYBINDING.md` and this
  document for the canonical names.

You can validate a config without launching the TUI:

```bash
inkhaven --project ~/Books/my-novel list >/dev/null
```

If the config is malformed the CLI prints an error like
`inkhaven: config error: found a punctuator character when expecting a quoteless string` and exits.

## Top-level fields

```hjson
{
  language: english
  prompts_file: prompts.hjson
  sync_interval_seconds: 60

  embeddings: { … }
  llm: { … }
  editor: { … }
  theme: { … }
  hierarchy: { … }
  keys: { … }
  backup: { … }
}
```

## `embeddings`

Controls how paragraph bodies are converted into vectors for semantic
search. Inkhaven uses [fastembed](https://github.com/Anush008/fastembed-rs)
under the hood.

```hjson
embeddings: {
  model: MultilingualE5Small
  chunk_size: 800
  chunk_overlap: 0.15
}
```

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `model` | string | `MultilingualE5Small` | Which fastembed model to download / use. Pick a multilingual one (E5) if you write in any non-English language. |
| `chunk_size` | int | `800` | Approximate characters per chunk fed to the embedder. Larger chunks → more context but coarser similarity. |
| `chunk_overlap` | float | `0.15` | Overlap fraction between adjacent chunks. `0.15` = 15 % overlap, smoothing chunk boundaries. |

Supported model names:

- `MultilingualE5Small` (default) — 384-dim, ~120 MB, fast, good
  multilingual recall including Russian
- `MultilingualE5Base` — 768-dim, ~300 MB, higher quality
- `MultilingualE5Large` — 1024-dim, ~1.1 GB, best multilingual quality
- `BGEM3` — 1024-dim, multilingual, strong English performance
- `BGESmallENV15`, `BGEBaseENV15`, `BGELargeENV15` — English-only,
  smaller binaries

Changing the model triggers a one-time download on next start (the
existing index is rebuilt next time you save a paragraph). If you switch
models you should run `inkhaven reindex` so the new embedder reprocesses
your prose.

## `llm`

Lists AI providers and picks one as the default.

```hjson
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
    ollama: {
      model: llama3.2
    }
  }
}
```

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `default` | string | `gemini` | Which entry in `providers` is used when no `--provider` flag is passed (CLI) and no override is hard-coded (TUI). |
| `providers.<name>.model` | string | varies | Model identifier passed to [genai](https://github.com/jeremychone/rust-genai). genai picks the adapter (Gemini / OpenAI / Anthropic / Ollama / …) from this string. |
| `providers.<name>.api_key_env` | string \| absent | varies | Environment variable that holds the API key. **Omit entirely** for local providers like Ollama. |

If `api_key_env` is set and that env var is unset at runtime, Inkhaven
refuses to spawn the inference with a clean status message — no crash,
no half-formed request.

To add an OpenAI provider:

```hjson
openai: {
  model: gpt-4.1-mini
  api_key_env: OPENAI_API_KEY
}
```

To add an Anthropic provider:

```hjson
claude: {
  model: claude-3-7-sonnet-latest
  api_key_env: ANTHROPIC_API_KEY
}
```

Switching the default is one edit: `default: claude` and you're done.

## `editor`

Controls the editor pane's behaviour. The visual look lives in
[`theme`](#theme).

```hjson
editor: {
  theme: default
  tab_width: 2
  wrap: true
  autosave_seconds: 5
  stemming: {
    languages: [
      "english"
      "russian"
    ]
  }
}
```

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `theme` | string | `default` | Reserved; the visual theme is configured under top-level `theme`. |
| `tab_width` | int | `2` | Currently informational — tui-textarea inserts a literal `\t`. |
| `wrap` | bool | `true` | Soft word-wrap inside the editor. `false` → horizontal scroll on long lines. |
| `autosave_seconds` | int | `5` | Seconds of editor inactivity after which a dirty paragraph is auto-saved. `0` disables idle autosave (Ctrl+S, paragraph-switch and quit-time autosaves still fire). Suspended while a grammar-correction highlight is active. |
| `startup_splash` | bool | `true` | 1.2.4+. Show a 7-second floating splash at launch with today's words / active minutes / streak / project shape. Any key dismisses early. Set `false` to skip. |
| `mouse_captured` | bool | `true` | 1.2.8+. Initial mouse-capture state on launch. `true` hands every mouse event to the TUI (click-to-focus, scroll-wheel per pane, in-TUI drag-select). `false` releases capture at startup so the terminal's native drag-select + system-clipboard copy (Cmd/Ctrl+Shift+C) work without pressing `Ctrl+Shift+M` first. The runtime toggle still flips state regardless. |
| `confirm_quit`   | bool | `false` | 1.2.8+. Pop a confirmation modal when the user presses `Ctrl+Q`. `Y` / `Enter` confirms and quits (with the usual autosave-first behaviour); `N` / `Esc` cancels. Useful when `Ctrl+Q` triggers terminal software flow-control or when the chord lands by accident. Default `false` — `Ctrl+Q` quits immediately as it always has. Ctrl+Q inside an already-open modal still quits unconditionally (intended as an escape hatch). |
| `stemming.languages` | list of strings | `["english", "russian"]` | **Legacy** — superseded by top-level `language` when that is non-empty. See [`language`](#prompts_file-and-language). |

The grammar-correction-highlight interaction: while you have an active
`g`-apply diff visible, idle autosave is suspended so the red overlay
doesn't disappear under you. Manual save (Ctrl+S) or leaving the editor
pane (focus loss) explicitly clears the overlay and resumes the normal
autosave cadence.

## `theme`

Every colour Inkhaven uses is configurable through this block. Values
are RGB hex strings (`#RRGGBB`) or the short `#RGB` form. Empty string
or an unparseable value falls back to the baked-in default.

The shipping defaults are
[Catppuccin Mocha](https://catppuccin.com/palette/) — a dark, balanced
palette tested on plenty of terminals.

```hjson
theme: {
  // Pane chrome
  pane_bg:           "#1e1e2e"
  pane_fg:           "#cdd6f4"
  line_number_fg:    "#6c7086"
  current_line_bg:   "#313244"

  // Borders
  border_focused:    "#cba6f7"
  border_unfocused:  "#45475a"
  border_dirty:      "#f9e2af"
  border_saved:      "#a6e3a1"
  border_readonly:   "#94e2d5"

  // Floating windows
  modal_bg:          "#181825"
  modal_fg:          "#cdd6f4"
  modal_border:      "#cba6f7"

  // Lexicon overlay
  places_fg:         "#89dceb"
  characters_fg:     "#f9e2af"

  // In-buffer search
  search_match_bg:   "#f38ba8"
  search_current_bg: "#f5c2e7"

  // Tree pane chrome
  tree_open_marker:  "#a6e3a1"
  tree_book_fg:      "#f5c2e7"
  tree_chapter_fg:   "#89b4fa"
  tree_subchapter_fg:"#94e2d5"
  tree_paragraph_fg: "#cdd6f4"

  // Editor header
  editor_position_fg:"#89dceb"

  // AI header chips
  ai_scope_fg:       "#fab387"
  ai_infer_fg:       "#94e2d5"

  // Grammar-check change overlay
  grammar_change_fg: "#f38ba8"

  // Typst syntax
  syntax_heading:    "#cba6f7"
  syntax_bold:       "#f9e2af"
  syntax_italic:     "#94e2d5"
  syntax_string:     "#a6e3a1"
  syntax_number:     "#fab387"
  syntax_comment:    "#6c7086"
  syntax_keyword:    "#cba6f7"
  syntax_function:   "#89dceb"
  syntax_operator:   "#94e2d5"
  syntax_list_marker:"#cba6f7"
  syntax_raw:        "#fab387"
  syntax_tag:        "#89b4fa"
  syntax_quote:      "#9399b2"
}
```

### Pane chrome

| Field | What it paints |
| ----- | -------------- |
| `pane_bg` | The background fill of every pane (Tree, Editor, AI, Search, AI prompt). |
| `pane_fg` | Default foreground inside panes. |
| `line_number_fg` | The dim gutter to the left of editor text. |
| `current_line_bg` | The horizontal stripe behind the cursor's line in the editor. |

### Borders

`border_focused` and `border_unfocused` apply to every non-editor pane.
The editor swaps in `border_saved` (green), `border_dirty` (yellow), or
`border_readonly` (teal) **only while focused** so the buffer state is
glanceable.

### Floating windows

Every modal (Add / Delete / Rename / FindReplace / QuickRef /
FilePicker / Help / PromptPicker / SnapshotPicker) shares `modal_bg`,
`modal_fg`, and `modal_border`.

### Lexicon overlay (Places / Characters)

`places_fg` colours any token in the editor that matches a paragraph
title in the **Places** system book; `characters_fg` does the same for
**Characters**. Stemming is applied per the project `language`, so a
Russian project's place "Москва" lights up "Москвы", "Москве", and so on
automatically. See [`LOCATIONS.md`](LOCATIONS.md) and
[`CHARACTERS.md`](CHARACTERS.md).

### In-buffer search (Ctrl+F)

`search_match_bg` paints every match; `search_current_bg` highlights the
one the cursor is sitting on (Ctrl+X advances). Both apply on top of the
syntax colour, so the underlying text stays readable.

### Tree pane

`tree_open_marker` is the colour of the ▸ glyph that flags the
currently-loaded paragraph. The four per-kind colours
(`tree_book_fg`, `tree_chapter_fg`, `tree_subchapter_fg`,
`tree_paragraph_fg`) drive each row's title colour; books and chapters
also get bold so the upper hierarchy has visual weight.

### Editor header chip

`editor_position_fg` colours the trailing `L… C…` cursor read-out in the
Editor pane's title.

### AI header chips

`ai_scope_fg` is the F9 scope chip; `ai_infer_fg` is the F10 inference
mode chip. The chips are always shown (`infer=…` is always visible so an
accidentally-armed Local mode is obvious; `scope=…` appears only when
non-None).

### Grammar-check overlay

`grammar_change_fg` colours every character that differs from the
pre-correction baseline after a `g`-apply in the AI pane. Persists until
save, paragraph switch, or `Ctrl+B C`.

### Typst syntax

The thirteen `syntax_*` fields drive the editor's tree-sitter-based Typst
highlighter. Adjust them to match an external colour scheme you like.

## `hierarchy`

```hjson
hierarchy: {
  unbounded_subchapters: false
}
```

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `unbounded_subchapters` | bool | `false` | When `false` the hierarchy is exactly **Book → Chapter → Subchapter → Paragraph**. When `true`, subchapters may nest under subchapters arbitrarily — useful for legal documents, deeply structured manuals, etc. |

## `keys`

Several global chords are configurable. Everything else is hard-coded.

```hjson
keys: {
  save:             Ctrl+s
  search:           Ctrl+/
  ai_prompt:        Ctrl+i
  next_pane:        Tab
  prev_pane:        Shift+Tab
  page_up:          PageUp
  page_down:        PageDown
  meta_prefix:      Ctrl+b
  bund_prefix:      Ctrl+z
  view_prefix:      Ctrl+v
  bindings:         []
}
```

| Field | Default | What it does |
| ----- | ------- | ------------ |
| `save`        | `Ctrl+s`     | Save current paragraph. |
| `search`      | `Ctrl+/`     | Focus the top Search bar. |
| `ai_prompt`   | `Ctrl+i`     | Focus the bottom AI prompt bar. |
| `next_pane`   | `Tab`        | Cycle focus Tree → Editor → AI. |
| `prev_pane`   | `Shift+Tab`  | Cycle in reverse. |
| `page_up`     | `PageUp`     | PageUp (used in Tree + Editor; configurable for users on terminals that re-encode it). |
| `page_down`   | `PageDown`   | PageDown. |
| `meta_prefix` | `Ctrl+b`     | The Meta prefix chord. The action table is pane-specific — see [`KEYBINDING.md`](KEYBINDING.md) §1.1. |
| `bund_prefix` | `Ctrl+z`     | The Bund prefix chord (1.2+). |
| `view_prefix` | `Ctrl+v`     | The View prefix chord (1.2.4+) — markdown export, similar-paragraph mode, progress modal, paragraph links, bookmarks, fuzzy picker. |
| `bindings`    | `[]`         | User overlay rebinding sub-chords. Supports `layer: "meta_sub" | "bund_sub" | "view_sub" | "top_level"`. See [`KEYS_REASSIGNMENT.md`](KEYS_REASSIGNMENT.md). |

If your terminal multiplexer eats `Ctrl+B` (tmux uses it as the default
prefix), set `meta_prefix: Ctrl+g` or `Ctrl+;` or similar.

Chord syntax accepts:

- modifier prefixes: `Ctrl+`, `Shift+`, `Alt+`
- bare key names: `Tab`, `Enter`, `Esc`, `PageUp`, `PageDown`, `Home`,
  `End`, `Up`, `Down`, `Left`, `Right`, `Backspace`, `Delete`
- function keys: `F1` … `F12`
- printable characters: literal letter / digit / symbol

Multiple modifiers stack (`Ctrl+Shift+m`).

## `backup`

Drives the `inkhaven backup` CLI and the TUI's auto-backup-on-exit hook.

```hjson
backup: {
  out_dir: "backups"
  max_age: "7d"
  wait_for_key_after_backup: true
}
```

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `out_dir` | string | `"backups"` | Where `.zip` snapshots land. Relative paths resolve against the project root; absolute paths are used as-is. Created if missing. Empty string disables auto-backup. |
| `max_age` | [humantime](https://docs.rs/humantime) duration | `"7d"` | Maximum age of the last successful backup before the TUI's exit hook creates a fresh one. Values like `"24h"`, `"12h"`, `"30m"`, `"1w"` all work. `"0s"` disables auto-backup but keeps the manual `inkhaven backup` command active. |
| `wait_for_key_after_backup` | bool | `true` | 1.2.6+. When a backup finishes — either the manual `Ctrl+B Shift+B` chord or the exit-hook auto-backup — hold the splash on screen with a `Press any key to continue…` prompt so the user can read the destination path before the TUI dismisses it. Set `false` to keep the 1.2.5 auto-dismiss behaviour. |

`Ctrl+B Shift+B` (1.2.6+) triggers a manual backup that bypasses
the `max_age` cooldown — the splash always fires, the archive is
always written.

When the on-exit hook fires you see a splash:

```
┌── Inkhaven · backup ──────────────────┐
│  Performing database backup…          │
│  Project: /home/you/Books/my-novel    │
│  [████████····]  321/512 ( 63%)       │
└───────────────────────────────────────┘
```

The store handle is dropped before the zip runs so DuckDB / HNSW have
checkpointed to disk and the archive captures a consistent snapshot.

See [`MAINTENANCE.md`](MAINTENANCE.md) for backup / restore commands.

## `prompts_file` and `language`

```hjson
prompts_file: prompts.hjson
language: english
```

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `prompts_file` | string | `"prompts.hjson"` | Path to the prompt library (resolved against the project root). See [`PROMPTS.md`](PROMPTS.md). |
| `language` | string | `"english"` | Primary writing language. Drives Snowball stemmers for the Places / Characters highlight overlay AND the default F7 grammar-check prompt. Accepts: `arabic, danish, dutch, english, finnish, french, german, greek, hungarian, italian, norwegian, portuguese, romanian, russian, spanish, swedish, tamil, turkish`. Empty string falls back to `editor.stemming.languages`. |

To write a Russian-language novel:

```hjson
language: russian
```

To write multilingual content where the stemmer should know about more
than one language:

```hjson
language: ""
editor: {
  stemming: { languages: ["english", "russian"] }
}
```

## `typst_compile`

Controls `Ctrl+B B` / `Ctrl+B O` ("compile / take the book") and the
typst-as-library knobs introduced in 1.2.5. Both engines ship in
every 1.2.5+ build; the user picks at runtime via the `engine`
field below.

```hjson
typst_compile: {
  engine:                   "external"   // "external" | "inprocess"
  diagnostics:              true         // typst-syntax parse errors on idle/save
  diagnostics_idle_seconds: 2            // debounce for the idle recheck
  semantic_diagnostics:     false        // upgrade idle check to full typst::compile
  bundle_fonts:             true         // ship CM + Linux Libertine in the binary
  use_system_fonts:         true         // also search system fonts
  packages_enabled:         true         // fetch @preview/<pkg> from packages.typst.org
  wait_for_key_after_compile: true       // hold splash after compile finishes
  error_system_prompt:      ""           // override the AI compile-error prompt
}
```

| Field                       | Type   | Default      | Description |
| --------------------------- | ------ | ------------ | ----------- |
| `engine`                    | string | `"external"` | Picks the compiler driving `Ctrl+B B` / `Ctrl+B O`. `external` (default) shells out to the host's `typst` binary on PATH — exact 1.2.4 behaviour. `inprocess` runs `typst::compile + typst-pdf` inside the inkhaven process: no shell-out, no `typst` install required, structured diagnostics with span info. Compile happens on a worker thread so the TUI spinner stays animated. Both engines write the PDF to the same path. |
| `diagnostics`               | bool   | `true`       | When true, run `typst-syntax` against the open paragraph on save and on idle (`diagnostics_idle_seconds`). Parse errors land on the status bar as `typst: line L:C — <message>`. Pure parser — no eval / layout / render, no font setup, no package resolution. Bund and HJSON content types are skipped automatically. Set `false` to suppress entirely. |
| `diagnostics_idle_seconds`  | int    | `2`          | Minimum seconds of editor idle before the typst recheck runs. `0` is allowed (every tick); large values approach "only on save". Piggy-backs on the same idle clock as `editor.autosave_seconds`. |
| `semantic_diagnostics`      | bool   | `false`      | When **true** AND `engine = "inprocess"`, run a full `typst::compile` against the open paragraph in isolation after the parser passes cleanly. Catches semantic errors (undefined functions, type errors, font-not-found) the parser can't see. **False positives are expected** when the paragraph references book-level definitions — the isolated compile doesn't see the assembled preamble. Costs ~20–200 ms per check on warm caches. Has no effect with `engine = "external"`. |
| `bundle_fonts`              | bool   | `true`       | 1.2.5+. Ship Computer Modern and Linux Libertine inside the inkhaven binary so the in-process engine can lay out even on hosts without system fonts. Adds ~10 MB. Set `false` if every host inkhaven runs on already has the fonts your manuscript needs. No effect when `engine = "external"`. |
| `use_system_fonts`          | bool   | `true`       | 1.2.5+. Also search the host's system fonts via fontdb. Combined with `bundle_fonts: true` (the default), you get both. Turn off for reproducible builds where the only allowed fonts are the embedded ones. No effect when `engine = "external"`. |
| `packages_enabled`          | bool   | `true`       | 1.2.5+. When the in-process engine sees `@preview/<pkg>` (or any non-local package id), fetch and unpack it from `packages.typst.org` via `typst-kit`'s package storage. Cached on disk in the platform's standard cache dir (`~/Library/Caches/typst/packages` on macOS, `~/.cache/typst/packages` on Linux, `%LOCALAPPDATA%\typst\packages` on Windows). Set `false` to fail-fast on package imports — useful for hermetic / offline builds. No effect when `engine = "external"`. |
| `wait_for_key_after_compile` | bool   | `true`       | 1.2.6+. When the Ctrl+B B / Ctrl+B O typst-compile splash finishes, hold it on screen with a `Press any key to continue…` prompt so the user can read the "Build complete." / "Build failed." line before control returns to the editor. Cancelled compiles (Esc) skip the wait. Set `false` to auto-dismiss as in 1.2.5. |
| `error_system_prompt`       | string | `""`         | Override the AI system prompt used when `typst compile` returns non-zero. Empty falls back to the baked-in default. |

The diagnostics path is entirely additive — turning it off
restores the exact 1.2.4 behaviour. `engine: "inprocess"` is the
single switch that lights up the in-process compiler; nothing
else needs to change. At TUI startup an `info!` line records
which engine is active so you can confirm the setting took
effect.

**`inprocess` properties in 1.2.5:**

- `@preview/<pkg>` imports work out of the box via the package
  downloader. First fetch of a package is online; subsequent
  uses hit the on-disk cache. Set `packages_enabled: false` to
  fail-fast on package imports (hermetic builds).
- Fonts are bundled (Computer Modern + Linux Libertine) AND the
  host's system fonts are searched. Either source can be
  disabled independently via `bundle_fonts` /
  `use_system_fonts`.
- The PDF bytes match what `typst compile` of the same version
  produces; if you mix `external` and `inprocess` across runs,
  pin the host's `typst` binary to the same release (`0.14.x`
  for 1.2.5) so the output stays byte-identical.

**TUI integration (1.2.5+):**

- **Splash + interrupt** — Ctrl+B B / Ctrl+B O paint a centered
  splash with the spinner, the book title, the active engine
  (e.g. `internal · fonts: bundled + system · @preview: on`
  *or* `external · /usr/local/bin/typst`), elapsed seconds, and
  a footer hint. **Esc** in the splash cancels the compile:
  external engine receives SIGTERM, in-process worker is
  abandoned (it keeps running until typst finishes naturally;
  the foreground unblocks immediately).
- **Autosave before A/B/O** — Ctrl+B A (assemble), Ctrl+B B
  (build), and Ctrl+B O (take) all flush the primary editor
  (and the secondary editor in similar-paragraph mode) to disk
  before the assembler walks `.typ` files. No more "I just
  pressed Ctrl+B B and the build used yesterday's saved
  version".
- **Engine visibility** — Ctrl+B V (credits / version pane)
  carries a `Typst engine` line with the same summary the
  splash uses; the engine identity is also logged at INFO at
  TUI startup so the choice shows up in `inkhaven.log`.

## `output`

Multi-format export hookup for `Ctrl+B O` ("take the book"). Each
format in `extra_formats` is generated alongside the PDF using the
same combined `.typ` source the PDF compile sees.

```hjson
output: {
  // Case-insensitive: "markdown", "tex", "epub" supported in 1.2.3.
  // Unknown entries log a WARN and are skipped. Per-format errors
  // land on the status bar but never abort the take — the PDF is
  // already on disk before extras run.
  extra_formats: ["markdown", "tex"]
}
```

| Field            | Type           | Default | Description |
| ---------------- | -------------- | ------- | ----------- |
| `extra_formats`  | `["str", …]`   | `[]`    | Additional formats produced alongside the PDF on every `Ctrl+B O`. Files land next to the PDF with the same stem (`story-YYYYDDMM-HHMM.md`, …). Empty list = PDF-only, same as 1.2.2. |

The CLI `inkhaven export <fmt>` ignores this list — it picks one
format explicitly. See tutorial
[`15-multi-format-export.md`](Tutorials/15-multi-format-export.md).

## `goals`

Writing-progress goals — fuels the status-bar widget (today /
streak / per-book pace) and the Ctrl+V G progress modal
(sparkline, status-ladder bar, deadline forecasting). All fields
are optional; commenting them out / zero / empty disables that
particular goal but still records events so the modal has
history to show.

```hjson
goals: {
  daily_words: 1500
  active_minutes_daily: 60
  streak_grace_per_week: 1
  auto_promote_on_target: true
  books: {
    story: { target_words: 80000, deadline: "2026-12-31" }
  }
  status_ladder: {
    ready: 1
    final: 3
  }
}
```

| Field                    | Type            | Default | Description |
| ------------------------ | --------------- | ------- | ----------- |
| `daily_words`            | int             | `0`     | Project-wide daily target. Status-bar shows `today N/M words` when non-zero. |
| `active_minutes_daily`   | int             | `0`     | 1.2.4+. Daily active-time target. Active time sums save→save gaps capped at 5 minutes per gap (AFK breaks don't count). Status-bar shows `45m / 60m` when non-zero. |
| `streak_grace_per_week`  | int             | `0`     | Missed days forgiven inside a rolling 7-day window before the streak breaks. `0` = strict, `1` = one rest day allowed per week. |
| `books`                  | map<slug, BookGoal> | `{}` | Per-book targets, keyed by **book slug** (matches `Node.slug`, case-insensitive). |
| `books.<slug>.target_words` | int          | `0`     | Total words the book should reach. `0` hides the per-book pace line. |
| `books.<slug>.deadline`  | str (`YYYY-MM-DD`) | `""` | Date by which `target_words` should be hit. Empty disables deadline pacing. Past-due deadlines collapse to "remaining gap, all at once". |
| `status_ladder`          | map<status, int> | `{}` | Trailing-7-days promotion targets keyed by status name **lowercased** (`ready`, `final`, `third`, `second`, `first`, `napkin`). Modal shows `→ ready: N/M this week`. |
| `auto_promote_on_target` | bool             | `true` | 1.2.4+. When a save crosses a paragraph's `target_words` (set via `Ctrl+V T` or `ink.paragraph.set_target`), advance its status one ladder rung. Idempotent per `(paragraph, status)`; a manual `Ctrl+B R` resets the bookkeeping. Set `false` to keep promotions manual. |

**Today's words** = current total − today's morning baseline.
The baseline is captured per UTC day on project open (idempotent
per day). System books (Help / Scripts / Typst / Prompts / Places
/ Characters / Notes / Artefacts / Research) are excluded from
every aggregate — only user-book manuscript words count.

See tutorial [`17-writing-goals.md`](Tutorials/17-writing-goals.md)
for the full workflow including streak grace examples and pace
forecasting.

## `sync_interval_seconds`

```hjson
sync_interval_seconds: 60
```

| Type | Default | Description |
| ---- | ------- | ----------- |
| int | `60` | Seconds between background calls to `Store::sync()` — flushes the HNSW vector index and checkpoints DuckDB. `0` disables the background timer; saves still trigger sync explicitly. |

You rarely need to touch this. The default is conservative.

## Migration and forward compatibility

- Every field is `#[serde(default)]`. Old configs work with new releases
  out of the box.
- When a field becomes obsolete it remains parseable (silently ignored)
  so downgrading also doesn't break.
- Inkhaven never edits your `inkhaven.hjson` in place. New fields are
  exposed via the documented defaults; you opt in by adding them
  yourself, copying from `assets/default_project.hjson` (or this file).
- To reset the config to shipping defaults: rename the existing
  `inkhaven.hjson`, run `inkhaven init --force` against the same
  project, then re-merge any customisations.

Full annotated template lives at
[`assets/default_project.hjson`](../assets/default_project.hjson) — that
is the same file `inkhaven init` writes verbatim.

---

## 1.2.6 — new HJSON blocks

Two new top-level stanzas land in the 1.2.6 cycle. Both
are opt-in; existing projects upgrade transparently.

### `ai` (1.2.6+) — AI-pane behaviour

```hjson
ai: {
  // 1.2.6+ — record (user, assistant) turns onto the open
  // paragraph's `Node.ai_memory` when Paragraph-scope
  // prompts fire. Subsequent Paragraph-scope prompts
  // pre-pend that memory to the chat-history payload.
  // Visible session chat history is untouched.
  per_paragraph_memory:           false

  // Max total turns (user + assistant) kept per paragraph.
  // Oldest pair evicts first when length exceeds the cap.
  // 0 = disabled regardless of `per_paragraph_memory`.
  per_paragraph_memory_max_turns: 10

  // 1.2.6+ — route `r` (Replace) and `g` (ReplaceCorrected)
  // through a side-by-side diff modal before any bytes
  // change. `a` accepts; `r` rejects; `e` is an alias for
  // `a`. Set false to revert to the pre-1.2.6 immediate
  // apply.
  diff_review_on_apply:           true

  // 1.2.6+ — re-seed the Prompts book on `inkhaven init`
  // AND on every TUI open with the five embedded prompt
  // .example seeds. Idempotent — paragraphs with the same
  // title are skipped.
  reseed_prompt_examples:         true
}
```

All four fields are `#[serde(default)]`; missing block →
default values. The implementation in
`crate::config::AiConfig` carries the canonical types.

### `timeline` (1.2.6+) — story timeline

```hjson
timeline: {
  // Master switch. When false, every timeline chord, CLI
  // subcommand, and Bund word lands a "feature disabled"
  // hint instead of running. Off by default so existing
  // projects upgrade transparently.
  enabled: false

  // Default track label used when an event's `track`
  // field is None. Shown in the swim-lane row header.
  default_track: "main"

  // Calendar configuration. Three preset shapes; `custom`
  // for everything else.
  calendar: {
    // "gregorian" | "sols" | "custom"
    preset: "custom"

    // Name of the base unit (one tick == one of these).
    base_unit: "day"

    // Unit stack, base-first. Each entry's `per_parent`
    // says how many of THIS unit make one of the next
    // (parent) unit. The first entry's per_parent is
    // ignored. `names` is optional — when empty the
    // formatter falls back to numeric.
    units: [
      { name: "day", names: [] }
      { name: "month", per_parent: 30,
        names: ["Frostmoon", "Snowfall", "Greenstart",
                "Bloomtide", "Highsun", "Goldfall",
                "Mistwane", "Stormrise", "Coldgate",
                "Longnight", "Hearthlit", "Yearfall"] }
      { name: "year", per_parent: 12, names: [] }
    ]

    // Seasons (used by Precision::Season fuzz windows).
    seasons: [
      { name: "winter", start_month: 1, span_months: 3 }
      { name: "spring", start_month: 4, span_months: 3 }
      { name: "summer", start_month: 7, span_months: 3 }
      { name: "autumn", start_month: 10, span_months: 3 }
    ]

    // Epoch label appended to positive years.
    epoch_label:        "A"
    // Epoch label for negative years (prequels).
    epoch_before_label: "BA"

    // Format string used by `Calendar::format()`. Tokens:
    //   {year}, {epoch_label}, {epoch_before_label},
    //   {month}, {month-name}, {day}, {hour}
    display_format: "{year}{epoch_label}.{month}.{day}"

    // Optional landmark aliases the parser recognises.
    parse_aliases: [
      { match: "Founding", ticks: 0 }
    ]
  }

  // Swim-lane display knobs.
  display: {
    show_orphans:        true   # synthetic orphan row at the
                                # bottom of the swim lane
    swim_lane_max_rows:  12     # truncate beyond this with a
                                # "+N more" row
    default_zoom:        1.0    # initial ticks-per-cell
  }
}
```

#### Calendar preset shortcuts

`preset: "sols"` expands to a single-unit calendar with
`day` as the only unit, `Sol` as the epoch label, and
`"Sol {day}"` as the format string. Useful for
"days since day zero" timelines (Mars colony stories,
generation ships, anything where the year isn't a useful
unit).

`preset: "gregorian"` expands to a Year / Month / Day
stack with English month names and 30-day months
(approximate — calendars don't model leap years; the
ticks are absolute). Useful for real-world dates.

`preset: "custom"` honours every field above verbatim.

### `inkhaven.hjson` recap (1.2.6 cycle adds)

```hjson
{
  ai: {
    per_paragraph_memory:           false
    per_paragraph_memory_max_turns: 10
    diff_review_on_apply:           true
    reseed_prompt_examples:         true
  }

  timeline: {
    enabled: false
    default_track: "main"
    calendar: { preset: "gregorian" }
    display: {
      show_orphans:       true
      swim_lane_max_rows: 12
      default_zoom:       1.0
    }
  }
}
```

Both stanzas are additive. Removing them restores the
pre-1.2.6 behaviour exactly.

## 1.2.8 — new HJSON blocks

### `scrivener` (1.2.8+) — Scrivener-importer behaviour

```hjson
scrivener: {
  // List of CustomMeta field names (case-insensitive) that
  // `inkhaven import-scrivener` interprets as event dates.
  // For each matching field on an imported paragraph, the
  // value is fed through the project's HJSON-configured
  // calendar; a successful parse attaches `EventData` to the
  // resulting node (event landed at the parsed start tick,
  // no end, no track override).  Bad values are not fatal —
  // they land on the report's error list with the source
  // field name + raw value.
  //
  // Defaults cover the most common English-language
  // Scrivener templates ("Date" / "Story Date" / "Event
  // Date").  Non-English templates customise this list.
  date_fields: ["Date", "Story Date", "Event Date"]
}
```

The pass is gated on `timeline.enabled = true` — Scrivener
date import is a no-op when the project hasn't opted into
the timeline feature, even if the .scriv file carries
CustomMeta dates.  Scrivener field IDs in
`<CustomMetaDataSettings>` are resolved against the
project-level registry; unknown IDs (referenced by an item
but missing from the registry) are silently skipped.

### `editor.mouse_captured` (1.2.8+)

Already documented inline in the `editor` table above.
Sets the initial mouse-capture state on launch; runtime
`Ctrl+Shift+M` still flips it regardless.

### `shell` (1.2.8+) — embedded nushell pane

```hjson
shell: {
  // Whether `Ctrl+Z o` opens the embedded shell pane.
  // Set false to make the chord a status-hint no-op
  // (the engine + nu deps stay linked into the binary
  // either way — saving binary size requires a custom
  // cargo build with --no-default-features once we add
  // a feature flag, currently not gated).
  enabled: true

  // In-memory cap on (command, output) turn pairs the
  // pane retains across the session.  Older pairs roll
  // off the front.  The SQLite history at
  // `.inkhaven/shell_history.db` is uncapped — this
  // bounds working memory + seeds the Up-arrow recall
  // ring on first open of each session.
  max_buffered_turns: 50

  // Per-turn cap on the number of output lines retained
  // from a single command's stdout (and stderr separately).
  // A `cat /var/log/system.log` or `git log` can emit
  // tens of thousands of lines; without this cap they
  // bloat the in-memory ring and slow PgUp/PgDn scroll
  // rendering.  Excess tail is replaced with a
  // "… (N more lines truncated)" marker — output is
  // capped but never silently dropped.  Raise this if
  // you want to retain the full output of large commands
  // (cost: memory + render time grow linearly).
  max_output_lines: 1000

  // 1.2.8+ — basenames of external programs refused
  // before spawn.  Full-screen TUI apps (vim, less, top,
  // tmux, …) cannot run inside the embedded pane: they
  // open `/dev/tty` directly and write escape sequences
  // past the editor's piped stdio, corrupting ratatui's
  // alt-screen surface.  Match is case-insensitive
  // against the program's basename, so `^vim`,
  // `^/usr/bin/vim`, and `^VIM` all hit a `"vim"` entry.
  // The default list covers common editors, pagers,
  // monitors, multiplexers, remote shells, debuggers,
  // fuzzy finders, TTY-needing REPLs, DB clients, and
  // privileged binaries.  Override to add internal tools
  // or to *allow* something the default rejects:
  //   blocked_externals: ["less", "top", "vim"]   // shorter list
  //   blocked_externals: []                       // disable entirely
  blocked_externals: [
    "vim", "nvim", "vi", "view", "ex",
    "emacs", "emacsclient", "nano", "pico", "joe", "jed",
    "mc", "mcedit", "ranger", "nnn", "lf", "yazi",
    "less", "more", "most", "pg",
    "top", "htop", "btop", "atop", "iotop", "iftop", "nethogs", "glances",
    "tmux", "screen", "byobu", "dtach", "abduco",
    "ssh", "telnet", "mosh", "rlogin",
    "gdb", "lldb",
    "fzf", "peco", "sk", "skim",
    "ipython", "irb", "pry",
    "psql", "mysql", "sqlite3", "redis-cli",
    "sudo", "su", "passwd"
  ]

  // 1.2.8+ — wall-clock budget for a single command's
  // evaluation.  After this many seconds the watchdog
  // raises a nu interrupt; if the worker doesn't respond
  // within a 2-second grace window, the engine is
  // restarted (env vars + `def` declarations + `cd`
  // state are lost) and the user sees a friendly
  // explanation.  Set high (e.g. 600) if you legitimately
  // run long-baked pipelines like remote pulls; lower for
  // a tighter "this should be quick" SLA.
  external_timeout_secs: 30

  // Typst markup wrapping a `Ctrl+Z h` → `i` insert.
  // `{output}` is substituted verbatim — the default
  // uses a backtick-delimited typst raw block which
  // bounds the literal without escaping, so embedded
  // quotes / backslashes / pipes survive intact.
  insert_template: "#raw(block: true, lang: \"shell\", `{output}`)"
}
```

The embedded shell loads nushell's full default command
set (`ls`, `where`, `str`, `path`, `into`, …) and runs
in the same process as the editor — no subprocess, no
PTY.  Long-running TTY apps (`vim`, `top`, `less`) are
explicitly out of scope; use a separate terminal for
those.

Per-project history lives at
`<project>/.inkhaven/shell_history.db` (bundled SQLite,
no system dependency).  Survives TUI restart.

`Ctrl+Z O` (Shift) drops the engine + in-memory ring but
leaves the on-disk DB alone.  Full reset is manual:
`rm .inkhaven/shell_history.db` from another terminal.

See [`Tutorials/35-embedded-shell.md`](Tutorials/35-embedded-shell.md)
for the full chord ladder + use-case walkthrough.
