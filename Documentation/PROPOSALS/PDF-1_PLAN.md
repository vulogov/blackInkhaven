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
both crates are pure Rust, so the single-binary promise holds.

### Audit result (2026-06-11) — GREEN ✓

Empirically resolved `lopdf 0.41.0` + `barcoders 2.0.0` against a fresh
tree and diffed against inkhaven's:

- **No external-app dependency, no native-library build.** The *only*
  `-sys` crate anywhere in the combined tree is `core-foundation-sys`,
  which is **already in inkhaven** and is a pure-Rust macOS OS-FFI shim
  (not an external app/lib).  No `cc`/`cmake`/`bindgen`/`pkg-config`/
  `openssl`/`freetype`/`pdfium`/`ghostscript`.  Confirmed by building
  `lopdf` + `barcoders` from scratch with no external tool, and a runtime
  round-trip (load → save → reload a PDF) + EAN-13 generation, both clean.
- **10 new crates, all pure Rust, all permissive (MIT or MIT/Apache):**
  `lopdf`, `barcoders`, the RustCrypto set pulled for encrypted-PDF
  reading (`aes`, `cbc`, `ecb`, `cipher`, `block-padding`, `inout`),
  `rangemap`, `stringprep`.  The crypto crates are non-optional (can't be
  feature-gated off) but are tiny, well-audited RustCrypto.
- **Everything else lopdf needs is already vendored** — `flate2` +
  `miniz_oxide` + `zlib-rs` (pure-Rust compression), `nom`, `weezl`,
  `encoding_rs`, `md-5`, `linked-hash-map`, `rayon`, `time`.
- **Trim option:** lopdf's default features pull three date libs
  (`chrono` + `jiff` + `time`); add it with `default-features = false`
  and reuse inkhaven's existing `time` (or `chrono`) to avoid `jiff`.
- **Verdict:** meets the constraint exactly — solves the PDF problem with
  pure-Rust crates, zero external-app dependency.  inkhaven already
  vendors far heavier native deps (DuckDB via `libduckdb-sys`, ONNX via
  `ort-sys`); `lopdf` adds none.  **Cleared to add.**  (Still run
  `cargo deny` at add-time for ongoing advisory/license tracking.)
  **`barcoders` is kept** (project decision) rather than hand-rolling
  EAN-13 — the crate is clean and audited; a hand-rolled encoder would be
  avoidable tech debt.

## Module layout

`src/pdf/` exactly as the RFC §7.2 (`doc`, `geometry`, `paper`, `ingest`,
`ops`, `stamp`, `impose/{layout,creep,marks,sheet}`, `cover`, `barcode`,
`preflight`, `outline`, `meta`, `emit`).  `mod pdf;` in `main.rs`.  The
pure-math pieces (`geometry`, `paper`, `impose::layout`, `impose::creep`,
`barcode`) are unit-testable with no I/O — build and test those first
inside each phase, the way `replace.rs` separated the matcher from the
walk.

## Configuration — HJSON through the cascade (firm)

All PDF-subsystem configuration is HJSON, deserialized through the
existing `Config::load_layered` cascade — **the same path as every other
setting**, so it merges defaults → project `inkhaven.hjson` → global
`~/.config/inkhaven/config.hjson` + `conf/*.hjson`, with the global
layer winning (1.2.20).  That means:

- **`imposition:`** (binding style, sheets-per-signature, sheet size,
  margins, creep + paper stock, marks, blank-page policy), **`cover:`**
  (paper stocks, spine text, barcode), **`preflight:`** (target DPI,
  profile) are top-level keys, each supporting **named profiles** (RFC
  App. A — e.g. `default`, `chapbook`) selected by name from the CLI
  (`--config <key>`) / book-take (`imposed_pdf_config`) / Bund.
- A user can keep a house imposition profile in their **global** config
  and have it apply to every project — exactly the override pattern the
  1.2.20 cascade was built for.
