# Tutorial 65 — Hand-binding: imposition, covers, preflight

*Inkhaven 1.3.0+*

You have finished the manuscript. `inkhaven export pdf` (or `Ctrl+B O`)
gives you a clean reading PDF — one page after another, in reading order.
That is exactly the **wrong** order for a printer.

To fold and sew (or perfect-bind) a book by hand, the pages have to be
**imposed**: rearranged so that when a stack of sheets is folded in half,
the page numbers come out consecutive. Page 1 sits next to the last page
on the same side of the same sheet. 1.3.0 adds the whole production
pipeline — imposition, covers with a computed spine, ISBN barcodes, and
a preflight check — over a pure-Rust PDF core (no external apps).

Everything here is a `inkhaven pdf …` subcommand, an `ink.pdf.*` Bund
word, or an `extra_formats` book-take. The reading PDF you already build
is the input; nothing about your manuscript changes.

## The five-minute version

```sh
# 1. build the reading PDF as usual
inkhaven export pdf --book my-novel            # → my-novel.pdf

# 2. check it is print-ready
inkhaven pdf preflight my-novel.pdf

# 3. impose into folding signatures
inkhaven pdf impose my-novel.pdf --config default   # → my-novel-imposed.pdf

# 4. generate a cover with a computed spine
inkhaven pdf cover --pages 220 --title "My Novel" --author "A. Writer" \
    --isbn 9780306406157 --out my-novel-cover.pdf
```

Print `my-novel-imposed.pdf` double-sided, fold each signature, sew or
glue, wrap the cover. That is a book.

## Imposition

A **signature** is a small stack of sheets folded together (4 sheets =
16 pages is typical). A book is a row of signatures. `pdf impose`
reorders your interior into imposed sheets and adds the **printer marks**
a binder needs:

```sh
inkhaven pdf impose my-novel.pdf --config default
inkhaven pdf impose my-novel.pdf --config chapbook        # saddle-stitch
inkhaven pdf impose my-novel.pdf --config default --dry-run   # plan only
```

`--dry-run` prints the plan — how many signatures and sheets, the creep,
and a schematic of the first sheet — without writing anything:

```
imposition `default` · perfect_bound · A3 landscape
220 pages → 14 signatures × 4 sheets (1 padding page appended)
creep: shingle, 0.10 mm/sheet, max 0.60 mm at the spine
sheet 1 front:  [ 16 |  1 ]   back:  [  2 | 15 ]
```

Five profiles ship built in:

- **`default`** — perfect-bound, 4 sheets/signature, A3, creep on.
- **`chapbook`** — saddle-stitch, A4, no creep (a single folded booklet).
- **`us_perfect`** — `default` on Tabloid (11×17) sheets, the US-paper
  analogue that folds to two US-Letter-half pages.
- **`us_chapbook`** — `chapbook` on Tabloid sheets.
- **`thick`** — heavy perfect-bound: 8-sheet (32-page) signatures with
  push-out creep so the outer leaves don't bind short after trimming.

