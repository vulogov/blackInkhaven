# Tutorial 62 — Global config and shareable themes

*Inkhaven 1.2.20+*

Every project carries its own `inkhaven.hjson` ([Tutorial 11](11-theming.md),
[Tutorial 36](36-config-editor.md)). That's right for project-specific
settings — the books, the language, the goals. But some preferences are
*yours*, not the project's: your colour scheme, your keybindings. Before
1.2.20 you had to repeat them in every project's config.

1.2.20 adds **user-global overrides**: a personal config layer that applies
to *every* project, with no per-project edits. This tutorial covers the
layering rules and the 15 ready-made colour schemes that ship with it.

## The cascade

After a project's `inkhaven.hjson` is read, Inkhaven layers your global
override files on top. Precedence, lowest → highest:

1. compiled-in defaults
2. the project's `inkhaven.hjson`
3. `~/.config/inkhaven/config.hjson`
4. `~/.config/inkhaven/conf/*.hjson` — every `.hjson` file in that folder,
   in sorted filename order, each overriding the previous

So a global file **wins over the project**. That direction is deliberate:
`inkhaven init` writes a *full* config (every theme colour is present), so
if the project won you'd never see your global colours. The override files
are **partial** — you put in only the keys you want to change, and
everything else falls through to the project.

## Set a preference once, everywhere

Say you always want a lighter editor foreground and your own echo-overlay
colour, in every project. Create the global file:

```bash
mkdir -p ~/.config/inkhaven
$EDITOR ~/.config/inkhaven/config.hjson
```

```hjson
// ~/.config/inkhaven/config.hjson — applies to all projects
{
  theme: {
    pane_fg:               "#e0e0e0"
    style_warning_echo_fg: "#7aa2f7"
  }
}
```

Restart the TUI in any project and those two colours are in effect — the
rest of that project's theme is untouched. Nothing in the project's
`inkhaven.hjson` changed.

You can split overrides across the `conf/` folder if you like — handy for
keeping concerns separate:

```
~/.config/inkhaven/
├── config.hjson          # base personal overrides
└── conf/
    ├── 10-theme.hjson    # colours
    └── 20-keys.hjson     # keybindings
```

Higher-numbered filenames win on a conflict, so `conf/20-keys.hjson`
overrides anything set earlier. The directory honours `$XDG_CONFIG_HOME`
(falling back to `~/.config`).

## 15 ready-made colour schemes

The repository ships a `color_styles/` folder with the most popular
classic, dark, and light schemes, already written as partial `theme`
overrides:

**Dark** — Dracula · Nord · Gruvbox Dark · Solarized Dark · Tokyo Night ·
Monokai · One Dark · Material Ocean · Catppuccin Mocha *(Inkhaven's
built-in default)*

**Light** — Solarized Light · Gruvbox Light · GitHub Light · Catppuccin
Latte · One Light · Ayu Light

To theme **every** project with one of them, copy it into your global
`conf/`:

```bash
mkdir -p ~/.config/inkhaven/conf
cp color_styles/nord.hjson ~/.config/inkhaven/conf/
```

Restart the TUI — Nord everywhere. Keep only one scheme file in `conf/` at
a time, or the lexically-last filename wins.

To theme a **single** project instead, open the scheme file, copy its
`theme { … }` block, and paste it over that project's `theme` block in
`inkhaven.hjson`.

Each preset maps the scheme's palette onto Inkhaven's full set of semantic
theme fields — editor, tree pane, modals, syntax colours, the
style-warning overlays, the AI chips — so switching restyles the whole UI
cohesively. See [`color_styles/README.md`](../../color_styles/README.md)
for the index.

## `NO_COLOR` — monochrome everywhere

Sometimes the answer isn't a different palette but *no* palette. Set the
`NO_COLOR` environment variable to any non-empty value (the
[no-color.org](https://no-color.org) convention) and the whole TUI goes
monochrome — every theme colour, from any of the presets above, resolves
to the terminal default:

```bash
NO_COLOR=1 inkhaven
```

Structure still reads perfectly: headings, selection, matches and the
like fall back to **bold**, reversed, and underlined attributes rather
than hue. Reach for it on a light-background or monochrome terminal, or
for accessibility. It's an **environment override**, not a config key —
it sits outside the cascade above and wins over any `theme` block,
global or project.

## When something is wrong

- A malformed **global** file is **skipped with a warning** — a typo in
  your personal override never bricks every project. Only a malformed
  **project** `inkhaven.hjson` is fatal (exactly as before).
- The in-app config editor (`Ctrl+B 0`, [Tutorial 36](36-config-editor.md))
  edits the **project** file directly, so what it shows is the raw project
  config — not the global-merged result you see live. That asymmetry is
  intentional: you edit *your project's* config there; your global layer is
  a separate, personal thing.

## See also

- [Tutorial 11 — Theming](11-theming.md): every theme colour knob and what
  it controls.
- [Tutorial 36 — Config editor](36-config-editor.md): editing the project
  `inkhaven.hjson` from inside the TUI.
- [`../CONFIGURATION.md`](../CONFIGURATION.md): the full config reference,
  including the "Global overrides" section.
