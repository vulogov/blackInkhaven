#import "../design.typ": *

#chapter(number: 18, title: "Building the font")

You have glyphs. Now you *bind* each one to a sound and a character slot, and
compile them into a finished font file. Inkhaven does the whole compilation
itself — no font-editing software required — and the result is an ordinary
TrueType font you can install, embed in documents, and share.

#section("Binding a glyph")

To make a glyph part of your language's writing system, you *import* it: Inkhaven
preflights it (the lint check from last chapter), copies it into the language's
glyph store, and records which sound it represents and which character slot it
occupies.

```sh
inkhaven language font-import-glyph Eldar --svg ./a.svg --phoneme a --codepoint a
```

This binds the drawing `a.svg` to the phoneme /a/ and to the character "a". For a
sound that has an ordinary letter, you can use that letter as the codepoint, as
here. For invented symbols with no letter, you use a number from a special range
(explained next).

#term("Codepoint")[
  The unique number a computer uses to identify a character. Every character — "A",
  "ß", "あ", an emoji — has a codepoint in the *Unicode* standard, written like
  *U+0041*. A font maps codepoints to glyphs: when text contains a codepoint, the
  font supplies the matching drawn shape.
]

#term("The Private Use Area")[
  A block of Unicode codepoints (starting at *U+E000*) deliberately left
  unassigned, reserved for anyone to use for their own characters. This is exactly
  where your invented symbols belong — they are real, usable characters, but they
  do not collide with any standard ones. Bind a glyph with no natural letter to a
  Private Use codepoint like `U+E000`.
]

For an invented symbol, bind it to a Private Use codepoint:

```sh
inkhaven language font-import-glyph Eldar --svg ./sun.svg --phoneme o --codepoint U+E000 --name sun
```

#section("The font lives in your language")

These bindings are stored as a `font` block in the language — a record of the
family name, the design grid size, and every glyph with its codepoint and
phoneme. You do not have to write this by hand; `font-import-glyph` builds it for
you. To review what you have bound, with the status of each glyph's artwork:

```sh
inkhaven language font-config Eldar
```

This lists every glyph, its codepoint, the phoneme it stands for, and whether its
drawing is present and usable.

#section("Compiling the font")

When your glyphs are bound, compile them into a font file straight from the
language:

```sh
inkhaven language font-build --language Eldar --format ttf --out Eldar
```

This produces `Eldar.ttf`, a standard TrueType font. Inkhaven does the entire
compilation in-process: it converts each glyph's outline into font curves, lays
them out on the design grid, and assembles a complete font file — with no
external program. You can also ask for `ufo` (an editable font *source* you can
open in professional font editors) or `both`.

#term("TrueType and UFO")[
  *TrueType* (a `.ttf` file) is the everyday font format your computer already
  uses — ready to install and use immediately. *UFO* (Unified Font Object) is an
  editable font *source*, the working format of professional font editors like
  FontForge or Glyphs. `font-build` can produce either; use TTF to *use* the
  font, UFO to *keep editing* it.
]

#callout(label: "Using your font")[
  The `.ttf` file is a real font. Install it like any other to type your script
  in any program, or — as Part VII shows — let Inkhaven embed it directly in the
  PDF books it produces, so your dictionary shows each headword in its own native
  script. To preview the font in a document, compile a Typst file that uses it
  with `typst compile --font-path <folder> document.typ`.
]

#recap((
  [Bind each glyph to a *phoneme* and a *codepoint* with `font-import-glyph`.],
  [Ordinary sounds can use their letter; invented symbols use the *Private Use
   Area* (`U+E000` and up).],
  [Bindings live in a `font` block; `font-config` lists them with artwork status.],
  [`font-build --format ttf` compiles a real *TrueType* font in-process; `ufo`
   gives an editable source.],
))
