# Inkhaven color styles

Fifteen ready-to-use color schemes for Inkhaven — the classics plus the
most popular modern dark and light palettes. Each file is a **partial
theme override** built for the 1.2.20+ config cascade: it sets only the
`theme` block, so it layers cleanly over any project without touching the
rest of your config.

## How to use

You have two options.

### 1. Apply to *every* project (recommended)

Copy the scheme you want into your user-global config directory — it then
applies to every project you open, with no per-project edits:

```bash
mkdir -p ~/.config/inkhaven/conf
cp color_styles/nord.hjson ~/.config/inkhaven/conf/
```

Inkhaven layers `~/.config/inkhaven/conf/*.hjson` on top of each project's
`inkhaven.hjson` (global wins). See
[`Documentation/CONFIGURATION.md` → "Global overrides"](../Documentation/CONFIGURATION.md#global-overrides-1220)
for the full precedence rules. Keep only one scheme file in `conf/` at a
time, or the lexically-last filename wins.

### 2. Apply to a *single* project

Open the scheme file, copy its `theme { … }` block, and paste it into that
project's `inkhaven.hjson`, replacing the existing `theme` block.

Either way, restart the TUI to see the new colors (config is read at
startup).

## The schemes

### Dark

| Scheme | File | Notes |
|--------|------|-------|
| Dracula | `dracula.hjson` | The famous high-contrast purple/pink dark theme. |
| Nord | `nord.hjson` | Cool, low-saturation arctic blues. Easy on the eyes. |
| Gruvbox Dark | `gruvbox-dark.hjson` | Warm, retro, earthy — a long-time classic. |
| Solarized Dark | `solarized-dark.hjson` | Ethan Schoonover's precision-balanced classic. |
| Tokyo Night | `tokyo-night.hjson` | Deep indigo night palette, very popular. |
| Monokai | `monokai.hjson` | The original vivid editor classic. |
| One Dark | `one-dark.hjson` | Atom's signature balanced dark theme. |
| Material Ocean | `material-ocean.hjson` | Deep blue-black Material variant. |
| Catppuccin Mocha | `catppuccin-mocha.hjson` | Soft pastel dark — **Inkhaven's built-in default**. |

### Light

| Scheme | File | Notes |
|--------|------|-------|
| Solarized Light | `solarized-light.hjson` | The classic warm-paper light counterpart. |
| Gruvbox Light | `gruvbox-light.hjson` | Cream background, earthy accents. |
| GitHub Light | `github-light.hjson` | Clean, familiar, high-legibility white. |
| Catppuccin Latte | `catppuccin-latte.hjson` | Soft pastel light. |
| One Light | `one-light.hjson` | Atom's crisp light theme. |
| Ayu Light | `ayu-light.hjson` | Bright, minimal, signature orange accent. |

## Customizing

Each file maps a scheme's palette onto Inkhaven's semantic theme fields
(editor, tree pane, modals, syntax, style-warning overlays, AI chips). To
tweak one color, edit its line — every value is an `#rrggbb` hex string.
For what each field controls, see the
[`theme` section of CONFIGURATION.md](../Documentation/CONFIGURATION.md#theme).

Want a scheme that isn't here? Copy the closest file, rename it, and adjust
the palette — the field set is already complete, so you only change values.
