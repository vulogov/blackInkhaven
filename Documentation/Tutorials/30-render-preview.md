# 30 — Render preview with zoom

Inkhaven 1.2.5 added **paragraph-level render preview** —
`Ctrl+V R` rasterises the open paragraph through the
in-process typst engine and floats the PNG on top of the
editor. 1.2.6 added live `+ / -` zoom inside the modal.

This tutorial assumes you've already set
`typst_compile.engine = "inprocess"` per
[`24-typst-in-process.md`](24-typst-in-process.md). Without
the in-process engine `Ctrl+V R` falls back to a hint pointing
at the HJSON setting.

## The chord

`Ctrl+V R` (Editor scope, default action `view.render_paragraph`):

1. Saves the paragraph if dirty (so the render reflects the
   on-disk state, including any `prompts_file` / `globals.typ`
   imports).
2. Synthesises a minimal typst document around the paragraph
   body (the book's `globals.typ` + `settings.typ` get pulled
   in if they exist).
3. Compiles every page at ~144 dpi (2.0 ppt = "pixels per
   typst point") and emits a `Vec<PngBytes>`.
4. Floats the first page in a modal:

```
┌── render ¶ `The Storm` · 1240×1640 · +/- zoom · S saves current · A saves all
│
│   [rendered PNG of page 1]
│
│
│
│   ←/→ navigate · S saves · A saves all · Esc closes ──────────────────────────┘
```

(Terminals with kitty / iterm2 / sixel graphics show the
inline image; half-block-capable terminals fall back to
coarser preview.)

## Inside the modal

| Chord                | Effect |
|----------------------|--------|
| `←` / `↑`            | Previous page. |
| `→` / `↓`            | Next page. |
| `Home` / `End`       | Jump to first / last page. |
| `+` / `=`            | **Zoom in** — multiply ticks/cell by 0.66 (≈ 1.5× display). |
| `-` / `_`            | **Zoom out** — multiply by 1.5 (≈ 0.66× display). |
| `0`                  | Reset zoom to 1.00× (default density, recentered cursor). |
| `S`                  | Save the current page to PNG. |
| `A`                  | Save every page to PNGs. |
| `Esc`                | Close the modal. |

## Live zoom (1.2.6+)

Each `+` / `-` press re-renders every page at the new PPI
and swaps the modal's page list in place. The current page is
preserved (clamped to the new length if zoom-in produced more
pages).

```
… · 1240×1640 · zoom 1.00×       (default)
… · 1860×2460 · zoom 1.50×       (after +)
… · 837×1107  · zoom 0.67×       (after - from default)
```

Range: [0.05, 6.00]× the default PPI. At the limit the
status bar reports `"render ¶: zoom at limit"` instead of
silently no-oping.

The cursor's screen column stays anchored through zoom — so
zooming in feels like "drill into this part" rather than
"jump to the start". Same pattern as the timeline view (see
[`31-story-timeline.md`](31-story-timeline.md)).

## Save flows

`S` (single) opens a save-as picker pre-populated with
`<paragraph-slug>-YYYYMMDD-HHMM.png`:

```
┌── Save rendered page ──────────────────────────────────┐
│ Path: │/Users/gandalf/work/the-storm-20260522-1432.png│ │
│ Enter writes · Esc cancels                              │
└─────────────────────────────────────────────────────────┘
```

The default lands in the launch directory (your project root
or whatever you started `inkhaven` from), so subsequent saves
batch naturally.

`A` (all) variants the filename:
`<paragraph-slug>-YYYYMMDD-HHMM-page-001.png`,
`-page-002.png`, etc. — one PNG per rendered page, written in
sequence. Useful for multi-page paragraphs (rare — paragraphs
are usually < 1 page — but happens with embedded figures or
long quotations).

The save flow always uses the **default render DPI**
regardless of the modal's current zoom — zoom is for screen
preview, save is for "publish the artefact".

## When to use it

- **Quick rich-formatting check** — see how `*bold*` /
  `#emph[…]` / blockquotes / math render without leaving the
  editor.
- **Catch overflow** — long lines, malformed margins, missed
  hyphens. Visible on the rendered page even when typst
  compiles cleanly.
- **Verify imports** — when you reference a function defined
  in `globals.typ`, the parse / semantic check (see
  [`27-diagnostics.md`](27-diagnostics.md)) might let through
  what the render reveals.
- **Preview a figure** — paragraph with `#image("cover.png")`
  shows the actual image as it'll appear in the book.

## When NOT to use it

- **Full-book preview** — use `Ctrl+B B` ("build the book") or
  `inkhaven export pdf` and open the artefact in a real PDF
  viewer. Per-paragraph render strips the book's structural
  context (headers, page breaks, chapter numbering).
- **Long paragraphs on big books** — rendering walks the
  whole book's imports + fonts. A first render after opening
  the project can take several seconds.
- **Mid-sentence rewrite loops** — the render path saves the
  paragraph first. If you're holding many small edits, save
  manually with `Ctrl+S` and render in batches.

## Configuration

The render modal honours these knobs:

```hjson
images: {
  # Default true. When false, Ctrl+V R lands a status hint
  # explaining why no image will show up. Set to false on
  # terminals without graphics support to avoid the misleading
  # "fall back to half-blocks" path.
  preview_enabled: true
}

typst_compile: {
  # In-process is REQUIRED for Ctrl+V R. The external CLI
  # engine doesn't expose the per-paragraph render path.
  engine: "inprocess"

  # Fonts to bundle in the binary. `bundle_fonts: true` (default)
  # ships Computer Modern + Linux Libertine so paragraph
  # render works on hosts without system fonts.
  bundle_fonts: true

  # Allow @preview/<pkg>/... imports. When false, paragraphs
  # that depend on @preview packages will render with a
  # "package not found" diagnostic instead of the actual
  # content.
  packages_enabled: true
}
```

## Recap

- `Ctrl+V R` — float a rasterised PNG of the open paragraph.
- `←/→` page · `+/-` zoom · `0` reset · `S` save current ·
  `A` save all · `Esc` close.
- Zoom range [0.05, 6.00]×; cursor column stays anchored.
- Save flow uses default render DPI (not the zoom value) — the
  save is the artefact, the zoom is just the preview.
- Requires `typst_compile.engine = "inprocess"` (see
  [`24-typst-in-process.md`](24-typst-in-process.md)).
