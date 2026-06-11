# RFC PDF-1 — PDF Management and Imposition Pipeline

| | |
|---|---|
| **RFC** | PDF-1 |
| **Title** | PDF Management and Imposition Pipeline |
| **Status** | Draft |
| **Created** | 2026-06-07 |
| **Author** | Vladimir Ulogov |
| **Target version** | 1.3.0 |
| **Depends on** | none |
| **Supersedes** | none |

---

## 1. Summary

Add a self-contained PDF management subsystem to Inkhaven covering book imposition (signatures, creep compensation, fold/crop/spine marks), page operations (extract, split, merge, rotate, reorder), cover-and-spine generation with ISBN barcode, preflight checks, and outline injection from the tree. The subsystem operates primarily on PDFs Inkhaven itself produced (via `Ctrl+B O` or `inkhaven export pdf`), uses only pure-Rust crates, preserves the single-binary deployment model, and exposes its full surface through CLI subcommands, the TUI book-take pipeline, and an `ink.pdf.*` Bund stdlib.

The marquee feature is **signature imposition for hand-binding and small print-shop workflows**, with creep compensation and collation marks — capabilities that no terminal-native writing tool currently provides and that fit Inkhaven's audience precisely.

## 2. Motivation

Inkhaven today produces a single linear PDF: pages 1 through N in reading order, suitable for a screen reader, a POD printer, or a duplex office printer. That covers the digital-distribution and POD-publishing paths but stops short of two real workflows the Inkhaven audience cares about:

- **Hand bookbinding.** A writer who finishes a manuscript in Inkhaven and wants to print and bind it themselves — chapbooks, art editions, gift copies of finished novels — needs the linear PDF rearranged into signatures and printed duplex on large sheets, with marks to guide cutting, folding, and gathering.
- **Small print-shop output.** A self-publisher taking a manuscript to a local print shop (rather than KDP / IngramSpark / Lulu) is expected to deliver an imposed, marked, preflight-clean PDF. Shops will impose for a fee, but Inkhaven users are the demographic who would rather do it themselves.

POD platforms do their own imposition and have their own spec sheets; this RFC explicitly does **not** target POD-platform compliance as primary scope. That work (PDF/X-4, CMYK, KDP cover templates) is a possible follow-up RFC.

In addition to imposition, several PDF helpers have repeatedly come up in writer workflows — page extraction for samples, watermarks for review copies, ISBN barcodes, grayscale conversion, outline generation, metadata stamping — all of which compose cleanly with imposition and share the same underlying machinery.

## 3. Goals

1. **Imposition** of Inkhaven-authored PDFs into print-ready signatures for the four binding styles that cover the artisan / print-shop case: saddle-stitch, perfect-bound (multi-signature), concertina, and Japanese stab.
2. **Creep compensation** with paper-stock presets, two shift strategies, and explicit override.
3. **Printer marks**: crop, fold, registration, spine markers (collation bars), signature numbering, optional color bars.
4. **Page operations** at PDF level: extract, split, merge, rotate, reorder, delete-range.
5. **Cover-and-spine generation** with a spine-width calculator from project metadata + paper stock + page count.
6. **ISBN barcode** rendering (EAN-13 + optional 5-digit price add-on) embedded in cover or interior.
7. **Preflight checks** appropriate to Inkhaven-authored output: font embedding, image resolution at print size, page-size consistency, color usage, blank-page detection.
8. **Outline / bookmark injection** from the Book/Chapter/Subchapter tree.
9. **PDF metadata stamping** synchronized to project HJSON (title, author, subject, keywords).
10. **Single binary**: every dependency is pure Rust; no external tool invocation; no native libraries beyond what's already vendored (DuckDB, etc.).
11. **Three surfaces**: CLI subcommand tree (`inkhaven pdf …`), TUI integration into the book-take format list, and `ink.pdf.*` Bund stdlib.

## 4. Non-goals

The following are explicitly **out of scope** for PDF-1:

- POD-platform-specific compliance (KDP, IngramSpark, Lulu). Future RFC if needed.
- PDF/X-1a / PDF/X-4 / PDF/A compliance modes.
- CMYK or ICC-profile color conversion.
- Rendering arbitrary external PDFs to images. Inkhaven-authored output is rendered via the existing `typst-render` path from source; arbitrary-PDF rasterization is deferred.
- OCR of scanned PDFs.
- Form filling, digital signatures, encryption with public-key infrastructure.
- Dust-jacket layout for hardcovers.
- Direct printer-driver / CUPS integration.
- A graphical placement editor; imposition is config-driven only.

## 5. Constraints

- **Single binary.** No new external tool dependencies. `pdftk`, Ghostscript, `qpdf`, `pdfium`, `mutool`, etc. are not options.
- **Pure Rust.** Every new dependency must be pure Rust or already vendored with C sources in the existing dep graph. No new native shared-library dependencies.
- **Primary input: Inkhaven-authored PDFs.** Features that depend on a parsed Typst source tree (outline injection, sample-by-chapter, preflight that knows expected page geometry) require Inkhaven authorship; features that operate purely on PDF object structure (imposition, extract, merge, rotate, metadata, watermark, barcode) work on any input PDF.
- **No source compromise.** PDF manipulation never modifies the Typst source. Imposition is always re-runnable from the same source by re-rendering.

## 6. Audience

Primary: artisan / hand binder, small print-shop self-publisher. These users are CLI-comfortable, value reproducibility, prefer terminal-native tools, and currently fall back on a patchwork of `pdfjam`, `bookbinder`, Affinity Publisher, or Adobe InDesign. None of those integrate with their writing tool.

Secondary: any Inkhaven user who occasionally needs to manipulate the output PDF — extract a sample, add a watermark, generate a cover, stamp metadata — without leaving the editor.

Out of scope as a primary audience: POD-platform users whose printer does its own imposition and supplies its own templates.

## 7. Design overview

### 7.1 Pipeline

The complete PDF subsystem is a pipeline that composes named stages, each taking and returning a `PdfDoc` value:

```
            ┌─────────────────────────────────────────────────┐
            │            Inkhaven PDF pipeline                │
            │                                                 │
  source ──▶│  ingest → ops → marks → impose → meta → emit    │──▶ output
            │     │      │      │       │       │             │
            │  load    extract  crop  signature  outline      │
            │  parse   split    fold  creep      author       │
            │  index   merge    spine n-up       title        │
            │          rotate   reg   blank-pad  keywords     │
            │          stamp                                  │
            │          watermark                              │
            └─────────────────────────────────────────────────┘
```

Each stage is independently invocable. A simple watermarking command runs only `ingest → ops → emit`. A full hand-binding print run runs the entire pipeline.

### 7.2 Module layout

```
src/pdf/
    mod.rs              -- public API, error types
    doc.rs              -- PdfDoc wrapper around lopdf::Document
    geometry.rs         -- mm/pt/in conversions, page-size presets
    paper.rs            -- paper-stock presets, thickness lookup
    ingest.rs           -- load and index input PDF
    ops.rs              -- extract, split, merge, rotate, delete, reorder
    stamp.rs            -- watermark and content stamps
    impose/
        mod.rs          -- coordinator
        layout.rs       -- signature math, page→position mapping
        creep.rs        -- creep compensation
        marks.rs        -- crop, fold, registration, spine, color
        sheet.rs        -- per-sheet rendering
    cover.rs            -- cover-and-spine PDF generation
    barcode.rs          -- ISBN EAN-13 generation
    preflight.rs        -- analysis and warnings
    outline.rs          -- inject bookmarks from tree
    meta.rs             -- PDF metadata read/write
    emit.rs             -- serialize PdfDoc to bytes
```

### 7.3 Surfaces

Three concurrent entry points, all routed through the same `pdf::*` library:

**CLI**: `inkhaven pdf <subcommand>` — full operation tree, scriptable.
**TUI**: book-take format list (`take.formats: [pdf, imposed_pdf, cover_pdf, …]`); `Ctrl+B O` produces all configured artifacts.
**Bund**: `ink.pdf.*` stdlib functions, sandbox-gated under existing `fs_write` capability.

## 8. Detailed design

### 8.1 `PdfDoc` — the value type

