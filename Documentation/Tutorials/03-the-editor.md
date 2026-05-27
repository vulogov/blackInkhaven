# 3 — The editor

The editor pane is where you spend most of your time. This tutorial
covers movement, selection, find/replace, snapshots, split-edit, and
the visual feedback the editor gives you about save / dirty / read-only
state.

The keyboard is the primary interface. Everything works without the
mouse, but click and scroll work too (see
[`../KEYBINDING.md`](../KEYBINDING.md) §0).

## Opening a paragraph

In the Tree pane, navigate to a paragraph row and press `Enter`. Focus
moves to the Editor pane and the paragraph's `.typ` file loads. The
border colour reflects state:

- **Green** — saved (clean)
- **Yellow** — modified (dirty)
- **Teal** — read-only (Help subtree)
- **Plain border** — pane is unfocused

The pane title shows the paragraph's title, the dirty / read-only
chip, and the live `L<row> C<col>` cursor read-out (in sky-blue by
default — `theme.editor_position_fg`):

```
Editor — Opening Scene · L4 C18
```

## Movement

| Key | Action |
| --- | ------ |
| arrows | Move cursor one cell |
| `Ctrl+Left` / `Ctrl+Right` | Move one word |
| `Home` / `End` | Start / end of line |
| `Ctrl+Home` / `Ctrl+End` | Top / bottom of buffer |
| `PageUp` / `PageDown` | One viewport |
| `Shift+arrows` | Extend linear selection |

Mouse:

- Left-click positions the cursor where you click (gutter clicks are
  ignored).
- Scroll wheel scrolls by 3 lines.

## Selection

Two selection modes:

### Linear selection

Hold Shift while moving the cursor. Standard text-editor behaviour.
The selection range is highlighted with `REVERSED` background. Cut /
copy / paste operate on this selection.

### Vertical block selection

For multi-line column edits. The flow:

1. `Alt+arrow` enters block-select mode and starts an anchor at the
   current cursor. The selection forms a rectangle from the anchor
   to the cursor.
2. Keep moving with `Alt+arrows` to grow the rectangle.
3. `Alt+C` copies the rectangle to the system clipboard as a
   multi-line string (one row per line). Anchor cleared.
4. `Esc` cancels without copying.

Rectangular **paste** is not yet supported in this release — block
mode is copy-only.

## Clipboard

Inkhaven uses the system clipboard via `arboard`. Behaviour fallbacks
gracefully if no clipboard is available (e.g. headless SSH session)
— operations still work using tui-textarea's internal yank buffer.

| Key | Action |
| --- | ------ |
| `Ctrl+C` | Copy selection to system clipboard |
| `Ctrl+K` | Cut selection |
| `Ctrl+P` | Paste at cursor |
| `Ctrl+A` | Select all |

(`Ctrl+V` would conflict with terminal paste bindings on many setups
— `Ctrl+P` is the explicit Inkhaven paste.)

## Edits

| Key | Action |
| --- | ------ |
| `Ctrl+U` | Undo |
| `Ctrl+Y` | Redo |
| `Ctrl+D` | Delete current line |
| `Ctrl+E` | Delete from cursor to end of line |
| `Ctrl+W` | Delete from cursor to start of line |
| `Ctrl+Backspace` | Delete previous word |

`Ctrl+Z` is **intentionally unbound** (it conflicts with the shell's
job-control SIGTSTP on many setups). Use `Ctrl+U` for undo.

## Save

`Ctrl+S` writes the `.typ` file to disk, updates bdslib metadata,
re-embeds the content, and clears the dirty flag. The status bar
reports:

```
saved books/sample-novel/01-chapter-one/01-opening.typ (412 words, re-embedded)
```

Three save triggers:

1. **Explicit** — `Ctrl+S`.
2. **Idle autosave** — after `editor.autosave_seconds` of typing
   inactivity (default 5 s). Disabled when a grammar-correction
   highlight is active.
3. **Implicit on focus / paragraph switch** — autosaves a dirty
   paragraph when you switch panes or open another paragraph.

The title chip `[modified]` appears when dirty:

```
Editor — Opening Scene [modified] · L4 C18
```

## The visual overlays

The editor renders several overlays on top of plain Typst text. They
compose:

