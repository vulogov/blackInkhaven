# 5 — Configuring the project

Every project has one file that sets the rules for the rest: `inkhaven.hjson`. HJSON is JSON with comments and trailing-comma forgiveness — friendly to write by hand.

The file lives at the project root. Edit it with any text editor; changes take effect on the next inkhaven launch.

## What lives in the config

The default file is heavily commented. Major blocks:

| Block | Role |
|-------|------|
| `language` | Primary writing language. Drives stemmer choice + the F7 grammar prompt default. |
| `editor` | Word-wrap, autosave idle, status-bar shape, sounds, theme. See Chapter 27. |
| `llm` | AI provider configuration. Six providers; one default. See Chapter 18. |
| `ai` | AI behaviour: per-paragraph memory, diff review on apply, prompt-example seeding. See Chapter 21. |
| `typst_compile` | External CLI vs in-process engine; fonts; package resolution. See Chapter 23. |
| `typst_page / typst_fonts / typst_layout` | Page size, fonts, par layout for the bundled templates. |
| `output` | Multi-format export defaults (epub metadata, markdown variant, …). See Chapter 25. |
| `goals` | Word-count targets + streak tracking. See Chapter 9. |
| `timeline` | Story-timeline feature: enable, default track, calendar config. See Chapter 17. |
| `backup` | Auto-backup-on-exit destination + max-age. See Chapter 11. |
| `images` | Terminal-graphics support (kitty / iterm2 / sixel / half-block fallback). |
| `scripting` | Bund sandbox policy + bootstrap script. See Chapter 29. |
| `keys` | Chord rebinding overlay. See Chapter 28. |
| `theme` | Colour palette. See Chapter 27. |

## A real example — minimal

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

This minimal stanza: English as the language, local Ollama for AI (no internet required), no typewriter audio, the bundled in-process typst compiler, and the timeline feature enabled with gregorian dates. Three minutes of config and you have a working setup.

## Editing safely

The TUI doesn't reload the config on the fly. Edit `inkhaven.hjson`, quit (`Ctrl+Q`), launch again.

The full reference lives at `Documentation/CONFIGURATION.md`. Appendix B in this book is an abbreviated reference card — every knob and its default, no examples.

> **Safe to break:** If you make `inkhaven.hjson` invalid, inkhaven refuses to start with a clear error pointing at the line. Your prose is never at risk; the config is a separate file from the database. Worst case: revert your edit, launch again.

## Project-level vs per-book

Most configuration is project-level — one file for the whole directory. A few things are per-book:

| Item | Where it lives |
|------|----------------|
| Book title / slug | Set with `inkhaven add book` or F2 on the book node. |
| Per-book typst skeleton | `books/<slug>/globals.typ`, `settings.typ`, `index.typ` — created automatically. |
| Per-book Timeline chapter | Lazily created on first `event add` for that book. |

The per-book typst files (`globals.typ`, `settings.typ`, `index.typ`) are where you put book-specific `#set` and `#show` rules. The build pipeline imports them into the assembled document automatically.

## Recap

- `inkhaven.hjson` at the project root is the single config file.
- Comments + trailing commas allowed (HJSON, not strict JSON).
- Restart inkhaven to pick up changes.
- Appendix B summarises every knob; full reference in `Documentation/CONFIGURATION.md`.
- Per-book customisation lives in `books/<slug>/globals.typ` + `settings.typ`.
