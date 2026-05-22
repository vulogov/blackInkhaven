# 24 — Typst in-process: render, diagnose, preview

Inkhaven 1.2.5 absorbs the Typst compiler in-process. You can
flip a single HJSON knob and the editor stops shelling out to
the `typst` CLI for builds, gains semantic diagnostics next to
parse errors, renders any paragraph to a floating PNG preview,
and `inkhaven doctor`'s the whole setup at a glance.

All of it ships in every 1.2.5 build. The default `engine`
stays `external` so existing setups see zero behaviour change
until you opt in.

## TL;DR — flip the switch

```hjson
typst_compile: {
  engine: "inprocess"   // was "external"
}
```

Re-launch inkhaven. `Ctrl+B B` now compiles inside the
inkhaven process. `Ctrl+B V` (credits) shows
`Typst engine` → `internal · fonts: bundled + system · @preview: on`.
`inkhaven doctor` agrees.

Switch back any time by setting `engine: "external"` (or just
deleting the field — `"external"` is the default).

## The engine line

Three places report which engine is active:

1. **`inkhaven doctor`** — explicit `Typst engine` section
   with every related knob.
2. **`Ctrl+B V` credits modal** — `Typst engine` row, with
   the same summary string.
3. **The compile splash** (`Ctrl+B B`, `Ctrl+B O`) — `Engine:`
   line above `Elapsed:` so you see what's running while it
   runs.

Format:

```
internal · fonts: bundled + system · @preview: on
external · /usr/local/bin/typst
external · `typst` NOT FOUND on PATH
```

## Fonts

The in-process engine ships **bundled** with Computer Modern
and Linux Libertine (the typst-kit `embed-fonts` set) and
**also** searches your system fonts. Either source can be
toggled independently:

```hjson
typst_compile: {
  bundle_fonts:      true   // ship CM + Linux Libertine in the binary (~10 MB)
  use_system_fonts:  true   // also fontdb-search the user's system
}
```

Disable **both** and the World has zero fonts — every compile
errors with `font not found`. The doctor's `Notes` section flags
this combination explicitly.

For **hermetic / reproducible builds**: `bundle_fonts: true` +
`use_system_fonts: false` — the host's font collection no
longer affects layout. PDFs will be byte-identical across
machines that run the same inkhaven binary.

## `@preview/<pkg>` package resolution

```typ
#import "@preview/cetz:0.3.0": *
```

Just works. First time a package is referenced, it's fetched
from `packages.typst.org` over HTTPS (via `typst-kit`'s
`ureq`-based downloader — no tokio dep) and cached at the
platform standard:

| Platform | Cache |
|---|---|
| macOS   | `~/Library/Caches/typst/packages` |
| Linux   | `~/.cache/typst/packages` |
| Windows | `%LOCALAPPDATA%\typst\packages` |

`inkhaven doctor` prints the path + total cache size in its
**Package cache** section.

For **offline / hermetic builds**: set `packages_enabled:
false`. The compiler then refuses every `@preview/...` import
with a clean `package fetching is disabled` diagnostic, no
network attempt.

## Parse-time diagnostics (Phase 1 — 1.2.5+)

```hjson
typst_compile: {
  diagnostics:              true   // run typst-syntax on save / idle
  diagnostics_idle_seconds: 2      // debounce
}
```

On every save and on every `diagnostics_idle_seconds` of
editor idle, inkhaven re-parses the open paragraph via
`typst-syntax`. The first parse error lands on the status
bar:

```
typst: line 12:5 — expected `}`, found end of file
```

No engine dependency — `diagnostics` works whether
`engine = "external"` or `"inprocess"`. Bund and HJSON
content types are skipped automatically.

## Semantic diagnostics (opt-in)

```hjson
typst_compile: {
  semantic_diagnostics: true   // requires engine = "inprocess"
}
```

When **both** flags are on and the parse check is clean, a
full `typst::compile` runs against the open paragraph in
isolation and surfaces semantic errors (`#unknown_func` →
"unknown variable", type mismatches, missing-font references).

