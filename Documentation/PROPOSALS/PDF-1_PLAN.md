# PDF-1 — implementation plan

_Inkhaven-grounded build plan for [RFC PDF-1](PDF-1.md) (PDF management &
imposition).  The RFC is the design of record; this maps it to the actual
codebase, fixes the integration seams, calls the dependency decision, and
sequences the work to inkhaven's release cadence.  Lands under the 1.3
**"From Draft to Submission"** theme as its print-production pillar._

## Where it plugs in (verified against the tree)

| RFC surface | Existing home | What's new |
| --- | --- | --- |
| PDF source (the linear book PDF) | `typst-pdf 0.14` via `typst_compile` (`Ctrl+B B`) | nothing — the subsystem *consumes* this output |
| CLI `inkhaven pdf …` | `cli/mod.rs` `Command` enum + dispatch (mirror `Command::Facts(FactsCommand)`) | `Command::Pdf(PdfCommand)` + `src/cli/pdf.rs` |
| TUI book-take | `Ctrl+B O` → `OutputConfig.extra_formats` (`markdown`/`tex`/`epub` today, in `src/export/`) | `imposed_pdf` / `cover_pdf` entries; `Ctrl+B I` imposition-preview overlay |
| Bund stdlib | `src/scripting/stdlib/` already uses `ink.*` (`ink.node.*`, `ink.review.*`) via `vm.register_inline` | `src/scripting/stdlib/pdf.rs` → `ink.pdf.*` |
| Outline injection | `assemble::assemble_book()` emits the combined `.typ` | emit `#metadata((node_id: …))` near each heading (P0) |
| Imposition config | `Config::load_layered` cascade (project + global `~/.config/inkhaven`) | top-level `imposition:` / `cover:` / `preflight:` keys (same merge semantics) |

So all three RFC surfaces have a precedent to copy — no new architecture,
just a new `src/pdf/` library wired into established seams.

## Dependency decision (the one real exception to make)

The subsystem needs a PDF object library; inkhaven has none.  Per the RFC,
**`lopdf`** (pure-Rust, MIT) for parse/write + page ops + imposition via
Form XObjects, and **`barcoders`** (pure-Rust, MIT/Apache) for EAN-13 —
or ~150 lines of hand-rolled EAN-13 to avoid even that.  `printpdf` is
rejected (lopdf alone covers cover layout).

This is a **deliberate exception to the project's zero-new-deps habit** —
justified because there is no in-tree way to manipulate PDF objects and
both crates are pure Rust, so the single-binary promise holds.  **Gate
before P0 starts:** audit `lopdf` + `barcoders` (and their transitive
trees) for (a) pure-Rust / no new native libs, (b) license compatibility,
(c) maintenance health, (d) `cargo deny` clean.  If `barcoders` drags in
anything, hand-roll EAN-13 (the RFC says ~150 lines incl. check digit).

## Module layout

`src/pdf/` exactly as the RFC §7.2 (`doc`, `geometry`, `paper`, `ingest`,
`ops`, `stamp`, `impose/{layout,creep,marks,sheet}`, `cover`, `barcode`,
`preflight`, `outline`, `meta`, `emit`).  `mod pdf;` in `main.rs`.  The
pure-math pieces (`geometry`, `paper`, `impose::layout`, `impose::creep`,
`barcode`) are unit-testable with no I/O — build and test those first
inside each phase, the way `replace.rs` separated the matcher from the
walk.

## Stability-bar requirements (1.2.15 standard applies)

- **Atomic emit.** `pdf::emit` writes the output PDF through
  `io_atomic::write` (never a torn PDF on crash).  Same for cover output.
- **No panic surfaces.** `PdfDoc` ops return `Result`; no `unwrap`/index
  panics on malformed input PDFs (external PDFs are untrusted bytes —
  treat every `lopdf` access as fallible, map to a `pdf::Error`).
- **No source compromise** (RFC §5): the PDF path never touches the Typst
  source; imposition is always re-runnable from a fresh render.
- **Broken-pipe-safe CLI** (1.2.23): `inkhaven pdf preflight … | head`
  must not crash — already handled by the panic-hook fix, just don't
  re-introduce a custom output path that bypasses it.

