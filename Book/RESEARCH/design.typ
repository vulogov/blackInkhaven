// Grounding Your Book in Fact — design tokens + page chrome.
//
// Modelled on Book/CONLANG_DEVELOPMENT/design.typ (self-contained, built on the
// fonts Typst bundles) with two additions: the afterword helpers borrowed from
// the 1.2.6 manual (sans family, drop cap, contact table) and a small set of
// fletcher diagrams so the book teaches with pictures instead of screenshots.

#import "@preview/fletcher:0.5.8" as fletcher: diagram, node, edge

#let book_title    = "Grounding Your Book in Fact"
#let book_subtitle = "Researching Fiction and Non-Fiction with Inkhaven's Research Assistant"
#let book_author   = "Vladimir Ulogov"
#let book_year     = "2026"
#let book_version  = "Inkhaven 3.0.0"

// ── Palette — warm paper, cool ink, restrained accents ──────────────
#let ink_black   = rgb("#1a1a1a")
#let ink_gray    = rgb("#5d5d5d")
#let ink_faint   = rgb("#9a9a9a")
#let ink_rule    = rgb("#c6c0b5")
#let ink_accent  = rgb("#2f5d7a")            // slate blue — chapter numbers (research = cool)
#let ink_smoke   = rgb("#7d736a")            // muted brown — cover eyebrow
#let ink_paper   = rgb("#fdfaf3")            // warm cream — cover ground
#let ink_term    = rgb("#2f5d7a")            // slate blue — term definitions
#let ink_code_bg = rgb("#f3eee4")
#let ink_call_bg = rgb("#f6f1e6")
#let ink_term_bg = rgb("#eef3f7")
#let ink_recap   = rgb("#3f6b4a")            // muted green — recap accent
#let ink_recap_bg = rgb("#e9f3ea")           // pastel mint — "what you learned"

// Bundled families only (no host-font setup, no warnings).
#let body_family = ("Libertinus Serif", "New Computer Modern")
// Typst bundles no sans family we can rely on warning-free, so the small-caps
// "eyebrow" labels use the serif (as the CONLANG book does). Kept as its own
// token so a host with a real sans can swap it in one place.
#let sans_family = ("Libertinus Serif", "New Computer Modern")
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
// NOTE (Typst 0.14): a block's `below` now sets the gap literally, rather than
// max()-ing with the surrounding paragraph spacing as older Typst did — so the
// tiny `below` values the CONLANG design used collapse a heading onto its body
// here. These give the heading real breathing room below while keeping more
// space above (so a heading still belongs to what follows it).
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

// ── Term box — DEFINE a term (this book assumes no research background). ──
#let term(name, body) = {
  v(2mm)
  block(
    fill: ink_term_bg, stroke: (left: 2pt + ink_term),
    inset: (left: 9pt, right: 9pt, top: 7pt, bottom: 7pt),
    width: 100%, radius: 1pt, breakable: false,
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
    fill: ink_call_bg, stroke: (left: 2pt + ink_accent),
    inset: (left: 9pt, right: 9pt, top: 7pt, bottom: 7pt),
    width: 100%, radius: 1pt, breakable: false,
    {
      text(font: body_family, size: 8pt, weight: "bold", fill: ink_accent, tracking: 1.5pt, upper(label))
      v(2mm)
      body
    },
  )
  v(2mm)
}

// ── Two-track callout — the same task for a novelist and a non-fiction
//    writer, side by side. Used throughout to keep both audiences in view. ──
#let two_track(fiction, nonfiction) = {
  v(2mm)
  // Wrap the two columns in a single non-breakable block so the whole unit —
  // both boxes and their "FICTION"/"NON-FICTION" headers — moves to the next page
  // together rather than splitting across a page boundary.
  block(breakable: false, width: 100%, grid(
    columns: (1fr, 1fr), gutter: 5mm,
    block(
      fill: ink_call_bg, stroke: (left: 2pt + ink_accent),
      inset: 8pt, width: 100%, radius: 1pt, breakable: false,
      { text(font: body_family, size: 8pt, weight: "bold", fill: ink_accent, tracking: 1pt, "FICTION"); v(2mm); fiction },
    ),
    block(
      fill: ink_recap_bg, stroke: (left: 2pt + ink_recap),
      inset: 8pt, width: 100%, radius: 1pt, breakable: false,
      { text(font: body_family, size: 8pt, weight: "bold", fill: ink_recap, tracking: 1pt, "NON-FICTION"); v(2mm); nonfiction },
    ),
  ))
  v(2mm)
}