**False positives are expected** for paragraphs that depend on
book-level definitions (custom `#show` rules, `#let`-defined
helpers in your book's preamble). The isolated compile doesn't
see them. Turn off if your manuscript style is preamble-heavy;
leave on for self-contained scenes.

### Navigate between diagnostics — Ctrl+V N (1.2.5+)

Press `Ctrl+V N` to jump the editor cursor to the next
diagnostic in the buffer. Wraps at the end. Status bar
reports `diag 2/5  line 12:5  — <message>`. Combined with the
parse / semantic checks above this gives a tight feedback
loop: type, save, hit Ctrl+V N to land on the first problem,
fix, repeat.

## Render preview — Ctrl+V R (1.2.5+)

The fun one. Press `Ctrl+V R` with a Typst paragraph open.
Inkhaven saves the buffer, compiles it in-process via
`typst-render`, and floats the rendered PNG on top of the
editor:

```
┌── 🖨 The storm  ·  1024×558 · page 1/3 ────────────────────┐
│                                                            │
│       [ rendered PNG appears here ]                        │
│                                                            │
│  ←/→ navigate · S saves current · A saves all · Esc closes │
└────────────────────────────────────────────────────────────┘
```

Inside the preview:

| Key | Action |
|---|---|
| `←` / `→` (or `↑` / `↓`) | Previous / next page |
| `Home` / `End` | First / last page |
| `S` | Save **current page** at full DPI (288 dpi) via a save-as picker |
| `A` | Save **every page** — picker takes a base path; files land as `<base>-page-001.png`, `<base>-page-002.png`, … |
| `Esc` | Close back to the editor |

Cancelling the save picker (`Esc` while it's open) restores
the preview with navigation state intact. Successfully writing
a file closes back to the editor.

Preview DPI is 2.0 px/pt (~144 dpi); save DPI is 4.0 px/pt
(~288 dpi). Both are fixed in 1.2.5; per-render zoom is on the
1.2.6 roadmap.

Requires a terminal with graphics support (kitty / iterm2 /
sixel / unicode half-blocks via ratatui-image). Terminals
without graphics surface a status-bar hint instead of opening
an empty modal.

## TUI integration

The in-process engine is a peer of the external CLI in every
TUI flow:

- **Compile splash** (`Ctrl+B B`, `Ctrl+B O`) animates the
  spinner during in-process compiles too — runs on a worker
  thread, foreground stays responsive.
- **Esc cancels** in-flight compiles. External: `Child::kill`
  (SIGTERM). In-process: cancel flag flips, foreground
  unblocks immediately, worker keeps running until typst
  finishes naturally (typst is deterministic and bounded —
  worst case a few seconds of CPU after you gave up).
- **Autosave before A/B/O** — Ctrl+B A (assemble), Ctrl+B B
  (build), Ctrl+B O (take) all save the primary editor (and
  the secondary editor in similar-paragraph mode) before
  walking `.typ` files. No more "I just pressed Ctrl+B B and
  the build used yesterday's saved version".

## `inkhaven doctor`

The new diagnostic CLI. Prints three sections + warnings:

```sh
$ inkhaven doctor
inkhaven doctor — v1.2.5
================================================================

─── Binary ───
  version                          1.2.5
  description                      Inkhaven — TUI literary work editor for Typst books
  rust-version (min)               1.85
  repository                       https://github.com/vulogov/blackInkhaven

─── Typst engine ───
  engine                           internal · fonts: bundled + system · @preview: on
  external typst path              /usr/local/bin/typst
  bundle_fonts (HJSON)             true
  use_system_fonts (HJSON)         true
  packages_enabled (HJSON)         true
  semantic_diagnostics (HJSON)     false

─── Package cache ───
  path                             /Users/you/Library/Caches/typst/packages
  entries                          668
  size                             5.8 MB

─── Project ───
  root                             /Users/you/Projects/tides
  status                           initialised
  config                           …/inkhaven.hjson
  user books                       2
  paragraphs (user)                412
  words (user paragraphs)          82,431

─── Notes ───
  no warnings
```

The **Notes** section calls out actionable problems like
`engine = external but typst is NOT on PATH` or `engine has
BOTH bundle_fonts AND use_system_fonts disabled`. Run from
anywhere — works inside and outside an inkhaven project; the
**Project** section just collapses when you're outside one.

Pipe-friendly: no colour, no TTY tricks. Great for CI
preflight (`inkhaven doctor | grep '⚠'`).

## Migration from 1.2.4

Zero. `engine` defaults to `"external"`, `diagnostics`
defaults to `true` (parse-only — the same surface 1.2.4 didn't
have but won't surprise you), `semantic_diagnostics` defaults
to `false`, `bundle_fonts` / `use_system_fonts` /
`packages_enabled` all default to sensible values that only
matter when you flip to `inprocess`. Existing HJSON configs
load unchanged.

## Known limits in 1.2.5

- **Semantic-diagnostic false positives** for paragraphs that
  reference book-level definitions. See "Semantic diagnostics"
  above; leave off when in doubt.
- **Per-render DPI is fixed.** The 2.0 / 4.0 px/pt split is
  baked in for now; per-preview zoom (`+` / `-`) is on the
  1.2.6 list.
- **Render preview is on-demand.** Always-on side-by-side live
  preview is a separate feature; the current chord-based
  preview is the foundation.
- **In-process engine version is pinned** to the typst release
  inkhaven was built against (currently `0.14.x`). If you
  alternate `external` and `inprocess` across builds, keep
  your host's `typst` binary on the same major.minor for
  byte-identical PDFs.

## See also

- [`KEYBINDING.md`](../KEYBINDING.md) — full chord table including
  the new view sub-chords (R / N).
- [`CONFIGURATION.md`](../CONFIGURATION.md) — every `typst_compile.*`
  field with defaults.
- [`INKHAVEN_CHEAT_SHEET.typ`](../INKHAVEN_CHEAT_SHEET.typ) — printable
  one-pager.
- [`RELEASE_NOTES/1.2.5.md`](../RELEASE_NOTES/1.2.5.md) — the headline
  story + every feature in this cycle.
