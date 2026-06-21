#import "../design.typ": *

#chapter(number: 20, title: "Designing glyphs")

So far your language has been written in ordinary letters. Now you can give it its
own *script* — an alphabet of shapes you design, compiled into a real, usable
*font* that renders your words in their native form. This is one of Inkhaven's
most striking features, and it does it entirely on its own, with no other
software. This part is optional and the most advanced; the language is complete
without it.

#term("Script and glyph")[
  A *script* is a system of written symbols for a language (the Latin script, the
  Cyrillic script, the Korean Hangul script). A *glyph* is one such symbol — one
  drawn shape. Designing a script means designing its glyphs, usually one per
  sound or per syllable.
]

#term("Font")[
  A file containing the drawn glyphs of a script in a form computers can display
  and print. When you "install a font" you are giving your system a set of glyph
  shapes tied to characters. Inkhaven compiles your glyph drawings into a
  standard *TrueType* font that any program can use.
]

#section("What a glyph file looks like")

Each glyph is a small *SVG* file — a vector drawing, the kind made of shapes and
curves rather than pixels, so it scales to any size cleanly. A glyph for the sound
/a/ might be a simple drawing saved as `a.svg`. You can draw glyphs in any vector
editor (Inkscape, Illustrator, and many free tools), or — as we will see — have
the AI draft them from a description.

#term("SVG (vector drawing)")[
  *Scalable Vector Graphics* — a way of describing a drawing as shapes and curves
  (a filled outline) rather than a grid of pixels. Because it is defined
  mathematically, it stays crisp at any size, which is exactly what a font needs.
  Each glyph is one SVG file.
]

#section("What makes a good font glyph")

Not every drawing works as a font glyph. A font glyph must be a *filled shape* —
a solid black outline — not a thin line, not a photo, not a coloured gradient. A
font is monochrome: it has one ink colour, so a glyph is defined purely by its
outline. Before you commit a glyph, check it with the *lint* command:

```sh
inkhaven language glyph-lint --svg ./a.svg
```

This reports whether the SVG is suitable: it requires filled paths, and flags
common mistakes — a stroke-only line (which has no fillable shape), an embedded
photo, a gradient, or a near-white fill where you probably meant to cut a hole.

#callout(label: "The white-fill trap")[
  A frequent beginner mistake is to draw the hole in a letter like "O" by
  painting a white circle over a black one. In a monochrome font this does *not*
  make a hole — both shapes become solid ink. The correct way to cut a hole (a
  *counter*) is to draw the inner outline wound in the opposite direction to the
  outer one. The lint command warns you when it sees a likely white-fill mistake.
]

#term("Counter")[
  An enclosed empty space inside a glyph — the hole in "O", "D", or "e". In a
  font, a counter is made by drawing the inner outline in the *reverse* direction
  to the outer outline, not by painting it white. The opposing directions tell
  the font where the ink is and where the hole is.
]

#section("Drawing a glyph with AI")

If you would rather describe a glyph than draw it, the AI can draft one for you.
You give a short description; it produces an SVG, runs it through the same lint
check, and shows you the result:

```sh
inkhaven language glyph-draft Eldar --describe "a vertical stroke with a hook" \
    --phoneme p --out p.svg
```

By default it only drafts and previews. The drawing is advisory — review it,
re-run with a better description if you like, and only when you are happy do you
keep it. (Adding `--yes`, and only if the draft passes the lint check, binds it
straight into your font, which the next chapter explains.) The AI is told to draw
filled, monochrome shapes and to cut counters the correct way, so its output is
usually ready to use.

#callout(label: "Hand-drawn and AI glyphs mix freely")[
  You can draw some glyphs yourself and have the AI draft others; both go through
  the same lint check and into the same font. A common workflow is to let the AI
  rough out a whole alphabet from descriptions, lint them all, and then redraw by
  hand the few you want to perfect.
]

#recap((
  [A *script* is made of *glyphs*; each is a filled *SVG* vector drawing, one per
   sound.],
  [A font glyph must be a *filled, monochrome shape*; `glyph-lint` checks
   suitability.],
  [Cut holes (*counters*) with reverse-wound outlines, never white fill — lint
   warns you.],
  [`glyph-draft --describe …` has the AI draft a glyph from words; advisory,
   lint-checked, kept only on your approval.],
))