// ── Chapter-end recap ───────────────────────────────────────────────
#let recap(items) = {
  v(7mm)
  block(
    fill: ink_recap_bg, stroke: (left: 2pt + ink_recap),
    inset: (left: 9pt, right: 9pt, top: 8pt, bottom: 8pt),
    width: 100%, radius: 1pt, breakable: false,
    {
      text(font: body_family, size: 9pt, weight: "bold", fill: ink_recap, tracking: 1.5pt, "WHAT YOU LEARNED")
      v(2mm)
      list(..items)
    },
  )
}

// ── Terminal screen — a faithful monospace rendering of a CLI / TUI screen
//    (the app IS a terminal; a monospace frame is truer than a diagram and
//    keeps the book self-contained). `body` is a raw block; `caption` names it.
//    Ported from the POETRY companion. ────────────────────────────────────────
#let screen(caption: "", body) = {
  v(2mm)
  block(breakable: false, width: 100%, {
    block(
      fill: ink_smoke,
      inset: (left: 8pt, right: 8pt, top: 3pt, bottom: 3pt),
      width: 100%,
      radius: (top-left: 2pt, top-right: 2pt),
      {
        text(font: mono_family, size: 8pt, fill: ink_paper, "● ● ●")
        h(6pt)
        text(font: body_family, size: 8.5pt, style: "italic", fill: ink_paper, caption)
      },
    )
    block(
      fill: ink_code_bg,
      stroke: 0.5pt + ink_rule,
      inset: 8pt,
      width: 100%,
      radius: (bottom-left: 2pt, bottom-right: 2pt),
      text(font: mono_family, size: 8.5pt, body),
    )
  })
  v(2mm)
}

// ── Afterword helpers (borrowed from the 1.2.6 manual's design) ─────
#let dropcap(letter) = box(baseline: 0.62em,
  text(font: body_family, size: 2.7em, weight: "bold", fill: ink_accent, letter))

#let chord_row(name, desc) = (name, desc)
#let chord_table(rows) = block(width: 100%, {
  for (name, desc) in rows {
    grid(columns: (28mm, 1fr), gutter: 4mm,
      text(font: body_family, weight: "bold", size: 10pt, fill: ink_black, name),
      text(font: body_family, size: 10pt, fill: ink_black, desc))
    v(1.6mm)
  }
})

// ── Figure caption wrapper for the diagrams below ───────────────────
#let figure_note(body) = align(center,
  text(font: body_family, style: "italic", size: 9pt, fill: ink_gray, body))

// ── Diagram helpers (fletcher) — the book teaches with these, not
//    screenshots. Each returns a centred, captioned figure. ───────────

// A shared node style for the boxes.
#let dnode(pos, body, fill: ink_call_bg) = node(
  pos, align(center, text(font: body_family, size: 8.5pt, body)),
  stroke: 0.6pt + ink_rule, fill: fill, corner-radius: 2pt, inset: 6pt,
)

// The trust ladder — the spine of the whole book. Higher = more verifiable.
#let trust_ladder() = {
  v(3mm)
  align(center, diagram(
    spacing: (0mm, 5mm),
    dnode((0, 0), [*Computed · Simulation*\ deterministic — you can re-run it], fill: ink_recap_bg),
    dnode((0, 1), [*Structured*\ Wikidata · GeoNames — cited by id]),
    dnode((0, 2), [*Scholarly*\ OpenAlex · arXiv — a real paper]),
    dnode((0, 3), [*Documents*\ sources you imported]),
    dnode((0, 4), [*Web*\ a cited page, fact-checked]),
    dnode((0, 5), [*Model*\ an educated guess], fill: ink_code_bg),
    edge((0, 5), (0, 4), "->"), edge((0, 4), (0, 3), "->"),
    edge((0, 3), (0, 2), "->"), edge((0, 2), (0, 1), "->"),
    edge((0, 1), (0, 0), "->"),
    edge((1.15, 5.2), (1.15, -0.2), "->", stroke: 1pt + ink_accent,
      label: text(font: body_family, size: 8pt, fill: ink_accent, [more\ trustworthy]),
      label-side: right),
  ))
  figure_note[The trust ladder — every fact you keep carries a rung, recorded as its provenance.]
  v(3mm)
}

