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

### P0.3 — ops + meta (landed)

- **`pdf::ops`** — page operations on the lopdf page tree:
  - `PageSpec` (`All`/`Single`/`Range`/`List`, 1-based) with a CLI
    parser (`1,3,5-8,12` / `all`) and `resolve(count)` (sorted, deduped,
    clamped, reversed-range tolerant).
  - `extract` (clone + delete-complement + prune; source untouched),
    `delete` (in place, guards emptying), `rotate` (`Rotation` 90/180/270,
    accumulates `/Rotate`), `reorder` (0-based permutation, validated),
    `split` (`EveryNPages` / `OnPages`, via `extract`).
  - **`merge`** reparents each source's page **subtree** under a fresh
    top `Pages` node (object-IDs renumbered via `renumber_objects_with`)
    rather than flattening — so inherited `MediaBox` / `Resources`
    survive.  Validated on *real typst output*: merging two typst PDFs
    keeps the image XObjects (new `#[ignore]` corpus gate
    `merge_preserves_typst_resources`).
  - 7 unit tests + the merge corpus gate.
- **`pdf::meta`** — `PdfMetadata` (title/author/subject/keywords/creator/
  producer) over the `Info` dictionary: `read_metadata`, `write_metadata`
  (creates `Info` if absent; `None`/empty removes), `strip_metadata`.
  PDF text strings are ASCII-literal or UTF-16BE-with-BOM (non-ASCII
  titles round-trip).  3 unit tests (round-trip, strip, unicode title).
- Shared `test_support::minimal_pdf` fixture (used by doc/ops/meta).

Normal suite 1201 → 1211; 2 ignored corpus gates (run with
`cargo test --bin inkhaven -- --ignored`).

Next P0: `outline` injection (the additive `assemble` `#metadata` change),
then the surfaces — `Command::Pdf` (`src/cli/pdf.rs`) + `ink.pdf.*`
(`scripting/stdlib/pdf.rs`) — which retire the module `dead_code` allow.

### P0.4 — outline injection + assemble marker + feature-proof (landed)

