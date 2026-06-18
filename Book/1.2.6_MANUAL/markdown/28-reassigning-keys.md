# 28 — Reassigning keys

Every chord in inkhaven is a binding to a named action like `editor.save` or `view.story_graph`. Bindings live in HJSON; overlays at runtime work too. Re-skin the chord set without recompiling.

## The keys stanza

```hjson
keys: {
  bindings: [
    # Move the Bund prefix from Ctrl+Z to Ctrl+B b.
    { chord: "Ctrl+B b", action: "bund.run_buffer" }

    # Bind F11 back to explain-diagnostic (Linux/Windows
    # only — macOS grabs F11).
    { chord: "F11", action: "editor.explain_diagnostic" }

    # Disable a default chord (action: "none").
    { chord: "Ctrl+V w", action: "none" }
  ]
}
```

Each entry is `{ chord, action }`. Multiple bindings to the same action work — both chords fire it.

## Chord syntax

| Shape | Example |
|-------|---------|
| Letter | `a` or `Z`. |
| Modifier + key | `Ctrl+B`, `Alt+F2`, `Shift+Tab`, `Super+L`. |
| Modifier + capital | `Shift+W` (the W chord with shift). |
| Two-step | `Ctrl+B` then `P` — write as `Ctrl+B p`. |
| F-keys | `F1` through `F12`. `Ctrl+F12` for the modifier variant. |
| Special | `Enter`, `Esc`, `Tab`, `Space`, `BackSpace`, `Up`, `Down`, … |

## Listing actions

```
inkhaven keys list
```

Prints every action + its current chord. Useful when you forget the action name.

## Layered chord tables

Inkhaven groups chords into layers:

| Layer | Description |
|-------|-------------|
| TopLevel | Direct chords (Ctrl+S, F7, etc.). |
| MetaSub | After the Meta prefix (Ctrl+B → action). |
| BundSub | After the Bund prefix (Ctrl+Z → action). |
| ViewSub | After the View prefix (Ctrl+V → action). |

The `Ctrl+B H` cheat sheet (Chapter 27) shows all four layers grouped — useful when you want to see what chord slots are still free.

## Dynamic rebinding via Bund

`ink.key.bind` lets a Bund script rebind at runtime:

```bund
"Ctrl+B j" "view.fuzzy_paragraph_picker" ink.key.bind
```

Useful in `scripting.bootstrap` for per-project chord adjustments without touching the project-shared HJSON. Or in a hook that reacts to a state change.

Policy: `keymap` (default-denied — opt in via `scripting.enabled_categories`).

## Re-binding the prefix keys

The three prefix chords are reconfigurable:

```hjson
keys: {
  meta_prefix:  "Ctrl+B"        # default
  bund_prefix:  "Ctrl+Z"        # default
  view_prefix:  "Ctrl+V"        # default
}
```

If `Ctrl+Z` collides with your terminal's suspend chord, move the Bund prefix to `Ctrl+\` or `Alt+B`.

> **Caveat:** Changing the prefix doesn't change the sub-chord names in `MetaSub` / `BundSub` / `ViewSub`. Inkhaven's internal action table is keyed by the layer name plus the sub-chord; the prefix is just how the user gets to the layer.

## F11 → Ctrl+F12 — the macOS workaround

In 1.2.6, the AI-explain-diagnostic chord moved from `F11` to `Ctrl+F12` because macOS grabs F11 globally (Mission Control / Show Desktop) — the chord never reached the TUI.

Linux + Windows users who prefer F11 can bind it back:

```hjson
keys: {
  bindings: [
    { chord: "F11", action: "editor.explain_diagnostic" }
  ]
}
```

The original Ctrl+F12 binding stays — both fire the action.

## Recap

- Every chord binds an action via `keys.bindings` in HJSON.
- Chord syntax: `Ctrl+B p`, `Shift+W`, `Ctrl+F12`.
- `inkhaven keys list` enumerates actions + current chords.
- Four chord layers: TopLevel, MetaSub, BundSub, ViewSub.
- `ink.key.bind` from Bund rebinds at runtime.
- Prefix chords (Meta / Bund / View) are reconfigurable.