## Phased build (mapped to inkhaven releases)

The RFC's ~10-week / one-dev estimate is a *large* subsystem — bigger than
a single point release at this cadence.  Each RFC phase is independently
shippable, so map them to successive 1.3.x cuts:

### P0 — Foundations → **1.3.0**
`PdfDoc` + `PdfSource`, `geometry`, `paper`, `ingest`, `emit` (atomic),
`meta`, `ops` (extract/split/merge/rotate/reorder/delete), `outline`
injection **including the `assemble` `#metadata` marker change**.  Surfaces:
`Command::Pdf` with the page-op + metadata + outline subcommands;
`src/scripting/stdlib/pdf.rs` for those words.  Tests: geometry/paper
unit tests, page-spec parsing, **a corpus test of `lopdf` round-tripping
real inkhaven typst-pdf output** (with images, embedded fonts, vector
content) — this is the make-or-break risk gate (RFC §14); if `lopdf`
mangles typst output, the whole RFC is blocked, so P0 proves it first.

### P1 — Imposition → **1.3.1**
`impose/` complete: layout math (4 binding styles), creep (shingle/
pushout), all marks, sheet emission via Form XObjects.  `inkhaven pdf
impose`; the `Ctrl+B I` preview overlay (ratatui pane + ASCII schematic,
RFC App.D); `imposed_pdf` book-take format; `ink.pdf.impose`.  Tests:
**property tests** (every source page appears once; pairs share a sheet;
creep monotonic; signature sums) + the `--test` numbered-page mode for
hand-fold visual verification.

### P2 — Cover · barcode · preflight → **1.3.2**
`cover` (spine-width calc from page-count + paper stock), `barcode`
(EAN-13 + check digit), `preflight` (the image-DPI check is the
highest-value one).  CLI subcommands; `cover_pdf` book-take format; the
`pdf sample` convenience command.

### P3 — Polish → **1.3.3**
`grayscale`, `optimize`, `metadata strip`, watermark/stamp variants;
`Documentation/Tutorials/65-hand-binding.md`; KEYBINDING (`Ctrl+B I`),
CONFIGURATION (`imposition:`/`cover:`/`preflight:`).

(Or compress P0+P1 into 1.3.0 if velocity allows — they're the marquee.
The point is each cut is preflight-clean and self-contained.)

## Inkhaven-specific risks / decisions to settle before P0

1. **`lopdf` ↔ typst-pdf fidelity** — *the* gating risk.  De-risk in the
   first days of P0 with the corpus round-trip test, before building on
   top.  If gaps: upstream patch or a carried patch set (RFC §14).
2. **The `assemble` `#metadata` change touches a shipped feature.**  It
   must be additive + invisible to existing output (markers are inert to
   the renderer) and gated so hand-written `.typ` (no markers) degrades to
   an unannotated PDF + a `--no-outline` note, never an error.
3. **`extra_formats` ordering dependency.**  `imposed_pdf` requires `pdf`
   first; the take engine must enforce build-then-impose ordering (RFC
   §10.3) and let an optional format fail without aborting the take (the
   existing extras behaviour already does per-format error isolation).
4. **Bund PDF value semantics** (RFC open-Q 3) — `ink.pdf.load` returns a
   ref-counted handle, not a copied document, with explicit `ink.pdf.clone`.
   Confirm the Adam VM supports opaque handle values.
5. **Zero-new-deps stance** — adding `lopdf` is the first runtime dep in
   several cycles; worth an explicit OK, since it's a standing project
   value (see the release/stability memory).

## Relationship to the rest of "From Draft to Submission"

PDF-1 is the **print-production** pillar.  The other Draft→Submission
pieces are *separate* tracks (not in this RFC), to scope independently:

- **Standard manuscript format** export (double-spaced, 12pt, running
  headers, `#` scene breaks, rounded word count) — a new `export` target,
  not a PDF-object operation.
- **Submission package** — RAG-grounded query-letter / synopsis / comp-
  title helpers (a great whole-book AI use) + a lightweight tracker.

These compose with PDF-1 but don't depend on it; PDF-1 can lead because it
has a complete RFC.

## Implementation log

_(entries land here as the work lands)_
