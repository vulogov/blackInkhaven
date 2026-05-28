# 45 — Config TUI

`inkhaven config -p <dir>` launches a standalone TUI
for editing `<project>/inkhaven.hjson` — every
knob inkhaven reads at boot, surfaced in a schema-
aware editor with typed widgets, comment
preservation, timestamped backups, and an in-process
help pane that knows about every field.

The 1.2.10 release ships this in six phases (see
`Documentation/PROPOSALS/CONFIG_TUI.md` for the
design doc and `Documentation/RELEASE_NOTES/1.2.10.md`
for the rollout log).  This tutorial covers the
shipped surface.

## Why a separate editor

The existing `Ctrl+B 0` in-app HJSON editor is
still the right tool for **textual edits** —
search-and-replace, hand-typed comments, copy-paste
from a config snippet someone sent you.  The Config
TUI is the right tool for **structured edits** —
flipping a boolean, picking a colour, choosing
between enum variants, browsing what every field
does without leaving the keyboard.

Both editors round-trip cleanly because the Config
TUI uses **surgical text rewrite**: it only mutates
the byte ranges of changed values, leaving every
comment + unknown field + whitespace block byte-
identical.  Save in either editor, open in the
other, work continues seamlessly.

## Layout

```
┌──────────────────┬──────────────────────────────────────────┐
│ Config tree      │ editor.style_warnings.show_dont_tell     │
│                  │                                          │
│ ▼ editor         │   enabled                  [x] true      │
│   theme: …       │   use_stemming             [x] true      │
│   ▼ style_warn…  │   english_emotion_adjs     [ list ]      │
│     ▼ show_dont… │   russian_emotion_adjs     [ list ]      │
│       enabled    │   …                                      │
│       linking_v… │                                          │
│   tts: …         │  (default · unset · annotated · changed) │
│ ▶ llm            │                                          │
│ ▶ theme          │                                          │
│ ▶ timeline       │  Ctrl+S save · Ctrl+H help · Ctrl+B i    │
│ ▶ goals          │  inspect comments · Esc back             │
└──────────────────┴──────────────────────────────────────────┘
```

Tree on the left.  Detail pane on the right —
either the focused field's value editor (for
leaves) or a child list (for stanzas).

## Where the schema comes from

Two sources combine:

  * **Auto-derived from `Config::default()`.**  At
    runtime, the schema walks the serialised JSON
    of `Config::default()` to discover every field,
    its default value, and its rough type.  Any new
    field you add to the Rust `Config` struct shows
    up automatically.
  * **Build-time doc-comment extraction.**  A
    `build.rs` step parses `src/config.rs` with
    `syn` and emits a `(path, doc_comment)` lookup
    table baked into the binary.  So the help pane
    has docs for every field, not just the ones
    you've added to `CONFIGURATION.md`.

  * **A small hand-rolled metadata table** narrows
    generic `String` leaves into richer types:
    `theme.*_bg` / `*_fg` / `*_border` → Color,
    paths ending in `_dir` / `_directory` / `_path`
    / `_file` → Path, `typst_compile.engine` and
    `embeddings.model` → Enum.

## Chord set

### Tree pane

| Chord       | Action                                |
|-------------|---------------------------------------|
| `↑↓`        | Navigate                              |
| `Enter` / `Space` | Expand/collapse a stanza, or open the value editor on a leaf |
| `e`         | Open value editor (any focused leaf)   |
| `r`         | Reset focused leaf to its default      |
| `a`         | Add map entry (when focused on a known map path, e.g. `llm.providers`) |
| `d`         | Delete map entry (when focused on a direct child of a known map path) |
| `PgUp/PgDn` | Page by 10                             |
| `Home/End`  | First / last visible row               |

### Global chords

```
Ctrl+S        save (confirmation modal + atomic write + backup)
Ctrl+R        rollback picker (list .config-backups/)
Ctrl+H / ?    field-aware help pane (CONFIGURATION.md row + doc-comment + structural metadata)
Ctrl+I        comment inspector (HJSON `#` / `//` / `/* */` comments attached to the focused field)
Ctrl+A        edit annotation on the focused field (free-text note stored in .config-annotations.hjson)
Esc / Ctrl+Q  quit (confirm if unsaved)
```

### Value editor widgets

When you press `Enter` / `e` on a leaf, the right
pane swaps to a typed editor.  Six widget kinds
ship:

| Type     | Widget                                                     |
|----------|------------------------------------------------------------|
| `bool`   | `Space` / `t` / `f` toggle; `y` / `n` snap; `Enter` commits |
| `int`    | digits + sign + Backspace input; `↑`/`↓` for ±1; bounds-checked |
| `float`  | same shape with `.` / `e` / `E` accepted; rejects NaN / infinity |
| `string` | free text; multibyte clean                                  |
| Color    | hex digit + `#`; live RGB swatch + decomposition; theme-preview pane (fg / bg / border variants based on path suffix); validates `#RRGGBB` |
| Path     | text input + live `✓ exists` / `○ created on first use` check |
| Enum     | `↑↓` cycler over the variant list; Home/End jump endpoints  |
| List     | vertical `Vec<String>` editor: browse mode (↑↓ / a add / d delete / e edit) + inline single-line edit submode |