| Overlay | Source | Style |
| ------- | ------ | ----- |
| Syntax highlight | tree-sitter-typst | Per-token colours from `theme.syntax_*` |
| Lexicon (Places / Characters) | system books | cyan / yellow bold (see [LOCATIONS.md](../LOCATIONS.md)) |
| Added-since-save | LCS diff vs. `saved_lines` | bold |
| Grammar-correction | LCS diff vs. correction baseline | `theme.grammar_change_fg` red |
| Find / replace match | regex hits | `theme.search_match_bg` |
| Current find hit | regex cursor | `theme.search_current_bg` |
| Current line | cursor row | `theme.current_line_bg` background |
| Selection | tui-textarea selection_range | REVERSED |

You can leave them all on; conflicts resolve in a deterministic order
so the most actionable cue wins (selection > search match > grammar
diff > lexicon > syntax).

## Focus mode (distraction-free)

`Ctrl+B W` hides every other pane — Tree, AI, Search, AI-prompt —
and gives the editor the full window. Useful for long drafting
sessions where the cross-pane chrome is more distraction than
help. Re-press to restore the four-pane layout. The chord is
internally still called "typewriter mode" in some log strings
and the HJSON serde key (`global.toggle_typewriter`); the chord
binding has been there since early 1.2 but the documentation
calls it "focus mode" from 1.2.9 onward because that's what it
actually does.

Mutually exclusive with `Ctrl+B K` (AI-fullscreen). Toggling
either turns the other off — there's no "AI + editor split with
everything else hidden" mode by design.

## Style warnings (filter words)

`Ctrl+B Shift+F` toggles an inline overlay that
underlines stylistically weak words in amber.  Today
the overlay flags **filter words** — intensifier
crutches (`just`, `really`, `very`), hedges (`seemed`,
`felt`, `appeared`), and generic placeholders
(`actually`, `basically`).  The writer's job is to
question whether each flagged word earns its place;
none of them are always-delete.

Built-in word lists ship for `english`, `russian`,
`french`, `german`, `spanish`.  The active list is
selected by the project's top-level `language` field —
no per-paragraph language switching.  Russian users
get a curated list of `очень / просто / именно /
довольно / казалось / выглядел / вдруг / возможно`
+ more.

Add your own:

```hjson
editor: {
  style_warnings: {
    enabled: true
    filter_words: {
      enabled: true
      extra_words: ["lifted", "shifted", "blinked"]
    }
  }
}
```

Words in `extra_words` apply on top of the language
default — case-insensitive, exact-word match (no
substring partials, so `lifted` won't flag
`shoplifted`).

The overlay composes with other editor highlights:
selection still reverses, search match still wins, the
filter underline persists underneath.  No performance
cost — the detector runs once per render frame on the
visible rows and the comparison is a `HashSet<String>`
lookup per Unicode-segmented word.

