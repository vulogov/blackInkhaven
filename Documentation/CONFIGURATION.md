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
- [Global overrides](#global-overrides-1220)
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

## Global overrides (1.2.20+)

You don't have to repeat yourself across projects. After a project's
`inkhaven.hjson` is read, Inkhaven layers any **user-global override files**
on top, so a personal preference (your theme, your keybinds) applies to
*every* project without editing each project's config.

Precedence, lowest → highest:

1. compiled-in defaults
2. the project's `inkhaven.hjson`
3. `~/.config/inkhaven/config.hjson`
4. `~/.config/inkhaven/conf/*.hjson` — every `.hjson` file in that folder,
   in sorted (lexical) filename order, each overriding the previous

So a global file **wins over the project**. This is deliberate: `inkhaven
init` writes a *full* config, so if the project won you'd never see your
global colours. The override files are **partial** — put in only the keys
you want to change; everything else falls through to the project:

```hjson
// ~/.config/inkhaven/config.hjson — applies to all projects
{
  theme: {
    style_warning_echo_fg: "#7aa2f7"
    pane_fg: "#e0e0e0"
  }
}
```

Split overrides across `conf/` if you like — e.g.
`conf/10-theme.hjson`, `conf/20-keys.hjson` — and the higher-numbered file
wins on a conflict. The directory honours `$XDG_CONFIG_HOME` (falling back
to `~/.config`).

A malformed **global** file is skipped with a warning rather than breaking
every project — only a malformed **project** `inkhaven.hjson` is fatal. The
in-app config editor (`Ctrl+B A`) still edits the *project* file directly,
so what it shows is the raw project config, not the global-merged result.

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
  pool_size: 4
}
```

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `model` | string | `MultilingualE5Small` | Which fastembed model to download / use. Pick a multilingual one (E5) if you write in any non-English language. |
| `chunk_size` | int | `800` | Approximate characters per chunk fed to the embedder. Larger chunks → more context but coarser similarity. |
| `chunk_overlap` | float | `0.15` | Overlap fraction between adjacent chunks. `0.15` = 15 % overlap, smoothing chunk boundaries. |
| `pool_size` | int | `4` | 1.3.12+. r2d2 connection-pool size for each backing DuckDB file (metadata + content). **Clamped to a minimum of 2 at open time** so a background job (e.g. the deep AI refresh) can always check out a connection while the main thread holds another — a pool of 1 would deadlock. Raise it if you script heavy concurrent access; the default 4 is ample for the TUI + one background job. |

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
| `tts.enabled`    | bool   | `false`   | 1.2.9+. Master switch for the `Ctrl+B S` read-aloud feature. When `false` (default), pressing the chord opens a friendly explanation modal instead of speaking. Set `true` to enable; the OS TTS engine (`tts-rs`: AVFoundation on macOS, SAPI / WinRT on Windows, Speech Dispatcher on Linux) is lazily initialised on first invocation and cached for subsequent reads. |
| `tts.voice`      | string | `"Milena"` | 1.2.9+. Voice-name fragment. Case-insensitive substring match against installed voice names; the matcher prefers entries that also contain `Enhanced` or `Premium`, so `"Milena"` picks `Milena (Enhanced)` when the premium variant is installed. Default `Milena` is a Russian female voice that ships free with macOS (one-time download via System Settings → Accessibility → Spoken Content → Russian) and Windows (Settings → Time & language → Speech → Add a voice). Empty string falls back to the engine's system default voice. On Linux, the voice name depends on which engines speech-dispatcher is configured to use. |
| `tts.speed`      | f32    | `1.0`     | 1.2.9+. Speech rate as a multiplier over the engine's "normal" rate. `1.0` = normal, `0.8` = 80% (slower / more deliberate), `1.2` = 120% (faster). Clamped to the engine's `[min_rate, max_rate]` bounds at playback time, so extreme values silently saturate. |
| `tts.greeting`   | string | `""`      | 1.2.9+. Spoken at TUI startup just after the daily-progress splash, before the main editor frame renders. Non-blocking — speech plays in parallel with the editor coming up. Empty string skips (default). Honoured only when `enabled = true`. Example: `"Welcome back"` (English) or `"Доброе утро, Владимир"` (Russian, paired with `voice: "Milena"` / `"Katya (Enhanced)"`). |
| `tts.goodbye`    | string | `""`      | 1.2.9+. Spoken at TUI shutdown just before terminal teardown. Inkhaven **blocks** up to 5 seconds for the speech to drain so the shell doesn't truncate it — keep the text short (a few words). Empty string skips (default). Example: `"Goodbye"` / `"До скорого"`. |
| `style_warnings.enabled` | bool | `false` | 1.2.9+. Master switch for inline style-warning overlays. When on, the editor underlines stylistically weak prose (currently filter words) in amber so the writer can question + rewrite. `Ctrl+B Shift+F` toggles in-session without rewriting HJSON. |
| `style_warnings.filter_words.enabled` | bool | `true` (if master is on) | 1.2.9+. Filter-word detector — flags intensifier crutches (`just`, `really`, `very`) + hedges (`seemed`, `felt`) + sensory verbs. Built-in word lists ship for `english`, `russian`, `french`, `german`, `spanish`; the active list is keyed by the top-level `language` field. |
| `style_warnings.filter_words.use_stemming` | bool | `true` | 1.2.9+. Match via Snowball stemming so list entries are LEMMAS (e.g. `seem`, `казаться`) and the detector catches every inflection (`seemed` / `seems` / `seeming`, `казался` / `казалась` / `казалось` / `казались`). Set `false` to fall back to exact-lowercased match — you'd then need to list every form individually. |
| `style_warnings.filter_words.<lang>` | array | `[]` | 1.2.9+. Per-language list — one of `english`, `russian`, `french`, `german`, `spanish`. **Empty list = use the built-in default for that language; non-empty = REPLACE the default.** Use `extra_words` for additive overrides. Run `inkhaven doctor --filter-words-snippet` to get the populated built-in lists as HJSON ready to paste under `filter_words`. |
| `style_warnings.filter_words.extra_words` | array | `[]` | 1.2.9+. User-supplied words added **on top of** the language default. Case-insensitive; stemmed when `use_stemming` is on. Example: `["totally", "obviously"]`. |
| `style_warnings.repeated_phrases.enabled` | bool | `true` (if master is on) | 1.2.9+. Repeated-phrase detector — slides an `n`-word window across the paragraph, stems each window, flags every occurrence of any n-gram that repeats `threshold` or more times. Catches writer-crutch gestures like `lifted her shoulders` recurring across a chapter. |
| `style_warnings.repeated_phrases.n` | int | `4` | 1.2.9+. Window size — number of consecutive non-stop words compared. `3` is noisy (catches incidental patterns); `5+` misses most crutches. |
| `style_warnings.repeated_phrases.threshold` | int | `3` | 1.2.9+. Minimum occurrence count before flagging. `2` is too noisy in most prose; `3` is the editing-craft default. |
| `style_warnings.repeated_phrases.use_stemming` | bool | `true` | 1.2.9+. Stem the words before n-gram comparison so `lifted her shoulders` matches `lifting her shoulders`. Disable for exact-form matching. |
| `style_warnings.repeated_phrases.<lang>_stop_words` | array | `[]` | 1.2.9+. Per-language stop-word list (`english_stop_words`, `russian_stop_words`, …) — closed-class words excluded from n-gram comparison so `the dog and X` doesn't repeat-match `the dog and Y`. Empty = use built-in default for that language. |
| `style_warnings.show_dont_tell.enabled` | bool | `true` (if master is on) | 1.2.9+. Show-don't-tell detector — flags telling prose patterns: copula + emotion adjective (`was angry`), manner-of-emotion adverbs (`angrily`), and direct cognition verbs (`realised`, `knew`). Underline colour `style_warning_show_dont_tell_fg` (default `#94e2d5`). Pair with the AI-driven scan via `Ctrl+B Shift+T` for deeper analysis. |
| `style_warnings.show_dont_tell.use_stemming` | bool | `true` | 1.2.9+. Stem entries with the project's Snowball algorithm so inflections collapse — `seemed`/`seems`/`seeming` all match a single `seem` linking-verb entry. Disable for exact-form matching. |
| `style_warnings.show_dont_tell.<lang>_linking_verbs` / `*_emotion_adjectives` / `*_manner_adverbs` / `*_cognition_verbs` | array | `[]` | 1.2.9+. Per-language word lists across the four detector categories. Empty = use built-in default for that language; non-empty = REPLACE the default. **1.2.11+** — curated built-ins now ship for all five supported languages (English / Russian / French / German / Spanish), not just English. Per-genre tuning belongs in `inkhaven show-dont-tell bootstrap <lang>` which uses the configured LLM as a one-shot vocabulary curator. |
| `style_warnings.anachronism.year` | int | _unset_ | 1.3.8+. The manuscript's setting year. **The anachronism detector is OFF until this is set** — a contemporary novel sees nothing. Once set, terms whose earliest plausible year postdates it are flagged (a "wristwatch" in an 1840 novel). Findings surface in `inkhaven edit` (category `anachronism`), jumpable to the exact word. |
| `style_warnings.anachronism.terms` | array | `[]` | 1.3.8+. User additions to the ~35-term built-in lexicon — each `{ term: "spyglass-cam", earliest: 1990 }`. **Additive**, not a replacement: your terms extend the built-ins (telephone 1876, scientist 1834, okay 1839, …). Matched case-insensitively against whole words. |
| `pov_chip_enabled` | bool | `true` | 1.2.9+. Status-bar POV / character chip. When on, the status bar shows the most-mentioned character in the currently-open paragraph (the heuristic POV character) plus up to three additional named characters present. Driven by the project's existing `characters` lexicon — no separate tagging required. `Ctrl+B Shift+P` toggles in-session without rewriting HJSON. Chip colours are themed via `theme.pov_chip_bg` / `theme.pov_chip_fg` (1.2.10+ — explicit RGB defaults `#8b1d88` background / `#ffffff` foreground; tune these if the contrast doesn't read in your terminal palette). |
| `prompt_language_mode` | string | `"book_defined"` | 1.2.11+. Prompt-language resolver mode. `"book_defined"` uses the top-level `language` field for every AI prompt resolution; `"paragraph_detected"` runs `whatlang` on the live paragraph body and falls back to `book_defined` for paragraphs shorter than `prompt_language_detection_min_chars`. `Ctrl+B Shift+N` cycles a session-local override on top of this knob; the AI pane title bar's `lang=` chip reflects the active mode. See `Documentation/PROPOSALS/MULTILINGUAL_PROMPTS.md` for the resolver design. |
| `prompt_language_detection_min_chars` | int | `50` | 1.2.11+. Minimum non-whitespace character count required before `prompt_language_mode = "paragraph_detected"` will attempt whatlang detection. Below this threshold, the resolver silently uses the book language — whatlang is unreliable on short text. Edit-time cache invalidation (on save, on AI-diff accept, on external file change) also uses this value as the length-delta threshold. |
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

  // 1.2.13+ — invented-language word overlay.  Italic
  // when applied (mnemonic: italics is the typesetting
  // convention for foreign-language inclusions in
  // English prose).  Walks every Language/<lang>/
  // Dictionary entry's surface forms (lemma + every
  // paradigm in `inflection:` map) and lights them up
  // in the manuscript editor.
  language_word_fg:  "#b4a8e1"

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

  // 1.2.12+ — per-detector style-warning modifier
  // overrides.  Empty string maps to "underline"
  // (the 1.2.9-1.2.11 hardcoded default).  Accepts
  // "underline", "bold", "dim", "reversed",
  // "italic", "none", or "+"-combined like
  // "underline+bold".  Useful on terminal palettes
  // where the teal underline reads faint.
  style_warning_filter_word_modifier:       ""
  style_warning_repeated_phrase_modifier:   ""
  style_warning_show_dont_tell_modifier:    ""
  // 1.3.9+ — the live anachronism overlay.  A warm
  // amber-orange "wrong era" caution (default
  // "#eba672"), distinct from the show-don't-tell teal.
  // Off until you set editor.style_warnings.anachronism
  // .year; rides the master style-warnings toggle.
  style_warning_anachronism_fg:             "#eba672"
  // 1.2.20+ — the live echo overlay (Ctrl+B Shift+K).
  // Its own colour (default a muted purple "#b48ead")
  // so a cross-paragraph echo doesn't read as a
  // within-paragraph repeated phrase; modifier accepts
  // the same grammar as the three above.
  style_warning_echo_fg:                    "#b48ead"
  style_warning_echo_modifier:              ""

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

