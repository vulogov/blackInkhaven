// Theology and Philosophy with Inkhaven — design tokens + page chrome.
//
// Visual style follows Book/DEVELOPING/design.typ (same warm earth palette, same
// named callouts, same typography) so the two books read as one shelf. Diagrams
// are drawn with fletcher rather than screenshots. Adds a `transcript` helper for
// showing a Research Assistant exchange (a `/command` and its answer).

#import "@preview/fletcher:0.5.8" as fletcher: diagram, node, edge

#let book_title    = "Theology and Philosophy with Inkhaven"
#let book_subtitle = "Researching, Writing, and Composing a Work of Argument — A Worked Example"
#let book_author   = "Vladimir Ulogov"
#let book_year     = "2026"
#let book_version  = "Inkhaven 1.6.22"

// ── Palette — warm paper, earth ink, growth-green accents ───────────
#let ink_black   = rgb("#1e1a15")
#let ink_gray    = rgb("#5d564c")
#let ink_faint   = rgb("#9a9084")
#let ink_rule    = rgb("#c9bfa9")
#let ink_accent  = rgb("#8a5a2b")            // burnt sienna — chapter numbers, terms
#let ink_smoke   = rgb("#7d736a")            // muted brown — cover eyebrow
#let ink_paper   = rgb("#fbf6ea")            // warm cream — cover ground
#let ink_code_bg = rgb("#f2ecdd")

// Callout accents + their pale grounds — one hue per kind of aside.
#let ink_term    = rgb("#8a5a2b")            // sienna  — TERM
#let ink_term_bg = rgb("#f4ebdd")
#let ink_green   = rgb("#3f6b4a")            // forest  — INSIGHT / recap
#let ink_green_bg = rgb("#e8f1e9")
#let ink_gold    = rgb("#94711f")            // amber   — ASK YOURSELF (questions)
#let ink_gold_bg = rgb("#f6efd6")
#let ink_rust    = rgb("#9c4526")            // rust    — PITFALL
#let ink_rust_bg = rgb("#f6e6df")
#let ink_teal    = rgb("#2f6668")            // teal    — NOTE / TRY IT
#let ink_teal_bg = rgb("#e3eeee")

// Bundled families only (no host-font setup, no warnings).
#let body_family = ("Libertinus Serif", "New Computer Modern")
#let sans_family = ("Libertinus Serif", "New Computer Modern")
#let mono_family = ("DejaVu Sans Mono",)

#let book_page = (
  paper: "iso-b5",
  margin: (inside: 26mm, outside: 20mm, top: 22mm, bottom: 24mm),
  numbering: "1",
)

// ── Part divider ────────────────────────────────────────────────────
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

// ── Section / subsection ────────────────────────────────────────────
#let section(title) = {
  hide(heading(level: 2, numbering: none, outlined: true, title))
  block(
    sticky: true, above: 8mm, below: 3.2mm,
    text(font: body_family, size: 15pt, weight: "bold", fill: ink_black, title),
  )
}
#let subsection(title) = {
  block(
    sticky: true, above: 5.5mm, below: 2.4mm,
    text(font: body_family, size: 11.5pt, weight: "bold", fill: ink_black, title),
  )
}

// ── Term box — DEFINE a term the first time it appears. ─────────────
#let term(name, body) = {
  v(2mm)
  block(
    fill: ink_term_bg, stroke: (left: 2pt + ink_term),
    inset: (left: 9pt, right: 9pt, top: 7pt, bottom: 7pt),
    width: 100%, radius: 1pt, breakable: false,
    {
      set par(justify: false, first-line-indent: 0pt)
      text(font: body_family, size: 8pt, weight: "bold", fill: ink_term, tracking: 1pt, "TERM")
      h(6pt)
      text(font: body_family, size: 11pt, weight: "bold", fill: ink_term, name)
      v(2mm)
      body
    },
  )
  v(2mm)
}