`Esc` cancels any editor; `Enter` commits.

## Save semantics

`Ctrl+S`:

  1. Validate every staged change.  Refuses to save
     if anything is `invalid` (e.g. malformed colour).
  2. Confirmation modal lists every pending edit
     partitioned into `splice` / `append` /
     `+entry` / `-entry` buckets.
  3. `y` / `Enter` commits via atomic write
     (`.hjson.tmp` + rename).
  4. Backup copy lands in
     `<project>/.config-backups/inkhaven_YYYYMMDD_HHMMSS.hjson`.
  5. Restart-required overlay (magenta `restart
     required` chip on the top bar) — most config
     fields are read at boot, so the main TUI needs
     to be relaunched to pick up the changes.

The save pipeline is **comment-preserving**.  A
hand-rolled HJSON walker records the byte range of
every leaf's value at load time; saves only mutate
those byte ranges, leaving every comment + unknown
field + indent style byte-identical.  Phase-2-level
plumbing detail; the upshot is that a hand-curated
`inkhaven.hjson` survives a save round-trip
verbatim.

## Unknown / user-added fields

Fields in the live HJSON that the schema doesn't
recognise (forward-compat experiments, integration
config, typo'd keys) get a yellow `N unknown` chip
on the top bar.  The detail pane on their parent
stanza lists them under "unknown sub-fields
preserved as-is".  The save pipeline never touches
them — they sit outside every recorded splice range
— but the editor also doesn't display them as
first-class tree entries.

## Map paths

`llm.providers` is recognised as a *map* (a
`HashMap<String, T>` in the Rust schema, not a
fixed-shape stanza).  Live HJSON entries under
`llm.providers` that aren't in the defaults appear
as first-class tree entries — editable, inspectable,
annotatable like any other leaf.

Inside the focused map stanza:

  * `a` → name prompt → new entry templated from
    any existing default → staged
  * `d` on a map entry → confirm → struck-through
    until save (or dropped entirely if the entry
    was newly-added in this session)

## Comment inspector (`Ctrl+I`)

Distinct from `Ctrl+H` help:

  * **`Ctrl+H` help** — shows inkhaven's **author**
    docs for the field: structural metadata
    (path / type / default / source), the matching
    CONFIGURATION.md row when present, plus the
    Rust doc-comment auto-extracted from
    `src/config.rs` at build time.
  * **`Ctrl+I` inspector** — shows the **user's**
    own notes: HJSON `#` / `//` / `/* */`
    comments attached to the focused field's
    stanza in `inkhaven.hjson` + any annotation
    you've attached via `Ctrl+A`.

Both panes draw from the byte-range index — no
re-parsing per render.

## Annotations (`Ctrl+A`)

Single-line free-text note attached to a config
field's path.  Stored in a flat HJSON sidecar at
`<project>/.config-annotations.hjson` (`BTreeMap<String,
String>`).  Annotated fields render a `+` chip next
to their state chip in the tree pane (so a
configured-AND-annotated leaf reads `●+`, a staged-
AND-annotated leaf `✱+`).  Annotations persist
across sessions.

Empty input on `Ctrl+A` clears the entry; empty
store deletes the sidecar file.

## Rollback (`Ctrl+R`)

Lists every `inkhaven_YYYYMMDD_HHMMSS.hjson` file
in `.config-backups/`, newest-first.

  * `Enter` *stages* the backup into the working
    tree — every leaf diff becomes a pending
    change.  `Ctrl+S` commits.
  * `v` previews the backup contents in a
    scrollable pane.
  * `d` deletes the backup with confirm.
  * `Esc` back to the main view.

The first `Ctrl+S` after a rollback writes a fresh
backup of the pre-rollback state, so the safety
chain stays intact.

## See also

  * `Documentation/PROPOSALS/CONFIG_TUI.md` — the
    design doc.
  * `Documentation/CONFIGURATION.md` — flat
    reference for every config field (the
    curated half of the help pane's content).
  * `Documentation/Tutorials/44-prompts-editor.md`
    — the sibling standalone TUI for
    `prompts.hjson`.
  * `Documentation/Tutorials/36-config-editor.md`
    — the in-app `Ctrl+B 0` HJSON editor (the
    textual sibling to this structured one).
