// The Book of Inkhaven — cover art.
//
// Compile to PNG:
//   typst compile --format png --ppi 300 \
//     Book/images/book-cover-art.typ Book/images/book-cover-art.png
//
// Self-contained — uses only widely-available glyphs and
// generic font families so the cover renders cleanly on any
// host that has *some* serif + monospace + sans installed.
//
// Design: warm cream paper, burnt-sienna ink. Heavy serif
// title. A stylized inkhaven "tree of words" below the
// subtitle — three branches with paragraph-pilcrow leaves,
// rooted at the editor cursor glyph. A drop of ink in the
// margin. Author + version at the foot.

#set page(
  width:  800pt,
  height: 1200pt,
  margin: 0pt,
)

#let ink_paper   = rgb("#fdfaf3")
#let ink_black   = rgb("#1a1a1a")
#let ink_accent  = rgb("#7a4a2f")   // burnt sienna
#let ink_faint   = rgb("#c6c0b5")
#let ink_smoke   = rgb("#7d736a")

// ── Background ───────────────────────────────────────────
#rect(
  width:  100%,
  height: 100%,
  fill:   ink_paper,
  stroke: none,
)

// ── Inner frame (two-rule border) ────────────────────────
#place(
  top + left,
  dx: 36pt,
  dy: 36pt,
  rect(
    width:  800pt - 72pt,
    height: 1200pt - 72pt,
    stroke: 1pt + ink_accent,
    fill:   none,
  ),
)
#place(
  top + left,
  dx: 44pt,
  dy: 44pt,
  rect(
    width:  800pt - 88pt,
    height: 1200pt - 88pt,
    stroke: 0.4pt + ink_accent,
    fill:   none,
  ),
)

// ── Top ornament row ─────────────────────────────────────
// Three concentric circles inked in burnt-sienna. Crisp +
// universal — no font glyph dependency.
#let dot(dx, r) = place(
  top + center,
  dx: dx,
  dy: 92pt,
  circle(radius: r, fill: ink_accent),
)
#dot(-60pt, 5pt)
#dot(-30pt, 3.5pt)
#dot(  0pt, 7pt)
#dot( 30pt, 3.5pt)
#dot( 60pt, 5pt)

// ── Title block ──────────────────────────────────────────
#place(
  top + center,
  dy: 140pt,
  block(
    width: 100%,
    align(center)[
      #text(
        size: 18pt,
        fill: ink_smoke,
        tracking: 9pt,
        upper("The Book of"),
      )
      #v(18pt)
      #text(
        size: 110pt,
        weight: "bold",
        fill: ink_black,
        tracking: 4pt,
        "INKHAVEN",
      )
      #v(4pt)
      #line(length: 280pt, stroke: 0.6pt + ink_accent)
      #v(14pt)
      #text(
        size: 18pt,
        style: "italic",
        fill: ink_smoke,
        "An Author's Guide to the Literary TUI",
      )
    ],
  ),
)

// ── Tree of words (centred lower half of the cover) ──────
//
// Centre of composition at (cover_centre_x, dy ≈ 760pt).
// The trunk is vertical, ~150pt tall. Three branches fan
// upward; pilcrow leaves sit on each branch end. The
// cursor `▮` anchors the base of the trunk; a horizontal
// ground line ruled with em dashes sits just under it.

// Tree placement — geometry tuned so the leaves sit
// clearly below the subtitle and the cursor sits well
// above the author block.
#let tree_origin_dy = 900pt          // base of trunk (cursor)
#let trunk_height   = 130pt
#let branch_dy      = tree_origin_dy - trunk_height   // top of trunk = 770pt

// Trunk (vertical line, bold)
#place(
  top + center,
  dy: branch_dy,
  line(
    start: (0pt, 0pt),
    end:   (0pt, trunk_height),
    stroke: 3pt + ink_accent,
  ),
)

// Main branches — three branches fan up from the trunk top
#place(
  top + center,
  dy: branch_dy,
  line(
    start: (0pt, 0pt),
    end:   (-110pt, -80pt),
    stroke: 2pt + ink_accent,
  ),
)
#place(
  top + center,
  dy: branch_dy,
  line(
    start: (0pt, 0pt),
    end:   (110pt, -80pt),
    stroke: 2pt + ink_accent,
  ),
)
#place(
  top + center,
  dy: branch_dy,
  line(
    start: (0pt, 0pt),
    end:   (0pt, -100pt),
    stroke: 2pt + ink_accent,
  ),
)

// Twigs branching off the main branches
#place(
  top + center,
  dy: branch_dy - 80pt,
  line(
    start: (-110pt, 0pt),
    end:   (-155pt, -40pt),
    stroke: 1.2pt + ink_accent,
  ),
)
#place(
  top + center,
  dy: branch_dy - 80pt,
  line(
    start: (-110pt, 0pt),
    end:   (-75pt, -45pt),
    stroke: 1.2pt + ink_accent,
  ),
)
#place(
  top + center,
  dy: branch_dy - 80pt,
  line(
    start: (110pt, 0pt),
    end:   (155pt, -40pt),
    stroke: 1.2pt + ink_accent,
  ),
)
#place(
  top + center,
  dy: branch_dy - 80pt,
  line(
    start: (110pt, 0pt),
    end:   (75pt, -45pt),
    stroke: 1.2pt + ink_accent,
  ),
)