They're just named entries in the `imposition:` config block, and you
can add your own — see [Configuration](#configuration) below.

### Creep (shingling)

When you fold a thick signature, the inner sheets stick out past the
outer ones; trimming makes the inner pages slightly narrower and pushes
their content toward the spine. **Creep** compensates by nudging each
sheet's content outward in proportion to how deep it sits in the fold.
The `default` profile shingles automatically using the paper caliper; the
`chapbook` profile (one thin booklet) turns it off.

### Previewing in the TUI

In the **tree** pane, select a book and press **`Ctrl+B Q`** (*Q* for
*quire*) to open the imposition preview overlay for it — the same plan
`--dry-run` prints, without leaving your manuscript. (It is tree-scoped
so it never shadows the editor's `Ctrl+B Q` = translate-to-invented.)
`Enter` imposes; `Esc` cancels.

### As a book-take

Add `imposed_pdf` to `output.extra_formats` and **`Ctrl+B O`** will
impose the just-built PDF automatically, writing `…-imposed.pdf` beside
it. The profile is `output.imposed_pdf_config` (default `default`).

## Covers and spine

A cover is one landscape page laid out `[ bleed | back | spine | front |
bleed ]`. The spine width is **computed** from the interior page count
and the paper you print on:

```sh
inkhaven pdf cover --pages 220 \
    --title "My Novel" --author "A. Writer" \
    --back "A novel about …" \
    --image front-art.png \
    --isbn 9780306406157 \
    --out my-novel-cover.pdf
```

Trim size, bleed, and the interior / cover paper stocks come from the
`cover:` config block; `--width-mm` / `--height-mm` / `--spine-mm`
override them per run. The spine width formula is

```
spine = pages × interior_caliper × 0.5  +  2 × cover_caliper  +  binding allowance
```

so a 220-page novel on 80 gsm uncoated stock with a 250 gsm cover comes
out around 12 mm. If your printer hands you an exact spine, pass
`--spine-mm` and skip the calculation.

The **front art** fills the front panel out to the bleed, aspect-preserved.
`--fit cover` (the default) scales to fill and centre-crops the overflow —
no distortion, no white gaps; `--fit fit` scales to fit inside (may leave
gaps); `--fit stretch` is the old distort-to-fill. The **spine text**
auto-fits: it never exceeds the configured `spine_font_size_pt` but shrinks
to run the title along the spine height and sit within the spine thickness,
and is dropped entirely when the spine is too thin to carry legible type.

The `--isbn` flag renders a real **EAN-13 barcode** (validated check
digit) on the back panel. You can also produce a standalone barcode:

```sh
inkhaven pdf barcode 9780306406157 --out barcode.pdf
```

As a book-take, `cover_pdf` in `output.extra_formats` generates
`…-cover.pdf` on `Ctrl+B O` — spine from the just-built page count, title
from the book.

## Preflight

Before you commit a ream of paper, **preflight** catches the problems
that only show up in print:

```sh
inkhaven pdf preflight my-novel.pdf --profile hand_binding
```

```
my-novel.pdf
  pages: 220
  page size: consistent
  target: 300 dpi
  font: LXGWWenKai [embedded]
  image: p.12 Im0 480×320px @ 96 dpi (DeviceRGB)
  1 warning(s):
    ⚠ page 12: image `Im0` at 96 dpi (below 300 target)
```

The highest-value check is **effective image DPI** — it follows each
image through the page's transformation matrix to find the size it is
*actually printed at*, so a 480-px screenshot blown up to fill the page
is flagged as 96 dpi even though the file says 480. Preflight also checks
font embedding, page-size consistency, blank pages, and colour usage.
Profiles set the DPI target: `hand_binding`, `print_shop`, `strict`
(or `--dpi N`).

It also flags the **press hazards** that silently change on output:
**overprint** (`/OP` graphics states — a knockout you meant to trap, or
vice-versa), **transparency** (constant alpha < 1, a non-Normal blend
mode, or a soft mask — flatten before a non-PDF/X workflow), and **spot
colours** (Separation / DeviceN colorants, each a separate plate). If you
only intend black ink, `inkhaven pdf grayscale` collapses content colour
and converts images — now including DCTDecode (JPEG) photos, re-embedded as
grayscale JPEGs — to DeviceGray.

## Finishing touches

```sh
inkhaven pdf grayscale my-novel.pdf      # mono — cheaper to print / proof
inkhaven pdf optimize  my-novel.pdf      # prune + compress, lossless
inkhaven pdf watermark my-novel.pdf --text DRAFT       # diagonal stamp
inkhaven pdf sample    my-novel.pdf --count 8          # quick proof subset
```

- **grayscale** desaturates without re-rendering — content-stream colour
  is neutralized and RGB/CMYK images become DeviceGray. (Best-effort:
  JPEG and exotic colour spaces are left as-is.)
- **optimize** is a lossless slim-down: orphan objects pruned, every
  uncompressed stream Flate-compressed. Run it after imposition or merge.
- **watermark** stamps translucent text and/or an image onto a page
  range (`--pages 2-4`), centred or in any corner (`--position`).
- **sample** pulls a handful of evenly-spaced pages (first + last always)
  for a fast proof print before you run the whole book.

## Scripting it (Bund)

The same operations are `ink.pdf.*` words, so a release script can build
the whole production set in one pass:

```forth
"my-novel.pdf" ink.pdf.load          \ ( -- h )
dup "hand_binding" ink.pdf.preflight  \ ( h -- h n )  warnings; 0 = ready
drop
dup "default" ink.pdf.impose          \ ( h -- h h2 )  imposed copy
"my-novel-imposed.pdf" ink.pdf.save
"9780306406157" ink.pdf.cover         \ ( h isbn -- h h3 )  cover from the book
"my-novel-cover.pdf" ink.pdf.save
```

`ink.pdf.load` / `ink.pdf.save` are the only words that touch disk, and
they are gated by the same `fs_read` / `fs_write` policy as `ink.fs.*`.

## Where to go next

- The full design rationale and formulas: the
  [PDF-1 proposal](../PROPOSALS/PDF-1.md).
- Every `imposition:` / `cover:` / `preflight:` knob:
  [`../CONFIGURATION.md`](../CONFIGURATION.md).
- The `Ctrl+B Q` chord and the rest of the meta layer:
  [`../KEYBINDING.md`](../KEYBINDING.md).