### Lexicon overlay (Places / Characters / Language)

`places_fg` colours any token in the editor that matches a paragraph
title in the **Places** system book; `characters_fg` does the same for
**Characters**. Stemming is applied per the project `language`, so a
Russian project's place "Москва" lights up "Москвы", "Москве", and so on
automatically. See [`LOCATIONS.md`](LOCATIONS.md) and
[`CHARACTERS.md`](CHARACTERS.md).

`language_word_fg` (1.2.13+) colours every word from every Language
sub-book's `Dictionary` chapter — the lemma *and* every paradigm value
in the entry's `inflection: {...}` map.  Rendered ITALIC + colour to
mirror the typesetting convention for foreign-language inclusions in
prose.  Cursor on a Language hit shows `[word · POS · translation]` in
the editor footer (lifted live from the entry's HJSON).  See
Tutorial 49 for the end-to-end Language-book workflow and
[`PROPOSALS/LANGUAGE_BOOK.md`](PROPOSALS/LANGUAGE_BOOK.md) for the
full design.

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

## 1.2.14 — new HJSON blocks

### `project` (1.2.14+) — project word-count goal

Drives the `Ctrl+V Shift+G` project-goal modal.
Optional block — when omitted, the modal explains
how to add it.

```hjson
{
  project: {
    // Total word count target across the books named
    // in `counted_books` (empty list = all user books).
    word_count_goal: 80000

    // ISO date the goal should be hit by.  Used to
    // project finish-date deltas in the modal verdict.
    target_date: "2026-12-31"

    // Books that count toward the goal.  Empty list
    // means "every top-level user book" (right for
    // single-novel projects).  Name explicit books for
    // RPG sourcebooks or anthology projects.
    counted_books: []
  }
}
```

Verdict glyphs in the modal: `✓ Ahead` (projected
finish < target), `· On track` (within 7 days),
`✗ Behind` (projected finish > target + 7), `✓
Complete` (current count ≥ goal).  Projection uses
the 30-day word delta from `progress_cache.sparkline`
— a rolling rate that matches the writing-progress
modal's pace calculation.

### `editor.continuation_anchor_count` (1.2.14+)

How many previous paragraphs the `Ctrl+V d` AI
continuation envelope sends as voice anchors.
Default: `3`.  Higher = the LLM has more voice
context but the envelope grows; cap around 5-6 for
typical paragraph sizes.

```hjson
{
  editor: {
    continuation_anchor_count: 3
  }
}
```

### `editor.footnote_style` (1.2.14+)

Inline footnote markup style for the `Ctrl+V f`
insertion.  Two values:

* `"typst"` (default) — inserts `#footnote[<body>]`
  at the cursor.  The assembled-book renderer
  honours this directly.
* `"markdown"` — inserts `[^id]` at the cursor and
  appends `[^id]: <body>` after the paragraph.
  Use when exporting to markdown-only targets.

```hjson
{
  editor: {
    footnote_style: "typst"
  }
}
```

### `snippets` (1.2.14+) — editor snippet expansion

Trigger-keyed text expansions.  When the editor
sees a trigger followed by Space, it replaces the
trigger with the expansion.  Trigger keys
conventionally start with `\` to avoid clashes
with prose.

```hjson
{
  snippets: {
    "\\dt": "{datetime}"
    "\\sl": "{slug}"
    "\\au": "— {author}"
    "\\todo": "TODO ({date}): {cursor}"
  }
}
```

Built-in placeholders:

| Placeholder | Expands to |
|-------------|------------|
| `{date}` | Today's date, ISO 8601 (`2026-05-31`) |
| `{time}` | Now, 24h (`14:23`) |
| `{datetime}` | `{date} {time}` |
| `{slug}` | Open paragraph's slug |
| `{book}` | Containing book's title |
| `{chapter}` | Containing chapter's title |
| `{author}` | `top_level.author` from inkhaven.hjson |
| `{cursor}` | Marker that controls post-expansion cursor position.  After the expansion pastes, the cursor jumps to where `{cursor}` was (instead of ending at the tail of the pasted text). |

Snippets without `{cursor}` paste atomically.  See
Tutorial 51 for the full snippet workflow.

Three picker-based placeholders (`{char_lookup}` /
`{place_lookup}` / `{artefact_lookup}`) and the
`bund:` prefix for Bund-VM expansion are queued for
a future release — they need an async snippet state
machine the current synchronous pipeline doesn't
yet have.

## 1.2.15 — new HJSON blocks

### `health` (1.2.15+) — background health monitor

The background health monitor catches project
inconsistencies before they cause data loss
(missed backups, orphan rescue files from
prior panics, etc.).  Disabled by default
so existing projects don't inherit a new
background task without opting in.

```hjson
{
  health: {
    enabled: false           // flip to true to spawn
                             // the monitor task
    auto_repair: {
      rescue_orphans: false  // when true, delete
                             // *.inkhaven-rescue
                             // files older than 30d
                             // automatically.  Use
                             // only on projects
                             // where you know
                             // you've reviewed
                             // every crash report.
    }
  }
}
```

When enabled, the monitor runs three checks at
independent cadences:

* **Project root reachable** (90 s) — Critical on
  missing, Warning on type mismatch.
* **Backup freshness** (5 min) — Warning when the
  newest backup is older than `backup.max_age`.
* **Rescue file orphans** (1 h) — Warning when
  `*.inkhaven-rescue` files older than 7 days
  exist under the project.  Auto-repair (when
  enabled) deletes files older than 30 days.

Findings drive the status-bar `health` chip:
`✓` clean, `✎` repaired, `⚠` warning, `✗` error.
Every non-`Ok` event is appended to
`<project>/.inkhaven/health.log` (size-rotated at
1 MB × 5 archives).

See [Tutorial 52](Tutorials/52-health-and-doctor.md)
for the full workflow.

### `scripting.trust_decision` (1.2.15+)

Gate for the auto-load of `Scripts` system-book
paragraphs at project open.  Default `"ask"`.

```hjson
{
  scripting: {
    trust_decision: "ask"  // "ask" | "trust" | "deny"
  }
}
```

Three values:

* **`"ask"`** (default) — scripts run only when
  `<project>/.inkhaven/trust` exists and contains
  the marker line `trust`.  Without that file the
  scripts are skipped and a warning lands in
  `.inkhaven.log`.
* **`"trust"`** — scripts run unconditionally.
  Use only on projects you authored or audited.
* **`"deny"`** — never run scripts regardless of
  the trust file.  Useful for read-only review.

The trust file lives outside the project sources
by convention (gitignored, since `.inkhaven/`
holds machine-local state); an attacker shipping
a project cannot pre-grant trust to themselves
via the file.  The HJSON `"trust"` value is the
project author's declaration "I wrote these
scripts" — recipients of a shared project should
audit before keeping that value.

See [Tutorial 53](Tutorials/53-bund-trust-gate.md)
and `Documentation/SECURITY_WARNING.md` §3.2.

### `scripting.fs_unsandboxed` (1.2.15+)

When `true`, `ink.fs.read` and `ink.fs.write`
operate on unrestricted paths.  Default `false`:
the words confine their paths to the project
root via `crate::path_safety::resolve_within`.

```hjson
{
  scripting: {
    fs_unsandboxed: false  // default — paths
                           // confined to project root
  }
}
```

The sandbox applies whether or not the `fs_read`
/ `fs_write` category gates are enabled.  Set
`true` only on trusted projects where a script
genuinely needs to reach a shared location
outside the project tree.

The 1.2.15 audit identified the previous
unsandboxed behaviour as a privilege risk: a
Bund script with `fs_write` enabled could
overwrite anywhere the user could reach
(`~/.ssh/authorized_keys`, system files with
sudo, etc.).  Confinement is the safer default.

## 1.2.16 — new HJSON blocks

### `backup.amber_threshold` (1.2.16+)

The 1.2.15 health monitor's backup-freshness
check went straight from Ok → Warn at
`backup.max_age`.  1.2.16 adds an intermediate
"amber" state so the user has visibility BEFORE
the warn fires.

```hjson
{
  backup: {
    max_age: "30d"         // 1.2.15 — when warn fires
    amber_threshold: 0.5   // 1.2.16 — when amber chip
                           //          appears (50% of
                           //          max_age by default)
  }
}
```

* `amber_threshold` is a fraction in `[0.0, 1.0]`.
* When backup age ≥ `amber_threshold × max_age`
  but < `max_age`, the status-bar chip turns
  amber with the `ℹ` glyph (Severity::Info).
* Above `max_age`, the existing 1.2.15
  Warning path takes over (yellow chip).

Set `0.0` to disable the amber tier (chip
behaves as 1.2.15).  Set `1.0` and the chip
flips straight to Warn (also 1.2.15 behaviour).

### `editor.show_glossary_chip` (1.2.16+)

Toggles the worldbuilding-density chip in the
status bar.

```hjson
{
  editor: {
    show_glossary_chip: true  // default
  }
}
```

When enabled, the status bar shows a
`<N>C·<N>P·<N>A` chip — cumulative counts of
**C**haracters / **P**laces / **A**rtefacts
entries across the system books.  Auto-hides
on fresh projects (all three counts zero) so
empty-project users don't see noise.

Useful at-a-glance check for cast density:
30 chapters in with only 4 named characters
tells you the cast is thin; 50 named places
across a 200-paragraph manuscript tells you
the reader has a thicket to navigate.

### `editor.show_facts_chip` (1.2.21+)

Toggles a Facts chip in the status bar — `⚑<N>`,
the number of entries in the **Facts** system
book (the world's invariants).  Off by default
(opt-in); auto-hides when the Facts book is
empty.

```hjson
{
  editor: {
    show_facts_chip: true
  }
}
```

A companion to the glossary chip: it surfaces
how richly the world's ground rules are
documented, so you notice a 40-chapter
manuscript still running on three facts.

### `editor.startup_haiku` (1.4.17+)

HAIKU-1. Emits a hand-curated haiku to the Output
pane at three moments: at **startup**, when a new
**manuscript paragraph** is created (user books
only — never system books like Characters or
Places, and never seeded/structural paragraphs),
and on demand via **`Ctrl+Z p`**. The poem is in
the book's language (`editor.language`), one of
EN / RU / DE / FR / ES, falling back to English.
On by default.

```hjson
{
  editor: {
    startup_haiku: true
  }
}
```

All 25 poems (5 languages × 5) are baked into the
binary — **no AI, no network**, present even on an
airgapped machine. A process-global rotation
counter advances on each trigger, so you rarely
see the same poem twice in a session. Set `false`
to silence the automatic moments; the `Ctrl+Z p`
chord still works on demand regardless.

See [Tutorial 100 — The startup haiku](Tutorials/100-haiku.md).

### `snippets` — `bund:` prefix + picker placeholders (1.2.16+)

Extends the 1.2.14 `snippets` block.  No new
config keys — the additions live inside
existing snippet bodies.

```hjson
{
  snippets: {
    enabled: true                   // 1.2.14
    expand_on: ["space", "tab"]     // 1.2.14
    bindings: [
      // Bund-prefix bodies (NEW in 1.2.16):
      { trigger: ";today",
        body: "bund:ink.now \"%Y-%m-%d\" ink.fmt" }
      { trigger: ";doubled",
        body: "bund:40 2 *" }

      // Picker placeholder bodies (NEW in 1.2.16):
      { trigger: ";cmeets",
        body: "She turned to {char_lookup}." }
      { trigger: ";at",
        body: "In {place_lookup}, the air was thin." }
      { trigger: ";holds",
        body: "He held the {artefact_lookup} aloft." }
    ]
  }
}
```

#### `bund:` prefix

When the snippet body starts with `bund:`, the
remainder is interpreted as a Bund-VM program.
The top-of-stack value at program end (coerced
to string) becomes the expansion.

* Sync placeholders are expanded BEFORE
  evaluation, so `bund:"{author}" ink.print`
  injects `{author}` first then runs the
  program.
* Script execution honours the trust gate +
  category policy — a `bund:` body that calls
  a `store_write` word in an untrusted project
  is denied with a warning.
* Empty stack / non-string top → empty
  expansion (snippet pastes nothing) plus a
  status-bar warning.

#### Picker placeholders

Three new placeholders interrupt expansion to
ask the author which entity to insert:

| Placeholder | Picker |
|-------------|--------|
| `{char_lookup}` | Characters system book |
| `{place_lookup}` | Places system book |
| `{artefact_lookup}` | Artefacts system book |

Behaviour:

* The leading sync placeholders (`{author}`,
  `{date}`, etc.) are resolved first.
* Text before the picker placeholder is pasted
  at the cursor immediately.
* The corresponding picker modal opens; on
  Enter the picked entry name + the tail of
  the snippet body are inserted at the cursor.
* Esc cancels — the head stays inserted, the
  tail is dropped.
* Only the **first** picker placeholder in a
  body fires the modal; any subsequent
  placeholders pass through as literal text
  (a documented limitation, can be lifted in
  a follow-up cycle by queueing modal
  continuations).

See [Tutorial 41 — Snippets](Tutorials/41-snippets-and-expansion.md)
for the full snippet reference and
`Documentation/RELEASE_NOTES/1.2.16.md` Phase
P.6 for the implementation log.

## 1.2.17 — new HJSON blocks

### `editor.tts` (extended for Piper)

1.2.9 introduced the `editor.tts.*` block for the
macOS `say` backend.  1.2.17 keeps every 1.2.9
field intact + layers a backend-agnostic engine
on top so the same chord (`Ctrl+B S`) can route
to either macOS `say` (the System backend) or
the new neural Piper backend.

```hjson
{
  editor: {
    tts: {
      // 1.2.9+ (preserved)
      enabled: false
      voice: "Milena"
      speed: 1.0
      greeting: ""
      goodbye: ""

      // 1.2.17+ (new)
      engine: "auto"
      voices_dir: ".inkhaven/voices"
      auto_download: true
      catalog_url: "https://huggingface.co/rhasspy/piper-voices/raw/main/voices.json"
      catalog_ttl_hours: 24
      binary_path: null
      auto_download_binary: true
      cache_max_voices: 5
      play_command: null
      sample_rate_hz: 22050
      auto_gitignore: true
    }
  }
}
```

Per-field reference:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `engine`              | string  | `"auto"` | Backend selector: `"auto"` prefers Piper if resolvable + falls back to System; `"piper"` forces Piper (errors if unresolvable); `"system"` forces the 1.2.9 macOS `say` backend. |
| `voices_dir`          | string  | `".inkhaven/voices"` | Directory for the Piper voice cache + catalog snapshot, resolved relative to the project root via `crate::path_safety::resolve_within_str`.  Absolute paths + `..` traversal are rejected at startup. |
| `auto_download`       | bool    | `true`   | When `true`, missing voices are streamed from `catalog_url` on first use.  When `false`, missing voices produce a clear "voice X is not downloaded; run `inkhaven tts voice download X`" error. |
| `catalog_url`         | string  | Hugging Face piper-voices `voices.json` | Voice catalog URL.  Override only if you maintain a private / mirrored catalog with the same JSON shape. |
| `catalog_ttl_hours`   | u32     | `24`     | How long the local catalog cache is fresh.  After expiry the next voice operation re-fetches.  Network failures during refresh fall back to the stale cache + log a warning rather than blocking synthesis. |
| `binary_path`         | string? | `null`   | Explicit path to a `piper` binary.  When `null`, inkhaven autoresolves via PATH then `~/.cache/inkhaven/piper-<plat>/`.  Treats the explicit override as authoritative — doesn't silently fall back to PATH if the path is set but unreadable. |
| `auto_download_binary`| bool    | `true`   | When `true`, the `inkhaven tts binary download` CLI fetches the platform-appropriate Piper release from GitHub.  TUI startup never auto-downloads — the chord-triggered surfaces always assume a pre-resolved binary. |
| `cache_max_voices`    | usize   | `5`      | LRU cap on the project's `voices_dir`.  Beyond the cap, the least-recently-used voice's `.onnx` + `.onnx.json` are removed.  Voice models are 25–100 MB each. |
| `play_command`        | string? | `null`   | Override the platform default playback command.  `{path}` is replaced with the resolved WAV path at spawn time.  Default per platform: `afplay {path}` (macOS), `paplay {path}` → `aplay {path}` fallback (Linux), `powershell -c "(New-Object Media.SoundPlayer '{path}').PlaySync()"` (Windows). |
| `sample_rate_hz`      | u32     | `22050`  | Sample rate for Piper synthesis output.  Piper's native rate is 22050 Hz; changing this triggers a resample inside the playback pipeline.  Most users should leave the default. |
| `auto_gitignore`      | bool    | `true`   | When `true`, the first auto-downloaded voice appends `.inkhaven/voices/` to the project's `.gitignore` (creating the file if absent).  Voices are large opaque blobs; checking them into git is universally wrong.  One-time, idempotent, atomic via `crate::io_atomic`. |

See [Tutorial 56](Tutorials/56-tts-piper.md) for the
full Piper workflow including the `Ctrl+B Shift+V`
voice picker, the `inkhaven tts` CLI surface, and the
known Apple-Silicon Piper limitation.

## 1.2.18 — new HJSON blocks

### `editor.reading_time_chip` + `editor.reading_wpm` (1.2.18+)

The R.3 reading-time chip + R.4 reader-pace preview
both read at a configurable words-per-minute.

```hjson
{
  editor: {
    reading_time_chip: false   // default — opt in
    reading_wpm: 200           // default
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `reading_time_chip` | bool | `false` | Show a status-bar chip for the current book: `📖 <remaining> / <total>` read-aloud length at `reading_wpm`, where remaining counts from the open paragraph to the book's end.  Off by default (the status bar is already busy); opt in when targeting an audiobook length or word budget.  Cheap — one O(n) walk of the current book's paragraphs. |
| `reading_wpm` | u32 | `200` | Words-per-minute for the reading-time chip, the reader-pace preview (`Ctrl+B Shift+E`), and the per-chapter timing displayed by the audiobook export.  200 ≈ silent-reading average; ~150 ≈ audiobook narration; ~300 ≈ a fast reader. |

See [Tutorial 58](Tutorials/58-reading-pace.md).

## 1.2.19 — new HJSON blocks

### `editor.echo_*` (1.2.19+)

Tunables for the C.1 `echo-repetition` doctor scan (a
distinctive word reused close together).

```hjson
{
  editor: {
    echo_window: 5        // default
    echo_min_repeats: 3   // default
    echo_max_global: 40   // default
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `echo_window` | usize | `5` | Window (consecutive paragraphs) for the echo scan.  A distinctive word reused `echo_min_repeats` times within this many paragraphs is flagged. |
| `echo_min_repeats` | usize | `3` | Occurrences within `echo_window` required to flag.  Lower = more sensitive. |
| `echo_max_global` | usize | `40` | Distinctiveness ceiling: words used more than this many times across a chapter are treated as common vocabulary (legitimately reused) and skipped, even when clustered.  Tune up for long works, down for short stories. |
| `echo_overlay` | bool | `false` | Default state of the live echo overlay (`Ctrl+B Shift+K`, 1.2.20+ C.1.b): underline, in the open paragraph, words echoing across nearby paragraphs — the inline companion to the `echo-repetition` doctor scan, reusing the three `echo_*` tunables above.  Painted in `theme.style_warning_echo_fg` (default `#b48ead`, distinct from the repeated-phrase overlay).  The session toggle overrides this; set `true` to always start on. |

### `editor.paragraph_long_secs` (1.2.20+)

Threshold for the R.3.b `paragraph-too-long` doctor scan.

```hjson
{
  editor: {
    paragraph_long_secs: 180   // default
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `paragraph_long_secs` | u32 | `180` | A paragraph whose estimated read time at `reading_wpm` exceeds this many seconds is flagged `paragraph-too-long` (Info, no autofix) — a wall of text the reader meets in one unbroken block.  Default 180s ≈ 600 words at 200 wpm.  Set lower to flag denser paragraphs; the finding is author-judgment (length can be a deliberate run-on). |

### `editor.disk_warn_mb` (1.2.20+)

```hjson
{
  editor: {
    disk_warn_mb: 100   // default; 0 disables
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `disk_warn_mb` | u64 | `100` | Low-disk pre-flight threshold (MiB).  When the volume holding the project has less than this much free space, the editor shows a one-time status-line warning at startup, before a session of edits or a long export.  Atomic writes already fail safely on a full disk (the original file survives, the error surfaces) — this is the proactive heads-up.  `0` disables the check. |

### `editor.warn_uncommitted_on_exit` (1.2.20+)

```hjson
{
  editor: {
    warn_uncommitted_on_exit: true   // default
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `warn_uncommitted_on_exit` | bool | `true` | On quit, if the project is a git repo with uncommitted changes (modified, staged, or untracked paths), confirm before exiting (`y`/Enter quits anyway, `n`/Esc stays).  The open paragraph is autosaved first, so the working tree reflects your latest edits.  Best-effort: silently skipped when the project isn't a git repo or `git` isn't installed (inkhaven shells out to your own `git`; it has no git dependency).  Set `false` to quit without the check. |

### Revision sidecars (1.2.19+)

The C.3 / C.4 revision features store their AI-extracted
data under `<project>/.inkhaven/`:

| File | Written by | Read by |
|------|-----------|---------|
| `continuity.json` | `inkhaven continuity extract` | `inkhaven continuity list` + the `continuity-drift` doctor scan |
| `tensions.json` | `inkhaven tension scan` | `inkhaven tension list` + the `unresolved-tension` doctor scan |

These are machine-generated; edit them by hand only if you
know the shape.  Both record the extraction language so the
drift / matching comparison uses the right stemmer.  Add
`.inkhaven/` to `.gitignore` if you don't want them tracked.

The `numeric-contradiction` scan's quantity lexicons
(number-words, units, directions) are **built in** for
English / French / Spanish; other languages skip the scan
cleanly (Russian + German + a `bootstrap-continuity` seed
CLI land in a follow-up).

See [Tutorial 59](Tutorials/59-revision-and-continuity.md)
for the full revision workflow + the multilingual-coverage
table, and [Tutorial 60](Tutorials/60-manuscript-format.md)
for the manuscript-format export.

## 1.3.0 — PDF production blocks

The PDF-1 subsystem (`inkhaven pdf …`, the `ink.pdf.*` Bund words, and
the `imposed_pdf` / `cover_pdf` book-takes) reads three new top-level
blocks. All merge through the normal cascade (project `inkhaven.hjson` →
global `~/.config/inkhaven`, global wins). See
[Tutorial 65](Tutorials/65-hand-binding.md) for the end-to-end workflow.

### `imposition` (1.3.0+) — folding-signature profiles

Named profiles for `inkhaven pdf impose --config <name>`, the `Ctrl+B Q`
preview, and the `imposed_pdf` book-take. Six are built in — `default`
and `chapbook` (A-series), `us_perfect` and `us_chapbook` (their Tabloid /
US-Letter analogues), `thick` (8-sheet push-out signatures for heavy
books), and `a5_book` (1.3.13 — a perfect-bound **A5 codex** folded from A4
sheets: gathered 16-page signatures with shingle creep and a
signature-number mark, the popular home-binding recipe) — and you add your
own keys to extend (a missing profile is an error listing the known names).

For a quick saddle-stitch booklet with **no profile at all**, use
`inkhaven pdf booklet <input>` (1.3.13): it builds the parameters directly,
auto-fitting the press sheet to two source pages side-by-side (so any trim
size works), one nested signature, balanced blanks. `--sheet <preset>`
centres the spread on a named sheet instead; `--creep` adds shingle
compensation; `--no-marks` drops crop/fold marks; `--dry-run` previews.

```hjson
imposition: {
  profiles: {
    default: {
      style: "perfect_bound"          // perfect_bound | saddle_stitch | side_stab
      sheets_per_signature: 4
      target_sheet_size: "A3"          // a preset name, or { width_mm, height_mm }
      orientation: "auto"              // auto | portrait | landscape
      margins: { bleed_mm: 3.0, crop_offset_mm: 5.0, fold_mark_length_mm: 8.0 }
      creep: {
        enabled: true
        paper_stock: "uncoated_80gsm"  // caliper source for the shingle
        thickness_mm_override: null    // set to bypass the stock caliper
        strategy: "shingle"            // shingle | pushout | none
      }
      marks: {
        crop: true, fold: true, registration: true,
        spine_marker: true, signature_number: true, color_bar: false
      }
      blank_page_policy: "append"      // append | balance
    }
    // chapbook: saddle_stitch, A4, creep off — a single folded booklet.
  }
}
```

### `cover` (1.3.0+) — cover/spine defaults

House defaults for `inkhaven pdf cover` and the `cover_pdf` book-take.
The spine width is **computed** from the interior page count + these
paper stocks (`spine = pages × interior_caliper × 0.5 + 2 × cover_caliper
+ binding allowance`); CLI flags (`--width-mm` / `--height-mm` /
`--spine-mm`) override per run.

```hjson
cover: {
  front_width_mm: 152.0      // 6 in
  front_height_mm: 229.0     // 9 in trade
  bleed_mm: 3.0
  interior_stock: "uncoated_80gsm"
  cover_stock: "cover_250gsm"
  spine_font_size_pt: 11.0    // max size; auto-shrinks to fit the spine
  image_fit: "cover"          // cover (default) | fit | stretch
}
```

`image_fit` controls how the front art fills its region: `cover`
(aspect-preserving full-bleed crop — the right default), `fit` (scale to
fit inside, may leave gaps), or `stretch` (distort to fill). Per run,
`inkhaven pdf cover --fit <mode>` overrides it.

### `preflight` (1.3.0+) — print-readiness DPI targets

Selectable targets for `inkhaven pdf preflight --profile <name>` (the
`--dpi N` flag overrides). `strict` uses 300 dpi with no override.

```hjson
preflight: {
  default_profile: "hand_binding"   // hand_binding | print_shop | strict
  hand_binding_dpi: 300
  print_shop_dpi: 300
}
```

Beyond the DPI target, every profile also reports the press hazards that
silently change on output — **overprint**, **transparency** (alpha < 1,
non-Normal blend modes, soft masks), and **spot colours** (Separation /
DeviceN colorants, one plate each) — as warnings; there are no knobs to
turn these off.

### `output` additions (1.3.0+)

Two `output.extra_formats` entries operate on the just-built PDF rather
than the `.typ` source, so they run after the PDF lands:

- `imposed_pdf` — impose with `output.imposed_pdf_config` (a profile name
  from `imposition.profiles`, default `"default"`), writing
  `…-imposed.pdf`.
- `cover_pdf` — generate `…-cover.pdf` from the page count + the `cover:`
  block (title from the book, no barcode).

```hjson
output: {
  extra_formats: ["imposed_pdf", "cover_pdf"]
  imposed_pdf_config: "default"
}
```

## 1.3.8 — world-consistency blocks

### `facts` (1.3.8+) — series-shared canon

A top-level `facts` block lets a multi-book series share one canon. Point
`shared_path` at a directory of plain-text fact files — one file per fact,
the filename (de-slugified) is the title and the contents are the body:

```hjson
facts: {
  shared_path: "../series-bible/facts"
}
```

- `inkhaven facts check` then layers the shared canon under each book's
  local Facts book (**local wins** on a title clash) before running the
  internal-consistency AI pass — so a contradiction between this book's
  prose and the series bible is caught.
- `inkhaven facts import` copies a snapshot of `shared_path` into *this*
  book's Facts book as paragraphs (idempotent — skips ones already
  present; dry-run by default, `--yes` to write, `--from <dir>` to override
  the configured path).

Unset = no series sharing; each book's Facts book stands alone.

### `editor.style_warnings.anachronism` (1.3.8+) — era checking

See the `style_warnings.anachronism.year` / `.terms` rows in the [`editor`
table](#editor) above. Set the manuscript's setting `year` to arm the
detector; terms postdating it (built-in ~35-term lexicon plus your
additive `terms`) are flagged in `inkhaven edit` under category
`anachronism`.

## 1.3.10 — semantic-drift block

### `drift` (1.3.10+) — divergent-description retrieval

`inkhaven drift scan` finds descriptions of the same entity (character /
place / artefact) that diverge across the manuscript without a hard fact
clash. It **reuses the existing on-save vector index** to retrieve each
entity's description paragraphs, then asks the LLM which contradict; the
`drift` block tunes the retrieval that bounds the AI prompt:

```hjson
drift: {
  top_k: 24          // vector hits pulled per entity before name-filtering
  max_snippets: 8    // descriptions kept per entity (bounds the judge prompt)
}
```

- `top_k` (default `24`) — how many semantic-search hits to pull per entity
  before keeping only the paragraphs that actually name it. Raise it for a
  long book where an entity is described many times; lower it to go faster.
- `max_snippets` (default `8`) — the cap on descriptions sent to the judge
  per entity. The larger it is, the more thorough (and more expensive) each
  entity's check. Findings surface in `inkhaven edit` (category `drift`,
  jump-only) and the `Ctrl+V Shift+L` story bible.

## 1.3.13 — per-language detector maps

The style + drift detectors key off the top-level `language` field. Five
languages ship fully curated (`english`, `russian`, `french`, `german`,
`spanish`); 1.3.13 lets you curate **any** language through per-language
**maps** (keyed by language name) instead of the old five fixed fields. An
uncurated language gets **empty** built-ins (no false flags) — never the
English lists. Check coverage with **`inkhaven lang status [--language <l>]`**,
or have the LLM populate every map at once with **`inkhaven lang bootstrap
<language> [--yes]`** (it writes exactly these paths, with a backup +
comment-preserving in-place patch).

```hjson
editor: {
  style_warnings: {
    filter_words:     { languages: { polish: [ "tylko", "naprawdę", … ] } }
    show_dont_tell:   { languages: { polish: {
      linking_verbs:      [ … ]   // copula / quasi-copula asserting inner state
      emotion_adjectives: [ … ]   // adjectives that name an emotion outright
      manner_adverbs:     [ … ]   // emotion-labelling adverbs
      cognition_verbs:    [ … ]   // verbs that narrate thought
    } } }
    repeated_phrases: { languages: { polish: [ "i", "w", "na", "że", … ] } }
  }
}
drift: {
  pronouns: {
    polish: {
      character: [ "on", "ona", "oni", … ]  // 3rd-person, standalone words only
      place:     [ "tam", "tu", … ]
      artefact:  [ "to", "ten", … ]
    }
  }
}
```

- All words are **lemmas** — Snowball stemming (18 languages) catches the
  inflections; set `use_stemming: false` to match exactly instead.
- An entry under `languages.<lang>` is the curated list for that language; the
  old five fixed fields still deserialize and remain valid.
- Two complete worked examples ship in
  [`custom_languages/`](../custom_languages/): **Arabic** (RTL, pro-drop) and
  **Hungarian** (agglutinative, genderless 3rd person).

### Localized world-check prompts

The four AI world-consistency scans (`facts check` / `facts scan` / `drift` /
`continuity`) run with prompts **localized** to the project language for the
five curated languages and force their findings' explanations into that
language. An uncurated language falls back to the English prompt **with a
warning**. Each prompt is overridable through the 3-tier cascade — a `Prompts`
book entry (`{slug}-{lang}` → `{slug}`) → `prompts.hjson` → the localized
built-in (see [`PROMPTS.md`](PROMPTS.md)).

## 1.3.32–1.3.37 — config additions

The road to 1.4.0 added several user-facing knobs. All default to the
behaviour that shipped hardcoded, so an existing `inkhaven.hjson` keeps
working unchanged; set any of these to opt in.

### `cost` (1.3.34/1.3.37) — AI-cost dashboard

Surfaced by `Ctrl+B $` / `inkhaven cost`. The daily caps are **informative,
not gates** — past a budget the slow tracks warn and continue (the permissive
principle), so these tune the bars + warning thresholds rather than blocking
anything.

```hjson
cost: {
  world_daily_call_cap: 200            // world fact-check slow-track ceiling
  inner_socrates_daily_call_cap: 150   // Inner Socrates slow-track ceiling
  usage_retention_days: 30             // days of per-category tallies kept in
                                       // .inkhaven/ai_usage.json before pruning
}
```

### `goals.day_boundary` (1.3.37) — when the writing day rolls over

Governs the streak, daily word/active totals, AI-usage tallies, and the
slow-track caps so they all agree on "today". `utc` (default) resets at
00:00 UTC; `local` resets at the writer's local midnight — so an evening
session far from UTC isn't attributed to "tomorrow" and the streak doesn't
flip mid-evening. (Uses the current UTC offset as a fixed shift — exact except
across a DST transition.)

```hjson
goals: { day_boundary: local }
```

### `project_lock` (1.3.36) — single-instance advisory lock

Two sessions on one project both write `metadata.db` / `.session.json`;
interleaved writes can corrupt the store. The lock (an OS advisory `flock` on
`<project>/.inkhaven.lock`, auto-released on crash) guards against that —
**informing, never hard-blocking** by default.

```hjson
project_lock: {
  enabled: true            // false disables single-instance guarding entirely
  on_conflict: "prompt"    // when another live session holds the lock:
                           //   "prompt" — interactive y/N (warn+proceed headless)
                           //   "warn"   — always warn and proceed
                           //   "refuse" — never open a second session
}
```

### `editor` — durability & behaviour knobs (1.3.37)

```hjson
editor: {
  crash_mirror_seconds: 2           // cadence of crash-rescue buffer mirrors
                                    //   (lower = fewer keystrokes lost on a panic)
  deleted_paragraph_history: 10     // kill-ring depth for Ctrl+V Shift+U undelete
  external_change_auto_reload: true // false → warn instead of silently reloading
                                    //   a CLEAN buffer when its file changes on disk
  fact_check_idle_seconds: 5        // idle delay before the auto fact-check fires
  visited_history_cap: 0            // 0 = unbounded; cap the back/forward visit
                                    //   list (and .session.json growth)
}
```

### `backup` — retention & explicit toggle (1.3.37)

In addition to the existing `out_dir` / `max_age` / `wait_for_key_after_backup`
/ `amber_threshold`:

```hjson
backup: {
  auto_backup_on_exit: true   // clear toggle for the exit-hook backup (vs the
                              //   non-obvious out_dir=""/max_age=0s side-effects)
  keep_last: 0                // 0 = keep all (prior behaviour); otherwise the
                              //   oldest backup zips beyond this count are pruned
}
```

## 1.4.1 — Book-RAG ("Chat with Your Book")

### `book_rag` (1.4.1+) — AI-pane Book-scope retrieval

The AI pane's **Book scope** is retrieval-augmented (BOOK_RAG-1). A prompt in
Book scope no longer ships the whole manuscript: it retrieves the
semantically relevant paragraphs (via the same on-save vector index every
other semantic feature uses), expands each with a little surrounding context,
composes a focused, token-budgeted grounding block, and asks the model to
answer **citing** those passages by their location path (`[chapter/scene]`).
This is always on for Book
scope — there is no enable/disable toggle and no "send the whole book"
fallback. **The other scopes (Selection / Paragraph / Subchapter / Chapter)
are unchanged.** The `book_rag` block tunes the retrieval:

```hjson
book_rag: {
  top_k: 5                   // direct semantic hits retrieved per query
  context_expansion: 1       // ± sibling paragraphs included around each hit
  max_context_tokens: 8000   // budget for the composed context (≈ chars/4)
  include_system_books: [    // author-content system books also searched,
    notes, research, places,  //   by system_tag — so a Book-scope answer can
    characters, artefacts,    //   draw on your notes, world, and character
    world, language           //   bibles, not just the manuscript prose
  ]
  exclude_system_books: [    // meta/internal system books never searched
    scripts, prompts, typst, help, intent
  ]
}
```

- `top_k` (default `5`) — how many directly-matching paragraphs ground each
  answer. Raise it for broader, more synthesis-heavy questions; lower it to
  keep answers tightly sourced.
- `context_expansion` (default `1`) — the number of paragraphs kept on each
  side of a hit, so a retrieved passage reads in context rather than mid-scene.
  `0` retrieves hits only.
- `max_context_tokens` (default `8000`) — the ceiling on the composed
  grounding block (estimated as characters ÷ 4; there is no tokenizer in-tree).
  Best hits are admitted first; once the budget is reached the rest are
  dropped, but at least the top passage always survives.
- `include_system_books` / `exclude_system_books` (defaults above) — which
  system books join the manuscript in the retrieval pool, addressed by their
  `system_tag`. A tag in **both** lists is excluded. The retrieval is always
  anchored to the user book containing the cursor; these lists only add the
  author's reference material around it.

Retrieval happens **once per chat session** — follow-up questions reuse the
same passages, and clearing the chat history (which also ends the session)
makes the next Book prompt retrieve afresh. After you save an edited paragraph
while a Book-scope conversation is open, inkhaven nudges you once to clear the
chat and re-ground on the new text. Every Book-scope call is tagged in the
cost dashboard under the `book_rag` category — **informative only; it never
blocks a request.**

Inspect exactly what a query would retrieve from the terminal, without an LLM
call:

```sh
inkhaven book-rag retrieve "what does Mara fear about the voyage?"
inkhaven book-rag retrieve "the harbour market" --top-k 8 --context
```

`--book-name` selects the book in a multi-book project; `--top-k` overrides
the config for that run only; `--context` prints the composed grounding block
the model would receive instead of the human-readable passage listing. See
[Tutorial 87 — Chat with Your Book](Tutorials/87-chat-with-your-book.md).

## 1.4.2–1.4.3 — Inner Editor (INNER_EDITOR-1)

### `inner_editor` (1.4.2+) — the literary/stylistic companion

The second **Inner-family** member. Where Inner Socrates interrogates facts and
structure, **Inner Editor** *observes* literacy and style through a single
configurable persona — brief, grounded, **non-prescriptive** observations
graded **Praise / Note / Concern**. LLM-only, paragraph scope. The whole block
is optional; every default preserves good behaviour, and the feature only
engages when an LLM provider is configured. Cost caps **inform, never block**.

```hjson
inner_editor: {
  enabled: true                 // false fully disables (manual chord then just informs)

  engagement: {
    idle_threshold_seconds: 60  // ambient paragraph-pause wait before auto-engaging
    cooldown_seconds: 120       // same-¶ cooldown; an edit re-arms the timer
    max_findings_per_paragraph: 3
  }

  context: {
    preceding_paragraphs: 3     // interpretation context sent with the paragraph
    following_paragraphs: 0
  }

  persona: {
    tone: balanced              // critical | balanced | encouraging
    verbosity: concise          // concise | standard | detailed
    praise_frequency: moderate  // rare | moderate | frequent
    genre_aware: true           // fold the project `genre` into the prompt
    belief_stance_enabled: true // allow the "does the prose believe itself?" category
    categories: {               // turn any of the eight off individually
      literary_richness: true
      tautology: true
      style_observation: true
      style_instability: true
      dictionary_richness: true
      belief_stance: true
      craft_praise: true
      editorial_suggestions: true
    }
  }

  output: {
    severity_threshold: note    // praise | note | concern — the visible floor;
                                //   at `note`, Praise is persisted but not pushed
                                //   to Output (filter / `findings list` to see it)
    group_by_paragraph: true
    always_show_persona_label: true
  }

  llm: {
    editor_engagement: {        // caps INFORM (warn + continue), never block
      max_calls_per_session: 80
      confirm_above_calls: 40
      max_calls_per_day: 200    // shown in `inkhaven cost` / Ctrl+B $
      max_calls_per_month: 4000
    }
    conversation: { max_calls_per_session: 30, max_calls_per_day: 80 }
    backoff_max_retries: 3
    backoff_initial_seconds: 30
  }
}

// Project-wide, consumed by Inner Editor's `genre_aware` AND (1.4.6) the
// genre-aware Inner Socrates system prompt:
genre: literary_realism         // fiction: literary_realism | fantasy | scifi |
                                //   mystery | memoir | historical | romance | horror | ya | comedy
                                // nonfiction (1.4.6): nonfiction | technical |
                                //   documentation | academic | science | business
                                // ideas (1.4.7): utopian | philosophy | theology
```

With no `genre` declared, both companions keep their original fiction framing —
the nonfiction and ideas genres are purely additive. A declared nonfiction genre
reframes the Inner Socrates interrogator (procedures as reproduction attempts,
claims needing support) and the Inner Editor (clarity/completeness as the craft).
The **ideas genres** (1.4.7) go further: `philosophy` and `theology` frame the
interrogator to probe argument and coherence rather than empirical evidence (a
theology reader respects revelation/tradition as ground, never demanding proof);
`utopian` frames the imagined society as an argument to interrogate.

- **`tone` / `verbosity` / `praise_frequency`** are parsed tolerantly — a bad
  value falls back to its default rather than failing the whole config. The
  persona's character is fixed; these knobs modulate how it manifests.
- **`severity_threshold: note`** keeps Praise out of the default Output view (it
  is still recorded — `inkhaven inner-editor findings list --severity praise`,
  or lower the threshold). Praise must be *earned* by the prose; generic
  encouragement is forbidden by prompt design.
- The grounding prompt is localized **EN/RU/ES/FR/DE** (English fallback) and is
  overridable through the standard chain — a `Prompts` book entry named
  `inner-editor-system` (optionally tagged `lang:<code>`) wins over the bundled
  default. See [PROMPTS.md](PROMPTS.md).

**Engaging it.** `Ctrl+V O` (O = *Observe*) opens the overview → `E` engage the
open paragraph, `C` open an Editor conversation (also F9 → Editor scope), `A`
toggle the opt-in ambient auto-engage, `F` jump to the findings. From the
terminal:

```sh
inkhaven inner-editor engage --paragraph <id>     # or --text "…"
inkhaven inner-editor findings list [--severity praise|note|concern]
inkhaven inner-editor findings history --paragraph <id>
inkhaven inner-editor intent <category> [--chapter <id>] [--description "…"]
inkhaven inner-editor config show
inkhaven inner-editor usage
```

`inner-editor intent <category>` declares an observation category a deliberate
choice — it writes into the **shared intent ledger** (the same one Inner
Socrates uses), and future Editor findings of that category in scope are
suppressed. Scripts get the read-only `ink.inner_editor.*` inspectors plus
`intent.declare` (store_write) and `engage` (ai_write); see
[BUND_TUTORIAL.md](Bund/BUND_TUTORIAL.md). See
[Tutorial 88 — Inner Editor](Tutorials/88-inner-editor.md).

## 1.4.6 — Nonfiction reader personas (AUDIENCE-1)

### `inner_socrates_default_persona` (1.4.6+) — project default persona

A top-level project key naming the Inner Socrates persona a project should start
with — useful for nonfiction/technical books that want a nonfiction reader by
default instead of the fiction `inner-socrates`:

```hjson
// Top-level, beside `genre`:
inner_socrates_default_persona: skeptical-practitioner
```

- The value is any persona id — bundled (`inner-socrates`, `careful-editor`,
  `skeptical-reader`, `first-time-reader`, `slow-reader`, `skeptical-practitioner`,
  `domain-newcomer`, `expert-reviewer`, `end-user`) or one of your own.
- It is consulted **only** when no persona has been explicitly set for the
  project. The moment you run `inner-socrates persona activate <id>` (or cycle
  with `Ctrl+B J → S`), that explicit choice is stored and wins over this key.
- `None` (the default) keeps the bundled fiction `inner-socrates`.
- The Inner Socrates overview marks the active persona `(project default from
  config)` while it comes from this key rather than an explicit activation.

See [Tutorial 90 — Nonfiction reader personas](Tutorials/90-nonfiction-personas.md).

## 1.4.12 — Narrative voice profiling (NARR-1)

`inkhaven prose` measures voice as a statistical property of the whole book —
**deterministically, zero-AI, zero-cost** — per chapter, in five embedded
languages (EN/RU/DE/FR/ES). **`prose profile`** / **`prose drift`** print it;
**`Ctrl+V V`** runs a background check whose threshold crossings land in the
Output pane as informational findings; **`Ctrl+V Shift+V`** toggles ambient.
Profiles live in `.inkhaven/prose.duckdb`.

```hjson
prose: {
  deep_metrics: false        // include Tier-2 (sensory balance + active/passive ratio)
  mattr_window: 100          // MATTR sliding-window size (tokens)
  baseline_chapter: 1        // drift / violations are measured against this chapter
  language: null             // override (en/ru/de/fr/es); null → project language → EN + note
  ambient: false             // Ctrl+V Shift+V default; re-run after an editing pause
  ambient_cooldown_secs: 90  // floor between ambient runs (it's a whole-book scan)
  thresholds: {              // a chapter metric crossing its threshold vs baseline → finding
    sent_len_cv: 0.15
    burstiness_b: 0.15
    mattr: 0.05
    modal_density: 0.020
    interiority_ratio: 0.10
    de_erlebte_rede_particle_density: 0.05   // DE only
    sensory_channel_max: 0.15
    active_passive_ratio: 1.5
  }
  extra_modal_tokens: []         // appended to the active language's modal/hedging list
  extra_interiority_phrases: []  // appended to the active language's FID phrase list
}
```

- **Self-gating / zero-cost:** nothing runs until you ask (`prose` CLI / `Ctrl+V V`
  / ambient); the background check is content-hash lazy, so only edited chapters
  recompute, and it never calls an LLM.
- **Multilingual:** every language-sensitive metric has a curated list for all
  five languages; an unsupported language gets the Tier-1 rhythm metrics + a note.
  FR/ES authors can add subjunctive hedging collocations via `extra_modal_tokens`.
- **Findings are informational only** — they tell you *where* the voice moved.

See [PROSE_VOICE.md](PROSE_VOICE.md) and
[Tutorial 95 — Narrative voice](Tutorials/95-narrative-voice.md).

## 1.4.14 — Dialogue quality & attribution (DIALOG-1)

The optional `dialogue:` block tunes dialogue detection. Omit it entirely for
the defaults below.

```hjson
dialogue: {
  // Token window for the attribution name search around a span boundary.
  attribution_window: 60
  // Consecutive unattributed turns tolerated in an established two-speaker
  // exchange before the zero-attribution finding fires.
  unattributed_run_threshold: 8
  // Consecutive dialogue-only paragraphs before the talking-head finding.
  talking_head_threshold: 6
  // Minimum words for a non-speech sentence to count as an action beat.
  beat_min_words: 8
  // Said-bookism density delta (above the book baseline) that triggers a finding.
  said_bookism_threshold: 0.15
  // Minimum attributed utterances for a character fingerprint to be shown.
  fingerprint_min_utterances: 5
  // Dialogue language override (en/ru/de/fr/es); null → project language → en.
  language: null
  // Verbs added to the active language's NEUTRAL list, so genre speech verbs
  // (SF/fantasy) aren't counted as said-bookisms.
  extra_neutral_verbs: ["transmitted", "sent"]
  // Verbs added to the said-bookism list.
  extra_said_bookisms: []
}
```

- **Deterministic, zero-AI, zero new crates.** Five languages (EN/RU/DE/FR/ES)
  across three quotation conventions. Profiles live in `.inkhaven/dialogue.duckdb`,
  invalidated by a content hash (only edited chapters re-detect).
- **`extra_neutral_verbs`** is the key knob for science fiction / fantasy:
  `transmitted`, `intoned`, `decreed`, `sent` (telepathy) are intentional speech
  verbs, not said-bookisms — listing them here removes them from the density
  metric without disabling detection of the rest.
- The dialogue findings ride the `Ctrl+B Shift+C` review pass; `Ctrl+V Shift+Q`
  opens the per-character fingerprint view. Read-only `ink.dialogue.*` Bund.

See [Tutorial 97 — Dialogue quality](Tutorials/97-dialogue-quality.md).

## 1.4.15 — Utopian/dystopian coherence (WORLD-6)

The optional `utopia:` block tunes the coherence checker. Omit it for the
defaults below.

```hjson
utopia: {
  // Stage 2 cost-warning threshold (USD). Fires before pairing if the
  // projected cost exceeds this — informs, never blocks.
  stage2_cost_warn: 0.10
  // Chapters processed per Stage 3 background pass.
  stage3_batch_size: 5
  // Minimum chapter word count for the Stage 3 entailment scan.
  stage3_min_chapter_words: 200
  // Consecutive non-claim paragraphs that break a premise group
  // (1 = any gap breaks; 0 = all tagged paragraphs are one group).
  group_gap_threshold: 1
}
```

- Declare premises in the **World** book with `para:utopia-{premise,mechanism,
  consequence,elimination}` paragraphs (glyphs ⊢ ⚙ ⇒ ∅). Run the check with
  **`inkhaven world utopia-check [--stage 1|2|3|all]`** — Stage 2 (pairing) is
  **explicit-only** (O(pairs) LLM calls). It reads only what you declare and
  reasons about logical/systemic structure only; moral/theological coherence is
  out of scope (reserved for a future Inner Theologian).
- Profiles live in `.inkhaven/utopia.duckdb`. Findings are advisory (`info`),
  surface in the `Ctrl+B Shift+C` review pass, and ground the
  `utopian-architect` persona. Read-only `ink.utopia.*` Bund. `inkhaven world`
  is now a subcommand group; the bare `inkhaven world` consistency snapshot is
  unchanged.

See [Tutorial 98 — Utopia coherence](Tutorials/98-utopia-coherence.md).

## 1.4.16 — Character arc tracking (CHAR-1)

The optional `char:` block tunes the character-arc tracker. Omit it for the
defaults below.

```hjson
char: {
  // Consecutive unchanged chapters (after the baseline) before a stall fires.
  stall_threshold: 4
  // Tokens before an action verb a name may sit and still count as the actor.
  active_window_before: 5
  // Tokens after a verb a name may sit and count as the patient.
  active_window_after: 8
  // Minimum chapters of extracted state before the LLM arc checks run for a
  // character (fewer → stall check only).
  min_chapters_for_check: 3
  // Enrich the state chain with DIALOG-1 utterance/hedge signals.
  enrich_from_dialogue: true
  // Enrich the state chain with NARR-1 chapter interiority.
  enrich_from_voice: true
  // Arc language override (en/ru/de/fr/es); null → project language → English.
  language: null
  // Genre verbs the agency scorer should treat as deliberate action, appended
  // to the active language's list (e.g. SF "warped", "teleported").
  extra_action_verbs: []
  // State-extraction cost-warning threshold (USD). Informs, never blocks.
  extraction_cost_warn: 0.20
}
```

- Declare an arc in a **Characters**-book paragraph's `character_arc` HJSON block
  (`arc_type`, `desired_state_start` / optional `desired_midpoint_state` /
  `desired_state_end`). Run **`inkhaven character refresh`** (agency + state
  extraction), **`check`** (arc completeness — exits 1/2 for CI), **`plan`**
  (Planning-Board coverage gaps), **`arc <name>`** (report).
- Agency is deterministic and zero-AI; state extraction is the only LLM pass
  (content-hash lazy). Profiles live in `.inkhaven/char.duckdb`. Findings are
  advisory (`info`): the deterministic layers + cached problems surface in the
  `Ctrl+B Shift+C` review pass (Output `character` category) and the
  **`Ctrl+V Shift+N`** arc view. Read-only `ink.char.*` Bund. It never edits
  prose.

See [Tutorial 99 — Character arcs](Tutorials/99-character-arcs.md).

## 1.4.18 — Inner Theologian (INNER-THEOLOGIAN-1)

The optional `theologian:` block tunes the tradition-neutral moral/theological
reader. Omit it for the defaults below; `enabled: false` gates everything.

```hjson
theologian: {
  // Master switch. false gates everything, including the fast-track.
  enabled: true
  // Fire a Category-1 question on paragraph idle (auto-fire). false → only
  // Ctrl+B J→T and the Output-pane fast-track signals.
  on_paragraph_idle: true
  // Idle seconds before auto-fire; null → Inner Socrates' Slow threshold.
  idle_threshold_seconds: null
  // Slow-track LLM sub-budget (USD per session). Caps inform, never block.
  session_budget: 0.15
  // Run the deterministic fast-track in the review pass / deep-refresh.
  fast_track: true
  // Paragraphs after a harm event checked for acknowledgment (Signal 1).
  moral_invisibility_window: 3
  // Paragraphs after lethal violence checked for consequence (Signal 2).
  consequence_gap_window: 5
  // Emit the sacred-vocabulary-in-levity signal (Signal 3).
  sacred_levity_signal: true
  // Tradition lens codes to EXCLUDE from slow-track hints (default none — all
  // eleven available). e.g. ["gnostic"].
  disabled_lenses: []
  // Question/marker language override (en/ru/de/fr/es); null → project → en.
  language: null
}
```

- **Eleven tradition lenses** (Catholic, Protestant, Orthodox, Gnostic, LDS,
  Islam, Judaism, Hinduism, Buddhism, Confucianism, secular philosophy), applied
  uniformly — no tradition privileged. The persona belongs to none and **never
  delivers a verdict**; everything is a question.
- **Fast track** (deterministic, zero-AI): three `info` signals — moral
  invisibility, consequence gap, sacred levity — in the Output `theologian`
  category (`⚖`), folded into the `Ctrl+B Shift+C` review pass. **Slow track**
  (LLM): `Ctrl+B J→T` engages the open paragraph; `inkhaven theologian session
  [--chapter N] [--category 1-6] [--lens <code>]` runs it from the CLI.
- `inkhaven theologian scan` exits 1 on any unsuppressed signal (a
  pre-submission prompt); `theologian suppress --para <id> --reason "…"` mutes
  one. Read-only `ink.theologian.signals` + `ink.theologian.suppress` Bund.
  Findings live in `.inkhaven/inner_theologian.db`. It never edits prose.

See [Tutorial 101 — Inner Theologian](Tutorials/101-inner-theologian.md).

## 1.4.19 — Mythology & symbolic pattern library (MYTH-1)

The optional `myth:` block tunes the mythological & symbolic pattern library over
the **declared** Mythology system book. Omit it for the defaults below;
`enabled: false` gates the deterministic review-pass findings and the heatmap
chord.

```hjson
myth: {
  // Master switch. false gates the review-pass findings + the heatmap chord.
  enabled: true
  // Chapter buckets the heatmap collapses the book into.
  heatmap_buckets: 8
  // A symbol must appear in at least this many chapters before the LLM
  // consistency check runs on it.
  consistency_min_chapters: 5
  // A motif must have at least this many occurrences before the LLM
  // completeness check runs on it.
  motif_min_occurrences: 3
  // The final act = the last this-percent of chapters (motif-absent check).
  final_act_pct: 25
  // Warn (inform, never block) when an `inkhaven myth check` LLM run is
  // estimated to exceed this many USD.
  check_cost_warn: 0.08
}
```

- **Declarations only.** The library tracks the symbols (`para:myth-symbol`,
  `⊛`), motifs (`para:myth-motif`, `∿`), and archetypes (`para:myth-archetype`,
  `⍟`) you declare in the Mythology book. It never discovers, never interprets,
  never edits prose. Symbol vocabulary highlights in the editor (`myth_symbol_fg`,
  lavender by default).
- **Deterministic** (zero-AI): archetype vacant / absent, motif absent from the
  final act — in the Output `myth` category, folded into the `Ctrl+B Shift+C`
  review pass. **LLM** (explicit, `inkhaven myth check`): symbol consistency,
  motif completeness, archetype role fulfilment.
- **`Ctrl+V Shift+M`** builds the symbol/motif/archetype heatmap into the Thoughts
  pane and jumps to the nearest declared symbol. CLI: `inkhaven myth
  scan|check|profile|refresh|suppress` (`check` exits 1 on any finding). Bund:
  `ink.myth.{symbols,motifs,archetypes,density,findings}` (read) +
  `ink.myth.suppress` (write). Findings live in `.inkhaven/myth.duckdb`.
- The **Myth-Reader** Inner Socrates persona reads for symbolic resonance; your
  declared symbol traditions ground **Inner Theologian** and your declared motifs
  ground the **utopian-architect** persona.

See [Tutorial 102 — Mythological & Symbolic Pattern Library](Tutorials/102-mythology.md).

## 1.5.0 — Research Assistant (RESRCH-1)

The optional `research:` block tunes `inkhaven research` — a separate TUI screen
for AI-assisted research that transfers verified findings into the Facts / Notes
corpus. Omit the block for the defaults below.

```hjson
research: {
  // Default thread to open (null = picker / `default`).
  default_thread: null
  // Max Facts paragraphs prepended per query as RAG context.
  rag_top_n: 5
  // Per-session cost-cap warning (USD). Informs, never blocks.
  session_budget_warn: 0.50
  // Max pinned Facts nodes (Ctrl+P).
  max_pinned_nodes: 3
  // Show the keybind hints bar by default (? toggles).
  show_keybind_hints: true
  // Minimum terminal width; below it, a resize message shows.
  min_width: 80
  // Facts tree / chat split: tree columns out of 10 (4 = 40% tree).
  split_ratio: 4
  // /diff: number of similar facts to show.
  diff_top_n: 3
  // /verify: minimum sentence word count for claim extraction.
  verify_min_sentence_words: 8
  // /fact: warn of a near-duplicate at/above this similarity (0..1); informs,
  // never blocks (RESRCH-2.1).
  dedup_warn_score: 0.92
  // /import: max characters per embedded chunk of an imported document (R2-B).
  import_chunk_chars: 1500
}
```

- **Separate TUI** (`inkhaven research [--thread <name>]`): Facts tree (40%, navigate
  / pin / `n` add) + streaming RAG chat (60%) + a `/command` query prompt. `Tab`
  cycles focus, **F10** cycles the RAG mode (Facts+Full / Facts only / Full only).
- **Commands:** `/fact` / `/note` (extract the last response → an **editable
  confirmation overlay** → Facts / Notes; every insertion is reviewed), `/goto`,
  `/diff` (corpus similarity), `/verify` (claim confidence), `/chain` (sequential
  pipeline), `/rag` / `/clear` / `/save`. Inserts re-embed immediately, so new
  facts reach every consumer with no rebuild.
- **Threads** persist under `.inkhaven/research-threads/<slug>.json` (queries,
  insertions, pins, RAG mode); `--list-threads` / `--export-thread` (table/json/md)
  are non-interactive. **No facts.duckdb** — facts are paragraphs in the Facts book,
  indexed in the shared vector store. Zero new runtime crates.

See [Tutorial 103 — The Research Assistant](Tutorials/103-research-assistant.md).

## 1.4.10 — Jinja template paragraphs (STRUCT-1)

A paragraph with `content_type: "jinja"` is a [minijinja](https://docs.rs/minijinja)
template the **assembler renders to Typst** before `typst compile` runs (two
layers, never nested). Add one with **`e`** in the Tree pane — under a user book
for a data-driven manuscript template, or under the **Snippets** system book for
a reusable fragment that other templates `{% include "snippets/<path>.jinja" %}`.
(To convert an existing paragraph instead, cycle its type with `t`/`T`:
`typst → hjson → jinja → bund`.)

- **Context:** the template sees `title`, `slug`, `book.{title,slug,genre}`,
  `chapter.{title,slug}`, `language`, `genre`, and `linked["<slug>"].<field>` —
  HJSON data from paragraphs linked with `Ctrl+V a`. It is **self-gating**: no
  Jinja paragraphs, no behaviour change, no enable flag.
- **Editor:** tree glyph `⟡`, header badge `[jinja]`, Jinja syntax highlighting.
  Jinja paragraphs are skipped by the Inner Editor, Inner Socrates, and the idle
  fact-checker (markup, not prose).
- **Errors:** a render failure (bad syntax, missing include, typo'd variable)
  **aborts the whole assembly** by default with the offending paragraph + error —
  CI-safe, no silently-dropped content. Opt into continue-and-flag mode with:

  ```hjson
  jinja: {
    // false (default): a render error aborts Book assembly. true: write a
    // visible Typst error block into the paragraph's place and keep going,
    // so you can fix broken templates one at a time.
    continue_on_error: false
  }
  ```

  - `continue_on_error` (default `false`) — abort vs. visible-error-block-and-
    continue on a Jinja render failure.

See [JINJA_TEMPLATES.md](JINJA_TEMPLATES.md) and
[Tutorial 93 — Jinja templates](Tutorials/93-jinja-templates.md).

## 1.4.9 — Reusable content blocks (REUSE-1)

Write a warning, a procedure note, or an admonition once and reference it from
many places. Reusable prose lives as ordinary Typst paragraphs in a **Snippets**
system book; elsewhere you reference it with a standard Typst
`#include "../../snippets/<slug>.typ"`. At Book assembly inkhaven copies every
snippet to a `snippets/` sidecar next to the assembled book, so the include
resolves at `typst compile` — **no new Typst syntax**.

- **Authoring:** add a paragraph under the **Snippets** book (its slug is the
  include target); write any Typst. There is **no enable flag** — the feature is
  self-gating, and a project with no Snippets book pays nothing.
- **Insert/replace:** `Ctrl+V x` fuzzy-picks a snippet and inserts a
  depth-correct `#include` (or replaces the path of the include under the
  cursor). `Ctrl+V Shift+X` lists snippets + reference counts.
- **Validation:** a save-time check flags any `#include` whose snippet slug isn't
  defined (status bar / F8 / `Ctrl+V N`), gated on `typst_compile.diagnostics`
  (default on). `inkhaven snippets check` validates the whole project (exit 1 on
  a missing reference; orphaned snippets are warnings).
- **Scripting:** read-only `ink.snippets.{list,get,check}`.

The Snippets book is excluded from BOOK-RAG by default (reusable boilerplate is
not retrieval-useful prose); add `"snippets"` to `book_rag.include_system_books`
if you want snippet text searchable in Book-scope chat. See
[Tutorial 92 — Reusable snippets](Tutorials/92-reusable-snippets.md).

## 1.4.8 — Terminology governance (TERMS-1)

A **Glossary** system book holds canonical terms with their **banned synonyms**
as HJSON paragraphs. The editor red-underlines the synonyms in prose (so
"auth token" is flagged while the canonical "access token" is clean), and
`inkhaven terms check` scans the whole project for them. There is **no enable
flag** — the overlay is self-gating: an empty Glossary flags nothing.

A glossary entry (one HJSON paragraph under the Glossary book — press `P` there
and the title becomes the `term`):

```hjson
{
  term: access token
  definition: A short-lived credential.
  synonyms: [
    auth token
    authentication token
  ]
  // scope: global   // or a book slug to enforce only in that book
  // note: chosen for clarity
}
```

- `term` (required) — the canonical form to standardise on.
- `synonyms` — the banned forms, flagged in prose (1–3 words each).
- `scope` — `global` (default) or a book slug; `inkhaven terms check --book`
  honours it precisely (the live overlay applies the whole Glossary).

**Toggle:** `Ctrl+V z` flips the overlay; `Ctrl+V Shift+Z`, with the cursor on a
flagged synonym, declares its canonical term a **deliberate variant** (an intent
that stops flagging it). **Colour:** the theme key `style_warning_banned_synonym_fg`
(default red `#e05a5a`) sets the underline hue.

**CLI:** `inkhaven terms check [--book <slug>] [--json]` (exits non-zero on
findings — CI-ready); `inkhaven terms suggest [--book] [--auto-create]` proposes
glossary entries via the LLM. **Scripting:** read-only `ink.terms.{list,get,check}`
plus `ink.terms.declare_intent` (store_write). The Glossary book is excluded
from BOOK-RAG. See
[Tutorial 91 — Terminology governance](Tutorials/91-terminology-governance.md).

## 1.4.5 — Bibliography & citations (SOURCES-1)

### `sources` (1.4.5+) — the citation engine

Citations live in a system book — **Sources** — as structured **HJSON
paragraphs** (no new content type; the editor highlights them as HJSON). Each
new user book gets a matching Sources chapter, auto-seeded with one placeholder
entry. Cite in prose with Typst's `@key` syntax; at Book assembly (`Ctrl+B A`
or `inkhaven build`) every entry is compiled to `sources.bib` and a
`#bibliography(...)` line is appended, so `typst compile` resolves the
citations and renders the bibliography. The `sources` block tunes it:

```hjson
sources: {
  all: true                    // collect every entry, regardless of chapter
  bibliography_style: ieee     // Typst #bibliography style (CSL name)
  auto_bibliography: true      // append #bibliography(...) automatically
}
```

- `all` (default `true`) — when `true`, **all** entries under the Sources book
  are written to every assembled book's `sources.bib`. Set `false` to scope
  citations per book: only the Sources chapter **named after the assembled
  book** is collected (so a multi-book project keeps separate bibliographies).
  `inkhaven sources check` and the `Ctrl+V @` picker honour the same scope.
- `bibliography_style` (default `ieee`) — the style passed to Typst's
  `#bibliography(..., style: "…")`. Any CSL style name Typst accepts works
  (`apa`, `chicago-author-date`, `mla`, …).
- `auto_bibliography` (default `true`) — append the `#bibliography(...)` line
  during assembly. Set `false` if you'd rather place it by hand in your Typst
  setup (the `sources.bib` file is still written whenever entries exist).

An entry is **valid** once it has a non-empty `key`; keyless drafts are skipped
at compile time. Authoring paths: add a paragraph under Sources in the TUI
(its title becomes the cite key, body pre-seeded with the schema), or
`inkhaven sources import <file.bib>` to bulk-import BibTeX. Validate before a
build with `inkhaven sources check` (exits non-zero on any undefined `@key`).
Scripts get the read-only `ink.sources.{list,get,check,bibtex}` inspectors
(all `store_read`). See
[Tutorial 89 — Bibliography & Citations](Tutorials/89-bibliography-and-citations.md).
