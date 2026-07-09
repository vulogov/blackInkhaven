#import "../design.typ": *

#chapter(number: 3, title: "Colours, type, and the look")

The default site is deliberately plain and warm — cream paper, earth-brown headings,
a serif face for reading. When you want it to look like _yours_, you change one file.
This chapter is about that file and the small language it is written in.

#section("What styling is")

The words of your book and the way those words _look_ are two separate things. The
same chapter can be set in any typeface, any colour, any width. That separation has a
name on the web.

#term("CSS")[
  _Cascading Style Sheets_ — the language that describes how a web page looks: its
  colours, its fonts, its spacing, its layout. HTML says _what_ the page contains; CSS
  says how it should _appear_. They are kept in separate files so you can restyle a
  site completely without touching a word of its content.
]

In your exported site, all of the look lives in one file: `theme.css`. Change it, and
the whole site changes with it.

#section("Getting your own copy")

You do not edit the site's `theme.css` directly — a fresh export would overwrite it.
Instead you edit your _source_ copy, the one Inkhaven builds from. Write the defaults
out first, as we saw in Chapter 2:

#run[```
inkhaven export html --eject-templates my-design
```]

Inside `my-design` you will find two folders — `functional` and `theme` — and the
file you want is `my-design/theme/theme.css`.

#term("The `functional` and `theme` split")[
  Inkhaven keeps its design files in two piles. `theme` is the _look_: the stylesheet
  and small visual pieces like the header and footer. `functional` is the _machinery_:
  the page skeleton and the navigation logic. You restyle by touching `theme` and
  leaving `functional` alone — so you can change every colour on the site without any
  risk of breaking how the chapters link together.
]

#section("The colours, in one place")

Open `theme/theme.css`. Near the top you will see a block that begins `:root {` and a
list of names that start with two dashes:

#config("theme/theme.css", [```css
:root {
  --bg:      #fbf6ea;   /* page background — warm cream */
  --text:    #1e1a15;   /* body text — near-black ink */
  --accent:  #8a5a2b;   /* headings and links — sienna */
  --code-bg: #f2ecdd;   /* the tint behind code */
}
```])

#term("A CSS variable")[
  A named colour or value, written `--name`, defined once and used everywhere the
  design refers to it. Change `--accent` in this one place and every heading and link
  on the site changes with it. The `#8a5a2b` is a colour written as a _hex code_ — a
  `#` followed by six characters standing for red, green, and blue.
]

Suppose you want a deep teal instead of the sienna accent. Change the one line:

#config("theme/theme.css", [```css
  --accent: #2f6668;   /* was #8a5a2b */
```])

Export with your design, and every heading and link across the whole site is now
teal. You changed one value; the "cascade" in the name did the rest.

#run[```
inkhaven export html -o site --templates my-design
```]

#note[
  The default theme already answers to light and dark screens: readers whose devices
  are set to dark mode get a dark version automatically. Those dark colours live a
  little further down the same file, in a block marked `prefers-color-scheme: dark`.
  Adjust them the same way — by changing the values.
]

#section("Type")

Just below the colours you will find the fonts:

#config("theme/theme.css", [```css
  --serif: "Iowan Old Style", Palatino, Georgia, serif;
  --mono:  "SFMono-Regular", Menlo, monospace;
```])

#term("A font stack")[
  A list of typefaces, best first. The reader's browser uses the first one their
  computer actually has, and falls back to the next. Ending with a general word like
  `serif` guarantees _something_ appropriate always shows. Listing common,
  already-installed fonts is what keeps the site self-contained — it never has to
  fetch a font from elsewhere.
]

To set the reading face to Georgia everywhere, put it first:

#config("theme/theme.css", [```css
  --serif: Georgia, "Times New Roman", serif;
```])

#pitfall[
  It is tempting to reach for a beautiful font hosted on the web and link to it. Resist
  it. The moment a page loads a font from someone else's server, your site is no
  longer self-contained — it breaks offline, and it depends on that server staying up
  forever. Style with fonts readers already have, and the site stays whole. If you
  truly need a special face, the safe path is to bundle the font file into your design
  folder yourself, not to link out.
]

#insight[
  Almost all the styling you will ever want is a matter of changing values that are
  already named for you at the top of `theme.css` — the background, the accent, the
  fonts. You rarely need to understand the rest of the file. Find the name, change the
  value, export, look. That loop is the whole craft of restyling.
]

#recap((
  [*CSS* describes how a page looks; all of your site's look lives in `theme.css`.],
  [Edit your ejected *source* copy under `theme/`, not the site's output copy.],
  [Colours and fonts are named *CSS variables* at the top of the file — change the value in one place, the whole site follows.],
  [Keep the site self-contained: style with fonts readers already have, or bundle a font file; never link to one on the web.],
))
