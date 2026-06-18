# 23 — Typst inside Inkhaven

Inkhaven's typesetter is Typst — a modern, academic-friendly language that compiles markup to beautifully-typeset PDFs. The compiler lives *inside* the inkhaven binary; you don't need to install Typst separately.

## Two engines

Two compile paths ship in 1.2.5+:

| Engine | Description |
|--------|-------------|
| `engine: "external"` | Default. Shells out to a `typst` binary on `PATH`. |
| `engine: "inprocess"` | 1.2.5+. The full Typst compiler + fonts + `@preview` packages linked into the inkhaven binary. |

Set in HJSON:

```hjson
typst_compile: {
  engine: "inprocess"

  bundle_fonts:    true     # Bundle CM + Linux Libertine.
  use_system_fonts: true    # Also walk host fonts.
  packages_enabled: true    # Allow @preview/<pkg> resolution.
}
```

## Why the in-process engine matters

Most TUI features that touch typst (render preview, semantic diagnostics, per-page navigation) require the in-process engine. The external CLI engine is faster to launch but can't expose the structured `SourceDiagnostic` data the in-process compiler returns.

If you ever see "this feature needs `engine = \"inprocess\"`" in the status bar, that's why.

## Per-book Typst skeleton

Every user book gets three auto-generated typst files in `books/<book-slug>/`:

| File | Role |
|------|------|
| `index.typ` | The entry point. Inkhaven appends every paragraph's body to this. |
| `settings.typ` | Document-wide `#set` / `#show` rules. Edit to customise. |
| `globals.typ` | Functions / variables imported everywhere. Edit for custom typography. |

Edit them in any text editor. Inkhaven re-reads them on every build / render. The default `globals.typ` is empty; the default `settings.typ` sets page size + font + paragraph indent from `typst_page` / `typst_fonts` / `typst_layout` in HJSON.

## `@preview` packages

Typst's package ecosystem (`@preview/...`) works when `packages_enabled: true`. First reference downloads the package + caches it; subsequent references are offline.

```typst
#import "@preview/codly:1.0.0": *

#show: codly-init.with()

#raw(
  "fn main() { println!(\"hello\"); }",
  lang: "rust",
)
```

Cache lives in `~/.cache/typst/packages/`. Inkhaven respects typst's existing locations so packages downloaded by the external CLI are reused by the in-process engine.

## Diagnostics from Typst

Two checks run on the open paragraph (Chapter 24 covers the full surface):

| Check | Engine required |
|-------|------------------|
| Parse | `typst-syntax` only. Always on. Catches syntax errors. |
| Semantic | Full compile via the in-process engine. Catches reference errors, type errors, missing imports. |

The parse check is engine-independent. The semantic check requires `engine: "inprocess"`.

## Doctor — check the setup

```
inkhaven doctor
```

Reports engine choice, fonts available, package cache state, and any actionable warnings. Run after changing engines or moving the project.

![figure: doctor-typst-section](images/doctor-typst-section.png) — `inkhaven doctor`: typst section. Engine, fonts (system + bundled), package cache, and any warnings.

## Font bundling

When `bundle_fonts: true` (default), the binary ships Computer Modern + Linux Libertine inside it. A headless CI machine without system fonts can still build the book. Adds ~10 MB to the binary.

For book-specific custom fonts, drop the `.ttf` / `.otf` files into `books/<book-slug>/fonts/` and add a `#set text(font: ...)` rule in `settings.typ`. Inkhaven's in-process engine sees them.

> **Fonts not found?** Two common causes: (1) `bundle_fonts: false` AND `use_system_fonts: false` — no fonts at all. (2) The font name in `#set text(font: ...)` doesn't match the installed font's family name. `fc-list` (Linux) or Font Book (macOS) is the truth-source.

## Recap

- Two engines: `external` (default) shells out; `inprocess` (1.2.5+) is built-in.
- Most TUI typst features need `inprocess`.
- Per-book skeleton: `index.typ`, `settings.typ`, `globals.typ`.
- `@preview/<pkg>` works when `packages_enabled: true`.
- `inkhaven doctor` reports the setup; bundled fonts ship in the binary.