- **`pdf::outline`** — `OutlineItem { title, page, children }`,
  `inject_outline` (builds a `/Outlines` bookmark tree with proper
  First/Last/Next/Prev/Parent/Count linking + `/Dest [page /Fit]`,
  page-index clamped, replaces any existing outline) and `read_outline`
  (reads it back; handles `/Dest` arrays *and* `/A /GoTo /D` actions, so
  it also reads typst's own heading bookmarks).  3 unit tests
  (nested round-trip, unicode title + page clamp, empty).
- **Assemble marker (additive)** — `build_branch_index` now emits a
  zero-size `#metadata((node_id: "<uuid>"))` at each branch's body start.
  `#metadata` is query-only — invisible to layout, absent from the PDF —
  and carries no label (so branches can't collide).  The hook for the
  future node↔page correlation (introspector query at compile time);
  nothing consumes it yet, so output is byte-for-byte the same to the
  reader.
- **End-to-end feature-proof** (`full_pdf_feature_proof`, `#[ignore]`d) —
  compiles a synthetic multi-chapter `.typ` *including the `#metadata`
  marker* (proving it compiles inertly) and exercises the whole
  manipulation set on the real PDF: meta write/read/strip, outline
  inject/read round-trip, extract, split (counts sum back), merge (image
  survives), rotate, delete, reorder, and an atomic `save`→`load`.

The PDF *manipulation* feature set (P0 minus the surfaces) is now complete
and proven on real typst output.  Normal suite 1211 → 1214; 3 ignored
corpus gates.

Next: the **surfaces** — `Command::Pdf` (`src/cli/pdf.rs`) + `ink.pdf.*`
(`scripting/stdlib/pdf.rs`) — which make all this usable and retire the
module `dead_code` allow.

### P0.5 — `Command::Pdf` CLI surface (landed)

`inkhaven pdf <subcommand>` (`src/cli/pdf.rs` + `PdfCommand` in
`cli/mod.rs`) over the P0 library:

- `info` (pages, page-1 size in pt+mm, source, title/author, outline
  size), `extract --pages`, `delete --pages`, `rotate --pages --degrees`,
  `reorder --mapping` (1-based), `split --every|--at [--out-dir]`,
  `merge <inputs>... --out`, `metadata` (read / `--title|--author|
  --subject|--keywords` set / `--strip`), `outline` (list bookmarks).
- Output via a `write_pdf` helper: **atomic** (`PdfDoc::save` →
  `io_atomic`) and **creates the parent dir**; a mutating op never
  silently overwrites its input — defaults to a `<stem>-<op>.pdf` sibling.

**Live-verified** end-to-end on a real **169-page** typst-built book
(`inkhaven build --compile` → `inkhaven pdf …`): `info` (A4, 3 bookmarks),
`outline` correctly read typst's own heading bookmarks (Ch1→p.1, Ch2→p.56,
Ch3→p.111 — proving `read_outline` handles typst's `/A /GoTo` actions),
metadata set/read (creator `Typst 0.14.2`), extract, merge→171pp, rotate,
reorder — no crash artifacts.  One bug found + fixed: `split` into a
non-existent `--out-dir` (now created first).

The library now has real callers — `doc`/`ops`/`meta`/`outline` are live;
the module `dead_code` allow covers only the P1/P2-bound remainder
(`paper`, imposition geometry, `inject_outline`, the `ink.pdf.*` layer).
Suite 1214 (+ 3 ignored corpus gates).

P0 (foundations) is functionally complete bar the `ink.pdf.*` Bund layer.

### P0.6 — `ink.pdf.*` Bund stdlib (landed)

`src/scripting/stdlib/pdf.rs`: PDFs held as opaque **handles** (ints) in
a thread-local registry (RFC §9 open-Q 3 — no copy on every word
boundary).  Words: `ink.pdf.load` ( path -- h ), `save` ( h path -- ),
`pages` ( h -- n ), `extract` ( h spec -- h' ), `delete` / `rotate` /
`reorder` ( … -- h, in place), `merge` ( h1 h2 -- h', consumes inputs),
`title` / `set_title` / `set_author` / `strip_metadata`.  Thin wrappers
over the proven `pdf` lib.

Sandbox: only the disk-crossing words are categorised in `policy.rs` —
`ink.pdf.load` → `fs_read` (default-allowed, path-confined to the project
root), `ink.pdf.save` → `fs_write` (**default-denied**); both `load`/
`save` confine paths via the shared `helpers::resolve_fs_path` (moved out
of `fs.rs`).  The in-memory ops stay uncategorised (allowed; they persist
only via `save`).  Drift-guard test pins `save → fs_write`.

Verified: words register + error cleanly (unknown handle, stack
underflow), `save` denied-by-default at init, `load` path-confined.
Suite 1214 → 1215.  **VM round-trip — verified.** A `bund` script on a
real typst-built PDF (`ink.pdf.load` → `set_title` → `save`, with
`fs_write` enabled) wrote the output, and an independent
`inkhaven pdf metadata` read back the exact title — proving the full
load → mutate → save chain round-trips through the Adam VM.

**P0 (Foundations) is complete** — deps + fidelity gate, geometry/paper,
PdfDoc, ops, meta, outline, the `inkhaven pdf` CLI, and the `ink.pdf.*`
Bund layer.  Next: **P1 — imposition** (the marquee feature).

### P1.1 — imposition layout + creep math (landed)

The pure, unit-tested core of the marquee feature (no I/O), opening P1:

- **`impose` enums** — `BindingStyle` (saddle-stitch / perfect-bound /
  concertina / stab, with `parse` + `columns_per_side` + `is_folded`),
  `BlankPolicy` (prepend / append / balance / error), `CreepStrategy`
  (none / shingle / pushout).
- **`impose::layout`** — `plan(style, sheets_per_signature, source_pages,
  blank) -> Layout` of `Sheet { signature, sheet_in_sig, front, back }`
  with `Slot { page: Option<usize>, column }`.  Folded styles use the RFC
  §8.2.2 signature formula (`folded_positions`: front-left `4S-2i+2`,
  front-right `2i-1`, back-left `2i`, back-right `4S-2i+1`, offset by
  `g·4S`); stab/concertina are sequential 1-up (leaf k = pages 2k-1 / 2k).
  Padding blanks placed per `BlankPolicy`.
- **`impose::creep`** — `creep_shift_mm(strategy, i, thickness_mm)`: zero
  on the outermost sheet, growing one caliper per nested sheet toward the
  spine (shingling); `max_creep_mm` for the preview.  Direction applied at
  emission; shingle/pushout share the magnitude.
- 13 tests, incl. the **RFC §13 correctness property** (every source page
  appears exactly once) swept over all 4 styles × 10 page counts × 3
  blank policies, plus the classic 4-/8-page saddle layouts pinned by
  hand, multi-signature perfect-bound, and creep monotonicity.

Suite 1215 → 1228.  Next P1: `impose::sheet` (Form-XObject placement of
each slot, with creep offset + the target sheet geometry), then `marks`,
then the `imposition:` config + `inkhaven pdf impose` + `Ctrl+B I`
preview + `ink.pdf.impose`.

### P1.2 — sheet emission (Form XObjects) (landed)

`impose::sheet::emit(src, layout, params)` + the `impose::impose(src,
ImpositionParams)` coordinator turn a linear PDF into print-ready sheets:

- Each **source page → a Form XObject** (`lopdf::xobject::form`): BBox =
  the page MediaBox, content = `get_page_content` (decoded), `/Resources`
  = the page's effective resources (inline dict cloned, or the nearest
  reference up the page→parent chain — so fonts/images/vector carry).
- The imposed pages (front + back per sheet, **duplex order**) are sized
  to `params.sheet_size`; each `Slot` places its XObject with a
  `q 1 0 0 1 tx ty cm /Pg<n> Do Q` op.  `place()` centres the 2-up block
  (1-up centres one page) and applies the **creep** offset toward the
  spine (left page +creep, right page −creep).
- A fresh page tree + catalog roots at the imposed pages; `prune_objects`
  drops the orphaned source pages (the XObjects embed the content + hold
  the resources alive).
- `ImpositionParams { style, sheets_per_signature, blank, sheet_size,
  creep, paper_thickness_mm }`.

3 unit tests (8-pp saddle → 4 sides, 5-pp stab → 6 sides, creep
placement) + a **real-typst corpus gate** (`imposition_preserves_typst_
content`, ignored): imposing a 4-page typst doc keeps the Form XObjects,
the **embedded font, and the image** through the conversion.

Imposition produces valid, content-preserving PDFs on real output.  Suite
1228 → 1231 (+ 4 ignored corpus gates).  Next P1: `impose::marks` (crop /
fold / registration / spine collation-bars / signature numbers), then the
`imposition:` config + `inkhaven pdf impose` + `Ctrl+B I` preview +
`ink.pdf.impose`.

### P1.3 — printer marks (landed)

`impose::marks` — pure PDF content-stream ops (no SVG), appended to each
imposed sheet side by `sheet::emit`:

- **crop** — four L-marks at the trim-box corners, offset out by
  `crop_offset_mm`, 5 mm long.
- **fold** — dashed segments across the spine fold at block top + bottom
  (`fold_mark_length_mm`); folded styles only.
- **registration** — a printer's cross (crosshair + a 4-Bézier circle)
  centred above + below the block.
- **spine_marker** — the collation bar across the spine fold, its y
  **descending monotonically with the signature** (`spine_bar_y`) so the
  gathered spine shows a staircase; folded styles only.
- **signature_number** — the 1-based numeral near the fold, via a base-14
  `/F1` Helvetica resource the emitter adds when the mark is on.
- **color_bar** — a grayscale calibration strip (off by default).

`MarkConfig` (default: all but color_bar) + `MarkGeometry`; the emitter
computes the layout-wide block geometry and the per-sheet signature.
`ImpositionParams` gains `marks` / `crop_offset_mm` / `fold_mark_length_mm`.

6 mark unit tests (crop = 8 segments, fold dashed + folded-only,
registration crosshair+circle, **staircase monotonicity**, signature
text, default config) — all pure.  The real-typst imposition corpus gate
still passes with marks drawn.  Suite 1231 → 1237.

Next P1: the **surfaces** — the `imposition:` HJSON config (named
profiles via the cascade), `inkhaven pdf impose`, the `Ctrl+B I` preview
overlay, and `ink.pdf.impose`.

### P1.4 — imposition config + `inkhaven pdf impose` (landed)

- **`impose::config`** — the HJSON `imposition:` block: `ImpositionConfig
  { profiles }` (default `default` = perfect-bound/A3/shingle + `chapbook`
  = saddle/A4/no-creep), `ImpositionProfile` (style / sheets_per_signature
  / target_sheet_size [preset name *or* `{width_mm,height_mm}`, untagged]
  / orientation / margins / creep [enabled + stock + override + strategy]
  / marks / blank_page_policy).  `resolve(profile) -> ImpositionParams`
  parses the enums, resolves the sheet size + orientation (auto →
  landscape for 2-up), and the paper caliper (override → stock → 0.1).
- **Config wiring** — `Config.imposition` (`#[serde(default)]`), so
  profiles merge through `Config::load_layered` (project + global) like
  every other setting — the HJSON-configurable requirement, honoured.
- **`inkhaven pdf impose <input> [--config <profile>] [--out <file>]`** —
  loads the cascade config, resolves the profile, imposes, writes
  atomically; reports signatures / sheets / pages.  Unknown profile →
  clean error listing the available ones.

5 config tests (default + chapbook resolve, unknown-profile error, JSON
deserialize with custom sheet + thickness override, bad-style error).
**Live-verified** on a real 56-page A4 book: `--config default` →
4 signatures, 16 sheets → 32 A3-landscape imposed pages (correct
signature math), no crash artifacts.  Suite 1237 → 1242.

Imposition is now usable end-to-end from the CLI.  Next P1: the `Ctrl+B I`
preview overlay + `ink.pdf.impose` Bund word (and the `imposed_pdf`
book-take format).

### P1.5 — `ink.pdf.impose` Bund word (landed)

`ink.pdf.impose` ( handle profile -- handle' ) over the existing handle
registry: resolves the named profile from `active_config().imposition`
(project + global cascade) — falling back to the built-in `default` /
`chapbook` profiles when no project is registered — then runs the proven
`impose::impose`, returning a fresh handle.  In-memory (no disk I/O →
uncategorised, allowed); `load`/`save` still gate the disk crossing.

Verified: registered + clean errors (unknown profile resolved against the
fallback config, unknown handle, stack underflow), and a **full VM
round-trip on a real 56-page book** — `load → impose "default" → save`
produced 32 A3-landscape imposed pages (matching the CLI).  Suite 1242.

The RFC §9 example release script now works: `… ink.pdf.load …
imposition-profile ink.pdf.impose "…-imposed.pdf" ink.pdf.save`.  P1 is
functionally complete bar the `Ctrl+B I` TUI preview overlay + the
`imposed_pdf` book-take format.

### P1.6a — imposition preview generator + `pdf impose --dry-run` (landed)

`impose::preview` — pure: `build(profile, source_pages, params) ->
ImpositionPreview` runs the layout + creep math and `lines()` renders the
RFC App. D content: profile / source pages / style / signature size /
signatures (imposed pages + blanks) / sheet size (pt + mm) / paper +
creep (max shift) / marks, plus the first sheet's schematic
(`Front: [   8 ][   1 ]` / `Back:  [   2 ][   7 ]`, blanks as `[  —  ]`).
3 tests (saddle summary + schematic, padding blanks, stab 1-up).

`inkhaven pdf impose --dry-run` prints it without imposing — the same
generator the `Ctrl+B I` overlay will render.  Suite 1242 → 1245.

**TUI chord note:** `Ctrl+B I` is already `OpenBookInfo` and `Ctrl+B
Shift+I` is taken (`ViewEditEventMetadata`), so the overlay needs a
different chord or to fold into book-info — a product decision parked for
the user.  The preview *content* + generator are done and reusable.

### P1.6b — `Ctrl+B Q` imposition-preview overlay (landed)

`Ctrl+B I` was taken (book-info), so the overlay gets a dedicated chord
**`Ctrl+B Q`** — *Q for quire* (a gathering of folded sheets = exactly
what imposition makes).  `Action::OpenImpositionPreview` (label/help/entry
in keybind.rs, `run_action` arm).

`open_imposition_preview` resolves the current book's built PDF
(`resolve_artefacts_dir`/`<slug>.pdf` — the open_book_info path), loads it
for the page count, resolves the `default` profile from `self.cfg.
imposition`, builds the preview, and opens `Modal::ImpositionPreview {
source, out, profile, params, lines }`.  The modal renders the App. D
preview text (`draw_imposition_preview_modal`); **Enter** imposes (load →
`impose::impose` → atomic save to `<slug>-imposed.pdf`, status report),
**Esc** cancels.  Missing PDF → "build the book first (Ctrl+B B)".

Suite 1245 (compiles + keybind registry green; the pure preview content is
unit-tested).  Interactive overlay — the on-screen click-through isn't
headless-verifiable (same caveat as the find/replace review modal); the
engine + generator + open/apply paths are proven.

**P1 (imposition) is functionally complete**: layout · creep · sheet
emission · marks · config · `inkhaven pdf impose` (+`--dry-run`) ·
`ink.pdf.impose` · `Ctrl+B Q` preview.  Remaining P1 nicety: the
`imposed_pdf` book-take format (a `Ctrl+B O` extra-format entry).