```rust
pub struct PdfDoc {
    inner: lopdf::Document,
    page_ids: Vec<ObjectId>,
    page_sizes: Vec<Rect>,     // cached; PDFs can vary per page
    source: PdfSource,         // Inkhaven | External
    project_ref: Option<ProjectRef>,
}

pub enum PdfSource {
    Inkhaven {
        source_typst_root: PathBuf,
        tree_snapshot: TreeSnapshotId,  // for outline injection
    },
    External,
}
```

`PdfDoc` is the lingua franca: every operation takes `&mut PdfDoc` or `&PdfDoc` and returns `Result<PdfDoc>`. The `source` field gates features that require knowledge of the originating Typst tree (outline injection, sample-by-chapter).

### 8.2 Imposition

#### 8.2.1 Configuration

```hjson
imposition: {
    style: "perfect_bound"          // saddle_stitch | perfect_bound | concertina | stab
    sheets_per_signature: 4         // ignored for saddle_stitch | concertina | stab
    target_sheet_size: "A3"         // any preset or { width_mm, height_mm }
    pages_per_sheet_side: 2         // 2 for octavo-style; 4 only for special cases
    orientation: "auto"             // auto | portrait | landscape

    margins: {
        bleed_mm: 3
        crop_offset_mm: 5
        fold_mark_length_mm: 8
        gutter_mm: 0                // additional, on top of source PDF gutter
        outer_margin_mm: 0
    }

    creep: {
        enabled: true
        paper_stock: "uncoated_80gsm"   // preset; resolves to thickness_mm
        thickness_mm_override: null     // null = use preset
        strategy: "shingle"             // shingle | pushout | none
    }

    marks: {
        crop: true
        fold: true
        registration: true
        spine_marker: true              // collation bars
        signature_number: true          // small numeral on spine fold
        color_bar: false                // for color jobs only
    }

    blank_page_policy: "append"         // prepend | append | balance | error

    output: {
        filename_template: "<book>-imposed-<YYYYDDMM>-<HHMM>.pdf"
    }
}
```

This block is mergeable into `inkhaven.hjson` under a top-level `imposition:` key, with the same merge semantics as existing config.

#### 8.2.2 Algorithm

The core imposition primitive is a function from `(signature_index, sheet_index, position)` to `source_page_number`. For a perfect-bound book with `S` sheets per signature (so 4S pages per signature):

For signature `g` (0-indexed) and sheet `i` (1-indexed, 1 = outermost), pages are positioned as:

```
sheet front side (side A):
    left  position: g·4S + (4S - 2i + 2)
    right position: g·4S + (2i - 1)
sheet back side (side B):
    left  position: g·4S + (2i)
    right position: g·4S + (4S - 2i + 1)
```

For saddle-stitch: a single signature with `S = total_pages / 4` sheets. For concertina: pages emitted in source order on alternating sides of a long strip (one "sheet" of `N` panels). For stab binding: each leaf is its own sheet; source page `n` goes to leaf `⌈n/2⌉`, side `A` for odd `n`, side `B` for even `n`.

The implementation:

```rust
pub fn impose(src: &PdfDoc, cfg: &ImpositionConfig) -> Result<PdfDoc> {
    let padded = pad_to_signature_multiple(src, cfg)?;
    let layout = compute_layout(&padded, cfg);            // SheetPlan list
    let mut dst = PdfDoc::new(target_sheet_size(cfg));
    for sheet in layout.sheets() {
        emit_sheet(&mut dst, &padded, &sheet, cfg)?;
    }
    Ok(dst)
}
```

`emit_sheet` is where lopdf's Form XObject capability does the heavy lifting. Each source page becomes a Form XObject (a reusable, transformable PDF graphics block); the destination sheet's content stream places those XObjects at the computed coordinates with the appropriate `cm` (concatenate-matrix) operator for translation, rotation, and creep offset. This is the standard imposition technique and is robust to source-PDF complexity (fonts, images, vector content all carried correctly).

#### 8.2.3 Creep compensation

Two strategies, both controlled by paper thickness `t` and signature half-position `δ = (S - i)` where `i` is sheet index from outermost.

- **Shingle**: x-shift per source page = `δ × t × 2`, applied inward (toward spine), measured along the page width direction.
- **Push-out**: same magnitude, but applied to *content* via a clip-and-shift transform on each XObject placement; trim stays aligned to the outermost sheet's edge.

