// Book of Inkhaven — design tokens + page chrome.
//
// Book-author taste: roomy margins, serif body, sans
// headings, code blocks that look like code without
// dominating the page, figure captions that read as
// captions and not afterthoughts.

#let book_title  = "Книга Inkhaven"
#let book_subtitle = "Авторское руководство по литературному TUI"
#let book_authors = ("Владимир Улогов", "(и соавторы inkhaven)")
#let book_version = "1.2.6"
#let book_year    = "2026"

// ── Colour palette ───────────────────────────────────────
// Subtle. Not Catppuccin — print-oriented. Warm paper,
// cool ink, restrained accent for callouts.
#let ink_black    = rgb("#1a1a1a")
#let ink_gray     = rgb("#5d5d5d")
#let ink_faint    = rgb("#a3a3a3")
#let ink_paper    = rgb("#fdfaf3")           // very warm off-white
#let ink_rule     = rgb("#c6c0b5")
#let ink_accent   = rgb("#7a4a2f")           // burnt-sienna for chapter numbers + drop caps
#let ink_code_bg  = rgb("#f3eee4")
#let ink_call_bg  = rgb("#f6f1e6")
#let ink_call_rule = rgb("#7a4a2f")

// ── Type families ───────────────────────────────────────
// Body: serif (Linux Libertine ships in the in-process
// engine's `embed-fonts` set, so the book always
// compiles cleanly without host font setup).
// Headings: sans for visual contrast.
// Code: monospace.
#let body_family   = ("Linux Libertine", "STIX Two Text", "DejaVu Serif")
#let sans_family   = ("Linux Biolinum O", "DejaVu Sans")
#let mono_family   = ("DejaVu Sans Mono", "Menlo")

// ── Page template ───────────────────────────────────────
//
// Two-sided book layout: inner / outer margins so left/
// right pages mirror. Running header on every page except
// chapter openings; page numbers in the outer footer.

#let book_page = (
  paper: "iso-b5",                        // friendly book size
  margin: (inside: 28mm, outside: 22mm, top: 22mm, bottom: 24mm),
  numbering: "1",
)

// ── Helpers used by chapters ────────────────────────────

// Big drop-cap word for chapter openings.
#let dropcap(letter, color: ink_accent) = box(
  baseline: 28pt,
  text(
    font: body_family,
    weight: "bold",
    size: 60pt,
    fill: color,
    letter,
  ),
)

// Chapter opening. Use once at the top of each chapter:
//   #chapter(number: 3, part: "Part I — Foundations",
//     title: "The Project Tree")
//
// Emits a hidden `heading(level: 1, ...)` first so the
// outline() in the front matter + the PDF bookmarks pick
// up the chapter title. The visible chapter display is
// then hand-laid below (number + title + rule).
#let chapter(number: 0, part: "", title: "") = {
  pagebreak(weak: true, to: "odd")
  hide(heading(
    level: 1,
    numbering: none,
    outlined: true,
    bookmarked: true,
    [#str(number) — #title],
  ))
  v(2cm)
  align(left)[
    #if part != "" {
      text(
        font: sans_family,
        size: 9pt,
        tracking: 2pt,
        fill: ink_gray,
        upper(part),
      )
      v(2mm)
    }
    #text(
      font: sans_family,
      size: 110pt,
      weight: "bold",
      fill: ink_accent,
      str(number),
    )
    #v(-8mm)
    #text(
      font: body_family,
      size: 28pt,
      weight: "regular",
      fill: ink_black,
      title,
    )
  ]
  v(1.5cm)
  line(length: 100%, stroke: 0.5pt + ink_rule)
  v(1cm)
}

