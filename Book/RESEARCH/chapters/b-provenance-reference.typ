#import "../design.typ": *

#appendix(letter: "B", title: "The Trust Ladder and Provenance")

Every fact you keep records a *provenance* — where it came from — and that origin
places it on the trust ladder. This appendix lists the origins from firmest to
softest, with the glyph each wears in the Facts tree and whether it crosses the
fact-check gate.

#section("The rungs, top to bottom")

#let rung(glyph, origin, meaning, gate) = {
  grid(columns: (10mm, 26mm, 1fr), gutter: 4mm, inset: (y: 2pt),
    align(center, text(size: 13pt, fill: ink_accent, glyph)),
    text(font: body_family, weight: "bold", size: 10pt, origin),
    text(font: body_family, size: 9.5pt, meaning + " " + text(fill: ink_gray, style: "italic", gate)))
  v(1mm)
  line(length: 100%, stroke: 0.3pt + ink_rule)
  v(1mm)
}

#rung("≡", "computed", "A fact you derived by calculation (/calc). The firmest rung — anyone can re-run it.", "Gate-skipped.")
#rung("≡", "simulation", "A deterministic fact from the World book's simulation. As firm as computed, for the same reason.", "Gate-skipped.")
#rung("◆", "wikidata", "A structured datum, cited by Q-id.", "Gate-skipped.")
#rung("⊕", "geonames", "A real place from the gazetteer, cited by id.", "Gate-skipped.")
#rung("§", "openalex", "A scholarly work, cited by DOI; auto-filed to Sources.", "Gate-skipped.")
#rung("§", "arxiv", "A preprint, cited by id; auto-filed to Sources.", "Gate-skipped.")
#rung("▪", "document", "Drawn from a source you imported into your corpus.", "Fact-checked at the gate.")
#rung("↑", "promoted", "A Note you promoted into the Facts book.", "Gate-skipped — trusted as the note's own grounding.")
#rung("◇", "web", "Grounded on a cited web page.", "Fact-checked at the gate.")
#rung("·", "model", "The model's unaided answer — an educated guess.", "Fact-checked (and refuted, if enabled).")

#callout(label: "Two glyphs outside the ladder")[
  Two marks are not rungs at all. The verdict glyphs from an audit — *✓* passed,
  *?* dubious, *✗* failed (`/factcheck`) — sit *on top of* a fact's tier glyph to
  report its last check. And *※* marks an *undisputed* (authorial) fact, which sits
  outside the ladder entirely: it is exempt from `/factcheck` and checked only for
  internal coherence by `/undisputed`. Its ※ takes on the coherence verdict's
  colour — plausible, odd, or incoherent.
]

#callout(label: "Origins without a tier mark")[
  A few origins carry *no* tier glyph at all — a fact kept from `/archive` (the
  Internet Archive), `/wikisource`, or entered by hand (`manual`) shows no rung
  mark in the tree. They are still recorded with full provenance (visible in
  `/sources`); they simply sit off the tiered ladder rather than on a named rung.
]

#section("Reading a fact's provenance")

The tier glyph is the at-a-glance version. The full record — the specific source,
the query that produced it, any check verdict folded in — travels with the fact and
answers the one question this whole book is built around: *how do you know?*

The point of the ladder is not to forbid the low rungs. A novelist grounding the
feel of a place on a web page has done legitimate work; the `◇` simply keeps that
fact honest about where it stands. What the ladder insists on is only this — that a
fact never pretend to be firmer than its origin, and that *you always know which
rung you are standing on.*
