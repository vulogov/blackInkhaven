// Developing a Constructed Language with Inkhaven — design tokens + page chrome.
//
// Modelled on Book/1.2.6_MANUAL/design.typ, but self-contained and built on
// fonts Typst bundles ("Libertinus Serif", "New Computer Modern") so the book
// compiles warning-free anywhere with a bare `typst compile`.

#let book_title    = "Developing a Constructed Language"
#let book_subtitle = "A Beginner's Guide to Conlanging with Inkhaven"
#let book_author   = "Vladimir Ulogov"
#let book_year     = "2026"

// ── Palette — warm paper, cool ink, restrained accents ──────────────
#let ink_black   = rgb("#1a1a1a")
#let ink_gray    = rgb("#5d5d5d")
#let ink_faint   = rgb("#9a9a9a")
#let ink_rule    = rgb("#c6c0b5")
#let ink_accent  = rgb("#7a4a2f")            // burnt sienna — chapter numbers
#let ink_term    = rgb("#2f5d7a")            // slate blue — term definitions
#let ink_code_bg = rgb("#f3eee4")
#let ink_call_bg = rgb("#f6f1e6")
#let ink_term_bg = rgb("#eef3f7")

// Bundled families only (no host-font setup, no warnings).
#let body_family = ("Libertinus Serif", "New Computer Modern")
#let mono_family = ("DejaVu Sans Mono",)            // bundled with Typst

#let book_page = (
  paper: "iso-b5",
  margin: (inside: 26mm, outside: 20mm, top: 22mm, bottom: 24mm),
  numbering: "1",
)