// Appendix opening — same shape as `chapter` but with a
// letter instead of a number.
#let appendix(letter: "A", title: "") = {
  pagebreak(weak: true, to: "odd")
  hide(heading(
    level: 1,
    numbering: none,
    outlined: true,
    bookmarked: true,
    [Appendix #letter — #title],
  ))
  v(2cm)
  align(left)[
    #text(
      font: sans_family,
      size: 9pt,
      tracking: 2pt,
      fill: ink_gray,
      upper("Appendix " + letter),
    )
    #v(2mm)
    #text(
      font: sans_family,
      size: 110pt,
      weight: "bold",
      fill: ink_accent,
      letter,
    )
    #v(-8mm)
    #text(
      font: body_family,
      size: 28pt,
      weight: "regular",
      fill: ink_black,
      title,
    )
  ]
  v(1.5cm)
  line(length: 100%, stroke: 0.5pt + ink_rule)
  v(1cm)
}

// Section heading inside a chapter. Emits a hidden
// `heading(level: 2)` for outline depth-2 entries; the
// visible heading is then drawn below in custom sans.
#let section(title) = {
  hide(heading(level: 2, numbering: none, outlined: true, title))
  v(8mm)
  text(font: sans_family, size: 14pt, weight: "bold", fill: ink_black, title)
  v(1mm)
}

// Sub-section.
#let subsection(title) = {
  v(4mm)
  text(font: sans_family, size: 11pt, weight: "bold", fill: ink_black, title)
  v(0.5mm)
}

// Pull-quote / callout box — used for "if you're new"
// nudges, important warnings, etc.
#let callout(label: "Note", body) = {
  v(3mm)
  block(
    fill: ink_call_bg,
    stroke: (left: 2pt + ink_call_rule),
    inset: (left: 8pt, right: 8pt, top: 8pt, bottom: 8pt),
    width: 100%,
    {
      text(
        font: sans_family,
        size: 8pt,
        weight: "bold",
        fill: ink_call_rule,
        tracking: 1pt,
        upper(label),
      )
      v(2mm)
      body
    },
  )
  v(3mm)
}

// Inline figure. Loads `images/<id>.png` (relative to this
// design.typ — i.e. `Book/images/<id>.png`) and renders it
// fit-to-page-width capped at `height`. Caption sits below.
//
// Build fails loudly with `file not found` when the PNG is
// missing — by design. The previous placeholder-rectangle
// fallback was silent and let figures ship with empty
// rectangles in the final PDF. See SCREENSHOTS.md for the
// id catalog + capture recipes.
#let figure_slot(id: "tree-pane-empty", caption: "", height: 60mm) = {
  v(3mm)
  align(center,
    image(
      "images/" + id + ".png",
      width: 100%,
      height: height,
      fit: "contain",
    )
  )
  if caption != "" {
    v(1mm)
    align(center,
      text(font: sans_family, size: 9pt, fill: ink_gray, style: "italic", caption)
    )
  }
  v(3mm)
}

// Chord-table — two columns: chord on the left, action on
// the right. Used heavily in the book.
#let chord_row(chord, action) = (
  text(font: mono_family, size: 10pt, chord),
  text(font: body_family, size: 11pt, action),
)
#let chord_table(rows) = {
  v(3mm)
  table(
    columns: (auto, 1fr),
    stroke: (x, y) => if y == 0 { (bottom: 0.5pt + ink_rule) } else { none },
    align: (left, left),
    inset: (x: 6pt, y: 4pt),
    table.header(
      text(font: sans_family, size: 9pt, weight: "bold", "Chord"),
      text(font: sans_family, size: 9pt, weight: "bold", "What it does"),
    ),
    ..rows.flatten(),
  )
  v(3mm)
}

// Chapter-end recap box.
#let recap(items) = {
  v(8mm)
  block(
    fill: ink_call_bg,
    stroke: (left: 2pt + ink_accent),
    inset: (left: 8pt, right: 8pt, top: 8pt, bottom: 8pt),
    width: 100%,
    {
      text(
        font: sans_family,
        size: 9pt,
        weight: "bold",
        fill: ink_accent,
        tracking: 1.5pt,
        "ИТОГИ ГЛАВЫ",
      )
      v(2mm)
      list(..items.map(i => i))
    },
  )
}