// ── Named callouts — one colour per kind of aside. ──────────────────
#let _callout(label, accent, bg, body) = {
  v(2mm)
  block(
    fill: bg, stroke: (left: 2pt + accent),
    inset: (left: 9pt, right: 9pt, top: 7pt, bottom: 7pt),
    width: 100%, radius: 1pt, breakable: false,
    {
      set par(justify: false, first-line-indent: 0pt)
      text(font: body_family, size: 8pt, weight: "bold", fill: accent, tracking: 1.5pt, upper(label))
      v(2mm)
      body
    },
  )
  v(2mm)
}

// ── A concrete edit to a config / definition file. ──────────────────
#let config(name, body) = {
  v(2mm)
  block(
    fill: white, stroke: (left: 2pt + ink_accent),
    inset: (left: 9pt, right: 9pt, top: 6pt, bottom: 8pt),
    width: 100%, radius: 1pt, breakable: false,
    {
      set par(justify: false, first-line-indent: 0pt)
      text(font: body_family, size: 8pt, weight: "bold", fill: ink_accent, tracking: 1.5pt, "EDIT · " + name)
      v(2mm)
      body
    },
  )
  v(2mm)
}

// A practical remark about how Inkhaven behaves.
#let note(body) = _callout("Note", ink_teal, ink_teal_bg, body)
// A deeper principle — the "why", the thing to remember.
#let insight(body) = _callout("Insight", ink_green, ink_green_bg, body)
// A question to put to yourself before you begin.
#let question(body) = _callout("Ask Yourself", ink_gold, ink_gold_bg, body)
// A common mistake, and how to avoid it.
#let pitfall(body) = _callout("Pitfall", ink_rust, ink_rust_bg, body)
// A short hands-on exercise at the keyboard.
#let tryit(body) = _callout("Try It", ink_accent, ink_term_bg, body)

// ── A Research Assistant exchange — a `/command` and the answer it returns. ──
#let transcript(cmd, body) = {
  v(2mm)
  block(
    fill: ink_code_bg, stroke: 0.5pt + ink_rule,
    inset: (left: 9pt, right: 9pt, top: 7pt, bottom: 8pt),
    width: 100%, radius: 2pt, breakable: false,
    {
      set par(justify: false, first-line-indent: 0pt, leading: 0.6em)
      text(font: mono_family, size: 9pt, weight: "bold", fill: ink_accent, "❯ " + cmd)
      v(2.4mm)
      set text(font: body_family, size: 9.5pt, fill: ink_gray)
      body
    },
  )
  v(2mm)
}

// ── Chapter-end recap ───────────────────────────────────────────────
#let recap(items) = {
  v(7mm)
  block(
    fill: ink_green_bg, stroke: (left: 2pt + ink_green),
    inset: (left: 9pt, right: 9pt, top: 8pt, bottom: 8pt),
    width: 100%, radius: 1pt, breakable: false,
    {
      set par(justify: false, first-line-indent: 0pt)
      text(font: body_family, size: 9pt, weight: "bold", fill: ink_green, tracking: 1.5pt, "WHAT YOU LEARNED")
      v(2mm)
      list(..items)
    },
  )
}

// ── Afterword helpers ───────────────────────────────────────────────
#let dropcap(letter) = box(baseline: 0.62em,
  text(font: body_family, size: 2.7em, weight: "bold", fill: ink_accent, letter))

#let figure_note(body) = align(center,
  text(font: body_family, style: "italic", size: 9pt, fill: ink_gray, body))

// A two-column table of labelled links / contacts (afterword).
#let chord_row(name, desc) = (name, desc)
#let chord_table(rows) = block(width: 100%, {
  for (name, desc) in rows {
    grid(columns: (34mm, 1fr), gutter: 4mm,
      text(font: mono_family, weight: "bold", size: 9.5pt, fill: ink_black, name),
      text(font: body_family, size: 10pt, fill: ink_black, desc))
    v(1.6mm)
  }
})

// ── Diagram helpers (fletcher) ──────────────────────────────────────
#let dnode(pos, body, fill: ink_term_bg) = node(
  pos, align(center, text(font: body_family, size: 8.5pt, body)),
  stroke: 0.6pt + ink_rule, fill: fill, corner-radius: 2pt, inset: 6pt,
)

