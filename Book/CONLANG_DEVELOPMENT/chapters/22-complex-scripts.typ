#import "../design.typ": *

#chapter(number: 22, title: "Complex scripts and typing")

Not every writing system is a simple row of letters. Korean stacks letters into
square syllable blocks; Egyptian hieroglyphs pack signs into rectangular groups.
And once you have a font, you will want to *type* in it — to turn romanized text
into your script automatically. This chapter covers both: composing complex
units, and typing the script.

#section("Composed blocks")

Some scripts build one written unit out of several component glyphs arranged in
two dimensions — a Korean syllable square is a *lead* consonant, a *vowel*, and a
*tail* consonant placed in a grid. Inkhaven supports this with *spatial
templates*: named layouts that say where each component goes.

#term("Spatial template")[
  A named layout that divides a square into cells — *left/right*, *top/bottom*, a
  *2×2 quadrat*, *three stacked rows* — into which component glyphs are placed to
  build a composite character. Used for syllable-block scripts (like Hangul) and
  packed scripts (like Egyptian quadrats).
]

List the available templates, then compose a block by dropping a glyph into each
cell:

```sh
inkhaven language font-templates Eldar
inkhaven language font-compose Eldar --template lr \
    --name ka --codepoint U+AC00 --phoneme ka \
    --slot left=lead --slot right=vowel --yes
```

The built-in templates are `lr` (left/right), `tb` (top/bottom), `quad` (a 2×2
grid), and `stack3` (three rows). This *bakes* the two components into a single
new glyph — a precomposed block — that lives in the font alongside its parts.

(The example binds the block to `U+AC00`, the real Hangul syllable *가*, because
it is modelling a real script. For an invented script, give it a *Private Use
Area* codepoint instead, exactly as in the previous chapter.)

#section("Layout-time arrangement")

Baking every possible block into the font works for a script with a fixed set of
syllables, but not for one where signs combine freely and endlessly — like
hieroglyphs. For those, Inkhaven can arrange the components at *layout time*
instead, emitting a small Typst snippet that places each component in its cell
when the document is typeset:

```sh
inkhaven language spatial-typst Eldar --template tb \
    --name quadrat_sunbar --slot top=sun --slot bottom=bar
```

You drop the snippet into a Typst document, and the quadrat renders with the
glyphs stacked — no precomposed glyph required. This is the path for scripts where
the number of possible combinations is too large to bake.

#section("Typing the script")

Finally, you want to write *in* your script without hunting for codepoints. The
*transliterate* command turns romanized text into your script's characters, using
the phoneme bindings you made when building the font:

```sh
inkhaven language transliterate Eldar --text "katha"
```

It reads the romanized input left to right, matching the *longest* glyph key at
each step — so a two-letter key like `th` or `ka` wins over the single letters
`t` and `h` — and outputs the corresponding string of your script's codepoints.
Anything it cannot match passes through unchanged and is flagged, so you know
which sounds still need a glyph.

#term("Transliteration (input method)")[
  Converting text from one script into another — here, from the Latin letters you
  type into your invented script's characters. The rule "longest key wins"
  means a digraph (two letters standing for one sound, like `th`) is matched as a
  unit before its individual letters. This is the engine behind typing a
  constructed script.
]

#callout(label: "This is enough for a real script")[
  With glyphs, a compiled font, optional composed blocks, and transliteration,
  your language has a complete, usable writing system. The dictionary and grammar
  books of the next part will show each word in this native script, rendered with
  the very font you built.
]

#recap((
  [*Spatial templates* arrange component glyphs into composite units (Hangul
   squares, quadrats).],
  [`font-compose` *bakes* a composed block into the font; `spatial-typst`
   arranges components at *layout time* for open-ended scripts.],
  [`transliterate` types the script: romanized text → script codepoints, longest
   key first.],
  [Together these give a complete, typeable writing system rendered by your own
   font.],
))
