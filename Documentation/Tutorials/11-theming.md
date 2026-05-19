# 11 — Theming

Inkhaven ships with a Catppuccin-Mocha-style dark theme by default —
balanced contrast, low eye strain, decent on most terminals. Every
colour you see is configurable through `inkhaven.hjson`. This
tutorial walks the theme block, explains what each field paints,
and shows two alternative palettes you can paste in to try.

## The theme block

Look at `inkhaven.hjson`'s `theme: { … }` section. Every field is a
hex colour string — `#RRGGBB` or the short `#RGB` form. Empty
string falls back to the shipped default (the same Catppuccin Mocha
colour Inkhaven would use if the field were absent).

## Anatomy of a frame

When you look at the TUI, the colour decisions come from several
layers:

1. **Pane background** — the fill of every pane (Tree, Editor, AI,
   Search, AI prompt). One colour, applied via the Block widget's
   `.style()`.
2. **Pane foreground** — default text colour inside panes.
3. **Pane border** — focused / unfocused / state-specific
   (editor swaps in dirty / saved / read-only when focused).
4. **Overlays** — current line, line numbers, lexicon (Places /
   Characters), search match, grammar-change, syntax tokens.

Below is each field and what it paints.

## Pane chrome

```hjson
theme: {
  pane_bg:           "#1e1e2e"
  pane_fg:           "#cdd6f4"
  line_number_fg:    "#6c7086"
  current_line_bg:   "#313244"
  …
}
```

| Field | What it paints |
| ----- | -------------- |
| `pane_bg` | Background fill of every pane. |
| `pane_fg` | Default foreground for text inside panes. |
| `line_number_fg` | The dim gutter to the left of editor text. |
| `current_line_bg` | The horizontal stripe behind the cursor's line in the editor. |

## Borders

```hjson
border_focused:    "#cba6f7"
border_unfocused:  "#45475a"
border_dirty:      "#f9e2af"
border_saved:      "#a6e3a1"
border_readonly:   "#94e2d5"
```

`border_focused` / `border_unfocused` apply to every non-editor pane.
The editor pane swaps in:

- `border_dirty` (yellow) while focused **and** the buffer has
  unsaved changes.
- `border_saved` (green) while focused **and** clean.
- `border_readonly` (teal) while focused **and** the open paragraph
  lives in the Help subtree.

Unfocused, the editor uses `border_unfocused` like every other pane.

## Floating windows

```hjson
modal_bg:          "#181825"
modal_fg:          "#cdd6f4"
modal_border:      "#cba6f7"
```

Every modal in Inkhaven uses these three colours: Add, Delete, Rename,
FindReplace, QuickRef (`Ctrl+B H`), Help (F1), FilePicker (F3),
PromptPicker (`/` in AI prompt), SnapshotPicker (F6).

`modal_bg` is intentionally slightly darker than `pane_bg` so
floating overlays read as "on top of" the pane behind them.

## Lexicon overlays

```hjson
places_fg:         "#89dceb"
characters_fg:     "#f9e2af"
```

Mentions of Place names (cyan) and Character names (yellow) in the
editor. See [`07-places-and-characters.md`](07-places-and-characters.md).

## In-buffer search

```hjson
search_match_bg:   "#f38ba8"
search_current_bg: "#f5c2e7"
```

When you press `Ctrl+F` in the editor and search the buffer, every
match is painted with `search_match_bg`; the cursor's current match
gets `search_current_bg` so it stands out among siblings.

## Tree pane

```hjson
tree_open_marker:  "#a6e3a1"
tree_book_fg:      "#f5c2e7"
tree_chapter_fg:   "#89b4fa"
tree_subchapter_fg:"#94e2d5"
tree_paragraph_fg: "#cdd6f4"
```

| Field | Effect |
| ----- | ------ |
| `tree_open_marker` | The ▸ glyph and row colour for the currently-loaded paragraph (overrides per-kind colour). |
| `tree_book_fg` | Book rows. Bold by default. |
| `tree_chapter_fg` | Chapter rows. Bold. |
| `tree_subchapter_fg` | Subchapter rows. |
| `tree_paragraph_fg` | Paragraph rows. Plain (no bold). |

Books and chapters are bolded so the upper hierarchy has visual
weight; paragraphs use plain text since they dominate row count and
should read calm.

## Editor header chip