Future detectors (show-don't-tell, sentence rhythm)
will share the same toggle.

### Repeated phrases

The repeated-phrase detector flags every occurrence of
any 4-word phrase (configurable via
`style_warnings.repeated_phrases.n`) that appears 3+
times in the open paragraph (configurable via
`threshold`).  Marker colour: soft magenta + underline.

Snowball stemming aligns inflections before comparison
— `she lifted her shoulders` matches `she was lifting
her shoulders`.  Closed-class words (`the`, `and`,
`и`, `в`) are filtered via a per-language stop list so
common connectives don't inflate the count.  Built-in
stop lists ship for the five supported languages;
override via `<lang>_stop_words`.

```hjson
editor: {
  style_warnings: {
    repeated_phrases: {
      enabled: true
      n: 4
      threshold: 3
      use_stemming: true
    }
  }
}
```

Useful for catching writer-crutch gestures and
favourite turns of phrase you didn't realise had
become reflexive.  Toggles with `Ctrl+B Shift+F`
alongside filter-words.

## Read aloud (TTS)

`Ctrl+B S` (in the editor pane) speaks the open paragraph
through the host OS's text-to-speech engine.  Useful for
catching awkward phrasing that the eye glosses over —
reading prose aloud is the single best self-edit
technique that doesn't need another person.

The feature is **off by default**.  Enable it by adding
to `inkhaven.hjson`:

```hjson
editor: {
  tts: {
    enabled: true
    voice: "Milena"   // Russian female; the default
    speed: 1.0        // multiplier over the engine's "normal" rate
    greeting: ""      // spoken at startup; empty skips
    goodbye: ""       // spoken at shutdown; empty skips
  }
}
```

### Greeting + goodbye

When `enabled = true` and either field is non-empty,
inkhaven speaks the configured text at startup
(non-blocking, plays in parallel with the editor
coming up) and at shutdown (blocking, up to 5
seconds, so the shell doesn't truncate it).

Examples:

```hjson
greeting: "Welcome back"
goodbye:  "See you tomorrow"

# or, with voice: "Katya (Enhanced)":
greeting: "Доброе утро, Владимир"
goodbye:  "До скорого"
```

Keep the goodbye short — under five seconds of audio
— so quit doesn't feel slow.  The greeting can be
longer; it overlaps with the editor's startup work.

### Platform support

| Platform | Backend                 | Russian voices                                                      |
| -------- | ----------------------- | ------------------------------------------------------------------- |
| macOS    | AVFoundation            | Milena (default), Yuri.  Katya (Enhanced) for premium quality.       |
| Windows  | SAPI / WinRT            | Pavel (male), Irina (female).  Pre-installed on Windows 10+.        |
| Linux    | Speech Dispatcher       | Depends on the configured speechd backend (espeak-ng default = robotic; install RHVoice or piper for natural Russian). |

On macOS and Windows, voices ship with the OS — only a
one-time download via system settings is needed for
non-English languages.  On Linux you also need to
install the speech-dispatcher daemon (`apt install
speech-dispatcher` or distro equivalent) and may want
to swap its default backend for natural Russian
output.

### Voice selection

`tts.voice` is a case-insensitive substring match
against installed voice names.  `"Milena"` matches both
`Milena` and `Milena (Enhanced)` / `Milena (Premium)`;
the matcher prefers entries that also contain
`Enhanced` or `Premium`, so the premium variant is
picked automatically when installed.

Other voices: `"Yuri"`, `"Katya"`, `"Daniel"`, `"Karen"`,
etc.  Run **`inkhaven doctor --voices`** to print every
voice visible to the TTS engine on this machine —
name, language, gender, one per line.  Works on every
platform `tts-rs` supports.

### Playback UI

While speech is in progress, a `Read aloud` modal floats
over the editor showing:

- A spinner + the elapsed time
- The chosen voice
- The first 80 chars of the paragraph as a reminder of
  what's being read

Any key (Esc / Space / anything) stops playback
immediately.  The modal also closes automatically when
the paragraph finishes naturally.

### What gets read

The full paragraph body, with the leading typst heading
line (`= Title`) stripped — it's structural, not prose.
Empty / whitespace-only paragraphs surface a status
warning and don't open the modal.

### Performance

The TTS engine is lazily initialised on the first
`Ctrl+B S` and cached for the rest of the session —
subsequent reads skip the init cost.  Engine init
failures (Linux without speech-dispatcher, etc.) are
cached too, so a missing-engine modal doesn't pay the
init cost on every keystroke.

## Find and replace (regex)

`Ctrl+F` opens the Find overlay:

```
┌── Find (regex) ─────────────────────────────────┐
│                                                 │
│  Search: │                                      │
│                                                 │
│  Enter find · Esc cancel                        │
└─────────────────────────────────────────────────┘
```

Type a pattern (Rust regex syntax — `(?i)` for case-insensitive,
`(?s)` for dotall, `\bword\b` for word boundaries), press Enter.
Every match in the buffer is painted; the cursor jumps to the first.

- `Ctrl+X` jumps to the next match (wraps at end).
- `Esc` clears the search overlay (back to plain editing).

For find-and-replace, press `Ctrl+R` instead:

```
┌── Find & Replace (regex) ───────────────────────┐
│                                                 │
│  > Search:  the\s+thunder│                      │
│            Replace: the storm                   │
│                                                 │
│  Enter run · Tab switch field · Esc cancel      │
└─────────────────────────────────────────────────┘
```

`Tab` switches between Search and Replace fields. Enter performs one
replacement at the current match; `Ctrl+R` again (while replace is
active) does **replace all** and dismisses. `Ctrl+X` advances to the
next match.

The regex is the standard Rust crate's syntax — see
[https://docs.rs/regex/latest/regex/](https://docs.rs/regex/latest/regex/)
for the full reference.

## Snapshots (F5 / F6)

Snapshots are versioned copies of a paragraph stored alongside it in
the database. They're separate from autosave — autosave just commits
the latest version; snapshots capture point-in-time bookmarks.

- `F5` creates a fresh snapshot of the current buffer. The status
  reports `snapshot N saved`.
- `F6` opens the snapshot history picker:

  ```
  ┌── Snapshots for `Opening Scene` ────────────────┐
  │  > 3   2026-05-19 14:32   412 words            │
  │    2   2026-05-19 14:05   401 words            │
  │    1   2026-05-19 13:48    98 words            │
  │                                                  │
  │  Enter to load · Esc to cancel                  │
  └──────────────────────────────────────────────────┘
  ```

  Arrow keys move the selection; Enter loads that snapshot into the
  buffer (marking it dirty so you can decide whether to keep it).

Snapshots live forever — Inkhaven never garbage-collects them.

## Split-edit mode (F4 / Ctrl+F4)

For when you want to see two versions of a paragraph side by side.
`F4` toggles split-edit; the editor pane splits horizontally:

- **Upper half** — your live editor (read-write).
- **Lower half** — a frozen snapshot of the buffer at the moment you
  pressed F4. Read-only. Dim text.

Scroll the lower pane independently with `Ctrl+H` (up) and `Ctrl+J`
(down). The live cursor in the upper half moves with normal keys.

When you're done:

- `F4` again closes split, dropping the snapshot.
- `Ctrl+F4` **accepts** — the lower pane's content replaces the live
  buffer. Useful for rolling back to the pre-edit version after a
  destructive change you decided you didn't want.

## File picker (F3) — load a file into the buffer

`F3` opens a file picker overlay rooted at your home directory.
Navigate with arrows; `→` expands a directory, `←` collapses (or
steps to the parent). `Enter` on a file **replaces the current buffer**
with that file's content. Mark dirty (not saved yet).

Useful for pulling drafts from `~/Drafts/` directly into Inkhaven.

For importing whole directory trees into the hierarchy, use the
Tree pane's F3 (different overlay context — creates new paragraphs)
or the `inkhaven import-help` CLI.

## Read-only mode (Help subtree)

If you open a paragraph that lives inside the **Help** system book,
the editor goes into read-only mode:

- Border is teal.
- Title carries `[read-only]`.
- All mutating keystrokes are intercepted with a `Help is read-only`
  status message.
- Navigation, copy, search, scroll, focus chords still work.
- `Ctrl+S` is a no-op (status: "Help is read-only — nothing to save").

To make the Help book editable, do not nest your prose under it. The
Help book is reserved for `inkhaven import-help` output and is
designed to feed F1 lookups.

## Cursor memory across paragraphs

When you switch between paragraphs, each one remembers where the
cursor was the last time you visited. This survives:

- Switching panes (Tab, Ctrl+1..5, etc.)
- Opening a different paragraph in the same session.
- A full Inkhaven restart (the data is persisted to
  `.session.json`).

So jumping to "the place I was editing this morning" is a one-Enter
operation.

## What you have learned

- Movement, selection, clipboard, edits, undo / redo all work as you
  would expect.
- Save state is visible in the border colour and the title chip.
- Find / replace uses regex; `Ctrl+X` advances.
- F5 / F6 are snapshot creation / picker.
- F4 toggles split-edit; Ctrl+F4 accepts the snapshot side.
- F3 loads an arbitrary file into the buffer.
- Help subtree paragraphs are read-only by design.
- Cursor position is per-paragraph and survives restarts.

## Next steps

- [`04-search-and-discovery.md`](04-search-and-discovery.md) — finding
  prose with semantic search.
- [`05-ai-writing-assistant.md`](05-ai-writing-assistant.md) — how the
  AI pane works.
- [`06-grammar-check.md`](06-grammar-check.md) — F7 grammar workflow.