// Leaf paragraphs — pilcrows at branch tips
// leaf_dy_outer = where the twig tips end (top of foliage)
// leaf_dy       = where the centre branch ends (single tip)
#let leaf_dy       = branch_dy - 100pt - 14pt
#let leaf_dy_outer = branch_dy - 120pt - 14pt

#let leaf(dx, dy) = place(
  top + center,
  dx: dx,
  dy: dy,
  text(
    size: 26pt,
    fill: ink_accent,
    "¶",
  ),
)

// Five leaves: two on the left twig pair, centre branch
// tip, two on the right twig pair.
#leaf(-155pt, leaf_dy_outer)
#leaf( -75pt, leaf_dy_outer)
#leaf(   0pt, leaf_dy)
#leaf(  75pt, leaf_dy_outer)
#leaf( 155pt, leaf_dy_outer)

// Cursor — anchors the base of the trunk. A solid block
// the same shape as the editor's open-paragraph glyph.
// Placed slightly below the trunk's base for visual lift.
#place(
  top + center,
  dy: tree_origin_dy - 4pt,
  rect(
    width:  18pt,
    height: 28pt,
    fill: ink_black,
    stroke: none,
  ),
)

// Ground line below cursor — three em-dash glyphs each side
#place(
  top + center,
  dx: -88pt,
  dy: tree_origin_dy + 22pt,
  text(
    size: 16pt,
    fill: ink_faint,
    tracking: 4pt,
    "— — —",
  ),
)
#place(
  top + center,
  dx: 88pt,
  dy: tree_origin_dy + 22pt,
  text(
    size: 16pt,
    fill: ink_faint,
    tracking: 4pt,
    "— — —",
  ),
)

// ── Tagline beneath the tree ─────────────────────────────
#place(
  top + center,
  dy: tree_origin_dy + 65pt,
  text(
    size: 12pt,
    style: "italic",
    fill: ink_smoke,
    "every paragraph a leaf, every chord a branch",
  ),
)

// ── Ink drop (decorative, lower-left margin) ─────────────
// Big drop + a smaller one — suggests an inkwell off-page.
#place(
  bottom + left,
  dx: 110pt,
  dy: -220pt,
  circle(radius: 18pt, fill: ink_black),
)
#place(
  bottom + left,
  dx: 145pt,
  dy: -198pt,
  circle(radius: 7pt, fill: ink_black),
)
#place(
  bottom + left,
  dx: 160pt,
  dy: -180pt,
  circle(radius: 3pt, fill: ink_black),
)

// ── Quill (decorative, lower-right margin) ───────────────
// A diagonal line ending at a small filled-circle nib, with
// three feather ticks running parallel to the shaft.
#place(
  bottom + right,
  dx: -120pt,
  dy: -210pt,
  rotate(28deg,
    line(
      start: (0pt, 0pt),
      end:   (-130pt, -180pt),
      stroke: 2pt + ink_accent,
    ),
  ),
)
// Feather ticks
#place(
  bottom + right,
  dx: -200pt,
  dy: -350pt,
  rotate(28deg,
    line(
      start: (0pt, 0pt),
      end:   (20pt, -10pt),
      stroke: 1pt + ink_accent,
    ),
  ),
)
#place(
  bottom + right,
  dx: -190pt,
  dy: -360pt,
  rotate(28deg,
    line(
      start: (0pt, 0pt),
      end:   (20pt, -10pt),
      stroke: 1pt + ink_accent,
    ),
  ),
)
#place(
  bottom + right,
  dx: -180pt,
  dy: -370pt,
  rotate(28deg,
    line(
      start: (0pt, 0pt),
      end:   (20pt, -10pt),
      stroke: 1pt + ink_accent,
    ),
  ),
)
// Nib drop at the tip
#place(
  bottom + right,
  dx: -124pt,
  dy: -202pt,
  circle(radius: 5pt, fill: ink_black),
)

// ── Author + version block ───────────────────────────────
#place(
  bottom + center,
  dy: -116pt,
  block(
    width: 100%,
    align(center)[
      #line(length: 90pt, stroke: 0.5pt + ink_accent)
      #v(12pt)
      #text(
        size: 14pt,
        tracking: 5pt,
        fill: ink_black,
        upper("Vladimir Ulogov"),
      )
      #v(8pt)
      #text(
        size: 10pt,
        style: "italic",
        fill: ink_smoke,
        "and the inkhaven contributors",
      )
      #v(20pt)
      #text(
        size: 9pt,
        fill: ink_smoke,
        tracking: 3pt,
        "VERSION 1.2.6  ·  2026",
      )
    ],
  ),
)

// ── Bottom ornament row (mirrors the top) ────────────────
#let dot_b(dx, r) = place(
  bottom + center,
  dx: dx,
  dy: -54pt,
  circle(radius: r, fill: ink_accent),
)
#dot_b(-60pt, 5pt)
#dot_b(-30pt, 3.5pt)
#dot_b(  0pt, 7pt)
#dot_b( 30pt, 3.5pt)
#dot_b( 60pt, 5pt)