// ── Part divider — a page announcing a part ─────────────────────────
#let part(number: "I", title: "") = {
  pagebreak(weak: true)
  hide(heading(level: 1, numbering: none, outlined: true, bookmarked: true, [Part #number — #title]))
  v(7cm)
  align(center)[
    #text(font: body_family, size: 11pt, tracking: 3pt, fill: ink_gray, upper("Part " + number))
    #v(6mm)
    #line(length: 36%, stroke: 0.5pt + ink_rule)
    #v(6mm)
    #text(font: body_family, size: 26pt, weight: "bold", fill: ink_black, title)
  ]
  // No trailing pagebreak — the next chapter opens its own fresh page.
}

// ── Chapter opening ─────────────────────────────────────────────────
#let chapter(number: 0, title: "") = {
  pagebreak(weak: true)
  hide(heading(level: 1, numbering: none, outlined: true, bookmarked: true, [#str(number) — #title]))
  v(1.6cm)
  align(left)[
    #text(font: body_family, size: 9pt, tracking: 2pt, fill: ink_gray, upper("Chapter " + str(number)))
    #v(1mm)
    #text(font: body_family, size: 84pt, weight: "bold", fill: ink_accent, str(number))
    #v(-6mm)
    #text(font: body_family, size: 25pt, weight: "regular", fill: ink_black, title)
  ]
  v(1cm)
  line(length: 100%, stroke: 0.5pt + ink_rule)
  v(8mm)
}

// ── Appendix opening — a letter instead of a number ─────────────────
#let appendix(letter: "A", title: "") = {
  pagebreak(weak: true)
  hide(heading(level: 1, numbering: none, outlined: true, bookmarked: true, [Appendix #letter — #title]))
  v(1.6cm)
  align(left)[
    #text(font: body_family, size: 9pt, tracking: 2pt, fill: ink_gray, upper("Appendix " + letter))
    #v(1mm)
    #text(font: body_family, size: 84pt, weight: "bold", fill: ink_accent, letter)
    #v(-6mm)
    #text(font: body_family, size: 25pt, weight: "regular", fill: ink_black, title)
  ]
  v(1cm)
  line(length: 100%, stroke: 0.5pt + ink_rule)
  v(8mm)
}

// ── Section / subsection ────────────────────────────────────────────
#let section(title) = {
  hide(heading(level: 2, numbering: none, outlined: true, title))
  v(7mm)
  text(font: body_family, size: 15pt, weight: "bold", fill: ink_black, title)
  v(1.5mm)
}
#let subsection(title) = {
  v(4mm)
  text(font: body_family, size: 11.5pt, weight: "bold", fill: ink_black, title)
  v(0.5mm)
}

// ── Term box — DEFINE a linguistic term (used heavily, this book is for
//    readers new to linguistics). ─────────────────────────────────────
#let term(name, body) = {
  v(2mm)
  block(
    fill: ink_term_bg,
    stroke: (left: 2pt + ink_term),
    inset: (left: 9pt, right: 9pt, top: 7pt, bottom: 7pt),
    width: 100%,
    radius: 1pt,
    {
      text(font: body_family, size: 8pt, weight: "bold", fill: ink_term, tracking: 1pt, "TERM")
      h(6pt)
      text(font: body_family, size: 11pt, weight: "bold", fill: ink_term, name)
      v(2mm)
      body
    },
  )
  v(2mm)
}

// ── Note / tip callout ──────────────────────────────────────────────
#let callout(label: "Note", body) = {
  v(2mm)
  block(
    fill: ink_call_bg,
    stroke: (left: 2pt + ink_accent),
    inset: (left: 9pt, right: 9pt, top: 7pt, bottom: 7pt),
    width: 100%,
    radius: 1pt,
    {
      text(font: body_family, size: 8pt, weight: "bold", fill: ink_accent, tracking: 1.5pt, upper(label))
      v(2mm)
      body
    },
  )
  v(2mm)
}

// ── Chapter-end recap ───────────────────────────────────────────────
#let recap(items) = {
  v(7mm)
  block(
    fill: ink_call_bg,
    stroke: (left: 2pt + ink_accent),
    inset: (left: 9pt, right: 9pt, top: 8pt, bottom: 8pt),
    width: 100%,
    radius: 1pt,
    {
      text(font: body_family, size: 9pt, weight: "bold", fill: ink_accent, tracking: 1.5pt, "WHAT YOU LEARNED")
      v(2mm)
      list(..items)
    },
  )
}

// ── Master document wrapper ─────────────────────────────────────────
#let book(pages) = {
  set document(title: book_title, author: book_author)
  set text(font: body_family, size: 11pt, fill: ink_black, lang: "en")
  set par(leading: 0.72em, justify: true, first-line-indent: 1em)
  show raw.where(block: true): it => block(
    fill: ink_code_bg, stroke: 0.5pt + ink_rule, inset: 7pt, radius: 2pt, width: 100%,
    text(font: mono_family, size: 9pt, it),
  )
  show raw.where(block: false): it => box(
    fill: ink_code_bg, inset: (x: 2pt, y: 0pt), outset: (y: 2pt), radius: 1pt,
    text(font: mono_family, size: 9.5pt, it),
  )

  // Front matter — cover and contents — has no header or page number.
  set page(paper: book_page.paper, margin: book_page.margin, numbering: none, header: none)

  // Cover — kept comfortably shorter than the page so nothing spills.
  v(1.8cm)
  align(center)[
    #image("logo_black.png", width: 40%)
    #v(8mm)
    #text(font: body_family, size: 25pt, weight: "bold", fill: ink_black, book_title)
    #v(4mm)
    #text(font: body_family, size: 13pt, style: "italic", fill: ink_gray, book_subtitle)
    #v(10mm)
    #text(font: body_family, size: 10.5pt, fill: ink_gray, "A complete guide to the Inkhaven ConLang Suite")
    #v(2mm)
    #text(font: body_family, size: 9pt, fill: ink_faint, book_author + " · " + book_year + " · examples assume Inkhaven 1.3.17+")
  ]
  pagebreak()

  // Contents.
  text(font: body_family, size: 22pt, weight: "bold", fill: ink_black, "Contents")
  v(7mm)
  outline(title: none, indent: auto, depth: 2)
  pagebreak()

  // Body — running header + page numbers, restarting at 1.
  set page(
    numbering: "1",
    number-align: center,
    header: context {
      if counter(page).get().first() > 1 {
        align(center, text(font: body_family, size: 8pt, fill: ink_faint, tracking: 1.5pt, upper(book_title)))
      }
    },
  )
  counter(page).update(1)
  for p in pages [ #p ]
}
