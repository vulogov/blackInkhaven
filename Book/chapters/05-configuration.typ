#import "../design.typ": *

#chapter(number: 5, part: "Part I — Foundations",
  title: "Configuring the project")

#dropcap("E")very project has one file that sets the rules for
the rest: `inkhaven.hjson`. HJSON is JSON with comments and
trailing-comma forgiveness — friendly to write by hand.

The file lives at the project root. Edit it with any text
editor; changes take effect on the next inkhaven launch.

#section("What lives in the config")

The default file is heavily commented. Major blocks:

#chord_table((
  chord_row("language", "Primary writing language. Drives stemmer choice + the F7 grammar prompt default."),
  chord_row("editor", "Word-wrap, autosave idle, status-bar shape, sounds, theme. See Chapter 27."),
  chord_row("llm", "AI provider configuration. Six providers; one default. See Chapter 18."),
  chord_row("ai", "AI behaviour: per-paragraph memory, diff review on apply, prompt-example seeding. See Chapter 21."),
  chord_row("typst_compile", "External CLI vs in-process engine; fonts; package resolution. See Chapter 23."),
  chord_row("typst_page / typst_fonts / typst_layout", "Page size, fonts, par layout for the bundled templates."),
  chord_row("output", "Multi-format export defaults (epub metadata, markdown variant, …). See Chapter 25."),
  chord_row("goals", "Word-count targets + streak tracking. See Chapter 9."),
  chord_row("timeline", "Story-timeline feature: enable, default track, calendar config. See Chapter 17."),
  chord_row("backup", "Auto-backup-on-exit destination + max-age. See Chapter 11."),
  chord_row("images", "Terminal-graphics support (kitty / iterm2 / sixel / half-block fallback)."),
  chord_row("scripting", "Bund sandbox policy + bootstrap script. See Chapter 29."),
  chord_row("keys", "Chord rebinding overlay. See Chapter 28."),
  chord_row("theme", "Colour palette. See Chapter 27."),
))

#section("A real example — minimal")

```hjson
{
  language: "english"

  llm: {
    default_provider: "ollama"
    ollama: { model: "qwen2.5:7b" }
  }

  editor: {
    typewriter_sounds: false
  }

  typst_compile: {
    engine: "inprocess"
  }

  timeline: {
    enabled: true
    calendar: { preset: "gregorian" }
  }
}
```

This minimal stanza: English as the language, local Ollama
for AI (no internet required), no typewriter audio, the
bundled in-process typst compiler, and the timeline feature
enabled with gregorian dates. Three minutes of config and
you have a working setup.

#section("Editing safely")

The TUI doesn't reload the config on the fly. Edit
`inkhaven.hjson`, quit (`Ctrl+Q`), launch again.

The full reference lives at `Documentation/CONFIGURATION.md`.
Appendix B in this book is an abbreviated reference card —
every knob and its default, no examples.

#callout(label: "Safe to break")[
  If you make `inkhaven.hjson` invalid, inkhaven refuses to
  start with a clear error pointing at the line. Your prose
  is never at risk; the config is a separate file from the
  database. Worst case: revert your edit, launch again.
]

#section("Project-level vs per-book")

Most configuration is project-level — one file for the whole
directory. A few things are per-book:

#chord_table((
  chord_row("Book title / slug", "Set with `inkhaven add book` or F2 on the book node."),
  chord_row("Per-book typst skeleton", "`books/<slug>/globals.typ`, `settings.typ`, `index.typ` — created automatically."),
  chord_row("Per-book Timeline chapter", "Lazily created on first `event add` for that book."),
))

The per-book typst files (`globals.typ`, `settings.typ`,
`index.typ`) are where you put book-specific `#set` and
`#show` rules. The build pipeline imports them into the
assembled document automatically.

#recap((
  [`inkhaven.hjson` at the project root is the single config file.],
  [Comments + trailing commas allowed (HJSON, not strict JSON).],
  [Restart inkhaven to pick up changes.],
  [Appendix B summarises every knob; full reference in `Documentation/CONFIGURATION.md`.],
  [Per-book customisation lives in `books/<slug>/globals.typ` + `settings.typ`.],
))