// The research loop this book walks, on one worked question.
#let research_arc() = {
  v(3mm)
  align(center, diagram(
    spacing: 9mm,
    dnode((0, 0), [*Frame*\ the question,\ set the genre]),
    dnode((1, 0), [*Gather*\ primary sources\ · scripture]),
    dnode((2, 0), [*Interrogate*\ SCHOLAR: relate,\ contradict, converge]),
    dnode((2, 1), [*Compose*\ draft with\ cited loci]),
    dnode((1, 1), [*Confront*\ the draft against\ the corpus], fill: ink_green_bg),
    dnode((0, 1), [*Produce*\ essay · bibliography\ · index locorum], fill: ink_green_bg),
    edge((0, 0), (1, 0), "->"), edge((1, 0), (2, 0), "->"),
    edge((2, 0), (2, 1), "->"), edge((2, 1), (1, 1), "->"),
    edge((1, 1), (0, 1), "->"),
    edge((1, 1), (1, 0), "->", stroke: (dash: "dashed")),
  ))
  figure_note[The work is a loop, not a line: you frame the question, gather the sources, interrogate them for tension and agreement, draft against what you found, confront the draft with the corpus, and produce the finished argument — returning to gather more wherever the confront exposes a gap.]
  v(3mm)
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
  show raw.where(block: false): it => highlight(
    fill: ink_code_bg, extent: 1.5pt, radius: 1pt,
    text(font: mono_family, size: 9.5pt, it),
  )

  // ── Cover — typographic, warm cream ground, sienna rule frame. ──
  set page(paper: book_page.paper, margin: 0pt, numbering: none, header: none, fill: ink_paper)
  block(width: 100%, height: 100%)[
    #place(top + left, dx: 12mm, dy: 12mm,
      rect(width: 100% - 24mm, height: 100% - 24mm, stroke: 1pt + ink_accent))
    #place(top + left, dx: 14mm, dy: 14mm,
      rect(width: 100% - 28mm, height: 100% - 28mm, stroke: 0.4pt + ink_accent))
    #place(top + center, dy: 34mm, {
      let dot(dx, r) = place(top + center, dx: dx, dy: 0pt, circle(radius: r, fill: ink_accent))
      dot(-18mm, 1.6mm); dot(-9mm, 1.1mm); dot(0mm, 2.2mm); dot(9mm, 1.1mm); dot(18mm, 1.6mm)
    })
    #place(top + center, dy: 58mm, block(width: 78%)[
      #set par(justify: false)
      #align(center)[
        #text(font: body_family, size: 12pt, tracking: 4pt, fill: ink_smoke, upper("Working with Inkhaven"))
        #v(11mm)
        #text(font: body_family, size: 27pt, weight: "bold", fill: ink_black, book_title)
        #v(6mm)
        #line(length: 55%, stroke: 0.6pt + ink_accent)
        #v(6mm)
        #text(font: body_family, size: 12.5pt, style: "italic", fill: ink_smoke, book_subtitle)
      ]
    ])
    #place(bottom + center, dy: -30mm, align(center)[
      #text(font: body_family, size: 10pt, fill: ink_smoke, book_author)
      #v(2mm)
      #text(font: body_family, size: 9pt, fill: ink_smoke, book_year + " · examples assume " + book_version + " or newer")
    ])
  ]
  pagebreak()

  set page(margin: book_page.margin, fill: white)
  text(font: body_family, size: 22pt, weight: "bold", fill: ink_black, "Contents")
  v(7mm)
  outline(title: none, indent: auto, depth: 2)
  pagebreak()

  set page(
    numbering: "1", number-align: center,
    header: context {
      if counter(page).get().first() > 1 {
        align(center, text(font: body_family, size: 8pt, fill: ink_faint, tracking: 1.5pt, upper(book_title)))
      }
    },
  )
  counter(page).update(1)
  for p in pages [ #p ]
}