Default strategy is **shingle**. It's geometrically simpler, easier to verify visually with the crop marks, and what most print shops expect.

#### 8.2.4 Paper stock presets

Initial preset table (extensible via HJSON):

| Preset name | Description | Thickness (mm) |
|---|---|---|
| `bible_70gsm` | Thin bible / dictionary paper | 0.060 |
| `uncoated_70gsm` | Lightweight uncoated | 0.085 |
| `uncoated_80gsm` | Standard novel interior | 0.100 |
| `uncoated_90gsm` | Heavier uncoated | 0.115 |
| `uncoated_100gsm` | Premium uncoated | 0.130 |
| `uncoated_120gsm` | Heavy uncoated | 0.160 |
| `coated_matte_80gsm` | Coated matte | 0.080 |
| `coated_matte_100gsm` | Coated matte premium | 0.105 |
| `coated_gloss_100gsm` | Coated gloss | 0.100 |
| `cover_250gsm` | Cover stock | 0.300 |
| `cover_300gsm` | Heavy cover stock | 0.360 |

Default: `uncoated_80gsm` for interior. Override via `thickness_mm_override` for exotic stocks.

#### 8.2.5 Marks

Each mark is a vector primitive drawn as a small content stream appended to the sheet's existing content:

- **Crop marks**: four L-shapes at the corners of each source-page region, offset outside the trim by `crop_offset_mm`, length `5mm`, stroke `0.25pt` black.
- **Fold marks**: a short dashed segment crossing the spine fold at the top and bottom edges of the sheet, length `fold_mark_length_mm`.
- **Registration marks**: a small crosshair-and-circle figure (the conventional printer's-cross) centered above and below the imposed area, used by the printer for plate alignment.
- **Spine markers (collation bars)**: a thick black bar (3 × 6 mm typical) crossing the spine fold, positioned at a y-offset that increases monotonically with signature number. When signatures are gathered and viewed from the spine edge, the bars form a descending staircase. Misaligned bar = misordered signature.
- **Signature number**: a small numeral (8 pt) printed on the spine fold of each signature, in addition to the spine bar.
- **Color bar** (off by default): standard color reference strip at the sheet edge for color-printing calibration.

All marks are rendered using lopdf primitives directly; no SVG conversion required.

### 8.3 Page operations

Pure-Rust operations on `PdfDoc`, all implemented via lopdf's page-tree manipulation:

```rust
pub fn extract(src: &PdfDoc, pages: &PageSpec) -> Result<PdfDoc>;
pub fn split(src: &PdfDoc, mode: SplitMode) -> Result<Vec<PdfDoc>>;
pub fn merge(docs: &[PdfDoc]) -> Result<PdfDoc>;
pub fn rotate(doc: &mut PdfDoc, pages: &PageSpec, degrees: Rotation) -> Result<()>;
pub fn delete(doc: &mut PdfDoc, pages: &PageSpec) -> Result<()>;
pub fn reorder(doc: &mut PdfDoc, mapping: &[usize]) -> Result<()>;

pub enum PageSpec {
    All,
    Single(usize),
    Range(usize, usize),         // inclusive
    List(Vec<PageSpec>),
    ByChapter(ChapterRef),       // Inkhaven-source only
}

pub enum SplitMode {
    EveryNPages(usize),
    ByBookmark,                  // requires existing outline
    ByChapter,                   // Inkhaven-source only; uses tree
    OnPages(Vec<usize>),
}
```

`PageSpec::ByChapter` and `SplitMode::ByChapter` use the tree-snapshot reference in `PdfDoc::source` to look up which page ranges correspond to which chapters. For external PDFs these variants return `Err(NotInkhavenSource)`.

### 8.4 Cover and spine generation

```rust
pub struct CoverSpec {
    pub front_width_mm: f32,
    pub front_height_mm: f32,
    pub spine_width_mm: f32,     // computed by spine_calc
    pub bleed_mm: f32,
    pub front_image: Option<PathBuf>,
    pub spine_text: SpineText,   // title + author at correct rotation
    pub back_text: Option<String>,
    pub barcode: Option<BarcodeSpec>,
}

pub fn spine_width_mm(
    page_count: usize,
    interior_paper: PaperStock,
    cover_paper: PaperStock,
) -> f32 {
    page_count as f32 * interior_paper.thickness_mm * 0.5
        + cover_paper.thickness_mm * 2.0
        + cover_paper.binding_compensation_mm()
}

pub fn build_cover(spec: &CoverSpec) -> Result<PdfDoc>;
```

The cover PDF is a single-page document with three logical regions (back / spine / front), trim marks, and optional barcode placement. Generated as native PDF via lopdf — no Typst pass required, since cover layout is purely geometric.

### 8.5 ISBN barcode

```rust
pub struct BarcodeSpec {
    pub isbn: String,            // 13 digits, validated checksum
    pub price_addon: Option<String>,  // 5-digit price code, optional
    pub position: BarcodePosition,
    pub height_mm: f32,
    pub include_human_readable: bool,
}

pub fn render_barcode(spec: &BarcodeSpec) -> Result<Vec<PdfPathOp>>;
```

EAN-13 generation in pure Rust (~150 lines including check-digit validation; no crate strictly required, but `barcoders` is acceptable if review confirms it's pure-Rust and dependency-light). Output is a list of PDF path operations that get appended to the cover's content stream — no PNG round-trip, scales perfectly to any DPI.

### 8.6 Preflight

For Inkhaven-authored PDFs we have ground truth (the Typst source); preflight becomes verification rather than guessing.

```rust
pub struct PreflightReport {
    pub page_count: usize,
    pub page_size_consistency: Consistency,
    pub fonts: Vec<FontReport>,           // (name, embedded, subset)
    pub images: Vec<ImageReport>,         // (page, effective_dpi, format)
    pub color_pages: Vec<usize>,          // pages with non-grayscale ink
    pub blank_pages: Vec<usize>,
    pub warnings: Vec<Warning>,
}

pub fn preflight(doc: &PdfDoc, profile: PreflightProfile) -> PreflightReport;

pub enum PreflightProfile {
    HandBinding { target_dpi: u32 },     // 300 default
    PrintShop { target_dpi: u32, paper_stock: PaperStock },
    Strict,
}
```

The image-DPI check is the highest-value preflight: it catches the single most common "my printed book looks bad" failure (a 72-dpi screenshot pasted into a manuscript at full page size). For each image, compute `effective_dpi = pixel_dimension / placed_size_inches`, flag if below `target_dpi`.

### 8.7 Outline injection

```rust
pub fn inject_outline(doc: &mut PdfDoc, tree: &Tree) -> Result<()>;
```

For Inkhaven-authored output: walk the tree, find the page number of each chapter and subchapter (looked up from the Typst-side bookmark annotations the compiler emits when given the right `#outline` invocation), build a hierarchical `/Outlines` dictionary, attach to the document catalog. The Typst-side change needed is small: the existing assemble step (`Ctrl+B A`) emits `#set heading(...)` directives; we additionally emit `#metadata((node_id: "..."))` near each heading so the outline injector can correlate. This change to the assemble step is part of P0.

### 8.8 Metadata

```rust
pub struct PdfMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Vec<String>,
    pub creator: String,           // always "Inkhaven <version>"
    pub producer: String,          // typst version + inkhaven version
}

pub fn read_metadata(doc: &PdfDoc) -> PdfMetadata;
pub fn write_metadata(doc: &mut PdfDoc, m: &PdfMetadata);
pub fn strip_metadata(doc: &mut PdfDoc);  // for privacy
```

On book-take, metadata is auto-populated from project HJSON (`book.title`, `book.author`, `book.keywords`) unless explicitly disabled.

## 9. Bund stdlib

All operations exposed as Bund words under `ink.pdf.*`, sandbox-gated:

| Word | Sandbox category | Description |
|---|---|---|
| `ink.pdf.load` | `fs_read` | Load PDF into a Bund value |
| `ink.pdf.save` | `fs_write` | Write PDF to disk |
| `ink.pdf.pages` | none | Page count |
| `ink.pdf.extract` | none | Extract page range, returns new doc |
| `ink.pdf.split` | none | Split into multiple docs |
| `ink.pdf.merge` | none | Concatenate docs |
| `ink.pdf.rotate` | none | Rotate pages |
| `ink.pdf.delete` | none | Delete pages |
| `ink.pdf.reorder` | none | Permute pages |
| `ink.pdf.stamp` | none | Add watermark/stamp |
| `ink.pdf.impose` | none | Run imposition pipeline |
| `ink.pdf.cover` | none | Build cover PDF |
| `ink.pdf.barcode` | none | Render ISBN barcode |
| `ink.pdf.preflight` | none | Run preflight |
| `ink.pdf.outline` | none | Inject outline from tree |
| `ink.pdf.metadata.get` | none | Read metadata |
| `ink.pdf.metadata.set` | none | Write metadata |
| `ink.pdf.metadata.strip` | none | Clear metadata |

A typical Bund release script becomes:

```
: release ( -- )
    "./build/book.pdf" ink.pdf.load
    book.metadata ink.pdf.metadata.set
    ink.tree.current ink.pdf.outline
    dup imposition.config ink.pdf.impose
    "./build/book-imposed.pdf" ink.pdf.save
    "./build/book.pdf" ink.pdf.save
;
hook.on_book_take : release ;
```

## 10. Surfaces — CLI, TUI, Book-Take

### 10.1 CLI

```
inkhaven pdf impose      <input> [--config <key>] [--out <file>]
inkhaven pdf extract     <input> --pages <spec> [--out <file>]
inkhaven pdf split       <input> --mode <mode> [--out-dir <dir>]
inkhaven pdf merge       <inputs>... --out <file>
inkhaven pdf rotate      <input> --pages <spec> --degrees <90|180|270>
inkhaven pdf reorder     <input> --mapping <a,b,c,…>
inkhaven pdf cover       --pages <n> [--barcode <isbn>] --out <file>
inkhaven pdf barcode     <isbn> [--addon <price>] --out <file>
inkhaven pdf preflight   <input> [--profile <profile>]
inkhaven pdf outline     <input> --out <file>   # Inkhaven-source only
inkhaven pdf metadata    <input> [get|set|strip] [--key=val …]
inkhaven pdf sample      [--first <n>] [--chapters <range>] [--watermark <text>] --out <file>
inkhaven pdf grayscale   <input> --out <file>
inkhaven pdf optimize    <input> --target <web|archive|print>
```

All commands take `--project <path>` for project-aware operations and default to operating on the most recent book-take PDF if no input is specified.

### 10.2 TUI

The book-take format list (`take.formats`) gains new entries:

```hjson
book: {
    take: {
        formats: [pdf, imposed_pdf, cover_pdf]
        imposed_pdf_config: "default"        // refers to imposition: key
        cover_pdf_isbn: "978-...."
    }
}
```

`Ctrl+B O` now produces all configured artifacts in one pass, with a per-format status line in the take overlay. Failures of optional formats (e.g. cover generation when ISBN is missing) do not block the primary PDF.

A new chord `Ctrl+B I` opens an **Imposition preview** overlay: a ratatui pane showing the imposition plan (sheet count, signature breakdown, blank-page padding, creep amount) and a small ASCII-art schematic of the first sheet's layout. Enter triggers the impose; Esc cancels. Useful sanity check before printing.

### 10.3 Book-take integration

The book-take pipeline already covered in the earlier export discussion gains PDF-specific orchestration. The `imposed_pdf` format depends on `pdf` (you can't impose what hasn't been compiled), and the engine enforces this ordering automatically.

## 11. Dependency selection

All new crates are pure Rust:

| Crate | Purpose | License | Pure Rust |
|---|---|---|---|
| `lopdf` | Core PDF parsing/writing | MIT | Yes |
| `printpdf` | Optional, for cover gen | MIT | Yes |
| `barcoders` | EAN-13 generation | MIT/Apache | Yes |

Already in `Cargo.toml` and reused: `typst-render`, `resvg`, `image`, `zip`.

Not used: pdfium, mupdf, ghostscript bindings, qpdf, pdftk wrappers. None of these meet the single-binary constraint.

A judgment call exists between `lopdf` and `printpdf`: `lopdf` reads existing PDFs and is what we need for imposition / page operations; `printpdf` is generative-only and could be used for cover layout. We can do everything with `lopdf` alone (cover layout is a 1-page document, easy to build directly). Decision: **use `lopdf` exclusively**; reject `printpdf` to minimize dep surface.

## 12. Implementation phases

**P0 — Foundations (3 weeks).** `PdfDoc`, `geometry`, `paper`, `ingest`, `emit`, `meta`, `ops` (extract/split/merge/rotate/reorder/delete), `outline` injection (including Typst assemble-step changes for `#metadata` markers). CLI subcommands for these. Bund stdlib for these. Tests with golden PDFs.

**P1 — Imposition (4 weeks).** `impose/` module complete: layout math, four binding styles, creep compensation, all marks, sheet emission via Form XObjects. CLI `pdf impose`. TUI `Ctrl+B I` preview. `imposed_pdf` book-take format. Bund stdlib. Property tests for permutation correctness.

**P2 — Cover, barcode, preflight (2 weeks).** `cover`, `barcode`, `preflight` modules. CLI subcommands. `cover_pdf` book-take format. Sample-generation convenience command.

**P3 — Polish (1 week).** Grayscale conversion, optimize-for-web pass, metadata strip, watermark/stamp variations, documentation, tutorial in `Documentation/Tutorials/HAND_BINDING.md`.

Total: ~10 weeks for a single developer. Each phase is shippable in isolation.

## 13. Testing strategy

- **Unit tests**: geometry conversions, paper-stock lookups, EAN-13 check digits, page-spec parsing.
- **Property tests** for imposition: every source page appears exactly once in output; pairs are on the same sheet; signature totals sum correctly; creep offsets are monotonic.
- **Golden-PDF tests**: a small corpus of fixture PDFs (4pp, 16pp, 64pp, 200pp) with expected output PDFs checked in. Comparison via lopdf's object-tree diff, not byte-equality (PDF serialization isn't deterministic across versions).
- **Roundtrip tests**: load → identity-op → save produces semantically identical PDF.
- **Integration test** with the existing book-take pipeline: a full project compiles, imposes, and verifies the imposed output is preflight-clean.

## 14. Risks and alternatives

**Risk: `lopdf` doesn't handle every PDF feature Inkhaven's Typst output uses.** Mitigation: P0 includes a corpus test against current Inkhaven-produced PDFs (with images, embedded fonts, vector content, outline annotations). If gaps exist, contribute upstream or carry a small patch set.

**Risk: imposition correctness is subtle and visually verifiable only.** Mitigation: property tests on the permutation; a small "test print" mode (`inkhaven pdf impose --test`) emits a numbered, marked PDF where every page is just "Page N" in 200pt — visually trivial to fold by hand and verify.

**Risk: creep compensation defaults are wrong for many users.** Mitigation: paper-stock presets are explicit, the default (`uncoated_80gsm`, `shingle`) is the most common case, and the imposition preview overlay surfaces the computed creep amount so users see what's about to happen.

**Risk: Typst source assumptions break outline injection on hand-written `.typ` files.** Mitigation: `#metadata` markers are emitted only by the assemble step; users editing raw `.typ` get the unannotated PDF and a `--no-outline` warning in book-take.

**Alternative considered: shell out to `pdfjam` / Ghostscript.** Rejected per the no-external-tools constraint and the single-binary promise.

**Alternative considered: render entire imposition via Typst itself** (Typst can compose pages from other Typst-compiled documents). Rejected because (a) it requires the user's original Typst source to be re-compilable, (b) it doesn't work for already-built PDFs that have been hand-edited, and (c) it doesn't generalize to the external-PDF cases where we still want page operations to work.

**Alternative considered: defer all PDF work to a follow-up release.** Rejected because hand-binding is a clearly missing feature in the Inkhaven workflow and the underlying machinery (`lopdf`) is mature enough to ship.

## 15. Open questions

1. **Bookmarks vs. tagged-PDF.** The outline injection generates `/Outlines` (the legacy bookmark format). Should P0 also generate tagged-PDF structure trees for accessibility? Probably not for P0; reconsider in PDF-2.
2. **Cover-image color space.** If the user supplies an RGB JPEG for the front cover, do we convert to CMYK on cover-PDF emission? Out of scope per the no-CMYK non-goal; document as RGB-only.
3. **Bund return-value semantics for PDFs.** Should `ink.pdf.load` return a handle (light value, ref-counted in the VM) or a full document value (heavy, copied on every word boundary)? Recommendation: handle, with explicit `ink.pdf.clone` for branching workflows.
4. **Imposition over external PDFs with non-uniform page sizes.** Define behavior: error out by default, with a `--rescale` flag that forces all pages to the modal size.
5. **Spine marker visibility under heavy ink coverage.** Position the bars in the trim margin, not the bleed, so they remain visible if the spine is trimmed flush.

## 16. Appendices

### A. Full HJSON config schema

```hjson
imposition: {
    profiles: {
        default: {
            style: "perfect_bound"
            sheets_per_signature: 4
            target_sheet_size: "A3"
            pages_per_sheet_side: 2
            orientation: "auto"
            margins: { bleed_mm: 3, crop_offset_mm: 5, fold_mark_length_mm: 8, gutter_mm: 0, outer_margin_mm: 0 }
            creep: { enabled: true, paper_stock: "uncoated_80gsm", thickness_mm_override: null, strategy: "shingle" }
            marks: { crop: true, fold: true, registration: true, spine_marker: true, signature_number: true, color_bar: false }
            blank_page_policy: "append"
        }
        chapbook: {
            style: "saddle_stitch"
            target_sheet_size: "A4"
            // creep disabled because saddle-stitch on ≤32pp doesn't need it
            creep: { enabled: false }
            marks: { crop: true, fold: true, registration: false, spine_marker: false, signature_number: false }
            blank_page_policy: "balance"
        }
    }
}

cover: {
    profiles: {
        default: {
            interior_paper: "uncoated_80gsm"
            cover_paper: "cover_250gsm"
            bleed_mm: 3
            spine_text: { include_title: true, include_author: true, font_size_pt: 10 }
            barcode: { position: "back_bottom_right", height_mm: 25, include_human_readable: true }
        }
    }
}

preflight: {
    target_dpi: 300
    profile: "hand_binding"
}
```

### B. Imposition formula derivation

For a folded sheet with `2k+1`-th and `2k+2`-th source pages (1-indexed `k`), nesting `S` sheets gives a 4S-page signature. Sheet `i` (from outermost) carries:

- Outside: pages `2i-1` and `4S - 2i + 2`
- Inside: pages `2i` and `4S - 2i + 1`

Left/right of each side determined by recto-verso convention (right = odd page number, except when otherwise constrained by gutter direction). Multi-signature offsets by `4S × (g-1)` where `g` is signature index.

### C. Module interaction diagram

```
                ┌──── pdf::ingest ─── pdf::doc ───┐
                │                                  │
                ▼                                  ▼
        pdf::ops ◄── pdf::geometry      pdf::preflight
        pdf::stamp                                 │
                │                                  │
                ▼                                  ▼
        pdf::impose ◄── pdf::paper            (report)
        ├ layout
        ├ creep
        ├ marks
        └ sheet
                │
                ▼
        pdf::outline ◄── tree (Inkhaven)
        pdf::meta ◄── project HJSON
                │
                ▼
        pdf::emit
                │
                ▼
            (PDF on disk)

        pdf::cover ── pdf::barcode (independent leg)
```

### D. Sample TUI imposition preview

```
┌─ Imposition preview (default profile) ────────────────────────────┐
│  Source           : ./build/my-novel.pdf                          │
│  Pages            : 248                                           │
│  Style            : perfect_bound                                 │
│  Signature size   : 16 pages (4 sheets)                           │
│  Signatures       : 16  (256 imposed pages, +8 blanks)            │
│  Sheet size       : A3 landscape (420×297 mm)                     │
│  Paper stock      : uncoated_80gsm (0.10 mm)                      │
│  Creep            : shingle, max shift 1.5 mm at innermost sheet  │
│  Marks            : crop · fold · registration · spine            │
│  Output           : ./my-novel-imposed-20260607-1421.pdf          │
│                                                                   │
│  Sheet 1 (signature 1, outermost):                                │
│    Front: [page 16 ][ page 1 ]                                    │
│    Back:  [page  2 ][ page 15]                                    │
│                                                                   │
│  ─────────────────────────────────────────────────────────────    │
│  Enter: impose      I: edit profile      Esc: cancel              │
└───────────────────────────────────────────────────────────────────┘
```

---

**End of RFC PDF-1.**
