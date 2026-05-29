# 36 — Editing the project config from inside Inkhaven

`Ctrl+B 0` opens `<project>/inkhaven.hjson` in a full-
screen modal editor.  No paragraph chrome — no typst
diagnostics, no lexicon hits, no gutter-diff overlay —
just a text area with HJSON syntax highlighting so you can
tweak the project's configuration without dropping back to
a terminal.

This tutorial covers when to use the in-TUI editor vs an
external editor, the editing chords, the save flow, and
what happens when you change something the running editor
already has loaded.

## Opening the editor

From any pane:

```
Ctrl+B 0
```

The editor floats over the rest of the TUI.  The title
shows the absolute path; a `• [modified]` chip appears
once you start typing.

If `<project>/inkhaven.hjson` doesn't exist yet, the modal
opens with an empty buffer.  The status line announces
`hjson: ... not found — opening empty buffer (Ctrl+S to
create)` — saving will create the file.

## Editing

The modal mirrors the main paragraph editor's chord set
so muscle memory carries over:

| Chord                | Action                              |
| -------------------- | ----------------------------------- |
| Arrows               | cursor motion                        |
| `Home` / `End`       | line start / end                     |
| `PgUp` / `PgDn`      | paragraph-back / paragraph-forward   |
| `Shift+arrow`        | extend selection                     |
| `Ctrl+Home` / `End`  | document top / bottom                |
| `Ctrl+Left` / `Right`| word back / forward                  |
| `Ctrl+Backspace`     | delete word                          |
| `Ctrl+U` / `Ctrl+Y`  | undo / redo                          |
| `Ctrl+K`             | cut                                  |
| `Ctrl+C`             | copy                                 |
| `Ctrl+P`             | paste                                |
| `Ctrl+A`             | select all                           |
| `Ctrl+D`             | delete line                          |
| `Ctrl+E`             | delete to end of line                |
| `Ctrl+W`             | delete to start of line              |
| `Ctrl+S`             | save                                 |
| `Esc`                | close (warns on unsaved edits)       |

Plain typing, `Tab`, `Enter`, `Backspace`, `Delete` work
as you'd expect.

## Syntax highlighting

HJSON tokens are coloured by the same lexer (`hjson_highlight`)
that the regular editor uses for `.hjson` paragraphs:

- **Keys** (`foo:`) — function colour
- **Quoted strings** (`"value"`) — string colour
- **Numbers** (`42`, `1.5`) — number colour
- **Keywords** (`true`, `false`, `null`) — keyword colour, bold
- **Line comments** (`// …`, `# …`) — comment colour
- **Block comments** (`/* … */`) — comment colour, multi-line
- **Triple-quoted strings** (`''' … '''`) — string colour, multi-line

Brackets and punctuation stay in the pane foreground so
the structure stands out without screaming.

## Saving

`Ctrl+S` writes the buffer to `inkhaven.hjson`.  Two paths:

- **No actual change since open** — `hjson: saved (no changes
  since last save)` flashes on the status line; the modal
  stays open and you keep editing.

- **Bytes differ from the original** — a centered
  *Restart required* overlay pops on top of the editor:

  ```
  ┌─ Restart required ─┐
  │                    │
  │  Config changed    │
  │                    │
  │  inkhaven.hjson    │
  │  has been written  │
  │  to disk.          │
  │                    │
  │  The running       │
  │  editor is still   │
  │  using the OLD     │
  │  config — restart  │
  │  inkhaven to apply │
  │  your changes.     │
  │                    │
  │  Press any key to  │
  │  dismiss           │
  └────────────────────┘
  ```

  Any key dismisses the overlay; the modal stays open so
  you can continue editing.  Subsequent saves of the same
  bytes don't re-fire the overlay (the baseline updates on
  each save) — only fresh changes do.

## Why the restart-required overlay?

Inkhaven loads `inkhaven.hjson` once at startup.  The
configuration is then frozen into the running `Config`
struct that every pane consults for theme colours, AI
provider routing, autosave intervals, shell defaults, and
so on.  Live-reloading the config is intentionally NOT
supported: hot-swapping a theme colour or a chord binding
mid-session creates surprising inconsistencies (panes
already painted with the old colour, chord history
trained on the old binding).

The overlay is informational only.  The save itself
succeeded — your next `inkhaven` launch picks up the new
values.

## Closing without saving

`Esc` closes the modal.  When the buffer has unsaved
edits the status line warns:

```
hjson: closed with unsaved edits — re-open with Ctrl+B 0 to recover from disk
```

The on-disk file is untouched.  Reopening with
`Ctrl+B 0` re-reads from disk, so the unsaved buffer
contents are gone — but you can also use your shell's
undo (the file's previous version is whatever you last
saved).

## When to use the in-TUI editor

The modal is best for quick tweaks:

- Trying a new theme (`theme: dark_warm`)
- Adjusting the autosave interval (`editor.autosave_seconds: 10`)
- Adding to the shell blocklist (`shell.blocked_externals: [...]`)
- Toggling a feature flag (`editor.confirm_quit: true`)

For larger refactors — restructuring AI provider configs,
hand-writing a complex regex, editing your themes' colour
palette — drop to an external editor.  The HJSON file is
plain text, version-controlled if you opt in, and
inkhaven re-reads it on every launch.

## Differences from the paragraph editor

| Feature                       | Paragraph editor       | HJSON editor             |
| ----------------------------- | ---------------------- | ------------------------ |
| Syntax highlighting           | Typst / Bund / HJSON   | HJSON only               |
| Typst diagnostics             | Yes (`●` gutter)       | No                        |
| Lexicon hits / Places / Chars | Yes                    | No                        |
| Match overlay (`Ctrl+F`)      | Yes                    | No                        |
| Diff-since-save overlay       | Yes                    | No                        |
| Snapshot / `F5`               | Yes                    | No                        |
| Read-only mode                | Help book only         | Never (config is editable)|
| Restart-required after save   | No                     | Yes                       |

The HJSON editor is intentionally lean.  It exists for
the config file specifically; if you want a richer Bund
or HJSON editing experience use the regular tree-opened
paragraph route.

## 1.2.12 additions

- **`Ctrl+R` fires an LLM review** of the current
  buffer.  Same "reviewer LLM, not executor" pattern
  the standalone prompts-editor TUI uses (1.2.10).
  The model critiques the HJSON as a piece of work:
  invalid combinations, dangerous defaults, fields
  that look misnamed, fields that should probably
  be set but aren't.  The system prompt asks the
  reviewer to quote dotted field paths
  (`editor.style_warnings.show_dont_tell.enabled`,
  not "the SDT field") so the critique is
  concretely actionable.  The response streams
  into `App.inference` and is visible in the
  regular AI pane once you close the modal (Esc).
  Status bar tracks progress while the modal is up.
  Use it as a "second opinion" before saving a
  config change you're unsure about.