```hjson
editor_position_fg: "#89dceb"
```

The `L<row> C<col>` cursor read-out in the Editor pane title.
Default sky-blue.

## AI header chips

```hjson
ai_scope_fg:       "#fab387"
ai_infer_fg:       "#94e2d5"
```

The AI pane title shows `scope=Paragraph` and `infer=Full` chips. The
scope chip uses `ai_scope_fg` (peach by default), the inference mode
chip uses `ai_infer_fg` (teal by default). Both are bold.

The scope chip only appears when scope ≠ None; the inference mode
chip is always visible so the F10 state is never silently armed.

## Grammar-change overlay

```hjson
grammar_change_fg: "#f38ba8"
```

After a `g`-apply from a grammar check, characters that differ from
the pre-correction baseline render in this colour + bold. Default red.

## Typst syntax

```hjson
syntax_heading:    "#cba6f7"
syntax_bold:       "#f9e2af"
syntax_italic:     "#94e2d5"
syntax_string:     "#a6e3a1"
syntax_number:     "#fab387"
syntax_comment:    "#6c7086"
syntax_keyword:    "#cba6f7"
syntax_function:   "#89dceb"
syntax_operator:   "#94e2d5"
syntax_list_marker:"#cba6f7"
syntax_raw:        "#fab387"
syntax_tag:        "#89b4fa"
syntax_quote:      "#9399b2"
```

Drives the tree-sitter-typst highlighter. Pick colours that match
your prose-reading preferences:

- `syntax_heading` — `= Heading`, `== Heading`, etc.
- `syntax_bold` / `syntax_italic` — `*bold*` and `_italic_`.
- `syntax_string` — quoted strings.
- `syntax_number` — numeric literals.
- `syntax_comment` — `// comment`.
- `syntax_keyword` — Typst control words like `set`, `show`, `if`, …
- `syntax_function` — function names in `#calc.…`, `#set …`, etc.
- `syntax_operator` — symbols (`=`, `+`, `*`).
- `syntax_list_marker` — `-` and `+` at the start of list lines.
- `syntax_raw` — `` `code` `` inline code and fenced ``` blocks.
- `syntax_tag` — markup labels and reference targets.
- `syntax_quote` — block quotes.

## Two alternative palettes

Save your `inkhaven.hjson`, then try one of these by replacing the
`theme:` block:

### Solarized Dark

```hjson
theme: {
  pane_bg:            "#002b36"
  pane_fg:            "#839496"
  line_number_fg:     "#586e75"
  current_line_bg:    "#073642"

  border_focused:     "#268bd2"
  border_unfocused:   "#586e75"
  border_dirty:       "#b58900"
  border_saved:       "#859900"
  border_readonly:    "#2aa198"

  modal_bg:           "#073642"
  modal_fg:           "#93a1a1"
  modal_border:       "#268bd2"

  places_fg:          "#2aa198"
  characters_fg:      "#b58900"
  search_match_bg:    "#dc322f"
  search_current_bg:  "#cb4b16"

  tree_open_marker:   "#859900"
  tree_book_fg:       "#d33682"
  tree_chapter_fg:    "#268bd2"
  tree_subchapter_fg: "#2aa198"
  tree_paragraph_fg:  "#93a1a1"

  editor_position_fg: "#2aa198"
  ai_scope_fg:        "#cb4b16"
  ai_infer_fg:        "#2aa198"
  grammar_change_fg:  "#dc322f"

  syntax_heading:     "#d33682"
  syntax_bold:        "#b58900"
  syntax_italic:      "#2aa198"
  syntax_string:      "#859900"
  syntax_number:      "#cb4b16"
  syntax_comment:     "#586e75"
  syntax_keyword:     "#268bd2"
  syntax_function:    "#2aa198"
  syntax_operator:    "#93a1a1"
  syntax_list_marker: "#d33682"
  syntax_raw:         "#cb4b16"
  syntax_tag:         "#268bd2"
  syntax_quote:       "#586e75"
}
```

### Gruvbox Dark

```hjson
theme: {
  pane_bg:            "#282828"
  pane_fg:            "#ebdbb2"
  line_number_fg:     "#7c6f64"
  current_line_bg:    "#3c3836"

  border_focused:     "#d3869b"
  border_unfocused:   "#504945"
  border_dirty:       "#fabd2f"
  border_saved:       "#b8bb26"
  border_readonly:    "#8ec07c"

  modal_bg:            "#1d2021"
  modal_fg:            "#ebdbb2"
  modal_border:        "#d3869b"

  places_fg:          "#83a598"
  characters_fg:      "#fabd2f"
  search_match_bg:    "#fb4934"
  search_current_bg:  "#d3869b"

  tree_open_marker:   "#b8bb26"
  tree_book_fg:       "#d3869b"
  tree_chapter_fg:    "#83a598"
  tree_subchapter_fg: "#8ec07c"
  tree_paragraph_fg:  "#ebdbb2"

  editor_position_fg: "#83a598"
  ai_scope_fg:        "#fe8019"
  ai_infer_fg:        "#8ec07c"
  grammar_change_fg:  "#fb4934"

  syntax_heading:     "#d3869b"
  syntax_bold:        "#fabd2f"
  syntax_italic:      "#8ec07c"
  syntax_string:      "#b8bb26"
  syntax_number:      "#fe8019"
  syntax_comment:     "#7c6f64"
  syntax_keyword:     "#d3869b"
  syntax_function:    "#83a598"
  syntax_operator:    "#8ec07c"
  syntax_list_marker: "#d3869b"
  syntax_raw:         "#fe8019"
  syntax_tag:         "#83a598"
  syntax_quote:       "#a89984"
}
```

## Light themes

Inkhaven works fine on light backgrounds — just invert the
`pane_bg` / `pane_fg` and adjust the accent colours. Example
Catppuccin Latte (light theme):

```hjson
theme: {
  pane_bg:            "#eff1f5"
  pane_fg:            "#4c4f69"
  line_number_fg:     "#9ca0b0"
  current_line_bg:    "#dce0e8"

  border_focused:     "#8839ef"
  border_unfocused:   "#bcc0cc"
  border_dirty:       "#df8e1d"
  border_saved:       "#40a02b"
  border_readonly:    "#179299"

  modal_bg:            "#e6e9ef"
  modal_fg:            "#4c4f69"
  modal_border:        "#8839ef"

  places_fg:          "#04a5e5"
  characters_fg:      "#df8e1d"
  search_match_bg:    "#d20f39"
  search_current_bg:  "#ea76cb"

  tree_open_marker:   "#40a02b"
  tree_book_fg:       "#ea76cb"
  tree_chapter_fg:    "#1e66f5"
  tree_subchapter_fg: "#179299"
  tree_paragraph_fg:  "#4c4f69"

  editor_position_fg: "#04a5e5"
  ai_scope_fg:        "#fe640b"
  ai_infer_fg:        "#179299"
  grammar_change_fg:  "#d20f39"
}
```

(The `syntax_*` fields here are left to their dark-theme defaults to
keep the example short; for a polished light theme you would also
override them.)

## Tips

- After editing the theme, restart the TUI to see changes. There is
  no live reload.
- Test with `theme.pane_bg` and `theme.pane_fg` first — those drive
  the dominant impression. Then iterate borders, overlays, syntax.
- If a colour you set doesn't appear, look at the parsing path in
  [`../CONFIGURATION.md`](../CONFIGURATION.md#theme). Malformed
  values silently fall back to the default — there is no error.
- Truecolor terminals render exact hex values; 256-colour terminals
  pick the closest neighbour automatically. Either way `Color::Rgb`
  is what ratatui sends.

## What you have learned

- Every Inkhaven colour is configurable through the `theme:` block.
- Fields fall into pane chrome, borders, modals, lexicon overlays,
  search overlay, tree colours, editor header chip, AI header chips,
  grammar-change overlay, and Typst syntax.
- Hex strings are `#RRGGBB` or `#RGB`. Empty / invalid → default.
- Three palettes ready to paste: Catppuccin Mocha (default),
  Solarized Dark, Gruvbox Dark, Catppuccin Latte (light).
- Restart the TUI to pick up theme changes.

## Next steps

- [`../CONFIGURATION.md`](../CONFIGURATION.md) — full HJSON reference.
- [`../KEYBINDING.md`](../KEYBINDING.md) — the keystrokes that
  surface in the theme (border state colours, current-line, etc.).
- Build your own palette: pick a base colour from a palette generator
  like [`coolors.co`](https://coolors.co) and fill out the fields.