// Master document wrapper.
//
// `pages` is an array of `include`s (one per chapter file).
// Usage from BOOK_OF_INKHAVEN.typ:
//
//   #book((
//     include "chapters/00-prologue.typ",
//     include "chapters/01-what-inkhaven-is.typ",
//     ...
//   ))
#let book(pages) = {
  set document(
    title: book_title,
    author: book_authors,
  )
  set page(
    paper: book_page.paper,
    margin: book_page.margin,
    numbering: "1",
    number-align: center,
    header: context {
      let pn = counter(page).get().first()
      if pn > 2 {
        align(center, text(
          font: sans_family,
          size: 8pt,
          fill: ink_faint,
          tracking: 1.5pt,
          upper(book_title),
        ))
      }
    },
  )
  set text(font: body_family, size: 11pt, fill: ink_black, lang: "ru")
  set par(leading: 0.72em, justify: true, first-line-indent: 1em)
  // Code blocks.
  show raw: it => {
    if it.block {
      block(
        fill: ink_code_bg,
        stroke: 0.5pt + ink_rule,
        inset: 6pt,
        radius: 2pt,
        width: 100%,
        text(font: mono_family, size: 9.5pt, it),
      )
    } else {
      box(
        fill: ink_code_bg,
        inset: (x: 2pt, y: 0pt),
        outset: (y: 2pt),
        radius: 1pt,
        text(font: mono_family, size: 9.5pt, it),
      )
    }
  }
  // Top-of-chapter pages have no header.
  // (Handled via the `to: "odd"` pagebreak in `chapter`.)

  // ── Cover page ─────────────────────────────────────
  // Full-bleed image cover. The PNG is generated from
  // `images/book-cover-art.typ` (see Book/README.md).
  // Recompile with:
  //   typst compile --format png --ppi 300 \
  //     Book/images/book-cover-art.typ \
  //     Book/images/book-cover-art.png
  // The image is sized to fill the page; the cover's
  // own internal frame + margins handle visual padding.
  set page(margin: 0pt, numbering: none, header: none)
  image("images/book-cover-art.png", width: 100%, height: 100%, fit: "cover")
  pagebreak()

  // ── Copyright / colophon ────────────────────────────
  set page(margin: book_page.margin)
  v(60%)
  align(left)[
    #text(font: body_family, size: 9pt, fill: ink_gray,
      "Книга Inkhaven — спутник литературного редактора inkhaven TUI."
    )
    #v(4mm)
    #text(font: body_family, size: 9pt, fill: ink_gray,
      "Исходники: " + h(2pt) + raw("https://github.com/vulogov/blackInkhaven", lang: "txt"),
    )
    #v(4mm)
    #text(font: body_family, size: 9pt, fill: ink_gray,
      "Свёрстано в Typst. Версии бинарника inkhaven сопоставляются с версиями книги по строке версии в выходных данных."
    )
  ]
  pagebreak()

  // ── Dedication ──────────────────────────────────────
  // Right-hand (recto) page, alone, centered both axes,
  // light serif italic. No page number, no header. The
  // text floats in white space — convention for dedication
  // pages in printed books.
  set page(margin: book_page.margin, numbering: none, header: none)
  align(center + horizon)[
    #text(
      font: body_family,
      style: "italic",
      size: 16pt,
      fill: ink_gray,
      "… моей жене. С любовью.",
    )
  ]
  pagebreak()

  // ── Table of Contents ──────────────────────────────
  text(font: sans_family, size: 22pt, weight: "bold", fill: ink_black, "Содержание")
  v(8mm)
  outline(title: none, indent: auto, depth: 2)
  pagebreak()

  // ── Body ────────────────────────────────────────────
  set page(numbering: "1", number-align: center)
  counter(page).update(1)
  for p in pages [
    #p
  ]
}
