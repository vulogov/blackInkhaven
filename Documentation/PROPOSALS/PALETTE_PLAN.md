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
- **P1 — the palette modal.** _Done._ Bound to **`Ctrl+Shift+P`** (the VS Code
  convention) — **not** `Ctrl+P`, which is the editor paste (gated `!shift`, so no
  collision). New `Action::OpenCommandPalette` (label/description + default top-level
  binding) so the palette is itself rebindable + self-listing; `run_action` arm;
  `Modal::CommandPalette { input, entries, cursor, scroll }`; `open_command_palette`
  (collects from `keybind::read()`); `command_palette_handle_key` (arrows/page/type-
  to-filter; Enter → `run_action(action)`; Esc closes generically); `handle_modal_key`
  + render dispatch; `draw_command_palette_modal` painter (label · chord · description,
  reversed cursor row). Binding-resolve test (normalization-agnostic) + the build.
  Full suite 1779 → 1780.
- **P1-fix** — _superseded._ First tried `Ctrl+Shift+P` (top-level + a hardcoded
  intercept). Live testing showed its terminal reporting is too erratic — on the
  user's terminal the first press never matched (the modal opened but painted a key
  late, swallowing the next keystroke into its filter).
- **P1-final** — _done._ **Standardized on `Ctrl+V Space`** as the canonical palette
  chord (`view_sub`, `Scope::Any`) — a two-key chord with no Shift+letter terminal
  ambiguity. Removed the `Ctrl+Shift+P` top-level binding and its hardcoded
  intercept; the description now reads "Ctrl+V Space"; the palette still self-lists
  via the registry (now from `view_sub`). Kept the opt-in `INKHAVEN_KEYLOG` tracer
  as general chord-debugging infra. Resolve-tested in multiple panes + the build.
- **P2 — quick help/reference + `?` overlay.** _Done._ Inkhaven already has a
  pane-aware **quickref** (`Ctrl+B H`) that renders the live binding tables, so the
  palette auto-surfaced in its "View chords" section. Made it prominent: added the
  palette + `Ctrl+V` prefix + the quickref-opener to the static global section, and
  `?` to the tree section. Bound **`?` → quickref in the Tree pane only** (a
  pure-navigation pane; editor / AI / search keep `?` literal; `Ctrl+B H` works
  everywhere). Updated `KEYBINDING.md` (Global row, View-mode `Space` row, tree
  `?` row). Added a quickref test asserting the palette lists in every pane.
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
