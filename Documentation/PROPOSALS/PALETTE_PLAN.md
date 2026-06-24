# 1.3.33 — Command palette + `?` keybinding overlay

The second stop on the road to 1.4.0 ([ROADMAP-1.4.0.md](ROADMAP-1.4.0.md)). The
`Ctrl+B` chord tree has grown large (W / J / D / A / Tab / …); a fuzzy command
palette is the single biggest discoverability win and pure additive QoL.

## What exists (the win: a real registry)

`src/tui/keybind.rs` is already a data-driven registry — no scraping needed:

- **`Action`** enum (`Debug, Clone, PartialEq, Eq, Hash, Serialize`) — every
  user-reachable command, with `.label()` (short UI name) and `.description()`
  (longer help text).
- **`Scope`** (`Copy`) — `Any` / `Editor` / `Tree` / `Ai`, with `.matches(focus)`.
- **`KeyBindings`** — four binding tables (`top_level`, `meta_sub`, `bund_sub`,
  `view_sub`) of `BindingEntry { chord: KeyChord, action: Action, scope: Scope }`,
  plus the layer prefixes (`meta_prefix` = `Ctrl+B`, `view_prefix` = `Ctrl+V`,
  `bund_prefix` = `Ctrl+Z`). `KeyChord::to_display_string()` renders a chord.
- **`keybind::read()`** — the process-wide live table.
- **`App::run_action(Action)`** (app.rs) — the single dispatch point that executes
  any command. The palette executes a selection by cloning its `Action` and calling
  this. No replay of synthetic key events.

Modals follow a clear pattern (Modal variant → open method → `render/modals.rs`
painter → `handle_modal_key` arm). `TextInput` + `handle_text_input_key` drive
filter input; an existing `fuzzy_filter_entries` shows the substring-scoring style.

## Phases

- **P0 — the palette registry (pure).** A new `src/tui/palette.rs`: `PaletteEntry`
  (action + label + description + rendered chord string + scope), `collect(&KeyBindings)
  -> Vec<PaletteEntry>` (walks the four tables, prefixes sub-chords with their layer
  chord, dedupes by `Action`, sorts by label), and `fuzzy_filter(&[PaletteEntry],
  query) -> Vec<usize>` (score by label/chord/description). Pure → unit-tested +
  the stability-rider **registry proptests**. **(this increment)**
- **P1 — the `Ctrl+P` palette modal.** `Modal::CommandPalette`, `open_command_palette`,
  the `render/modals.rs` painter, the `handle_modal_key` arm, and the top-level
  `Ctrl+P` binding. Enter → `run_action(entry.action)`.
- **P2 — the `?` keybinding overlay + quick help/reference.** A pane-scoped,
  read-only chord cheat-sheet (`?` when not in a text field), built from the same
  registry. **Update the in-app quick help/reference and `Documentation/KEYBINDING.md`**
  to mention `Ctrl+P` + `?` (the roadmap DoD regenerates KEYBINDING anyway).
- **P3 — CLI-dispatch error pass (stability rider).** A small consistency pass on
  CLI command error messages touched alongside this work.

## Non-goals

No change to the `Action` enum's semantics, the binding tables, `run_action`, or any
existing chord. The palette is a new surface over the existing registry. No external
deps.

## Increment log

- **P0** — _done._ `src/tui/palette.rs`: `PaletteEntry` (action + label + description
  + layer-prefixed chord string + scope, with `applies_in(focus)`), `collect(&KeyBindings)`
  (walks top_level/meta_sub/view_sub/bund_sub, prefixes sub-chords with their layer
  chord, dedupes by `Action`, sorts by label), `fuzzy_filter` (label-prefix > label
  > chord > description scoring; blank query → all). Pure. 6 unit tests + 2 registry
  proptests (filter returns in-range unique indices on arbitrary queries; whitespace
  query returns all). Full suite 1771 → 1779.
</content>
