# 27 — Theming and the cheat sheet

Every colour in the inkhaven TUI is configurable. The defaults ship Catppuccin Mocha — a popular warm-dark palette — but every knob is exposed in `inkhaven.hjson`.

## The theme stanza

```hjson
theme: {
  bg:                "#1e1e2e"      # base background
  fg:                "#cdd6f4"      # default text
  current_line_bg:   "#313244"
  selection_bg:      "#414559"
  status_bar_bg:     "#181825"
  modal_border:      "#cba6f7"
  # … many more
}
```

Hex strings with the `#` prefix. Inkhaven applies them at boot. The full list is in Appendix B.

## Quick palette swap

To switch the whole TUI to Gruvbox Dark:

```hjson
theme: {
  bg:           "#282828"
  fg:           "#ebdbb2"
  current_line_bg: "#3c3836"
  selection_bg: "#504945"
  modal_border: "#fb4934"
  # … customise to taste
}
```

To switch to Solarised Light:

```hjson
theme: {
  bg: "#fdf6e3"
  fg: "#586e75"
  # … light-theme adjustments
}
```

## Per-syntax colours

The editor highlights typst syntax — headings, function calls, code blocks. Theme keys for those:

| Key | What it colours |
|-----|-----------------|
| syntax_heading | Heading text. |
| syntax_emphasis / syntax_strong | Italic / bold text. |
| syntax_function | `#function(...)` calls. |
| syntax_operator | Operators. |
| syntax_list_marker | `- ` / `+ ` markers. |
| syntax_raw | Inline + block code. |
| syntax_quote | Block quotes. |

## Lexicon overlay colours

Character / Place / Artefact highlights:

| Key | Default |
|-----|---------|
| editor_character_fg | Character match (cyan). |
| editor_place_fg | Place match (yellow). |
| editor_artefact_fg | Artefact match (mauve). |

## Editor knobs (other)

```hjson
editor: {
  typewriter_sounds:        false   # tap/return sound effects
  autosave_idle_seconds:    30
  word_wrap:                true
  splash:                   true    # show progress-pulse on launch
  paragraph_target_default: 250
}
```

![figure: startup-pulse-splash](images/startup-pulse-splash.png) — Startup pulse splash: today's words, current streak, active time, by-status counts. Auto-closes after 7s or any key.

## The cheat sheet

`Ctrl+B H` opens the quick-reference overlay — every chord relevant to the current pane:

![figure: ctrl-b-h-cheat](images/ctrl-b-h-cheat.png) — Ctrl+B H: pane-aware quick reference. Scoped to current focus + the layer-aware chord tables.

The printed cheat sheet (`Documentation/INKHAVEN_CHEAT_SHEET.typ`) compiles to a two-column A4 PDF with every chord, hook, and CLI subcommand. Print it; hang it next to the keyboard.

## Credits

`Ctrl+B V` opens the credits pane — version, dependencies, the embedded logo, and a one-liner about the typst engine in use.

## Tutorial-by-tutorial colour examples

The full `Documentation/Tutorials/11-theming.md` walks through every theme key with before/after screenshots. Worth a read once you start customising in earnest.

## Recap

- Theme stanza in HJSON — every colour configurable.
- Per-syntax knobs for editor highlighting; per-lexicon knobs for the overlay.
- `Ctrl+B H` — pane-aware cheat sheet on demand.
- `Documentation/INKHAVEN_CHEAT_SHEET.typ` compiles to a printable two-column A4 reference.
- `Ctrl+B V` — credits + logo.