- New `ImpositionConfig` / `CoverConfig` / `PreflightConfig` serde
  structs land in `config.rs` in **P1/P2**, with `#[serde(default)]` so a
  project that sets nothing still gets sane defaults.

No imposition parameter is hard-coded or CLI-only; the config block is
the source of truth, the CLI/TUI/Bund just select a profile or override
individual fields.

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

### P0.1 — deps added + fidelity gate PASSED (landed)

- `lopdf 0.41` (`default-features = false`, `features = ["chrono",
  "rayon", "time"]` — drops the redundant `jiff`) + `barcoders 2` added
  to `Cargo.toml`; both compile cleanly into inkhaven.
- New `src/pdf/` module (`mod pdf;`), so far just the **fidelity gate**:
  `src/pdf/mod.rs` `corpus_tests::lopdf_round_trips_typst_pdf_output`.
  It compiles a rich typst body (heading + bold/italic prose + `#line` /
  `#rect` / `#circle` vector + a real PNG `#image` + a `#pagebreak`) to
  genuine typst-pdf bytes via `InkhavenWorld::in_memory` + `typst_pdf::pdf`
  (the same path `Ctrl+B B` uses), then asserts `lopdf` (1) parses it,
  (2) sees the embedded font subset (`FontDescriptor` + `FontFile*`),
  (3) sees the image XObject (`Subtype /Image`), and (4) round-trips
  (load → save → reload) preserving the 2-page tree.
- `#[ignore]`d (compiles typst, per the existing convention), run with
  `cargo test --bin inkhaven -- --ignored lopdf_round_trips`.  **Result:
  PASS** — RFC §14's make-or-break risk is cleared; `lopdf` handles real
  typst-pdf output (fonts, images, vector, multi-page).  Normal suite
  1190 + 1 ignored gate.

Next P0: `PdfDoc`/`PdfSource`, `geometry`, `paper`, `ingest`, `emit`
(atomic), `meta`, `ops`, then `outline` (with the `assemble` `#metadata`
change), wired to `Command::Pdf` + `ink.pdf.*`.

### P0.2 — geometry + paper + PdfDoc (landed)

The pure-math core + the value type, all unit-tested:

- **`pdf::geometry`** — `mm`/`pt`/`in` conversions, `Size` (points, with
  portrait/landscape), `Rect` (points, PDF `MediaBox` order, corner-
  normalising), and `page_size(name)` presets (ISO A3–A6/B5, US
  Letter/Legal/Tabloid, trade trims 6×9 / 5.5×8.5 / pocket).  4 tests.
- **`pdf::paper`** — `PaperStock { name, thickness_mm }` + the RFC §8.2.4
  preset table, case-insensitive `paper_stock(name)`, `DEFAULT_INTERIOR`
  / `DEFAULT_COVER`, `binding_compensation_mm`.  4 tests.
- **`pdf::doc`** — `PdfDoc` (wraps `lopdf::Document`, caches page order +
  inherited `MediaBox` sizes) and `PdfSource` (`Inkhaven { typst_root }`
  / `External`).  `load` / `load_mem` / `from_document`, `page_count` /
  `page_ids` / `page_size`, `to_bytes`, and **`save` via `io_atomic`**
  (atomic emit — no torn PDF).  `page_mediabox` walks the `Parent` chain
  (inheritable attr) with a cycle guard + Letter fallback.  3 tests
  (build a minimal n-page PDF → load → assert pages/sizes/source →
  round-trip).
- **`pdf::Error`/`Result`** (mod.rs): `Lopdf` / `Io` / `NotInkhavenSource`
  / `Other`, with `From<lopdf::Error>` + `From<io::Error>`.

Module-level `#![allow(dead_code)]` (the library is built ahead of its
`Command::Pdf` / `ink.pdf.*` wiring; removed when the first caller lands).
Suite 1190 → 1201.

Next P0: `ingest` (richer page indexing), `ops` (extract/split/merge/
rotate/reorder/delete via the lopdf page tree), `meta`, then `outline`
(with the `assemble` `#metadata` change).
