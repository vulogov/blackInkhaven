// Kant's Transcendental Idealism and Eternal Progression — design tokens.
//
// A restrained academic-essay style, self-contained on the fonts Typst bundles.
// This is the *outcome* volume — the essay the companion manual's process
// produced — so it looks like a scholarly article, not a manual: single column,
// footnote citations, an abstract block, a reference list, and an Index Locorum.

#let title    = "Kant's Transcendental Idealism and Eternal Progression"
#let subtitle = "On the Resemblance, and the Rupture, Between an Asymptotic and a Teleological Perfection"
#let author   = "Vladimir Ulogov"
#let year     = "2026"

// ── Palette — near-black ink on white, one restrained accent ────────
#let ink_black = rgb("#161310")
#let ink_gray  = rgb("#544f47")
#let ink_faint = rgb("#8f877b")
#let ink_rule  = rgb("#cfc7b6")
#let ink_accent = rgb("#5a4a2e")            // dark umber — headings, rules

#let body_family = ("Libertinus Serif", "New Computer Modern")
#let mono_family = ("DejaVu Sans Mono",)

// ── Section headings ────────────────────────────────────────────────
#let section(title) = {
  v(6mm)
  block(sticky: true, below: 3mm,
    text(font: body_family, size: 13pt, weight: "bold", fill: ink_accent, title))
}
#let subsection(title) = {
  v(3mm)
  block(sticky: true, below: 2mm,
    text(font: body_family, size: 11pt, weight: "bold", style: "italic", fill: ink_black, title))
}

// ── Epigraph — a quoted passage set apart under a section. ──────────
#let epigraph(body, attribution) = {
  v(1mm)
  pad(left: 8mm, right: 8mm, {
    set par(justify: false, first-line-indent: 0pt, leading: 0.68em)
    text(font: body_family, size: 10pt, style: "italic", fill: ink_gray, body)
    v(1mm)
    align(right, text(font: body_family, size: 9pt, fill: ink_faint, "— " + attribution))
  })
  v(2mm)
}

// ── An Index Locorum entry — a source heading and its cited loci. ───
// Mirrors exactly what `inkhaven index-locorum --format typst` emits: a level-2
// heading per source, a bullet per locus, natural-sorted, chapter tagged.
#let locorum_source(title, key) = {
  v(2.5mm)
  block(sticky: true, below: 1.4mm,
    text(font: body_family, size: 11pt, weight: "bold", fill: ink_accent, title)
    + h(4pt)
    + text(font: mono_family, size: 8.5pt, fill: ink_faint, "@" + key))
}
#let locus(ref, where) = {
  set par(hanging-indent: 6mm, first-line-indent: 0pt, justify: false, leading: 0.6em)
  text(font: body_family, size: 10pt, fill: ink_black, ref)
  h(1em)
  text(font: body_family, size: 9pt, fill: ink_gray, where)
  linebreak()
}

// ── Master document wrapper ─────────────────────────────────────────
#let article(abstract: [], keywords: (), body) = {
  set document(title: title, author: author)
  set text(font: body_family, size: 10.5pt, fill: ink_black, lang: "en")
  set par(leading: 0.68em, justify: true, first-line-indent: 1.2em)
  set page(
    paper: "a4",
    margin: (x: 34mm, y: 30mm),
    numbering: "1",
    number-align: center,
  )
  // Footnotes (where the loci land) a touch smaller.
  show footnote.entry: set text(size: 8.5pt)
  set footnote.entry(separator: line(length: 32%, stroke: 0.4pt + ink_rule))
  show heading: set text(font: body_family)
  show raw: set text(font: mono_family, size: 9.5pt)

  // ── Title block ──────────────────────────────────────────────────
  v(4mm)
  align(center, {
    text(font: body_family, size: 9pt, tracking: 3pt, fill: ink_gray,
      upper("An Essay in Comparative Philosophical Theology"))
    v(6mm)
    text(font: body_family, size: 20pt, weight: "bold", fill: ink_black, title)
    v(3mm)
    text(font: body_family, size: 11.5pt, style: "italic", fill: ink_gray, subtitle)
    v(5mm)
    line(length: 26%, stroke: 0.5pt + ink_accent)
    v(4mm)
    text(font: body_family, size: 11pt, fill: ink_black, author)
    v(1mm)
    text(font: body_family, size: 9pt, fill: ink_faint, year)
  })
  v(7mm)

  // ── Abstract + keywords ──────────────────────────────────────────
  block(width: 100%, inset: (x: 8mm), {
    set par(justify: true, first-line-indent: 0pt, leading: 0.64em)
    text(font: body_family, size: 9pt, weight: "bold", fill: ink_accent, tracking: 1pt, "ABSTRACT")
    v(1.5mm)
    text(font: body_family, size: 9.5pt, fill: ink_gray, abstract)
    if keywords.len() > 0 {
      v(2mm)
      text(font: body_family, size: 9pt, weight: "bold", fill: ink_accent, "Keywords: ")
      text(font: body_family, size: 9pt, style: "italic", fill: ink_gray, keywords.join(" · "))
    }
  })
  v(6mm)
  line(length: 100%, stroke: 0.5pt + ink_rule)
  v(4mm)

  body
}