// The everyday loop — how one fact enters the book.
#let fact_loop() = {
  v(3mm)
  align(center, diagram(
    spacing: 7mm,
    dnode((0, 0), [*Ask*\ a question]),
    dnode((1, 0), [*Ground*\ on your facts]),
    dnode((2, 0), [*Confirm*\ the gate]),
    dnode((3, 0), [*Fact*\ kept + cited], fill: ink_recap_bg),
    edge((0, 0), (1, 0), "->"), edge((1, 0), (2, 0), "->"), edge((2, 0), (3, 0), "->"),
  ))
  figure_note[Every claim earns its place: nothing is written to your Facts without your say-so.]
  v(3mm)
}

// The authoritative sources fanning into your corpus — the Part II picture.
#let sources_fan() = {
  v(3mm)
  align(center, diagram(
    spacing: (16mm, 4mm),
    dnode((0, 0), [*Wikidata · GeoNames*\ structured]),
    dnode((0, 1), [*OpenAlex · arXiv*\ scholarly]),
    dnode((0, 2), [*Gutenberg*\ public-domain books]),
    dnode((0, 3), [*Web*\ cited pages]),
    dnode((2, 1.5), [*Your Facts*\ each one cited], fill: ink_recap_bg),
    edge((0, 0), (2, 1.5), "->"), edge((0, 1), (2, 1.5), "->"),
    edge((0, 2), (2, 1.5), "->"), edge((0, 3), (2, 1.5), "->"),
  ))
  figure_note[Each source starts a fact higher on the ladder than a model's guess — and each one hands you a citation for free.]
  v(3mm)
}

// Triangulation — one claim held up against independent sources at once.
#let triangulate_diagram() = {
  v(3mm)
  align(center, diagram(
    spacing: (15mm, 3mm),
    dnode((0, 1), [*A claim*\ to test]),
    dnode((1, 0), [Wikidata]),
    dnode((1, 1), [OpenAlex]),
    dnode((1, 2), [arXiv]),
    dnode((2, 1), [*Verdict*\ do they agree?], fill: ink_recap_bg),
    edge((0, 1), (1, 0), "->"), edge((0, 1), (1, 1), "->"), edge((0, 1), (1, 2), "->"),
    edge((1, 0), (2, 1), "->"), edge((1, 1), (2, 1), "->"), edge((1, 2), (2, 1), "->"),
  ))
  figure_note[A claim the sources *agree* on, with none contradicting, is far firmer than any single one alone.]
  v(3mm)
}

// Corroborate vs. refute — two mirror-image checks.
#let two_gates() = {
  v(3mm)
  align(center, diagram(
    spacing: (14mm, 4mm),
    dnode((1, 0), [*A candidate fact*]),
    dnode((0, 1), [*Triangulate*\ "who SUPPORTS this?"]),
    dnode((2, 1), [*Refute*\ "can I DISPROVE this?"]),
    dnode((1, 2), [*Kept only if it survives both angles*], fill: ink_recap_bg),
    edge((1, 0), (0, 1), "->"), edge((1, 0), (2, 1), "->"),
    edge((0, 1), (1, 2), "->"), edge((2, 1), (1, 2), "->"),
  ))
  figure_note[Corroboration asks who agrees; refutation attacks the claim. A fact that passes both is one you can lean on.]
  v(3mm)
}

// Computing a fact — looked-up inputs become a derived, top-rung fact.
#let compute_climb() = {
  v(3mm)
  align(center, diagram(
    spacing: (14mm, 3mm),
    dnode((0, 0), [*Place A*\ GeoNames]),
    dnode((0, 1), [*Place B*\ GeoNames]),
    dnode((1, 0.5), [#strong[/calc]\ haversine]),
    dnode((2, 0.5), [*Distance*\ computed — top rung], fill: ink_recap_bg),
    edge((0, 0), (1, 0.5), "->"), edge((0, 1), (1, 0.5), "->"),
    edge((1, 0.5), (2, 0.5), "->"),
  ))
  figure_note[Two looked-up places become one derived fact — the firmest kind, because anyone can re-run the sum.]
  v(3mm)
}

// Borrowed vs invented — two kinds of fact, checked two different ways.
#let authorial_split() = {
  v(3mm)
  align(center, diagram(
    spacing: (16mm, 5mm),
    dnode((0, 0.5), [*A fact in\ your book*]),
    dnode((1, 0), [*Borrowed*\ on the ladder →\ /factcheck]),
    dnode((1, 1), [*Invented*\ undisputed ※ →\ /undisputed (coherence)], fill: ink_recap_bg),
    edge((0, 0.5), (1, 0), "->"), edge((0, 0.5), (1, 1), "->"),
  ))
  figure_note[Borrowed facts are checked against the world; invented facts are checked only against themselves.]
  v(3mm)
}

