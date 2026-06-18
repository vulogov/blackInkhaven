# 24 — Diagnostics and render preview

Two features bring the Typst feedback loop into the editor: structured diagnostics surfaced at the line they fire, and a paragraph-level render preview that rasterises the open paragraph without running the full build.

## Gutter markers

Lines that carry a diagnostic get a red `●` in the line-number gutter:

![figure: gutter-diagnostic](images/gutter-diagnostic.png) — Editor gutter: line 3 has a red ● because it references an undefined function. Marker stays visible while you fix.

Both parse + semantic diagnostics surface here. The marker slot is otherwise a space, so a clean buffer pays no visual cost.

## F8 — diagnostics list

`F8` (Editor scope) pops a list of every diagnostic in the open paragraph:

![figure: f8-list](images/f8-list.png) — F8: diagnostics list. Each row shows line:col + message. Enter jumps cursor; Esc closes.

| Chord | What it does |
|-------|--------------|
| ↑ / ↓ | Move cursor. |
| Enter | Jump editor cursor to the selected diagnostic; close the modal. |
| Esc | Close. |

## Ctrl+V N / Shift+N — next / previous diagnostic

Without opening the modal:

| Chord | What it does |
|-------|--------------|
| Ctrl+V N | Jump to next diagnostic in the open buffer (wraps). |
| Ctrl+V Shift+N | Previous (wraps). |

Both refresh the diagnostics cache up-front so navigation follows the live buffer, not the last save.

## Ctrl+F12 — AI explain

Covered in Chapter 22. Brief: AI explains the diagnostic at cursor with ±5 lines of context.

## Ctrl+V R — render preview

Quick paragraph-level render. Saves the paragraph first (so the render reflects on-disk state), then rasterises every page at 144 dpi:

![figure: ctrl-v-r-modal](images/ctrl-v-r-modal.png) — Ctrl+V R: rendered PNG of the open paragraph. ← / → navigate pages, + / - zoom, S saves current page.

| Chord | What it does |
|-------|--------------|
| ← / → | Previous / next page. |
| Home / End | First / last page. |
| + / = | Zoom in (multiply ticks/cell by 0.66). |
| - / _ | Zoom out (1.5×). |
| 0 | Reset zoom to 1.00×. |
| S | Save the current page to PNG. |
| A | Save every page to PNGs. |
| Esc | Close the modal. |

Zoom range [0.05, 6.00]× the default DPI. Cursor's screen column stays anchored through zoom — zooming feels like drilling in.

## Save flows

`S` opens a save-as picker pre-populated with `<paragraph-slug>-YYYYMMDD-HHMM.png`. `A` writes one PNG per page with `-page-NNN.png` suffix.

Save always uses the default render DPI (not the zoom value) — zoom is for screen preview; save is for the artefact.

## When NOT to use render preview

- **Full-book layout** — Use `Ctrl+B B` (build) and open the PDF in a viewer. Per-paragraph render strips cross-paragraph context (headers, page numbering).
- **Mid-sentence rewrite loops** — the path saves first. If you're holding many small edits, save manually with `Ctrl+S` and preview in batches.

## Configuration

```hjson
images: {
  preview_enabled: true       # required for Ctrl+V R
}

typst_compile: {
  engine: "inprocess"         # required for render
  diagnostics_idle_seconds: 2.0
  diagnostics_max: 50
}
```

The `images.preview_enabled` switch matters because on terminals without graphics support the half-block fallback is too coarse to be useful — false makes the chord land a clean hint instead.

## `hook.on_diagnostic` recap

For full coverage see Chapter 22. The hook fires only on state changes (not every idle tick) — `( uuid count first-message -- )`.

## Recap

- Gutter `●` on every line with a diagnostic.
- `F8` lists diagnostics; Enter jumps cursor.
- `Ctrl+V N` / `Shift+N` navigate in place; `Ctrl+F12` AI explain.
- `Ctrl+V R` previews the open paragraph; `+/-` live zoom; `S` / `A` save PNGs.
- `hook.on_diagnostic` for Bund-side reactions.
