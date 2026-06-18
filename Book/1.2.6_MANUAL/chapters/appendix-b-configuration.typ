#import "../design.typ": *

#appendix(letter: "B", title: "Configuration reference")

#dropcap("E")very HJSON knob, by stanza. The canonical
machine-readable reference lives at
`Documentation/CONFIGURATION.md`; this appendix is the
condensed printed companion.

#section("Top-level")

```hjson
{
  language: "english"
  prompts_file: "prompts.hjson"
  artefacts_directory: ""           # empty → sibling
  sync_interval_seconds: 5
}
```

#section("editor")

```hjson
editor: {
  typewriter_sounds:        false
  autosave_idle_seconds:    30
  word_wrap:                true
  startup_splash:           true
  paragraph_target_default: 250
  stemming: { languages: ["english"] }
}
```

#section("llm — AI providers")

```hjson
llm: {
  default_provider: "ollama"

  ollama:  { model: "qwen2.5:7b", host: "http://localhost:11434" }
  gemini:  { model: "gemini-2.0-flash-exp" }
  claude:  { model: "claude-sonnet-4-6" }
  openai:  { model: "gpt-4o-mini" }
  deepseek:{ model: "deepseek-chat" }
  grok:    { model: "grok-2-latest" }
}
```

API keys come from environment variables: `GEMINI_API_KEY`,
`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `DEEPSEEK_API_KEY`,
`XAI_API_KEY`. Ollama needs none.

#section("ai (1.2.6+)")

```hjson
ai: {
  per_paragraph_memory:           false
  per_paragraph_memory_max_turns: 10
  diff_review_on_apply:           true
  reseed_prompt_examples:         true
}
```

#section("typst_compile")

```hjson
typst_compile: {
  engine: "external"             # or "inprocess"
  bundle_fonts: true
  use_system_fonts: true
  packages_enabled: true
  diagnostics_idle_seconds: 2.0
  diagnostics_max: 50
}
```

#section("typst_page / typst_fonts / typst_layout")

```hjson
typst_page: {
  paper: "us-letter"            # or "a4", "iso-b5", …
  margin: { top: "2.5cm", bottom: "2.5cm",
            left: "2cm", right: "2cm" }
}

typst_fonts: {
  body: "Linux Libertine"
  size: "11pt"
}

typst_layout: {
  par: { justify: true, indent: "1em" }
}
```

#section("images")

```hjson
images: {
  preview_enabled: true
}
```

False when the host terminal can't render images — kills the
half-block fallback that's usually worse than no preview.

#section("output")

```hjson
output: {
  extra_formats: ["markdown", "epub"]
  epub_author: "Vladimir Ulogov"
}
```

#section("goals")

```hjson
goals: {
  daily_words: 800
  morning_baseline: true
  streak_grace: 1
  per_book_deadline: {
    "my-first-book": "2026-08-31"
  }
}
```

#section("timeline (1.2.6+)")

```hjson
timeline: {
  enabled: true
  default_track: "main"
  calendar: { preset: "gregorian" }    # or "sols" / "custom"
  display: {
    show_orphans: true
    swim_lane_max_rows: 12
    default_zoom: 1.0
  }
}
```

Custom calendar block — see Chapter 17 + 5 for the full
shape.

#section("backup")

```hjson
backup: {
  out_dir: ""              # empty → sibling
  max_age: "24h"           # "0s" disables auto-backup
}
```

#section("sound")

```hjson
sound: {
  typewriter_enabled: false
  return_chime: true
}
```

#section("scripting")

```hjson
scripting: {
  enabled_categories: []
  no_default_deny: false
  bootstrap: ""
}
```

Categories: `store_read`, `store_write`, `fs_read`,
`fs_write`, `net`, `shell`, `code_eval`, `keymap`, `ai_write`,
`editor_write`, `theme_write`. Default deny:
`fs_write`, `net`, `shell`, `code_eval`, `keymap`.

#section("keys")

```hjson
keys: {
  meta_prefix:  "Ctrl+B"
  bund_prefix:  "Ctrl+Z"
  view_prefix:  "Ctrl+V"
  bindings: [
    # { chord, action } pairs
  ]
}
```

#section("theme")

The full list of theme keys is long; see
`Documentation/CONFIGURATION.md`. Most-used:

```hjson
theme: {
  bg:                "#1e1e2e"
  fg:                "#cdd6f4"
  current_line_bg:   "#313244"
  selection_bg:      "#414559"
  status_bar_bg:     "#181825"
  modal_border:      "#cba6f7"
  modal_bg:          "#181825"
  modal_fg:          "#cdd6f4"
  tree_book_fg:      "#cba6f7"
  tree_chapter_fg:   "#f9e2af"
  tree_subchapter_fg:"#fab387"
  tree_paragraph_fg: "#cdd6f4"
  tree_script_fg:    "#cba6f7"
  tree_image_fg:     "#94e2d5"
  tree_open_marker:  "#a6e3a1"
  editor_character_fg: "#94e2d5"
  editor_place_fg:     "#f9e2af"
  editor_artefact_fg:  "#cba6f7"
  syntax_heading:    "#f38ba8"
  syntax_emphasis:   "#cdd6f4"
  syntax_strong:     "#fab387"
  syntax_function:   "#89dceb"
  syntax_operator:   "#94e2d5"
  syntax_list_marker:"#cba6f7"
  syntax_raw:        "#fab387"
  syntax_tag:        "#89b4fa"
  syntax_quote:      "#9399b2"
  line_number_fg:    "#5d5d5d"
  grammar_change_fg: "#a6e3a1"
  status_napkin:     "#9399b2"
  status_first:      "#fab387"
  status_second:     "#f9e2af"
  status_third:      "#a6e3a1"
  status_final:      "#89dceb"
  status_ready:      "#cba6f7"
}
```
