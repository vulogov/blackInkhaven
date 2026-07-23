#import "../design.typ": *

#chapter(number: 1, title: "Verse in Inkhaven")

A poem is not a paragraph of prose with the line breaks left in. It has structure that
prose does not — the line is a unit, the stanza is a unit, and both carry meaning that
would be destroyed by reflowing the text to fit the page. Before Inkhaven can measure
verse, it has to *hold* verse: know that this block is a line and that one a quatrain,
and preserve those boundaries all the way to the printed page. This chapter is about how
it does that, and how you tell it that a given passage is a poem at all.

#section("The verse paragraph family")

Inkhaven does not have a separate \"poetry project\" with its own hierarchy. A book of
verse is an ordinary Inkhaven book; poems live inside it as paragraphs of a special
kind. Every paragraph in Inkhaven carries a *structural type* — a short tag that says
what the paragraph #emph[is] — and poetry adds a family of them, all prefixed
`para:verse-`:

#table(
  columns: (auto, auto, 1fr),
  stroke: none,
  inset: (x: 6pt, y: 3pt),
  align: (left, center, left),
  table.header(
    text(weight: "bold", size: 9pt)[Structural type],
    text(weight: "bold", size: 9pt)[Glyph],
    text(weight: "bold", size: 9pt)[What it holds],
  ),
  table.hline(stroke: 0.5pt + ink_rule),
  [`para:verse-line`],        [‖], [a single line of verse],
  [`para:verse-stanza`],      [♩], [a stanza of any length — the workhorse],
  [`para:verse-couplet`],     [‗], [a two-line unit],
  [`para:verse-tercet`],      [⁚], [a three-line unit],
  [`para:verse-quatrain`],    [⁛], [a four-line unit],
  [`para:verse-translation`], [⇄], [a translated stanza, paired with its source],
)

#v(2mm)

The glyphs are not decoration — they are how the Tree and Outline panes show you the
shape of a poem at a glance. A sequence of `♩ ♩ ♩ ♩` in the outline is four stanzas; a
run of `⁛` is a poem built in quatrains. You are never guessing at structure you cannot
see.

#term("Structural type")[
  The tag on every Inkhaven paragraph naming its role — `para:heading`, `para:body`,
  `para:verse-stanza`, and so on. It is metadata, not markup: it travels with the
  paragraph in the store and drives how the paragraph is displayed, measured, and
  exported, without putting anything visible into your text.
]

Which one should you reach for? Ninety per cent of the time, `para:verse-stanza`. It
holds a stanza of any length and is what the measuring tools expect. The fixed-size
units — couplet, tercet, quatrain — earn their keep when a poem's form is built from
them (a poem *in* quatrains, a sonnet's closing couplet) and you want the outline to
say so. `para:verse-line` is for the rare case where a single line stands alone. And
`para:verse-translation` is the subject of Chapter 7.

#callout(label: "Line breaks are preserved")[
  Inside a verse paragraph, the newlines you type are the newlines that print. Inkhaven
  does not reflow verse the way it reflows prose. A line is a line; where you break it
  is where it breaks. This is the one place in the editor where whitespace is load-bearing.
]

#section("Declaring a form: the poem: block")

A stanza sitting on its own is just text with line breaks. What turns it into something
Inkhaven can *hold you to* is a declaration of intent — a statement that says \"this is
meant to be a sonnet, in iambic pentameter, rhyming ABAB CDCD EFEF GG.\" That
declaration is a *poem block*: a small HJSON record attached to a poem (as a sidecar, or
inline) that names its form.

Here is the block for a sonnet, exactly as `inkhaven poetry forms --form sonnet` prints
it — an HJSON record, one field per line:

```
poem: {
  // 14 lines of iambic pentameter (generic English scheme)
  form: sonnet
  metre: iambic
  feet: 5
  metre_tradition: accentual_syllabic
  rhyme_scheme: "ABAB CDCD EFEF GG"
  language: en
}
```

Every field is a promise you are making about the poem, and every measuring tool in
this book reads them:

- *`metre` and `feet`* — the accentual-syllabic target: `iambic` × 5 is iambic
  pentameter, the ten-syllable line. Chapter 3 scans against this.
- *`metre_tradition`* — which system of measure applies. `accentual_syllabic` (English,
  German), `syllabic` (French and the Japanese forms, counting syllables only), or
  `accentual` (counting stresses only). The engine measures the right thing for the tradition.
- *`rhyme_scheme`* — the pattern of line endings; matching letters must rhyme. Chapter 4
  checks it.
- *`stanzas` and `lines_per_stanza`* — the architecture the completion tools in Chapter 6
  hold you to. A form prints them only when it fixes them: a haiku's block carries
  `stanzas: 1` and `lines_per_stanza: 3`; the open sonnet block above leaves them unstated.

#subsection("Why declare it at all?")

