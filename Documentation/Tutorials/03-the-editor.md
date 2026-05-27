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

## Show-don't-tell (1.2.9+)

`Show, don't tell` — the writing principle that says
*"her knuckles whitened around the glass"* is better
than *"she was angry"*.  Inkhaven gives you two
layers for catching the second kind:

### Inline overlay (always-on)

Hooks under the same `Ctrl+B Shift+F` toggle as
filter-words + repeated-phrases.  Underlines (in soft
teal) three categories of telling:

  * **`was angry`-style 2-grams** — a linking verb
    (`be` / `seem` / `feel` / `appear` / `look` /
    `become` / `remain` / `grow` / `sound`) followed
    by an emotion adjective.
  * **Manner-of-emotion adverbs** — `angrily`,
    `sadly`, `nervously`, `wearily`, … (the `-ly`
    adverbs that label the emotion outright).
  * **Cognition verbs** — `realised`, `understood`,
    `knew`, `wondered`, `decided`, … (verbs that
    tell the reader what the character is thinking
    rather than letting it come through).

Stemmed, so `seemed` matches `seem`; `realises`
matches `realised`.  Won't flag `was running` or
`looks at the door` — the 2-gram requires both
halves to match (linking verb + known emotion).

### AI scan (`Ctrl+B Shift+T`)

Sends the open paragraph to the configured LLM with
a system prompt asking for telling passages and
suggested rewrites.  Response streams into the AI
pane — same plumbing as F12 critique.

When to use which:

  * Use the **inline overlay** while you're
    drafting.  It nudges you to swap `she was nervous`
    for something embodied as you type.
  * Use the **AI scan** during revision passes.
    It catches subtler telling — declarative
    statements, narration that explains rather than
    dramatises — and proposes concrete alternatives.

The AI prompt name is `show-dont-tell`; override it
via your project's prompts book or the global
prompts.hjson, same as the critique prompts.

## Sentence-rhythm gauge (1.2.9+)

`Ctrl+B Shift+H` opens a modal that quantifies the
rhythm of the open paragraph — useful for noticing
when your sentences have drifted into a monotone
drone and need a short one to break the pattern.

What it shows:

  * A **verdict** colour-coded by how varied your
    sentence lengths are: red MONOTONE, yellow
    STEADY, green VARIED, cyan CHOPPY.  The verdict
    is computed from the coefficient of variation
    (CV = stdev / mean) so it stays meaningful
    whether your average sentence is 8 words or 18.
  * The **numbers**: N sentences, mean word count,
    stdev, CV, min, max.
  * A **per-sentence bar chart** where each row is
    one sentence and the bar's length reflects its
    word count (capped at 40 chars for display).
    Tells you at a glance whether your paragraph
    looks like a flat plateau or like jagged peaks.
  * **Outlier callouts** — the three shortest and
    three longest sentences with line numbers + a
    preview, so you can jump to them and decide
    whether the extreme is intentional.

The split is intentionally simple: `.` / `!` / `?`
followed by whitespace, with common abbreviations
suppressed (Mr., Mrs., Dr., e.g., i.e., Ph.D., …)
and `...` treated as a pause rather than a
terminator.  Good enough for literary text — the
goal is a gauge, not a parser.

Inside the modal:

  * `↑` / `↓` / `PgUp` / `PgDn` / `Home` / `End`
    scroll the bar chart.
  * Any other key closes.

Mnemonic: `Shift+H` for *heartbeat*.

## POV / character chip (1.2.9+)

The status bar gains a small magenta chip showing
the characters present in the currently-open
paragraph.  Example:

```
[Editor]  POV: Anna  +Bob, Carol   • saved 2s ago …
```

The most-mentioned character wins the **POV slot**
(rationale: in third-person limited prose the
narrator's gaze inevitably centres on the POV
character).  Ties broken by who's named first in
the paragraph.  Up to three additional characters
trail behind as the supporting cast.

The chip is driven by the existing `characters`
lexicon — the paragraphs you've already nested
under the Characters book.  No separate tagging,
no POV annotation, no per-paragraph frontmatter
to keep current.

Config (`inkhaven.hjson`):

```hjson
editor: {
  pov_chip_enabled: true
}
```

Runtime toggle: `Ctrl+B Shift+P` flips the chip on
/ off without rewriting HJSON.  Session-local
override on top of the persisted setting.

Edge cases:

  * No characters mentioned → no chip; the status
    bar reverts to its non-1.2.9 layout.
  * No paragraph open → no chip.
  * First-person POV (the narrator is `I`, not in
    the lexicon) → chip surfaces the *other*
    prominent character, which is the contextually
    useful piece of information anyway.

## Concordance view (1.2.9+)

`Ctrl+B Shift+L` opens a project-wide concordance:
every distinct word in your manuscript, ranked by
how often it appears, with a few in-context excerpts
per row.

What it's good for:

  * Spotting overworked vocabulary you didn't realise
    you lean on (`somehow`, `slightly`, `glanced`).
  * Locating where a character / place is mentioned
    without re-running search across the book.
  * Auditing your prose's lexical fingerprint — the
    top of the list is your *real* voice, not the
    voice you imagine.

What the pipeline does for you:

  * Strips out stop-words (`the`, `and`, `и`, `в`)
    so the top is meaningful lexical content.
  * Groups inflections via Snowball stemming —
    `walk`, `walked`, `walking`, `walks` collapse to
    one row whose "variants" trailer shows the
    surface forms.
  * Filters single-character tokens and bare
    digit runs.
  * Honours the project's `language` field — the
    Russian / French / German / Spanish stop-word
    lists ship built-in.

Inside the modal:

  * Type to filter (case-insensitive substring
    match across headwords + variants).
  * `↑` / `↓` / `PgUp` / `PgDn` / `Home` / `End`
    navigate.
  * `Ctrl+S` toggles sort (count ↔ alphabetical).
    Plain `s` types into the filter.
  * `Esc` closes.

Bottom panel shows up to three KWIC excerpts for the
selected row — `«word»` marks the matched token in
each excerpt so it's visually obvious in monospace.

Performance: well under a second on a 100k-word
manuscript.  The build runs once at modal open;
filter + sort changes are instant after that.

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

### Save as audio file (`Ctrl+B Shift+R`)

Companion to the read-aloud chord.  Instead of speaking
through the speakers, writes the paragraph to an audio
file on disk — useful for sharing a draft chapter as
audio, building a podcast-style preview, or just having
the prose available for car listening.

Pressing `Ctrl+B Shift+R` (editor scope) opens a path
picker pre-filled with
`<project>/audio/<paragraph-slug>.aiff`.  Edit the path
if you want a different name or directory, then `Enter`
commits.  `Esc` cancels.

The same voice + speed as `Ctrl+B S` is used.  Output
format follows the file extension — `.aiff` (default
AIFF-C compressed), `.wav` (linear PCM), `.m4a` (AAC)
all work on macOS 13+.  The parent directory is
created if it doesn't exist.

Status bar reports the written file size + path on
success.  Failures (no disk space, permission denied,
voice unavailable) surface there too.

Only works on macOS for now — the underlying `say -o`
command is macOS-specific.  Non-macOS hosts see the
same "TTS is macOS-only in 1.2.9" modal as `Ctrl+B S`.

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