// Composing out — the corpus finally producing output back into the book.
#let compose_out() = {
  v(3mm)
  align(center, diagram(
    spacing: (15mm, 3mm),
    dnode((0, 1.5), [*Your corpus*\ Facts + Sources]),
    dnode((1, 0), [#strong[/synthesize]\ cited overview]),
    dnode((1, 1), [#strong[/outline]\ fact-citing plan]),
    dnode((1, 2), [#strong[/gaps]\ what's missing]),
    dnode((1, 3), [#strong[/bibliography]\ references]),
    dnode((2, 1.5), [*Your book*], fill: ink_recap_bg),
    edge((0, 1.5), (1, 0), "->"), edge((0, 1.5), (1, 1), "->"),
    edge((0, 1.5), (1, 2), "->"), edge((0, 1.5), (1, 3), "->"),
    edge((1, 0), (2, 1.5), "->"), edge((1, 1), (2, 1.5), "->"),
    edge((1, 2), (2, 1.5), "->"), edge((1, 3), (2, 1.5), "->"),
  ))
  figure_note[The corpus you built is not the end product — it is the raw material the last commands turn back into your book.]
  v(3mm)
}

// The gaps -> batch -> facts loop — the corpus filling its own holes, headlessly.
#let batch_loop() = {
  v(3mm)
  align(center, diagram(
    spacing: 8mm,
    dnode((0, 0), [#strong[/gaps]\ open questions]),
    dnode((1, 0), [a questions\ file]),
    dnode((2, 0), [#strong[\-\-batch]\ headless]),
    dnode((3, 0), [candidate\ facts]),
    dnode((3, 1), [*your corpus*], fill: ink_recap_bg),
    edge((0, 0), (1, 0), "->"), edge((1, 0), (2, 0), "->"),
    edge((2, 0), (3, 0), "->"), edge((3, 0), (3, 1), "->", label: text(size: 7pt, [confirm])),
    edge((3, 1), (0, 0), "->", stroke: (dash: "dashed"), label: text(size: 7pt, [repeat])),
  ))
  figure_note[The corpus can be told to go and fill its own gaps while you do something else.]
  v(3mm)
}

// The larger arc — what the Research Assistant does over a whole project.
#let research_arc() = {
  v(3mm)
  align(center, diagram(
    spacing: 9mm,
    dnode((0, 0), [*Acquire*\ ask · search · import]),
    dnode((1, 0), [*Cross-check*\ triangulate · refute]),
    dnode((1, 1), [*Maintain*\ upgrade · staleness]),
    dnode((0, 1), [*Compose*\ synthesize · bibliography], fill: ink_recap_bg),
    edge((0, 0), (1, 0), "->"), edge((1, 0), (1, 1), "->"),
    edge((1, 1), (0, 1), "->"), edge((0, 1), (0, 0), "->", stroke: (dash: "dashed")),
  ))
  figure_note[The corpus you build is not a dead pile of notes — it is checked, kept fresh, and composed back out.]
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
  show raw.where(block: false): it => box(
    fill: ink_code_bg, inset: (x: 2pt, y: 0pt), outset: (y: 2pt), radius: 1pt,
    text(font: mono_family, size: 9.5pt, it),
  )

  // ── Cover — typographic, warm cream ground, slate-blue rule frame. ──
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
    #place(top + center, dy: 60mm, block(width: 74%)[
      #set par(justify: false)
      #align(center)[
        #text(font: body_family, size: 12pt, tracking: 4pt, fill: ink_smoke, upper("Research with Inkhaven"))
        #v(11mm)
        #text(font: body_family, size: 30pt, weight: "bold", fill: ink_black, book_title)
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

  // Contents — plain paper.
  set page(margin: book_page.margin, fill: white)
  text(font: body_family, size: 22pt, weight: "bold", fill: ink_black, "Contents")
  v(7mm)
  outline(title: none, indent: auto, depth: 2)
  pagebreak()

  // Body — running header + page numbers.
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