Because measurement needs a target. \"Is this line the right length?\" has no answer
until you have said what length is right. The poem block is where you say it. Declare
iambic pentameter and a slipped foot becomes a *finding*; declare nothing and the same
line is just a line. The declaration is what turns a passive ruler into an active second
reader — and, crucially, you are the one who writes it. Inkhaven never guesses your
intent and never imposes a form. It measures only against the promises you chose to make.

#section("The forms library")

You rarely write a poem block from scratch, because Inkhaven ships a library of the
forms poets actually use. `inkhaven poetry forms` lists them:

```
$ inkhaven poetry forms
poetry forms — `--form <name> [--language en|ru|fr|de|es]` prints a poem: block:

  sonnet                  14 lines of iambic pentameter (generic English scheme)
  petrarchan_sonnet       Italian sonnet: octave (ABBAABBA) + sestet (CDECDE)
  shakespearean_sonnet    English sonnet: 3 quatrains + a couplet, volta near line 12
  haiku                   3 lines, 5-7-5 syllables, unrhymed
  senryuu                 haiku form (5-7-5) on human nature rather than the seasons
  tanka                   5 lines, 5-7-5-7-7 syllables, unrhymed
  ghazal                  autonomous couplets sharing a radif; a signature in the closing maqta
  villanelle              19 lines: 5 tercets + a quatrain, two refrains on a schedule
  pantoum                 interlocking quatrains: lines 2 and 4 recur as 1 and 3 of the next
  terza_rima             chained tercets: ABA BCB CDC …
  ottava_rima            8-line stanzas rhyming ABABABCC
  blank_verse             unrhymed iambic pentameter
  heroic_couplet          rhymed iambic-pentameter couplets (AA BB …)
  limerick                5 anapestic lines rhyming AABBA (long-long-short-short-long)
  ode                     elevated address; stanzaic but formally open
  elegy                   reflective lament; formally open, often iambic
  free_verse              no metre or rhyme — the Inner Poet observes tendencies only
  prose_poem              poetic prose without line breaks
```

There are eighteen in all — the fixed forms of the European tradition (sonnets, the
villanelle, terza and ottava rima), the syllabic forms of the Japanese (haiku, senryū,
tanka), the ghazal from the Persian and Urdu, and open forms (ode, elegy, free verse,
prose poem) for when you want the tools without the corset. To use one, print its block
and paste it onto your poem:

```
$ inkhaven poetry forms --form villanelle
poem: {
  // 19 lines: 5 tercets + a quatrain, two refrains on a schedule
  form: villanelle
  metre: iambic
  feet: 5
  metre_tradition: accentual_syllabic
  rhyme_scheme: "ABA ABA ABA ABA ABA ABAA"
  language: en
  stanzas: 6
}
```

#subsection("Forms speak five languages")

The `--language` flag does more than stamp a label. A form carries language-specific
prosodic defaults, because the *same* form behaves differently in a different tongue. Ask
for a sonnet in Russian —

```
$ inkhaven poetry forms --form sonnet --language ru
poem: {
  ...
  language: ru
  allow_pyrrhic: true
  require_final_stress: true
}
```

— and the block comes back with `allow_pyrrhic` and `require_final_stress` set, encoding
the conventions of the Russian iambic line (its tolerance for a stress-light foot, its
insistence on a stressed final syllable). This is the multilingual promise the whole
workbench keeps: a feature that only worked for English would be a feature that only
half-worked. Every form, every measure, is meant to hold in all five languages, and where
it cannot, the tool tells you rather than pretending.

#subsection("Rolling your own")

When no built-in fits — an invented form, a nonce stanza, a house style — scaffold a
custom block:

```
$ inkhaven poetry forms --new --name my-triolet
// A custom-form scaffold — edit the fields, then paste this into a
// `poem:` sidecar, or into .inkhaven/custom-forms.hjson to reuse it.
poem: {
  title: "my-triolet"
  form: custom
  metre: iambic
  feet: 5
  metre_tradition: accentual_syllabic
  rhyme_scheme: "ABAB"
  language: en
}
```

Edit the fields to describe your form, drop it on a poem, and every tool in this book
measures against it exactly as it would a built-in. The library is a starting point, not
a fence.

#recap((
  [A book of verse is an ordinary Inkhaven book; poems live in it as paragraphs of the
   `para:verse-*` family — stanza, couplet, tercet, quatrain, line, translation — each
   with a glyph that shows the poem's shape in the outline.],
  [`para:verse-stanza` (♩) is the workhorse; the fixed-size units earn their place when a
   poem is *built* from them and you want the structure visible.],
  [Line breaks inside a verse paragraph are load-bearing — Inkhaven preserves them
   instead of reflowing.],
  [A *poem block* declares a poem's intended form — metre, feet, tradition, rhyme scheme,
   architecture. Measurement needs this target; you write it, Inkhaven never guesses it.],
  [`inkhaven poetry forms` ships eighteen forms, localised to all five languages;
   `--new` scaffolds a custom one. Every measuring tool reads the block the same way.],
))
